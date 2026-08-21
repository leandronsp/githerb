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
