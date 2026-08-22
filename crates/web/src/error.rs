//! What can go wrong in the server itself.
//!
//! Two things only: the address refused to bind, or the listener failed after
//! it did. Everything a client gets wrong is a status code, not an error,
//! because a bad request is the server working, not the server broken.

use std::fmt;
use std::io;

/// Why the server could not run.
#[derive(Debug)]
pub enum Error {
    /// The listener could not be created on that address.
    Bind {
        /// The address as it was given.
        addr: String,
        /// What the operating system said about it.
        cause: io::Error,
    },
    /// The listener failed while it was accepting connections.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind { addr, cause } => write!(f, "cannot bind {addr}: {cause}"),
            Self::Io(cause) => write!(f, "listener failed: {cause}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bind { addr: _, cause } | Self::Io(cause) => Some(cause),
        }
    }
}

impl From<io::Error> for Error {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}
