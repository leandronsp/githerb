//! `gitstore` against a real repository.
//!
//! A fake git proves nothing about git, so every test here builds a real one
//! in a temp directory with the real binary and throws it away afterwards.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use gitstore::{Error, RebaseOutcome, Repo};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A repository built for one test, with a scratch area beside it for
/// worktrees, removed when the test ends.
struct TempRepo {
    home: PathBuf,
    root: PathBuf,
}

impl TempRepo {
    fn new() -> TempRepo {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let home =
            std::env::temp_dir().join(format!("githerb-gitstore-{}-{id}", std::process::id()));
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

    /// A loose blob with this text, so `cat_blobs` has something to read.
    fn blob(&self, text: &str) -> String {
        let name = format!("blob-{}", COUNTER.fetch_add(1, Ordering::Relaxed));
        self.write(&name, text);
        let sha = self.git(&["hash-object", "-w", &name]);
        fs::remove_file(self.root.join(name)).unwrap();

        sha
    }

    /// A path beside the repository, for a worktree.
    fn scratch(&self, name: &str) -> PathBuf {
        self.home.join(name)
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

/// A repository with one commit on main.
fn with_a_commit() -> (TempRepo, String) {
    let temp = TempRepo::new();
    temp.write("README.md", "one\n");
    let sha = temp.commit("first");

    (temp, sha)
}

// --- open ---

#[test]
fn open_finds_the_root_from_a_subdirectory() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let nested = temp.root.join("deep/nested");
    fs::create_dir_all(&nested).unwrap();

    let repo = Repo::open(&nested)?;

    assert_eq!(repo.root(), temp.root.as_path());
    Ok(())
}

#[test]
fn open_finds_the_git_directory_every_worktree_shares() -> Result<(), Error> {
    let (temp, _) = with_a_commit();

    let repo = Repo::open(&temp.root)?;

    assert_eq!(repo.git_dir(), temp.root.join(".git").as_path());
    Ok(())
}

#[test]
fn open_refuses_a_directory_that_is_not_a_repository() {
    let temp = TempRepo::new();
    let outside = temp.scratch("plain");
    fs::create_dir_all(&outside).unwrap();

    let err = Repo::open(&outside).unwrap_err();

    let reported = err.to_string();
    let Error::NotARepository(path) = err else {
        panic!("expected NotARepository, got {reported}");
    };
    assert_eq!(path, outside);
}

#[test]
fn a_refusal_carries_the_argv_and_what_git_said() {
    let (temp, _) = with_a_commit();

    let err = temp
        .open()
        .run(&["rev-parse", "--verify", "refs/heads/nope"])
        .unwrap_err();

    let message = err.to_string();
    assert!(
        message.starts_with("git rev-parse --verify refs/heads/nope: fatal:"),
        "{message}"
    );
}

// --- revisions ---

#[test]
fn resolve_turns_a_revision_into_a_commit() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();

    assert_eq!(repo.resolve("HEAD")?, sha);
    assert_eq!(repo.resolve("main")?, sha);
    Ok(())
}

#[test]
fn head_of_reads_a_branch_by_its_full_ref() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();

    assert_eq!(temp.open().head_of("refs/heads/main")?, sha);
    Ok(())
}

#[test]
fn merge_base_is_the_commit_two_branches_share() -> Result<(), Error> {
    let (temp, base) = with_a_commit();
    temp.git(&["checkout", "-q", "-b", "side"]);
    temp.write("side.txt", "side\n");
    let side = temp.commit("side");
    temp.git(&["checkout", "-q", "main"]);
    temp.write("main.txt", "main\n");
    let main = temp.commit("main");

    assert_eq!(temp.open().merge_base(&side, &main)?, base);
    Ok(())
}

#[test]
fn is_ancestor_is_true_up_the_chain_and_false_across_it() -> Result<(), Error> {
    let (temp, base) = with_a_commit();
    temp.write("more.txt", "more\n");
    let later = temp.commit("later");
    temp.git(&["checkout", "-q", "-b", "side", &base]);
    temp.write("side.txt", "side\n");
    let side = temp.commit("side");
    let repo = temp.open();

    assert!(repo.is_ancestor(&base, &later)?);
    assert!(!repo.is_ancestor(&later, &side)?);
    Ok(())
}

#[test]
fn diff_is_a_unified_patch() -> Result<(), Error> {
    let (temp, first) = with_a_commit();
    temp.write("README.md", "two\n");
    let second = temp.commit("second");

    let diff = temp.open().diff(&first, &second)?;

    assert_eq!(
        diff.lines().next(),
        Some("diff --git a/README.md b/README.md")
    );
    Ok(())
}

#[test]
fn subject_is_the_first_line_of_the_message() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();

    assert_eq!(temp.open().subject(&sha)?, "first");
    Ok(())
}

// --- refs ---

#[test]
fn update_ref_creates_a_ref_and_refs_lists_the_namespace() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();

    repo.update_ref("refs/githerb/proposals/one/1", &sha, None)?;
    repo.update_ref("refs/githerb/proposals/two/1", &sha, None)?;

    assert_eq!(
        repo.refs("refs/githerb/proposals")?,
        vec![
            ("refs/githerb/proposals/one/1".to_owned(), sha.clone()),
            ("refs/githerb/proposals/two/1".to_owned(), sha),
        ]
    );
    Ok(())
}

#[test]
fn update_ref_refuses_when_the_old_value_is_not_what_it_was() -> Result<(), Error> {
    let (temp, first) = with_a_commit();
    temp.write("README.md", "two\n");
    let second = temp.commit("second");
    let repo = temp.open();
    repo.update_ref("refs/heads/target", &first, None)?;

    let err = repo
        .update_ref("refs/heads/target", &second, Some(&second))
        .unwrap_err();

    assert!(matches!(err, Error::Git { .. }), "{err}");
    assert_eq!(repo.head_of("refs/heads/target")?, first);
    Ok(())
}

#[test]
fn update_ref_moves_the_ref_when_the_old_value_matches() -> Result<(), Error> {
    let (temp, first) = with_a_commit();
    temp.write("README.md", "two\n");
    let second = temp.commit("second");
    let repo = temp.open();
    repo.update_ref("refs/heads/target", &first, None)?;

    repo.update_ref("refs/heads/target", &second, Some(&first))?;

    assert_eq!(repo.head_of("refs/heads/target")?, second);
    Ok(())
}

// --- notes ---

#[test]
fn a_notes_ref_nobody_wrote_to_is_an_empty_log() -> Result<(), Error> {
    let (temp, _) = with_a_commit();

    assert_eq!(
        temp.open().note_list("githerb/annotations")?,
        Vec::<(String, String)>::new()
    );
    assert_eq!(temp.open().notes("githerb/annotations")?.len(), 0);
    Ok(())
}

#[test]
fn a_broken_notes_ref_is_a_failure_not_an_empty_log() {
    let (temp, _) = with_a_commit();
    // A notes ref pointing at a blob exists but cannot be read as notes,
    // which is the case an "absent means empty" rule must not swallow.
    let blob = temp.blob("not a notes tree\n");
    temp.git(&["update-ref", "refs/notes/githerb/annotations", &blob]);

    let err = temp.open().note_list("githerb/annotations").unwrap_err();

    assert!(matches!(err, Error::Git { .. }), "{err}");
}

#[test]
fn note_append_then_notes_reads_the_text_back() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();

    repo.note_append("githerb/annotations", &sha, r#"{"kind":"comment"}"#)?;

    assert_eq!(
        repo.notes("githerb/annotations")?
            .get(&sha)
            .map(String::as_str),
        Some("{\"kind\":\"comment\"}\n")
    );
    Ok(())
}

#[test]
fn two_appends_land_on_two_lines() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();

    repo.note_append("githerb/annotations", &sha, "first")?;
    repo.note_append("githerb/annotations", &sha, "second")?;

    let notes = repo.notes("githerb/annotations")?;
    let text = notes.get(&sha).cloned().unwrap_or_default();
    assert_eq!(text.lines().collect::<Vec<&str>>(), vec!["first", "second"]);
    Ok(())
}

#[test]
fn notes_reads_every_annotated_object() -> Result<(), Error> {
    let (temp, first) = with_a_commit();
    temp.write("README.md", "two\n");
    let second = temp.commit("second");
    let repo = temp.open();

    repo.note_append("githerb/annotations", &first, "on one")?;
    repo.note_append("githerb/annotations", &second, "on two")?;

    let notes = repo.notes("githerb/annotations")?;
    assert_eq!(notes.len(), 2);
    assert_eq!(notes.get(&first).map(String::as_str), Some("on one\n"));
    assert_eq!(notes.get(&second).map(String::as_str), Some("on two\n"));
    Ok(())
}

#[test]
fn two_objects_with_the_same_note_both_keep_it() -> Result<(), Error> {
    let (temp, first) = with_a_commit();
    temp.write("README.md", "two\n");
    let second = temp.commit("second");
    let repo = temp.open();

    repo.note_append("githerb/annotations", &first, "same")?;
    repo.note_append("githerb/annotations", &second, "same")?;

    let notes = repo.notes("githerb/annotations")?;
    assert_eq!(notes.get(&first).map(String::as_str), Some("same\n"));
    assert_eq!(notes.get(&second).map(String::as_str), Some("same\n"));
    Ok(())
}

// --- objects ---

#[test]
fn cat_blobs_reads_every_object_in_one_call() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let one = temp.blob("alpha\n");
    let two = temp.blob("beta\n");
    let three = temp.blob("gamma\n");

    let blobs = temp
        .open()
        .cat_blobs(&[one.as_str(), two.as_str(), three.as_str()])?;

    assert_eq!(blobs.len(), 3);
    assert_eq!(blobs.get(&one).map(String::as_str), Some("alpha\n"));
    assert_eq!(blobs.get(&two).map(String::as_str), Some("beta\n"));
    assert_eq!(blobs.get(&three).map(String::as_str), Some("gamma\n"));
    Ok(())
}

#[test]
fn cat_blobs_skips_an_object_that_is_not_there() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let present = temp.blob("here\n");
    let absent = "0000000000000000000000000000000000000000";

    let blobs = temp.open().cat_blobs(&[absent, present.as_str()])?;

    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs.get(&present).map(String::as_str), Some("here\n"));
    Ok(())
}

#[test]
fn cat_blobs_of_nothing_runs_no_git_at_all() -> Result<(), Error> {
    let (temp, _) = with_a_commit();

    assert_eq!(temp.open().cat_blobs(&[])?.len(), 0);
    Ok(())
}

// --- fingerprint ---

#[test]
fn fingerprint_is_stable_while_nothing_happens() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let repo = temp.open();

    assert_eq!(repo.fingerprint()?, repo.fingerprint()?);
    Ok(())
}

#[test]
fn fingerprint_changes_when_a_ref_moves() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();
    let before = repo.fingerprint()?;

    repo.update_ref("refs/githerb/proposals/one/1", &sha, None)?;

    assert_ne!(repo.fingerprint()?, before);
    Ok(())
}

#[test]
fn fingerprint_changes_when_a_note_is_written() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();
    let before = repo.fingerprint()?;

    repo.note_append("githerb/annotations", &sha, "something")?;

    assert_ne!(repo.fingerprint()?, before);
    Ok(())
}

// --- config ---

#[test]
fn config_round_trips_a_value_and_answers_none_for_one_that_is_unset() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let repo = temp.open();

    repo.config_set("githerb.agent", "claude")?;

    assert_eq!(repo.config_get("githerb.agent")?, Some("claude".to_owned()));
    assert_eq!(repo.config_get("githerb.missing")?, None);
    Ok(())
}

#[test]
fn user_name_reads_the_identity_git_was_given() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let repo = temp.open();

    repo.config_set("user.name", "Reviewer")?;

    assert_eq!(repo.user_name()?, Some("Reviewer".to_owned()));
    Ok(())
}

#[test]
fn an_empty_user_name_is_nobody() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    let repo = temp.open();

    repo.config_set("user.name", "")?;

    assert_eq!(repo.user_name()?, None);
    Ok(())
}

// --- worktrees ---

#[test]
fn worktree_add_checks_the_commit_out_and_remove_deletes_it() -> Result<(), Error> {
    let (temp, first) = with_a_commit();
    temp.write("README.md", "two\n");
    temp.commit("second");
    let repo = temp.open();
    let dir = temp.scratch("work");

    repo.worktree_add(&dir, &first)?;

    assert_eq!(repo.head(&dir)?, first);
    assert_eq!(fs::read_to_string(dir.join("README.md")).unwrap(), "one\n");

    repo.worktree_remove(&dir)?;

    assert!(!dir.exists());
    Ok(())
}

#[test]
fn worktree_prune_forgets_a_checkout_whose_directory_is_gone() -> Result<(), Error> {
    let (temp, sha) = with_a_commit();
    let repo = temp.open();
    let dir = temp.scratch("abandoned");
    repo.worktree_add(&dir, &sha)?;
    fs::remove_dir_all(&dir).unwrap();

    repo.worktree_prune()?;

    assert_eq!(temp.git(&["worktree", "list"]).lines().count(), 1);
    Ok(())
}

// --- rebase ---

#[test]
fn rebase_onto_replays_a_clean_history() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    temp.write("f.txt", "base\n");
    let base = temp.commit("base");
    temp.git(&["checkout", "-q", "-b", "side"]);
    temp.write("side.txt", "side\n");
    let side = temp.commit("side");
    temp.git(&["checkout", "-q", "main"]);
    temp.write("main.txt", "main\n");
    let main = temp.commit("main");
    let repo = temp.open();
    let dir = temp.scratch("rebase");
    repo.worktree_add(&dir, &side)?;

    let outcome = repo.rebase_onto(&dir, &main, &base)?;

    assert_eq!(outcome, RebaseOutcome::Done);
    assert!(!repo.rebasing(&dir));
    assert_ne!(repo.head(&dir)?, side);
    assert!(repo.is_ancestor(&main, &repo.head(&dir)?)?);
    Ok(())
}

#[test]
fn rebase_onto_stops_on_a_conflict_and_abort_puts_it_back() -> Result<(), Error> {
    let (temp, _) = with_a_commit();
    temp.write("f.txt", "base\n");
    let base = temp.commit("base");
    temp.git(&["checkout", "-q", "-b", "side"]);
    temp.write("f.txt", "side\n");
    let side = temp.commit("side");
    temp.git(&["checkout", "-q", "main"]);
    temp.write("f.txt", "main\n");
    let main = temp.commit("main");
    let repo = temp.open();
    let dir = temp.scratch("rebase");
    repo.worktree_add(&dir, &side)?;

    let outcome = repo.rebase_onto(&dir, &main, &base)?;

    assert_eq!(outcome, RebaseOutcome::Conflicted);
    assert!(repo.rebasing(&dir));

    repo.rebase_abort(&dir)?;

    assert!(!repo.rebasing(&dir));
    assert_eq!(repo.head(&dir)?, side);
    Ok(())
}

// --- the checked-out branch ---

#[test]
fn the_current_branch_is_what_head_points_at_and_none_when_detached() -> Result<(), Error> {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    let sha = temp.commit("root");
    let repo = Repo::open(&temp.root)?;
    assert_eq!(repo.current_branch()?.as_deref(), Some("refs/heads/main"));
    temp.git(&["checkout", "-q", "--detach", &sha]);
    assert_eq!(repo.current_branch()?, None);
    Ok(())
}

#[test]
fn a_fast_forward_moves_the_branch_the_index_and_the_working_tree() -> Result<(), Error> {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.commit("root");
    let repo = Repo::open(&temp.root)?;
    temp.git(&["checkout", "-q", "-b", "work"]);
    temp.write("f.txt", "new\n");
    let sha = temp.commit("work");
    temp.git(&["checkout", "-q", "main"]);

    repo.fast_forward(&sha)?;

    assert_eq!(repo.head_of("refs/heads/main")?, sha);
    assert_eq!(repo.run(&["status", "--porcelain"])?, "");
    assert_eq!(
        std::fs::read_to_string(temp.root.join("f.txt")).unwrap(),
        "new\n"
    );
    Ok(())
}
