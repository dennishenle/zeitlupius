use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Zoned};

use crate::model::{Interval, Project};
use crate::time::overlap_seconds_between;

const DAILY_MAX_DAYS: i32 = 31;
const WEEKLY_MAX_DAYS: i32 = 182;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BucketTotal {
    pub start: Zoned,
    pub end: Zoned,
    pub seconds: i64,
}

/// Buckets cover `interval` exactly, so their seconds sum to `project_total_in`.
pub fn bucket_totals(
    project: &Project,
    interval: &Interval,
    now: &Zoned,
    tz: &TimeZone,
) -> (Granularity, Vec<BucketTotal>) {
    let (granularity, edges) = bucket_edges(interval, tz);
    let buckets = edges
        .windows(2)
        .map(|w| {
            let (start, end) = (&w[0], &w[1]);
            let seconds = project
                .sessions
                .iter()
                .map(|s| overlap_seconds_between(s, start, end, now))
                .sum();
            BucketTotal {
                start: start.clone(),
                end: end.clone(),
                seconds,
            }
        })
        .collect();
    (granularity, buckets)
}

fn bucket_edges(interval: &Interval, tz: &TimeZone) -> (Granularity, Vec<Zoned>) {
    let (lo, hi) = interval.bounds(tz);
    let (first, end) = (lo.date(), hi.date());
    let (granularity, mut edges) = match interval {
        Interval::Day(d) => (Granularity::Hour, hour_starts(*d, tz)),
        Interval::Week(_) | Interval::Month(_) => (
            Granularity::Day,
            at_midnight(stepped_dates(first, end, 1), tz),
        ),
        Interval::Year(_) => (
            Granularity::Month,
            at_midnight(month_starts(first, end), tz),
        ),
        Interval::Custom { from, to } => {
            let days = to.since(*from).expect("range").get_days() + 1;
            if days <= DAILY_MAX_DAYS {
                (
                    Granularity::Day,
                    at_midnight(stepped_dates(first, end, 1), tz),
                )
            } else if days <= WEEKLY_MAX_DAYS {
                (
                    Granularity::Week,
                    at_midnight(stepped_dates(first, end, 7), tz),
                )
            } else {
                (
                    Granularity::Month,
                    at_midnight(month_starts(first, end), tz),
                )
            }
        }
    };
    edges.push(hi);
    (granularity, edges)
}

/// Always 24 wall-clock hours: with compatible disambiguation a skipped DST hour
/// becomes an empty bucket and a repeated hour a two-hour bucket.
fn hour_starts(day: Date, tz: &TimeZone) -> Vec<Zoned> {
    (0..24)
        .map(|h| day.at(h, 0, 0, 0).to_zoned(tz.clone()).expect("hour"))
        .collect()
}

fn at_midnight(dates: Vec<Date>, tz: &TimeZone) -> Vec<Zoned> {
    dates
        .into_iter()
        .map(|d| d.at(0, 0, 0, 0).to_zoned(tz.clone()).expect("midnight"))
        .collect()
}

fn stepped_dates(first: Date, end: Date, step_days: i64) -> Vec<Date> {
    let mut out = Vec::new();
    let mut d = first;
    while d < end {
        out.push(d);
        d = d
            .checked_add(Span::new().days(step_days))
            .expect("date in range");
    }
    out
}

fn month_starts(first: Date, end: Date) -> Vec<Date> {
    let month = Span::new().months(1);
    let mut out = vec![first];
    let mut d = first
        .first_of_month()
        .checked_add(month)
        .expect("date in range");
    while d < end {
        out.push(d);
        d = d.checked_add(month).expect("date in range");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectName, Session, SessionId};
    use crate::time::project_total_in;
    use jiff::civil::date;

    fn z(year: i16, m: i8, d: i8, h: i8, min: i8) -> Zoned {
        date(year, m, d)
            .at(h, min, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }
    fn closed(start: Zoned, stop: Zoned) -> Session {
        Session {
            id: SessionId::generate(),
            start,
            stop: Some(stop),
            note: None,
        }
    }
    fn open(start: Zoned) -> Session {
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
    fn empty_buckets(interval: &Interval) -> (Granularity, Vec<BucketTotal>) {
        bucket_totals(
            &project_with(vec![]),
            interval,
            &z(2026, 5, 4, 12, 0),
            &TimeZone::UTC,
        )
    }

    #[test]
    fn day_splits_into_24_hours() {
        let (g, b) = empty_buckets(&Interval::Day(date(2026, 5, 4)));
        assert_eq!((g, b.len()), (Granularity::Hour, 24));
        assert_eq!(b[0].start, z(2026, 5, 4, 0, 0));
        assert_eq!(b[23].end, z(2026, 5, 5, 0, 0));
    }

    #[test]
    fn week_splits_into_7_days_from_monday() {
        let (g, b) = empty_buckets(&Interval::Week(date(2026, 5, 6)));
        assert_eq!((g, b.len()), (Granularity::Day, 7));
        assert_eq!(b[0].start, z(2026, 5, 4, 0, 0));
    }

    #[test]
    fn month_splits_into_calendar_days() {
        let (g, b) = empty_buckets(&Interval::Month(date(2026, 2, 15)));
        assert_eq!((g, b.len()), (Granularity::Day, 28));
        let (g, b) = empty_buckets(&Interval::Month(date(2026, 5, 1)));
        assert_eq!((g, b.len()), (Granularity::Day, 31));
    }

    #[test]
    fn year_splits_into_12_months() {
        let (g, b) = empty_buckets(&Interval::Year(2026));
        assert_eq!((g, b.len()), (Granularity::Month, 12));
        assert_eq!(b[1].start, z(2026, 2, 1, 0, 0));
    }

    #[test]
    fn custom_up_to_31_days_is_daily() {
        let i = Interval::Custom {
            from: date(2026, 5, 1),
            to: date(2026, 5, 31),
        };
        let (g, b) = empty_buckets(&i);
        assert_eq!((g, b.len()), (Granularity::Day, 31));
    }

    #[test]
    fn custom_32_days_is_weekly_with_short_last_chunk() {
        let i = Interval::Custom {
            from: date(2026, 5, 1),
            to: date(2026, 6, 1),
        };
        let (g, b) = empty_buckets(&i);
        assert_eq!((g, b.len()), (Granularity::Week, 5));
        assert_eq!(b[4].start, z(2026, 5, 29, 0, 0));
        assert_eq!(b[4].end, z(2026, 6, 2, 0, 0));
    }

    #[test]
    fn custom_182_days_is_weekly() {
        let i = Interval::Custom {
            from: date(2024, 4, 18),
            to: date(2024, 10, 16),
        };
        let (g, b) = empty_buckets(&i);
        assert_eq!((g, b.len()), (Granularity::Week, 26));
    }

    #[test]
    fn custom_over_182_days_is_monthly_with_partial_edges() {
        let i = Interval::Custom {
            from: date(2024, 4, 18),
            to: date(2024, 10, 17),
        };
        let (g, b) = empty_buckets(&i);
        assert_eq!((g, b.len()), (Granularity::Month, 7));
        assert_eq!(b[0].start, z(2024, 4, 18, 0, 0));
        assert_eq!(b[1].start, z(2024, 5, 1, 0, 0));
        assert_eq!(b[6].end, z(2024, 10, 18, 0, 0));
    }

    #[test]
    fn buckets_are_contiguous_and_cover_the_interval() {
        let intervals = [
            Interval::Day(date(2026, 5, 4)),
            Interval::Week(date(2026, 5, 4)),
            Interval::Month(date(2026, 5, 4)),
            Interval::Year(2026),
            Interval::Custom {
                from: date(2026, 5, 1),
                to: date(2026, 6, 1),
            },
            Interval::Custom {
                from: date(2024, 4, 18),
                to: date(2024, 10, 17),
            },
        ];
        for i in intervals {
            let (_, b) = empty_buckets(&i);
            let (lo, hi) = i.bounds(&TimeZone::UTC);
            assert_eq!(b.first().unwrap().start, lo, "{i:?}");
            assert_eq!(b.last().unwrap().end, hi, "{i:?}");
            for w in b.windows(2) {
                assert_eq!(w[0].end, w[1].start, "{i:?}");
            }
        }
    }

    #[test]
    fn bucket_sum_matches_project_total() {
        let p = project_with(vec![
            closed(z(2026, 5, 3, 23, 0), z(2026, 5, 4, 1, 30)),
            closed(z(2026, 5, 4, 9, 0), z(2026, 5, 4, 10, 15)),
            open(z(2026, 5, 4, 22, 0)),
        ]);
        let now = z(2026, 5, 4, 22, 45);
        for i in [
            Interval::Day(date(2026, 5, 4)),
            Interval::Week(date(2026, 5, 4)),
            Interval::Month(date(2026, 5, 4)),
            Interval::Year(2026),
        ] {
            let (_, b) = bucket_totals(&p, &i, &now, &TimeZone::UTC);
            let sum: i64 = b.iter().map(|x| x.seconds).sum();
            assert!(sum > 0, "{i:?}");
            assert_eq!(sum, project_total_in(&p, &i, &now, &TimeZone::UTC), "{i:?}");
        }
    }

    #[test]
    fn session_crossing_hour_edge_is_split() {
        let p = project_with(vec![closed(z(2026, 5, 4, 9, 30), z(2026, 5, 4, 10, 30))]);
        let (_, b) = bucket_totals(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 23, 0),
            &TimeZone::UTC,
        );
        assert_eq!(b[9].seconds, 1800);
        assert_eq!(b[10].seconds, 1800);
        assert_eq!(b.iter().map(|x| x.seconds).sum::<i64>(), 3600);
    }

    #[test]
    fn running_session_counts_until_now() {
        let p = project_with(vec![open(z(2026, 5, 4, 10, 0))]);
        let (_, b) = bucket_totals(
            &p,
            &Interval::Day(date(2026, 5, 4)),
            &z(2026, 5, 4, 11, 30),
            &TimeZone::UTC,
        );
        assert_eq!(b[10].seconds, 3600);
        assert_eq!(b[11].seconds, 1800);
        assert!(b[12..].iter().all(|x| x.seconds == 0));
    }

    fn berlin_full_day(day: jiff::civil::Date) -> Vec<BucketTotal> {
        let tz = TimeZone::get("Europe/Berlin").unwrap();
        let next = day.tomorrow().unwrap();
        let p = project_with(vec![closed(
            day.at(0, 0, 0, 0).to_zoned(tz.clone()).unwrap(),
            next.at(0, 0, 0, 0).to_zoned(tz.clone()).unwrap(),
        )]);
        let now = next.at(12, 0, 0, 0).to_zoned(tz.clone()).unwrap();
        bucket_totals(&p, &Interval::Day(day), &now, &tz).1
    }

    #[test]
    fn spring_forward_day_keeps_24_buckets_with_empty_skipped_hour() {
        let b = berlin_full_day(date(2026, 3, 29));
        assert_eq!(b.len(), 24);
        assert_eq!(b[2].seconds, 0);
        assert_eq!(b.iter().map(|x| x.seconds).sum::<i64>(), 23 * 3600);
    }

    #[test]
    fn fall_back_day_doubles_repeated_hour() {
        let b = berlin_full_day(date(2026, 10, 25));
        assert_eq!(b.len(), 24);
        assert_eq!(b[2].seconds, 7200);
        assert_eq!(b.iter().map(|x| x.seconds).sum::<i64>(), 25 * 3600);
    }
}
