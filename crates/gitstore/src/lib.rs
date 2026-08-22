//! The git binary as storage: refs, notes, objects, diffs and worktrees.
//!
//! Git is the database and the git binary is its client, because git is the
//! one program guaranteed to agree with git. This crate knows nothing about
//! what a record means; it moves refs, appends note lines and reads objects
//! back, and it does the reading in a fixed number of processes however large
//! the log gets.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod error;
mod git;
mod notes;
mod run;
mod worktree;

pub use error::Error;
pub use git::Repo;
pub use worktree::RebaseOutcome;

/// The namespace holding one ref per revision of every proposal.
pub const PROPOSAL_REFS: &str = "refs/githerb/proposals";

/// The notes ref carrying proposal lifecycle events, in the short form git
/// wants after `--ref=`.
pub const EVENT_NOTES: &str = "githerb/proposals";

/// The notes ref carrying annotations, in the short form git wants after
/// `--ref=`.
pub const RECORD_NOTES: &str = "githerb/annotations";
