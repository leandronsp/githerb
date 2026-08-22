//! What can go wrong when the storage client is another program.
//!
//! git explains itself better than we could, so a refusal carries the argv
//! that caused it and the stderr it printed, unchanged.

use std::fmt;
use std::path::PathBuf;

/// Every way this crate fails.
#[derive(Debug)]
pub enum Error {
    /// git ran and said no.
    Git {
        /// The arguments after `git`, joined by spaces.
        args: String,
        /// What git printed on stderr, trimmed.
        stderr: String,
    },
    /// The directory given is not inside a git repository.
    NotARepository(PathBuf),
    /// git could not be started, or a path could not be read.
    Io(std::io::Error),
    /// Bytes that were meant to be text and are not utf-8.
    Utf8,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Git { args, stderr } => write!(f, "git {args}: {stderr}"),
            Error::NotARepository(path) => write!(f, "not a git repository: {}", path.display()),
            Error::Io(err) => write!(f, "io error: {err}"),
            Error::Utf8 => write!(f, "not valid utf-8"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::Git { .. } | Error::NotARepository(_) | Error::Utf8 => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
