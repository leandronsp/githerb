package runner

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/config"
	"github.com/leandronsp/githerb/internal/review"
)

// Runner watches one repository and does what its log asks for.
type Runner struct {
	Proposals review.Proposals
	Git       review.Git
	Config    config.Config
	Root      string
	Agent     string
	Author    string
	Now       app.Clock
	Every     time.Duration
	Say       func(string)
}

// The reasons a job stops short. They are values because a caller may want to
// tell them apart, and because a message built at the throw site is a message
// nobody can match on.
var (
	ErrStopped        = errors.New("runner stopped")
	ErrNothingChanged = errors.New("the agent left the worktree where it found it")
	ErrNothingToApply = errors.New("nothing is open on this revision")
	ErrConflictsLeft  = errors.New("the rebase is still conflicted")
)

// Run works until the context is done. One job at a time, on purpose: an agent
// job is minutes of a machine and somebody's money, and two of them on one
// repository is how a rebase lands on top of an apply.
func (r Runner) Run(ctx context.Context) error {
	ticker := time.NewTicker(r.every())
	defer ticker.Stop()

	if _, err := r.Recover(); err != nil {
		r.say("%v", err)
	}

	for {
		if _, err := r.Once(ctx); err != nil && !errors.Is(err, context.Canceled) {
			r.say("%v", err)
		}

		select {
		case <-ctx.Done():
			return ErrStopped
		case <-ticker.C:
		}
	}
}

// Recover hands back the claims of a runner that is gone. This one holds the
// repository's lock, so anything still marked started was left by a process
// that died: cleared, not failed, because the task was never tried to the end.
func (r Runner) Recover() (int, error) {
	proposals, err := r.Proposals.List()
	if err != nil {
		return 0, err
	}

	cleared := 0

	for _, proposal := range proposals {
		activity := proposal.Activity()
		if proposal.State() != review.StateOpen || !activity.Working() {
			continue
		}

		job := Job{ID: proposal.ID(), Task: activity.Task(), Why: "cleared"}
		if err := r.report(job, review.PhaseCleared, "the runner that claimed this is gone"); err != nil {
			return cleared, err
		}

		r.say("%s: %s was left claimed, handing it back", proposal.ID(), activity.Task())

		cleared++
	}

	return cleared, nil
}

// Once takes one pass: it works out what is pending and does the first of it.
func (r Runner) Once(ctx context.Context) ([]Job, error) {
	proposals, err := r.Proposals.List()
	if err != nil {
		return nil, err
	}

	behind, err := r.stale(proposals)
	if err != nil {
		return nil, err
	}

	jobs := Pending(proposals, behind, r.Config.Required())

	for _, job := range jobs {
		if err := r.do(ctx, job); err != nil {
			return jobs, err
		}
	}

	return jobs, nil
}

// stale answers the one question the log cannot: whether the branch a proposal
// lands on has moved past the commit it was cut from.
func (r Runner) stale(proposals []review.Proposal) (map[review.ProposalID]bool, error) {
	behind := make(map[review.ProposalID]bool, len(proposals))

	for _, proposal := range proposals {
		if proposal.State() != review.StateOpen {
			continue
		}

		tip, err := r.Git.HeadOf(proposal.Target())
		if err != nil {
			return nil, err
		}

		common, err := r.Git.MergeBase(tip, proposal.Head().SHA())
		if err != nil {
			return nil, err
		}

		behind[proposal.ID()] = common != tip
	}

	return behind, nil
}

func (r Runner) do(ctx context.Context, job Job) error {
	proposal, err := r.Proposals.Load(job.ID)
	if err != nil {
		return err
	}

	r.say("%s: %s, %s", job.ID, job.Task, job.Why)

	if err := r.report(job, review.PhaseStarted, ""); err != nil {
		return err
	}

	note, err := r.perform(ctx, job, proposal)
	if err != nil {
		r.say("%s: %s failed: %v", job.ID, job.Task, err)

		return r.report(job, review.PhaseFailed, firstLine(err.Error()))
	}

	r.say("%s: %s done, %s", job.ID, job.Task, note)

	return r.report(job, review.PhaseFinished, note)
}

func (r Runner) perform(ctx context.Context, job Job, proposal review.Proposal) (string, error) {
	switch job.Task {
	case review.TaskApply:
		return r.apply(ctx, proposal)
	case review.TaskRebase:
		return r.rebase(ctx, proposal)
	case review.TaskCheck:
		return r.check(proposal)
	default:
		return "", fmt.Errorf("task %q: %w", job.Task, review.ErrUnknownTask)
	}
}

// apply hands the open notes to the agent in a worktree of the head, and takes
// whatever it committed as the next revision.
func (r Runner) apply(ctx context.Context, proposal review.Proposal) (string, error) {
	brief := proposal.Brief()
	if brief == "" {
		return "", ErrNothingToApply
	}

	return r.agentRevision(ctx, proposal, brief, nil)
}

// rebase moves the work onto a target that ran ahead. Git does it when the
// change still applies; when it does not, the agent that wrote the code is the
// one asked to resolve it, in the same worktree, mid-rebase.
func (r Runner) rebase(ctx context.Context, proposal review.Proposal) (string, error) {
	tip, err := r.Git.HeadOf(proposal.Target())
	if err != nil {
		return "", err
	}

	replay := func(where tree) error {
		_, err := where.git(ctx, "rebase", "--onto", string(tip), string(proposal.Base()))
		if err == nil {
			return nil
		}

		if _, called := r.call(ctx, where, conflictBrief(proposal, tip)); called != nil {
			_, _ = where.git(ctx, "rebase", "--abort")

			return called
		}

		if where.rebasing() {
			_, _ = where.git(ctx, "rebase", "--abort")

			return ErrConflictsLeft
		}

		return nil
	}

	return r.agentRevision(ctx, proposal, "", replay)
}

// agentRevision runs something inside a worktree and records the commit it
// leaves behind. A worktree that comes back at the same commit means nothing
// happened, and a revision nobody changed is worse than no revision.
func (r Runner) agentRevision(
	ctx context.Context, proposal review.Proposal, brief string, instead func(tree) error,
) (string, error) {
	head := proposal.Head().SHA()

	where, err := openTree(r.Root, head)
	if err != nil {
		return "", err
	}

	defer where.close()

	said := ""

	if instead != nil {
		err = instead(where)
	} else {
		said, err = r.call(ctx, where, brief)
	}

	if err != nil {
		return "", err
	}

	moved, err := where.head()
	if err != nil {
		return "", err
	}

	if moved == head {
		return "", ErrNothingChanged
	}

	revise := app.Revise{Proposals: r.Proposals, Git: r.Git}

	next, err := revise.Run(string(proposal.ID()), string(moved))

	// An agent that happens to have this CLI may have recorded the revision
	// itself. The commit is what matters and it is there either way.
	if errors.Is(err, review.ErrRevisionKnown) {
		next, err = r.Proposals.Load(proposal.ID())
	}

	if err != nil {
		return "", err
	}

	return fmt.Sprintf("revision %d at %s %s", next.Head().Number(), short(moved), said), nil
}

func (r Runner) check(proposal review.Proposal) (string, error) {
	use := app.Check{
		Proposals: r.Proposals, Config: r.Config, Root: r.Root,
		Author: r.Author, Now: r.Now,
	}

	results, err := use.Run(string(proposal.ID()))
	if err != nil {
		return "", err
	}

	failed := 0

	for _, result := range results {
		if result.Status() == review.CheckFailed {
			failed++
		}
	}

	if failed > 0 {
		return "", fmt.Errorf("%d of %d checks failed: %w", failed, len(results), review.ErrCheckFailed)
	}

	return fmt.Sprintf("%d checks passed", len(results)), nil
}

func (r Runner) report(job Job, phase review.Phase, note string) error {
	use := app.Report{Proposals: r.Proposals, Author: r.Author, Now: r.Now}

	// The note is cut here rather than trusted. A record refused for being one
	// character too long would leave the job looking like it never finished,
	// which is worse than a sentence that ends early.
	if _, err := use.Run(string(job.ID), string(job.Task), string(phase), cut(note)); err != nil {
		return err
	}

	return nil
}

// noteCeiling is the domain's ceiling for a one line note, minus room for the
// ellipsis that says it was cut.
const noteCeiling = 137

func cut(note string) string {
	note = firstLine(note)

	if len([]rune(note)) <= noteCeiling {
		return note
	}

	return string([]rune(note)[:noteCeiling]) + "..."
}

func (r Runner) say(format string, args ...any) {
	if r.Say != nil {
		r.Say(fmt.Sprintf(format, args...))
	}
}

func (r Runner) every() time.Duration {
	if r.Every <= 0 {
		return 2 * time.Second
	}

	return r.Every
}

func conflictBrief(proposal review.Proposal, onto review.SHA) string {
	return fmt.Sprintf(
		"A rebase of %s onto %s stopped on a conflict.\n"+
			"Resolve every conflict in this worktree, keeping what the proposal is for, then\n"+
			"git add the files and run git rebase --continue until the rebase is finished.\n"+
			"Change nothing else.\n",
		proposal.ID(), short(onto))
}

func short(sha review.SHA) string {
	if len(sha) < 7 {
		return string(sha)
	}

	return string(sha)[:7]
}
