use crate::model::{Interval, Session};

pub fn session_overlap_seconds(
    session: &Session,
    interval: &Interval,
    now: &jiff::Zoned,
    tz: &jiff::tz::TimeZone,
) -> i64 {
    let s = &session.start;
    let e = session.stop.as_ref().unwrap_or(now);
    if e <= s {
        return 0;
    }
    let (lo, hi) = interval.bounds(tz);
    let start = if s > lo { s.clone() } else { lo };
    let end = if e < hi { e.clone() } else { hi };
    if end <= start {
        return 0;
    }
    end.timestamp().as_second() - start.timestamp().as_second()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProjectName;
    use jiff::{civil::date, tz::TimeZone};

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }
    fn closed(start: jiff::Zoned, stop: jiff::Zoned) -> Session {
        Session {
            start,
            stop: Some(stop),
            note: None,
        }
    }
    fn open(start: jiff::Zoned) -> Session {
        Session {
            start,
            stop: None,
            note: None,
        }
    }

    #[test]
    fn entirely_inside() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 4, 9, 0), z(2026, 5, 4, 11, 0));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 4, 23, 0), &TimeZone::UTC),
            2 * 3600,
        );
    }

    #[test]
    fn no_overlap_returns_zero() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 5, 0, 0), z(2026, 5, 5, 1, 0));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 5, 23, 0), &TimeZone::UTC),
            0,
        );
    }

    #[test]
    fn partial_at_start() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 3, 23, 0), z(2026, 5, 4, 1, 30));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 4, 23, 0), &TimeZone::UTC),
            5400,
        );
    }

    #[test]
    fn partial_at_end() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = closed(z(2026, 5, 4, 23, 30), z(2026, 5, 5, 0, 30));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 5, 23, 0), &TimeZone::UTC),
            1800,
        );
    }

    #[test]
    fn open_session_uses_now() {
        let i = Interval::Day(date(2026, 5, 4));
        let s = open(z(2026, 5, 4, 22, 0));
        assert_eq!(
            session_overlap_seconds(&s, &i, &z(2026, 5, 4, 22, 30), &TimeZone::UTC),
            1800,
        );
    }

    #[test]
    fn dst_aware_25_hour_day() {
        let tz = TimeZone::get("Europe/Berlin").unwrap();
        let i = Interval::Day(date(2026, 10, 25));
        let s = Session {
            start: date(2026, 10, 25)
                .at(0, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap(),
            stop: Some(
                date(2026, 10, 26)
                    .at(0, 0, 0, 0)
                    .to_zoned(tz.clone())
                    .unwrap(),
            ),
            note: None,
        };
        let now = date(2026, 10, 26)
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap();
        assert_eq!(session_overlap_seconds(&s, &i, &now, &tz), 25 * 3600);
        assert!(ProjectName::parse("a").is_ok());
    }
}
