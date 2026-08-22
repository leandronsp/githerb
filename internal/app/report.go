package app

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Report writes down what an agent is doing to a proposal. It is the only way
// anything gets into the work log, and the log is the only thing that says
// whether somebody is already on this.
type Report struct {
	Proposals review.Proposals
	Author    string
	Now       Clock
}

// Run appends one line of work against the head revision.
func (r Report) Run(id, task, phase, note string) (review.Work, error) {
	proposal, err := r.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return review.Work{}, err
	}

	wanted, err := review.ParseTask(task)
	if err != nil {
		return review.Work{}, err
	}

	reached, err := review.ParsePhase(phase)
	if err != nil {
		return review.Work{}, err
	}

	head := proposal.Head().SHA()

	line, err := review.NewWork(head, wanted, reached, r.Author, note, r.Now())
	if err != nil {
		return review.Work{}, err
	}

	if err := r.Proposals.Annotate(head, review.WorkRecord(line)); err != nil {
		return review.Work{}, err
	}

	return line, nil
}
