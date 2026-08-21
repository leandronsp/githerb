package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Land moves the target branch onto the proposal's head revision. It does not
// care which branch that is: landing onto another proposal's branch is how a
// stack gets built before any of it reaches the trunk.
type Land struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run lands the proposal and returns it in its new state.
func (l Land) Run(id string) (review.Proposal, error) {
	proposal, err := l.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Proposal{}, err
	}

	landed, err := proposal.Landed()
	if err != nil {
		return review.Proposal{}, err
	}

	event, err := review.Landed(proposal.ID(), l.Author, l.Now())
	if err != nil {
		return review.Proposal{}, err
	}

	if err := l.Proposals.Land(proposal, event); err != nil {
		return review.Proposal{}, err
	}

	return landed, nil
}
