package gitstore

import (
	"bytes"
	"errors"
	"fmt"
	"os/exec"
	"strings"

	"github.com/leandronsp/githerb/internal/review"
)

// ErrGit is every way the git binary can say no. The message it printed is
// wrapped in, because git explains itself better than we could.
var ErrGit = errors.New("git refused")

// ErrNotFound is a ref, note or object that is not there.
var ErrNotFound = errors.New("not found")

// Repo is a git repository on disk.
type Repo struct {
	dir string
}

// Open points at a repository. It does not check that one is there; the first
// command that needs it will say so.
func Open(dir string) Repo { return Repo{dir: dir} }

// Dir is where the repository lives.
func (r Repo) Dir() string { return r.dir }

// Resolve turns anything git accepts as a revision into a commit.
func (r Repo) Resolve(revision string) (review.SHA, error) {
	out, err := r.run("rev-parse", "--verify", "--end-of-options", revision+"^{commit}")
	if err != nil {
		return "", err
	}

	return review.SHA(out), nil
}

// HeadOf is the commit a branch points at.
func (r Repo) HeadOf(branch review.Branch) (review.SHA, error) {
	return r.Resolve(branch.Ref())
}

// MergeBase is the commit two revisions last had in common.
func (r Repo) MergeBase(one, other review.SHA) (review.SHA, error) {
	out, err := r.run("merge-base", string(one), string(other))
	if err != nil {
		return "", err
	}

	return review.SHA(out), nil
}

// Diff is the patch between two commits.
func (r Repo) Diff(from, to review.SHA) (string, error) {
	return r.run("diff", string(from)+".."+string(to))
}

// run calls git and returns its trimmed output, or the reason it refused.
func (r Repo) run(args ...string) (string, error) {
	// The arguments are ours, never a shell, and the values that reach here
	// from outside are validated first: a branch cannot start with a dash and
	// rev-parse is given --end-of-options.
	cmd := exec.Command("git", args...) //nolint:gosec // G204: see above
	cmd.Dir = r.dir

	var stdout, stderr bytes.Buffer

	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("git %s: %s: %w", strings.Join(args, " "), strings.TrimSpace(stderr.String()), ErrGit)
	}

	return strings.TrimRight(stdout.String(), "\n"), nil
}
