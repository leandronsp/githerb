package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Dispatch hands the open notes on a proposal to an agent. It writes the
// request down rather than starting anything, because whoever runs the agent
// is a separate process that may not be up yet.
type Dispatch struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run records the request against the head revision.
func (d Dispatch) Run(id string) (review.Proposal, error) {
	proposal, err := d.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Proposal{}, err
	}

	head := proposal.Head().SHA()

	ask, err := review.NewDispatch(head, d.Author, d.Now())
	if err != nil {
		return review.Proposal{}, err
	}

	if err := d.Proposals.Annotate(head, review.DispatchRecord(ask)); err != nil {
		return review.Proposal{}, err
	}

	return proposal, nil
}
