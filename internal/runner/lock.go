package runner

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
)

// ErrBusy is another runner already working this repository.
var ErrBusy = errors.New("a runner is already on this repository")

// Lock takes the repository's runner lock. Two loops on one repository would
// claim the same job twice, and the log cannot help with that: it is a set,
// and both claims would be true.
func Lock(root string) (func(), error) {
	path, err := lockPath(root)
	if err != nil {
		return nil, err
	}

	// The path is the repository's own git directory plus a constant name.
	//nolint:gosec // G304: see above
	file, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if errors.Is(err, os.ErrExist) {
		if alive(path) {
			return nil, fmt.Errorf("%s: %w", path, ErrBusy)
		}

		// The runner that wrote it is gone. Its lock is a leftover, not a claim.
		if err := os.Remove(path); err != nil {
			return nil, fmt.Errorf("clearing a stale lock: %w", err)
		}

		return Lock(root)
	}

	if err != nil {
		return nil, fmt.Errorf("taking the lock: %w", err)
	}

	_, _ = fmt.Fprintf(file, "%d\n", os.Getpid())
	_ = file.Close()

	return func() { _ = os.Remove(path) }, nil
}

func lockPath(root string) (string, error) {
	//nolint:gosec // G204: the root came from git rev-parse, not from a user.
	out, err := exec.Command("git", "-C", root, "rev-parse", "--git-common-dir").Output()
	if err != nil {
		return "", fmt.Errorf("finding the git directory: %w", err)
	}

	dir := strings.TrimSpace(string(out))
	if !filepath.IsAbs(dir) {
		dir = filepath.Join(root, dir)
	}

	return filepath.Join(dir, "githerb-runner.lock"), nil
}

// alive reports whether whoever wrote the lock is still running.
func alive(path string) bool {
	// The path is one we built from git rev-parse.
	//nolint:gosec // G304: see above
	raw, err := os.ReadFile(path)
	if err != nil {
		return false
	}

	pid, err := strconv.Atoi(strings.TrimSpace(string(raw)))
	if err != nil {
		return false
	}

	process, err := os.FindProcess(pid)
	if err != nil {
		return false
	}

	return process.Signal(syscall.Signal(0)) == nil
}
