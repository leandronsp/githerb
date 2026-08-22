package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os/signal"
	"syscall"
	"time"

	"github.com/leandronsp/githerb/internal/runner"
)

// runLoop is the half of githerb that moves on its own: it reads what the log
// asks for and answers it with the repository's own agent.
func runLoop(args []string) error {
	set := flag.NewFlagSet("run", flag.ContinueOnError)
	every := set.Duration("every", 2*time.Second, "how often to look for work")
	once := set.Bool("once", false, "take one pass and stop")

	if err := set.Parse(args); err != nil {
		return ErrUsage
	}

	s, err := newSession()
	if err != nil {
		return err
	}

	release, err := runner.Lock(s.repo.Dir())
	if err != nil {
		return err
	}

	defer release()

	loop := runner.Runner{
		Proposals: s.proposals,
		Git:       s.git,
		Config:    s.config,
		Root:      s.repo.Dir(),
		Agent:     s.config.Agent.Command,
		Author:    s.author,
		Now:       s.now,
		Every:     *every,
		Say:       func(line string) { fmt.Println(line) },
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	if *once {
		jobs, err := loop.Once(ctx)
		if len(jobs) == 0 && err == nil {
			fmt.Println("nothing to do")
		}

		return err
	}

	fmt.Printf("watching %s every %s\n", s.repo.Dir(), *every)

	if err := loop.Run(ctx); err != nil && !errors.Is(err, runner.ErrStopped) {
		return err
	}

	return nil
}
