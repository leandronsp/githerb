package review_test

import (
	"errors"
	"strings"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func chunk(t *testing.T) review.Chunk {
	t.Helper()

	made, err := review.NewChunk(
		"Checks appear where you already look",
		"cmd/githerb",
		"you had to open the browser to know whether a check ran",
		"list carries a column and show names each one",
		"one column in list, the detail in show",
		"a separate checks command nobody would remember",
	)
	if err != nil {
		t.Fatalf("chunk: %v", err)
	}

	return made
}

func TestAChunkCarriesTheDecision(t *testing.T) {
	t.Parallel()

	made := chunk(t)

	if made.Before() == "" || made.After() == "" || made.Decision() == "" {
		t.Fatalf("chunk is missing its point: %+v", made)
	}

	if made.Anchored() {
		t.Fatal("a chunk points nowhere until it is told to")
	}
}

func TestAChunkCanPointAtTheLinesThatCarryIt(t *testing.T) {
	t.Parallel()

	anchored, err := chunk(t).At("cmd/githerb/commands.go", span(t))
	if err != nil {
		t.Fatalf("anchor: %v", err)
	}

	if !anchored.Anchored() || anchored.File() != "cmd/githerb/commands.go" {
		t.Fatalf("anchor is %v %q", anchored.Anchored(), anchored.File())
	}
}

// The caps are the anti-slop mechanism. An instruction is advice and a
// constructor is a rule, and only one of them survives a different agent.
func TestProlixityIsRefused(t *testing.T) {
	t.Parallel()

	long := strings.Repeat("a", 300)

	cases := []struct {
		name  string
		title string
		what  string
		want  error
	}{
		{"a title that runs on", long, "the call", review.ErrTooLong},
		{"a paragraph where a line goes", "t", long, review.ErrTooLong},
		{"a decision with a newline in it", "t", "one\ntwo", review.ErrNotOneLine},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewChunk(tc.title, "internal", "was", "is", tc.what, "")
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestAChunkWithoutThePointIsRefused(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name     string
		title    string
		before   string
		after    string
		decision string
		want     error
	}{
		{"no title", "", "was", "is", "call", review.ErrNoTitle},
		{"no before", "t", "", "is", "call", review.ErrNoBeforeAfter},
		{"no after", "t", "was", "", "call", review.ErrNoBeforeAfter},
		{"no decision", "t", "was", "is", "", review.ErrNoDecision},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewChunk(tc.title, "internal", tc.before, tc.after, tc.decision, "")
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestTheAlternativeIsOptional(t *testing.T) {
	t.Parallel()

	made, err := review.NewChunk("t", "internal", "was", "is", "the call", "")
	if err != nil {
		t.Fatalf("chunk: %v", err)
	}

	if made.Rejected() != "" {
		t.Fatalf("rejected is %q", made.Rejected())
	}
}
