//! Values the tests build on, so a test says what it is about and nothing else.

use crate::branch::Branch;
use crate::check::{Check, CheckName, CheckStatus};
use crate::comment::Comment;
use crate::identity::{Author, FilePath, ProposalId, Sha};
use crate::proposal::Proposal;
use crate::rationale::Rationale;
use crate::reply::Reply;
use crate::resolution::Resolution;
use crate::span::{Anchor, Side, Span};
use crate::timestamp::Timestamp;
use crate::work::{Dispatch, Phase, Task, Work};

/// A commit name nobody has to read, told apart by one number.
pub fn sha(seed: u8) -> Sha {
    Sha::parse(&format!("{seed:02x}").repeat(20)).unwrap()
}

/// Somebody who signs a record.
pub fn author(name: &str) -> Author {
    Author::parse(name).unwrap()
}

/// A moment, that many seconds after the epoch.
pub fn at(unix: i64) -> Timestamp {
    Timestamp::from_unix(unix)
}

/// Lines 42 to 47 of a file, on the new side.
pub fn anchor(file: &str) -> Anchor {
    Anchor::new(
        FilePath::parse(file).unwrap(),
        Span::new(Side::New, 42, 47).unwrap(),
    )
}

/// An open proposal at revision 1, cut from [`sha(0)`] onto `main`.
pub fn proposal() -> Proposal {
    Proposal::open(
        ProposalId::parse("land-the-gate").unwrap(),
        "Land the gate",
        Branch::parse("main").unwrap(),
        sha(0),
        sha(1),
        at(1_000),
    )
    .unwrap()
}

/// A note on a revision.
pub fn note(revision: &Sha, body: &str) -> Comment {
    Comment::new(
        revision.clone(),
        anchor("internal/app/land.go"),
        body,
        author("leandro"),
        at(1_100),
    )
    .unwrap()
}

/// The author explaining a revision.
pub fn explanation(revision: &Sha, body: &str) -> Rationale {
    Rationale::new(
        revision.clone(),
        anchor("internal/app/land.go"),
        body,
        author("leandro"),
        at(1_100),
    )
    .unwrap()
}

/// An answer under a note.
pub fn answer(note: &Comment, body: &str, unix: i64) -> Reply {
    Reply::new(
        note.id().clone(),
        note.revision().clone(),
        body,
        author("claude-code"),
        at(unix),
    )
    .unwrap()
}

/// Somebody saying a note is answered.
pub fn resolution(note: &Comment) -> Resolution {
    Resolution::new(note.id().clone(), author("leandro"), at(1_200))
}

/// A result on a revision.
pub fn check(revision: &Sha, name: &str, status: CheckStatus) -> Check {
    Check::new(
        CheckName::parse(name).unwrap(),
        status,
        revision.clone(),
        41,
        author("githerb-ci@laptop"),
        at(1_300),
    )
}

/// A line of what an agent did.
pub fn work(revision: &Sha, task: Task, phase: Phase, note: Option<&str>, unix: i64) -> Work {
    Work::new(
        revision.clone(),
        task,
        phase,
        author("githerb-run"),
        note,
        at(unix),
    )
    .unwrap()
}

/// A person handing the open notes over.
pub fn dispatch(revision: &Sha, unix: i64) -> Dispatch {
    Dispatch::new(revision.clone(), author("leandro"), at(unix))
}

/// The name of a check the repository requires.
pub fn required(name: &str) -> CheckName {
    CheckName::parse(name).unwrap()
}
