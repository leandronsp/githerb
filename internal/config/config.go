// Package config reads what the repository asks of a proposal before it lands.
package config

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/BurntSushi/toml"

	"github.com/leandronsp/githerb/internal/review"
)

// File is where a repository says what it wants.
const File = ".githerb.toml"

// ErrBadConfig is a configuration file that does not parse.
var ErrBadConfig = errors.New("bad configuration")

// Config is what the repository declares.
type Config struct {
	// Checks are commands that must pass on the head revision before it lands,
	// keyed by the name the record carries.
	Checks map[string]string `toml:"checks"`

	// Agent is the command that answers a handover. It is declared here for the
	// same reason a check is: githerb runs what the repository asks for and
	// never learns what any of it is.
	Agent Agent `toml:"agent"`
}

// Agent is how this repository runs an agent.
type Agent struct {
	// Command reads the brief on stdin and works in the directory it is given.
	Command string `toml:"command"`
}

// Load reads the configuration, or returns an empty one when there is none. A
// repository that declares nothing is gated by the review alone, which is a
// reasonable thing to want.
func Load(root string) (Config, error) {
	// The path is the repository root plus a constant name, and the root came
	// from git rev-parse rather than from a user.
	//nolint:gosec // G304: see above
	raw, err := os.ReadFile(filepath.Join(root, File))
	if errors.Is(err, os.ErrNotExist) {
		return Config{Checks: map[string]string{}, Agent: Agent{Command: ""}}, nil
	}

	if err != nil {
		return Config{}, fmt.Errorf("reading %s: %w", File, err)
	}

	var loaded Config
	if err := toml.Unmarshal(raw, &loaded); err != nil {
		return Config{}, fmt.Errorf("%s: %w: %w", File, err, ErrBadConfig)
	}

	if loaded.Checks == nil {
		loaded.Checks = map[string]string{}
	}

	return loaded, nil
}

// Required is every check name that has to pass, in a stable order.
func (c Config) Required() []review.CheckName {
	names := make([]review.CheckName, 0, len(c.Checks))
	for name := range c.Checks {
		names = append(names, review.CheckName(name))
	}

	sort.Slice(names, func(i, j int) bool { return names[i] < names[j] })

	return names
}
