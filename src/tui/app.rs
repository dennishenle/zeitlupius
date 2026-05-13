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
    /// Cached count of sessions currently shown in the Sessions panel.
    /// Updated by the view layer each draw so input handling can clamp.
    pub session_count: usize,
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
            session_count: 0,
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

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Projects => Focus::Sessions,
            Focus::Sessions => Focus::Projects,
        };
    }

    pub fn clamp_session_selected(&mut self, count: usize) {
        self.session_count = count;
        if count == 0 {
            self.session_selected = 0;
        } else if self.session_selected >= count {
            self.session_selected = count - 1;
        }
    }

    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Projects => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.session_selected = 0;
                }
            }
            Focus::Sessions => {
                if self.session_selected > 0 {
                    self.session_selected -= 1;
                }
            }
        }
    }
    pub fn move_down(&mut self) {
        match self.focus {
            Focus::Projects => {
                if self.selected + 1 < self.projects.len() {
                    self.selected += 1;
                    self.session_selected = 0;
                }
            }
            Focus::Sessions => {
                if self.session_selected + 1 < self.session_count {
                    self.session_selected += 1;
                }
            }
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
        self.session_selected = 0;
    }

    pub fn page_prev(&mut self) {
        self.interval = self.interval.previous();
        self.session_selected = 0;
    }
    pub fn page_next(&mut self) {
        self.interval = self.interval.next();
        self.session_selected = 0;
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
    fn focus_defaults_to_projects() {
        let a = AppState::new(date(2026, 5, 4), names(&["a"]));
        assert_eq!(a.focus, Focus::Projects);
        assert_eq!(a.session_selected, 0);
    }

    #[test]
    fn toggle_focus_cycles() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.toggle_focus();
        assert_eq!(a.focus, Focus::Sessions);
        a.toggle_focus();
        assert_eq!(a.focus, Focus::Projects);
    }

    #[test]
    fn move_down_in_sessions_focus_advances_session_selected() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.focus = Focus::Sessions;
        a.session_count = 3;
        a.move_down();
        assert_eq!(a.session_selected, 1);
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn move_down_in_sessions_clamps_at_end() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.focus = Focus::Sessions;
        a.session_count = 2;
        a.session_selected = 1;
        a.move_down();
        assert_eq!(a.session_selected, 1);
    }

    #[test]
    fn changing_selected_project_resets_session_selected() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a", "b"]));
        a.session_selected = 4;
        a.move_down();
        assert_eq!(a.session_selected, 0);
    }

    #[test]
    fn switch_interval_resets_session_selected() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.session_selected = 3;
        a.switch_interval(IntervalKind::Week, date(2026, 5, 4));
        assert_eq!(a.session_selected, 0);
    }

    #[test]
    fn page_prev_resets_session_selected() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.session_selected = 3;
        a.page_prev();
        assert_eq!(a.session_selected, 0);
    }

    #[test]
    fn clamp_session_selected_drops_when_count_shrinks() {
        let mut a = AppState::new(date(2026, 5, 4), names(&["a"]));
        a.session_selected = 5;
        a.clamp_session_selected(3);
        assert_eq!(a.session_selected, 2);
        a.clamp_session_selected(0);
        assert_eq!(a.session_selected, 0);
    }
}
