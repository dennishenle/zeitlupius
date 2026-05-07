use crate::model::{Interval, Project};
use crate::time::session_overlap_seconds;

pub fn project_total_in(
    project: &Project,
    interval: &Interval,
    now: &jiff::Zoned,
    tz: &jiff::tz::TimeZone,
) -> i64 {
    project
        .sessions
        .iter()
        .map(|s| session_overlap_seconds(s, interval, now, tz))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectName, Session};
    use jiff::{civil::date, tz::TimeZone};

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn sums_sessions_within_interval() {
        let p = Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![
                Session {
                    start: z(2026, 5, 4, 9, 0),
                    stop: Some(z(2026, 5, 4, 10, 0)),
                    note: None,
                },
                Session {
                    start: z(2026, 5, 4, 14, 0),
                    stop: Some(z(2026, 5, 4, 15, 30)),
                    note: None,
                },
            ],
        };
        let total = project_total_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(total, 3600 + 5400);
    }

    #[test]
    fn ignores_sessions_outside_interval() {
        let p = Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                start: z(2026, 5, 3, 9, 0),
                stop: Some(z(2026, 5, 3, 10, 0)),
                note: None,
            }],
        };
        let total = project_total_in(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(total, 0);
    }
}
