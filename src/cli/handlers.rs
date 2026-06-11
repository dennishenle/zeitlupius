use std::io::Write;

use jiff::tz::TimeZone;

use crate::cli::format::{fmt_hm, fmt_hms};
use crate::cli::{Command, IntervalArgs, ReportArgs};
use crate::error::{Error, Result};
use crate::model::{Interval, ProjectName};
use crate::ops;
use crate::storage::ProjectStore;

pub fn run<S: ProjectStore, W: Write>(
    store: &S,
    cmd: Command,
    out: &mut W,
    now: &jiff::Zoned,
    tz: &TimeZone,
) -> Result<()> {
    match cmd {
        Command::List => list(store, out),
        Command::Create { project } => {
            ops::create(store, &ProjectName::parse(&project)?)?;
            writeln!(out, "created '{project}'")?;
            Ok(())
        }
        Command::Delete { project, force: _ } => {
            ops::delete(store, &ProjectName::parse(&project)?)?;
            writeln!(out, "deleted '{project}'")?;
            Ok(())
        }
        Command::Start { project, note } => {
            ops::start(store, &ProjectName::parse(&project)?, now, note.as_deref())?;
            writeln!(out, "started '{project}' at {now}")?;
            Ok(())
        }
        Command::Stop { project } => {
            ops::stop(store, &ProjectName::parse(&project)?, now)?;
            writeln!(out, "stopped '{project}' at {now}")?;
            Ok(())
        }
        Command::Status { project } => {
            let only = match project {
                Some(p) => Some(ProjectName::parse(&p)?),
                None => None,
            };
            let entries = ops::status(store, only.as_ref(), now)?;
            if entries.is_empty() {
                writeln!(out, "no running timers")?;
            } else {
                for e in entries {
                    writeln!(
                        out,
                        "● {:<24} started {}   running {}",
                        e.name,
                        e.start,
                        fmt_hms(e.running_seconds)
                    )?;
                }
            }
            Ok(())
        }
        Command::Report(args) => report(store, args, out, now, tz),
        Command::Session(sub) => session(store, sub, out),
    }
}

fn list<S: ProjectStore, W: Write>(store: &S, out: &mut W) -> Result<()> {
    let names = ops::list(store)?;
    if names.is_empty() {
        return Ok(());
    }
    let now = jiff::Zoned::now();
    for n in names {
        let project = ops::load(store, &n)?;
        match project.running_session() {
            Some(s) => {
                let secs = (now.timestamp().as_second() - s.start.timestamp().as_second()).max(0);
                writeln!(out, "* {:<24} (running for {})", n, fmt_hms(secs))?;
            }
            None => writeln!(out, "  {n}")?,
        }
    }
    Ok(())
}

fn report<S: ProjectStore, W: Write>(
    store: &S,
    args: ReportArgs,
    out: &mut W,
    now: &jiff::Zoned,
    tz: &TimeZone,
) -> Result<()> {
    let only = match args.project {
        Some(p) => Some(ProjectName::parse(&p)?),
        None => None,
    };
    let interval = build_interval(&args.interval, args.page, now)?;
    let rep = ops::report(store, only.as_ref(), interval, now, tz)?;
    if args.json {
        write!(out, "{{\"interval\":{:?},\"rows\":[", rep.interval.label())?;
        for (i, r) in rep.rows.iter().enumerate() {
            if i > 0 {
                write!(out, ",")?;
            }
            write!(
                out,
                "{{\"project\":{:?},\"seconds\":{}}}",
                r.name.as_str(),
                r.seconds
            )?;
        }
        writeln!(out, "],\"total_seconds\":{}}}", rep.total_seconds)?;
    } else {
        writeln!(out, "{}", rep.interval.label())?;
        let name_width = rep
            .rows
            .iter()
            .map(|r| r.name.as_str().len())
            .chain(std::iter::once("TOTAL".len()))
            .max()
            .unwrap_or("TOTAL".len());
        let time_width = rep
            .rows
            .iter()
            .map(|r| fmt_hm(r.seconds).len())
            .chain(std::iter::once(fmt_hm(rep.total_seconds).len()))
            .max()
            .unwrap_or_else(|| fmt_hm(rep.total_seconds).len());
        for r in &rep.rows {
            writeln!(
                out,
                "  {:<name_width$}  {:>time_width$}",
                r.name.as_str(),
                fmt_hm(r.seconds)
            )?;
        }
        writeln!(
            out,
            "  {:<name_width$}  {:>time_width$}",
            "TOTAL",
            fmt_hm(rep.total_seconds)
        )?;
    }
    Ok(())
}

fn session<S: ProjectStore, W: Write>(
    store: &S,
    sub: crate::cli::SessionCmd,
    out: &mut W,
) -> Result<()> {
    use crate::cli::SessionCmd;
    use crate::model::SessionId;
    match sub {
        SessionCmd::List { project, json } => {
            let name = ProjectName::parse(&project)?;
            let sessions = ops::list_sessions(store, &name)?;
            if json {
                write!(out, "[")?;
                for (i, s) in sessions.iter().enumerate() {
                    if i > 0 {
                        write!(out, ",")?;
                    }
                    let stop_json = match &s.stop {
                        Some(z) => format!("\"{z}\""),
                        None => "null".into(),
                    };
                    let dur = match &s.stop {
                        Some(z) => z.timestamp().as_second() - s.start.timestamp().as_second(),
                        None => 0,
                    };
                    let note_json = match &s.note {
                        Some(n) => format!("\"{}\"", n.replace('\\', "\\\\").replace('"', "\\\"")),
                        None => "null".into(),
                    };
                    write!(
                        out,
                        "{{\"id\":\"{}\",\"start\":\"{}\",\"stop\":{},\"duration_seconds\":{},\"note\":{}}}",
                        s.id, s.start, stop_json, dur, note_json
                    )?;
                }
                writeln!(out, "]")?;
            } else {
                writeln!(
                    out,
                    "ID        START                  STOP                   DURATION  NOTE"
                )?;
                for s in &sessions {
                    let stop = s
                        .stop
                        .as_ref()
                        .map(|z| z.strftime("%d.%m.%Y %H:%M:%S").to_string())
                        .unwrap_or_else(|| "running                ".to_string());
                    let dur_secs = match &s.stop {
                        Some(z) => z.timestamp().as_second() - s.start.timestamp().as_second(),
                        None => 0,
                    };
                    writeln!(
                        out,
                        "{}  {}    {}    {}  {}",
                        s.id,
                        s.start.strftime("%d.%m.%Y %H:%M:%S"),
                        stop,
                        fmt_hms(dur_secs),
                        s.note.as_deref().unwrap_or(""),
                    )?;
                }
            }
            Ok(())
        }
        SessionCmd::Delete {
            project,
            id,
            force: _,
        } => {
            let name = ProjectName::parse(&project)?;
            let sid = SessionId::parse(&id)?;
            let removed = ops::delete_session(store, &name, &sid)?;
            let stop = removed
                .stop
                .as_ref()
                .map(|z| z.to_string())
                .unwrap_or_else(|| "running".into());
            let dur = match &removed.stop {
                Some(z) => z.timestamp().as_second() - removed.start.timestamp().as_second(),
                None => 0,
            };
            writeln!(
                out,
                "deleted {} from '{}' ({} → {}, {})",
                removed.id,
                name,
                removed.start,
                stop,
                fmt_hms(dur)
            )?;
            Ok(())
        }
        SessionCmd::SetNote { project, id, note } => {
            let name = ProjectName::parse(&project)?;
            let sid = SessionId::parse(&id)?;
            ops::update_note(store, &name, &sid, note)?;
            writeln!(out, "updated note for session {id} in '{project}'")?;
            Ok(())
        }
    }
}

fn build_interval(args: &IntervalArgs, page: i32, now: &jiff::Zoned) -> Result<Interval> {
    let today = now.date();
    if let (Some(f), Some(t)) = (args.from.as_deref(), args.to.as_deref()) {
        let from = parse_date(f)?;
        let to = parse_date(t)?;
        if from > to {
            return Err(Error::InvalidRange {
                from: f.to_string(),
                to: t.to_string(),
            });
        }
        return Ok(Interval::Custom { from, to });
    }
    let mut iv = if args.week {
        Interval::Week(today)
    } else if args.month {
        Interval::Month(today)
    } else if args.year {
        Interval::Year(today.year())
    } else {
        Interval::Day(today)
    };
    if page < 0 {
        for _ in 0..(-page) {
            iv = iv.previous();
        }
    } else if page > 0 {
        for _ in 0..page {
            iv = iv.next();
        }
    }
    Ok(iv)
}

fn parse_date(s: &str) -> Result<jiff::civil::Date> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidDate(s.into()));
    }
    let dd: i8 = parts[0].parse().map_err(|_| Error::InvalidDate(s.into()))?;
    let mm: i8 = parts[1].parse().map_err(|_| Error::InvalidDate(s.into()))?;
    let yyyy: i16 = parts[2].parse().map_err(|_| Error::InvalidDate(s.into()))?;
    jiff::civil::Date::new(yyyy, mm, dd).map_err(|_| Error::InvalidDate(s.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::SessionCmd;
    use crate::storage::mem::MemStore;
    use jiff::{civil::date, tz::TimeZone};

    fn z(h: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn list_empty_outputs_nothing() {
        let s = MemStore::new();
        let mut buf = Vec::new();
        run(&s, Command::List, &mut buf, &z(12), &TimeZone::UTC).unwrap();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn create_then_list_shows_project() {
        let s = MemStore::new();
        let mut buf = Vec::new();
        run(
            &s,
            Command::Create {
                project: "p".into(),
            },
            &mut buf,
            &z(12),
            &TimeZone::UTC,
        )
        .unwrap();
        buf.clear();
        run(&s, Command::List, &mut buf, &z(12), &TimeZone::UTC).unwrap();
        let txt = String::from_utf8(buf).unwrap();
        assert!(txt.contains("p"));
    }

    #[test]
    fn report_day_sums() {
        let s = MemStore::new();
        let mut sink = Vec::new();
        run(
            &s,
            Command::Create {
                project: "p".into(),
            },
            &mut sink,
            &z(8),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Start {
                project: "p".into(),
                note: None,
            },
            &mut sink,
            &z(9),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Stop {
                project: "p".into(),
            },
            &mut sink,
            &z(10),
            &TimeZone::UTC,
        )
        .unwrap();
        let mut buf = Vec::new();
        run(
            &s,
            Command::Report(ReportArgs {
                project: None,
                interval: IntervalArgs {
                    day: true,
                    week: false,
                    month: false,
                    year: false,
                    from: None,
                    to: None,
                },
                page: 0,
                json: false,
            }),
            &mut buf,
            &z(23),
            &TimeZone::UTC,
        )
        .unwrap();
        let txt = String::from_utf8(buf).unwrap();
        assert!(txt.contains("1:00"));
        assert!(txt.contains("TOTAL"));
    }

    #[test]
    fn report_aligns_time_column() {
        let s = MemStore::new();
        let mut sink = Vec::new();

        for project in ["short", "longer-name"] {
            run(
                &s,
                Command::Create {
                    project: project.into(),
                },
                &mut sink,
                &z(8),
                &TimeZone::UTC,
            )
            .unwrap();
        }

        run(
            &s,
            Command::Start {
                project: "short".into(),
                note: None,
            },
            &mut sink,
            &z(8),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Stop {
                project: "short".into(),
            },
            &mut sink,
            &z(9),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Start {
                project: "longer-name".into(),
                note: None,
            },
            &mut sink,
            &z(9),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Stop {
                project: "longer-name".into(),
            },
            &mut sink,
            &z(12),
            &TimeZone::UTC,
        )
        .unwrap();

        let mut buf = Vec::new();
        run(
            &s,
            Command::Report(ReportArgs {
                project: None,
                interval: IntervalArgs {
                    day: true,
                    week: false,
                    month: false,
                    year: false,
                    from: None,
                    to: None,
                },
                page: 0,
                json: false,
            }),
            &mut buf,
            &z(23),
            &TimeZone::UTC,
        )
        .unwrap();

        let txt = String::from_utf8(buf).unwrap();
        let time_columns: Vec<_> = txt
            .lines()
            .skip(1)
            .map(|line| line.find(':').unwrap())
            .collect();

        assert!(time_columns.windows(2).all(|cols| cols[0] == cols[1]));
    }

    #[test]
    fn parse_date_rejects_garbage() {
        assert!(parse_date("nope").is_err());
        assert!(parse_date("31.02.2026").is_err());
        assert_eq!(parse_date("18.04.2024").unwrap().day(), 18);
    }

    #[test]
    fn session_list_shows_id_and_duration() {
        let s = MemStore::new();
        let mut sink = Vec::new();
        run(
            &s,
            Command::Create {
                project: "p".into(),
            },
            &mut sink,
            &z(8),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Start {
                project: "p".into(),
                note: None,
            },
            &mut sink,
            &z(9),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Stop {
                project: "p".into(),
            },
            &mut sink,
            &z(10),
            &TimeZone::UTC,
        )
        .unwrap();

        let mut buf = Vec::new();
        run(
            &s,
            Command::Session(SessionCmd::List {
                project: "p".into(),
                json: false,
            }),
            &mut buf,
            &z(23),
            &TimeZone::UTC,
        )
        .unwrap();
        let txt = String::from_utf8(buf).unwrap();
        assert!(txt.contains("ID"), "header missing: {txt}");
        assert!(txt.contains("1:00:00"), "duration missing: {txt}");
    }

    #[test]
    fn session_list_json_is_an_array() {
        let s = MemStore::new();
        let mut sink = Vec::new();
        run(
            &s,
            Command::Create {
                project: "p".into(),
            },
            &mut sink,
            &z(8),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Start {
                project: "p".into(),
                note: None,
            },
            &mut sink,
            &z(9),
            &TimeZone::UTC,
        )
        .unwrap();
        run(
            &s,
            Command::Stop {
                project: "p".into(),
            },
            &mut sink,
            &z(10),
            &TimeZone::UTC,
        )
        .unwrap();

        let mut buf = Vec::new();
        run(
            &s,
            Command::Session(SessionCmd::List {
                project: "p".into(),
                json: true,
            }),
            &mut buf,
            &z(23),
            &TimeZone::UTC,
        )
        .unwrap();
        let txt = String::from_utf8(buf).unwrap();
        assert!(txt.starts_with('['), "json must be an array: {txt}");
        assert!(
            txt.contains("\"duration_seconds\":3600"),
            "json missing duration: {txt}"
        );
    }
}
