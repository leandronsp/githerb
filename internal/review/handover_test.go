package review_test

import (
	"strings"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func TestAHandoverCarriesEveryOpenNoteAndHowToAnswerIt(t *testing.T) {
	t.Parallel()

	note := comment(t, "this branch is unreachable")

	made, err := proposal(t).WithRecord(review.CommentRecord(note))
	if err != nil {
		t.Fatalf("comment: %v", err)
	}

	brief := made.Handover()

	for _, want := range []string{
		"land-the-gate",
		"internal/app/land.go:42-47",
		"this branch is unreachable",
		"githerb resolve land-the-gate " + string(note.ID()),
		"githerb revise land-the-gate",
	} {
		if !strings.Contains(brief, want) {
			t.Fatalf("the handover is missing %q:\n%s", want, brief)
		}
	}
}

func TestAHandoverWithNothingOpenIsEmpty(t *testing.T) {
	t.Parallel()

	if brief := proposal(t).Handover(); brief != "" {
		t.Fatalf("a proposal with no open notes handed over %q", brief)
	}
}

func TestTheBriefForARunnerNeverAsksForACommandBack(t *testing.T) {
	t.Parallel()

	note := comment(t, "this branch is unreachable")

	made, err := proposal(t).WithRecord(review.CommentRecord(note))
	if err != nil {
		t.Fatalf("comment: %v", err)
	}

	brief := made.Brief()

	if !strings.Contains(brief, "this branch is unreachable") {
		t.Fatalf("the brief lost the note:\n%s", brief)
	}

	// A runner records the revision. An agent told to do it as well records it
	// first, and then the runner is the one that looks like it failed.
	for _, banned := range []string{"githerb revise", "githerb resolve"} {
		if strings.Contains(brief, banned) {
			t.Fatalf("the brief tells the agent to run %q:\n%s", banned, brief)
		}
	}

	if made.Brief() == made.Handover() {
		t.Fatalf("the two briefs are the same text")
	}
}
