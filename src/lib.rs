//! Library core for zeitlupius. UI front-ends call into the crate.

pub mod error;
pub mod model;
pub mod ops;
pub mod storage;
pub mod time;

pub use error::{Error, ErrorKind, Result};
pub use model::ProjectName;
pub use storage::ProjectStore;
