//! What each task does, from the worktree it does it in to the records it
//! leaves behind.
//!
//! Three tasks and no more: answer the notes, move the work onto a target that
//! ran ahead, run what the repository declares. Each one is a sequence with
//! one shape: open a worktree of the head, do something in it, and read the
//! result back out of git rather than out of whatever the agent claimed.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use app::Config;
use gitstore::RebaseOutcome;
use review::{Proposal, ProposalId, RecordId, Sha};

use crate::agent::Agent;
use crate::answers::{Answer, AnswersFile, read_answers};
use crate::error::Error;
use crate::runner::{Runner, first_line};
use crate::tree::Worktree;

/// Where the agent is told to write what it says.
const ANSWERS_ENV: &str = "GITHERB_ANSWERS";

/// Which proposal the agent is being asked about.
const PROPOSAL_ENV: &str = "GITHERB_PROPOSAL";

impl Runner {
    /// Hand the open notes to the agent in a worktree of the head.
    ///
    /// The agent answers in words, in code, or in both, and neither half is
    /// optional enough to fail on its own: a question answered without a
    /// commit is work, and a commit with nothing said is work somebody has to
    /// be told about.
    pub(super) fn apply(
        &self,
        config: &Config,
        proposal: &Proposal,
        shutdown: &AtomicBool,
    ) -> Result<String, Error> {
        let brief = review::brief(proposal);

        if brief.is_empty() {
            return Err(Error::NothingToApply);
        }

        let agent = agent(config)?;
        let answers = AnswersFile::create(proposal.id().as_str())?;
        let path = answers.path().to_str().ok_or(Error::NothingToApply)?;
        let head = proposal.head().sha().clone();
        let where_it_runs = Worktree::open(self.store.repo(), head.as_str())?;

        agent.call(
            where_it_runs.path(),
            &brief,
            &[(ANSWERS_ENV, path), (PROPOSAL_ENV, proposal.id().as_str())],
            shutdown,
        )?;

        let mut said = self.speak(proposal.id(), answers.path());
        let moved = Sha::parse(&where_it_runs.head()?)?;

        if moved == head {
            if said == 0 {
                return Err(Error::NothingChanged);
            }

            return Ok(format!("answered {said}, no code changed"));
        }

        // An agent that changed the code and said nothing leaves the person
        // who asked staring at a revision with no explanation.
        said += self.vouch(proposal, &moved);

        let next = self.record(proposal.id(), &moved)?;

        Ok(format!(
            "revision {} at {}, answered {said}",
            next.head().number(),
            moved.short()
        ))
    }

    /// Move the work onto a target that ran ahead.
    ///
    /// Git does it whenever the change still applies. When it does not, the
    /// agent that wrote the code is the one asked to resolve it, in the same
    /// worktree, mid-rebase, and only if somebody asked for an agent at all.
    pub(super) fn rebase(
        &self,
        config: &Config,
        proposal: &Proposal,
        shutdown: &AtomicBool,
    ) -> Result<String, Error> {
        let tip = self.store.repo().head_of(&proposal.target().git_ref())?;
        let head = proposal.head().sha().clone();
        let where_it_runs = Worktree::open(self.store.repo(), head.as_str())?;

        let outcome =
            self.store
                .repo()
                .rebase_onto(where_it_runs.path(), &tip, proposal.base().as_str())?;

        if outcome == RebaseOutcome::Conflicted {
            self.resolve(config, proposal, &where_it_runs, &tip, shutdown)?;
        }

        let moved = Sha::parse(&where_it_runs.head()?)?;

        if moved == head {
            return Err(Error::NothingChanged);
        }

        let next = self.record(proposal.id(), &moved)?;

        Ok(format!(
            "revision {} at {}",
            next.head().number(),
            moved.short()
        ))
    }

    /// A conflict, handed to the agent that was asked for, or handed back.
    ///
    /// A conflict needs judgement, and judgement belongs to the agent somebody
    /// dispatched. Anything else stops here and says so, leaving the worktree
    /// where the rebase found it.
    fn resolve(
        &self,
        config: &Config,
        proposal: &Proposal,
        where_it_runs: &Worktree,
        tip: &str,
        shutdown: &AtomicBool,
    ) -> Result<(), Error> {
        if !proposal.dispatched() {
            return Err(self.abort(where_it_runs, Error::ConflictsLeft));
        }

        let agent = match agent(config) {
            Ok(agent) => agent,
            Err(refused) => return Err(self.abort(where_it_runs, refused)),
        };

        let brief = review::conflict_brief(proposal.id(), &Sha::parse(tip)?);

        if let Err(refused) = agent.call(where_it_runs.path(), &brief, &[], shutdown) {
            return Err(self.abort(where_it_runs, refused));
        }

        if where_it_runs.rebasing() {
            return Err(self.abort(where_it_runs, Error::ConflictsLeft));
        }

        Ok(())
    }

    /// Put the worktree back and carry the reason out.
    fn abort(&self, where_it_runs: &Worktree, refused: Error) -> Error {
        // The worktree is thrown away either way; aborting only stops git
        // holding a rebase open on it.
        let _ignored = self.store.repo().rebase_abort(where_it_runs.path());

        refused
    }

    /// Run what the repository declares against the head revision.
    pub(super) fn check(&self, config: &Config, proposal: &Proposal) -> Result<String, Error> {
        let results = app::check(
            &self.store,
            config,
            &self.author,
            app::now(),
            proposal.id(),
            &mut std::io::sink(),
        )?;

        let failed = app::refused(&results);

        if failed > 0 {
            return Err(Error::ChecksFailed {
                failed,
                total: results.len(),
            });
        }

        Ok(format!("{} checks passed", results.len()))
    }

    // --- what the agent said ---

    /// File what the agent wrote under the notes it answered, and say how many
    /// landed.
    ///
    /// An answer naming a note nobody wrote is skipped, not fatal: the agent
    /// is outside code we control and the log takes no guesses.
    fn speak(&self, id: &ProposalId, path: &Path) -> usize {
        let (answers, unreadable) = match read_answers(path) {
            Ok(read) => read,
            Err(refused) => {
                self.say(&format!("{id}: reading the answers: {refused}"));

                return 0;
            }
        };

        for line in unreadable {
            self.say(&format!("{id}: an answer this build cannot read: {line}"));
        }

        let mut said = 0;

        for answer in answers {
            match self.answer(id, &answer) {
                Ok(()) => said += 1,
                Err(refused) => self.say(&format!("{id}: {refused}")),
            }
        }

        said
    }

    /// One answer, as a reply under the note it names.
    fn answer(&self, id: &ProposalId, answer: &Answer) -> Result<(), Error> {
        let note = RecordId::parse(answer.note())?;

        app::reply(
            &self.store,
            &self.author,
            app::now(),
            id,
            &note,
            answer.say(),
        )?;

        Ok(())
    }

    /// Answer, on the runner's own authority, every note the agent left in
    /// silence. What it can say is small and true: here is the commit that
    /// came out of asking.
    fn vouch(&self, proposal: &Proposal, moved: &Sha) -> usize {
        let Ok(subject) = self.store.repo().subject(moved.as_str()) else {
            return 0;
        };

        let Ok(fresh) = self.store.load(proposal.id()) else {
            return 0;
        };

        let body = format!("left {}: {}", moved.short(), first_line(&subject));
        let mut said = 0;

        for note in proposal.open_comments() {
            if fresh.answers(note.id()).len() > proposal.answers(note.id()).len() {
                continue;
            }

            if app::reply(
                &self.store,
                &self.author,
                app::now(),
                proposal.id(),
                note.id(),
                &body,
            )
            .is_ok()
            {
                said += 1;
            }
        }

        said
    }

    /// File the commit an agent left as the next revision.
    fn record(&self, id: &ProposalId, moved: &Sha) -> Result<Proposal, Error> {
        match app::revise(&self.store, id, moved.as_str()) {
            Ok(next) => Ok(next),
            // An agent that happens to have this CLI may have recorded the
            // revision itself. The commit is what matters and it is there
            // either way.
            Err(refused) if already_known(&refused) => Ok(self.store.load(id)?),
            Err(refused) => Err(Error::App(refused)),
        }
    }
}

/// The agent the repository declares.
fn agent(config: &Config) -> Result<Agent, Error> {
    Agent::new(config.agent().unwrap_or_default())
}

/// Whether the store is telling us the revision was already recorded.
fn already_known(refused: &app::Error) -> bool {
    matches!(refused, app::Error::Review(review::Error::RevisionKnown(_)))
}
