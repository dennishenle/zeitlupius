use crate::model::{Interval, Project, Session};
use crate::time::session_overlap_seconds;

pub fn sessions_in<'a>(
    project: &'a Project,
    interval: &Interval,
    now: &jiff::Zoned,
    tz: &jiff::tz::TimeZone,
) -> Vec<&'a Session> {
    let mut out: Vec<&Session> = project
        .sessions
        .iter()
        .filter(|s| s.stop.is_some())
        .filter(|s| session_overlap_seconds(s, interval, now, tz) > 0)
        .collect();
    out.sort_by(|a, b| b.start.cmp(&a.start));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectName, SessionId};
    use jiff::{civil::date, tz::TimeZone};

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }
    fn closed(start: jiff::Zoned, stop: jiff::Zoned) -> Session {
        Session {
            id: SessionId::generate(),
            start,
            stop: Some(stop),
            note: None,
        }
    }
    fn open(start: jiff::Zoned) -> Session {
        Session {
            id: SessionId::generate(),
            start,
            stop: None,
            note: None,
        }
    }
    fn project_with(sessions: Vec<Session>) -> Project {
        Project {
            name: ProjectName::parse("p").unwrap(),
            sessions,
        }
    }

    #[test]
    fn empty_project_returns_empty() {
        let p = project_with(vec![]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn session_entirely_inside_is_included() {
        let p = project_with(vec![closed(z(2026, 5, 4, 9, 0), z(2026, 5, 4, 10, 0))]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, z(2026, 5, 4, 9, 0));
    }

    #[test]
    fn session_outside_interval_excluded() {
        let p = project_with(vec![closed(z(2026, 5, 3, 9, 0), z(2026, 5, 3, 10, 0))]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn session_straddling_start_included() {
        let p = project_with(vec![closed(z(2026, 5, 3, 23, 0), z(2026, 5, 4, 1, 0))]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, z(2026, 5, 3, 23, 0));
    }

    #[test]
    fn session_straddling_end_included() {
        let p = project_with(vec![closed(z(2026, 5, 4, 23, 30), z(2026, 5, 5, 0, 30))]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 5, 12, 0),
            &TimeZone::UTC,
        );
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn running_session_excluded_even_if_intersects() {
        let p = project_with(vec![open(z(2026, 5, 4, 22, 0))]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 22, 30),
            &TimeZone::UTC,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn sorted_by_start_descending() {
        let p = project_with(vec![
            closed(z(2026, 5, 4, 8, 0), z(2026, 5, 4, 9, 0)),
            closed(z(2026, 5, 4, 14, 0), z(2026, 5, 4, 15, 0)),
            closed(z(2026, 5, 4, 10, 0), z(2026, 5, 4, 11, 0)),
        ]);
        let out = sessions_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        let starts: Vec<_> = out.iter().map(|s| s.start.clone()).collect();
        assert_eq!(
            starts,
            vec![
                z(2026, 5, 4, 14, 0),
                z(2026, 5, 4, 10, 0),
                z(2026, 5, 4, 8, 0),
            ]
        );
    }
}
