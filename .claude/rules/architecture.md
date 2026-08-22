---
description: Layering and boundaries
globs: ["internal/**/*.go", "cmd/**/*.go"]
---

# Architecture

Git is the database. The tool is a lens on it and a gate in front of it.

Layout grows only when something pushes on it. Today:

- `internal/review` — the core. Proposals, revisions, annotations, the rules
  about them and the wire format they are stored in. Pure functions and value
  objects. It imports nothing from this project and nothing that touches a
  disk, a network or a clock. It is the most tested package here.
- `internal/gitstore` — the adapter that makes git behave like storage. Refs,
  notes, objects, diffs. Shells out to the git binary, because git is the one
  program guaranteed to agree with git.
- `internal/app` — use cases. Wires the core to the ports and holds the
  sequence of steps a command performs.
- `internal/web` — the local review surface. Renders a diff and turns a
  selection into an annotation.
- `internal/runner` — the loop that answers the log. Derives jobs from the
  records, claims one by writing that it started, runs the repository's own
  agent command in a throwaway worktree. It decides nothing about what a
  proposal means; it reads the same records the browser reads.
- `internal/patch` — a unified diff parsed into lines an annotation can point
  at. No git, no I/O, only text.
- `internal/config` — what the repository declares: its checks and its agent.
- `cmd/githerb` — argument parsing and nothing else.

## Rules

- **The core does no I/O.** No `os`, no `exec`, no `net`, no `time.Now`. A
  timestamp is a parameter. This is what makes it testable without a fixture.
- **Ports are declared by the core, satisfied by the adapters.** The core says
  what it needs; `gitstore` happens to satisfy it. The dependency arrow never
  points out of the core.
- **The wire format is the contract.** An annotation is one line of JSON, and
  an agent reads it without asking us anything. Changing that format is a
  breaking change and carries a version field for that reason.
- **Append-only.** Nothing is edited or deleted in the log. A resolution is a
  new record that points at the record it resolves, which is what lets two
  people annotate at once and git merge the result with `cat_sort_uniq`.
- **`cmd` decides nothing.** It parses flags and calls a use case. Any logic
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
