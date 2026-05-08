pub mod app;
pub mod input;
pub mod modals;
pub mod view;

use jiff::tz::TimeZone;

use crate::error::Result;
use crate::storage::ProjectStore;

pub fn run<S: ProjectStore>(_store: &S, _tz: &TimeZone) -> Result<()> {
    eprintln!("(TUI not implemented yet)");
    Ok(())
}
