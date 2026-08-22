# githerb

Code review and a gate for trunk, in one binary, with no server.

An agent proposes a slice of work. You read the diff in a browser and annotate
lines. The agent reads those annotations back as JSON, applies them, proposes
again. You land it. All of it lives in your repository as refs and notes, so
there is no service to run and nothing to sign up for.

```bash
make install

githerb propose --onto main --title "Rewrite the reader"
githerb review                    # browser: diff, threads, decisions, land
githerb dispatch <proposal>       # hand the notes to an agent, or the button
githerb land <proposal>
```

## Architecture

A Cargo workspace; the dependency arrows in the `Cargo.toml` files are the
diagram.

```
  you ──► browser ─┐                        ┌─► agent (any CLI, on stdin)
                   │                        │
                   ├──► crates/app ◄────────┘   use cases, one per verb
                   │      propose  annotate     revise  dispatch
                   │      land     resolve      check   abandon
                   │             │
                   │             ▼
                   │    crates/review              the core: pure, no I/O
                   │      Proposal (aggregate root)
                   │      Comment  Reply  Resolution  Check
                   │      Chunk  Work  Dispatch  Span
                   │             │
                   │    crates/gitstore            refs + notes, append-only,
                   │             │                 three processes to read it all
                   └─────────────┼──► your git repository
                                 │
  crates/runner ─────────────────┘    read the log, claim a job, run [agent]
                                      in a throwaway worktree, write it down

  crates/web      server-rendered HTML over SSE; one JS file, no framework,
                  a hand-rolled HTTP server on std::net
  crates/patch    unified diff in, anchored lines out
  src/main.rs     flags, and nothing else
```

| what | where |
|---|---|
| a revision | `refs/githerb/proposals/<id>/<n>` |
| opened, landed, retargeted | note on revision 1, `refs/notes/githerb/proposals` |
| notes, decisions, checks, agent work | note on the revision, `refs/notes/githerb/annotations` |

One line of JSON per record, versioned, append-only. A kind a build does not
know is skipped rather than fatal. Push those refs and a colleague has the whole
review, over whatever host you already pay for.

## Configuration

```toml
# .githerb.toml
[checks]
gate = "make check"        # runs in a worktree of the head revision

[agent]
command = "claude -p --permission-mode bypassPermissions"
```

`review` carries the runner, so the thing you leave open all day is the thing
that answers. `githerb run` is the same loop on its own, for a machine that
serves no pages. One runner per repository either way, and the lock decides.

The agent gets the brief on stdin and a throwaway worktree as its working
directory. It applies the notes and commits; whatever it leaves there is read
back as the next revision. It is never asked to run githerb, and the worktree
is why bypassing permissions is reasonable: nothing it does reaches the
checkout you have open.

## Requirements

Rust 1.98 or newer to build; git 2.42 or newer to run (`git notes append
--no-separator`).

## Roadmap

Where it goes next, and why, is in [ROADMAP.md](ROADMAP.md).

## Development

```bash
make          # the targets
make check    # format, clippy with warnings as errors, tests
make smoke    # the whole product against a real repository and a real browser
```

## License

AGPL-3.0-or-later. See [LICENSE](LICENSE).
