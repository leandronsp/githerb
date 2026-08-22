package runner_test

import (
	"testing"
	"time"

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
