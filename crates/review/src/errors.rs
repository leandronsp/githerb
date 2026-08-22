//! Every way the core refuses a value, in one enum.
//!
//! A refusal is a value, not a string: the caller matches the variant and the
//! message names the offending thing. The sentences are the ones the Go build
//! wrote, because they are what a person reads on a terminal and what the
//! tests of other tools grep for.

use std::fmt;

use crate::check::CheckName;
use crate::chunk::Field;
use crate::identity::{RecordId, Sha};
use crate::proposal::State;

/// Why the core refused a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A sha field that is not forty lowercase hex characters.
    NoRevision(String),
    /// A blank file path.
    NoFile,
    /// A note or an answer that says nothing.
    NoBody,
    /// A record with no author.
    NoAuthor,
    /// A resolution or an answer that names no note.
    NoTarget,
    /// A side that is neither `old` nor `new`.
    UnknownSide(String),
    /// A span that covers no line, or ends before it starts.
    EmptySpan {
        /// The first line asked for.
        start: i64,
        /// The last line asked for.
        end: i64,
    },
    /// A line that is not a record: bad JSON, a bad timestamp, negative seconds.
    Malformed(String),
    /// A format version this build does not speak. Refused, never skipped.
    Version(i64),
    /// A kind of record this build does not know. The store skips these.
    UnknownKind(String),
    /// A proposal with no name.
    NoProposalId,
    /// A proposal name a ref cannot carry.
    BadProposalId(String),
    /// A proposal or a chunk with no title.
    NoTitle,
    /// A proposal whose head is its base.
    NothingProposed,
    /// A revision the proposal already carries.
    RevisionKnown(Sha),
    /// A record about a revision this proposal never saw.
    UnknownRevision(Sha),
    /// A resolution or an answer naming a note this proposal does not carry.
    UnknownComment(RecordId),
    /// Landing refused because the head still has notes nobody resolved.
    OpenComments {
        /// How many are open.
        count: usize,
        /// Which revision they are on.
        revision: u32,
    },
    /// A change asked of a proposal that has already landed or been given up on.
    NotOpen(State),
    /// A state this build does not know.
    UnknownState(String),
    /// A proposal with no target branch.
    NoBranch,
    /// A branch name git would not accept.
    BadBranch(String),
    /// A check with no name.
    NoCheckName,
    /// A check status that is neither `passed` nor `failed`.
    UnknownStatus(String),
    /// A required check that ran on the head and said no.
    CheckFailed(CheckName),
    /// A required check that never ran on the head.
    CheckMissing(CheckName),
    /// A capped field carrying more than one line.
    NotOneLine(Field),
    /// A capped field longer than its ceiling.
    TooLong {
        /// Which field.
        field: Field,
        /// How long it is, in characters.
        chars: usize,
        /// How long it may be.
        ceiling: usize,
    },
    /// A chunk that does not say how it was and how it is.
    NoBeforeAfter,
    /// A chunk that does not name the call that was made.
    NoDecision,
    /// A task this build does not run.
    UnknownTask(String),
    /// A phase this build does not know.
    UnknownPhase(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRevision(raw) => {
                write!(f, "a comment must name the revision it applies to: {raw}")
            }
            Self::NoFile => write!(f, "a comment must name a file"),
            Self::NoBody => write!(f, "a comment must say something"),
            Self::NoAuthor => write!(f, "a record must name its author"),
            Self::NoTarget => write!(f, "a resolution must name the comment it resolves"),
            Self::UnknownSide(raw) => {
                write!(f, "a span is on the old side or the new side: {raw}")
            }
            Self::EmptySpan { start, end } => write!(
                f,
                "a span covers at least one line, ending at or after it starts: lines {start} to {end}"
            ),
            Self::Malformed(what) => write!(f, "not a record: {what}"),
            Self::Version(version) => write!(
                f,
                "a version of the format this build does not speak: {version}"
            ),
            Self::UnknownKind(kind) => {
                write!(f, "a kind of record this build does not know: {kind}")
            }
            Self::NoProposalId => write!(f, "a proposal must be named"),
            Self::BadProposalId(raw) => write!(f, "not a name a proposal ref can carry: {raw}"),
            Self::NoTitle => write!(f, "a proposal must have a title"),
            Self::NothingProposed => write!(f, "a proposal must move past its base"),
            Self::RevisionKnown(sha) => {
                write!(f, "that revision is already on the proposal: {sha}")
            }
            Self::UnknownRevision(sha) => write!(f, "that revision is not on this proposal: {sha}"),
            Self::UnknownComment(id) => write!(f, "that comment is not on this proposal: {id}"),
            Self::OpenComments { count, revision } => write!(
                f,
                "the head revision still has open comments: {count} on revision {revision}"
            ),
            Self::NotOpen(state) => write!(f, "the proposal is no longer open: it is {state}"),
            Self::UnknownState(raw) => write!(f, "a state this build does not know: {raw}"),
            Self::NoBranch => write!(f, "a proposal must name the branch it lands on"),
            Self::BadBranch(raw) => write!(f, "not a branch name git would accept: {raw}"),
            Self::NoCheckName => write!(f, "a check must be named"),
            Self::UnknownStatus(raw) => write!(f, "a check either passed or failed: {raw}"),
            Self::CheckFailed(name) => {
                write!(f, "a check failed on the head revision: {name}")
            }
            Self::CheckMissing(name) => write!(
                f,
                "a required check has not run on the head revision: {name}"
            ),
            Self::NotOneLine(field) => write!(f, "this is one line, and one line only: {field}"),
            Self::TooLong {
                field,
                chars,
                ceiling,
            } => write!(
                f,
                "longer than the ceiling, say it shorter: {field} is {chars} characters, the ceiling is {ceiling}"
            ),
            Self::NoBeforeAfter => write!(f, "a chunk says how it was and how it is"),
            Self::NoDecision => write!(f, "a chunk names the call that was made"),
            Self::UnknownTask(raw) => write!(f, "a task this build does not run: {raw}"),
            Self::UnknownPhase(raw) => write!(f, "a task starts, finishes or fails: {raw}"),
        }
    }
}

impl std::error::Error for Error {}

/// What the core returns when it can refuse.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_the_value_it_refused() {
        assert_eq!(
            Error::BadBranch("my branch".to_owned()).to_string(),
            "not a branch name git would accept: my branch"
        );
        assert_eq!(
            Error::TooLong {
                field: Field::Decision,
                chars: 300,
                ceiling: 200
            }
            .to_string(),
            "longer than the ceiling, say it shorter: decision is 300 characters, the ceiling is 200"
        );
    }
}
