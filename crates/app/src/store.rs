//! Proposals in refs and notes, and the whole log in three processes.
//!
//! A revision is a ref, the lifecycle is a note on revision one, and every
//! annotation is a note on the revision it is about. Nothing is edited and
//! nothing is deleted: a resolution is a new line naming the line it answers,
//! which is what lets two people annotate the same revision and lets git
//! merge the result instead of conflicting.
//!
//! Reading is the reason this type exists. `for-each-ref` gives every
//! revision of every proposal, and each notes ref is two more processes, so
//! the cost of the whole store is fixed however long the log gets.

use gitstore::{EVENT_NOTES, PROPOSAL_REFS, RECORD_NOTES, Repo};
use review::{Event, Proposal, ProposalId, Record, Revision, Sha};

use crate::error::{Error, Result};
use crate::snapshot::Snapshot;

/// What git is told to do with two notes that both changed: take the union of
/// their lines. An append-only log has no conflicts, and this is where git is
/// told so.
const MERGE_STRATEGY: &str = "cat_sort_uniq";

/// The proposals of one repository.
#[derive(Debug, Clone)]
pub struct Store {
    repo: Repo,
}

impl Store {
    /// Point a store at a repository.
    #[must_use]
    pub fn new(repo: Repo) -> Self {
        Store { repo }
    }

    /// Point a store at the repository containing `dir`, which is how the
    /// terminal gets one: from wherever the person is standing.
    ///
    /// # Errors
    ///
    /// A directory that is not inside a git repository.
    pub fn at(dir: impl AsRef<std::path::Path>) -> Result<Self> {
        Ok(Store::new(Repo::open(dir)?))
    }

    /// The repository underneath, for the things that are about git rather
    /// than about proposals.
    #[must_use]
    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    /// One cheap string that moves whenever anything this tool reads moved.
    pub fn fingerprint(&self) -> Result<String> {
        Ok(self.repo.fingerprint()?)
    }

    // --- reading ---

    /// Every proposal, rebuilt from the log.
    pub fn snapshot(&self) -> Result<Snapshot> {
        // The fingerprint is taken first: a write that lands halfway through
        // the read leaves a snapshot marked with the older fingerprint, so the
        // next comparison reads it again rather than believing this one.
        let fingerprint = self.fingerprint()?;
        let refs = self.repo.refs(PROPOSAL_REFS)?;
        let events = self.repo.notes(EVENT_NOTES)?;
        let records = self.repo.notes(RECORD_NOTES)?;

        Snapshot::assemble(&refs, &events, &records, fingerprint)
    }

    /// The whole log again, but only if it moved since `known`.
    ///
    /// This is what a page or a runner keeps: one cheap process answers "did
    /// anything happen", and nothing else is read when the answer is no.
    pub fn snapshot_if_changed(&self, known: &str) -> Result<Option<Snapshot>> {
        if self.fingerprint()? == known {
            return Ok(None);
        }

        self.snapshot().map(Some)
    }

    /// One proposal by name.
    pub fn load(&self, id: &ProposalId) -> Result<Proposal> {
        self.snapshot()?
            .get(id)
            .cloned()
            .ok_or_else(|| Error::NotFound(id.clone()))
    }

    /// Every proposal, newest first.
    pub fn list(&self) -> Result<Vec<Proposal>> {
        Ok(self.snapshot()?.proposals().to_vec())
    }

    // --- writing ---

    /// Write down a new proposal and its first revision.
    pub fn open(&self, proposal: &Proposal, event: &Event) -> Result<()> {
        self.configure()?;
        self.write_revision(proposal.id(), proposal.head())?;

        self.event(proposal.head().sha(), event)
    }

    /// Write down another attempt at an open proposal.
    pub fn revise(&self, id: &ProposalId, number: u32, sha: &Sha) -> Result<()> {
        self.write_revision(id, &Revision::new(number, sha.clone()))
    }

    /// Append one record to the log of a revision.
    pub fn annotate(&self, revision: &Sha, record: &Record) -> Result<()> {
        Ok(self
            .repo
            .note_append(RECORD_NOTES, revision.as_str(), &record.to_line())?)
    }

    /// Append one event to the log of a proposal, which lives on revision one.
    pub fn event(&self, first_revision: &Sha, event: &Event) -> Result<()> {
        Ok(self
            .repo
            .note_append(EVENT_NOTES, first_revision.as_str(), &event.to_line())?)
    }

    /// Move the target branch onto the proposal's head and write it down.
    ///
    /// Fast-forward only. A proposal that has fallen behind its target is a
    /// proposal whose review looked at the wrong code, and the compare-and-swap
    /// means the loser of a race is told rather than overwritten.
    pub fn land(&self, proposal: &Proposal, event: &Event) -> Result<()> {
        let target = proposal.target();
        let head = proposal.head().sha();
        let current = self.repo.head_of(&target.git_ref())?;

        if !self.repo.is_ancestor(&current, head.as_str())? {
            return Err(Error::NotFastForward(target.clone()));
        }

        // The branch the checkout is on cannot move alone: the index and the
        // working tree go with it, or the next commit from here reverts the
        // land. Any other branch is a ref, and a ref moves by itself.
        if self.repo.current_branch()?.as_deref() == Some(target.git_ref().as_str()) {
            self.repo.fast_forward(head.as_str()).map_err(|refused| {
                Error::WorkingTreeInTheWay {
                    target: target.clone(),
                    detail: first_line(&refused),
                }
            })?;
        } else {
            self.repo
                .update_ref(&target.git_ref(), head.as_str(), Some(&current))?;
        }

        self.event(first_revision(proposal), event)
    }

    /// Say that a proposal is either landing somewhere else now, or not
    /// landing at all.
    pub fn record(&self, proposal: &Proposal, event: &Event) -> Result<()> {
        self.event(first_revision(proposal), event)
    }

    /// Tell git that these notes merge by union.
    ///
    /// Idempotent, and done on every open rather than once at install time,
    /// because a clone somebody else made has never been told.
    fn configure(&self) -> Result<()> {
        for notes_ref in [RECORD_NOTES, EVENT_NOTES] {
            self.repo
                .config_set(&format!("notes.{notes_ref}.mergeStrategy"), MERGE_STRATEGY)?;
        }

        Ok(())
    }

    fn write_revision(&self, id: &ProposalId, revision: &Revision) -> Result<()> {
        let name = format!("{PROPOSAL_REFS}/{id}/{}", revision.number());

        Ok(self.repo.update_ref(&name, revision.sha().as_str(), None)?)
    }
}

/// Where a proposal's events live: the commit of revision one, whatever the
/// head is now.
fn first_revision(proposal: &Proposal) -> &Sha {
    proposal
        .revisions()
        .first()
        .map_or_else(|| proposal.head().sha(), |revision| revision.sha())
}

/// What git said, first line only, without the command that said it.
fn first_line(refused: &gitstore::Error) -> String {
    match refused {
        gitstore::Error::Git { stderr, .. } => stderr.lines().next().unwrap_or("").to_owned(),
        gitstore::Error::NotARepository(_) | gitstore::Error::Io(_) | gitstore::Error::Utf8 => {
            refused.to_string()
        }
    }
}
