package web

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sort"
	"strings"

	"github.com/leandronsp/githerb/internal/patch"
	"github.com/leandronsp/githerb/internal/review"
)

// Page is everything the review template needs, worked out once.
type Page struct {
	Proposal    review.Proposal
	Files       []patch.File
	Open        []review.Comment
	Checks      []review.Check
	Blocked     string
	Fingerprint string
}

func newPage(proposal review.Proposal, files []patch.File, required []review.CheckName) Page {
	open := proposal.Open()

	blocked := ""
	if err := proposal.Landable(required...); err != nil {
		blocked = err.Error()
	}

	return Page{
		Proposal:    proposal,
		Files:       files,
		Open:        open,
		Checks:      ordered(proposal.Checks(), required),
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

// ordered puts the declared checks first, in the order the repository declares
// them, so the panel does not shuffle between renders.
func ordered(current map[review.CheckName]review.Check, required []review.CheckName) []review.Check {
	var checks []review.Check

	for _, name := range required {
		if check, ran := current[name]; ran {
			checks = append(checks, check)
		}
	}

	return checks
}

// ID is the proposal's name, for building URLs in the template.
func (p Page) ID() string { return string(p.Proposal.ID()) }

// Landable reports whether the button should be live.
func (p Page) Landable() bool { return p.Blocked == "" }

// Missing are the declared checks that have not answered for this revision.
func (p Page) Missing(required []review.CheckName) int {
	return len(required) - len(p.Checks)
}

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

	// Map order in Go is deliberately random, so a fingerprint built from one
	// would flap and push an update on every tick.
	checks := proposal.Checks()

	names := make([]string, 0, len(checks))
	for name := range checks {
		names = append(names, string(name))
	}

	sort.Strings(names)

	for _, name := range names {
		fmt.Fprintf(&b, "%s=%s,", name, checks[review.CheckName(name)].Status())
	}

	sum := sha256.Sum256([]byte(b.String()))

	return hex.EncodeToString(sum[:8])
}
