package runner

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
)

// ErrNoAgent is a repository that asked for an agent without declaring one.
var ErrNoAgent = errors.New("this repository declares no [agent] command in .githerb.toml")

// call runs the repository's agent inside a worktree with the brief on stdin.
// githerb never learns what an agent is: the command comes from the repository
// the same way a check command does, and the brief is the same text a person
// would have pasted.
func (r Runner) call(ctx context.Context, where tree, brief string) (string, error) {
	if strings.TrimSpace(r.Agent) == "" {
		return "", ErrNoAgent
	}

	cmd := exec.CommandContext(ctx, "sh", "-c", r.Agent) //nolint:gosec // G204: declared by the repository, like a check
	cmd.Dir = where.dir
	cmd.Stdin = strings.NewReader(brief)

	var out bytes.Buffer

	cmd.Stdout = &out
	cmd.Stderr = &out

	err := cmd.Run()
	said := tail(out.String())

	if err != nil {
		return said, fmt.Errorf("the agent stopped: %s: %w", said, err)
	}

	return said, nil
}

// tail is the last thing the agent said, cut to something a one line record
// can hold.
func tail(output string) string {
	lines := strings.Split(strings.TrimSpace(output), "\n")

	for i := len(lines) - 1; i >= 0; i-- {
		if said := strings.TrimSpace(lines[i]); said != "" {
			if len([]rune(said)) > 120 {
				return string([]rune(said)[:120])
			}

			return said
		}
	}

	return ""
}
