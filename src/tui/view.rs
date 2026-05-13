use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::cli::format::{fmt_hm, fmt_hms};
use crate::model::{Interval, Project};
use crate::tui::app::{AppState, Modal};

pub struct DashboardData<'a> {
    pub state: &'a AppState,
    pub projects: &'a [Project],
    pub now: &'a jiff::Zoned,
    pub tz: &'a jiff::tz::TimeZone,
}

#[allow(dead_code)]
fn right_aligned_row(
    left: Vec<(String, Style)>,
    right: (String, Style),
    target: usize,
) -> Line<'static> {
    let left_width: usize = left.iter().map(|(t, _)| t.chars().count()).sum();
    let right_width = right.0.chars().count();

    // Impossibly narrow: just show the right value, no panic.
    if target < right_width + 2 {
        return Line::from(Span::styled(right.0, right.1));
    }

    // Fits with at least one filler space.
    if left_width + 1 + right_width <= target {
        let filler = " ".repeat(target - left_width - right_width);
        let mut spans: Vec<Span<'static>> = left
            .into_iter()
            .map(|(t, s)| Span::styled(t, s))
            .collect();
        spans.push(Span::raw(filler));
        spans.push(Span::styled(right.0, right.1));
        return Line::from(spans);
    }

    // Overflow: truncate the LAST left segment with "…".
    // Budget for left text (excluding the "…" glyph and the 1-col gap):
    //   max_text = target - right_width - 2
    //   final width = max_text + 1 (…) + 1 (gap) + right_width = target
    let max_text = target.saturating_sub(right_width + 2);
    let last_idx = left.len() - 1;
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(left.len() + 2);
    let mut remaining = max_text;
    for (i, (text, style)) in left.into_iter().enumerate() {
        if i < last_idx {
            let w = text.chars().count();
            if w <= remaining {
                spans.push(Span::styled(text, style));
                remaining -= w;
            } else {
                let truncated: String = text.chars().take(remaining).collect();
                spans.push(Span::styled(truncated, style));
                remaining = 0;
            }
        } else {
            let truncated: String = text.chars().take(remaining).collect();
            spans.push(Span::styled(format!("{truncated}…"), style));
        }
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(right.0, right.1));
    Line::from(spans)
}

pub fn draw(f: &mut Frame, data: &DashboardData) {
    let area = f.area();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // Body
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Footer
        ])
        .split(area);
    let header = outer[0];
    let body = outer[2];
    let footer = outer[4];
    draw_header(f, header, data);
    if data.projects.is_empty() {
        draw_empty(f, body);
    } else {
        draw_body(f, body, data);
    }
    draw_footer(f, footer, data);
    if !matches!(data.state.modal, Modal::None) {
        super::modals::draw(f, area, &data.state.modal);
    }
}

fn draw_header(f: &mut Frame, area: Rect, data: &DashboardData) {
    let kind = match data.state.interval {
        Interval::Day(_) => "Day",
        Interval::Week(_) => "Week",
        Interval::Month(_) => "Month",
        Interval::Year(_) => "Year",
        Interval::Custom { .. } => "Custom",
    };
    let label = data.state.interval.label();
    let txt = format!("zeitlupius · {kind} · {label}");
    let p = Paragraph::new(Line::from(Span::styled(
        txt,
        Style::default().add_modifier(Modifier::BOLD),
    )));
    f.render_widget(p, area);
}

fn draw_empty(f: &mut Frame, area: Rect) {
    let p = Paragraph::new("No projects yet — press n to create one.")
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn draw_body(f: &mut Frame, area: Rect, data: &DashboardData) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(cols[0]);
    draw_project_list(f, left[0], data);
    draw_sessions_panel(f, left[1], data);
    draw_detail(f, cols[1], data);
}

fn draw_project_list(f: &mut Frame, area: Rect, data: &DashboardData) {
    use crate::time::project_total_in;
    let items: Vec<ListItem> = data
        .projects
        .iter()
        .map(|p| {
            let secs = project_total_in(p, &data.state.interval, data.now, data.tz);
            let running = p.running_session().map(|s| {
                let r = data.now.timestamp().as_second() - s.start.timestamp().as_second();
                r.max(0)
            });
            let mut spans = Vec::new();
            if let Some(r) = running {
                spans.push(Span::styled("● ", Style::default().fg(Color::Green)));
                spans.push(Span::raw(format!("{:<24}", p.name)));
                spans.push(Span::styled(fmt_hms(r), Style::default().fg(Color::Green)));
            } else {
                spans.push(Span::raw("  "));
                spans.push(Span::raw(format!("{:<24}", p.name)));
                spans.push(Span::raw(fmt_hm(secs)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut ls = ListState::default();
    ls.select(Some(data.state.selected));
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Projects"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut ls);
}

fn draw_sessions_panel(f: &mut Frame, area: Rect, data: &DashboardData) {
    use crate::time::sessions_in;
    let sessions: Vec<&crate::model::Session> = match data.projects.get(data.state.selected) {
        Some(p) => sessions_in(p, &data.state.interval, data.now, data.tz),
        None => Vec::new(),
    };
    let items: Vec<ListItem> = sessions
        .iter()
        .map(|s| {
            let stop = s
                .stop
                .as_ref()
                .map(|z| z.strftime("%d.%m.%Y %H:%M:%S").to_string())
                .unwrap_or_default();
            let secs = match &s.stop {
                Some(stop) => stop.timestamp().as_second() - s.start.timestamp().as_second(),
                None => 0,
            };
            ListItem::new(Line::from(format!(
                "{} →  {}    {}",
                s.start.strftime("%d.%m.%Y %H:%M:%S"),
                stop,
                fmt_hms(secs),
            )))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Sessions"));
    f.render_widget(list, area);
}

fn draw_detail(f: &mut Frame, area: Rect, data: &DashboardData) {
    use crate::time::project_total_in;
    let Some(p) = data.projects.get(data.state.selected) else {
        return;
    };
    let today = data.now.date();
    let intervals = [
        ("Day  ", Interval::Day(today)),
        ("Week ", Interval::Week(today)),
        ("Month", Interval::Month(today)),
        ("Year ", Interval::Year(today.year())),
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            p.name.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (label, iv) in intervals {
        let secs = project_total_in(p, &iv, data.now, data.tz);
        lines.push(Line::from(format!(
            "{label}  {}  ({})",
            fmt_hm(secs),
            iv.label()
        )));
    }
    lines.push(Line::from(""));

    // Active sessions
    if let Some(s) = p.running_session() {
        let r = (data.now.timestamp().as_second() - s.start.timestamp().as_second()).max(0);
        lines.push(Line::from(Span::styled(
            "Active session",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!(
            " started {} for {}",
            s.start.strftime("%d.%m.%Y %H:%M:%S"),
            fmt_hms(r)
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Recent sessions (last 5)",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    // Last 5 sessions
    for s in p.sessions.iter().rev().take(5) {
        let stop = match &s.stop {
            Some(z) => z.strftime("%d.%m.%Y %H:%M:%S").to_string(),
            None => "running".to_string(),
        };
        let secs = match &s.stop {
            Some(stop) => stop.timestamp().as_second() - s.start.timestamp().as_second(),
            None => (data.now.timestamp().as_second() - s.start.timestamp().as_second()).max(0),
        };
        lines.push(Line::from(format!(
            " {}  →  {}    {}",
            s.start.strftime("%d.%m.%Y %H:%M:%S"),
            stop,
            fmt_hms(secs)
        )));
    }
    let par = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Detail"));
    f.render_widget(par, area);
}

fn draw_footer(f: &mut Frame, area: Rect, data: &DashboardData) {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);
    let line1 = "d/w/m/y interval · ←/→ page · c custom · j/k select";
    let line2 = "s start · S stop · n new · D delete · r reload · ? help · q quit";
    f.render_widget(Paragraph::new(line1), split[0]);
    let l2 = if let Some((msg, _)) = &data.state.status_line {
        Paragraph::new(Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(Color::Red),
        )))
    } else {
        Paragraph::new(line2)
    };
    f.render_widget(l2, split[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, ProjectName, Session};
    use jiff::{civil::date, tz::TimeZone};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn z(h: i8) -> jiff::Zoned {
        date(2026, 5, 4)
            .at(h, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
    }

    fn buffer_left_half(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let mid = buf.area.width / 2;
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..mid {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn renders_empty_dashboard_without_panic() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let state = AppState::new(date(2026, 5, 4), vec![]);
        let now = z(12);
        let tz = TimeZone::UTC;
        term.draw(|f| {
            draw(
                f,
                &DashboardData {
                    state: &state,
                    projects: &[],
                    now: &now,
                    tz: &tz,
                },
            )
        })
        .unwrap();
    }

    #[test]
    fn renders_with_projects_without_panic() {
        let backend = TestBackend::new(100, 25);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![Session {
                start: z(9),
                stop: Some(z(10)),
                note: None,
            }],
        }];
        let names = vec![ProjectName::parse("p").unwrap()];
        let state = AppState::new(date(2026, 5, 4), names);
        let now = z(11);
        let tz = TimeZone::UTC;
        term.draw(|f| {
            draw(
                f,
                &DashboardData {
                    state: &state,
                    projects: &projects,
                    now: &now,
                    tz: &tz,
                },
            )
        })
        .unwrap();
    }

    #[test]
    fn sessions_panel_lists_finished_sessions_and_skips_running() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![Project {
            name: ProjectName::parse("p").unwrap(),
            sessions: vec![
                Session {
                    start: z(9),
                    stop: Some(z(10)),
                    note: None,
                },
                Session {
                    start: z(11),
                    stop: None,
                    note: None,
                },
            ],
        }];
        let names = vec![ProjectName::parse("p").unwrap()];
        let state = AppState::new(date(2026, 5, 4), names);
        let now = date(2026, 5, 4)
            .at(12, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap();
        let tz = TimeZone::UTC;
        term.draw(|f| {
            draw(
                f,
                &DashboardData {
                    state: &state,
                    projects: &projects,
                    now: &now,
                    tz: &tz,
                },
            )
        })
        .unwrap();
        let text = buffer_left_half(&term);
        assert!(
            text.contains("04.05.2026 09:00:00"),
            "finished session timestamp missing"
        );
        // Sessions panel row format for running session: "START →      0:00:00" (stop is empty).
        // Detail panel shows "running" instead of the stop time. Verify running session
        // doesn't appear as a row in the Sessions panel.
        assert!(
            !text.contains("04.05.2026 11:00:00 →      0:00:00"),
            "running session leaked into Sessions panel as a row"
        );
    }

    #[test]
    fn sessions_panel_only_shows_selected_project_sessions() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).unwrap();
        let projects = vec![
            Project {
                name: ProjectName::parse("a").unwrap(),
                sessions: vec![],
            },
            Project {
                name: ProjectName::parse("b").unwrap(),
                sessions: vec![Session {
                    start: z(9),
                    stop: Some(z(10)),
                    note: None,
                }],
            },
        ];
        let names = vec![
            ProjectName::parse("a").unwrap(),
            ProjectName::parse("b").unwrap(),
        ];
        let state = AppState::new(date(2026, 5, 4), names); // selected = 0 = "a"
        let now = date(2026, 5, 4)
            .at(12, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap();
        let tz = TimeZone::UTC;
        term.draw(|f| {
            draw(
                f,
                &DashboardData {
                    state: &state,
                    projects: &projects,
                    now: &now,
                    tz: &tz,
                },
            )
        })
        .unwrap();
        let text = buffer_left_half(&term);
        // Sessions panel must not include project b's session
        assert!(
            !text.contains("04.05.2026 09:00:00"),
            "non-selected project's session leaked into left half"
        );
    }

    #[test]
    fn right_aligned_row_fits_with_filler() {
        let line = right_aligned_row(
            vec![("hello".to_string(), Style::default())],
            ("world".to_string(), Style::default()),
            20,
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(rendered.chars().count(), 20, "row should fill exactly target");
        assert!(rendered.starts_with("hello"));
        assert!(rendered.ends_with("world"));
    }

    #[test]
    fn right_aligned_row_truncates_when_overflow() {
        let line = right_aligned_row(
            vec![("verylongprojectname".to_string(), Style::default())],
            ("1:23".to_string(), Style::default()),
            10,
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(rendered.chars().count(), 10, "row should fill exactly target");
        assert!(rendered.contains('…'), "truncation marker expected");
        assert!(rendered.ends_with("1:23"), "right value must be intact");
    }

    #[test]
    fn right_aligned_row_keeps_indicator_when_truncating_name() {
        let line = right_aligned_row(
            vec![
                ("● ".to_string(), Style::default()),
                ("verylongprojectname".to_string(), Style::default()),
            ],
            ("1:23".to_string(), Style::default()),
            12,
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(rendered.chars().count(), 12);
        assert!(rendered.starts_with("● "), "indicator must be preserved");
        assert!(rendered.contains('…'));
        assert!(rendered.ends_with("1:23"));
    }

    #[test]
    fn right_aligned_row_handles_too_narrow_gracefully() {
        // target smaller than right + 2 falls back to just the right value, no panic.
        let line = right_aligned_row(
            vec![("name".to_string(), Style::default())],
            ("1:23".to_string(), Style::default()),
            3,
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(rendered.ends_with("1:23"));
    }

    #[test]
    fn right_aligned_row_truncates_first_of_two_left_segments() {
        // First segment is wider than the truncation budget. Result must still
        // fill exactly `target` columns.
        let line = right_aligned_row(
            vec![
                ("AAAA".to_string(), Style::default()),
                ("BB".to_string(), Style::default()),
            ],
            ("CC".to_string(), Style::default()),
            6,
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(rendered.chars().count(), 6, "row should fill exactly target");
        assert!(rendered.ends_with("CC"));
        assert!(rendered.contains('…'));
    }

    #[test]
    fn right_aligned_row_empty_left_does_not_panic() {
        // Empty left should never reach the overflow branch (provable), but exercise
        // both fits and narrow paths explicitly.
        let fits = right_aligned_row(vec![], ("1:23".to_string(), Style::default()), 10);
        let rendered: String = fits.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered.chars().count(), 10);
        assert!(rendered.ends_with("1:23"));

        let narrow = right_aligned_row(vec![], ("1:23".to_string(), Style::default()), 3);
        let rendered: String = narrow.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.ends_with("1:23"));
    }
}
