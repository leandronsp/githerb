//! The loop that answers the log.
//!
//! A job is derived from the same records the browser reads: whether somebody
//! handed a proposal over, whether the target ran ahead of it, whether the head
//! has been through what the repository declares. Nothing here decides what a
//! proposal means, and nothing here is stored, so a runner that dies leaves no
//! queue behind and the next one works the same answer out of the same log.
//!
//! What it owns is the mechanics: one runner per repository, one job at a time
//! because an agent job costs somebody money, a throwaway worktree per job so
//! nothing touches the checkout you have open, and two lines in the log around
//! every job so that a claim is visible and a failure is readable.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod agent;
pub mod answers;
pub mod error;
pub mod jobs;
pub mod lock;
pub mod tree;

#[cfg(test)]
mod scratch;

pub use agent::Agent;
pub use answers::{Answer, AnswersFile, read_answers};
pub use error::Error;
pub use jobs::{Job, pending};
pub use lock::Lock;
pub use tree::{Worktree, prune_leftovers};
