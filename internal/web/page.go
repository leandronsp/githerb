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
	Files       []patch.File
	Open        []review.Comment
	Fingerprint string
}

func newPage(proposal review.Proposal, files []patch.File) Page {
	open := proposal.Open()

	return Page{
		Proposal:    proposal,
		Files:       files,
		Open:        open,
		Fingerprint: fingerprint(proposal, open),
	}
}

// ID is the proposal's name, for building URLs in the template.
func (p Page) ID() string { return string(p.Proposal.ID()) }

// Landable reports whether the button should be live.
func (p Page) Landable() bool { return p.Proposal.Landable() == nil }

// CommentsOn are the open comments that point at a given file and line, which
// is how an annotation is drawn next to the line it is about.
func (p Page) CommentsOn(file string, side review.Side, line int) []review.Comment {
	var found []review.Comment

	for _, comment := range p.Open {
		span := comment.Span()
		if string(comment.File()) == file && span.Side() == side && span.End() == line {
			found = append(found, comment)
		}
	}

	return found
}

// fingerprint is what the stream compares to decide whether the page moved.
// Git has nothing to subscribe to, so the answer has to be derived.
func fingerprint(proposal review.Proposal, open []review.Comment) string {
	var b strings.Builder

	fmt.Fprintf(&b, "%s|%s|%d|", proposal.State(), proposal.Head().SHA(), proposal.Head().Number())

	for _, comment := range open {
		fmt.Fprintf(&b, "%s,", comment.ID())
	}

	sum := sha256.Sum256([]byte(b.String()))

	return hex.EncodeToString(sum[:8])
}
