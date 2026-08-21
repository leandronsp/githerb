package web

import (
	"bytes"
	"embed"
	"fmt"
	"html/template"
	"io"
	"net/http"
	"strings"

	"github.com/leandronsp/githerb/internal/review"
)

//go:embed templates
var templates embed.FS

var pages = template.Must(template.New("").Funcs(template.FuncMap{
	"sideNew": func() review.Side { return review.SideNew },
	"sideOld": func() review.Side { return review.SideOld },
	"short": func(sha review.SHA) string {
		if len(sha) < 7 {
			return string(sha)
		}

		return string(sha)[:7]
	},
}).ParseFS(templates, "templates/*.html"))

func (s Server) render(w http.ResponseWriter, name string, data any) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")

	if err := pages.ExecuteTemplate(w, name+".html", data); err != nil {
		s.fail(w, fmt.Errorf("rendering %s: %w", name, err))
	}
}

// patch sends one fragment down the stream in the shape Datastar expects:
// an event whose data lines each carry a piece of the element, morphed into
// whatever already has that id.
func (s Server) patch(w io.Writer, name string, data any) error {
	var body bytes.Buffer

	if err := pages.ExecuteTemplate(&body, name+".html", data); err != nil {
		return fmt.Errorf("rendering %s: %w", name, err)
	}

	// Server-sent events join consecutive data lines with a newline, so an
	// HTML fragment travels as itself with no encoding either side.
	var event strings.Builder

	event.WriteString("event: " + name + "\n")

	for _, line := range strings.Split(strings.TrimRight(body.String(), "\n"), "\n") {
		event.WriteString("data: " + line + "\n")
	}

	event.WriteString("\n")

	if _, err := io.WriteString(w, event.String()); err != nil {
		return fmt.Errorf("streaming: %w", err)
	}

	return nil
}
