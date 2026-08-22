package gitstore_test

import (
	"errors"
	"os/exec"
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/gitstore"
	"github.com/leandronsp/githerb/internal/review"
)

// A real repository, a real git, real refs and real notes. A fake git would
// prove nothing about git.
func repo(t *testing.T) (gitstore.Repo, gitstore.Store) {
	t.Helper()

	dir := t.TempDir()

	for _, args := range [][]string{
		{"init", "-q", "-b", "main"},
		{"config", "user.email", "test@githerb"},
		{"config", "user.name", "test"},
		{"commit", "-q", "--allow-empty", "-m", "root"},
	} {
		cmd := exec.Command("git", args...)
		cmd.Dir = dir

		if out, err := cmd.CombinedOutput(); err != nil {
			t.Fatalf("git %v: %v: %s", args, err, out)
		}
	}

	opened := gitstore.Open(dir)

	return opened, gitstore.NewStore(opened)
}

func commit(t *testing.T, r gitstore.Repo, message string) review.SHA {
	t.Helper()

	cmd := exec.Command("git", "commit", "-q", "--allow-empty", "-m", message)
	cmd.Dir = r.Dir()

	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("commit: %v: %s", err, out)
	}

	sha, err := r.Resolve("HEAD")
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}

	return sha
}

func clock() time.Time {
	return time.Date(2026, time.August, 21, 18, 4, 5, 0, time.UTC)
}

func openProposal(t *testing.T, store gitstore.Store, base, head review.SHA) review.Proposal {
	t.Helper()

	proposal, err := review.NewProposal("gate", "Land the gate", "main", base, head)
	if err != nil {
		t.Fatalf("proposal: %v", err)
	}

	event, err := review.Opened(proposal.ID(), proposal.Title(), proposal.Target(), base, "leandro", clock())
	if err != nil {
		t.Fatalf("event: %v", err)
	}

	if err := store.Open(proposal, event); err != nil {
		t.Fatalf("open: %v", err)
	}

	return proposal
}

func TestAProposalSurvivesARoundTripThroughGit(t *testing.T) {
	t.Parallel()

	r, store := repo(t)

	base, err := r.Resolve("HEAD")
	if err != nil {
		t.Fatalf("base: %v", err)
	}

	head := commit(t, r, "the work")
	openProposal(t, store, base, head)

	got, err := store.Load("gate")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if got.Title() != "Land the gate" || got.Target() != "main" || got.Base() != base {
		t.Fatalf("loaded %q onto %q from %q", got.Title(), got.Target(), got.Base())
	}

	if got.Head().Number() != 1 || got.Head().SHA() != head {
		t.Fatalf("head is %d/%s, want 1/%s", got.Head().Number(), got.Head().SHA(), head)
	}
}

func TestARevisionAndItsAnnotationsComeBack(t *testing.T) {
	t.Parallel()

	r, store := repo(t)
	base, _ := r.Resolve("HEAD")
	head := commit(t, r, "the work")
	openProposal(t, store, base, head)

	span, err := review.NewSpan(review.SideNew, 3, 5)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	comment, err := review.NewComment(head, "cmd/main.go", span, "this leaks", "leandro", clock())
	if err != nil {
		t.Fatalf("comment: %v", err)
	}

	if err := store.Annotate(head, review.CommentRecord(comment)); err != nil {
		t.Fatalf("annotate: %v", err)
	}

	loaded, err := store.Load("gate")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	open := loaded.Open()
	if len(open) != 1 || open[0].Body() != "this leaks" {
		t.Fatalf("open comments are %v", open)
	}

	second := commit(t, r, "the fix")
	if err := store.Revise("gate", review.NewRevision(2, second)); err != nil {
		t.Fatalf("revise: %v", err)
	}

	loaded, err = store.Load("gate")
	if err != nil {
		t.Fatalf("reload: %v", err)
	}

	if loaded.Head().Number() != 2 {
		t.Fatalf("head is revision %d, want 2", loaded.Head().Number())
	}

	if len(loaded.Open()) != 0 {
		t.Fatalf("a new revision left %d comments blocking", len(loaded.Open()))
	}
}

func TestARepeatedAnnotationIsStillOneAnnotation(t *testing.T) {
	t.Parallel()

	r, store := repo(t)
	base, _ := r.Resolve("HEAD")
	head := commit(t, r, "the work")
	openProposal(t, store, base, head)

	span, _ := review.NewSpan(review.SideNew, 1, 1)
	comment, _ := review.NewComment(head, "a.go", span, "same words", "leandro", clock())

	for range 3 {
		if err := store.Annotate(head, review.CommentRecord(comment)); err != nil {
			t.Fatalf("annotate: %v", err)
		}
	}

	loaded, err := store.Load("gate")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if len(loaded.Open()) != 1 {
		t.Fatalf("the same annotation three times counted %d", len(loaded.Open()))
	}
}

func TestLandingMovesTheBranchAndIsRemembered(t *testing.T) {
	t.Parallel()

	r, store := repo(t)
	base, _ := r.Resolve("HEAD")
	head := commit(t, r, "the work")

	// The work is on the branch tip here, so put main back where it was cut
	// from, which is what a proposal looks like before it lands.
	reset(t, r, base)

	proposal := openProposal(t, store, base, head)

	landed, err := review.Landed(proposal.ID(), "leandro", clock())
	if err != nil {
		t.Fatalf("event: %v", err)
	}

	if err := store.Land(proposal, landed); err != nil {
		t.Fatalf("land: %v", err)
	}

	moved, err := r.HeadOf("main")
	if err != nil {
		t.Fatalf("head: %v", err)
	}

	if moved != head {
		t.Fatalf("main is at %s, want %s", moved, head)
	}

	loaded, err := store.Load("gate")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if loaded.State() != review.StateLanded {
		t.Fatalf("state is %q, want landed", loaded.State())
	}
}

func TestLandingIsRefusedWhenTheTargetMovedOn(t *testing.T) {
	t.Parallel()

	r, store := repo(t)
	base, _ := r.Resolve("HEAD")
	head := commit(t, r, "the work")
	reset(t, r, base)

	proposal := openProposal(t, store, base, head)

	// Somebody else lands first, so the review looked at the wrong code.
	commit(t, r, "someone else got there first")

	landed, _ := review.Landed(proposal.ID(), "leandro", clock())
	if err := store.Land(proposal, landed); !errors.Is(err, gitstore.ErrGit) {
		t.Fatalf("got %v, want a refusal", err)
	}
}

func TestListingProposals(t *testing.T) {
	t.Parallel()

	r, store := repo(t)
	base, _ := r.Resolve("HEAD")
	head := commit(t, r, "the work")
	openProposal(t, store, base, head)

	all, err := store.List()
	if err != nil {
		t.Fatalf("list: %v", err)
	}

	if len(all) != 1 || all[0].ID() != "gate" {
		t.Fatalf("listed %v", all)
	}
}

func TestAProposalNobodyOpened(t *testing.T) {
	t.Parallel()

	_, store := repo(t)

	if _, err := store.Load("missing"); !errors.Is(err, gitstore.ErrNotFound) {
		t.Fatalf("got %v, want not found", err)
	}
}

func reset(t *testing.T, r gitstore.Repo, to review.SHA) {
	t.Helper()

	cmd := exec.Command("git", "update-ref", "refs/heads/main", string(to))
	cmd.Dir = r.Dir()

	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("reset: %v: %s", err, out)
	}
}

// note appends a raw line, the way a newer binary writing a kind this one has
// never heard of would.
func note(t *testing.T, r gitstore.Repo, ref string, revision review.SHA, line string) {
	t.Helper()

	cmd := exec.Command("git", "notes", "--ref="+ref, "append", "--no-separator", "-m", line, string(revision))
	cmd.Dir = r.Dir()

	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("note: %v: %s", err, out)
	}
}

func TestARecordFromTheFutureIsSkippedRatherThanFatal(t *testing.T) {
	t.Parallel()

	r, store := repo(t)

	base, err := r.Resolve("HEAD")
	if err != nil {
		t.Fatalf("base: %v", err)
	}

	head := commit(t, r, "the work")
	openProposal(t, store, base, head)

	note(t, r, "githerb/annotations", head,
		`{"v":1,"kind":"telepathy","author":"someone","at":"2026-08-21T18:04:05Z"}`)
	note(t, r, "githerb/proposals", head,
		`{"v":1,"kind":"summoned","id":"gate","author":"someone","at":"2026-08-21T18:04:05Z"}`)

	comment, err := review.NewComment(head, "a.txt", span(t), "still readable", "leandro", clock())
	if err != nil {
		t.Fatalf("comment: %v", err)
	}

	if err := store.Annotate(head, review.CommentRecord(comment)); err != nil {
		t.Fatalf("annotate: %v", err)
	}

	got, err := store.Load("gate")
	if err != nil {
		t.Fatalf("a kind this binary does not know took the proposal down: %v", err)
	}

	if len(got.Open()) != 1 {
		t.Fatalf("the readable records add up to %d open notes, want 1", len(got.Open()))
	}
}

func span(t *testing.T) review.Span {
	t.Helper()

	made, err := review.NewSpan(review.SideNew, 1, 1)
	if err != nil {
		t.Fatalf("span: %v", err)
	}

	return made
}

func TestAnAgentsWorkComesBackThroughGit(t *testing.T) {
	t.Parallel()

	r, store := repo(t)

	base, err := r.Resolve("HEAD")
	if err != nil {
		t.Fatalf("base: %v", err)
	}

	head := commit(t, r, "the work")
	openProposal(t, store, base, head)

	for _, phase := range []review.Phase{review.PhaseStarted, review.PhaseFailed} {
		line, err := review.NewWork(head, review.TaskRebase, phase, "claude-code", "conflicts in a.txt", clock())
		if err != nil {
			t.Fatalf("work: %v", err)
		}

		if err := store.Annotate(head, review.WorkRecord(line)); err != nil {
			t.Fatalf("annotate: %v", err)
		}
	}

	got, err := store.Load("gate")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if len(got.Work()) != 2 {
		t.Fatalf("the work log came back with %d lines, want 2", len(got.Work()))
	}

	activity := got.Activity()
	if !activity.Failed() || activity.Task() != review.TaskRebase || activity.Agent() != "claude-code" {
		t.Fatalf("activity is %+v, want a failed rebase by claude-code", activity)
	}
}
