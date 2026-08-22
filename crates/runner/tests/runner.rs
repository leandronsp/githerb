//! The loop, against real repositories and real agents.
//!
//! Every agent here is a shell command that does what an agent does: read the
//! brief on stdin, write to `$GITHERB_ANSWERS`, commit in the worktree it was
//! given. A faked one would prove nothing about the worktree, the answers file
//! or the records that come out.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use app::Store;
use review::{Anchor, Author, Comment, FilePath, Proposal, ProposalId, Side, Span, Timestamp};
use runner::{Error, Runner};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A repository with one commit on main, a store over it, and the helpers a
/// test needs to get a proposal into it.
struct Kit {
    home: PathBuf,
    root: PathBuf,
    store: Store,
}

impl Kit {
    fn new() -> Kit {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let home = std::env::temp_dir().join(format!("githerb-loop-{}-{id}", std::process::id()));
        fs::create_dir_all(&home).unwrap();

        // macOS hands out a symlinked temp dir and git answers with the real
        // path, so everything is done from the resolved one.
        let home = home.canonicalize().unwrap();
        let root = home.join("repo");
        fs::create_dir_all(&root).unwrap();

        // git first: a store is a repository, and there is not one yet.
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.name", "test"],
            &["config", "user.email", "test@githerb"],
        ] {
            run(&root, args);
        }

        let kit = Kit {
            home,
            root: root.clone(),
            store: Store::at(&root).unwrap(),
        };

        // Every kit gets its own root commit. Identical content committed in
        // the same second by the same author is the same sha, and two tests
        // sharing a sha share the proposal id that is derived from it.
        kit.write("a.txt", "one\ntwo\n");
        kit.write("kit.txt", &format!("{id}\n"));
        kit.commit("root");

        kit
    }

    fn git(&self, args: &[&str]) -> String {
        run(&self.root, args)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.root.join(name), text).unwrap();
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// A branch with one commit on it, proposed onto main.
    fn proposed(&self, second_line: &str) -> Proposal {
        self.git(&["checkout", "-q", "-b", "work"]);
        self.write("a.txt", &format!("one\n{second_line}\n"));
        self.commit("the work");

        app::propose(&self.store, &person(), now(), "The work", "main", "HEAD").unwrap()
    }

    /// A note on the second line of the head revision.
    fn note(&self, id: &ProposalId, body: &str) -> Comment {
        let anchor = Anchor::new(
            FilePath::parse("a.txt").unwrap(),
            Span::new(Side::New, 2, 2).unwrap(),
        );

        app::annotate(&self.store, &person(), now(), id, anchor, body).unwrap()
    }

    fn hand_over(&self, id: &ProposalId) {
        app::dispatch(&self.store, &person(), now(), id).unwrap();
    }

    fn load(&self, id: &ProposalId) -> Proposal {
        self.store.load(id).unwrap()
    }

    /// A runner over this repository, with the agent declared the way a
    /// repository declares one: in its own file, which the loop re-reads on
    /// every pass rather than trusting what it was handed at the start.
    fn runner(&self, agent: &str) -> Runner {
        self.declares("", agent)
    }

    /// The same, for a repository that also declares a check.
    fn gate(&self, command: &str) -> Runner {
        self.declares(&format!("[checks]\ngate = '''{command}'''\n"), "true")
    }

    fn declares(&self, checks: &str, agent: &str) -> Runner {
        fs::write(
            self.root.join(".githerb.toml"),
            format!("{checks}[agent]\ncommand = '''{agent}'''\n"),
        )
        .unwrap();

        Runner::new(
            self.store.clone(),
            self.root.clone(),
            Author::parse("githerb-run").unwrap(),
            Box::new(|_| {}),
        )
    }

    /// A path outside the worktree an agent can leave a mark at.
    fn mark(&self, name: &str) -> PathBuf {
        self.home.join(name)
    }
}

impl Drop for Kit {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

/// git in a directory, refusing to carry on when it says no.
fn run(dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
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

fn person() -> Author {
    Author::parse("leandro").unwrap()
}

fn now() -> Timestamp {
    app::now()
}

/// A shutdown flag nobody has flipped.
fn running() -> AtomicBool {
    AtomicBool::new(false)
}

// --- what a pass does ---

#[test]
fn a_runner_answers_a_handover_with_a_revision() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    let note = kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let agent = "cat > brief.txt && printf 'one\\nTWO_NAMED\\n' > a.txt \
                 && git add -A && git commit -qm 'the agent answered'";
    let done = kit.runner(agent).once(&running())?;

    let after = kit.load(proposal.id());
    assert_eq!(done, 1);
    assert_eq!(after.head().number(), 2);
    assert_eq!(after.work().len(), 2);
    assert!(after.activity().is_some_and(|line| line.idle()));
    assert!(!after.dispatched());
    assert_eq!(after.open_comments().len(), 0);

    // The brief the agent read is in the commit it made, so what reached its
    // stdin is a thing this test can look at.
    let brief = kit.git(&["show", &format!("{}:brief.txt", after.head().sha())]);
    assert!(brief.contains(note.id().as_str()), "{brief}");
    assert!(brief.contains("$GITHERB_ANSWERS"), "{brief}");
    Ok(())
}

#[test]
fn a_runner_rebases_what_the_target_ran_past() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");

    // Somebody else lands on the trunk, in another file, so the change still
    // applies and no agent is needed.
    kit.git(&["checkout", "-q", "main"]);
    kit.write("c.txt", "somebody else\n");
    kit.commit("the other work");

    let done = kit.runner("false").once(&running())?;

    let after = kit.load(proposal.id());
    assert_eq!(done, 1);
    assert_eq!(after.head().number(), 2);

    let tip = kit.git(&["rev-parse", "main"]);
    let parent = kit.git(&["rev-parse", &format!("{}^", after.head().sha())]);
    assert_eq!(parent, tip);
    Ok(())
}

#[test]
fn a_failure_is_written_down_and_not_retried() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let loop_over = kit.runner("echo 'no thanks' >&2; exit 3");
    assert_eq!(loop_over.once(&running())?, 1);

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert!(activity.failed());
    assert_eq!(activity.note(), Some("the agent stopped: no thanks"));

    // The second pass finds the same failure and leaves it alone.
    assert_eq!(loop_over.once(&running())?, 0);
    Ok(())
}

#[test]
fn a_claim_found_abandoned_is_handed_back() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");

    // What a runner that died mid-job leaves behind.
    app::report(
        &kit.store,
        &Author::parse("githerb-run").unwrap(),
        now(),
        proposal.id(),
        "check",
        "started",
        None,
    )
    .unwrap();
    assert!(
        kit.load(proposal.id())
            .activity()
            .is_some_and(|line| line.working())
    );

    let cleared = kit.runner("true").recover()?;

    let after = kit.load(proposal.id());
    assert_eq!(cleared, 1);
    assert!(after.activity().is_some_and(|line| line.idle()));
    assert_eq!(
        after
            .activity()
            .and_then(|line| line.note().map(str::to_owned)),
        Some("the runner that claimed this is gone".to_owned())
    );
    Ok(())
}

// --- what the agent says ---

#[test]
fn an_agent_can_answer_in_words_without_touching_code() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    let note = kit.note(proposal.id(), "only claude, or any agent?");
    kit.hand_over(proposal.id());

    let talker = format!(
        "printf '{{\"note\":\"{}\",\"say\":\"any agent CLI works\"}}\\n' >> \"$GITHERB_ANSWERS\"",
        note.id()
    );
    assert_eq!(kit.runner(&talker).once(&running())?, 1);

    let after = kit.load(proposal.id());
    assert_eq!(after.head().number(), 1);
    assert!(after.activity().is_some_and(|line| line.idle()));

    let answers = after.answers(note.id());
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].body(), "any agent CLI works");
    assert_eq!(answers[0].author().as_str(), "githerb-run");

    // Answering is not resolving: the note is still what blocks landing.
    assert_eq!(after.open_comments().len(), 1);
    Ok(())
}

#[test]
fn an_agent_that_changes_code_and_says_nothing_is_spoken_for() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    let note = kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let mute = "printf 'one\\nNAMED\\n' > a.txt && git add -A \
                && git commit -qm 'name the second line'";
    assert_eq!(kit.runner(mute).once(&running())?, 1);

    let after = kit.load(proposal.id());
    let answers = after.answers(note.id());
    assert_eq!(answers.len(), 1);
    assert_eq!(
        answers[0].body(),
        format!("left {}: name the second line", after.head().sha().short())
    );
    Ok(())
}

#[test]
fn a_long_thing_the_agent_said_becomes_a_note_rather_than_a_refusal() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let chatty = "printf 'x%.0s' $(seq 1 400); echo; exit 1";
    assert_eq!(kit.runner(chatty).once(&running())?, 1);

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    let note = activity.note().unwrap_or_default();
    assert!(activity.failed());
    assert!(note.starts_with("the agent stopped: xxx"), "{note}");
    assert!(note.chars().count() <= 140, "{}", note.chars().count());
    Ok(())
}

// --- stopping ---

#[test]
fn a_job_the_runner_is_stopped_in_the_middle_of_is_written_down_as_failed() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let marker = kit.mark("running");
    let slow = format!("echo $$ > '{}'; sleep 30", marker.display());
    let loop_over = kit.runner(&slow);
    let shutdown = AtomicBool::new(false);

    let done = thread::scope(|scope| {
        let worker = scope.spawn(|| loop_over.once(&shutdown));

        // Only the child can say it started, and it says so by writing its own
        // pid down before it sleeps.
        let deadline = Instant::now() + Duration::from_secs(20);
        while !marker.exists() {
            assert!(Instant::now() < deadline, "the agent never started");
            thread::sleep(Duration::from_millis(5));
        }

        let began = Instant::now();
        shutdown.store(true, Ordering::Relaxed);
        let done = worker.join().unwrap();

        assert!(
            began.elapsed() < Duration::from_secs(5),
            "it waited the sleep out"
        );

        done
    })?;

    // Nothing is left claimed: the job that was cut short says so itself.
    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert_eq!(done, 1);
    assert!(activity.failed());
    assert_eq!(activity.note(), Some("runner stopped"));
    Ok(())
}

#[test]
fn a_pass_with_nothing_to_answer_does_nothing() -> Result<(), Error> {
    let kit = Kit::new();
    kit.proposed("TWO");

    assert_eq!(kit.runner("false").once(&running())?, 0);
    Ok(())
}

// --- the loop ---

#[test]
fn the_loop_passes_again_after_its_floor_and_returns_when_it_is_stopped() -> Result<(), Error> {
    let kit = Kit::new();
    kit.proposed("TWO");
    let loop_over = kit.runner("false");
    let shutdown = AtomicBool::new(false);
    let every = Duration::from_millis(100);

    let mut passes = 0;
    let began = Instant::now();

    // A waiter that behaves: it waits out the budget it was given and reports
    // that nothing woke it.
    loop_over.run(
        &mut |budget| {
            passes += 1;
            thread::sleep(budget);
            shutdown.store(passes >= 2, Ordering::Relaxed);

            false
        },
        every,
        &shutdown,
    )?;

    assert_eq!(passes, 2);
    assert!(began.elapsed() >= every * 2, "{:?}", began.elapsed());
    Ok(())
}

#[test]
fn a_waiter_that_answers_instantly_does_not_spin_the_floor_away() -> Result<(), Error> {
    let kit = Kit::new();
    kit.proposed("TWO");
    let loop_over = kit.runner("false");
    let shutdown = AtomicBool::new(false);
    let every = Duration::from_millis(100);

    let mut answers = 0;
    let mut first: Option<Instant> = None;
    let mut waiting = Duration::ZERO;

    // A waiter that reports a wake-up it never waited for. The floor is the
    // runner's to hold, not the waiter's, so these answers have to cost real
    // time even though the waiter costs none.
    loop_over.run(
        &mut |_| {
            answers += 1;
            waiting = first.get_or_insert_with(Instant::now).elapsed();
            shutdown.store(answers >= 12, Ordering::Relaxed);

            true
        },
        every,
        &shutdown,
    )?;

    assert_eq!(answers, 12);
    assert!(waiting >= Duration::from_millis(80), "{waiting:?}");
    Ok(())
}

// --- more of what a pass does ---

#[test]
fn a_pass_hands_back_an_abandoned_claim_before_working_out_what_to_do() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    app::report(
        &kit.store,
        &Author::parse("githerb-run").unwrap(),
        now(),
        proposal.id(),
        "apply",
        "started",
        None,
    )
    .unwrap();

    kit.runner("true").once(&running())?;

    let after = kit.load(proposal.id());
    assert!(after.activity().is_some_and(|line| line.idle()));
    Ok(())
}

#[test]
fn an_agent_that_answers_and_commits_is_not_spoken_for_twice() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    let note = kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let both = format!(
        "printf '{{\"note\":\"{}\",\"say\":\"named it NAMED\"}}\\n' >> \"$GITHERB_ANSWERS\"; \
         printf 'one\\nNAMED\\n' > a.txt && git add -A && git commit -qm 'name the second line'",
        note.id()
    );
    kit.runner(&both).once(&running())?;

    let after = kit.load(proposal.id());
    let answers = after.answers(note.id());
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].body(), "named it NAMED");
    Ok(())
}

#[test]
fn an_agent_that_leaves_the_worktree_where_it_found_it_is_a_failure() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    kit.runner("true").once(&running())?;

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert_eq!(after.head().number(), 1);
    assert!(activity.failed());
    assert_eq!(
        activity.note(),
        Some("the agent left the worktree where it found it")
    );
    Ok(())
}

#[test]
fn a_conflict_nobody_asked_an_agent_about_is_handed_back() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");

    // The trunk changes the same line, so the replay cannot be mechanical.
    kit.git(&["checkout", "-q", "main"]);
    kit.write("a.txt", "one\nTRUNK\n");
    kit.commit("the other work");

    kit.runner("false").once(&running())?;

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert_eq!(after.head().number(), 1);
    assert!(activity.failed());
    assert_eq!(activity.note(), Some("the rebase is still conflicted"));
    Ok(())
}

// --- the gate ---

#[test]
fn the_gate_runs_on_the_head_and_is_written_down() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");

    assert_eq!(kit.gate("true").once(&running())?, 1);

    let after = kit.load(proposal.id());
    let checks = after.checks();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].name().as_str(), "gate");
    assert!(checks[0].passed());
    assert_eq!(
        after
            .activity()
            .and_then(|line| line.note().map(str::to_owned)),
        Some("1 checks passed".to_owned())
    );

    // A revision that answered is not asked again.
    assert_eq!(kit.gate("true").once(&running())?, 0);
    Ok(())
}

#[test]
fn a_gate_that_says_no_fails_the_job() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");

    assert_eq!(kit.gate("false").once(&running())?, 1);

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert!(activity.failed());
    assert_eq!(activity.note(), Some("1 of 1 checks failed"));
    assert_eq!(after.failing().len(), 1);
    Ok(())
}

#[test]
fn a_conflict_on_a_proposal_that_was_handed_over_goes_to_the_agent() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.hand_over(proposal.id());

    kit.git(&["checkout", "-q", "main"]);
    kit.write("a.txt", "one\nTRUNK\n");
    kit.commit("the other work");

    // The agent that wrote the code is the one asked to resolve it, in the
    // worktree git left halfway through the rebase.
    let mediator = "printf 'one\\nBOTH\\n' > a.txt && git add a.txt \
                    && GIT_EDITOR=true git rebase --continue";
    assert_eq!(kit.runner(mediator).once(&running())?, 1);

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert!(activity.idle(), "{:?}", activity.note());
    assert_eq!(after.head().number(), 2);

    let tip = kit.git(&["rev-parse", "main"]);
    let parent = kit.git(&["rev-parse", &format!("{}^", after.head().sha())]);
    assert_eq!(parent, tip);
    assert_eq!(
        kit.git(&["show", &format!("{}:a.txt", after.head().sha())]),
        "one\nBOTH"
    );
    Ok(())
}

#[test]
fn an_agent_that_walks_away_from_a_conflict_hands_it_back() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.hand_over(proposal.id());

    kit.git(&["checkout", "-q", "main"]);
    kit.write("a.txt", "one\nTRUNK\n");
    kit.commit("the other work");

    // An agent that says nothing went wrong and leaves the rebase where it
    // found it: halfway through.
    kit.runner("true").once(&running())?;

    let after = kit.load(proposal.id());
    let activity = after.activity().unwrap();
    assert_eq!(after.head().number(), 1);
    assert!(activity.failed());
    assert_eq!(activity.note(), Some("the rebase is still conflicted"));
    Ok(())
}

#[test]
fn the_agent_is_told_which_proposal_it_is_on() -> Result<(), Error> {
    let kit = Kit::new();
    let proposal = kit.proposed("TWO");
    kit.note(proposal.id(), "name it properly");
    kit.hand_over(proposal.id());

    let curious = "printf '%s\\n' \"$GITHERB_PROPOSAL\" > who.txt \
                   && git add -A && git commit -qm 'said who'";
    kit.runner(curious).once(&running())?;

    let after = kit.load(proposal.id());
    assert_eq!(
        kit.git(&["show", &format!("{}:who.txt", after.head().sha())]),
        proposal.id().as_str()
    );
    Ok(())
}
