pub mod app;
pub mod input;
pub mod modals;
pub mod view;

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use jiff::tz::TimeZone;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::error::Result;
use crate::model::{Interval, Project, ProjectName};
use crate::ops;
use crate::storage::ProjectStore;
use crate::tui::app::{AppState, Modal};
use crate::tui::input::{Action, dispatch};
use crate::tui::view::{DashboardData, draw as draw_dashboard};

const TICK: Duration = Duration::from_millis(250);
const RELOAD_EVERY: Duration = Duration::from_millis(1000);
const STATUS_TTL: Duration = Duration::from_secs(3);

pub fn run<S: ProjectStore>(store: &S, tz: &TimeZone) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let result = event_loop(store, tz, &mut term);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    result
}

fn event_loop<S: ProjectStore>(
    store: &S,
    tz: &TimeZone,
    term: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    let now = jiff::Zoned::now();
    let names = ops::list(store)?;
    let mut state = AppState::new(now.date(), names.clone());
    let mut projects = load_all(store, &names)?;
    let mut last_reload = Instant::now();

    while !state.should_quit {
        let now = jiff::Zoned::now();
        if let Some((_, started)) = &state.status_line
            && started.elapsed() > STATUS_TTL
        {
            state.status_line = None;
        }
        term.draw(|f| {
            draw_dashboard(
                f,
                &DashboardData {
                    state: &state,
                    projects: &projects,
                    now: &now,
                    tz,
                },
            );
        })?;

        if event::poll(TICK)?
            && let Event::Key(k) = event::read()?
        {
            let today = now.date();
            let action = dispatch(k, &mut state, today);
            match handle_action(store, action, &mut state, &now) {
                Ok(true) => {
                    let names = ops::list(store)?;
                    state.projects = names.clone();
                    state.ensure_selection_valid();
                    projects = load_all(store, &names)?;
                }
                Ok(false) => {}
                Err(e) => state.set_status(format!("error: {e}")),
            }
        }

        if last_reload.elapsed() >= RELOAD_EVERY {
            let names = ops::list(store)?;
            state.projects = names.clone();
            state.ensure_selection_valid();
            projects = load_all(store, &names)?;
            last_reload = Instant::now();
        }
    }
    Ok(())
}

fn load_all<S: ProjectStore>(store: &S, names: &[ProjectName]) -> Result<Vec<Project>> {
    names.iter().map(|n| ops::load(store, n)).collect()
}

fn handle_action<S: ProjectStore>(
    store: &S,
    action: Action,
    state: &mut AppState,
    now: &jiff::Zoned,
) -> Result<bool> {
    match action {
        Action::None => Ok(false),
        Action::Reload => Ok(true),
        Action::StartSelected => {
            let Some(n) = state.selected_project().cloned() else {
                return Ok(false);
            };
            ops::start(store, &n, now, None)?;
            state.set_status(format!("started {n}"));
            Ok(true)
        }
        Action::StopSelected => {
            let Some(n) = state.selected_project().cloned() else {
                return Ok(false);
            };
            ops::stop(store, &n, now)?;
            state.set_status(format!("stopped {n}"));
            Ok(true)
        }
        Action::DeleteSelected => {
            let Some(n) = state.selected_project().cloned() else {
                return Ok(false);
            };
            let p = ops::load(store, &n)?;
            state.modal = Modal::ConfirmDelete {
                project: n,
                running: p.running_session().is_some(),
            };
            Ok(false)
        }
        Action::DeleteConfirmed => {
            let Modal::ConfirmDelete { project, .. } =
                std::mem::replace(&mut state.modal, Modal::None)
            else {
                return Ok(false);
            };
            ops::delete(store, &project)?;
            state.set_status(format!("deleted {project}"));
            Ok(true)
        }
        Action::NewProject(raw) => {
            let n = ProjectName::parse(&raw)?;
            ops::create(store, &n)?;
            state.set_status(format!("created {n}"));
            Ok(true)
        }
        Action::ApplyCustomInterval(from, to) => {
            state.interval = Interval::Custom { from, to };
            Ok(false)
        }
        Action::ToggleFocus => Ok(false),
    }
}
