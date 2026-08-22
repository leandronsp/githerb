//! The worktree lifecycle, against a real repository.
//!
//! A fake git proves nothing about git, so every test here builds one in a
//! temp directory with the real binary and throws it away afterwards.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use gitstore::{RebaseOutcome, Repo};
use runner::{Error, Worktree, prune_leftovers};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A repository built for one test, removed when the test ends.
struct TempRepo {
    home: PathBuf,
    root: PathBuf,
}

impl TempRepo {
    fn new() -> TempRepo {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let home = std::env::temp_dir().join(format!("githerb-runner-{}-{id}", std::process::id()));
        fs::create_dir_all(&home).unwrap();

        // macOS hands out a symlinked temp dir and git answers with the real
        // path, so the comparison only works from the resolved one.
        let home = home.canonicalize().unwrap();
        let root = home.join("repo");
        fs::create_dir_all(&root).unwrap();

        let repo = TempRepo { home, root };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.name", "t"]);
        repo.git(&["config", "user.email", "t@x"]);

        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout)
            .unwrap()
            .trim_end()
            .to_owned()
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.root.join(name), text).unwrap();
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn open(&self) -> Repo {
        Repo::open(&self.root).unwrap()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

/// A trunk with one commit, and a branch that changed the same line.
struct Proposed {
    temp: TempRepo,
    base: String,
    head: String,
    tip: String,
}

fn proposed() -> Proposed {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    let base = temp.commit("root");

    temp.git(&["checkout", "-q", "-b", "work"]);
    temp.write("a.txt", "one\nTWO\n");
    let head = temp.commit("the work");

    temp.git(&["checkout", "-q", "main"]);
    temp.write("a.txt", "one\nTRUNK\n");
    let tip = temp.commit("the trunk moved");

    Proposed {
        temp,
        base,
        head,
        tip,
    }
}

// --- opening ---

#[test]
fn a_worktree_is_a_checkout_of_the_revision_it_was_opened_at() -> Result<(), Error> {
    let proposed = proposed();
    let repo = proposed.temp.open();

    let where_it_runs = Worktree::open(&repo, &proposed.head)?;

    assert_eq!(where_it_runs.head()?, proposed.head);
    assert_eq!(
        fs::read_to_string(where_it_runs.path().join("a.txt"))?,
        "one\nTWO\n"
    );
    Ok(())
}

#[test]
fn a_job_never_touches_the_checkout_you_have_open() -> Result<(), Error> {
    let proposed = proposed();
    let repo = proposed.temp.open();

    let where_it_runs = Worktree::open(&repo, &proposed.head)?;
    fs::write(where_it_runs.path().join("a.txt"), "one\nNAMED\n")?;

    assert_eq!(
        fs::read_to_string(proposed.temp.root.join("a.txt"))?,
        "one\nTRUNK\n"
    );
    Ok(())
}

#[test]
fn a_worktree_of_a_revision_the_repository_does_not_have_is_refused() {
    let proposed = proposed();
    let repo = proposed.temp.open();

    let refused = Worktree::open(&repo, "0000000000000000000000000000000000000000");

    assert!(matches!(refused, Err(Error::Git(_))), "{refused:?}");
}

// --- removing ---

#[test]
fn a_worktree_is_gone_from_the_disk_and_from_git_when_the_job_is_over() -> Result<(), Error> {
    let proposed = proposed();
    let repo = proposed.temp.open();
    let where_it_runs = Worktree::open(&repo, &proposed.head)?;
    let path = where_it_runs.path().to_path_buf();

    assert!(
        proposed
            .temp
            .git(&["worktree", "list"])
            .contains(path.to_str().unwrap())
    );

    drop(where_it_runs);

    assert!(!path.exists());
    assert!(
        !proposed
            .temp
            .git(&["worktree", "list"])
            .contains(path.to_str().unwrap())
    );
    Ok(())
}

#[test]
fn a_leftover_from_a_run_that_is_over_is_pruned_and_this_run_is_spared() -> Result<(), Error> {
    let proposed = proposed();
    let repo = proposed.temp.open();
    let ours = Worktree::open(&repo, &proposed.head)?;

    // A directory shaped like one an earlier process left behind. The pid is
    // not this one, which is the whole of what makes it somebody else's.
    let leftover = std::env::temp_dir().join("githerb-work-0-999");
    fs::create_dir_all(&leftover)?;
    fs::write(leftover.join("a.txt"), "one\n")?;

    prune_leftovers(&repo)?;

    assert!(!leftover.exists());
    assert!(ours.path().exists());
    assert_eq!(ours.head()?, proposed.head);
    Ok(())
}

// --- a rebase left halfway ---

#[test]
fn a_worktree_the_rebase_stopped_in_reads_as_rebasing() -> Result<(), Error> {
    let proposed = proposed();
    let repo = proposed.temp.open();
    let where_it_runs = Worktree::open(&repo, &proposed.head)?;

    let outcome = repo.rebase_onto(where_it_runs.path(), &proposed.tip, &proposed.base)?;

    assert_eq!(outcome, RebaseOutcome::Conflicted);
    assert!(where_it_runs.rebasing());
    Ok(())
}

#[test]
fn a_worktree_nobody_rebased_is_not_rebasing() -> Result<(), Error> {
    let proposed = proposed();
    let repo = proposed.temp.open();

    let where_it_runs = Worktree::open(&repo, &proposed.head)?;

    assert!(!where_it_runs.rebasing());
    Ok(())
}
