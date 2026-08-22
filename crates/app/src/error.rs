//! Every way a use case refuses.
//!
//! The core refuses values, git refuses commands, and this enum is where the
//! two meet the person who typed something. A refusal carries the offending
//! thing so the sentence can name it, and the sentences are the ones the Go
//! build wrote, because they are what people read on a terminal.

use std::fmt;

use review::{Branch, CheckName, ProposalId, Sha};

use crate::config::ConfigError;

/// What a use case answers with when it will not do the thing.
pub type Result<T> = std::result::Result<T, Error>;

/// Why a use case refused.
#[derive(Debug)]
pub enum Error {
    /// The core refused a value.
    Review(review::Error),
    /// git refused a command.
    Git(gitstore::Error),
    /// A diff that is not a diff.
    Patch(patch::ParseError),
    /// The repository declares something this build cannot read.
    Config(ConfigError),
    /// A description that is not the JSON this build expects.
    Description(String),
    /// A proposal nobody opened, or nobody opened by that name.
    NotFound(ProposalId),
    /// A ref under the proposal namespace whose basename is not a number.
    NotARevision(String),
    /// A revision number this proposal does not carry.
    NoSuchRevision(u32),
    /// A log line this build refuses to read past.
    Log {
        /// Which log: the proposal log or the annotation log.
        log: &'static str,
        /// The object the note is attached to.
        object: Sha,
        /// What the core said about the line.
        source: review::Error,
    },
    /// The target branch moved on, so landing would not be a fast-forward.
    NotFastForward(Branch),
    /// The target is the branch checked out here and the working tree has
    /// something in the way of moving with it.
    WorkingTreeInTheWay {
        /// The branch that is checked out.
        target: Branch,
        /// What git refused to overwrite, first line.
        detail: String,
    },
    /// A check command that died on a signal, which is not a verdict on it.
    CheckKilled(CheckName),
    /// The gate: checks ran on the head revision and some said no.
    CheckFailed {
        /// How many said no.
        failed: usize,
        /// How many ran.
        total: usize,
    },
    /// A file could not be made, read or written.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Review(err) => write!(f, "{err}"),
            Error::Git(err) => write!(f, "{err}"),
            Error::Patch(err) => write!(f, "{err}"),
            Error::Config(err) => write!(f, "{err}"),
            Error::Description(what) => write!(f, "reading the description: {what}"),
            Error::NotFound(id) => write!(f, "proposal {id}: not found"),
            Error::NotARevision(name) => write!(f, "ref {name} is not a revision"),
            Error::NoSuchRevision(number) => {
                write!(f, "that revision is not on this proposal: r{number}")
            }
            Error::Log {
                log,
                object,
                source,
            } => write!(f, "{log} on {object}: {source}"),
            Error::NotFastForward(target) => {
                write!(f, "{target} moved since the proposal was cut")
            }
            Error::WorkingTreeInTheWay { target, detail } => write!(
                f,
                "{target} is checked out here and the working tree is in the way: {detail}; commit or stash, then land"
            ),
            Error::CheckKilled(name) => write!(
                f,
                "a check that was killed is not a check that failed: {name}"
            ),
            Error::CheckFailed { failed, total } => write!(
                f,
                "a check said no: {failed} of {total} failed on the head revision"
            ),
            Error::Io(err) => write!(f, "io error: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Review(err) | Error::Log { source: err, .. } => Some(err),
            Error::Git(err) => Some(err),
            Error::Patch(err) => Some(err),
            Error::Config(err) => Some(err),
            Error::Io(err) => Some(err),
            Error::Description(_)
            | Error::NotFound(_)
            | Error::NotARevision(_)
            | Error::NoSuchRevision(_)
            | Error::NotFastForward(_)
            | Error::WorkingTreeInTheWay { .. }
            | Error::CheckKilled(_)
            | Error::CheckFailed { .. } => None,
        }
    }
}

impl From<review::Error> for Error {
    fn from(err: review::Error) -> Self {
        Error::Review(err)
    }
}

impl From<gitstore::Error> for Error {
    fn from(err: gitstore::Error) -> Self {
        Error::Git(err)
    }
}

impl From<patch::ParseError> for Error {
    fn from(err: patch::ParseError) -> Self {
        Error::Patch(err)
    }
}

impl From<ConfigError> for Error {
    fn from(err: ConfigError) -> Self {
        Error::Config(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_the_thing_it_refused() {
        let branch = Branch::parse("main").unwrap();

        assert_eq!(
            Error::NotFastForward(branch).to_string(),
            "main moved since the proposal was cut"
        );
        assert_eq!(
            Error::CheckFailed {
                failed: 1,
                total: 3
            }
            .to_string(),
            "a check said no: 1 of 3 failed on the head revision"
        );
    }

    #[test]
    fn a_log_line_says_which_log_and_which_object() {
        let err = Error::Log {
            log: "annotation log",
            object: Sha::parse(&"a".repeat(40)).unwrap(),
            source: review::Error::Version(2),
        };

        assert_eq!(
            err.to_string(),
            format!(
                "annotation log on {}: a version of the format this build does not speak: 2",
                "a".repeat(40)
            )
        );
    }
}
