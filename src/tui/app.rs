use crate::model::{Interval, ProjectName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalKind {
    Day,
    Week,
    Month,
    Year,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Projects,
    Sessions,
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    NewProject {
        input: String,
    },
    ConfirmDelete {
        project: ProjectName,
        running: bool,
    },
    ConfirmDeleteSession {
        project: ProjectName,
        session_id: crate::model::SessionId,
        start: jiff::Zoned,
        stop: Option<jiff::Zoned>,
        duration_seconds: i64,
    },
    CustomInterval {
        from: String,
        to: String,
        focus_to: bool,
        error: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub projects: Vec<ProjectName>,
    pub selected: usize,
    pub interval: Interval,
    pub modal: Modal,
    pub status_line: Option<(String, std::time::Instant)>,
    pub should_quit: bool,
    pub focus: Focus,
    pub session_selected: usize,
    pub sessions_visible: Vec<crate::model::SessionId>,
}

impl AppState {
    pub fn new(today: jiff::civil::Date, projects: Vec<ProjectName>) -> Self {
        Self {
            projects,
            selected: 0,
            interval: Interval::Day(today),
            modal: Modal::None,
            status_line: None,
            should_quit: false,
            focus: Focus::Projects,
            session_selected: 0,
            sessions_visible: Vec::new(),
        }
    }

    pub fn selected_project(&self) -> Option<&ProjectName> {
        self.projects.get(self.selected)
    }

    pub fn ensure_selection_valid(&mut self) {
        if self.projects.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.projects.len() {
            self.selected = self.projects.len() - 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.session_selected = 0;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.projects.len() {
            self.selected += 1;
            self.session_selected = 0;
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Projects => Focus::Sessions,
            Focus::Sessions => Focus::Projects,
        };
    }

    pub fn move_session_up(&mut self) {
        if self.session_selected > 0 {
            self.session_selected -= 1;
        }
    }

    pub fn move_session_down(&mut self) {
        if self.session_selected + 1 < self.sessions_visible.len() {
            self.session_selected += 1;
        }
    }

    pub fn ensure_session_selection_valid(&mut self) {
        if self.sessions_visible.is_empty() {
            self.session_selected = 0;
            if self.focus == Focus::Sessions {
                self.focus = Focus::Projects;
            }
        } else if self.session_selected >= self.sessions_visible.len() {
            self.session_selected = self.sessions_visible.len() - 1;
        }
    }

    pub fn switch_interval(&mut self, kind: IntervalKind, today: jiff::civil::Date) {
        self.interval = match kind {
            IntervalKind::Day => Interval::Day(today),
            IntervalKind::Week => Interval::Week(today),
            IntervalKind::Month => Interval::Month(today),
            IntervalKind::Year => Interval::Year(today.year()),
            IntervalKind::Custom => self.interval.clone(),
        };
    }

    pub fn page_prev(&mut self) {
        self.interval = self.interval.previous();
    }
    pub fn page_next(&mut self) {
        self.interval = self.interval.next();
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_line = Some((msg.into(), std::time::Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn names(xs: &[&str]) -> Vec<ProjectName> {
        xs.iter().map(|x| ProjectName::parse(x).unwrap()).collect()
    }

    #[test]
    fn navigation_clamps_at_edges() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a", "b", "c"]));
        a.move_up();
        assert_eq!(a.selected, 0);
        a.move_down();
        a.move_down();
        a.move_down();
        assert_eq!(a.selected, 2);
    }

    #[test]
    fn switch_interval_resets_to_today() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.page_prev();
        a.switch_interval(IntervalKind::Week, date(2026, 5, 4));
        assert!(matches!(a.interval, Interval::Week(_)));
    }

    #[test]
    fn ensure_selection_valid_after_deletion() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a", "b"]));
        a.selected = 1;
        a.projects.pop();
        a.ensure_selection_valid();
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn tab_focus_defaults_to_projects() {
        let a = AppState::new(date(2026, 5, 4), names(&["a"]));
        assert_eq!(a.focus, Focus::Projects);
    }

    #[test]
    fn move_session_clamps() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.sessions_visible = vec![
            crate::model::SessionId::parse("aaaaaaaa").unwrap(),
            crate::model::SessionId::parse("bbbbbbbb").unwrap(),
        ];
        a.move_session_down();
        a.move_session_down();
        a.move_session_down();
        assert_eq!(a.session_selected, 1);
        a.move_session_up();
        a.move_session_up();
        a.move_session_up();
        assert_eq!(a.session_selected, 0);
    }

    #[test]
    fn ensure_session_selection_valid_snaps_focus_when_empty() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.focus = Focus::Sessions;
        a.sessions_visible.clear();
        a.ensure_session_selection_valid();
        assert_eq!(a.focus, Focus::Projects);
        assert_eq!(a.session_selected, 0);
    }
}
