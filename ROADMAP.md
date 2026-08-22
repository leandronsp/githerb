# Roadmap

Where githerb stands and where it goes next, in the order it should go. Written
from a real smoke of the review surface on 2026-08-22, one day after the Rust
rewrite landed: two throwaway proposals on this repository, the runner running
the gate, a real agent answering a note, computed styles read in the browser.
Numbers below are measured unless they say otherwise.

## Where it stands

What works: selecting lines in the gutter and leaving a note inline; threads,
replies and resolutions arriving live over the event stream; handing the notes
to the agent and watching the chip while a real `claude -p` answers in about
forty seconds, in a throwaway worktree that is removed after; the gate running
on its own on an unchecked head; a 6000-line file collapsed and loaded on
demand; the origins strip across nine revisions; light and dark.

What the smoke found, severity ordered:

1. Type is too small for a day of reading: code 12.5px, the rail 12px, buttons
   and chips 11.5px, section labels and the board's meta line 10px, all in px.
   Browser zoom scales the frame, not the reading.
2. The gate's output is invisible in the page. `gate · failed` says nothing
   about why; what `make check` printed went to the terminal running
   `githerb review` and died with it.
3. Nothing tells the reviewer the agent is done. The chip flips from
   "is apply since 15:21" to "no agent on it" and a reply appears; a tab in
   the background shows no change, and the board never says who a proposal is
   waiting on.
4. Commit messages appear nowhere: a revision is a sha. The diff is read
   without the author's sentence about it.
5. Threads on earlier revisions and resolved threads live only in the rail. A
   landed proposal with nine revisions had thirteen resolved notes and none of
   them in the code.
6. There is no note at the level of the proposal; everything is anchored to a
   span of lines.
7. The keyboard works (`n`/`p` threads, `]`/`[` files, `c` compose, `Esc`) and
   nothing on screen says so.
8. The gate compiles from a cold worktree on every revision: 26 seconds to two
   minutes here.
9. No syntax colour and unified only. Fine for text; Rust at 12.5px grey is
   tiring.

## Now

Three slices, one proposal each, each with its proof.

### Type and density

- Actual: the sizes above, in px.
- Expected: code 14px on 1.6, prose 15px on 1.5, nothing under 12px, buttons
  13px; every size in rem off a root size so browser zoom scales the reading;
  a density preference (comfortable by default, compact) kept next to the
  theme in localStorage and applied before first paint; the bar, rail and diff
  keep their proportions at 1280px wide without the actions wrapping.
- Proof: the smoke gains a step that reads the computed sizes and refuses
  anything under 12px; screenshots before and after at 1440x900.

### A check keeps its last lines

- Actual: `gate · failed` in the bar and nothing else.
- Expected: the runner keeps the tail of a check's output in the repository,
  append-only, next to the check record; the chip opens a panel with that tail
  and the exit code; `githerb show` prints the tail for a failed check. To
  decide on the way: the output as a blob written with `git hash-object -w`
  and referenced by sha from an optional `output` field on the check line, so
  an older build still reads the line; or a new record kind, which older
  builds skip. Either keeps the wire format v1.
- Proof: a check that fails with a known line shows that line in the page and
  in `show`; a passing check shows nothing extra; a line without the field
  still parses.

### Tell the reviewer when the agent is done, and who is waited on

- Actual: the chip changes quietly; the board says nothing about waiting.
- Expected: the page title carries a badge while something happened since the
  reviewer last looked ("● githerb · 1 answer"); a toast says "githerb-run
  answered 1 note" or "gate failed"; the board row says waiting on you, on the
  agent or on the gate, folded from the same records: open notes with answers
  newer than the reviewer's last note, a dispatch not yet answered, a required
  check missing or running. A desktop notification when the tab is hidden,
  off by default.
- Proof: a smoke step dispatches, lets a `sh` agent answer, and asserts the
  title badge and the board text; no new field, a fold over records.

## Next

In the order they earn their place.

1. **Waiting on whom as a first-class state**, on the board and in the title.
   The tool is the inbox of somebody running several agents; triage at a
   glance is the product.
2. **A board across repositories** (`githerb board ~/code/*`). One person, five
   agents, five repositories, one place to look. Still no server: one local
   process reading several `.git` directories.
3. **Commit messages, and what changed since I looked.** Subjects per revision
   in the strip and the rail; when a new revision arrives, open on
   `since=r(n-1)` by default.
4. **A note on the proposal, and the agent's word on each revision.** One note
   without an anchor; the agent says what it did in a revision through
   `describe`, which already exists as a channel.
5. **Transparency of the agent run.** The chip opens the exact brief that was
   sent, how long it took and what came back; a button to run the checks
   again.
6. **A verdict from an external CI** (`githerb check --from <url|file>`). The
   record exists; the reader is missing.
7. **Plugging any agent in, documented first.** A hook or skill that runs
   `githerb propose` when the agent's task ends and reads the notes back with
   `comments --json`, which is already the API; the README opens with it.
8. **A real keyboard.** A `?` overlay, `j`/`k` line by line, `r` reply, `x`
   resolve, viewed per file with a 3/30 progress.
9. **Threads that follow the line across revisions.** Anchoring by content
   instead of line number, the structural gap since the beginning.
10. **Export a review** (`githerb export <id>` to markdown): the conversation as
    an artifact, a PR description or a post.
11. Light syntax colour and a side-by-side toggle, after the type.

## Done

- 2026-08-22: the Rust rewrite. Workspace of six crates and one binary, the
  wire format v1 kept, old refs and notes loading unchanged, three git
  processes to read the whole log, one watcher for every tab, fragments over
  the event stream instead of pages, rows of about a hundred bytes. The demo
  page went from 24.7KB to 5.2KB, a 7,500-line diff from 4.9MB to 3.9KB plus
  files on demand, idle git processes from seventeen a second per tab to two a
  second in total.
- 2026-08-22: a click on a file in the rail no longer toggles the theme.
