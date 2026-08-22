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
//!
//! Answering the log for as long as the process lives, which is what both
//! `githerb run` and the review surface do:
//!
//! ```no_run
//! use std::sync::atomic::AtomicBool;
//! use std::time::Duration;
//!
//! # fn main() -> Result<(), runner::Error> {
//! let store = app::Store::at(".")?;
//! let root = store.repo().root().to_path_buf();
//!
//! // One runner per repository, and nothing left over from the last one.
//! let _lock = runner::Lock::acquire(store.repo().git_dir())?;
//! runner::prune_leftovers(store.repo())?;
//!
//! let runner = runner::Runner::new(store, root, app::Identity::runner(), Box::new(|line| {
//!     eprintln!("{line}");
//! }));
//!
//! let shutdown = AtomicBool::new(false);
//! let mut wait = |budget: Duration| {
//!     std::thread::sleep(budget);
//!     false
//! };
//!
//! runner.run(&mut wait, Duration::from_secs(2), &shutdown)?;
//! # Ok(())
//! # }
//! ```

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod agent;
pub mod answers;
pub mod error;
pub mod jobs;
pub mod lock;
pub mod runner;
pub mod tree;

#[cfg(test)]
mod scratch;

pub use agent::Agent;
pub use answers::{Answer, AnswersFile, read_answers};
pub use error::Error;
pub use jobs::{Job, pending};
pub use lock::Lock;
pub use runner::Runner;
pub use tree::{Worktree, prune_leftovers};
