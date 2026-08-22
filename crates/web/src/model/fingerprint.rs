//! One short string that answers "has anything the page shows moved".
//!
//! It is hashed from the records, never from the html: rendering the same
//! proposal twice has to produce the same fingerprint, or every reconnecting
//! tab would be handed a page it already has. That was the first and largest
//! cause of the old surface stalling on load.

use std::hash::{DefaultHasher, Hash, Hasher};

use review::Proposal;

/// The fingerprint of everything a review page draws.
///
/// Whatever a render reads has to be hashed here, or the page will not follow
/// the repository. That is: the state, the head, the notes and their answers,
/// what is resolved, the checks on the head, what the author explained, the
/// work log and whether an agent has been asked for.
#[must_use]
pub fn fingerprint(proposal: &Proposal) -> String {
    let mut hasher = DefaultHasher::new();

    proposal.state().as_str().hash(&mut hasher);
    proposal.head().number().hash(&mut hasher);
    proposal.head().sha().as_str().hash(&mut hasher);
    proposal.target().as_str().hash(&mut hasher);
    proposal.title().hash(&mut hasher);

    for note in proposal.comments() {
        note.id().as_str().hash(&mut hasher);
        proposal.is_resolved(note.id()).hash(&mut hasher);
        for reply in proposal.answers(note.id()) {
            reply.id().as_str().hash(&mut hasher);
        }
    }

    for check in proposal.checks() {
        check.name().as_str().hash(&mut hasher);
        check.status().as_str().hash(&mut hasher);
        check.seconds().hash(&mut hasher);
    }

    for chunk in proposal.chunks() {
        chunk.title().hash(&mut hasher);
        chunk.decision().hash(&mut hasher);
    }

    for note in proposal.rationale() {
        note.id().as_str().hash(&mut hasher);
    }

    for line in proposal.work() {
        line.revision().as_str().hash(&mut hasher);
        line.task().as_str().hash(&mut hasher);
        line.phase().as_str().hash(&mut hasher);
        line.agent().as_str().hash(&mut hasher);
        line.note().hash(&mut hasher);
        line.at().unix().hash(&mut hasher);
    }

    // The asks are not readable one by one, and they only ever matter through
    // this answer: whether the head is waiting for somebody to pick it up.
    proposal.dispatched().hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{at, author, note, proposal};
    use review::{
        Check, CheckName, CheckStatus, Dispatch, Phase, Record, Reply, Resolution, Side, Task, Work,
    };

    /// A proposal carrying one note, and what that note is called.
    fn noted() -> (review::Proposal, review::RecordId) {
        let mut proposal = proposal();
        let head = proposal.head().sha().clone();
        let id = note(
            &mut proposal,
            &head,
            "README.md",
            Side::New,
            1,
            1,
            "drop it",
        );
        (proposal, id)
    }

    #[test]
    fn rendering_the_same_proposal_twice_gives_the_same_fingerprint() {
        let (proposal, _) = noted();
        assert_eq!(fingerprint(&proposal), fingerprint(&proposal));
        assert_eq!(fingerprint(&proposal).len(), 16);
    }

    #[test]
    fn a_note_moves_the_fingerprint() {
        let before = fingerprint(&proposal());
        let (proposal, _) = noted();
        assert_ne!(before, fingerprint(&proposal));
    }

    #[test]
    fn a_reply_moves_the_fingerprint() {
        let (mut proposal, id) = noted();
        let before = fingerprint(&proposal);
        let head = proposal.head().sha().clone();
        let reply = Reply::new(id, head, "done", author("agent"), at(51)).unwrap();
        proposal.apply(Record::Reply(reply)).unwrap();
        assert_ne!(before, fingerprint(&proposal));
    }

    #[test]
    fn a_resolution_moves_the_fingerprint() {
        let (mut proposal, id) = noted();
        let before = fingerprint(&proposal);
        proposal
            .apply(Record::Resolve(Resolution::new(
                id,
                author("leandro"),
                at(52),
            )))
            .unwrap();
        assert_ne!(before, fingerprint(&proposal));
    }

    #[test]
    fn a_check_moves_the_fingerprint() {
        let (mut proposal, _) = noted();
        let before = fingerprint(&proposal);
        let head = proposal.head().sha().clone();
        proposal
            .apply(Record::Check(Check::new(
                CheckName::parse("gate").unwrap(),
                CheckStatus::Passed,
                head,
                5,
                author("leandro"),
                at(53),
            )))
            .unwrap();
        assert_ne!(before, fingerprint(&proposal));
    }

    #[test]
    fn a_work_line_moves_the_fingerprint() {
        let (mut proposal, _) = noted();
        let before = fingerprint(&proposal);
        let head = proposal.head().sha().clone();
        let work = Work::new(
            head,
            Task::Apply,
            Phase::Started,
            author("githerb-run"),
            None,
            at(54),
        )
        .unwrap();
        proposal.apply(Record::Work(work)).unwrap();
        assert_ne!(before, fingerprint(&proposal));
    }

    #[test]
    fn a_dispatch_moves_the_fingerprint() {
        let (mut proposal, _) = noted();
        let before = fingerprint(&proposal);
        let head = proposal.head().sha().clone();
        proposal
            .apply(Record::Dispatch(Dispatch::new(
                head,
                author("leandro"),
                at(55),
            )))
            .unwrap();
        assert_ne!(before, fingerprint(&proposal));
    }

    #[test]
    fn a_new_revision_moves_the_fingerprint() {
        let (mut proposal, _) = noted();
        let before = fingerprint(&proposal);
        proposal.add_revision(crate::fixtures::sha('c')).unwrap();
        assert_ne!(before, fingerprint(&proposal));
    }
}
