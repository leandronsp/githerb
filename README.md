# githerb

Code review and a gate for trunk, in one binary, with no server.

An agent proposes a slice of work. You read the diff in a browser and annotate
lines. The agent reads those annotations back as JSON, applies them, proposes
again. You land it. All of it lives in your repository as refs and notes, so
there is no service to run and nothing to sign up for.

```bash
make install

githerb propose --onto main --title "Rewrite the reader"
githerb review                    # browser: diff, notes, decisions, land
githerb dispatch <proposal>       # hand the open notes to an agent
githerb run                       # a loop that answers what the log asks for
githerb land <proposal>
```

## Architecture

```
  you ──► browser ─┐                        ┌─► agent (any CLI, on stdin)
                   │                        │
                   ├──► internal/app ◄──────┘   use cases, one per verb
                   │      propose  annotate     revise  dispatch
                   │      land     resolve      check   abandon
                   │             │
                   │             ▼
                   │    internal/review             the core: pure, no I/O
                   │      Proposal (aggregate root)
                   │      Comment  Chunk  Check
                   │      Work     Dispatch  Span
                   │             │ ports declared here
                   │             ▼
                   │    internal/gitstore           refs + notes, append-only
                   │             │
                   └─────────────┼──► your git repository
                                 │
  internal/runner ───────────────┘    read the log, claim a job, run [agent]
                                      in a throwaway worktree, write it down

  internal/web     server-rendered HTML over SSE; one JS file, no framework
  internal/patch   unified diff in, anchored lines out
  cmd/githerb      flags, and nothing else
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
command = "claude -p"      # gets the brief on stdin, the worktree as cwd
```

## Roadmap

Done:

- propose, revise, annotate, resolve, land, abandon
- browser review: line and range notes, decisions, checks, live over SSE
- stacked proposals, and landing one retargets what was stacked on it
- gate: declared checks in a throwaway worktree, recorded per revision
- agent loop: dispatch, apply, rebase with the conflicts handed back
- work log: who picked it up, what happened, immutable

Next:

- read a verdict from an external CI instead of running the command here
- more than one job at a time, and runners somewhere other than your laptop
- fetching a colleague's proposals: the refs travel, the ergonomics do not
- notes that survive a revision, instead of resting on the one they were left on

## Development

```bash
make          # the targets
make check    # format, vet, warnings as errors, lint, tests
make smoke    # the whole product against a real repository and a real browser
```

## License

AGPL-3.0-or-later. See [LICENSE](LICENSE).
