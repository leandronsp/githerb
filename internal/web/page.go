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

	return Page{
		Proposal:    proposal,
		Since:       since,
		Files:       files,
		Open:        open,
		Chunks:      proposal.Chunks(),
		Rationale:   proposal.Rationale(),
		Checks:      proposal.SortedChecks(),
		Blocked:     blocked,
		Fingerprint: fingerprint(proposal, open),
	}
}

// Board is the dashboard: what is being reviewed, what got in, and what did
// not, which is the answer to the only three questions anyone asks.
type Board struct {
	Open      []review.Proposal
	Landed    []review.Proposal
	Abandoned []review.Proposal
}

func newBoard(proposals []review.Proposal) Board {
	var board Board

	for _, proposal := range proposals {
		switch proposal.State() {
		case review.StateOpen:
			board.Open = append(board.Open, proposal)
		case review.StateLanded:
			board.Landed = append(board.Landed, proposal)
		case review.StateAbandoned:
			board.Abandoned = append(board.Abandoned, proposal)
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
