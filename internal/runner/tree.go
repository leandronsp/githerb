package runner

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/leandronsp/githerb/internal/review"
)

// tree is a throwaway checkout of one commit. Every job gets its own, so an
// agent can rewrite whatever it likes without touching what you have open.
type tree struct {
	root string
	dir  string
}

func openTree(root string, head review.SHA) (tree, error) {
	dir, err := os.MkdirTemp("", "githerb-work-")
	if err != nil {
		return tree{}, fmt.Errorf("making a worktree: %w", err)
	}

	//nolint:gosec // G204: the path is one we made and the revision is a sha from a ref.
	add := exec.Command("git", "-C", root, "worktree", "add", "--detach", dir, string(head))

	if out, err := add.CombinedOutput(); err != nil {
		_ = os.RemoveAll(dir)

		return tree{}, fmt.Errorf("worktree for %s: %s: %w", head, firstLine(string(out)), err)
	}

	return tree{root: root, dir: dir}, nil
}

func (t tree) close() {
	//nolint:gosec // G204: the arguments are ours and the path is one we made.
	_ = exec.Command("git", "-C", t.root, "worktree", "remove", "--force", t.dir).Run()
	_ = os.RemoveAll(t.dir)
}

// head is where the checkout points now, which after an agent has been through
// it is how we know whether anything happened.
func (t tree) head() (review.SHA, error) {
	out, err := t.git(context.Background(), "rev-parse", "HEAD")
	if err != nil {
		return "", err
	}

	return review.SHA(strings.TrimSpace(out)), nil
}

func (t tree) git(ctx context.Context, args ...string) (string, error) {
	//nolint:gosec // G204: the arguments are ours, never a shell, and never user text.
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = t.dir

	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %s: %w", args[0], firstLine(string(out)), err)
	}

	return string(out), nil
}

// rebasing reports whether git is halfway through a rebase, which is what a
// worktree looks like when the agent walked away from a conflict.
func (t tree) rebasing() bool {
	for _, name := range []string{"rebase-merge", "rebase-apply"} {
		out, err := t.git(context.Background(), "rev-parse", "--git-path", name)
		if err != nil {
			continue
		}

		if _, err := os.Stat(strings.TrimSpace(out)); err == nil {
			return true
		}
	}

	return false
}

func firstLine(text string) string {
	trimmed := strings.TrimSpace(text)
	if line, _, found := strings.Cut(trimmed, "\n"); found {
		return line
	}

	return trimmed
}
