package app

import (
	"fmt"

	"github.com/leandronsp/githerb/internal/review"
)

// Revise adds another attempt to an open proposal, which is what an agent does
// after reading the annotations on the last one.
type Revise struct {
	Proposals review.Proposals
	Git       review.Git
}

// Run records the new revision and returns the proposal it belongs to.
func (r Revise) Run(id, head string) (review.Proposal, error) {
	proposal, err := r.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Proposal{}, err
	}

	sha, err := r.Git.Resolve(head)
	if err != nil {
		return review.Proposal{}, fmt.Errorf("revision %s: %w", head, err)
	}

	next, err := proposal.WithRevision(sha)
	if err != nil {
		return review.Proposal{}, err
	}

	if err := r.Proposals.Revise(next.ID(), next.Head()); err != nil {
		return review.Proposal{}, err
	}

	return next, nil
}
