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
    draw_project_list(f, cols[0], data);
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
        .block(Block::default().borders(Borders::RIGHT).title("Projects"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut ls);
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
}
