package review_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func TestRefusedSpans(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		side  review.Side
		start int
		end   int
		want  error
	}{
		{"unknown side", review.Side("sideways"), 1, 1, review.ErrUnknownSide},
		{"line zero", review.SideNew, 0, 1, review.ErrEmptySpan},
		{"negative line", review.SideOld, -3, -1, review.ErrEmptySpan},
		{"end before start", review.SideNew, 9, 4, review.ErrEmptySpan},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewSpan(tc.side, tc.start, tc.end)
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}
