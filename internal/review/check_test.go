package review_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/review"
)

func check(t *testing.T, status review.CheckStatus, on review.SHA) review.Check {
	t.Helper()

	made, err := review.NewCheck("suite", status, on, 41, "githerb-ci@laptop", at(t))
	if err != nil {
		t.Fatalf("check: %v", err)
	}

	return made
}

func TestACheckRoundTrips(t *testing.T) {
	t.Parallel()

	want := check(t, review.CheckPassed, rev)

	line, err := want.MarshalLine()
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	record, err := review.ParseLine(line)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	got, ok := record.Check()
	if !ok {
		t.Fatalf("parsed a %s, want a check", record.Kind())
	}

	if got != want {
		t.Fatalf("round trip changed the check\n got %#v\nwant %#v", got, want)
	}
}

func TestRefusedChecks(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name   string
		check  review.CheckName
		status review.CheckStatus
		on     review.SHA
		want   error
	}{
		{"no name", "", review.CheckPassed, rev, review.ErrNoCheckName},
		{"a status we do not know", "suite", "flaky", rev, review.ErrUnknownStatus},
		{"not a revision", "suite", review.CheckPassed, "nope", review.ErrNoRevision},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			_, err := review.NewCheck(tc.check, tc.status, tc.on, 1, "ci", at(t))
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v, want %v", err, tc.want)
			}
		})
	}
}

func TestTheGateWaitsForTheCheck(t *testing.T) {
	t.Parallel()

	t.Run("a required check that never ran blocks", func(t *testing.T) {
		t.Parallel()

		if err := proposal(t).Landable("suite"); !errors.Is(err, review.ErrCheckMissing) {
			t.Fatalf("got %v, want missing", err)
		}
	})

	t.Run("a required check that failed blocks", func(t *testing.T) {
		t.Parallel()

		made, err := proposal(t).WithRecord(review.CheckRecord(check(t, review.CheckFailed, rev)))
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		if err := made.Landable("suite"); !errors.Is(err, review.ErrCheckFailed) {
			t.Fatalf("got %v, want failed", err)
		}
	})

	t.Run("a passing check opens the gate", func(t *testing.T) {
		t.Parallel()

		made, err := proposal(t).WithRecord(review.CheckRecord(check(t, review.CheckPassed, rev)))
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		if err := made.Landable("suite"); err != nil {
			t.Fatalf("landable: %v", err)
		}
	})

	t.Run("a check nobody asked for does not block", func(t *testing.T) {
		t.Parallel()

		made, err := proposal(t).WithRecord(review.CheckRecord(check(t, review.CheckFailed, rev)))
		if err != nil {
			t.Fatalf("record: %v", err)
		}

		if err := made.Landable(); err != nil {
			t.Fatalf("landable: %v", err)
		}
	})
}

func TestACheckDoesNotSurviveANewRevision(t *testing.T) {
	t.Parallel()

	made, err := proposal(t).WithRecord(review.CheckRecord(check(t, review.CheckPassed, rev)))
	if err != nil {
		t.Fatalf("record: %v", err)
	}

	made, err = made.WithRevision(rev2)
	if err != nil {
		t.Fatalf("revision: %v", err)
	}

	if len(made.Checks()) != 0 {
		t.Fatalf("a result from revision one carried onto revision two")
	}

	if err := made.Landable("suite"); !errors.Is(err, review.ErrCheckMissing) {
		t.Fatalf("got %v, want missing", err)
	}
}

func TestAProposalCanBeGivenUpOn(t *testing.T) {
	t.Parallel()

	abandoned, err := proposal(t).Abandoned()
	if err != nil {
		t.Fatalf("abandon: %v", err)
	}

	if abandoned.State() != review.StateAbandoned {
		t.Fatalf("state is %q, want abandoned", abandoned.State())
	}

	if _, err := abandoned.Landed(); !errors.Is(err, review.ErrNotOpen) {
		t.Fatalf("an abandoned proposal landed: %v", err)
	}

	if _, err := abandoned.Abandoned(); !errors.Is(err, review.ErrNotOpen) {
		t.Fatalf("abandoned twice: %v", err)
	}
}

// The review asked why these had no tests. They had none because they lived in
// cmd, where a decision does not belong.
func TestTheShortestTrueThingAboutTheChecks(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name    string
		results []review.CheckStatus
		want    string
	}{
		{"nothing has run", nil, "no checks"},
		{"everything passed", []review.CheckStatus{review.CheckPassed}, "passing"},
		{"one said no", []review.CheckStatus{review.CheckFailed}, "1 failed"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			made := proposal(t)

			for i, status := range tc.results {
				result, err := review.NewCheck(
					review.CheckName(string(rune('a'+i))), status, rev, 1, "ci", at(t),
				)
				if err != nil {
					t.Fatalf("check: %v", err)
				}

				made, err = made.WithRecord(review.CheckRecord(result))
				if err != nil {
					t.Fatalf("record: %v", err)
				}
			}

			if got := made.CheckSummary(); got != tc.want {
				t.Fatalf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestChecksComeBackInAStableOrder(t *testing.T) {
	t.Parallel()

	made := proposal(t)

	for _, name := range []review.CheckName{"suite", "lint", "audit"} {
		result, err := review.NewCheck(name, review.CheckPassed, rev, 1, "ci", at(t))
		if err != nil {
			t.Fatalf("check: %v", err)
		}

		made, err = made.WithRecord(review.CheckRecord(result))
		if err != nil {
			t.Fatalf("record: %v", err)
		}
	}

	// Twice, because a map would give a different answer each time and that is
	// what makes a page flicker and a list unreadable.
	for range 2 {
		got := made.SortedChecks()
		if len(got) != 3 || got[0].Name() != "audit" || got[1].Name() != "lint" || got[2].Name() != "suite" {
			t.Fatalf("order is %v", got)
		}
	}
}
