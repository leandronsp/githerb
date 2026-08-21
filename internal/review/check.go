package review

import (
	"fmt"
	"strings"
	"time"
)

// CheckName is what a check is called in the repository's configuration.
type CheckName string

// CheckStatus is how a check ended.
type CheckStatus string

// A check either passed or it did not. There is no third answer worth
// recording, because a check that did not finish tells the gate nothing.
const (
	CheckPassed CheckStatus = "passed"
	CheckFailed CheckStatus = "failed"
)

// ParseCheckStatus turns untrusted input into a status.
func ParseCheckStatus(raw string) (CheckStatus, error) {
	switch CheckStatus(raw) {
	case CheckPassed:
		return CheckPassed, nil
	case CheckFailed:
		return CheckFailed, nil
	default:
		return "", fmt.Errorf("%q: %w", raw, ErrUnknownStatus)
	}
}

// Check is what a command said about one revision. Who ran it is a field, not
// an architecture: the same record comes from a laptop, from a loop on a spare
// machine, or from whatever CI the project already pays for.
type Check struct {
	name     CheckName
	status   CheckStatus
	revision SHA
	seconds  int
	author   string
	at       time.Time
}

// NewCheck is the only way to build a Check.
func NewCheck(name CheckName, status CheckStatus, revision SHA, seconds int, author string, at time.Time) (Check, error) {
	author = strings.TrimSpace(author)

	switch {
	case strings.TrimSpace(string(name)) == "":
		return Check{}, ErrNoCheckName
	case !shaPattern.MatchString(string(revision)):
		return Check{}, fmt.Errorf("%q: %w", revision, ErrNoRevision)
	case author == "":
		return Check{}, ErrNoAuthor
	case seconds < 0:
		return Check{}, fmt.Errorf("%d seconds: %w", seconds, ErrMalformed)
	}

	if _, err := ParseCheckStatus(string(status)); err != nil {
		return Check{}, err
	}

	return Check{
		name:     name,
		status:   status,
		revision: revision,
		seconds:  seconds,
		author:   author,
		at:       at.UTC().Truncate(time.Second),
	}, nil
}

// Name is what the check is called.
func (c Check) Name() CheckName { return c.name }

// Status is how it ended.
func (c Check) Status() CheckStatus { return c.status }

// Revision is the commit it ran against.
func (c Check) Revision() SHA { return c.revision }

// Seconds is how long it took.
func (c Check) Seconds() int { return c.seconds }

// Author is who or what ran it.
func (c Check) Author() string { return c.author }

// At is when, to the second, in UTC.
func (c Check) At() time.Time { return c.at }

// Passed is the question the gate actually asks.
func (c Check) Passed() bool { return c.status == CheckPassed }
