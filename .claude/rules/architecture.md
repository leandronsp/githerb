# Architecture

Git is the database. The tool is a lens on it and a gate in front of it.

A Cargo workspace, one crate per boundary, and the dependency arrows in the
`Cargo.toml` files are the architecture diagram. Layout grows only when
something pushes on it. Today:

- `crates/review` — the core. Proposals, revisions, annotations, the rules
  about them and the wire format they are stored in. Pure functions and value
  objects. Depends on `serde`, `serde_json`, `sha2` and nothing else; its
  `clippy.toml` disallows `std::process`, `std::fs`, `std::net` and the clock.
  It is the most tested crate here.
- `crates/patch` — a unified diff parsed into lines an annotation can point at.
  No git, no I/O, only text.
- `crates/gitstore` — the adapter that makes git behave like storage. Refs,
  notes, objects, diffs, worktrees. Shells out to the git binary, because git
  is the one program guaranteed to agree with git. Knows nothing about records.
- `crates/app` — use cases, one module per verb, plus the store that maps
  records to notes and what the repository declares in `.githerb.toml`. Holds
  the sequence of steps a command performs, and the only clock in the program.
- `crates/web` — the local review surface. A hand-rolled HTTP/1.1 server on
  `std::net`, Server-Sent Events, maud for markup. Renders a diff and turns a
  selection into an annotation.
- `crates/runner` — the loop that answers the log. Derives jobs from the
  records, claims one by writing that it started, runs the repository's own
  agent command in a throwaway worktree. It decides nothing about what a
  proposal means; it reads the same records the browser reads.
- `src/main.rs` — argument parsing and wiring, and nothing else.

## Rules

- **The core does no I/O.** No `std::process`, no `std::fs`, no `std::net`,
  no `SystemTime::now`. A timestamp is a parameter. The linter refuses the
  types, so this is enforced, not encouraged.
- **An interface with one implementation is indirection.** The store is a
  concrete type; use cases take `&Store`. A trait appears the day a second
  implementation or a test double needs it, and is defined where it is
  consumed.
- **The wire format is the contract.** An annotation is one line of JSON, and
  an agent reads it without asking us anything. Changing that format is a
  breaking change and carries a version field for that reason.
- **Append-only.** Nothing is edited or deleted in the log. A resolution is a
  new record that points at the record it resolves, which is what lets two
  people annotate at once and git merge the result with `cat_sort_uniq`.
- **`main` decides nothing.** It parses flags and calls a use case. Any logic
  found there belongs in `app` or in the core.
- **Unknown kinds are skipped, versions are not.** A record kind this build
  does not know came from a newer one and is passed over; a version it does not
  speak is refused. Leniency for what adds meaning, strictness for what changes
  it.
- **State is folded, never stored.** Whether a proposal is open, whether an
  agent is on it, whether it is waiting for one: all derived from the records.
  A status field is a second copy of the truth and it will go stale.
- **Nothing touches your working tree.** Checks and agent jobs run in a
  worktree made for them and removed after. The one exception is the git
  directory, where the runner keeps its lock.

## Performance rules

These are why the rewrite happened. Break one and the browser freezes again.

- Loading every proposal costs three git processes: one `for-each-ref`, one
  `notes list`, one `cat-file --batch` per notes ref. Never one process per
  revision, never one per proposal.
- Change detection is one process: `repo.fingerprint()`, probed by one watcher
  thread for the whole process. Pages and the runner subscribe to it; nothing
  polls git per tab.
- A diff is immutable per (from, to) and is cached.
- The page pushes fragments over SSE: the bar, the rail, the thread rows. The
  diff is pushed never; a new revision reloads the page. Nothing is pushed on
  connect when the client already has the current fingerprint.
- A diff row is three cells and an id, about a hundred bytes. Lookups per row
  come from maps built once per render, never from a scan of the notes.
