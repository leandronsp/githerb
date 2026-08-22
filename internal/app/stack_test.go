package app_test

import (
	"testing"

	"github.com/leandronsp/githerb/internal/app"
)

// A stack: the second proposal is cut from the first one's head and lands onto
// its branch, which is how work continues while the piece under it is still
// being reviewed.
func TestLandingAProposalRetargetsWhatWasStackedOnIt(t *testing.T) {
	t.Parallel()

	k := setup(t)
	propose := app.Propose{Proposals: k.proposals, Git: k.git, Author: "leandro", Now: clock}

	k.work(t, "one", "the first work")

	first, err := propose.Run("The first piece", "main", "HEAD")
	if err != nil {
		t.Fatalf("propose: %v", err)
	}

	k.work(t, "two", "the second work")

	second, err := propose.Run("The second piece", "one", "HEAD")
	if err != nil {
		t.Fatalf("propose: %v", err)
	}

	if second.Target() != "one" {
		t.Fatalf("the stacked proposal lands onto %q, want one", second.Target())
	}

	land := app.Land{Proposals: k.proposals, Git: k.git, Author: "leandro", Now: clock}

	landed, err := land.Run(string(first.ID()))
	if err != nil {
		t.Fatalf("land: %v", err)
	}

	if len(landed.Followed) != 1 || landed.Followed[0] != second.ID() {
		t.Fatalf("landing followed %v, want the stacked proposal", landed.Followed)
	}

	moved, err := k.proposals.Load(second.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if moved.Target() != "main" {
		t.Fatalf("the stacked proposal still lands onto %q, want main", moved.Target())
	}

	if moved.Base() != first.Head().SHA() {
		t.Fatalf("the base moved to %s, and it had no reason to", moved.Base())
	}
}
