package runner

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/review"
)

// answer is what an agent says back about one note. It is a file rather than
// stdout so the agent can talk to a person while it works, and it lives
// outside the worktree so it can never be committed by accident.
type answer struct {
	Note string `json:"note"`
	Say  string `json:"say"`
}

// answersPath is the file the agent is told to write, named in its environment.
func answersPath(id review.ProposalID) (string, error) {
	file, err := os.CreateTemp("", fmt.Sprintf("githerb-answers-%s-*.jsonl", safe(string(id))))
	if err != nil {
		return "", fmt.Errorf("making the answers file: %w", err)
	}

	path := file.Name()

	_ = file.Close()

	return path, nil
}

func safe(id string) string {
	return strings.Map(func(r rune) rune {
		if r == filepath.Separator || r == '.' {
			return '-'
		}

		return r
	}, id)
}

// speak files what the agent said under the notes it answered, and reports how
// many landed. An answer naming a note nobody wrote is skipped: the agent is
// outside code we control and the log takes no guesses.
func (r Runner) speak(id review.ProposalID, path string) (int, error) {
	// The path is one this process made in the temp directory.
	//nolint:gosec // G304: see above
	file, err := os.Open(path)
	if os.IsNotExist(err) {
		return 0, nil
	}

	if err != nil {
		return 0, fmt.Errorf("reading the answers: %w", err)
	}

	defer func() { _ = file.Close() }()

	use := app.Reply{Proposals: r.Proposals, Author: r.Author, Now: r.Now}
	said := 0

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}

		var spoken answer
		if err := json.Unmarshal([]byte(line), &spoken); err != nil {
			r.say("%s: an answer this build cannot read: %s", id, firstLine(line))

			continue
		}

		if _, err := use.Run(string(id), spoken.Note, spoken.Say); err != nil {
			r.say("%s: %v", id, err)

			continue
		}

		said++
	}

	if err := scanner.Err(); err != nil {
		return said, fmt.Errorf("reading the answers: %w", err)
	}

	return said, nil
}
