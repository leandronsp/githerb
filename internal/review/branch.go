package review

import (
	"fmt"
	"strings"
)

// Branch is where a proposal is meant to land. It is usually the trunk, but
// nothing here cares: landing onto another branch is how a stack of proposals
// gets built before any of it reaches main.
type Branch string

// ParseBranch turns untrusted input into a Branch, and is the only door into
// one. The rules are git's own, kept to the ones that matter here: no leading
// dash so a name can never be read as a flag, no path tricks, no ref syntax.
func ParseBranch(raw string) (Branch, error) {
	name := strings.TrimSpace(raw)

	switch {
	case name == "":
		return "", ErrNoBranch
	case strings.HasPrefix(name, "-"), strings.HasPrefix(name, "/"), strings.HasSuffix(name, "/"):
		return "", fmt.Errorf("%q: %w", raw, ErrBadBranch)
	case strings.Contains(name, ".."), strings.Contains(name, "//"), strings.HasSuffix(name, ".lock"):
		return "", fmt.Errorf("%q: %w", raw, ErrBadBranch)
	case strings.ContainsAny(name, " ~^:?*[\\"):
		return "", fmt.Errorf("%q: %w", raw, ErrBadBranch)
	}

	return Branch(name), nil
}

// Ref is the fully qualified ref a branch lives in.
func (b Branch) Ref() string { return "refs/heads/" + string(b) }
