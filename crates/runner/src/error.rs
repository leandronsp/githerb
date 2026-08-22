//! Every way answering the log can stop.
//!
//! A refusal is a value with a sentence on it, because most of these end up in
//! a `failed` record that a person reads a day later. The sentences are the
//! ones the Go build wrote, so a log written by one binary still reads the
//! same under the other.

use std::fmt;
use std::io;

/// What went wrong while the runner was working.
#[derive(Debug)]
pub enum Error {
    /// Another runner holds this repository. One at a time, because an agent
    /// job costs money.
    Busy,
    /// A job needed an agent and the repository declares none.
    NoAgent,
    /// The agent exited non-zero. The string is the last thing it said.
    AgentStopped(String),
    /// The runner was asked to stop while a job was in flight.
    Stopped,
    /// The agent left the worktree exactly where it found it.
    NothingChanged,
    /// Nothing on this revision is open, so there is nothing to hand over.
    NothingToApply,
    /// A rebase stopped on a conflict nobody resolved.
    ConflictsLeft,
    /// The gate ran on the head revision and some of it said no.
    ChecksFailed {
        /// How many said no.
        failed: usize,
        /// How many ran.
        total: usize,
    },
    /// A use case refused.
    App(app::Error),
    /// git said no.
    Git(gitstore::Error),
    /// A file, a directory or a child process failed.
    Io(io::Error),
    /// The core refused a record the runner tried to write.
    Review(review::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("a runner is already on this repository"),
            Self::NoAgent => {
                f.write_str("this repository declares no [agent] command in .githerb.toml")
            }
            Self::AgentStopped(said) => write!(f, "the agent stopped: {said}"),
            Self::Stopped => f.write_str("runner stopped"),
            Self::NothingChanged => f.write_str("the agent left the worktree where it found it"),
            Self::NothingToApply => f.write_str("nothing is open on this revision"),
            Self::ConflictsLeft => f.write_str("the rebase is still conflicted"),
            Self::ChecksFailed { failed, total } => {
                write!(f, "{failed} of {total} checks failed")
            }
            Self::App(cause) => write!(f, "{cause}"),
            Self::Git(cause) => write!(f, "{cause}"),
            Self::Io(cause) => write!(f, "io error: {cause}"),
            Self::Review(cause) => write!(f, "{cause}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Git(cause) => Some(cause),
            Self::Io(cause) => Some(cause),
            Self::Review(cause) => Some(cause),
            Self::App(cause) => Some(cause),
            Self::ChecksFailed { .. }
            | Self::Busy
            | Self::NoAgent
            | Self::AgentStopped(_)
            | Self::Stopped
            | Self::NothingChanged
            | Self::NothingToApply
            | Self::ConflictsLeft => None,
        }
    }
}

impl From<gitstore::Error> for Error {
    fn from(cause: gitstore::Error) -> Self {
        Self::Git(cause)
    }
}

impl From<io::Error> for Error {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}

impl From<app::Error> for Error {
    fn from(cause: app::Error) -> Self {
        Self::App(cause)
    }
}

impl From<review::Error> for Error {
    fn from(cause: review::Error) -> Self {
        Self::Review(cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_says_what_it_was_in_one_lowercase_line() {
        assert_eq!(
            Error::Busy.to_string(),
            "a runner is already on this repository"
        );
        assert_eq!(
            Error::AgentStopped("could not find claude".to_owned()).to_string(),
            "the agent stopped: could not find claude"
        );
        assert_eq!(
            Error::NothingChanged.to_string(),
            "the agent left the worktree where it found it"
        );
    }

    #[test]
    fn a_wrapped_refusal_keeps_the_sentence_the_other_crate_wrote() {
        let refused = Error::from(review::Error::NoAuthor);

        assert_eq!(refused.to_string(), review::Error::NoAuthor.to_string());
    }
}
