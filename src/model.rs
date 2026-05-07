use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectName(String);

const RESERVED: &[&str] = &[
    "list", "start", "stop", "status", "create", "delete", "report",
];
const MAX_LEN: usize = 64;

impl ProjectName {
    pub fn parse(raw: &str) -> Result<Self> {
        if raw.is_empty() {
            return Err(Error::InvalidProjectName(raw.into(), "name is empty"));
        }
        if raw.len() > MAX_LEN {
            return Err(Error::InvalidProjectName(
                raw.into(),
                "name longer than 64 chars",
            ));
        }
        if raw.starts_with('.') {
            return Err(Error::InvalidProjectName(
                raw.into(),
                "name cannot start with '.'",
            ));
        }
        if RESERVED.contains(&raw) {
            return Err(Error::InvalidProjectName(
                raw.into(),
                "name collides with a reserved CLI verb",
            ));
        }
        for ch in raw.chars() {
            let ok = ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.');
            if !ok {
                return Err(Error::InvalidProjectName(
                    raw.into(),
                    "only ASCII letters, digits, '-', '_', '.' allowed",
                ));
            }
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub start: jiff::Zoned,
    pub stop: Option<jiff::Zoned>,
    pub note: Option<String>,
}

impl Session {
    pub fn is_running(&self) -> bool {
        self.stop.is_none()
    }

    pub fn duration(&self, now: &jiff::Zoned) -> jiff::Span {
        let end = self.stop.as_ref().unwrap_or(now);
        if end < &self.start {
            return jiff::Span::new();
        }
        end.since((jiff::Unit::Minute, &self.start))
            .unwrap_or_else(|_| jiff::Span::new())
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub name: ProjectName,
    pub sessions: Vec<Session>,
}

impl Project {
    pub fn running_session(&self) -> Option<&Session> {
        self.sessions.last().filter(|s| s.is_running())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interval {
    Day(jiff::civil::Date),
    Week(jiff::civil::Date),
    Month(jiff::civil::Date),
    Year(i16),
    Custom {
        from: jiff::civil::Date,
        to: jiff::civil::Date,
    },
}

impl Interval {
    pub fn bounds(&self, tz: &jiff::tz::TimeZone) -> (jiff::Zoned, jiff::Zoned) {
        use jiff::civil::{Weekday, date};
        let (lo_d, hi_d) = match self {
            Interval::Day(d) => (*d, d.tomorrow().expect("date in range")),
            Interval::Week(d) => {
                let dow: i64 = match d.weekday() {
                    Weekday::Monday => 0,
                    Weekday::Tuesday => 1,
                    Weekday::Wednesday => 2,
                    Weekday::Thursday => 3,
                    Weekday::Friday => 4,
                    Weekday::Saturday => 5,
                    Weekday::Sunday => 6,
                };
                let mon = d.checked_sub(jiff::Span::new().days(dow)).expect("monday");
                let next_mon = mon
                    .checked_add(jiff::Span::new().days(7))
                    .expect("next monday");
                (mon, next_mon)
            }
            Interval::Month(d) => {
                let first = date(d.year(), d.month(), 1);
                let next_first = if d.month() == 12 {
                    date(d.year() + 1, 1, 1)
                } else {
                    date(d.year(), d.month() + 1, 1)
                };
                (first, next_first)
            }
            Interval::Year(y) => (date(*y, 1, 1), date(*y + 1, 1, 1)),
            Interval::Custom { from, to } => {
                let next = to.tomorrow().expect("date in range");
                (*from, next)
            }
        };
        let lo = lo_d.at(0, 0, 0, 0).to_zoned(tz.clone()).expect("midnight");
        let hi = hi_d.at(0, 0, 0, 0).to_zoned(tz.clone()).expect("midnight");
        (lo, hi)
    }

    pub fn contains(&self, t: &jiff::Zoned, tz: &jiff::tz::TimeZone) -> bool {
        let (lo, hi) = self.bounds(tz);
        t >= &lo && t < &hi
    }

    pub fn previous(&self) -> Self {
        match self {
            Interval::Day(d) => Interval::Day(d.yesterday().expect("date")),
            Interval::Week(d) => {
                Interval::Week(d.checked_sub(jiff::Span::new().days(7)).expect("date"))
            }
            Interval::Month(d) => {
                let (y, m) = if d.month() == 1 {
                    (d.year() - 1, 12)
                } else {
                    (d.year(), d.month() - 1)
                };
                Interval::Month(jiff::civil::date(y, m, 1))
            }
            Interval::Year(y) => Interval::Year(*y - 1),
            Interval::Custom { from, to } => {
                let len_days = to.since(*from).expect("range").get_days() + 1;
                let span = jiff::Span::new().days(len_days);
                Interval::Custom {
                    from: from.checked_sub(span).expect("date"),
                    to: to.checked_sub(span).expect("date"),
                }
            }
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Interval::Day(d) => Interval::Day(d.tomorrow().expect("date")),
            Interval::Week(d) => {
                Interval::Week(d.checked_add(jiff::Span::new().days(7)).expect("date"))
            }
            Interval::Month(d) => {
                let (y, m) = if d.month() == 12 {
                    (d.year() + 1, 1)
                } else {
                    (d.year(), d.month() + 1)
                };
                Interval::Month(jiff::civil::date(y, m, 1))
            }
            Interval::Year(y) => Interval::Year(*y + 1),
            Interval::Custom { from, to } => {
                let len_days = to.since(*from).expect("range").get_days() + 1;
                let span = jiff::Span::new().days(len_days);
                Interval::Custom {
                    from: from.checked_add(span).expect("date"),
                    to: to.checked_add(span).expect("date"),
                }
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            Interval::Day(d) => format!("{:02}.{:02}.{}", d.day(), d.month(), d.year()),
            Interval::Week(d) => {
                use jiff::civil::Weekday;
                let dow: i64 = match d.weekday() {
                    Weekday::Monday => 0,
                    Weekday::Tuesday => 1,
                    Weekday::Wednesday => 2,
                    Weekday::Thursday => 3,
                    Weekday::Friday => 4,
                    Weekday::Saturday => 5,
                    Weekday::Sunday => 6,
                };
                let mon = d.checked_sub(jiff::Span::new().days(dow)).expect("monday");
                let sun = mon.checked_add(jiff::Span::new().days(6)).expect("sun");
                let iso = d.iso_week_date();
                format!(
                    "CW {} · {:02}.{:02}–{:02}.{:02}.{}",
                    iso.week(),
                    mon.day(),
                    mon.month(),
                    sun.day(),
                    sun.month(),
                    sun.year(),
                )
            }
            Interval::Month(d) => {
                let names = [
                    "January",
                    "February",
                    "March",
                    "April",
                    "May",
                    "June",
                    "July",
                    "August",
                    "September",
                    "October",
                    "November",
                    "December",
                ];
                format!("{} {}", names[(d.month() - 1) as usize], d.year())
            }
            Interval::Year(y) => y.to_string(),
            Interval::Custom { from, to } => format!(
                "Custom · {:02}.{:02}.{} – {:02}.{:02}.{}",
                from.day(),
                from.month(),
                from.year(),
                to.day(),
                to.month(),
                to.year(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::{civil::date, tz::TimeZone};

    #[test]
    fn accepts_valid_names() {
        for ok in ["rust-zlp", "side_project", "admin", "v1.0", "a"] {
            assert!(ProjectName::parse(ok).is_ok(), "{ok} should be valid");
        }
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            ProjectName::parse(""),
            Err(Error::InvalidProjectName(_, _))
        ));
    }

    #[test]
    fn rejects_too_long() {
        let n = "a".repeat(65);
        assert!(ProjectName::parse(&n).is_err());
    }

    #[test]
    fn rejects_leading_dot() {
        assert!(ProjectName::parse(".hidden").is_err());
    }

    #[test]
    fn rejects_reserved_verbs() {
        for v in [
            "list", "start", "stop", "status", "create", "delete", "report",
        ] {
            assert!(ProjectName::parse(v).is_err(), "{v} should be reserved");
        }
    }

    #[test]
    fn rejects_invalid_chars() {
        for bad in ["with space", "slash/name", "ümlaut", "name!"] {
            assert!(ProjectName::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> jiff::Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn session_running_means_no_stop() {
        let s = Session {
            start: z(2026, 5, 4, 9, 0),
            stop: None,
            note: None,
        };
        assert!(s.is_running());
    }

    #[test]
    fn session_duration_uses_now_when_running() {
        let s = Session {
            start: z(2026, 5, 4, 9, 0),
            stop: None,
            note: None,
        };
        let now = z(2026, 5, 4, 10, 30);
        let span = s.duration(&now);
        assert_eq!(span.get_minutes(), 90);
    }

    #[test]
    fn session_duration_uses_stop_when_closed() {
        let s = Session {
            start: z(2026, 5, 4, 9, 0),
            stop: Some(z(2026, 5, 4, 9, 45)),
            note: None,
        };
        let now = z(2026, 5, 4, 23, 0);
        assert_eq!(s.duration(&now).get_minutes(), 45);
    }

    #[test]
    fn project_running_session_picks_open_one() {
        let mut p = Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![],
        };
        p.sessions.push(Session {
            start: z(2026, 5, 4, 9, 0),
            stop: Some(z(2026, 5, 4, 9, 30)),
            note: None,
        });
        assert!(p.running_session().is_none());
        p.sessions.push(Session {
            start: z(2026, 5, 4, 10, 0),
            stop: None,
            note: None,
        });
        assert!(p.running_session().is_some());
    }

    #[test]
    fn day_bounds_are_midnight_to_midnight() {
        let tz = TimeZone::get("Europe/Berlin").unwrap();
        let i = Interval::Day(date(2026, 5, 4));
        let (lo, hi) = i.bounds(&tz);
        assert_eq!(lo.year(), 2026);
        assert_eq!(lo.month(), 5);
        assert_eq!(lo.day(), 4);
        assert_eq!(lo.hour(), 0);
        assert_eq!(lo.minute(), 0);
        assert_eq!(hi.year(), 2026);
        assert_eq!(hi.month(), 5);
        assert_eq!(hi.day(), 5);
        assert_eq!(hi.hour(), 0);
    }

    #[test]
    fn iso_week_bounds_monday_to_next_monday() {
        let tz = TimeZone::UTC;
        let i = Interval::Week(date(2026, 5, 6));
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2026, 5, 4));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2026, 5, 11));
    }

    #[test]
    fn month_bounds_first_to_first() {
        let tz = TimeZone::UTC;
        let i = Interval::Month(date(2026, 2, 15));
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2026, 2, 1));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2026, 3, 1));
    }

    #[test]
    fn year_bounds_jan1_to_jan1() {
        let tz = TimeZone::UTC;
        let i = Interval::Year(2024);
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2024, 1, 1));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2025, 1, 1));
    }

    #[test]
    fn custom_bounds_inclusive_to_day() {
        let tz = TimeZone::UTC;
        let i = Interval::Custom {
            from: date(2024, 4, 18),
            to: date(2024, 7, 16),
        };
        let (lo, hi) = i.bounds(&tz);
        assert_eq!((lo.year(), lo.month(), lo.day()), (2024, 4, 18));
        assert_eq!((hi.year(), hi.month(), hi.day()), (2024, 7, 17));
    }

    #[test]
    fn contains_uses_local_bounds() {
        let tz = TimeZone::UTC;
        let i = Interval::Day(date(2026, 5, 4));
        let inside = z(2026, 5, 4, 12, 0);
        let outside = z(2026, 5, 5, 0, 0);
        assert!(i.contains(&inside, &tz));
        assert!(!i.contains(&outside, &tz));
    }

    #[test]
    fn previous_next_day() {
        let i = Interval::Day(date(2026, 5, 4));
        assert_eq!(i.previous(), Interval::Day(date(2026, 5, 3)));
        assert_eq!(i.next(), Interval::Day(date(2026, 5, 5)));
    }

    #[test]
    fn previous_next_week() {
        let i = Interval::Week(date(2026, 5, 4));
        let p = i.previous();
        let n = i.next();
        assert!(matches!(p, Interval::Week(_)));
        let tz = TimeZone::UTC;
        let (lo, _) = i.bounds(&tz);
        let (lop, _) = p.bounds(&tz);
        assert_eq!(
            (lo.timestamp().as_second() - lop.timestamp().as_second()) / 86400,
            7
        );
        let (lon, _) = n.bounds(&tz);
        assert_eq!(
            (lon.timestamp().as_second() - lo.timestamp().as_second()) / 86400,
            7
        );
    }

    #[test]
    fn previous_next_month_handles_year_wrap() {
        let i = Interval::Month(date(2026, 1, 15));
        assert_eq!(i.previous(), Interval::Month(date(2025, 12, 1)));
        let i2 = Interval::Month(date(2026, 12, 1));
        assert_eq!(i2.next(), Interval::Month(date(2027, 1, 1)));
    }

    #[test]
    fn previous_next_year() {
        assert_eq!(Interval::Year(2026).previous(), Interval::Year(2025));
        assert_eq!(Interval::Year(2026).next(), Interval::Year(2027));
    }

    #[test]
    fn custom_previous_next_shifts_by_range_length_in_days() {
        let i = Interval::Custom {
            from: date(2024, 4, 18),
            to: date(2024, 4, 20),
        };
        let p = i.previous();
        match p {
            Interval::Custom { from, to } => {
                assert_eq!((from.year(), from.month(), from.day()), (2024, 4, 15));
                assert_eq!((to.year(), to.month(), to.day()), (2024, 4, 17));
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn labels_are_human_readable() {
        let i_day = Interval::Day(date(2026, 5, 4));
        assert!(i_day.label().contains("04.05.2026"));
        let i_year = Interval::Year(2026);
        assert_eq!(i_year.label(), "2026");
    }
}
