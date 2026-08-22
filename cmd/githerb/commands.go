package main

import (
	"flag"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/config"
	"github.com/leandronsp/githerb/internal/review"
)

func propose(args []string) error {
	set := flag.NewFlagSet("propose", flag.ContinueOnError)
	onto := set.String("onto", "main", "the branch this lands on")
	title := set.String("title", "", "what the proposal does")

	if err := set.Parse(args); err != nil {
		return ErrUsage
	}

	if strings.TrimSpace(*title) == "" {
		return fmt.Errorf("a proposal needs a title: %w", ErrUsage)
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Propose{Proposals: s.proposals, Git: s.git, Author: s.author, Now: s.now}

	proposal, err := use.Run(*title, *onto, revisionArg(set.Args()))
	if err != nil {
		return err
	}

	fmt.Printf("%s  revision 1  onto %s\n", proposal.ID(), proposal.Target())

	return nil
}

func list() error {
	s, err := newSession()
	if err != nil {
		return err
	}

	proposals, err := s.proposals.List()
	if err != nil {
		return err
	}

	if len(proposals) == 0 {
		fmt.Println("no proposals yet")

		return nil
	}

	for _, proposal := range proposals {
		fmt.Printf("%-44s %-9s r%d  %2d open  %-8s onto %s\n",
			proposal.ID(), proposal.State(), proposal.Head().Number(),
			len(proposal.Open()), proposal.CheckSummary(), proposal.Target())
	}

	return nil
}

func show(args []string) error {
	proposal, _, err := loadOne(args)
	if err != nil {
		return err
	}

	fmt.Printf("%s\n%s\n\nonto %s, cut from %s\nstate %s, revision %d of %d\n\n",
		proposal.ID(), proposal.Title(), proposal.Target(),
		short(proposal.Base()), proposal.State(),
		proposal.Head().Number(), len(proposal.Revisions()))

	for _, revision := range proposal.Revisions() {
		fmt.Printf("  r%-3d %s\n", revision.Number(), short(revision.SHA()))
	}

	fmt.Println()

	for _, check := range proposal.SortedChecks() {
		fmt.Printf("%-16s %s in %ds\n", check.Name(), check.Status(), check.Seconds())
	}

	if len(proposal.SortedChecks()) > 0 {
		fmt.Println()
	}

	fmt.Printf("%s\n\n", agentLine(proposal))

	open := proposal.Open()
	if len(open) == 0 {
		fmt.Println("nothing open")

		return nil
	}

	for _, comment := range open {
		fmt.Printf("%s  %s:%d", comment.ID(), comment.File(), comment.Span().Start())

		if comment.Span().Lines() > 1 {
			fmt.Printf(":%d", comment.Span().End())
		}

		fmt.Printf("\n  %s\n", comment.Body())
	}

	return nil
}

// agentLine is the same sentence the browser puts in its chip, so the two
// surfaces never disagree about who is on it.
func agentLine(proposal review.Proposal) string {
	activity := proposal.Activity()

	switch {
	case activity.Working():
		return fmt.Sprintf("%s is %s since %s", activity.Agent(), activity.Task(), activity.Since().Format("15:04"))
	case activity.Failed():
		return fmt.Sprintf("%s failed: %s", activity.Task(), activity.Note())
	case proposal.Dispatched():
		return "waiting for an agent"
	default:
		return "no agent on it"
	}
}

func diff(args []string) error {
	proposal, s, err := loadOne(args)
	if err != nil {
		return err
	}

	patch, err := s.git.Diff(proposal.Base(), proposal.Head().SHA())
	if err != nil {
		return err
	}

	fmt.Println(patch)

	return nil
}

func comment(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	set := flag.NewFlagSet("comment", flag.ContinueOnError)
	file := set.String("file", "", "the file the comment is about")
	line := set.String("line", "", "a line, or a range as N:M")
	side := set.String("side", "new", "which side of the diff, new or old")
	body := set.String("body", "", "what the comment says")

	if err := set.Parse(args[1:]); err != nil {
		return ErrUsage
	}

	start, end, err := lineRange(*line)
	if err != nil {
		return err
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Annotate{Proposals: s.proposals, Author: s.author, Now: s.now}

	made, err := use.Run(args[0], *file, *side, start, end, *body)
	if err != nil {
		return err
	}

	fmt.Println(made.ID())

	return nil
}

func comments(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	set := flag.NewFlagSet("comments", flag.ContinueOnError)
	asJSON := set.Bool("json", false, "one record per line, for an agent to read")

	if err := set.Parse(args[1:]); err != nil {
		return ErrUsage
	}

	proposal, _, err := loadOne(args[:1])
	if err != nil {
		return err
	}

	for _, comment := range proposal.Open() {
		if err := printComment(comment, *asJSON); err != nil {
			return err
		}
	}

	return nil
}

func printComment(comment review.Comment, asJSON bool) error {
	if !asJSON {
		fmt.Printf("%s  %s:%d  %s\n", comment.ID(), comment.File(), comment.Span().Start(), comment.Body())

		return nil
	}

	line, err := comment.MarshalLine()
	if err != nil {
		return fmt.Errorf("rendering a comment: %w", err)
	}

	fmt.Println(string(line))

	return nil
}

// handover prints the whole review as one instruction, which is what the
// browser button copies and what an agent can be handed directly.
func handover(args []string) error {
	proposal, _, err := loadOne(args)
	if err != nil {
		return err
	}

	brief := proposal.Handover()
	if brief == "" {
		fmt.Println("nothing open")

		return nil
	}

	fmt.Print(brief)

	return nil
}

func dispatch(args []string) error {
	if len(args) != 1 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Dispatch{Proposals: s.proposals, Author: s.author, Now: s.now}

	proposal, err := use.Run(args[0])
	if err != nil {
		return err
	}

	fmt.Printf("%s handed over with %d open\n", proposal.ID(), len(proposal.Open()))

	return nil
}

// work is how an agent says it picked something up, finished it or gave up.
// The phases are named for a person reading the log, not for the machine.
var phases = map[string]string{"start": "started", "done": "finished", "fail": "failed"}

func workCmd(args []string) error {
	if len(args) < 2 {
		return ErrUsage
	}

	phase, known := phases[args[0]]
	if !known {
		return fmt.Errorf("work %q: %w", args[0], ErrUsage)
	}

	set := flag.NewFlagSet("work", flag.ContinueOnError)
	task := set.String("task", "", "apply, rebase or check")
	note := set.String("note", "", "one line about it, usually why it stopped")

	if err := set.Parse(args[2:]); err != nil {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Report{Proposals: s.proposals, Author: s.author, Now: s.now}

	line, err := use.Run(args[1], *task, phase, *note)
	if err != nil {
		return err
	}

	fmt.Printf("%s %s %s\n", line.Agent(), line.Task(), line.Phase())

	return nil
}

func resolve(args []string) error {
	if len(args) != 2 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Resolve{Proposals: s.proposals, Author: s.author, Now: s.now}

	return use.Run(args[0], args[1])
}

func revise(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Revise{Proposals: s.proposals, Git: s.git}

	proposal, err := use.Run(args[0], revisionArg(args[1:]))
	if err != nil {
		return err
	}

	fmt.Printf("%s  revision %d\n", proposal.ID(), proposal.Head().Number())

	return nil
}

func land(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Land{
		Proposals: s.proposals, Git: s.git,
		Required: s.config.Required(), Author: s.author, Now: s.now,
	}

	landing, err := use.Run(args[0])
	if err != nil {
		return err
	}

	landed := landing.Proposal

	fmt.Printf("%s landed onto %s at %s\n", landed.ID(), landed.Target(), short(landed.Head().SHA()))

	for _, id := range landing.Followed {
		fmt.Printf("%s now lands onto %s\n", id, landed.Target())
	}

	return nil
}

func checkCmd(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	if len(s.config.Required()) == 0 {
		fmt.Printf("no checks declared in %s\n", config.File)

		return nil
	}

	use := app.Check{
		Proposals: s.proposals,
		Config:    s.config,
		Root:      s.repo.Dir(),
		Author:    s.author,
		Now:       s.now,
	}

	results, err := use.Run(args[0])
	if err != nil {
		return err
	}

	failed := false

	for _, result := range results {
		fmt.Printf("%-16s %-7s %ds\n", result.Name(), result.Status(), result.Seconds())

		failed = failed || !result.Passed()
	}

	if failed {
		return fmt.Errorf("a check said no: %w", review.ErrCheckFailed)
	}

	return nil
}

func abandon(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Abandon{Proposals: s.proposals, Author: s.author, Now: s.now}

	proposal, err := use.Run(args[0])
	if err != nil {
		return err
	}

	fmt.Printf("%s abandoned\n", proposal.ID())

	return nil
}

const template = `{
  "chunks": [
    {
      "title":    "one line, at most 80 characters, what this decision is",
      "surface":  "what a person touches, or internal, at most 60",
      "before":   "how it worked, one line, product language, no code, at most 140",
      "after":    "how it works now, one line, at most 140",
      "decision": "the call that was made, one line, at most 200",
      "rejected": "the alternative not taken, optional, at most 140",
      "file":     "path/to/file.go",
      "side":     "new",
      "start":    12,
      "end":      18
    }
  ],
  "rationale": [
    {
      "file":  "path/to/file.go",
      "side":  "new",
      "start": 12,
      "end":   18,
      "body":  "why these lines are the way they are, one or two sentences, no more"
    }
  ]
}

Every field is one line and every line has a ceiling. Anything longer is
refused, which is the point: the format is what keeps a description short, not
the good intentions of whoever wrote it.
`

func describe(args []string) error {
	set := flag.NewFlagSet("describe", flag.ContinueOnError)
	showTemplate := set.Bool("template", false, "print the shape a description takes")

	if err := set.Parse(args); err != nil {
		return ErrUsage
	}

	if *showTemplate {
		fmt.Print(template)

		return nil
	}

	rest := set.Args()
	if len(rest) == 0 {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	use := app.Describe{Proposals: s.proposals, Author: s.author, Now: s.now}

	written, err := use.Run(rest[0], os.Stdin)
	if err != nil {
		return err
	}

	fmt.Printf("%d written\n", written)

	return nil
}

func loadOne(args []string) (review.Proposal, session, error) {
	if len(args) == 0 {
		return review.Proposal{}, session{}, ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return review.Proposal{}, session{}, err
	}

	proposal, err := s.proposals.Load(review.ProposalID(args[0]))
	if err != nil {
		return review.Proposal{}, session{}, err
	}

	return proposal, s, nil
}

func revisionArg(args []string) string {
	if len(args) > 0 && strings.TrimSpace(args[0]) != "" {
		return args[0]
	}

	return "HEAD"
}

func lineRange(raw string) (int, int, error) {
	if strings.TrimSpace(raw) == "" {
		return 0, 0, fmt.Errorf("a comment needs a line: %w", ErrUsage)
	}

	first, last, ranged := strings.Cut(raw, ":")

	start, err := strconv.Atoi(strings.TrimSpace(first))
	if err != nil {
		return 0, 0, fmt.Errorf("%q is not a line: %w", raw, ErrUsage)
	}

	if !ranged {
		return start, start, nil
	}

	end, err := strconv.Atoi(strings.TrimSpace(last))
	if err != nil {
		return 0, 0, fmt.Errorf("%q is not a range: %w", raw, ErrUsage)
	}

	return start, end, nil
}

func short(sha review.SHA) string {
	if len(sha) < 7 {
		return string(sha)
	}

	return string(sha)[:7]
}
