//! Errors returned by library operations.
//!
//! Each variant names what went wrong in terms a caller can act on. The
//! message is written for a person; the variant is for code.

use crate::model::ValidationError;

#[derive(Debug)]
pub enum Error {
    /// Input does not match the record format.
    Invalid(ValidationError),
    /// A named record, file, or setting does not exist.
    NotFound(String),
    /// The database changed after a preview, or an expected revision is not current.
    Stale(String),
    /// The operation is refused to protect data or files.
    Refused(String),
    /// The database needs `ath migrate` before this operation can run.
    MigrationRequired(Box<crate::store::migrate::MigrationPreview>),
    /// SQLite reported an error.
    Database(rusqlite::Error),
    /// A file operation failed.
    Io(std::io::Error),
    /// Any other failure, with its message.
    Other(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn other(message: impl Into<String>) -> Self {
        Error::Other(message.into())
    }

    pub fn refused(message: impl Into<String>) -> Self {
        Error::Refused(message.into())
    }

    pub fn stale(message: impl Into<String>) -> Self {
        Error::Stale(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Error::NotFound(message.into())
    }

    /// A short machine-readable name for the error kind.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::NotFound(_) => "not_found",
            Error::Stale(_) => "stale",
            Error::Refused(_) => "refused",
            Error::MigrationRequired(_) => "migration_required",
            Error::Database(_) => "database",
            Error::Io(_) => "io",
            Error::Other(_) => "error",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Invalid(error) => write!(f, "{error}"),
            Error::NotFound(message) | Error::Stale(message) | Error::Refused(message) | Error::Other(message) => {
                f.write_str(message)
            }
            Error::MigrationRequired(preview) => write!(
                f,
                "Database SQL schema {} must be upgraded to {} before other commands can run. Run ath migrate.",
                preview.source_sql_schema_version, preview.target_sql_schema_version
            ),
            Error::Database(error) => write!(f, "{error}"),
            Error::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ValidationError> for Error {
    fn from(error: ValidationError) -> Self {
        Error::Invalid(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::Database(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<crate::json::JsonError> for Error {
    fn from(error: crate::json::JsonError) -> Self {
        Error::Other(error.message)
    }
}
