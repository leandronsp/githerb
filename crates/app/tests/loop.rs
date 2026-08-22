//! The use cases against real repositories.
//!
//! A fake git proves nothing about git, so every test here builds a real
//! repository in a temp directory with the real binary, runs the verbs a
//! person would run, and throws the repository away afterwards.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use app::{Config, Error, Identity, Reader, Result, Store, format};
use gitstore::Repo;
use review::{Anchor, Author, FilePath, ProposalId, Side, Span, State, Timestamp};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A repository built for one test, removed when the test ends.
struct TempRepo {
    home: PathBuf,
    root: PathBuf,
}

impl TempRepo {
    fn new() -> TempRepo {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let home = std::env::temp_dir().join(format!("githerb-app-{}-{id}", std::process::id()));
        fs::create_dir_all(&home).unwrap();

        // macOS hands out a symlinked temp dir and git answers with the real
        // path, so the comparison only works from the resolved one.
        let home = home.canonicalize().unwrap();
        let root = home.join("repo");
        fs::create_dir_all(&root).unwrap();

        let repo = TempRepo { home, root };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.name", "test"]);
        repo.git(&["config", "user.email", "test@githerb"]);
        repo.git(&["commit", "-q", "--allow-empty", "-m", "root"]);

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

    /// A commit on a side branch, leaving the trunk where it was, which is the
    /// shape every proposal starts from.
    fn work(&self, branch: &str, message: &str) -> String {
        self.git(&["checkout", "-q", "-B", branch]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn head_of(&self, branch: &str) -> String {
        self.git(&["rev-parse", &format!("refs/heads/{branch}")])
    }

    fn store(&self) -> Store {
        Store::new(Repo::open(&self.root).unwrap())
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.home);
    }
}

fn author() -> Author {
    Author::parse("leandro").unwrap()
}

fn agent() -> Author {
    Author::parse("claude").unwrap()
}

fn now() -> Timestamp {
    Timestamp::from_unix(1_787_000_645)
}

fn anchor(file: &str, start: u32, end: u32) -> Anchor {
    Anchor::new(
        FilePath::parse(file).unwrap(),
        Span::new(Side::New, start, end).unwrap(),
    )
}

/// A repository with one proposal open on a side branch.
fn proposed(title: &str) -> (TempRepo, Store, ProposalId) {
    let temp = TempRepo::new();
    temp.work("gate", "the work");
    let store = temp.store();

    let proposal = app::propose(&store, &author(), now(), title, "main", "HEAD").unwrap();
    let id = proposal.id().clone();

    (temp, store, id)
}

// --- the loop ---

#[test]
fn the_whole_loop_ends_with_main_on_the_head() -> Result<()> {
    let (temp, store, id) = proposed("Land the gate");

    let comment = app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;

    let refused = app::land(&store, &[], &author(), now(), &id).unwrap_err();
    assert!(
        matches!(
            refused,
            Error::Review(review::Error::OpenComments { count: 1, .. })
        ),
        "{refused}"
    );

    // The agent reads the annotation, fixes it, and proposes again.
    temp.work("gate", "the fix");
    let revised = app::revise(&store, &id, "HEAD")?;
    assert_eq!(revised.head().number(), 2);

    app::resolve(&store, &agent(), now(), &id, comment.id())?;

    let landing = app::land(&store, &[], &author(), now(), &id)?;

    assert_eq!(landing.proposal().state(), State::Landed);
    assert_eq!(
        temp.head_of("main"),
        landing.proposal().head().sha().as_str()
    );
    assert_eq!(store.load(&id)?.state(), State::Landed);
    Ok(())
}

#[test]
fn a_comment_on_an_older_revision_does_not_block_the_head() -> Result<()> {
    let (temp, store, id) = proposed("Land the gate");

    app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;

    temp.work("gate", "the fix");
    app::revise(&store, &id, "HEAD")?;

    let landing = app::land(&store, &[], &author(), now(), &id)?;

    assert_eq!(landing.proposal().state(), State::Landed);
    // It fell off the head, so it no longer blocks, and it is still a question
    // nobody answered.
    assert_eq!(store.load(&id)?.conversation().len(), 1);
    Ok(())
}

#[test]
fn landing_onto_a_branch_that_is_not_the_trunk_leaves_main_alone() -> Result<()> {
    let temp = TempRepo::new();
    temp.work("feature", "the groundwork");
    temp.git(&["checkout", "-q", "-b", "feature-part-two"]);
    temp.git(&["commit", "-q", "--allow-empty", "-m", "the next piece"]);

    let store = temp.store();
    let proposal = app::propose(
        &store,
        &author(),
        now(),
        "The next piece",
        "feature",
        "HEAD",
    )?;

    assert_eq!(proposal.target().as_str(), "feature");

    let landing = app::land(&store, &[], &author(), now(), proposal.id())?;
    let head = landing.proposal().head().sha().as_str();

    assert_eq!(temp.head_of("feature"), head);
    assert_ne!(temp.head_of("main"), head);
    Ok(())
}

#[test]
fn a_proposal_is_named_after_its_title() {
    let (_temp, _store, id) = proposed("Land the gate, finally!");

    assert!(
        id.as_str().starts_with("land-the-gate-finally-"),
        "{}",
        id.as_str()
    );
}

#[test]
fn landing_is_refused_when_the_target_moved_on() -> Result<()> {
    let (temp, store, id) = proposed("Land the gate");

    temp.git(&["checkout", "-q", "main"]);
    temp.git(&["commit", "-q", "--allow-empty", "-m", "somebody else"]);

    let refused = app::land(&store, &[], &author(), now(), &id).unwrap_err();

    assert!(matches!(refused, Error::NotFastForward(_)), "{refused}");
    assert_eq!(store.load(&id)?.state(), State::Open);
    Ok(())
}

// --- stacking ---

#[test]
fn landing_a_proposal_retargets_what_was_stacked_on_it() -> Result<()> {
    let temp = TempRepo::new();
    let store = temp.store();

    temp.work("one", "the first work");
    let first = app::propose(&store, &author(), now(), "The first piece", "main", "HEAD")?;

    temp.work("two", "the second work");
    let second = app::propose(&store, &author(), now(), "The second piece", "one", "HEAD")?;
    assert_eq!(second.target().as_str(), "one");

    let landing = app::land(&store, &[], &author(), now(), first.id())?;

    assert_eq!(landing.followed(), &[second.id().clone()]);

    let moved = store.load(second.id())?;
    assert_eq!(moved.target().as_str(), "main");
    // Landing is a fast-forward, so nothing underneath it moved and the base
    // is still true.
    assert_eq!(moved.base(), second.base());
    Ok(())
}

// --- describe ---

#[test]
fn a_description_is_written_and_a_long_decision_is_refused() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");

    let written = app::describe(
        &store,
        &author(),
        now(),
        &id,
        r#"{
            "chunks": [{
                "title": "the gate",
                "surface": "the terminal",
                "before": "anything landed",
                "after": "the checks answer first",
                "decision": "refuse a land the checks did not answer",
                "file": "src/land.rs",
                "start": 12,
                "end": 18
            }],
            "rationale": [{"file": "src/land.rs", "start": 12, "body": "why it reads the log twice"}]
        }"#,
    )?;

    assert_eq!(written, 2);

    let proposal = store.load(&id)?;
    assert_eq!(proposal.chunks().len(), 1);
    assert_eq!(proposal.chunks()[0].title(), "the gate");
    assert_eq!(proposal.rationale().len(), 1);

    let long = format!(
        r#"{{"chunks":[{{"title":"t","before":"b","after":"a","decision":"{}"}}]}}"#,
        "x".repeat(201)
    );
    let refused = app::describe(&store, &author(), now(), &id, &long).unwrap_err();

    assert!(
        matches!(
            refused,
            Error::Review(review::Error::TooLong {
                field: review::Field::Decision,
                ..
            })
        ),
        "{refused}"
    );
    assert_eq!(store.load(&id)?.chunks().len(), 1);
    Ok(())
}

// --- checks ---

#[test]
fn a_check_that_passes_and_one_that_fails_are_both_recorded() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let config = Config::parse("[checks]\nno = \"false\"\nyes = \"true\"\n")?;
    let mut out = Vec::new();

    let results = app::check(&store, &config, &author(), now(), &id, &mut out)?;

    assert_eq!(results.len(), 2);
    assert_eq!(app::refused(&results), 1);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        format!(
            "{}\n{}\n",
            format::check_line(&results[0]),
            format::check_line(&results[1])
        )
    );

    let proposal = store.load(&id)?;
    assert_eq!(proposal.checks().len(), 2);
    assert_eq!(proposal.check_summary().to_string(), "1 failed");

    // A revision that already answered is not asked twice.
    let again = app::check(&store, &config, &author(), now(), &id, &mut Vec::new())?;
    assert_eq!(again.len(), 2);
    assert_eq!(store.load(&id)?.checks().len(), 2);
    Ok(())
}

#[test]
fn a_check_nobody_declared_is_no_checks_at_all() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let mut out = Vec::new();

    let results = app::check(&store, &Config::default(), &author(), now(), &id, &mut out)?;

    assert!(results.is_empty());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "no checks declared in .githerb.toml\n"
    );
    Ok(())
}

#[test]
fn a_check_that_was_killed_records_nothing() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let config = Config::parse("[checks]\nboom = \"kill -9 $$\"\n")?;

    let refused =
        app::check(&store, &config, &author(), now(), &id, &mut std::io::sink()).unwrap_err();

    assert!(matches!(refused, Error::CheckKilled(_)), "{refused}");
    assert!(store.load(&id)?.checks().is_empty());
    Ok(())
}

#[test]
fn a_required_check_that_never_ran_blocks_the_gate() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let required = vec![review::CheckName::parse("gate")?];

    let refused = app::land(&store, &required, &author(), now(), &id).unwrap_err();

    assert!(
        matches!(refused, Error::Review(review::Error::CheckMissing(_))),
        "{refused}"
    );
    Ok(())
}

// --- the store ---

#[test]
fn a_line_from_the_future_is_skipped_and_a_version_is_refused() -> Result<()> {
    let (temp, store, id) = proposed("Land the gate");
    let head = store.load(&id)?.head().sha().as_str().to_owned();

    temp.git(&[
        "notes",
        "--ref=githerb/annotations",
        "append",
        "--no-separator",
        "-m",
        r#"{"v":1,"kind":"telepathy","id":"","author":"x","at":"2026-01-01T00:00:00Z"}"#,
        &head,
    ]);

    assert_eq!(store.load(&id)?.id(), &id);

    temp.git(&[
        "notes",
        "--ref=githerb/annotations",
        "append",
        "--no-separator",
        "-m",
        r#"{"v":2,"kind":"comment","id":"","author":"x","at":"2026-01-01T00:00:00Z"}"#,
        &head,
    ]);

    let refused = store.load(&id).unwrap_err();

    assert!(matches!(refused, Error::Log { .. }), "{refused}");
    Ok(())
}

#[test]
fn the_same_annotation_three_times_counts_once() -> Result<()> {
    let (temp, store, id) = proposed("Land the gate");
    let comment = app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;
    let head = store.load(&id)?.head().sha().as_str().to_owned();
    let line = review::Record::Comment(comment).to_line();

    for _ in 0..2 {
        temp.git(&[
            "notes",
            "--ref=githerb/annotations",
            "append",
            "--no-separator",
            "-m",
            &line,
            &head,
        ]);
    }

    assert_eq!(store.load(&id)?.open_comments().len(), 1);
    Ok(())
}

#[test]
fn opening_a_proposal_tells_git_how_the_notes_merge() {
    let (temp, _store, _id) = proposed("Land the gate");

    assert_eq!(
        temp.git(&["config", "notes.githerb/annotations.mergeStrategy"]),
        "cat_sort_uniq"
    );
    assert_eq!(
        temp.git(&["config", "notes.githerb/proposals.mergeStrategy"]),
        "cat_sort_uniq"
    );
}

#[test]
fn a_snapshot_is_read_again_only_when_something_moved() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let snapshot = store.snapshot()?;

    assert!(store.snapshot_if_changed(snapshot.fingerprint())?.is_none());

    app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;

    let moved = store.snapshot_if_changed(snapshot.fingerprint())?;

    assert_eq!(moved.map(|read| read.open().count()), Some(1));
    Ok(())
}

#[test]
fn a_proposal_nobody_opened_is_not_found() {
    let temp = TempRepo::new();
    let id = ProposalId::parse("nothing-0000000").unwrap();

    let refused = temp.store().load(&id).unwrap_err();

    assert!(matches!(refused, Error::NotFound(_)), "{refused}");
}

// --- what the terminal reads ---

#[test]
fn a_listing_and_a_proposal_read_as_they_are_printed() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let comment = app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;
    let proposal = store.load(&id)?;

    assert_eq!(
        format::list(std::slice::from_ref(&proposal)),
        format!(
            "{:<44} {:<9} r1   1 open  {:<8} onto main\n",
            id.as_str(),
            "open",
            "no checks"
        )
    );
    assert!(
        format::show(&proposal).ends_with(&format!(
            "no agent on it\n\n{}  cmd/main.rs:3:5\n  this leaks\n",
            comment.id()
        )),
        "{}",
        format::show(&proposal)
    );
    assert_eq!(
        format::comments(&proposal, format::Scope::Open, format::Shape::Text),
        format!("{}  cmd/main.rs:3  this leaks\n", comment.id())
    );
    Ok(())
}

#[test]
fn a_handover_carries_the_notes_and_says_nothing_when_none_are_open() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");

    assert_eq!(app::handover(&store, &id, Reader::Person)?, "");

    app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;

    let brief = app::handover(&store, &id, Reader::Person)?;
    assert!(brief.contains("this leaks"), "{brief}");
    assert!(brief.contains(&format!("githerb revise {id}")), "{brief}");

    let agent = app::handover(&store, &id, Reader::Agent)?;
    assert!(!agent.contains("githerb revise"), "{agent}");
    Ok(())
}

#[test]
fn an_agent_says_what_it_is_doing_and_the_proposal_reads_it_back() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");

    app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;
    let asked = app::dispatch(&store, &author(), now(), &id)?;
    assert!(asked.dispatched());

    app::report(&store, &agent(), now(), &id, "apply", "started", None)?;

    let working = store.load(&id)?;
    assert!(!working.dispatched());
    assert!(working.agent_line().starts_with("claude is apply since"));

    app::report(
        &store,
        &agent(),
        now(),
        &id,
        "apply",
        "failed",
        Some("the tests would not build"),
    )?;

    assert_eq!(
        store.load(&id)?.agent_line(),
        "apply failed: the tests would not build"
    );
    Ok(())
}

#[test]
fn an_answer_lands_in_the_thread_and_a_note_it_does_not_know_is_refused() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");
    let comment = app::annotate(
        &store,
        &author(),
        now(),
        &id,
        anchor("cmd/main.rs", 3, 5),
        "this leaks",
    )?;

    let answer = app::reply(&store, &agent(), now(), &id, comment.id(), "it does not")?;

    let proposal = store.load(&id)?;
    assert_eq!(proposal.answers(comment.id()), vec![&answer]);
    // A reply says something and blocks nothing.
    assert_eq!(proposal.open_comments().len(), 1);

    let stranger = review::RecordId::parse("000000000000")?;
    let refused = app::reply(&store, &agent(), now(), &id, &stranger, "hello").unwrap_err();

    assert!(
        matches!(refused, Error::Review(review::Error::UnknownComment(_))),
        "{refused}"
    );
    Ok(())
}

#[test]
fn a_proposal_can_be_given_up_on() -> Result<()> {
    let (_temp, store, id) = proposed("Land the gate");

    app::abandon(&store, &author(), now(), &id)?;

    assert_eq!(store.load(&id)?.state(), State::Abandoned);

    let refused = app::abandon(&store, &author(), now(), &id).unwrap_err();
    assert!(
        matches!(refused, Error::Review(review::Error::NotOpen(_))),
        "{refused}"
    );
    Ok(())
}

#[test]
fn a_diff_reads_from_the_base_or_from_a_revision() -> Result<()> {
    let (temp, store, id) = proposed("Land the gate");

    fs::write(temp.root.join("a.txt"), "one\n").unwrap();
    temp.git(&["add", "a.txt"]);
    temp.git(&["commit", "-q", "-m", "the fix"]);
    app::revise(&store, &id, "HEAD")?;

    let whole = app::diff(&store, &id, None)?;
    assert!(whole.contains("+one"), "{whole}");

    let since = app::diff(&store, &id, Some(2))?;
    assert_eq!(since, "");

    let refused = app::diff(&store, &id, Some(9)).unwrap_err();
    assert!(matches!(refused, Error::NoSuchRevision(9)), "{refused}");
    Ok(())
}

#[test]
fn the_author_is_whoever_the_repository_says_when_nothing_else_does() {
    let temp = TempRepo::new();

    // The environment wins when it is set, which the CLI tests cover; with it
    // unset this is what git was told.
    if std::env::var("GITHERB_AUTHOR").is_err() {
        assert_eq!(
            Identity::detect(&Repo::open(&temp.root).unwrap()).as_str(),
            "test"
        );
    }
    assert_eq!(Identity::runner().as_str(), "githerb-run");
}

// --- landing onto the branch checked out here ---

/// `githerb review` runs in the checkout, and landing from the browser moves
/// the very branch that checkout is on. The branch, the index and the working
/// tree have to move together, or the next commit from here quietly reverts
/// what just landed.
#[test]
fn landing_onto_the_branch_checked_out_here_moves_the_working_tree_with_it() -> Result<()> {
    let temp = TempRepo::new();
    temp.git(&["checkout", "-q", "-b", "work"]);
    fs::write(temp.root.join("a.txt"), "landed\n").unwrap();
    temp.git(&["add", "a.txt"]);
    temp.git(&["commit", "-q", "-m", "the work"]);
    let store = temp.store();
    let proposal = app::propose(&store, &author(), now(), "The work", "main", "HEAD")?;
    temp.git(&["checkout", "-q", "main"]);
    assert!(!temp.root.join("a.txt").exists());

    let landing = app::land(&store, &[], &author(), now(), proposal.id())?;

    let head = landing.proposal().head().sha().as_str().to_owned();
    assert_eq!(temp.head_of("main"), head);
    assert_eq!(temp.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(
        temp.git(&["status", "--porcelain"]),
        "",
        "the checkout must be clean after landing"
    );
    assert_eq!(
        fs::read_to_string(temp.root.join("a.txt")).unwrap(),
        "landed\n"
    );
    Ok(())
}

#[test]
fn landing_onto_the_checked_out_branch_is_refused_when_the_tree_is_in_the_way() -> Result<()> {
    let temp = TempRepo::new();
    temp.git(&["checkout", "-q", "-b", "work"]);
    fs::write(temp.root.join("a.txt"), "landed\n").unwrap();
    temp.git(&["add", "a.txt"]);
    temp.git(&["commit", "-q", "-m", "the work"]);
    let store = temp.store();
    let proposal = app::propose(&store, &author(), now(), "The work", "main", "HEAD")?;
    temp.git(&["checkout", "-q", "main"]);
    // Somebody is halfway through something here: an untracked a.txt the land
    // would have to overwrite.
    fs::write(temp.root.join("a.txt"), "mine, unsaved\n").unwrap();
    let before = temp.head_of("main");

    let refused = app::land(&store, &[], &author(), now(), proposal.id()).unwrap_err();

    assert!(
        matches!(refused, Error::WorkingTreeInTheWay { .. }),
        "{refused}"
    );
    assert!(
        refused.to_string().contains("main is checked out here"),
        "{refused}"
    );
    assert_eq!(temp.head_of("main"), before, "main must not move");
    assert_eq!(
        fs::read_to_string(temp.root.join("a.txt")).unwrap(),
        "mine, unsaved\n"
    );
    assert_eq!(store.load(proposal.id())?.state(), State::Open);
    Ok(())
}
