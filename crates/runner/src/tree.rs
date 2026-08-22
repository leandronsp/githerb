//! A throwaway checkout for one job.
//!
//! Nothing a job does touches the checkout you have open. It gets its own
//! worktree in the temp directory, does whatever it likes to it, and the
//! directory goes away when the value does.
//!
//! Removing a worktree is two steps, and skipping either leaks. `rm -rf` alone
//! leaves git still listing it; `worktree remove` alone leaves the directory
//! when git refuses. A process killed between the two leaves both, which is
//! what [`prune_leftovers`] is for at start-up.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use gitstore::Repo;

use crate::error::Error;

/// What every worktree directory this build makes is called.
const PREFIX: &str = "githerb-work-";

/// Tells apart two worktrees of one process, which is a case a second job in
/// the same pass would otherwise get wrong.
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A detached checkout of one commit, removed when it is dropped.
#[derive(Debug)]
pub struct Worktree {
    repo: Repo,
    dir: PathBuf,
}

impl Worktree {
    /// Check `sha` out into a directory of its own.
    ///
    /// # Errors
    ///
    /// Whatever git says about adding the worktree, usually a revision it
    /// cannot resolve.
    pub fn open(repo: &Repo, sha: &str) -> Result<Worktree, Error> {
        let dir = std::env::temp_dir().join(format!(
            "{PREFIX}{}-{}",
            process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        if let Err(refused) = repo.worktree_add(&dir, sha) {
            // git creates the directory before it decides, so a refusal can
            // still leave one behind.
            let _ = fs::remove_dir_all(&dir);

            return Err(Error::Git(refused));
        }

        Ok(Worktree {
            repo: repo.clone(),
            dir,
        })
    }

    /// Where the checkout is, which is where a job runs.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// The commit it points at now, which is how we know whether the agent
    /// left anything behind.
    ///
    /// # Errors
    ///
    /// Whatever git says about the worktree.
    pub fn head(&self) -> Result<String, Error> {
        Ok(self.repo.head(&self.dir)?)
    }

    /// Whether git is halfway through a rebase here, which is what it looks
    /// like when the agent walked away from a conflict.
    #[must_use]
    pub fn rebasing(&self) -> bool {
        self.repo.rebasing(&self.dir)
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        // Both refusals are ignored on purpose: there is nobody to tell inside
        // a drop, and whatever survives is a directory the next start-up
        // prunes. Failing loudly here would replace a leaked directory with a
        // panic in the middle of a job.
        let _ = self.repo.worktree_remove(&self.dir);
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Remove the worktrees an earlier run left behind, and let git forget them.
///
/// Anything under the temp directory named for this build and not made by this
/// process belongs to a run that is over: the runner lock is held by the time
/// this is called, so no other runner of this repository is alive. A directory
/// belonging to a runner of *another* repository on the same machine would be
/// removed too, which is the price of not asking the operating system whether
/// a pid is alive. It costs that runner one job, and git prunes what is left.
///
/// # Errors
///
/// The temp directory cannot be read, or git refuses to prune.
pub fn prune_leftovers(repo: &Repo) -> Result<(), Error> {
    let ours = format!("{PREFIX}{}-", process::id());
    let temp = std::env::temp_dir();

    for entry in fs::read_dir(&temp)?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };

        if name.starts_with(PREFIX) && !name.starts_with(&ours) {
            let _ = fs::remove_dir_all(temp.join(name));
        }
    }

    repo.worktree_prune()?;

    Ok(())
}
