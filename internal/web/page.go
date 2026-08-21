package web

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"

	"github.com/leandronsp/githerb/internal/patch"
	"github.com/leandronsp/githerb/internal/review"
)

// Page is everything the review template needs, worked out once.
type Page struct {
	Proposal    review.Proposal
	Since       int
	Added       int
	Removed     int
	Files       []patch.File
	Open        []review.Comment
	Chunks      []review.Chunk
	Rationale   []review.Comment
	Checks      []review.Check
	Blocked     string
	Fingerprint string
}

func newPage(proposal review.Proposal, files []patch.File, required []review.CheckName, since int) Page {
	open := proposal.Open()

	blocked := ""
	if err := proposal.Landable(required...); err != nil {
		blocked = err.Error()
	}

	added, removed := patch.Count(files)

	return Page{
		Proposal:    proposal,
		Since:       since,
		Added:       added,
		Removed:     removed,
		Files:       files,
		Open:        open,
		Chunks:      proposal.Chunks(),
		Rationale:   proposal.Rationale(),
		Checks:      proposal.SortedChecks(),
		Blocked:     blocked,
		Fingerprint: fingerprint(proposal, open),
	}
}

// Row is a proposal on the board with the size of what it carries, so the
// list answers how big a thing is before it is opened.
type Row struct {
	Proposal review.Proposal
	Added    int
	Removed  int
}

// ID is the proposal's name, for the link.
func (r Row) ID() string { return string(r.Proposal.ID()) }

// Title is the proposal's one-line name.
func (r Row) Title() string { return r.Proposal.Title() }

// Target is the branch it lands on.
func (r Row) Target() review.Branch { return r.Proposal.Target() }

// Revision is the number of the head revision.
func (r Row) Revision() int { return r.Proposal.Head().Number() }

// Open is how many notes are still waiting for an answer.
func (r Row) Open() int { return len(r.Proposal.Open()) }

// Board is the dashboard: what is being reviewed, what got in, and what did
// not, which is the answer to the only three questions anyone asks.
type Board struct {
	Open      []Row
	Landed    []Row
	Abandoned []Row
}

func newBoard(rows []Row) Board {
	var board Board

	for _, row := range rows {
		switch row.Proposal.State() {
		case review.StateOpen:
			board.Open = append(board.Open, row)
		case review.StateLanded:
			board.Landed = append(board.Landed, row)
		case review.StateAbandoned:
			board.Abandoned = append(board.Abandoned, row)
		}
	}

	return board
}

// ID is the proposal's name, for building URLs in the template.
func (p Page) ID() string { return string(p.Proposal.ID()) }

// Landable reports whether the button should be live.
func (p Page) Landable() bool { return p.Blocked == "" }

// Missing are the declared checks that have not answered for this revision.
func (p Page) Missing(required []review.CheckName) int {
	return len(required) - len(p.Checks)
}

// Explains are the author's notes that end on a given line, so an explanation
// sits under the code it is about rather than in a list somewhere else.
func (p Page) Explains(file string, side review.Side, line int) []review.Comment {
	var found []review.Comment

	for _, comment := range p.Rationale {
		span := comment.Span()
		if string(comment.File()) == file && span.Side() == side && span.End() == line {
			found = append(found, comment)
		}
	}

	return found
}

// Anchor is the id a chunk points at, so the page can take the reader there.
func (p Page) Anchor(chunk review.Chunk) string {
	if !chunk.Anchored() {
		return ""
	}

	return fmt.Sprintf("L-%s-%s-%d", chunk.File(), chunk.Span().Side(), chunk.Span().Start())
}

// Revised reports whether there is an earlier revision to compare against,
// which is the only question a reviewer coming back actually has.
func (p Page) Revised() bool { return len(p.Proposal.Revisions()) > 1 }

// Previous is the revision before the head.
func (p Page) Previous() int { return p.Proposal.Head().Number() - 1 }

// Noted reports whether a line already carries an open note, so the diff can
// say so where the eye already is rather than only in the panel.
func (p Page) Noted(file string, side review.Side, line int) bool {
	for _, comment := range p.Open {
		span := comment.Span()
		if string(comment.File()) == file && span.Side() == side && line >= span.Start() && line <= span.End() {
			return true
		}
	}

	return false
}

// fingerprint is what the stream compares to decide whether the page moved.
// Git has nothing to subscribe to, so the answer has to be derived.
func fingerprint(proposal review.Proposal, open []review.Comment) string {
	var b strings.Builder

	fmt.Fprintf(&b, "%s|%s|%d|", proposal.State(), proposal.Head().SHA(), proposal.Head().Number())

	for _, comment := range open {
		fmt.Fprintf(&b, "%s,", comment.ID())
	}

	// Sorted, because map order in Go is deliberately random and a fingerprint
	// built from one would flap and push an update on every tick.
	for _, check := range proposal.SortedChecks() {
		fmt.Fprintf(&b, "%s=%s,", check.Name(), check.Status())
	}

	sum := sha256.Sum256([]byte(b.String()))

	return hex.EncodeToString(sum[:8])
}
