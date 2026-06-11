use jiff::tz::TimeZone;

use crate::error::Result;
use crate::model::{Interval, Project, ProjectName};
use crate::storage::ProjectStore;
use crate::time::project_total_in;

pub fn create<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<()> {
    store.create(name)
}

pub fn delete<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<()> {
    store.delete(name)
}

pub fn list_sessions<S: ProjectStore>(
    store: &S,
    name: &ProjectName,
) -> Result<Vec<crate::model::Session>> {
    Ok(store.load(name)?.sessions)
}

pub fn delete_session<S: ProjectStore>(
    store: &S,
    name: &ProjectName,
    id: &crate::model::SessionId,
) -> Result<crate::model::Session> {
    store.delete_session(name, id)
}

pub fn update_note<S: ProjectStore>(
    store: &S,
    name: &ProjectName,
    id: &crate::model::SessionId,
    note: Option<String>,
) -> Result<()> {
    store.update_note(name, id, note)
}

pub fn list<S: ProjectStore>(store: &S) -> Result<Vec<ProjectName>> {
    store.list()
}

pub fn start<S: ProjectStore>(
    store: &S,
    name: &ProjectName,
    now: &jiff::Zoned,
    note: Option<&str>,
) -> Result<()> {
    store.append_start(name, now, note)
}

pub fn stop<S: ProjectStore>(store: &S, name: &ProjectName, now: &jiff::Zoned) -> Result<()> {
    store.close_open(name, now)
}

pub fn load<S: ProjectStore>(store: &S, name: &ProjectName) -> Result<Project> {
    store.load(name)
}

#[derive(Debug, Clone)]
pub struct Row {
    pub name: ProjectName,
    pub seconds: i64,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub interval: Interval,
    pub rows: Vec<Row>,
    pub total_seconds: i64,
}

pub fn report<S: ProjectStore>(
    store: &S,
    only: Option<&ProjectName>,
    interval: Interval,
    now: &jiff::Zoned,
    tz: &TimeZone,
) -> Result<Report> {
    let names: Vec<ProjectName> = match only {
        Some(n) => vec![n.clone()],
        None => store.list()?,
    };
    let mut rows = Vec::new();
    let mut total = 0i64;
    for n in names {
        let p = store.load(&n)?;
        let s = project_total_in(&p, &interval, now, tz);
        total += s;
        rows.push(Row {
            name: n,
            seconds: s,
        });
    }
    Ok(Report {
        interval,
        rows,
        total_seconds: total,
    })
}

#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub name: ProjectName,
    pub start: jiff::Zoned,
    pub running_seconds: i64,
}

pub fn status<S: ProjectStore>(
    store: &S,
    only: Option<&ProjectName>,
    now: &jiff::Zoned,
) -> Result<Vec<StatusEntry>> {
    let names: Vec<ProjectName> = match only {
        Some(n) => vec![n.clone()],
        None => store.list()?,
    };
    let mut out = Vec::new();
    for n in names {
        let p = store.load(&n)?;
        if let Some(s) = p.running_session() {
            let secs = if now > s.start {
                now.timestamp().as_second() - s.start.timestamp().as_second()
            } else {
                0
            };
            out.push(StatusEntry {
                name: p.name.clone(),
                start: s.start.clone(),
                running_seconds: secs,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::mem::MemStore;
    use jiff::{civil::date, tz::TimeZone};

    fn z(h: i8, min: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn full_lifecycle() {
        let s = MemStore::new();
        let n = ProjectName::parse("rust-zlp").unwrap();
        create(&s, &n).unwrap();
        start(&s, &n, &z(9, 0), Some("note")).unwrap();
        stop(&s, &n, &z(10, 30)).unwrap();
        let r = report(
            &s,
            Some(&n),
            Interval::Day(date(2026, 5, 4)),
            &z(23, 0),
            &TimeZone::UTC,
        )
        .unwrap();
        assert_eq!(r.total_seconds, 5400);
    }

    #[test]
    fn status_lists_only_running() {
        let s = MemStore::new();
        let a = ProjectName::parse("a").unwrap();
        let b = ProjectName::parse("b").unwrap();
        create(&s, &a).unwrap();
        create(&s, &b).unwrap();
        start(&s, &a, &z(9, 0), None).unwrap();
        let st = status(&s, None, &z(9, 30)).unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].name.as_str(), "a");
        assert_eq!(st[0].running_seconds, 1800);
    }

    #[test]
    fn list_sessions_returns_in_storage_order() {
        let s = MemStore::new();
        let n = ProjectName::parse("p").unwrap();
        create(&s, &n).unwrap();
        start(&s, &n, &z(9, 0), None).unwrap();
        stop(&s, &n, &z(10, 0)).unwrap();
        start(&s, &n, &z(11, 0), None).unwrap();
        let sessions = list_sessions(&s, &n).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions[0].stop.is_some());
        assert!(sessions[1].stop.is_none());
    }

    #[test]
    fn delete_session_returns_the_removed_row() {
        let s = MemStore::new();
        let n = ProjectName::parse("p").unwrap();
        create(&s, &n).unwrap();
        start(&s, &n, &z(9, 0), None).unwrap();
        stop(&s, &n, &z(10, 0)).unwrap();
        let id = list_sessions(&s, &n).unwrap()[0].id.clone();
        let removed = delete_session(&s, &n, &id).unwrap();
        assert_eq!(removed.id, id);
        assert!(list_sessions(&s, &n).unwrap().is_empty());
    }
}
