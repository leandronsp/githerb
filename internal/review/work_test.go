package review_test

import (
	"errors"
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/review"
)

func work(t *testing.T, task review.Task, phase review.Phase, note string, delay time.Duration) review.Work {
	t.Helper()

	made, err := review.NewWork(rev, task, phase, "claude-code", note, at(t).Add(delay))
	if err != nil {
		t.Fatalf("work: %v", err)
	}

	return made
}

func TestWorkIsRefusedWithoutTheThingsThatIdentifyIt(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		task  review.Task
		phase review.Phase
		agent string
		want  error
	}{
		{"no task", "", review.PhaseStarted, "claude-code", review.ErrUnknownTask},
		{"a task nobody runs", "meditate", review.PhaseStarted, "claude-code", review.ErrUnknownTask},
		{"no phase", review.TaskApply, "", "claude-code", review.ErrUnknownPhase},
		{"no agent", review.TaskApply, review.PhaseStarted, "  ", review.ErrNoAuthor},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewWork(rev, tc.task, tc.phase, tc.agent, "", at(t))
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestAProposalKnowsWhetherAnAgentIsOnIt(t *testing.T) {
	t.Parallel()

	started, err := proposal(t).WithRecord(review.WorkRecord(work(t, review.TaskApply, review.PhaseStarted, "", 0)))
	if err != nil {
		t.Fatalf("start: %v", err)
	}

	busy := started.Activity()
	if !busy.Working() || busy.Task() != review.TaskApply || busy.Agent() != "claude-code" {
		t.Fatalf("activity is %+v, want claude-code applying", busy)
	}

	done, err := started.WithRecord(review.WorkRecord(work(t, review.TaskApply, review.PhaseFinished, "applied 2 notes", time.Minute)))
	if err != nil {
		t.Fatalf("finish: %v", err)
	}

	if !done.Activity().Idle() {
		t.Fatalf("activity is %+v, want idle", done.Activity())
	}

	if len(done.Work()) != 2 {
		t.Fatalf("the log kept %d work records, want 2", len(done.Work()))
	}
}

func TestAFailedTaskStaysOnTheProposalWithItsReason(t *testing.T) {
	t.Parallel()

	// Written in the wrong order on purpose: the log is a set once two machines
	// have merged it, so what orders it is the timestamp.
	made := proposal(t)

	for _, record := range []review.Work{
		work(t, review.TaskRebase, review.PhaseFailed, "conflicts in a.txt", time.Minute),
		work(t, review.TaskRebase, review.PhaseStarted, "", 0),
	} {
		next, err := made.WithRecord(review.WorkRecord(record))
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		made = next
	}

	activity := made.Activity()
	if !activity.Failed() || activity.Note() != "conflicts in a.txt" {
		t.Fatalf("activity is %+v, want a failed rebase", activity)
	}
}
