package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os/exec"
	"os/signal"
	"runtime"
	"syscall"

	"github.com/leandronsp/githerb/internal/web"
)

func reviewSurface(args []string) error {
	set := flag.NewFlagSet("review", flag.ContinueOnError)
	port := set.Int("port", 4270, "the port to serve on, 0 to pick a free one")
	open := set.Bool("open", true, "open a browser")

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
