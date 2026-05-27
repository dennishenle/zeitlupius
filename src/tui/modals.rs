use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::app::Modal;

pub fn draw(f: &mut Frame, area: Rect, modal: &Modal) {
    let popup = centered(area, 50, 30);
    f.render_widget(Clear, popup);
    match modal {
        Modal::None => {}
        Modal::NewProject { input } => {
            let p = Paragraph::new(vec![
                Line::from("Create project — name:"),
                Line::from(""),
                Line::from(Span::styled(
                    input.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Enter to create · Esc to cancel"),
            ])
            .block(Block::default().borders(Borders::ALL).title("New project"))
            .alignment(Alignment::Left);
            f.render_widget(p, popup);
        }
        Modal::ConfirmDelete { project, running } => {
            let mut lines = vec![
                Line::from(format!("Delete '{project}'?")),
                Line::from(""),
                Line::from("All sessions will be erased."),
            ];
            if *running {
                lines.push(Line::from(Span::styled(
                    "This project is running — the open session will be lost.",
                    Style::default().fg(Color::Yellow),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("y to confirm · N/Esc to cancel"));
            f.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Confirm delete"),
                ),
                popup,
            );
        }
        Modal::CustomInterval {
            from,
            to,
            focus_to,
            error,
        } => {
            let from_line = if !*focus_to {
                format!("> from: {from}")
            } else {
                format!("  from: {from}")
            };
            let to_line = if *focus_to {
                format!("> to:   {to}")
            } else {
                format!("  to:   {to}")
            };
            let mut lines = vec![
                Line::from("Custom interval (DD.MM.YYYY)"),
                Line::from(""),
                Line::from(from_line),
                Line::from(to_line),
                Line::from(""),
                Line::from("Tab to switch · Enter to apply · Esc to cancel"),
            ];
            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    err.clone(),
                    Style::default().fg(Color::Red),
                )));
            }
            f.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Custom interval"),
                ),
                popup,
            );
        }
        Modal::ConfirmDeleteSession {
            session_id,
            start,
            stop,
            duration_seconds,
            ..
        } => {
            let stop_str = match stop {
                Some(z) => z.strftime("%d.%m.%Y %H:%M:%S").to_string(),
                None => "running".into(),
            };
            let lines = vec![
                Line::from(format!("Delete session {session_id}?")),
                Line::from(""),
                Line::from(format!(
                    "{}  →  {}",
                    start.strftime("%d.%m.%Y %H:%M:%S"),
                    stop_str
                )),
                Line::from(format!(
                    "Duration: {}",
                    crate::cli::format::fmt_hms(*duration_seconds)
                )),
                Line::from(""),
                Line::from("y to confirm · N/Esc to cancel"),
            ];
            f.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Confirm delete session"),
                ),
                popup,
            );
        }
        Modal::Help => {
            let lines = vec![
                Line::from("Keys"),
                Line::from(""),
                Line::from("j/k or ↓/↑   move selection (in focused panel)"),
                Line::from("Tab          switch focus: Projects ↔ Sessions"),
                Line::from("d/w/m/y      switch interval"),
                Line::from("← / →        page back / forward"),
                Line::from("g / Home     reset to current"),
                Line::from("c            custom interval"),
                Line::from("s / S        start / stop selected project"),
                Line::from("n            new project"),
                Line::from("D            delete (project or session, by focus)"),
                Line::from("r            reload from disk"),
                Line::from("? / Esc      close help · q quit"),
            ];
            f.render_widget(
                Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Help")),
                popup,
            );
        }
    }
}

fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(v)[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn renders_each_modal_without_panic() {
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        for m in [
            Modal::NewProject {
                input: "abc".into(),
            },
            Modal::Help,
            Modal::CustomInterval {
                from: "01.01.2026".into(),
                to: "02.01.2026".into(),
                focus_to: true,
                error: None,
            },
        ] {
            term.draw(|f| draw(f, f.area(), &m)).unwrap();
        }
    }
}
