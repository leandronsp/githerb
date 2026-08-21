package patch_test

import (
	"errors"
	"testing"

	"github.com/leandronsp/githerb/internal/patch"
)

const sample = `diff --git a/a.txt b/a.txt
index 1234567..89abcde 100644
--- a/a.txt
+++ b/a.txt
@@ -1,4 +1,5 @@
 one
-two
+TWO
+two and a half
 three
 four
diff --git a/b.txt b/b.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/b.txt
@@ -0,0 +1,2 @@
+first
+second
`

func TestItReadsEveryFile(t *testing.T) {
	t.Parallel()

	files, err := patch.Parse(sample)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if len(files) != 2 {
		t.Fatalf("read %d files, want 2", len(files))
	}

	if files[0].Path != "a.txt" || files[1].Path != "b.txt" {
		t.Fatalf("paths are %q and %q", files[0].Path, files[1].Path)
	}
}

func TestALineKnowsItsNumberOnEachSide(t *testing.T) {
	t.Parallel()

	files, err := patch.Parse(sample)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	lines := files[0].Hunks[0].Lines

	cases := []struct {
		at   int
		kind patch.Kind
		old  int
		next int
		text string
	}{
		{0, patch.Context, 1, 1, "one"},
		{1, patch.Removed, 2, 0, "two"},
		{2, patch.Added, 0, 2, "TWO"},
		{3, patch.Added, 0, 3, "two and a half"},
		{4, patch.Context, 3, 4, "three"},
		{5, patch.Context, 4, 5, "four"},
	}

	for _, tc := range cases {
		t.Run(tc.text, func(t *testing.T) {
			t.Parallel()

			got := lines[tc.at]
			if got.Kind != tc.kind || got.Old != tc.old || got.New != tc.next || got.Text != tc.text {
				t.Fatalf("line %d is %+v, want %s %d/%d %q", tc.at, got, tc.kind, tc.old, tc.next, tc.text)
			}
		})
	}
}

func TestANewFileStartsAtOne(t *testing.T) {
	t.Parallel()

	files, err := patch.Parse(sample)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	lines := files[1].Hunks[0].Lines
	if len(lines) != 2 || lines[0].New != 1 || lines[1].New != 2 {
		t.Fatalf("lines are %+v", lines)
	}
}

func TestNothingChangedIsNotAnError(t *testing.T) {
	t.Parallel()

	files, err := patch.Parse("")
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if len(files) != 0 {
		t.Fatalf("read %d files from nothing", len(files))
	}
}

func TestDiffsWeRefuse(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		raw  string
	}{
		{"a hunk with no file", "@@ -1,1 +1,1 @@\n one"},
		{"a header with no numbers", "diff --git a/a b/a\n@@ nonsense @@\n one"},
		{"a truncated header", "diff --git a/a\n"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			if _, err := patch.Parse(tc.raw); !errors.Is(err, patch.ErrMalformed) {
				t.Fatalf("got %v, want malformed", err)
			}
		})
	}
}

func TestItCountsWhatChanged(t *testing.T) {
	t.Parallel()

	files, err := patch.Parse(sample)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if got, want := files[0].Added(), 2; got != want {
		t.Fatalf("a.txt added %d, want %d", got, want)
	}

	if got, want := files[0].Removed(), 1; got != want {
		t.Fatalf("a.txt removed %d, want %d", got, want)
	}

	added, removed := patch.Count(files)
	if added != 4 || removed != 1 {
		t.Fatalf("the patch counts +%d -%d, want +4 -1", added, removed)
	}
}
