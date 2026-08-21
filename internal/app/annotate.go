package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Annotate leaves a comment on a range of lines of a proposal's head revision.
// This is the human half of the loop.
type Annotate struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run appends the comment and returns it.
func (a Annotate) Run(id, file, side string, start, end int, body string) (review.Comment, error) {
	proposal, err := a.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Comment{}, err
	}

	parsed, err := review.ParseSide(side)
	if err != nil {
		return review.Comment{}, err
	}

	span, err := review.NewSpan(parsed, start, end)
	if err != nil {
		return review.Comment{}, err
	}

	head := proposal.Head().SHA()

	comment, err := review.NewComment(head, review.File(file), span, body, a.Author, a.Now())
	if err != nil {
		return review.Comment{}, err
	}

	// Ask the aggregate first: it refuses a comment that does not belong here.
	if _, err := proposal.WithRecord(review.CommentRecord(comment)); err != nil {
		return review.Comment{}, err
	}

	if err := a.Proposals.Annotate(head, review.CommentRecord(comment)); err != nil {
		return review.Comment{}, err
	}

	return comment, nil
}

// Resolve marks a comment as dealt with. This is the agent half of the loop.
type Resolve struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run appends the resolution.
func (r Resolve) Run(id, comment string) error {
	proposal, err := r.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return err
	}

	resolution, err := review.NewResolution(review.ID(comment), r.Author, r.Now())
	if err != nil {
		return err
	}

	if _, err := proposal.WithRecord(review.ResolutionRecord(resolution)); err != nil {
		return err
	}

	return r.Proposals.Annotate(proposal.Head().SHA(), review.ResolutionRecord(resolution))
}
