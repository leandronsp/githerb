package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Reply answers a note, from a person or from an agent. It is the other half
// of a review: a note that can only be resolved is a ticket, and a note that
// can be answered is a conversation.
type Reply struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run files the answer under the note it answers.
func (r Reply) Run(id, note, body string) (review.Reply, error) {
	proposal, err := r.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Reply{}, err
	}

	head := proposal.Head().SHA()

	answer, err := review.NewReply(review.ID(note), head, body, r.Author, r.Now())
	if err != nil {
		return review.Reply{}, err
	}

	if err := r.Proposals.Annotate(head, review.ReplyRecord(answer)); err != nil {
		return review.Reply{}, err
	}

	return answer, nil
}
