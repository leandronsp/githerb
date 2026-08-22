//! The repository, and every git call that is not a note or a worktree.
//!
//! `Repo` is a resolved pair of paths and nothing else. It holds no cache and
//! no handle, so two of them on the same directory cannot disagree.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::run::{ANSWER_IS_NO, stdout_of, stdout_or, trimmed};

/// A git repository on disk, resolved to its working tree and its git
/// directory.
#[derive(Debug, Clone)]
pub struct Repo {
    root: PathBuf,
    git_dir: PathBuf,
}

impl Repo {
    /// Point at the repository containing `dir`.
    ///
    /// `dir` may be any path inside it; the root comes from
    /// `rev-parse --show-toplevel` and the git directory from
    /// `rev-parse --git-common-dir`, which for a linked worktree is the one
    /// shared by all of them and therefore the one a lock belongs in.
    pub fn open(dir: impl AsRef<Path>) -> Result<Repo, Error> {
        let dir = dir.as_ref();

        let toplevel = stdout_of(dir, &["rev-parse", "--show-toplevel"], None)
            .map_err(|err| not_a_repository(err, dir))?;
        let root = PathBuf::from(trimmed(toplevel)?);

        // Asked from the root, git answers `.git`, which only joins back onto
        // the root correctly because that is where it was asked.
        let common = PathBuf::from(trimmed(stdout_of(
            &root,
            &["rev-parse", "--git-common-dir"],
            None,
        )?)?);
        let git_dir = if common.is_absolute() {
            common
        } else {
            root.join(common)
        };

        Ok(Repo { root, git_dir })
    }

    /// The top of the working tree.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The git directory shared by every worktree, where a lock belongs.
    #[must_use]
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// Run git at the root and read its output as text.
    pub fn run(&self, args: &[&str]) -> Result<String, Error> {
        trimmed(stdout_of(&self.root, args, None)?)
    }

    /// Run git at the root, feeding it `input` on stdin.
    pub fn run_with_stdin(&self, args: &[&str], input: &str) -> Result<String, Error> {
        trimmed(stdout_of(&self.root, args, Some(input))?)
    }

    // --- objects and refs ---

    /// Turn anything git accepts as a revision into a commit sha.
    pub fn resolve(&self, revision: &str) -> Result<String, Error> {
        let spec = format!("{revision}^{{commit}}");

        self.run(&["rev-parse", "--verify", "--end-of-options", &spec])
    }

    /// The commit a branch points at, named by its full ref
    /// (`refs/heads/main`).
    pub fn head_of(&self, branch_ref: &str) -> Result<String, Error> {
        self.resolve(branch_ref)
    }

    /// The commit two revisions last had in common.
    pub fn merge_base(&self, one: &str, other: &str) -> Result<String, Error> {
        self.run(&["merge-base", one, other])
    }

    /// Whether `ancestor` is reachable from `descendant`.
    ///
    /// git says no by exiting 1, which is an answer and not a failure.
    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, Error> {
        let args = ["merge-base", "--is-ancestor", ancestor, descendant];

        Ok(stdout_or(&self.root, &args, ANSWER_IS_NO)?.is_some())
    }

    /// The patch between two commits, with renames detected and no colour.
    pub fn diff(&self, from: &str, to: &str) -> Result<String, Error> {
        let range = format!("{from}..{to}");

        self.run(&["diff", "--no-color", "-M", &range])
    }

    /// The first line of a commit's message.
    pub fn subject(&self, sha: &str) -> Result<String, Error> {
        self.run(&["log", "-1", "--format=%s", sha])
    }

    /// Move a ref.
    ///
    /// With `old` given this is a compare-and-swap: git refuses when the ref
    /// no longer points where the caller last saw it, and the loser of a race
    /// gets that refusal rather than a silent overwrite.
    pub fn update_ref(&self, name: &str, new: &str, old: Option<&str>) -> Result<(), Error> {
        let Some(old) = old else {
            self.run(&["update-ref", name, new])?;

            return Ok(());
        };

        self.run(&["update-ref", name, new, old])?;

        Ok(())
    }

    /// Every ref under `prefix`, as (refname, sha), in one process.
    pub fn refs(&self, prefix: &str) -> Result<Vec<(String, String)>, Error> {
        let listing = self.run(&["for-each-ref", "--format=%(refname) %(objectname)", prefix])?;

        Ok(pairs(&listing))
    }

    /// One cheap string that changes whenever anything this tool reads
    /// changed.
    ///
    /// Appending to a note rewrites the notes tree and moves its ref, so a
    /// single `for-each-ref` over the three namespaces covers proposals,
    /// annotations and branches. This is what the watcher compares; only
    /// equality means anything.
    pub fn fingerprint(&self) -> Result<String, Error> {
        self.run(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/githerb/",
            "refs/notes/githerb/",
            "refs/heads/",
        ])
    }

    // --- config ---

    /// Read a config key, or `None` when it is not set.
    pub fn config_get(&self, key: &str) -> Result<Option<String>, Error> {
        let args = ["config", "--get", key];

        let Some(value) = stdout_or(&self.root, &args, ANSWER_IS_NO)? else {
            return Ok(None);
        };

        Ok(Some(trimmed(value)?))
    }

    /// Write a config key into the repository's own config.
    pub fn config_set(&self, key: &str, value: &str) -> Result<(), Error> {
        self.run(&["config", key, value])?;

        Ok(())
    }

    /// Who git thinks is working here, when it has been told.
    ///
    /// An empty `user.name` is nobody, not somebody called "".
    pub fn user_name(&self) -> Result<Option<String>, Error> {
        Ok(self
            .config_get("user.name")?
            .filter(|name| !name.trim().is_empty()))
    }
}

/// git refusing to find a toplevel means there is no repository here; any
/// other failure is about the machine, not about the directory.
fn not_a_repository(err: Error, dir: &Path) -> Error {
    if matches!(err, Error::Git { .. }) {
        return Error::NotARepository(dir.to_path_buf());
    }

    err
}

/// Split `for-each-ref` output into its two columns, dropping anything that
/// does not have both.
fn pairs(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(left, right)| (left.to_owned(), right.to_owned()))
        .collect()
}
