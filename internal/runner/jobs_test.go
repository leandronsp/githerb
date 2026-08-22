package runner_test

import (
	"context"
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/review"
	"github.com/leandronsp/githerb/internal/runner"
)

const (
	base = review.SHA("00112233445566778899aabbccddeeff00112233")
	head = review.SHA("9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b")
)

func at(minutes int) time.Time {
	return time.Date(2026, time.August, 22, 9, minutes, 0, 0, time.UTC)
}

func proposal(t *testing.T, records ...review.Record) review.Proposal {
	t.Helper()

	made, err := review.NewProposal("p", "A proposal", "main", base, head)
	if err != nil {
		t.Fatalf("proposal: %v", err)
	}

	for _, record := range records {
		next, err := made.WithRecord(record)
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		made = next
	}

	return made
}

func dispatched(t *testing.T, minutes int) review.Record {
	t.Helper()

	ask, err := review.NewDispatch(head, "leandro", at(minutes))
	if err != nil {
		t.Fatalf("dispatch: %v", err)
	}

	return review.DispatchRecord(ask)
}

func worked(t *testing.T, task review.Task, phase review.Phase, minutes int) review.Record {
	t.Helper()

	line, err := review.NewWork(head, task, phase, "claude-code", "", at(minutes))
	if err != nil {
		t.Fatalf("work: %v", err)
	}

	return review.WorkRecord(line)
}

func TestWhatThePendingWorkIs(t *testing.T) {
	t.Parallel()

	gate := []review.CheckName{"gate"}

	cases := []struct {
		name     string
		proposal review.Proposal
		stale    bool
		want     review.Task
	}{
		{"handed over", proposal(t, dispatched(t, 1)), false, review.TaskApply},
		{"the target ran ahead", proposal(t), true, review.TaskRebase},
		{"never checked", proposal(t), false, review.TaskCheck},
		{"handed over and stale, notes first", proposal(t, dispatched(t, 1)), true, review.TaskApply},
		{"already picked up", proposal(t, dispatched(t, 1), worked(t, review.TaskApply, review.PhaseStarted, 2)), true, ""},
		{"gave up on this revision", proposal(t, worked(t, review.TaskCheck, review.PhaseFailed, 2)), true, ""},
		{
			"handed over again after a failure",
			proposal(t, dispatched(t, 1), worked(t, review.TaskApply, review.PhaseFailed, 2), dispatched(t, 3)),
			false, review.TaskApply,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			stale := map[review.ProposalID]bool{"p": tc.stale}
			jobs := runner.Pending([]review.Proposal{tc.proposal}, stale, gate)

			if tc.want == "" {
				if len(jobs) != 0 {
					t.Fatalf("there is nothing to do and it found %+v", jobs)
				}

				return
			}

			if len(jobs) != 1 || jobs[0].Task != tc.want {
				t.Fatalf("found %+v, want one %s", jobs, tc.want)
			}
		})
	}
}

func TestALongNoteIsCutRatherThanRefused(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "a.txt", "one\nTWO\n")
	git(t, k.dir, "commit", "-qam", "the work")

	proposal := k.propose(t, "The work", "main")

	annotate := app.Annotate{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := annotate.Run(string(proposal.ID()), "a.txt", "new", 2, 2, "name it"); err != nil {
		t.Fatalf("annotate: %v", err)
	}

	dispatch := app.Dispatch{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := dispatch.Run(string(proposal.ID())); err != nil {
		t.Fatalf("dispatch: %v", err)
	}

	// An agent that ends on a paragraph. The record has a ceiling and the job
	// still has to come out finished.
	chatty := "printf 'x%.0s' $(seq 1 400); echo; printf 'one\\nNAMED\\n' > a.txt && git add -A && git commit -qm answered"

	if _, err := k.loop(chatty).Once(context.Background()); err != nil {
		t.Fatalf("once: %v", err)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if !after.Activity().Idle() {
		t.Fatalf("the job reads as %+v, want finished", after.Activity())
	}
}
