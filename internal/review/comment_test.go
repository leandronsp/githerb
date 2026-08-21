package review_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func TestCommentIDIsDerivedFromContent(t *testing.T) {
	t.Parallel()

	first := comment(t, "same words")
	second := comment(t, "same words")
	other := comment(t, "different words")

	if first.ID() != second.ID() {
		t.Fatalf("the same content produced two ids, %q and %q", first.ID(), second.ID())
	}

	if first.ID() == other.ID() {
		t.Fatalf("different content produced the same id, %q", first.ID())
	}
}

func TestRefusedComments(t *testing.T) {
	t.Parallel()

	single, err := review.NewSpan(review.SideNew, 1, 1)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	cases := []struct {
		name   string
		rev    review.SHA
		file   review.File
		body   string
		author string
		want   error
	}{
		{"no revision", "", file, "x", "leandro", review.ErrNoRevision},
		{"revision is not a sha", "nope", file, "x", "leandro", review.ErrNoRevision},
		{"no file", rev, "", "x", "leandro", review.ErrNoFile},
		{"no body", rev, file, "   ", "leandro", review.ErrNoBody},
		{"no author", rev, file, "x", "", review.ErrNoAuthor},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewComment(tc.rev, tc.file, single, tc.body, tc.author, at(t))
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}
