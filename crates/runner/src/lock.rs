//! One runner per repository, enforced by the operating system.
//!
//! The lock is an advisory lock on a file in the git directory, not a pidfile.
//! A pidfile has to guess whether the process that wrote it is still alive,
//! and it guesses wrong on a recycled pid and on two runners starting at once.
//! An `flock` needs no guess: the kernel drops it when the descriptor closes,
//! which includes the case nobody handles, a hard kill.
//!
//! The file still carries the pid, for the person who finds it and wants to
//! know who to look for. Nothing reads it back.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use crate::error::Error;

/// What the lock file is called inside the git directory.
const LOCK_FILE: &str = "githerb-runner.lock";

/// The right to answer the log for one repository.
///
/// Hold it for as long as the runner runs and drop it to release; there is no
/// close, because a lock you can forget to release is a lock you will forget
/// to release.
#[derive(Debug)]
pub struct Lock {
    file: File,
    path: PathBuf,
}

impl Lock {
    /// Take the lock, or say who has it.
    ///
    /// `git_dir` is the common git directory, so every worktree of a
    /// repository contends for the same lock.
    ///
    /// # Errors
    ///
    /// [`Error::Busy`] when another runner holds it, and [`Error::Io`] when
    /// the file cannot be opened or written.
    pub fn acquire(git_dir: &Path) -> Result<Lock, Error> {
        let path = git_dir.join(LOCK_FILE);

        // Never truncate on open: the holder's pid is in there and this
        // process has not been given the lock yet.
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)?;

        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(TryLockError::Error(cause)) => return Err(Error::Io(cause)),
        }

        file.set_len(0)?;
        writeln!(file, "{}", process::id())?;
        file.flush()?;

        Ok(Lock { file, path })
    }

    /// Where the lock file is, for a message that has to name it.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Closing the descriptor releases it anyway; this only makes the
        // moment explicit. The file itself stays: removing it would let a
        // second runner create a fresh one and lock that instead of this.
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn a_second_runner_on_one_repository_is_refused() -> Result<(), Error> {
        let scratch = Scratch::new("lock-refuses");
        let held = Lock::acquire(scratch.path())?;

        let refused = Lock::acquire(scratch.path());

        assert!(matches!(refused, Err(Error::Busy)), "{refused:?}");
        assert_eq!(held.path(), scratch.path().join(LOCK_FILE));
        Ok(())
    }

    #[test]
    fn dropping_the_lock_hands_the_repository_back() -> Result<(), Error> {
        let scratch = Scratch::new("lock-frees");
        drop(Lock::acquire(scratch.path())?);

        let after = Lock::acquire(scratch.path());

        assert!(after.is_ok(), "{after:?}");
        Ok(())
    }

    #[test]
    fn the_lock_file_says_which_process_holds_it() -> Result<(), Error> {
        let scratch = Scratch::new("lock-pid");
        let held = Lock::acquire(scratch.path())?;

        let written = std::fs::read_to_string(held.path())?;

        assert_eq!(written, format!("{}\n", process::id()));
        Ok(())
    }

    #[test]
    fn a_lock_beside_a_stale_one_is_taken_not_refused() -> Result<(), Error> {
        let scratch = Scratch::new("lock-stale");
        std::fs::write(scratch.path().join(LOCK_FILE), "999999\n")?;

        let taken = Lock::acquire(scratch.path())?;

        assert_eq!(
            std::fs::read_to_string(taken.path())?,
            format!("{}\n", process::id())
        );
        Ok(())
    }
}
