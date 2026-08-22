// Command githerb proposes changes, collects annotations on them and lands
// them, keeping all of it inside the repository it is run in.
package main

import (
	"errors"
	"fmt"
	"os"
)

// version is stamped at build time by the Makefile.
var version = "dev"

// ErrUsage is a command line that does not say what to do.
var ErrUsage = errors.New("usage")

func main() {
	if err := run(os.Args[1:]); err != nil {
		if errors.Is(err, ErrUsage) {
			fmt.Fprint(os.Stderr, usage)
			os.Exit(2)
		}

		fmt.Fprintf(os.Stderr, "githerb: %v\n", err)
		os.Exit(1)
	}
}

const usage = `githerb proposes work, collects annotations on it and lands it.

  githerb propose --onto main --title "what this does" [revision]
  githerb list
  githerb show <proposal>
  githerb diff <proposal>
  githerb comment <proposal> --file F --line N[:M] [--side new|old] --body "..."
  githerb comments <proposal> [--json] [--all]
  githerb resolve <proposal> <comment>
  githerb handover <proposal>          every open note as one brief, for an agent
  githerb work start|done|fail <proposal> --task apply|rebase|check [--note "..."]
  githerb dispatch <proposal>          hand the open notes to an agent
  githerb revise <proposal> [revision]
  githerb describe <proposal> < description.json    the decisions, from an agent
  githerb describe --template                       the shape it takes
  githerb check <proposal>
  githerb land <proposal>
  githerb abandon <proposal>
  githerb review [proposal]            open the review surface in a browser
  githerb version

Everything lives in the repository: proposals are refs under
refs/githerb/proposals, annotations are notes. Nothing here needs a server, and
pushing those refs is how a colleague sees them.
`

func run(args []string) error {
	if len(args) == 0 {
		return ErrUsage
	}

	command, rest := args[0], args[1:]

	switch command {
	case "propose":
		return propose(rest)
	case "list":
		return list()
	case "show":
		return show(rest)
	case "diff":
		return diff(rest)
	case "comment":
		return comment(rest)
	case "comments":
		return comments(rest)
	case "resolve":
		return resolve(rest)
	case "handover":
		return handover(rest)
	case "work":
		return workCmd(rest)
	case "dispatch":
		return dispatch(rest)
	case "revise":
		return revise(rest)
	case "describe":
		return describe(rest)
	case "check":
		return checkCmd(rest)
	case "abandon":
		return abandon(rest)
	case "land":
		return land(rest)
	case "review":
		return reviewSurface(rest)
	case "version":
		fmt.Println(version)

		return nil
	case "help", "-h", "--help":
		fmt.Print(usage)

		return nil
	default:
		return fmt.Errorf("unknown command %q: %w", command, ErrUsage)
	}
}
