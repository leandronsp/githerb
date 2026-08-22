package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"runtime"
	"syscall"
	"time"

	"github.com/leandronsp/githerb/internal/runner"
	"github.com/leandronsp/githerb/internal/web"
)

func reviewSurface(args []string) error {
	set := flag.NewFlagSet("review", flag.ContinueOnError)
	port := set.Int("port", 4270, "the port to serve on, 0 to pick a free one")
	open := set.Bool("open", true, "open a browser")
	work := set.Bool("run", true, "answer what the log asks for, in the same process")

	if err := set.Parse(args); err != nil {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	listener, err := web.Listen(*port)
	if err != nil {
		return err
	}

	where := fmt.Sprintf("http://%s", listener.Addr())
	if rest := set.Args(); len(rest) > 0 {
		where += "/p/" + rest[0]
	}

	fmt.Printf("reviewing at %s\n", where)

	if *open {
		launch(where)
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	// The review surface is the thing you leave running, so it carries the
	// runner too. Nothing here acts on its own: the agent still waits to be
	// handed the notes, which is the only trigger there is.
	if *work {
		defer alongside(ctx, s)()
	}

	server := web.Server{
		Proposals: s.proposals,
		Git:       s.git,
		Required:  s.config.Required(),
		Author:    s.author,
		Now:       s.now,
	}

	if err := server.Serve(ctx, listener); err != nil && !errors.Is(err, web.ErrClosed) {
		return err
	}

	return nil
}

// launch asks the desktop to open a URL, and shrugs if it cannot.
func launch(where string) {
	opener := "xdg-open"
	if runtime.GOOS == "darwin" {
		opener = "open"
	}

	//nolint:gosec // G204: the URL is ours, built from a listener address.
	_ = exec.Command(opener, where).Start()
}

// alongside starts the runner in the same process as the review surface and
// returns what to call when it is time to stop. A repository that already has
// a runner keeps it: the lock is the arbiter, and serving pages is useful with
// or without one.
func alongside(ctx context.Context, s session) func() {
	release, err := runner.Lock(s.repo.Dir())
	if err != nil {
		fmt.Printf("not answering the log: %v\n", err)

		return func() {}
	}

	author := s.author
	if os.Getenv("GITHERB_AUTHOR") == "" {
		author = "githerb-run"
	}

	loop := runner.Runner{
		Proposals: s.proposals,
		Git:       s.git,
		Config:    s.config,
		Root:      s.repo.Dir(),
		Agent:     s.config.Agent.Command,
		Author:    author,
		Now:       s.now,
		Every:     2 * time.Second,
		Say:       func(line string) { fmt.Println(line) },
	}

	go func() {
		if err := loop.Run(ctx); err != nil && !errors.Is(err, runner.ErrStopped) {
			fmt.Printf("the runner stopped: %v\n", err)
		}
	}()

	return release
}
