package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Abandon gives up on a proposal, so what did not get in stays visible next to
// what did instead of quietly disappearing.
type Abandon struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run abandons the proposal and returns it in its new state.
func (a Abandon) Run(id string) (review.Proposal, error) {
	proposal, err := a.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Proposal{}, err
	}

	abandoned, err := proposal.Abandoned()
	if err != nil {
		return review.Proposal{}, err
	}

	event, err := review.Abandoned(proposal.ID(), a.Author, a.Now())
	if err != nil {
		return review.Proposal{}, err
	}

	if err := a.Proposals.Abandon(proposal, event); err != nil {
		return review.Proposal{}, err
	}

	return abandoned, nil
}
