# githerb

A gate and a memory for trunk, for people who work with agents.

An agent finishes a slice and proposes it. You read the diff and leave
annotations on the lines you care about. The agent reads those annotations back
as structured data, applies them, and proposes again. You approve, it lands,
and the whole exchange stays in the repository as a record you can read next
week.

There is no server and no origin to point at. Git is already a database and it
is already on your machine.

## The loop

```bash
githerb propose --onto main --title "Rewrite the reader"
githerb comment <proposal> --file a.txt --line 2:3 --body "these two want a name"
githerb comments <proposal> --json      # what the agent reads
githerb revise <proposal>               # after the agent applies them
githerb resolve <proposal> <comment>
githerb land <proposal>
```

`land` does not care which branch it lands on. Proposing onto another proposal's
branch is how a stack is built before any of it reaches the trunk.

## Where it all lives

| what | where |
|---|---|
| a revision of a proposal | `refs/githerb/proposals/<id>/<n>` |
| a proposal opening and landing | note on revision 1, ref `refs/notes/githerb/proposals` |
| an annotation and its resolution | note on the revision, ref `refs/notes/githerb/annotations` |

All of them are refs, so `git push origin 'refs/githerb/*' 'refs/notes/githerb/*'`
is how a colleague gets them, over whatever host you already use. Nothing here
needs a githerb server, and GitHub works fine as dumb storage.

The log is append-only. A resolution is a new record pointing at the record it
resolves, never an edit, which is what lets two people annotate the same
revision and lets git merge the result instead of conflicting.

## What an agent sees

One line of JSON per record, versioned, and it never has to ask us anything:

```json
{"v":1,"kind":"comment","id":"d904383dbfaf","rev":"5fe1236...","file":"a.txt","side":"new","start":2,"end":3,"body":"these two want a name","author":"leandro","at":"2026-08-21T19:53:44Z"}
```

Set `GITHERB_AUTHOR` and an agent signs as itself, so the record says which
changes came from a machine and which came from you.

## Running it

```bash
make          # list the targets
make build    # bin/githerb
make install  # onto the PATH
make check    # the gate: format, vet, lint, tests
```

## State

The browser review surface is not here yet. Everything the loop needs works
from the terminal, and the annotation format is deliberately the same one the
web will write.

There is no gate on a command yet, so landing checks only that the review is
clean, not that the tests pass.
