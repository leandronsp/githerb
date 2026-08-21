package review_test

import (
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/review"
)

const (
	rev  = review.SHA("9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b")
	rev2 = review.SHA("1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d")
	base = review.SHA("00112233445566778899aabbccddeeff00112233")
	file = review.File("internal/app/land.go")
)

// at is a fixed clock, because the core takes the time as a parameter and a
// test that depends on the real one is a test that fails at midnight.
func at(t *testing.T) time.Time {
	t.Helper()

	moment, err := time.Parse(time.RFC3339, "2026-08-21T18:04:05Z")
	if err != nil {
		t.Fatalf("fixture clock: %v", err)
	}

	return moment
}

func span(t *testing.T) review.Span {
	t.Helper()

	made, err := review.NewSpan(review.SideNew, 42, 47)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	return made
}

func commentOn(t *testing.T, revision review.SHA, body string) review.Comment {
	t.Helper()

	made, err := review.NewComment(revision, file, span(t), body, "leandro", at(t))
	if err != nil {
		t.Fatalf("comment: %v", err)
	}

	return made
}

func comment(t *testing.T, body string) review.Comment {
	t.Helper()

	return commentOn(t, rev, body)
}
