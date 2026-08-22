package web

import (
	"context"
	"embed"
	"errors"
	"fmt"
	"net"
	"net/http"
	"time"

	"github.com/leandronsp/githerb/internal/app"
	"github.com/leandronsp/githerb/internal/review"
)

//go:embed static
var static embed.FS

// Server serves the review surface for one repository.
type Server struct {
	Proposals review.Proposals
	Git       review.Git
	Required  []review.CheckName
	Author    string
	Now       app.Clock
}

// ErrClosed is the server shutting down normally.
var ErrClosed = errors.New("server closed")

// Listen binds to loopback only. This is a tool you run in your own checkout,
// not a service, and it holds no authentication because there is nobody else
// on the other end.
func Listen(port int) (net.Listener, error) {
	listener, err := net.Listen("tcp", fmt.Sprintf("127.0.0.1:%d", port))
	if err != nil {
		return nil, fmt.Errorf("listening: %w", err)
	}

	return listener, nil
}

// Serve answers on the listener until the context is done.
func (s Server) Serve(ctx context.Context, listener net.Listener) error {
	// exhaustruct earns its place on our own types, where a missing field is a
	// missing decision. http.Server is twenty fields nobody sets on purpose.
	//nolint:exhaustruct_v5 // see above
	server := &http.Server{
		Handler:           s.routes(),
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		<-ctx.Done()

		_ = server.Close()
	}()

	if err := server.Serve(listener); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return fmt.Errorf("serving: %w", err)
	}

	return ErrClosed
}

func (s Server) routes() http.Handler {
	mux := http.NewServeMux()

	mux.Handle("GET /static/", http.FileServerFS(static))
	mux.HandleFunc("GET /{$}", s.index)
	mux.HandleFunc("GET /p/{id}", s.review)
	mux.HandleFunc("GET /p/{id}/events", s.events)
	mux.HandleFunc("GET /p/{id}/handover", s.handover)
	mux.HandleFunc("POST /p/{id}/comment", s.comment)
	mux.HandleFunc("POST /p/{id}/reply", s.reply)
	mux.HandleFunc("POST /p/{id}/resolve", s.resolve)
	mux.HandleFunc("POST /p/{id}/dispatch", s.dispatch)
	mux.HandleFunc("POST /p/{id}/land", s.land)
	mux.HandleFunc("POST /p/{id}/abandon", s.abandon)

	return mux
}

func (s Server) fail(w http.ResponseWriter, err error) {
	http.Error(w, err.Error(), http.StatusBadRequest)
}
