package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Land moves the target branch onto the proposal's head revision. It does not
// care which branch that is: landing onto another proposal's branch is how a
// stack gets built before any of it reaches the trunk.
type Land struct {
	Proposals review.Proposals
	Git       review.Git
	Required  []review.CheckName
	Author    string
	Now       Clock
}

// Landing is what happened: the proposal in its new state, and whatever was
// stacked on it and had to follow.
type Landing struct {
	Proposal review.Proposal
	Followed []review.ProposalID
}

// Run lands the proposal and moves whatever was aimed at it.
func (l Land) Run(id string) (Landing, error) {
	proposal, err := l.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return Landing{}, err
	}

	landed, err := proposal.Landed(l.Required...)
	if err != nil {
		return Landing{}, err
	}

	event, err := review.Landed(proposal.ID(), l.Author, l.Now())
	if err != nil {
		return Landing{}, err
	}

	if err := l.Proposals.Land(proposal, event); err != nil {
		return Landing{}, err
	}

	followed, err := l.follow(landed)
	if err != nil {
		return Landing{Proposal: landed, Followed: nil}, err
	}

	return Landing{Proposal: landed, Followed: followed}, nil
}

// follow moves the proposals that were stacked on this one. A proposal is
// stacked on it when the branch it lands on is sitting exactly on this head,
// which after a fast-forward land is also where the target now is, so the
// commits underneath them never move and nothing has to be rebased.
func (l Land) follow(landed review.Proposal) ([]review.ProposalID, error) {
	open, err := l.Proposals.List()
	if err != nil {
		return nil, err
	}

	head := landed.Head().SHA()

	var moved []review.ProposalID

	for _, proposal := range open {
		if proposal.State() != review.StateOpen || proposal.Target() == landed.Target() {
			continue
		}

		tip, err := l.Git.HeadOf(proposal.Target())
		if err != nil || tip != head {
			continue
		}

		if err := l.retarget(proposal, landed.Target()); err != nil {
			return moved, err
		}

		moved = append(moved, proposal.ID())
	}

	return moved, nil
}

func (l Land) retarget(proposal review.Proposal, target review.Branch) error {
	event, err := review.Retargeted(proposal.ID(), target, l.Author, l.Now())
	if err != nil {
		return err
	}

	return l.Proposals.Retarget(proposal, event)
}
