use std::fmt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("project '{0}' does not exist")]
    ProjectNotFound(String),
    #[error("project '{0}' already exists")]
    ProjectAlreadyExists(String),
    #[error("invalid project name '{0}': {1}")]
    InvalidProjectName(String, &'static str),
    #[error("invalid session id '{0}': {1}")]
    InvalidSessionId(String, &'static str),
    #[error("project '{0}' is already running (session started at {1})")]
    AlreadyRunning(String, String),
    #[error("project '{0}' is not running")]
    NotRunning(String),
    #[error("invalid date '{0}': expected DD.MM.YYYY")]
    InvalidDate(String),
    #[error("invalid range: {from} is after {to}")]
    InvalidRange { from: String, to: String },
    #[error("data dir is corrupted: {0}")]
    Corrupt(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    User,
    Internal,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::ProjectNotFound(_)
            | Error::ProjectAlreadyExists(_)
            | Error::InvalidProjectName(_, _)
            | Error::InvalidSessionId(_, _)
            | Error::AlreadyRunning(_, _)
            | Error::NotRunning(_)
            | Error::InvalidDate(_)
            | Error::InvalidRange { .. } => ErrorKind::User,
            Error::Corrupt(_) | Error::Io(_) | Error::Csv(_) => ErrorKind::Internal,
        }
    }
}

#[allow(dead_code)]
fn _force_fmt_use(_: &dyn fmt::Display) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_errors_have_user_kind() {
        let e = Error::ProjectNotFound("x".into());
        assert_eq!(e.kind(), ErrorKind::User);
    }

    #[test]
    fn corrupt_is_internal() {
        let e = Error::Corrupt("bad row".into());
        assert_eq!(e.kind(), ErrorKind::Internal);
    }

    #[test]
    fn display_matches_thiserror_format() {
        let e = Error::ProjectNotFound("foo".into());
        assert_eq!(format!("{e}"), "project 'foo' does not exist");
    }
}
