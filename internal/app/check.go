package app

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"time"

	"github.com/leandronsp/githerb/internal/config"
	"github.com/leandronsp/githerb/internal/review"
)

// ErrCheckKilled is a check that was stopped rather than answered.
var ErrCheckKilled = errors.New("a check that was killed is not a check that failed")

// Check runs what the repository declares against a proposal's head revision
// and writes down what happened.
//
// It runs in a throwaway worktree of that exact commit, not in your working
// tree, so the answer is about the code that would land rather than the code
// you happen to have open. It also means the check can run while you keep
// editing.
type Check struct {
	Proposals review.Proposals
	Config    config.Config
	Root      string
	Author    string
	Now       Clock
}

// Run runs every declared check that has not already answered for this
// revision, and returns the results.
func (c Check) Run(id string) ([]review.Check, error) {
	proposal, err := c.Proposals.Load(review.ProposalID(id))
	if err != nil {
		return nil, err
	}

	head := proposal.Head().SHA()
	already := proposal.Checks()

	var results []review.Check

	for _, name := range c.Config.Required() {
		// A revision that already answered is not asked twice. The commit is
		// the same commit; nothing about it changed while you were away.
		if done, ran := already[name]; ran {
			results = append(results, done)

			continue
		}

		result, err := c.one(name, c.Config.Checks[string(name)], head)
		if err != nil {
			return nil, err
		}

		if err := c.Proposals.Annotate(head, review.CheckRecord(result)); err != nil {
			return nil, err
		}

		results = append(results, result)
	}

	return results, nil
}

func (c Check) one(name review.CheckName, command string, head review.SHA) (review.Check, error) {
	dir, err := os.MkdirTemp("", "githerb-check-")
	if err != nil {
		return review.Check{}, fmt.Errorf("making a worktree: %w", err)
	}

	defer func() {
		//nolint:gosec // G204: the arguments are ours and the paths are ones we made.
		_ = exec.Command("git", "-C", c.Root, "worktree", "remove", "--force", dir).Run()
		_ = os.RemoveAll(dir)
	}()

	//nolint:gosec // G204: the arguments are ours and the revision is a sha we read from a ref.
	add := exec.Command("git", "-C", c.Root, "worktree", "add", "--detach", dir, string(head))

	if out, err := add.CombinedOutput(); err != nil {
		return review.Check{}, fmt.Errorf("worktree for %s: %s: %w", head, out, err)
	}

	began := time.Now()

	// The command comes from the repository, so this runs whatever the
	// repository says. That is the same trust you give a Makefile, and it is
	// why nothing here runs a check for a proposal fetched from someone else
	// without you asking for it.
	run := exec.Command("sh", "-c", command) //nolint:gosec // G204: see above
	run.Dir = dir
	run.Stdout = os.Stdout
	run.Stderr = os.Stderr

	status := review.CheckPassed

	if err := run.Run(); err != nil {
		// A command that died on a signal was killed, by a Ctrl-C or a
		// shutdown, and killing something is not a verdict on it. Recording it
		// as failed would block the revision on an answer nobody gave.
		var exit *exec.ExitError
		if errors.As(err, &exit) && exit.ExitCode() == -1 {
			return review.Check{}, fmt.Errorf("%s was killed: %w", name, ErrCheckKilled)
		}

		status = review.CheckFailed
	}

	result, err := review.NewCheck(name, status, head, int(time.Since(began).Seconds()), c.Author, c.Now())
	if err != nil {
		return review.Check{}, err
	}

	return result, nil
}
