package config_test

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/leandronsp/githerb/internal/config"
)

func write(t *testing.T, body string) string {
	t.Helper()

	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, config.File), []byte(body), 0o600); err != nil {
		t.Fatalf("write: %v", err)
	}

	return dir
}

func TestItReadsTheChecks(t *testing.T) {
	t.Parallel()

	dir := write(t, "[checks]\nsuite = \"make check\"\nlint = \"make lint\"\n")

	loaded, err := config.Load(dir)
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if loaded.Checks["suite"] != "make check" {
		t.Fatalf("checks are %v", loaded.Checks)
	}

	if got := loaded.Required(); len(got) != 2 || got[0] != "lint" || got[1] != "suite" {
		t.Fatalf("required is %v, want a stable order", got)
	}
}

func TestNoConfigurationIsNotAnError(t *testing.T) {
	t.Parallel()

	loaded, err := config.Load(t.TempDir())
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if len(loaded.Required()) != 0 {
		t.Fatalf("required is %v, want nothing", loaded.Required())
	}
}

func TestAConfigurationThatDoesNotParse(t *testing.T) {
	t.Parallel()

	dir := write(t, "[checks\nsuite =")

	if _, err := config.Load(dir); !errors.Is(err, config.ErrBadConfig) {
		t.Fatalf("got %v, want bad configuration", err)
	}
}
