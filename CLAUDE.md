# githerb

A gate and a memory for trunk, built for people who work with agents.

An agent finishes a slice and proposes it. You read the diff and leave
annotations on the lines you care about. The agent reads those annotations back
as structured data, applies them, and proposes again. You approve, it lands on
main, and the whole exchange stays in the repository as a record you can read
next week.

Git is the database. Proposals are refs, annotations are notes, and both travel
over any git transport, so a team needs no server and a solo developer needs no
infrastructure at all.

## Iron rules

Everything here is non-negotiable. Everything else is a preference.

### Make is the interface

`make` lists the targets. Nothing in this repository is invoked another way, and
a command worth typing twice becomes a target.

`make check` is the gate: format, clippy with warnings as errors, tests. Run it
when a change set is done and fix whatever it raises. Never commit with it red.

### The core does no I/O

`crates/review` imports nothing that touches a disk, a network or a clock. A
timestamp is a parameter. Its `clippy.toml` disallows the types that would, so
when a test there starts wanting a git repository, the boundary leaked and the
fix is in the source.

### Git is the database, and the git binary is the client

Storage is refs, notes and objects. `crates/gitstore` shells out to `git`,
because git is the one program guaranteed to agree with git. No library
reimplementation until something forces it. Reading every proposal costs three
git processes, not one per revision; see the performance rules in
`.claude/rules/architecture.md`.

### The log is append-only

Nothing is edited, nothing is deleted. A resolution is a new record pointing at
the record it resolves. That is what lets two people annotate the same revision
and lets git merge the result with `cat_sort_uniq` instead of a conflict.

### The wire format is a contract

An annotation is one line of JSON that an agent reads without asking us
anything. It carries a version field because changing it breaks other people's
tooling. Golden files guard it.

### Typing discipline is enforced, not encouraged

Named types over primitives, private fields with a validating constructor,
closed sets as enums with an exhaustive match, one error enum per crate. The
full rules are in `.claude/rules/typing.md`, and the workspace lints in
`Cargo.toml` fail the build on each one: clippy pedantic, `unwrap`/`expect`/
`panic` denied in library code, wildcard arms on our enums denied, `unsafe`
forbidden.

### The format refuses prolixity

A description is chunks, and every field on a chunk is one line with a ceiling
in the constructor. This is deliberate and it is the only defence that survives
a different agent or a different harness: an instruction is advice, a
constructor is a rule. When a field feels too small, the sentence is too long.

### The browser holds nothing but the selection

The server renders HTML and pushes fragments over an event stream: the bar, the
rail, the thread rows, never the diff. The client is one file, it knows which
lines are selected and which theme you picked, and it fetches nothing from a
CDN. A page that disagrees with the repository is a bug in the stream, never a
second copy of the state.

### TDD, and green is not the end

Write the failing test, make it pass, then break the line it protects and
confirm that exact test fails. A test that survives the mutation was not testing
anything.

### The log is the only state, and it says who is working

An agent brackets its work with two lines in the annotation log: started, then
finished or failed with one line about why. Activity, staleness, whether
somebody is already on a proposal: all of it is derived by folding the log,
never a field anyone writes twice. If you find yourself wanting a status column,
you want a record.

A kind this build does not know is skipped, not fatal. That is what makes the
format extensible: the day a newer binary writes a new kind, every older one
still opens the proposal.

### The runner owns nothing

`crates/runner` derives jobs from the same records the browser reads, claims
one by writing that it started, and runs the repository's command in a
throwaway worktree. It never decides what a proposal means, never edits your
checkout, and never retries a task that already failed on that revision. One
runner per repository, enforced by a lock in the git directory; one job at a
time, because an agent job costs money.

## Rules files

Mandatory reading before writing code, under `.claude/rules/`:

- `typing.md` — what the compiler will not give you and how to buy it back
- `architecture.md` — what each crate is allowed to know, and the performance rules
- `testing.md` — TDD and the Rust testing mechanics

## Commits

Conventional prefix, present imperative, lowercase after the prefix, no emoji.
The message explains why, not who wrote it. Never mention AI, agents or Claude.
Stage files explicitly. One logical change per commit.

Never rewrite a commit a proposal already points at. A revision is a sha in a
ref, so amending or rebasing after `githerb revise` leaves the proposal head
off the branch, and landing it would move the target to a commit that no longer
has the work. Amend before revising, or make the fix a new revision.
