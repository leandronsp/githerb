package main

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"

	"github.com/leandronsp/githerb/internal/gitstore"
	"github.com/leandronsp/githerb/internal/review"
)

// session is everything a command needs, assembled once.
type session struct {
	repo      gitstore.Repo
	proposals review.Proposals
	git       review.Git
	author    string
}

func newSession() (session, error) {
	root, err := repositoryRoot()
	if err != nil {
		return session{}, err
	}

	repo := gitstore.Open(root)

	return session{
		repo:      repo,
		proposals: gitstore.NewStore(repo),
		git:       repo,
		author:    author(repo),
	}, nil
}

func (s session) now() time.Time { return time.Now().UTC() }

func repositoryRoot() (string, error) {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")

	out, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("not inside a git repository: %w", gitstore.ErrGit)
	}

	return strings.TrimSpace(string(out)), nil
}

// author is who the record says did it: the environment when it is set, so an
// agent can sign as itself, and git's own identity otherwise.
func author(repo gitstore.Repo) string {
	if name := strings.TrimSpace(os.Getenv("GITHERB_AUTHOR")); name != "" {
		return name
	}

	cmd := exec.Command("git", "config", "user.name")
	cmd.Dir = repo.Dir()

	if out, err := cmd.Output(); err == nil {
		if name := strings.TrimSpace(string(out)); name != "" {
			return name
		}
	}

	return "unknown"
}
