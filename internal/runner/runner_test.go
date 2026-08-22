package runner_test

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/config"
	"github.com/leandronsp/githerb/internal/gitstore"
	"github.com/leandronsp/githerb/internal/review"
	"github.com/leandronsp/githerb/internal/runner"
)

// A real repository and a real agent, where the agent is a shell command that
// commits something. A faked one would prove nothing about the worktree.
type kit struct {
	dir       string
	repo      gitstore.Repo
	proposals review.Proposals
}

func setup(t *testing.T) kit {
	t.Helper()

	dir := t.TempDir()

	git(t, dir, "init", "-q", "-b", "main")
	git(t, dir, "config", "user.email", "test@githerb")
	git(t, dir, "config", "user.name", "test")
	write(t, dir, "a.txt", "one\ntwo\n")
	git(t, dir, "add", "-A")
	git(t, dir, "commit", "-qm", "root")

	repo := gitstore.Open(dir)

	return kit{dir: dir, repo: repo, proposals: gitstore.NewStore(repo)}
}

func git(t *testing.T, dir string, args ...string) string {
	t.Helper()

	cmd := exec.Command("git", args...)
	cmd.Dir = dir

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v: %s", args, err, out)
	}

	return string(out)
}

func write(t *testing.T, dir, name, body string) {
	t.Helper()

	if err := exec.Command("sh", "-c", "printf %s "+quote(body)+" > "+filepath.Join(dir, name)).Run(); err != nil {
		t.Fatalf("write %s: %v", name, err)
	}
}

func quote(text string) string { return "'" + text + "'" }

// loop declares the agent the way a repository does, in its own file, because
// the runner reads that file on every pass rather than trusting what it was
// handed when it started.
func (k kit) loop(agent string) runner.Runner {
	declare(k.dir, agent)

	return runner.Runner{
		Proposals: k.proposals,
		Git:       k.repo,
		Config:    config.Config{Checks: map[string]string{}, Agent: config.Agent{Command: agent}},
		Root:      k.dir,
		Agent:     agent,
		Author:    "claude-code",
		Now:       func() time.Time { return time.Now().UTC() },
		Every:     time.Millisecond,
		Say:       func(string) {},
	}
}

func declare(dir, agent string) {
	// A literal string, so nothing in a shell command has to be escaped twice.
	_ = os.WriteFile(
		filepath.Join(dir, ".githerb.toml"),
		[]byte("[agent]\ncommand = +agent+\n"),
		0o600,
	)
}

func (k kit) propose(t *testing.T, title, onto string) review.Proposal {
	t.Helper()

	use := app.Propose{
		Proposals: k.proposals, Git: k.repo, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	proposal, err := use.Run(title, onto, "HEAD")
	if err != nil {
		t.Fatalf("propose: %v", err)
	}

	return proposal
}

func TestARunnerAnswersAHandoverWithARevision(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "a.txt", "one\nTWO\n")
	git(t, k.dir, "commit", "-qam", "the work")

	proposal := k.propose(t, "The work", "main")

	annotate := app.Annotate{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := annotate.Run(string(proposal.ID()), "a.txt", "new", 2, 2, "name it properly"); err != nil {
		t.Fatalf("annotate: %v", err)
	}

	dispatch := app.Dispatch{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := dispatch.Run(string(proposal.ID())); err != nil {
		t.Fatalf("dispatch: %v", err)
	}

	agent := "cat > brief.txt && printf 'one\\nTWO_NAMED\\n' > a.txt && git add -A && git commit -qm 'the agent answered'"

	jobs, err := k.loop(agent).Once(context.Background())
	if err != nil {
		t.Fatalf("once: %v", err)
	}

	if len(jobs) != 1 || jobs[0].Task != review.TaskApply {
		t.Fatalf("the pass did %+v, want one apply", jobs)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if after.Head().Number() != 2 {
		t.Fatalf("the proposal is at revision %d, want 2", after.Head().Number())
	}

	if !after.Activity().Idle() || len(after.Work()) != 2 {
		t.Fatalf("the work log is %+v with %d lines, want an idle pair", after.Activity(), len(after.Work()))
	}

	if after.Dispatched() {
		t.Fatalf("the proposal is still waiting for an agent that already answered")
	}

	if len(after.Open()) != 0 {
		t.Fatalf("the new revision carries %d open notes, want none", len(after.Open()))
	}
}

func TestARunnerRebasesWhatTheTargetRanPast(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "b.txt", "the proposal\n")
	git(t, k.dir, "add", "-A")
	git(t, k.dir, "commit", "-qm", "the work")

	proposal := k.propose(t, "The work", "main")

	// Somebody else lands on the trunk, in another file, so the change still
	// applies and no agent is needed.
	git(t, k.dir, "checkout", "-q", "main")
	write(t, k.dir, "c.txt", "somebody else\n")
	git(t, k.dir, "add", "-A")
	git(t, k.dir, "commit", "-qm", "the other work")

	jobs, err := k.loop("false").Once(context.Background())
	if err != nil {
		t.Fatalf("once: %v", err)
	}

	if len(jobs) != 1 || jobs[0].Task != review.TaskRebase {
		t.Fatalf("the pass did %+v, want one rebase", jobs)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if after.Head().Number() != 2 {
		t.Fatalf("the proposal is at revision %d, want 2", after.Head().Number())
	}

	tip, err := k.repo.HeadOf("main")
	if err != nil {
		t.Fatalf("main: %v", err)
	}

	parent := git(t, k.dir, "rev-parse", string(after.Head().SHA())+"^")
	if review.SHA(trim(parent)) != tip {
		t.Fatalf("the new revision sits on %s, want the trunk at %s", trim(parent), tip)
	}
}

func TestAFailureIsWrittenDownAndNotRetried(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "a.txt", "one\nTWO\n")
	git(t, k.dir, "commit", "-qam", "the work")

	proposal := k.propose(t, "The work", "main")

	annotate := app.Annotate{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := annotate.Run(string(proposal.ID()), "a.txt", "new", 2, 2, "name it properly"); err != nil {
		t.Fatalf("annotate: %v", err)
	}

	dispatch := app.Dispatch{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := dispatch.Run(string(proposal.ID())); err != nil {
		t.Fatalf("dispatch: %v", err)
	}

	loop := k.loop("echo 'no thanks' >&2; exit 3")

	if _, err := loop.Once(context.Background()); err != nil {
		t.Fatalf("once: %v", err)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if !after.Activity().Failed() || after.Activity().Note() == "" {
		t.Fatalf("the failure is %+v, want a reason written down", after.Activity())
	}

	jobs, err := loop.Once(context.Background())
	if err != nil {
		t.Fatalf("second pass: %v", err)
	}

	if len(jobs) != 0 {
		t.Fatalf("it tried the same failing job again: %+v", jobs)
	}
}

func trim(out string) string {
	for len(out) > 0 && (out[len(out)-1] == '\n' || out[len(out)-1] == '\r') {
		out = out[:len(out)-1]
	}

	return out
}

func TestARunnerHandsBackAClaimItFindsAbandoned(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "a.txt", "one\nTWO\n")
	git(t, k.dir, "commit", "-qam", "the work")

	proposal := k.propose(t, "The work", "main")

	// A runner that died mid-job leaves this behind.
	report := app.Report{
		Proposals: k.proposals, Author: "githerb-run",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := report.Run(string(proposal.ID()), "check", "started", ""); err != nil {
		t.Fatalf("report: %v", err)
	}

	stuck, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if !stuck.Activity().Working() {
		t.Fatalf("the fixture did not leave a claim: %+v", stuck.Activity())
	}

	loop := k.loop("true")

	cleared, err := loop.Recover()
	if err != nil {
		t.Fatalf("recover: %v", err)
	}

	if cleared != 1 {
		t.Fatalf("it handed back %d claims, want 1", cleared)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if !after.Activity().Idle() {
		t.Fatalf("the proposal is still %+v, want idle", after.Activity())
	}
}

func TestAnAgentAnswersInWordsWithoutTouchingCode(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "a.txt", "one\nTWO\n")
	git(t, k.dir, "commit", "-qam", "the work")

	proposal := k.propose(t, "The work", "main")

	annotate := app.Annotate{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	note, err := annotate.Run(string(proposal.ID()), "a.txt", "new", 2, 2, "only claude, or any agent?")
	if err != nil {
		t.Fatalf("annotate: %v", err)
	}

	dispatch := app.Dispatch{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := dispatch.Run(string(proposal.ID())); err != nil {
		t.Fatalf("dispatch: %v", err)
	}

	// A question deserves a sentence, not a commit. The agent reads the id out
	// of the brief and writes where the environment tells it to.
	talker := `grep -o '\[note [0-9a-f]*\]' | head -1 | tr -d '[]' | ` +
		`sed 's/note /{"note":"/;s/$/","say":"any agent CLI works, the flags are Claude Code specific"}/' ` +
		`>> "$GITHERB_ANSWERS"`

	jobs, err := k.loop(talker).Once(context.Background())
	if err != nil {
		t.Fatalf("once: %v", err)
	}

	if len(jobs) != 1 || jobs[0].Task != review.TaskApply {
		t.Fatalf("the pass did %+v, want one apply", jobs)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if after.Head().Number() != 1 {
		t.Fatalf("a question produced revision %d, and it should produce none", after.Head().Number())
	}

	answers := after.Answers(note.ID())
	if len(answers) != 1 || answers[0].Author() != "claude-code" {
		t.Fatalf("the thread carries %+v, want one answer from the agent", answers)
	}

	if !after.Activity().Idle() {
		t.Fatalf("answering read as %+v, want a finished job", after.Activity())
	}

	if len(after.Open()) != 1 {
		t.Fatalf("the note stopped being open because it was answered, and answering is not resolving")
	}
}

func TestAnAgentThatChangesCodeAndSaysNothingIsSpokenFor(t *testing.T) {
	t.Parallel()

	k := setup(t)

	git(t, k.dir, "checkout", "-q", "-b", "work")
	write(t, k.dir, "a.txt", "one\nTWO\n")
	git(t, k.dir, "commit", "-qam", "the work")

	proposal := k.propose(t, "The work", "main")

	annotate := app.Annotate{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	note, err := annotate.Run(string(proposal.ID()), "a.txt", "new", 2, 2, "name it properly")
	if err != nil {
		t.Fatalf("annotate: %v", err)
	}

	dispatch := app.Dispatch{
		Proposals: k.proposals, Author: "leandro",
		Now: func() time.Time { return time.Now().UTC() },
	}

	if _, err := dispatch.Run(string(proposal.ID())); err != nil {
		t.Fatalf("dispatch: %v", err)
	}

	// The silent kind: it commits and writes nothing to the answers file.
	mute := "printf 'one\\nNAMED\\n' > a.txt && git add -A && git commit -qm 'name the second line'"

	if _, err := k.loop(mute).Once(context.Background()); err != nil {
		t.Fatalf("once: %v", err)
	}

	after, err := k.proposals.Load(proposal.ID())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	answers := after.Answers(note.ID())
	if len(answers) != 1 {
		t.Fatalf("the thread carries %d answers, want the runner speaking once for the agent", len(answers))
	}

	if !strings.Contains(answers[0].Body(), "name the second line") {
		t.Fatalf("the answer is %q, and it should name the commit that came out of asking", answers[0].Body())
	}
}
