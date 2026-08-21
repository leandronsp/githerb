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
