package main

import (
	"flag"
	"fmt"
	"strconv"
	"strings"

	"github.com/leandronsp/githerb/internal/app"
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
		fmt.Printf("%-44s %-9s r%d  %2d open  onto %s\n",
			proposal.ID(), proposal.State(), proposal.Head().Number(),
			len(proposal.Open()), proposal.Target())
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

	use := app.Land{Proposals: s.proposals, Author: s.author, Now: s.now}

	proposal, err := use.Run(args[0])
	if err != nil {
		return err
	}

	fmt.Printf("%s landed onto %s at %s\n", proposal.ID(), proposal.Target(), short(proposal.Head().SHA()))

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
