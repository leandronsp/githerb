package app_test

import (
	"errors"
	"os/exec"
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/gitstore"
	"github.com/leandronsp/githerb/internal/review"
)

func clock() time.Time { return time.Date(2026, time.August, 21, 18, 4, 5, 0, time.UTC) }

type kit struct {
	repo      gitstore.Repo
	proposals review.Proposals
	git       review.Git
}

func setup(t *testing.T) kit {
	t.Helper()

	dir := t.TempDir()

	run(t, dir, "init", "-q", "-b", "main")
	run(t, dir, "config", "user.email", "test@githerb")
	run(t, dir, "config", "user.name", "test")
	run(t, dir, "commit", "-q", "--allow-empty", "-m", "root")

	repo := gitstore.Open(dir)

	return kit{repo: repo, proposals: gitstore.NewStore(repo), git: repo}
}

func run(t *testing.T, dir string, args ...string) {
	t.Helper()

	cmd := exec.Command("git", args...)
	cmd.Dir = dir

	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v: %s", args, err, out)
	}
}

// work puts a commit on a side branch and leaves main where it was, which is
// the shape every proposal starts from.
func (k kit) work(t *testing.T, branch, message string) {
	t.Helper()

	run(t, k.repo.Dir(), "checkout", "-q", "-B", branch)
	run(t, k.repo.Dir(), "commit", "-q", "--allow-empty", "-m", message)
}

func TestTheWholeLoop(t *testing.T) {
	t.Parallel()

	k := setup(t)
	k.work(t, "gate", "the work")

	propose := app.Propose{Proposals: k.proposals, Git: k.git, Author: "leandro", Now: clock}

	proposal, err := propose.Run("Land the gate", "main", "HEAD")
	if err != nil {
		t.Fatalf("propose: %v", err)
	}

	annotate := app.Annotate{Proposals: k.proposals, Author: "leandro", Now: clock}

	comment, err := annotate.Run(string(proposal.ID()), "cmd/main.go", "new", 3, 5, "this leaks")
	if err != nil {
		t.Fatalf("annotate: %v", err)
	}

	land := app.Land{Proposals: k.proposals, Author: "leandro", Now: clock}

	if _, err := land.Run(string(proposal.ID())); !errors.Is(err, review.ErrOpenComments) {
		t.Fatalf("landed with an open comment: %v", err)
	}

	// The agent reads the annotation, fixes it, and proposes again.
	k.work(t, "gate", "the fix")

	revise := app.Revise{Proposals: k.proposals, Git: k.git}

	revised, err := revise.Run(string(proposal.ID()), "HEAD")
	if err != nil {
		t.Fatalf("revise: %v", err)
	}

	if revised.Head().Number() != 2 {
		t.Fatalf("head is revision %d, want 2", revised.Head().Number())
	}

	resolve := app.Resolve{Proposals: k.proposals, Author: "claude", Now: clock}
	if err := resolve.Run(string(proposal.ID()), string(comment.ID())); err != nil {
		t.Fatalf("resolve: %v", err)
	}

	landed, err := land.Run(string(proposal.ID()))
	if err != nil {
		t.Fatalf("land: %v", err)
	}

	if landed.State() != review.StateLanded {
		t.Fatalf("state is %q, want landed", landed.State())
	}

	tip, err := k.git.HeadOf("main")
	if err != nil {
		t.Fatalf("head: %v", err)
	}

	if tip != landed.Head().SHA() {
		t.Fatalf("main is at %s, want %s", tip, landed.Head().SHA())
	}
}

func TestLandingOntoABranchThatIsNotTheTrunk(t *testing.T) {
	t.Parallel()

	k := setup(t)

	// A stack: a feature branch, and a proposal that lands onto it.
	k.work(t, "feature", "the groundwork")
	run(t, k.repo.Dir(), "checkout", "-q", "-b", "feature-part-two")
	run(t, k.repo.Dir(), "commit", "-q", "--allow-empty", "-m", "the next piece")

	propose := app.Propose{Proposals: k.proposals, Git: k.git, Author: "leandro", Now: clock}

	proposal, err := propose.Run("The next piece", "feature", "HEAD")
	if err != nil {
		t.Fatalf("propose: %v", err)
	}

	if proposal.Target() != "feature" {
		t.Fatalf("target is %q, want feature", proposal.Target())
	}

	land := app.Land{Proposals: k.proposals, Author: "leandro", Now: clock}

	landed, err := land.Run(string(proposal.ID()))
	if err != nil {
		t.Fatalf("land: %v", err)
	}

	tip, err := k.git.HeadOf("feature")
	if err != nil {
		t.Fatalf("head: %v", err)
	}

	if tip != landed.Head().SHA() {
		t.Fatalf("feature is at %s, want %s", tip, landed.Head().SHA())
	}

	trunk, err := k.git.HeadOf("main")
	if err != nil {
		t.Fatalf("main: %v", err)
	}

	if trunk == tip {
		t.Fatal("landing onto feature moved main as well")
	}
}

func TestAProposalIsNamedAfterItsTitle(t *testing.T) {
	t.Parallel()

	k := setup(t)
	k.work(t, "gate", "the work")

	propose := app.Propose{Proposals: k.proposals, Git: k.git, Author: "leandro", Now: clock}

	proposal, err := propose.Run("Land the gate, finally!", "main", "HEAD")
	if err != nil {
		t.Fatalf("propose: %v", err)
	}

	if got := string(proposal.ID()); got[:len("land-the-gate-finally-")] != "land-the-gate-finally-" {
		t.Fatalf("id is %q", got)
	}
}
