package review

import (
	"fmt"
	"sort"
	"strings"
	"time"
)

// Task is what an agent was asked to do. The set is closed because a task
// nobody can run is a record nobody can act on.
type Task string

// The tasks an agent takes on a proposal.
const (
	TaskApply  Task = "apply"  // answer the open notes and submit a revision
	TaskRebase Task = "rebase" // move the work onto a target that ran ahead
	TaskCheck  Task = "check"  // run what the repository declares
)

// Phase is where a task is: it started, it finished, or it did not.
type Phase string

// The phases a task passes through.
const (
	PhaseStarted  Phase = "started"
	PhaseFinished Phase = "finished"
	PhaseFailed   Phase = "failed"
)

// Work is one line of what an agent did, appended as it happens. Two of them
// bracket a task, and what they add up to is the answer to whether anyone is
// on this proposal right now.
type Work struct {
	revision SHA
	task     Task
	phase    Phase
	agent    string
	note     string
	at       time.Time
}

// NewWork is the only way to build one.
func NewWork(revision SHA, task Task, phase Phase, agent, note string, at time.Time) (Work, error) {
	agent = strings.TrimSpace(agent)

	said, err := oneLine("note", note, maxSentence)
	if err != nil {
		return Work{}, err
	}

	switch {
	case !shaPattern.MatchString(string(revision)):
		return Work{}, fmt.Errorf("revision %q: %w", revision, ErrNoRevision)
	case !task.known():
		return Work{}, fmt.Errorf("task %q: %w", task, ErrUnknownTask)
	case !phase.known():
		return Work{}, fmt.Errorf("phase %q: %w", phase, ErrUnknownPhase)
	case agent == "":
		return Work{}, ErrNoAuthor
	}

	return Work{
		revision: revision,
		task:     task,
		phase:    phase,
		agent:    agent,
		note:     said,
		at:       at.UTC().Truncate(time.Second),
	}, nil
}

func (t Task) known() bool {
	switch t {
	case TaskApply, TaskRebase, TaskCheck:
		return true
	default:
		return false
	}
}

func (p Phase) known() bool {
	switch p {
	case PhaseStarted, PhaseFinished, PhaseFailed:
		return true
	default:
		return false
	}
}

// ParseTask reads a task off the wire or a command line.
func ParseTask(raw string) (Task, error) {
	task := Task(strings.TrimSpace(raw))
	if !task.known() {
		return "", fmt.Errorf("task %q: %w", raw, ErrUnknownTask)
	}

	return task, nil
}

// ParsePhase reads a phase off the wire or a command line.
func ParsePhase(raw string) (Phase, error) {
	phase := Phase(strings.TrimSpace(raw))
	if !phase.known() {
		return "", fmt.Errorf("phase %q: %w", raw, ErrUnknownPhase)
	}

	return phase, nil
}

// Revision is the head the work was about.
func (w Work) Revision() SHA { return w.revision }

// Task is what was being done.
func (w Work) Task() Task { return w.task }

// Phase is how far it got.
func (w Work) Phase() Phase { return w.phase }

// Agent is who was doing it.
func (w Work) Agent() string { return w.agent }

// Note is the one line it left behind, usually why it failed.
func (w Work) Note() string { return w.note }

// At is when, to the second, in UTC.
func (w Work) At() time.Time { return w.at }

// Activity is what the work log adds up to right now. One agent works on a
// proposal at a time, so the last line wins and there is nothing to merge.
type Activity struct {
	phase Phase
	task  Task
	agent string
	since time.Time
	note  string
}

// Working reports whether an agent has this proposal in hand.
func (a Activity) Working() bool { return a.phase == PhaseStarted }

// Failed reports whether the last thing tried did not work.
func (a Activity) Failed() bool { return a.phase == PhaseFailed }

// Idle reports whether nobody is on it.
func (a Activity) Idle() bool { return !a.Working() && !a.Failed() }

// Task is what is being done, or what failed.
func (a Activity) Task() Task { return a.task }

// Agent is who is doing it.
func (a Activity) Agent() string { return a.agent }

// Since is when the current phase began.
func (a Activity) Since() time.Time { return a.since }

// Note is the line the agent left, usually the reason it stopped.
func (a Activity) Note() string { return a.note }

// activityOf folds the work log in the order it happened.
func activityOf(records []Work) Activity {
	ordered := make([]Work, len(records))
	copy(ordered, records)

	sort.SliceStable(ordered, func(i, j int) bool { return ordered[i].at.Before(ordered[j].at) })

	var activity Activity

	for _, record := range ordered {
		activity = Activity{
			phase: record.phase,
			task:  record.task,
			agent: record.agent,
			since: record.at,
			note:  record.note,
		}
	}

	return activity
}

// Dispatch is a person handing the open notes to an agent. It carries nothing
// but the revision it was asked about, because everything the agent needs to
// read is already in the log.
type Dispatch struct {
	revision SHA
	author   string
	at       time.Time
}

// NewDispatch is the only way to build one.
func NewDispatch(revision SHA, author string, at time.Time) (Dispatch, error) {
	author = strings.TrimSpace(author)

	switch {
	case !shaPattern.MatchString(string(revision)):
		return Dispatch{}, fmt.Errorf("revision %q: %w", revision, ErrNoRevision)
	case author == "":
		return Dispatch{}, ErrNoAuthor
	}

	return Dispatch{revision: revision, author: author, at: at.UTC().Truncate(time.Second)}, nil
}

// Revision is the head it was asked about.
func (d Dispatch) Revision() SHA { return d.revision }

// Author is who asked.
func (d Dispatch) Author() string { return d.author }

// At is when, to the second, in UTC.
func (d Dispatch) At() time.Time { return d.at }
