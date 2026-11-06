use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, Block, Borders, Paragraph};

use crate::cli::format::fmt_hm;
use crate::model::{Interval, Project};
use crate::time::{BucketTotal, Granularity, bucket_totals};
use crate::tui::view::{DashboardData, interval_kind};

/// 2 border rows + 3 bar rows + 1 label row.
pub const MIN_HEIGHT: u16 = 6;

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Geometry {
    bar_width: u16,
    gap: u16,
}

impl Geometry {
    fn slot(self) -> u16 {
        self.bar_width + self.gap
    }
}

pub fn draw_graph(f: &mut Frame, area: Rect, project: &Project, data: &DashboardData) {
    let interval = &data.state.interval;
    let (granularity, buckets) = bucket_totals(project, interval, data.now, data.tz);
    let title = graph_title(interval_kind(interval), granularity, interval, &buckets);
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if buckets.iter().all(|b| b.seconds == 0) {
        f.render_widget(dim("No time tracked in this period"), inner);
        return;
    }
    let Some(geo) = geometry(buckets.len(), inner.width) else {
        f.render_widget(dim("widen terminal to see graph"), inner);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let bars: Vec<Bar> = buckets
        .iter()
        .map(|b| Bar::default().value(b.seconds as u64).text_value(""))
        .collect();
    let chart = BarChart::new(bars)
        .bar_width(geo.bar_width)
        .bar_gap(geo.gap)
        .bar_style(Style::default().fg(Color::Cyan));
    f.render_widget(chart, rows[0]);
    let labels: Vec<String> = buckets
        .iter()
        .enumerate()
        .map(|(i, b)| short_label(granularity, interval, i, &b.start, geo.slot()))
        .collect();
    f.render_widget(
        Paragraph::new(label_row(&labels, geo, inner.width)),
        rows[1],
    );
}

fn dim(msg: &'static str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        msg,
        Style::default().fg(Color::DarkGray),
    )))
}

/// `None` when there are no buckets or fewer columns than buckets.
fn geometry(n: usize, width: u16) -> Option<Geometry> {
    let w = width as usize;
    if n == 0 || n > w {
        return None;
    }
    let gap = if 2 * n - 1 <= w { 1 } else { 0 };
    let bar_width = ((w + gap) / n - gap).max(1);
    Some(Geometry {
        bar_width: bar_width as u16,
        gap: gap as u16,
    })
}

fn short_label(
    g: Granularity,
    interval: &Interval,
    index: usize,
    start: &jiff::Zoned,
    slot: u16,
) -> String {
    let d = start.date();
    match g {
        Granularity::Hour => index.to_string(),
        Granularity::Day if matches!(interval, Interval::Week(_)) => {
            WEEKDAYS[weekday_index(d)][..2].to_string()
        }
        Granularity::Day => d.day().to_string(),
        Granularity::Week => format!("{:02}.{:02}", d.day(), d.month()),
        Granularity::Month if slot < 4 => MONTHS[month_index(d)][..1].to_string(),
        Granularity::Month => MONTHS[month_index(d)].to_string(),
    }
}

fn long_label(g: Granularity, interval: &Interval, index: usize, start: &jiff::Zoned) -> String {
    let d = start.date();
    match g {
        Granularity::Hour => format!("{index:02}:00"),
        Granularity::Day if matches!(interval, Interval::Week(_)) => {
            WEEKDAYS[weekday_index(d)].to_string()
        }
        Granularity::Day => format!("{:02}.{:02}.", d.day(), d.month()),
        Granularity::Week => format!("from {:02}.{:02}", d.day(), d.month()),
        Granularity::Month => MONTHS[month_index(d)].to_string(),
    }
}

fn weekday_index(d: jiff::civil::Date) -> usize {
    d.weekday().to_monday_zero_offset() as usize
}

fn month_index(d: jiff::civil::Date) -> usize {
    (d.month() - 1) as usize
}

/// Labels are left-aligned under their bar; every `step`-th one is drawn so
/// neighbours keep at least one space between them.
fn label_row(labels: &[String], geo: Geometry, width: u16) -> String {
    let slot = geo.slot() as usize;
    let widest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let step = (widest + 1).div_ceil(slot).max(1);
    let mut row = vec![' '; width as usize];
    for (i, label) in labels.iter().enumerate().step_by(step) {
        let x = i * slot;
        if x + label.chars().count() > row.len() {
            break;
        }
        for (j, ch) in label.chars().enumerate() {
            row[x + j] = ch;
        }
    }
    row.into_iter().collect::<String>().trim_end().to_string()
}

fn graph_title(kind: &str, g: Granularity, interval: &Interval, buckets: &[BucketTotal]) -> String {
    let total: i64 = buckets.iter().map(|b| b.seconds).sum();
    if total == 0 {
        return format!("{kind} · total 0:00");
    }
    let mut peak = 0;
    for (i, b) in buckets.iter().enumerate() {
        if b.seconds > buckets[peak].seconds {
            peak = i;
        }
    }
    format!(
        "{kind} · total {} · peak {} {}",
        fmt_hm(total).trim_start(),
        fmt_hm(buckets[peak].seconds).trim_start(),
        long_label(g, interval, peak, &buckets[peak].start),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectName, Session, SessionId};
    use crate::tui::app::AppState;
    use jiff::{civil::date, tz::TimeZone};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// 2026-05-`d` at `h`:00 UTC. 2026-05-04 is a Monday.
    fn z(d: i8, h: i8) -> jiff::Zoned {
        date(2026, 5, d)
            .at(h, 0, 0, 0)
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
    fn project_with(sessions: Vec<Session>) -> Project {
        Project {
            name: ProjectName::parse("p").unwrap(),
            sessions,
        }
    }

    fn render(
        width: u16,
        height: u16,
        project: &Project,
        interval: Interval,
    ) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut state = AppState::new(date(2026, 5, 6), vec![project.name.clone()]);
        state.interval = interval;
        let now = z(6, 12);
        let tz = TimeZone::UTC;
        let data = DashboardData {
            state: &state,
            projects: std::slice::from_ref(project),
            now: &now,
            tz: &tz,
        };
        term.draw(|f| {
            let area = f.area();
            draw_graph(f, area, project, &data);
        })
        .unwrap();
        term
    }

    fn rows(term: &Terminal<TestBackend>) -> Vec<String> {
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    #[test]
    fn geometry_fills_width_with_gaps_when_room() {
        assert_eq!(
            geometry(7, 70),
            Some(Geometry {
                bar_width: 9,
                gap: 1
            })
        );
        assert_eq!(
            geometry(12, 70),
            Some(Geometry {
                bar_width: 4,
                gap: 1
            })
        );
        assert_eq!(
            geometry(24, 70),
            Some(Geometry {
                bar_width: 1,
                gap: 1
            })
        );
        assert_eq!(
            geometry(31, 62),
            Some(Geometry {
                bar_width: 1,
                gap: 1
            })
        );
    }

    #[test]
    fn geometry_drops_gap_when_tight_and_refuses_when_too_narrow() {
        assert_eq!(
            geometry(31, 40),
            Some(Geometry {
                bar_width: 1,
                gap: 0
            })
        );
        assert_eq!(geometry(31, 30), None);
        assert_eq!(geometry(0, 70), None);
    }

    #[test]
    fn label_row_thins_labels_so_they_never_touch() {
        let labels: Vec<String> = (0..24).map(|h| h.to_string()).collect();
        let row = label_row(
            &labels,
            Geometry {
                bar_width: 1,
                gap: 1,
            },
            47,
        );
        assert_eq!(row, "0   2   4   6   8   10  12  14  16  18  20  22");
    }

    #[test]
    fn label_row_drops_labels_that_would_overflow() {
        let labels = vec!["Jan".to_string(), "Feb".to_string(), "Mar".to_string()];
        let row = label_row(
            &labels,
            Geometry {
                bar_width: 3,
                gap: 1,
            },
            10,
        );
        assert_eq!(row, "Jan Feb");
    }

    #[test]
    fn short_labels_per_granularity() {
        let mon = z(4, 0);
        let day = Interval::Day(date(2026, 5, 4));
        let week = Interval::Week(date(2026, 5, 4));
        let month = Interval::Month(date(2026, 5, 4));
        let year = Interval::Year(2026);
        assert_eq!(short_label(Granularity::Hour, &day, 7, &mon, 2), "7");
        assert_eq!(short_label(Granularity::Day, &week, 0, &mon, 10), "Mo");
        assert_eq!(short_label(Granularity::Day, &month, 3, &mon, 2), "4");
        assert_eq!(short_label(Granularity::Week, &month, 0, &mon, 6), "04.05");
        assert_eq!(short_label(Granularity::Month, &year, 4, &mon, 3), "M");
        assert_eq!(short_label(Granularity::Month, &year, 4, &mon, 4), "May");
    }

    #[test]
    fn long_labels_per_granularity() {
        let tue = z(5, 0);
        let day = Interval::Day(date(2026, 5, 5));
        let week = Interval::Week(date(2026, 5, 4));
        let month = Interval::Month(date(2026, 5, 4));
        let year = Interval::Year(2026);
        assert_eq!(long_label(Granularity::Hour, &day, 9, &tue), "09:00");
        assert_eq!(long_label(Granularity::Day, &week, 1, &tue), "Tue");
        assert_eq!(long_label(Granularity::Day, &month, 4, &tue), "05.05.");
        assert_eq!(long_label(Granularity::Week, &month, 0, &tue), "from 05.05");
        assert_eq!(long_label(Granularity::Month, &year, 4, &tue), "May");
    }

    #[test]
    fn title_shows_total_and_first_peak() {
        let p = project_with(vec![
            closed(z(4, 9), z(4, 10)),
            closed(z(5, 9), z(5, 12)),
            closed(z(6, 8), z(6, 11)),
        ]);
        let week = Interval::Week(date(2026, 5, 4));
        let (g, b) = bucket_totals(&p, &week, &z(6, 12), &TimeZone::UTC);
        assert_eq!(
            graph_title("Week", g, &week, &b),
            "Week · total 7:00 · peak 3:00 Tue"
        );
    }

    #[test]
    fn title_for_empty_period_has_no_peak() {
        let day = Interval::Day(date(2026, 5, 4));
        let (g, b) = bucket_totals(&project_with(vec![]), &day, &z(6, 12), &TimeZone::UTC);
        assert_eq!(graph_title("Day", g, &day, &b), "Day · total 0:00");
    }

    #[test]
    fn renders_cyan_bars_and_weekday_labels() {
        let p = project_with(vec![closed(z(4, 9), z(4, 10)), closed(z(5, 9), z(5, 12))]);
        let term = render(72, 10, &p, Interval::Week(date(2026, 5, 4)));
        let rows = rows(&term);
        assert!(
            rows[0].contains("Week · total 4:00 · peak 3:00 Tue"),
            "{rows:#?}"
        );
        // Inner width 70 → bar width 9 + gap 1, so labels sit 10 columns apart.
        assert!(
            rows[8].contains(&format!("Mo{}Tu", " ".repeat(8))),
            "{rows:#?}"
        );
        let buf = term.backend().buffer();
        let full_cyan = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                buf[(x, y)].symbol() == "█" && buf[(x, y)].style().fg == Some(Color::Cyan)
            })
            .count();
        assert!(full_cyan > 0, "expected full cyan bar cells: {rows:#?}");
    }

    #[test]
    fn shows_message_when_nothing_tracked() {
        let p = project_with(vec![closed(z(4, 9), z(4, 10))]);
        let term = render(72, 10, &p, Interval::Day(date(2026, 5, 5)));
        let text = rows(&term).join("\n");
        assert!(text.contains("Day · total 0:00"), "{text}");
        assert!(text.contains("No time tracked in this period"), "{text}");
    }

    #[test]
    fn shows_hint_when_too_narrow_for_buckets() {
        let p = project_with(vec![closed(z(4, 9), z(4, 10))]);
        // Inner width 28 < 31 day buckets in May.
        let term = render(30, 10, &p, Interval::Month(date(2026, 5, 4)));
        let text = rows(&term).join("\n");
        assert!(text.contains("widen terminal to see graph"), "{text}");
    }
}
