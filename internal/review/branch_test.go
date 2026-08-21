package review_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func TestBranchesWeAccept(t *testing.T) {
	t.Parallel()

	for _, name := range []string{"main", "trunk", "feature/gate", "release-2.1", "a"} {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			got, err := review.ParseBranch(name)
			if err != nil {
				t.Fatalf("refused %q: %v", name, err)
			}

			if got.Ref() != "refs/heads/"+name {
				t.Fatalf("ref is %q", got.Ref())
			}
		})
	}
}

func TestBranchesWeRefuse(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		raw  string
		want error
	}{
		{"nothing", "   ", review.ErrNoBranch},
		{"reads as a flag", "-force", review.ErrBadBranch},
		{"leading slash", "/main", review.ErrBadBranch},
		{"trailing slash", "main/", review.ErrBadBranch},
		{"double dot", "a..b", review.ErrBadBranch},
		{"empty segment", "a//b", review.ErrBadBranch},
		{"lock suffix", "main.lock", review.ErrBadBranch},
		{"a space", "my branch", review.ErrBadBranch},
		{"ref syntax", "main^", review.ErrBadBranch},
		{"a colon", "a:b", review.ErrBadBranch},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			if _, err := review.ParseBranch(tc.raw); !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}
