package gitstore

import (
	"errors"
	"fmt"
	"path"
	"sort"
	"strconv"
	"strings"

	"github.com/leandronsp/githerb/internal/review"
)

// Where a proposal keeps its parts. Refs for the revisions, one note ref for
// what happened to the proposal, one for what people said about it.
const (
	proposalRefs   = "refs/githerb/proposals"
	eventNotes     = "githerb/proposals"
	recordNotes    = "githerb/annotations"
	notesRefPrefix = "refs/notes/"
)

// Store keeps proposals in a repository.
type Store struct {
	repo Repo
}

// NewStore points a store at a repository.
func NewStore(repo Repo) Store { return Store{repo: repo} }

// Open records a new proposal and its first revision.
func (s Store) Open(proposal review.Proposal, event review.Event) error {
	head := proposal.Head()

	if err := s.writeRevision(proposal.ID(), head); err != nil {
		return err
	}

	return s.appendEvent(head.SHA(), event)
}

// Revise records another attempt at an open proposal.
func (s Store) Revise(id review.ProposalID, revision review.Revision) error {
	return s.writeRevision(id, revision)
}

// Annotate appends one record to the log of a revision.
func (s Store) Annotate(revision review.SHA, record review.Record) error {
	line, err := marshalRecord(record)
	if err != nil {
		return err
	}

	return s.appendNote(recordNotes, revision, line)
}

// Land moves the target branch onto the proposal's head and records it.
func (s Store) Land(proposal review.Proposal, event review.Event) error {
	head := proposal.Head().SHA()
	target := proposal.Target()

	current, err := s.repo.HeadOf(target)
	if err != nil {
		return err
	}

	// Refuse anything but a fast-forward. A proposal that has fallen behind
	// its target is a proposal whose review looked at the wrong code.
	if _, err := s.repo.run("merge-base", "--is-ancestor", string(current), string(head)); err != nil {
		return fmt.Errorf("%s moved since the proposal was cut: %w", target, ErrGit)
	}

	if _, err := s.repo.run("update-ref", target.Ref(), string(head), string(current)); err != nil {
		return err
	}

	return s.appendEvent(s.firstRevision(proposal), event)
}

// Retarget records that the proposal lands on another branch now.
func (s Store) Retarget(proposal review.Proposal, event review.Event) error {
	return s.appendEvent(s.firstRevision(proposal), event)
}

// Abandon records that a proposal will not be landing.
func (s Store) Abandon(proposal review.Proposal, event review.Event) error {
	return s.appendEvent(s.firstRevision(proposal), event)
}

// Load rebuilds a proposal from everything written about it.
func (s Store) Load(id review.ProposalID) (review.Proposal, error) {
	revisions, err := s.revisionsOf(id)
	if err != nil {
		return review.Proposal{}, err
	}

	if len(revisions) == 0 {
		return review.Proposal{}, fmt.Errorf("proposal %q: %w", id, ErrNotFound)
	}

	events, err := s.eventsOn(revisions[0].SHA())
	if err != nil {
		return review.Proposal{}, err
	}

	proposal, err := open(id, revisions[0], events)
	if err != nil {
		return review.Proposal{}, err
	}

	proposal, err = retargeted(proposal, events)
	if err != nil {
		return review.Proposal{}, err
	}

	for _, revision := range revisions[1:] {
		next, err := proposal.WithRevision(revision.SHA())
		if err != nil {
			return review.Proposal{}, fmt.Errorf("revision %d: %w", revision.Number(), err)
		}

		proposal = next
	}

	proposal, err = s.foldRecords(proposal, revisions)
	if err != nil {
		return review.Proposal{}, err
	}

	return landIfEnded(proposal, events)
}

// List rebuilds every proposal, newest first.
func (s Store) List() ([]review.Proposal, error) {
	names, err := s.names()
	if err != nil {
		return nil, err
	}

	proposals := make([]review.Proposal, 0, len(names))

	for _, name := range names {
		proposal, err := s.Load(name)
		if err != nil {
			return nil, err
		}

		proposals = append(proposals, proposal)
	}

	return proposals, nil
}

func open(id review.ProposalID, first review.Revision, events []review.Event) (review.Proposal, error) {
	for _, event := range events {
		if event.Kind() == review.EventOpened {
			return review.NewProposal(id, event.Title(), event.Target(), event.Base(), first.SHA())
		}
	}

	return review.Proposal{}, fmt.Errorf("proposal %q has no opening event: %w", id, ErrNotFound)
}

// retargeted replays the moves in the order they happened. The log is a set
// once two machines have merged it, so the timestamp is what orders it, and a
// clock that lies is the limit of that.
func retargeted(proposal review.Proposal, events []review.Event) (review.Proposal, error) {
	moves := make([]review.Event, 0, len(events))

	for _, event := range events {
		if event.Kind() == review.EventRetargeted {
			moves = append(moves, event)
		}
	}

	sort.SliceStable(moves, func(i, j int) bool { return moves[i].At().Before(moves[j].At()) })

	for _, move := range moves {
		next, err := proposal.Retargeted(move.Target())
		if err != nil {
			return review.Proposal{}, fmt.Errorf("proposal %q: %w", proposal.ID(), err)
		}

		proposal = next
	}

	return proposal, nil
}

func landIfEnded(proposal review.Proposal, events []review.Event) (review.Proposal, error) {
	for _, event := range events {
		var (
			ended review.Proposal
			err   error
		)

		switch event.Kind() {
		case review.EventLanded:
			// The gate was answered when it landed. Reading it back is not the
			// moment to ask again.
			ended, err = proposal.Landed(nil...)
		case review.EventAbandoned:
			ended, err = proposal.Abandoned()
		case review.EventOpened, review.EventRetargeted:
			continue
		default:
			continue
		}

		if err != nil {
			return review.Proposal{}, fmt.Errorf("proposal %q: %w", proposal.ID(), err)
		}

		return ended, nil
	}

	return proposal, nil
}

func (s Store) foldRecords(proposal review.Proposal, revisions []review.Revision) (review.Proposal, error) {
	// Comments before resolutions, because a resolution needs the comment it
	// answers to be there already and the note is only sorted, not ordered.
	var comments, resolutions []review.Record

	for _, revision := range revisions {
		records, err := s.recordsOn(revision.SHA())
		if err != nil {
			return review.Proposal{}, err
		}

		for _, record := range records {
			if record.Kind() == review.KindResolve {
				resolutions = append(resolutions, record)
			} else {
				comments = append(comments, record)
			}
		}
	}

	for _, record := range append(comments, resolutions...) {
		next, err := proposal.WithRecord(record)
		if err != nil {
			return review.Proposal{}, err
		}

		proposal = next
	}

	return proposal, nil
}

func (s Store) firstRevision(proposal review.Proposal) review.SHA {
	return proposal.Revisions()[0].SHA()
}

func (s Store) writeRevision(id review.ProposalID, revision review.Revision) error {
	ref := path.Join(proposalRefs, string(id), strconv.Itoa(revision.Number()))

	_, err := s.repo.run("update-ref", ref, string(revision.SHA()))

	return err
}

func (s Store) revisionsOf(id review.ProposalID) ([]review.Revision, error) {
	prefix := path.Join(proposalRefs, string(id))

	out, err := s.repo.run("for-each-ref", "--format=%(refname) %(objectname)", prefix)
	if err != nil {
		return nil, err
	}

	var revisions []review.Revision

	for _, line := range lines(out) {
		name, sha, found := strings.Cut(line, " ")
		if !found {
			continue
		}

		number, err := strconv.Atoi(path.Base(name))
		if err != nil {
			return nil, fmt.Errorf("ref %q is not a revision: %w", name, ErrNotFound)
		}

		revisions = append(revisions, review.NewRevision(number, review.SHA(sha)))
	}

	sort.Slice(revisions, func(i, j int) bool {
		return revisions[i].Number() < revisions[j].Number()
	})

	return revisions, nil
}

func (s Store) names() ([]review.ProposalID, error) {
	out, err := s.repo.run("for-each-ref", "--format=%(refname)", proposalRefs)
	if err != nil {
		return nil, err
	}

	seen := map[review.ProposalID]bool{}

	var names []review.ProposalID

	for _, line := range lines(out) {
		name := review.ProposalID(path.Base(path.Dir(line)))
		if name != "" && !seen[name] {
			seen[name] = true

			names = append(names, name)
		}
	}

	return names, nil
}

func (s Store) appendEvent(revision review.SHA, event review.Event) error {
	line, err := event.MarshalLine()
	if err != nil {
		return fmt.Errorf("rendering an event: %w", err)
	}

	return s.appendNote(eventNotes, revision, string(line))
}

func (s Store) appendNote(ref string, revision review.SHA, line string) error {
	_, err := s.repo.run("notes", "--ref="+ref, "append", "--no-separator", "-m", line, string(revision))

	return err
}

func (s Store) readNote(ref string, revision review.SHA) ([]string, error) {
	out, err := s.repo.run("notes", "--ref="+ref, "show", string(revision))
	if err != nil {
		// No note is not a failure. A revision nobody commented on is normal.
		if errors.Is(err, ErrGit) {
			return nil, nil
		}

		return nil, err
	}

	return lines(out), nil
}

func (s Store) eventsOn(revision review.SHA) ([]review.Event, error) {
	raw, err := s.readNote(eventNotes, revision)
	if err != nil {
		return nil, err
	}

	events := make([]review.Event, 0, len(raw))

	for _, line := range raw {
		event, err := review.ParseEvent([]byte(line))

		// A kind this build has never heard of came from a newer one. Skipping
		// it is what makes the format extensible: the alternative is that the
		// day anyone writes a new kind, every older binary stops opening the
		// proposal entirely.
		if errors.Is(err, review.ErrUnknownKind) {
			continue
		}

		if err != nil {
			return nil, fmt.Errorf("proposal log: %w", err)
		}

		events = append(events, event)
	}

	return events, nil
}

func (s Store) recordsOn(revision review.SHA) ([]review.Record, error) {
	raw, err := s.readNote(recordNotes, revision)
	if err != nil {
		return nil, err
	}

	records := make([]review.Record, 0, len(raw))

	for _, line := range raw {
		record, err := review.ParseLine([]byte(line))

		// Same as the event log: an unknown kind is a newer binary talking, not
		// a broken repository.
		if errors.Is(err, review.ErrUnknownKind) {
			continue
		}

		if err != nil {
			return nil, fmt.Errorf("annotation log: %w", err)
		}

		records = append(records, record)
	}

	return records, nil
}

func marshalRecord(record review.Record) (string, error) {
	switch record.Kind() {
	case review.KindComment:
		comment, _ := record.Comment()

		line, err := comment.MarshalLine()

		return string(line), wrap(err)
	case review.KindResolve:
		resolution, _ := record.Resolution()

		line, err := resolution.MarshalLine()

		return string(line), wrap(err)
	case review.KindCheck:
		check, _ := record.Check()

		line, err := check.MarshalLine()

		return string(line), wrap(err)
	case review.KindChunk:
		chunk, _ := record.Chunk()

		line, err := chunk.MarshalLine()

		return string(line), wrap(err)
	case review.KindRationale:
		comment, _ := record.Comment()

		line, err := comment.MarshalLine()

		return strings.Replace(string(line), `"kind":"comment"`, `"kind":"rationale"`, 1), wrap(err)
	case review.KindWork:
		work, _ := record.Work()

		line, err := work.MarshalLine()

		return string(line), wrap(err)
	default:
		return "", fmt.Errorf("%q: %w", record.Kind(), review.ErrUnknownKind)
	}
}

func wrap(err error) error {
	if err == nil {
		return nil
	}

	return fmt.Errorf("rendering a record: %w", err)
}

func lines(out string) []string {
	var kept []string

	for _, line := range strings.Split(out, "\n") {
		if trimmed := strings.TrimSpace(line); trimmed != "" {
			kept = append(kept, trimmed)
		}
	}

	return kept
}
