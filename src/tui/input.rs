use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{AppState, IntervalKind, Modal};

pub enum Action {
    None,
    StartSelected,
    StopSelected,
    DeleteSelected,
    NewProject(String),
    DeleteConfirmed,
    DeleteSessionConfirmed,
    ApplyCustomInterval(jiff::civil::Date, jiff::civil::Date),
    Reload,
    EditNoteConfirmed {
        project: crate::model::ProjectName,
        session_id: crate::model::SessionId,
        note: Option<String>,
    },
}

pub fn dispatch(key: KeyEvent, state: &mut AppState, today: jiff::civil::Date) -> Action {
    let modal = state.modal.clone();
    match modal {
        Modal::None => dispatch_dashboard(key, state, today),
        Modal::NewProject { input } => dispatch_new_project(key, input, state),
        Modal::ConfirmDelete { .. } | Modal::ConfirmDeleteSession { .. } => {
            dispatch_confirm_delete(key, state)
        }
        Modal::CustomInterval {
            from, to, focus_to, ..
        } => dispatch_custom(key, from, to, focus_to, state),
        Modal::EditNote { input, .. } => dispatch_edit_note(key, input, state),
        Modal::Help => {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                state.modal = Modal::None;
            }
            Action::None
        }
    }
}

fn dispatch_dashboard(key: KeyEvent, state: &mut AppState, today: jiff::civil::Date) -> Action {
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.should_quit = true;
        return Action::None;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
            Action::None
        }
        KeyCode::Char('?') => {
            state.modal = Modal::Help;
            Action::None
        }
        KeyCode::Tab => {
            state.toggle_focus();
            Action::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            use crate::tui::app::Focus;
            match state.focus {
                Focus::Projects => state.move_down(),
                Focus::Sessions => state.move_session_down(),
            }
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            use crate::tui::app::Focus;
            match state.focus {
                Focus::Projects => state.move_up(),
                Focus::Sessions => state.move_session_up(),
            }
            Action::None
        }
        KeyCode::Char('d') => {
            state.switch_interval(IntervalKind::Day, today);
            Action::None
        }
        KeyCode::Char('w') => {
            state.switch_interval(IntervalKind::Week, today);
            Action::None
        }
        KeyCode::Char('m') => {
            state.switch_interval(IntervalKind::Month, today);
            Action::None
        }
        KeyCode::Char('y') => {
            state.switch_interval(IntervalKind::Year, today);
            Action::None
        }
        KeyCode::Left => {
            state.page_prev();
            Action::None
        }
        KeyCode::Right => {
            state.page_next();
            Action::None
        }
        KeyCode::Home | KeyCode::Char('g') => {
            use crate::model::Interval;
            state.interval = match state.interval {
                Interval::Day(_) => Interval::Day(today),
                Interval::Week(_) => Interval::Week(today),
                Interval::Month(_) => Interval::Month(today),
                Interval::Year(_) => Interval::Year(today.year()),
                Interval::Custom { .. } => Interval::Day(today),
            };
            Action::None
        }
        KeyCode::Char('c') => {
            state.modal = Modal::CustomInterval {
                from: String::new(),
                to: String::new(),
                focus_to: false,
                error: None,
            };
            Action::None
        }
        KeyCode::Char('n') => {
            state.modal = Modal::NewProject {
                input: String::new(),
            };
            Action::None
        }
        KeyCode::Char('s') => Action::StartSelected,
        KeyCode::Char('S') => Action::StopSelected,
        KeyCode::Char('D') => Action::DeleteSelected,
        KeyCode::Char('e') if matches!(state.focus, crate::tui::app::Focus::Sessions) => {
            let Some(n) = state.selected_project().cloned() else {
                return Action::None;
            };
            let Some(id) = state.sessions_visible.get(state.session_selected).cloned() else {
                return Action::None;
            };
            state.modal = Modal::EditNote {
                project: n,
                session_id: id,
                input: String::new(),
            };
            Action::None
        }
        KeyCode::Char('r') => Action::Reload,
        _ => Action::None,
    }
}

fn dispatch_new_project(key: KeyEvent, mut input: String, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
            Action::None
        }
        KeyCode::Enter => {
            state.modal = Modal::None;
            Action::NewProject(input)
        }
        KeyCode::Backspace => {
            input.pop();
            state.modal = Modal::NewProject { input };
            Action::None
        }
        KeyCode::Char(c) => {
            input.push(c);
            state.modal = Modal::NewProject { input };
            Action::None
        }
        _ => Action::None,
    }
}

fn dispatch_confirm_delete(key: KeyEvent, state: &mut AppState) -> Action {
    let is_session = matches!(state.modal, Modal::ConfirmDeleteSession { .. });
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if is_session {
                Action::DeleteSessionConfirmed
            } else {
                Action::DeleteConfirmed
            }
        }
        _ => {
            state.modal = Modal::None;
            Action::None
        }
    }
}

fn dispatch_custom(
    key: KeyEvent,
    mut from: String,
    mut to: String,
    mut focus_to: bool,
    state: &mut AppState,
) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
            Action::None
        }
        KeyCode::Tab => {
            focus_to = !focus_to;
            state.modal = Modal::CustomInterval {
                from,
                to,
                focus_to,
                error: None,
            };
            Action::None
        }
        KeyCode::Backspace => {
            if focus_to {
                to.pop();
            } else {
                from.pop();
            }
            state.modal = Modal::CustomInterval {
                from,
                to,
                focus_to,
                error: None,
            };
            Action::None
        }
        KeyCode::Char(c) => {
            if focus_to {
                to.push(c);
            } else {
                from.push(c);
            }
            state.modal = Modal::CustomInterval {
                from,
                to,
                focus_to,
                error: None,
            };
            Action::None
        }
        KeyCode::Enter => match (parse_dot_date(&from), parse_dot_date(&to)) {
            (Some(f), Some(t)) if f <= t => {
                state.modal = Modal::None;
                Action::ApplyCustomInterval(f, t)
            }
            _ => {
                state.modal = Modal::CustomInterval {
                    from,
                    to,
                    focus_to,
                    error: Some("invalid dates".into()),
                };
                Action::None
            }
        },
        _ => Action::None,
    }
}

fn parse_dot_date(s: &str) -> Option<jiff::civil::Date> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let dd: i8 = parts[0].parse().ok()?;
    let mm: i8 = parts[1].parse().ok()?;
    let yyyy: i16 = parts[2].parse().ok()?;
    jiff::civil::Date::new(yyyy, mm, dd).ok()
}

fn dispatch_edit_note(key: KeyEvent, mut input: String, state: &mut AppState) -> Action {
    let Modal::EditNote {
        project,
        session_id,
        ..
    } = state.modal.clone()
    else {
        return Action::None;
    };
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
            Action::None
        }
        KeyCode::Enter => {
            let note = if input.is_empty() { None } else { Some(input) };
            state.modal = Modal::None;
            Action::EditNoteConfirmed {
                project,
                session_id,
                note,
            }
        }
        KeyCode::Backspace => {
            input.pop();
            state.modal = Modal::EditNote {
                project: project.clone(),
                session_id: session_id.clone(),
                input,
            };
            Action::None
        }
        KeyCode::Char(c) => {
            input.push(c);
            state.modal = Modal::EditNote {
                project: project.clone(),
                session_id: session_id.clone(),
                input,
            };
            Action::None
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventKind, KeyEventState};
    use jiff::civil::date;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent {
            code: c,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
    fn names(xs: &[&str]) -> Vec<crate::model::ProjectName> {
        xs.iter()
            .map(|x| crate::model::ProjectName::parse(x).unwrap())
            .collect()
    }

    #[test]
    fn q_quits() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        let _ = dispatch(key(KeyCode::Char('q')), &mut s, date(2026, 5, 4));
        assert!(s.should_quit);
    }

    #[test]
    fn s_returns_start_action() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        let a = dispatch(key(KeyCode::Char('s')), &mut s, date(2026, 5, 4));
        assert!(matches!(a, Action::StartSelected));
    }

    #[test]
    fn n_opens_modal_and_typing_appends() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        dispatch(key(KeyCode::Char('n')), &mut s, date(2026, 5, 4));
        dispatch(key(KeyCode::Char('p')), &mut s, date(2026, 5, 4));
        let act = dispatch(key(KeyCode::Enter), &mut s, date(2026, 5, 4));
        match act {
            Action::NewProject(name) => assert_eq!(name, "p"),
            _ => panic!(),
        }
    }

    #[test]
    fn arrow_left_pages_back() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        dispatch(key(KeyCode::Left), &mut s, date(2026, 5, 4));
        match s.interval {
            crate::model::Interval::Day(d) => assert_eq!(d, date(2026, 5, 3)),
            _ => panic!(),
        }
    }

    use crate::tui::app::Focus;

    #[test]
    fn tab_toggles_focus_outside_modals() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        dispatch(key(KeyCode::Tab), &mut s, date(2026, 5, 4));
        assert_eq!(s.focus, Focus::Sessions);
        dispatch(key(KeyCode::Tab), &mut s, date(2026, 5, 4));
        assert_eq!(s.focus, Focus::Projects);
    }

    #[test]
    fn jk_routes_to_sessions_when_focused() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        s.focus = Focus::Sessions;
        s.sessions_visible = vec![
            crate::model::SessionId::parse("aaaaaaaa").unwrap(),
            crate::model::SessionId::parse("bbbbbbbb").unwrap(),
        ];
        dispatch(key(KeyCode::Char('j')), &mut s, date(2026, 5, 4));
        assert_eq!(s.session_selected, 1);
        assert_eq!(s.selected, 0, "project cursor must not move");
    }

    #[test]
    fn d_in_sessions_focus_opens_session_delete_modal() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        s.focus = Focus::Sessions;
        let act = dispatch(key(KeyCode::Char('D')), &mut s, date(2026, 5, 4));
        assert!(matches!(act, Action::DeleteSelected));
    }

    #[test]
    fn e_opens_edit_note_modal_in_sessions_focus() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        s.focus = Focus::Sessions;
        s.sessions_visible = vec![crate::model::SessionId::parse("aaaaaaaa").unwrap()];
        dispatch(key(KeyCode::Char('e')), &mut s, date(2026, 5, 4));
        assert!(matches!(s.modal, Modal::EditNote { .. }));
    }

    #[test]
    fn e_does_nothing_in_projects_focus() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        s.focus = Focus::Projects;
        let act = dispatch(key(KeyCode::Char('e')), &mut s, date(2026, 5, 4));
        assert!(matches!(act, Action::None));
        assert!(!matches!(s.modal, Modal::EditNote { .. }));
    }

    #[test]
    fn edit_note_modal_typing_and_confirm() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        s.focus = Focus::Sessions;
        s.sessions_visible = vec![crate::model::SessionId::parse("aaaaaaaa").unwrap()];
        dispatch(key(KeyCode::Char('e')), &mut s, date(2026, 5, 4));
        assert!(matches!(s.modal, Modal::EditNote { .. }));
        // Type some characters
        for ch in "test note".chars() {
            let _ = dispatch(key(KeyCode::Char(ch)), &mut s, date(2026, 5, 4));
        }
        let act = dispatch(key(KeyCode::Enter), &mut s, date(2026, 5, 4));
        match act {
            Action::EditNoteConfirmed { note, .. } => {
                assert_eq!(note.as_deref(), Some("test note"))
            }
            _ => panic!("expected EditNoteConfirmed"),
        }
    }

    #[test]
    fn edit_note_cancel_on_esc() {
        let mut s = AppState::new(date(2026, 5, 4), names(&["a"]));
        s.focus = Focus::Sessions;
        s.sessions_visible = vec![crate::model::SessionId::parse("aaaaaaaa").unwrap()];
        dispatch(key(KeyCode::Char('e')), &mut s, date(2026, 5, 4));
        let act = dispatch(key(KeyCode::Esc), &mut s, date(2026, 5, 4));
        assert!(matches!(act, Action::None));
        assert!(matches!(s.modal, Modal::None));
    }
}
