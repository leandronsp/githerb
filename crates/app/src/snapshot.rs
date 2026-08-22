//! Every proposal in the repository, rebuilt from the log in one pass.
//!
//! The whole store is three git processes: the refs, the event notes and the
//! annotation notes. Everything after that is folding text that is already in
//! memory, so this module does no I/O at all and can be tested with a handful
//! of strings.
//!
//! A snapshot remembers the fingerprint it was read at. That is what lets a
//! caller keep one and ask the store whether anything moved instead of
//! reading it all again.

use std::collections::{BTreeMap, HashMap};

use review::{
    Branch, Event, EventKind, Proposal, ProposalId, Record, Revision, Sha, State, Timestamp,
};

use crate::error::{Error, Result};

/// The proposal log, whole, as it stood at one fingerprint.
#[derive(Debug, Clone)]
pub struct Snapshot {
    proposals: Vec<Proposal>,
    fingerprint: String,
}

impl Snapshot {
    /// What the repository looked like when this was read. Only equality
    /// means anything.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Every proposal, newest first.
    #[must_use]
    pub fn proposals(&self) -> &[Proposal] {
        &self.proposals
    }

    /// One proposal by name.
    #[must_use]
    pub fn get(&self, id: &ProposalId) -> Option<&Proposal> {
        self.proposals.iter().find(|proposal| proposal.id() == id)
    }

    /// The proposals that have neither landed nor been given up on.
    pub fn open(&self) -> impl Iterator<Item = &Proposal> {
        self.proposals
            .iter()
            .filter(|proposal| proposal.state() == State::Open)
    }

    /// Rebuild every proposal from what the three reads found.
    ///
    /// `events` and `records` are the notes refs as git handed them over:
    /// annotated object to note text.
    ///
    /// # Errors
    ///
    /// A ref that is not a revision, a proposal nobody opened, or a log line
    /// this build refuses to read past.
    pub(crate) fn assemble(
        refs: &[(String, String)],
        events: &HashMap<String, String>,
        records: &HashMap<String, String>,
        fingerprint: String,
    ) -> Result<Self> {
        let mut proposals = Vec::new();

        for (id, revisions) in revisions(refs)? {
            proposals.push(rebuild(&id, &revisions, events, records)?);
        }

        // Newest first, and by name when two were opened in the same second,
        // so a listing does not shuffle between reads.
        proposals.sort_by(|one, other| {
            other
                .opened_at()
                .cmp(&one.opened_at())
                .then_with(|| one.id().cmp(other.id()))
        });

        Ok(Snapshot {
            proposals,
            fingerprint,
        })
    }
}

/// The revisions of every proposal, by name, each ascending by number.
fn revisions(refs: &[(String, String)]) -> Result<BTreeMap<ProposalId, Vec<Revision>>> {
    let mut found: BTreeMap<ProposalId, Vec<Revision>> = BTreeMap::new();

    for (name, sha) in refs {
        let (id, number) = split_ref(name)?;

        found
            .entry(id)
            .or_default()
            .push(Revision::new(number, Sha::parse(sha)?));
    }

    for revisions in found.values_mut() {
        revisions.sort_by_key(Revision::number);
    }

    Ok(found)
}

/// `refs/githerb/proposals/<id>/<n>` split into the two things it says.
fn split_ref(name: &str) -> Result<(ProposalId, u32)> {
    let (id, number) = name
        .strip_prefix(gitstore::PROPOSAL_REFS)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.rsplit_once('/'))
        .ok_or_else(|| Error::NotARevision(name.to_owned()))?;

    let number = number
        .parse::<u32>()
        .map_err(|_ignored| Error::NotARevision(name.to_owned()))?;

    Ok((ProposalId::parse(id)?, number))
}

/// One proposal, from its refs and the two logs attached to them.
fn rebuild(
    id: &ProposalId,
    revisions: &[Revision],
    events: &HashMap<String, String>,
    records: &HashMap<String, String>,
) -> Result<Proposal> {
    let first = revisions
        .first()
        .ok_or_else(|| Error::NotFound(id.clone()))?;

    let log = read(events, first.sha(), "proposal log", Event::parse_line)?;
    let mut proposal = opened(id, first.sha(), &log)?;

    for (_, target) in moves(&log) {
        proposal.retarget(target.clone())?;
    }

    for revision in revisions.iter().skip(1) {
        proposal.add_revision(revision.sha().clone())?;
    }

    for revision in revisions {
        let annotations = read(
            records,
            revision.sha(),
            "annotation log",
            Record::parse_line,
        )?;
        proposal.fold(annotations)?;
    }

    end(&mut proposal, &log);

    Ok(proposal)
}

/// The proposal as the first `opened` event described it.
fn opened(id: &ProposalId, head: &Sha, log: &[Event]) -> Result<Proposal> {
    for event in log {
        if let Event::Opened {
            title,
            target,
            base,
            at,
            ..
        } = event
        {
            return Ok(Proposal::open(
                id.clone(),
                title,
                target.clone(),
                base.clone(),
                head.clone(),
                *at,
            )?);
        }
    }

    Err(Error::NotFound(id.clone()))
}

/// Every retarget, oldest first. The log is a set once two machines have
/// merged it, so the timestamp is what orders it.
fn moves(log: &[Event]) -> Vec<(Timestamp, &Branch)> {
    let mut moves: Vec<(Timestamp, &Branch)> = log
        .iter()
        .filter_map(|event| match event {
            Event::Retargeted { target, at, .. } => Some((*at, target)),
            Event::Opened { .. } | Event::Landed { .. } | Event::Abandoned { .. } => None,
        })
        .collect();
    moves.sort_by_key(|(at, _)| *at);
    moves
}

/// Whatever ended the proposal, if anything did. The gate was answered when
/// it landed; reading it back is not the moment to ask again.
fn end(proposal: &mut Proposal, log: &[Event]) {
    for event in log {
        match event.kind() {
            EventKind::Landed => {
                proposal.mark_landed(event.at());
                return;
            }
            EventKind::Abandoned => {
                proposal.mark_abandoned(event.at());
                return;
            }
            EventKind::Opened | EventKind::Retargeted => {}
        }
    }
}

/// Parse one note into records, skipping what a newer build wrote.
///
/// A kind this build does not know came from a newer one and is passed over,
/// which is what keeps an old binary able to open a new proposal. A version
/// it does not speak is refused, because that changes what the fields mean.
fn read<T>(
    notes: &HashMap<String, String>,
    object: &Sha,
    log: &'static str,
    parse: fn(&str) -> review::Result<T>,
) -> Result<Vec<T>> {
    let Some(text) = notes.get(object.as_str()) else {
        return Ok(Vec::new());
    };

    let mut parsed = Vec::new();

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        match parse(line) {
            Ok(record) => parsed.push(record),
            Err(review::Error::UnknownKind(_)) => {}
            Err(err) => {
                return Err(Error::Log {
                    log,
                    object: object.clone(),
                    source: err,
                });
            }
        }
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(letter: char) -> String {
        std::iter::repeat_n(letter, 40).collect()
    }

    fn refname(id: &str, number: u32) -> String {
        format!("{}/{id}/{number}", gitstore::PROPOSAL_REFS)
    }

    fn opened_line(id: &str, at: &str) -> String {
        format!(
            r#"{{"v":1,"kind":"opened","id":"{id}","title":"a title","target":"main","base":"{}","author":"ada","at":"{at}"}}"#,
            sha('b')
        )
    }

    fn snapshot_of(refs: &[(String, String)], events: &[(String, String)]) -> Result<Snapshot> {
        annotated(refs, events, &[])
    }

    fn annotated(
        refs: &[(String, String)],
        events: &[(String, String)],
        records: &[(String, String)],
    ) -> Result<Snapshot> {
        Snapshot::assemble(
            refs,
            &events.iter().cloned().collect(),
            &records.iter().cloned().collect(),
            "fp".to_owned(),
        )
    }

    #[test]
    fn a_ref_whose_basename_is_not_a_number_is_not_a_revision() {
        let refs = vec![(refname("gate", 1).replace("/1", "/one"), sha('a'))];

        let err = snapshot_of(&refs, &[]).unwrap_err();

        assert!(matches!(err, Error::NotARevision(_)), "{err}");
    }

    #[test]
    fn a_proposal_nobody_opened_is_not_found() {
        let refs = vec![(refname("gate", 1), sha('a'))];

        let err = snapshot_of(&refs, &[]).unwrap_err();

        assert!(matches!(err, Error::NotFound(_)), "{err}");
    }

    #[test]
    fn the_revisions_come_back_in_order_however_the_refs_arrived() -> Result<()> {
        let refs = vec![
            (refname("gate", 2), sha('c')),
            (refname("gate", 1), sha('a')),
            (refname("gate", 10), sha('d')),
        ];
        let events = vec![(sha('a'), opened_line("gate", "2026-01-01T00:00:00Z"))];

        let snapshot = snapshot_of(&refs, &events)?;
        let proposal = &snapshot.proposals()[0];

        // The ref number orders the revisions; the proposal numbers them
        // itself, so a gap in the refs does not become a gap on the aggregate.
        assert_eq!(
            proposal
                .revisions()
                .iter()
                .map(|revision| revision.sha().as_str().to_owned())
                .collect::<Vec<_>>(),
            vec![sha('a'), sha('c'), sha('d')]
        );
        assert_eq!(proposal.head().number(), 3);
        Ok(())
    }

    #[test]
    fn the_newest_proposal_is_first() -> Result<()> {
        let refs = vec![(refname("old", 1), sha('a')), (refname("new", 1), sha('c'))];
        let events = vec![
            (sha('a'), opened_line("old", "2026-01-01T00:00:00Z")),
            (sha('c'), opened_line("new", "2026-02-01T00:00:00Z")),
        ];

        let snapshot = snapshot_of(&refs, &events)?;

        assert_eq!(
            snapshot
                .proposals()
                .iter()
                .map(|proposal| proposal.id().as_str())
                .collect::<Vec<_>>(),
            vec!["new", "old"]
        );
        Ok(())
    }

    #[test]
    fn a_kind_from_the_future_is_skipped_and_a_version_is_not() -> Result<()> {
        let refs = vec![(refname("gate", 1), sha('a'))];
        let events = vec![(sha('a'), opened_line("gate", "2026-01-01T00:00:00Z"))];
        let future =
            r#"{"v":1,"kind":"telepathy","id":"","author":"x","at":"2026-01-01T00:00:00Z"}"#;
        let records = vec![(sha('a'), future.to_owned())];

        let snapshot = annotated(&refs, &events, &records)?;
        assert_eq!(snapshot.proposals().len(), 1);

        let newer = r#"{"v":2,"kind":"comment","id":"","author":"x","at":"2026-01-01T00:00:00Z"}"#;
        let refused = annotated(&refs, &events, &[(sha('a'), newer.to_owned())]).unwrap_err();

        assert!(
            matches!(
                refused,
                Error::Log {
                    log: "annotation log",
                    ..
                }
            ),
            "{refused}"
        );
        Ok(())
    }

    #[test]
    fn an_event_kind_from_the_future_is_skipped_too() -> Result<()> {
        let refs = vec![(refname("gate", 1), sha('a'))];
        let future =
            r#"{"v":1,"kind":"summoned","id":"gate","author":"x","at":"2026-01-01T00:00:00Z"}"#;
        let events = vec![(
            sha('a'),
            format!("{future}\n{}", opened_line("gate", "2026-01-01T00:00:00Z")),
        )];

        assert_eq!(snapshot_of(&refs, &events)?.proposals().len(), 1);
        Ok(())
    }
}
