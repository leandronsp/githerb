package review_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func proposal(t *testing.T) review.Proposal {
	t.Helper()

	made, err := review.NewProposal("land-the-gate", "Land the gate", "main", base, rev)
	if err != nil {
		t.Fatalf("proposal: %v", err)
	}

	return made
}

func TestAProposalStartsOpenAtRevisionOne(t *testing.T) {
	t.Parallel()

	made := proposal(t)

	if made.Target() != "main" {
		t.Fatalf("target is %q, want main", made.Target())
	}

	if made.State() != review.StateOpen {
		t.Fatalf("state is %q, want open", made.State())
	}

	if got := made.Head(); got.Number() != 1 || got.SHA() != rev {
		t.Fatalf("head is %d/%s, want 1/%s", got.Number(), got.SHA(), rev)
	}
}

func TestRefusedProposals(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name   string
		id     review.ProposalID
		title  string
		target review.Branch
		base   review.SHA
		head   review.SHA
		want   error
	}{
		{"no id", "", "t", "main", base, rev, review.ErrNoProposalID},
		{"no title", "x", "  ", "main", base, rev, review.ErrNoTitle},
		{"no target", "x", "t", "  ", base, rev, review.ErrNoBranch},
		{"base is not a sha", "x", "t", "main", "nope", rev, review.ErrNoRevision},
		{"head is not a sha", "x", "t", "main", base, "nope", review.ErrNoRevision},
		{"head is the base", "x", "t", "main", base, base, review.ErrNothingProposed},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewProposal(tc.id, tc.title, tc.target, tc.base, tc.head)
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestARevisionIsAppendedAndNumbered(t *testing.T) {
	t.Parallel()

	first := proposal(t)

	second, err := first.WithRevision(rev2)
	if err != nil {
		t.Fatalf("revision: %v", err)
	}

	if got := second.Head(); got.Number() != 2 || got.SHA() != rev2 {
		t.Fatalf("head is %d/%s, want 2/%s", got.Number(), got.SHA(), rev2)
	}

	if len(first.Revisions()) != 1 {
		t.Fatalf("the original proposal grew to %d revisions", len(first.Revisions()))
	}
}

func TestARevisionAlreadyThereIsRefused(t *testing.T) {
	t.Parallel()

	if _, err := proposal(t).WithRevision(rev); !errors.Is(err, review.ErrRevisionKnown) {
		t.Fatalf("got %v, want %v", err, review.ErrRevisionKnown)
	}
}

func TestACommentMustLandOnARevisionOfThisProposal(t *testing.T) {
	t.Parallel()

	stranger := review.CommentRecord(commentOn(t, rev2, "on a revision we never saw"))

	if _, err := proposal(t).WithRecord(stranger); !errors.Is(err, review.ErrUnknownRevision) {
		t.Fatalf("got %v, want %v", err, review.ErrUnknownRevision)
	}
}

func TestAResolutionMustNameAKnownComment(t *testing.T) {
	t.Parallel()

	orphan, err := review.NewResolution("deadbeef", "claude", at(t))
	if err != nil {
		t.Fatalf("resolution: %v", err)
	}

	_, err = proposal(t).WithRecord(review.ResolutionRecord(orphan))
	if !errors.Is(err, review.ErrUnknownComment) {
		t.Fatalf("got %v, want %v", err, review.ErrUnknownComment)
	}
}

func TestTheSameRecordTwiceChangesNothing(t *testing.T) {
	t.Parallel()

	record := review.CommentRecord(comment(t, "rename this"))

	once, err := proposal(t).WithRecord(record)
	if err != nil {
		t.Fatalf("first: %v", err)
	}

	twice, err := once.WithRecord(record)
	if err != nil {
		t.Fatalf("second: %v", err)
	}

	if len(twice.Open()) != 1 {
		t.Fatalf("the log delivered the same record twice and it counted %d", len(twice.Open()))
	}
}

func TestOpenCommentsAreTheUnresolvedOnesOnTheHead(t *testing.T) {
	t.Parallel()

	made := proposal(t)

	addressed := comment(t, "rename this")
	pending := comment(t, "this leaks")

	for _, record := range []review.Record{
		review.CommentRecord(addressed),
		review.CommentRecord(pending),
	} {
		next, err := made.WithRecord(record)
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		made = next
	}

	if len(made.Open()) != 2 {
		t.Fatalf("open is %d, want 2", len(made.Open()))
	}

	resolution, err := review.NewResolution(addressed.ID(), "claude", at(t))
	if err != nil {
		t.Fatalf("resolution: %v", err)
	}

	made, err = made.WithRecord(review.ResolutionRecord(resolution))
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}

	open := made.Open()
	if len(open) != 1 || open[0].ID() != pending.ID() {
		t.Fatalf("open is %v, want only the pending one", open)
	}
}

func TestACommentOnAnOlderRevisionDoesNotBlockTheHead(t *testing.T) {
	t.Parallel()

	made, err := proposal(t).WithRecord(review.CommentRecord(comment(t, "on revision one")))
	if err != nil {
		t.Fatalf("record: %v", err)
	}

	made, err = made.WithRevision(rev2)
	if err != nil {
		t.Fatalf("revision: %v", err)
	}

	if len(made.Open()) != 0 {
		t.Fatalf("a new revision left %d open comments behind", len(made.Open()))
	}

	if err := made.Landable(); err != nil {
		t.Fatalf("landable: %v", err)
	}
}

func TestLanding(t *testing.T) {
	t.Parallel()

	t.Run("is refused while the head has open comments", func(t *testing.T) {
		t.Parallel()

		made, err := proposal(t).WithRecord(review.CommentRecord(comment(t, "not yet")))
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		if err := made.Landable(); !errors.Is(err, review.ErrOpenComments) {
			t.Fatalf("got %v, want %v", err, review.ErrOpenComments)
		}

		if _, err := made.Landed(); !errors.Is(err, review.ErrOpenComments) {
			t.Fatalf("got %v, want %v", err, review.ErrOpenComments)
		}
	})

	t.Run("moves a clean proposal to landed", func(t *testing.T) {
		t.Parallel()

		landed, err := proposal(t).Landed()
		if err != nil {
			t.Fatalf("land: %v", err)
		}

		if landed.State() != review.StateLanded {
			t.Fatalf("state is %q, want landed", landed.State())
		}
	})

	t.Run("happens once", func(t *testing.T) {
		t.Parallel()

		landed, err := proposal(t).Landed()
		if err != nil {
			t.Fatalf("land: %v", err)
		}

		if _, err := landed.Landed(); !errors.Is(err, review.ErrNotOpen) {
			t.Fatalf("got %v, want %v", err, review.ErrNotOpen)
		}
	})

	t.Run("a landed proposal takes no more records", func(t *testing.T) {
		t.Parallel()

		landed, err := proposal(t).Landed()
		if err != nil {
			t.Fatalf("land: %v", err)
		}

		_, err = landed.WithRecord(review.CommentRecord(comment(t, "too late")))
		if !errors.Is(err, review.ErrNotOpen) {
			t.Fatalf("got %v, want %v", err, review.ErrNotOpen)
		}
	})
}
