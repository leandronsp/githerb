package runner

import (
	"github.com/leandronsp/githerb/internal/review"
)

// Job is one thing to do to one proposal. At most one is pending per proposal,
// because two agents on the same branch is how work gets lost.
type Job struct {
	ID   review.ProposalID
	Task review.Task
	Why  string
}

// noJob is the absence of one. Naming it says the omission is deliberate, the
// way the domain names the halves of a record it is not carrying.
var noJob = Job{ID: "", Task: "", Why: ""}

// Pending derives the work from what the log says. Stale names the proposals
// whose target ran ahead of them, which is the one thing the records cannot
// answer on their own.
func Pending(proposals []review.Proposal, stale map[review.ProposalID]bool, required []review.CheckName) []Job {
	jobs := make([]Job, 0, len(proposals))

	for _, proposal := range proposals {
		job, found := next(proposal, stale[proposal.ID()], required)
		if found {
			jobs = append(jobs, job)
		}
	}

	return jobs
}

func next(proposal review.Proposal, stale bool, required []review.CheckName) (Job, bool) {
	activity := proposal.Activity()

	if proposal.State() != review.StateOpen || activity.Working() {
		return noJob, false
	}

	// A handover is the trigger, and the only thing that clears a failure. An
	// agent that speaks without being asked is an agent nobody wants, and a
	// loop that retries what already failed burns tokens all night.
	if proposal.Dispatched() {
		if stale && len(proposal.Open()) == 0 {
			return Job{ID: proposal.ID(), Task: review.TaskRebase, Why: "handed over, and behind"}, true
		}

		return Job{ID: proposal.ID(), Task: review.TaskApply, Why: "notes were handed over"}, true
	}

	if activity.Failed() {
		return noJob, false
	}

	switch {
	case stale:
		// Untriggered, so this one is mechanical: git can replay it or nobody
		// touches it. The agent is not called for a conflict nobody asked it
		// to look at.
		return Job{ID: proposal.ID(), Task: review.TaskRebase, Why: "the target ran ahead"}, true
	case missing(proposal, required):
		return Job{ID: proposal.ID(), Task: review.TaskCheck, Why: "the head has not been checked"}, true
	default:
		return noJob, false
	}
}

func missing(proposal review.Proposal, required []review.CheckName) bool {
	answered := proposal.Checks()

	for _, name := range required {
		if _, ran := answered[name]; !ran {
			return true
		}
	}

	return false
}
