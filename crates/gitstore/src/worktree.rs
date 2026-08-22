//! Throwaway checkouts, and the rebase that runs inside one.
//!
//! Nothing here touches the checkout you have open. A job gets its own
//! worktree, does whatever it likes to it, and the tree is removed after.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::git::Repo;
use crate::run::{arg, capture, refused, stdout_of, trimmed};

/// Where a rebase ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseOutcome {
    /// It replayed cleanly and the worktree is at the new head.
    Done,
    /// It stopped on a conflict and the worktree is halfway through a rebase.
    Conflicted,
}

impl Repo {
    /// Check `sha` out into its own detached worktree at `dir`.
    pub fn worktree_add(&self, dir: &Path, sha: &str) -> Result<(), Error> {
        self.run(&["worktree", "add", "--detach", arg(dir)?, sha])?;

        Ok(())
    }

    /// Remove a worktree, whatever state it was left in.
    pub fn worktree_remove(&self, dir: &Path) -> Result<(), Error> {
        self.run(&["worktree", "remove", "--force", arg(dir)?])?;

        Ok(())
    }

    /// Forget worktrees whose directories are gone.
    pub fn worktree_prune(&self) -> Result<(), Error> {
        self.run(&["worktree", "prune"])?;

        Ok(())
    }

    /// Replay the commits after `base` onto `onto`, inside a worktree.
    ///
    /// A conflict is an outcome, not an error: git exits non-zero and leaves
    /// the rebase in progress, which is exactly the state a human is asked to
    /// look at. Any other refusal is a failure.
    pub fn rebase_onto(
        &self,
        worktree_dir: &Path,
        onto: &str,
        base: &str,
    ) -> Result<RebaseOutcome, Error> {
        let args = ["rebase", "--onto", onto, base];
        let output = capture(worktree_dir, &args, None)?;

        if output.status.success() {
            return Ok(RebaseOutcome::Done);
        }

        if self.rebasing(worktree_dir) {
            return Ok(RebaseOutcome::Conflicted);
        }

        Err(refused(&args, &output.stderr))
    }

    /// Put a worktree back where the rebase found it.
    pub fn rebase_abort(&self, worktree_dir: &Path) -> Result<(), Error> {
        stdout_of(worktree_dir, &["rebase", "--abort"], None)?;

        Ok(())
    }

    /// Whether git is halfway through a rebase in this worktree, which is what
    /// it looks like when somebody walked away from a conflict.
    #[must_use]
    pub fn rebasing(&self, worktree_dir: &Path) -> bool {
        ["rebase-merge", "rebase-apply"]
            .iter()
            .any(|name| git_path_exists(worktree_dir, name))
    }

    /// The commit a worktree points at now, which after an agent has been
    /// through it is how we know whether anything happened.
    pub fn head(&self, worktree_dir: &Path) -> Result<String, Error> {
        trimmed(stdout_of(worktree_dir, &["rev-parse", "HEAD"], None)?)
    }
}

/// Whether a path inside the worktree's own git directory is there. git
/// answers with an absolute path for a linked worktree and a relative one for
/// the main tree, so both are resolved against the worktree.
fn git_path_exists(worktree_dir: &Path, name: &str) -> bool {
    let Ok(output) = stdout_of(worktree_dir, &["rev-parse", "--git-path", name], None) else {
        return false;
    };

    let Ok(text) = trimmed(output) else {
        return false;
    };

    let path = PathBuf::from(text);

    if path.is_absolute() {
        return path.exists();
    }

    worktree_dir.join(path).exists()
}
