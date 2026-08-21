---
description: TDD and Go testing
globs: ["**/*_test.go"]
---

# Testing

TDD, and the cycle is not done at green. Break the line the test is supposed to
protect and confirm that exact test fails. A test that survives the mutation was
not testing anything.

- **Table-driven with `t.Run`**, always named, so a failure says which case.
  `paralleltest` and `thelper` enforce the mechanics.
- **Standard library first.** `testing` plus `reflect.DeepEqual` or a hand
  written comparison covers nearly everything here. Reach for a helper only
  when the assertion is genuinely unreadable without one.
- **The core is tested without a repository.** If a test in `internal/review`
  needs a git repo on disk, the boundary leaked and the fix is in the source,
  not in the test.
- **The adapter is tested against a real repository.** `t.TempDir()`, a real
  `git init`, and the real binary. A fake git proves nothing about git.
- **`t.Cleanup` over defer** for teardown, so helpers can register their own.
- **No `time.Sleep`.** Wait on a channel, a `sync.WaitGroup` or a condition.
- **Golden files for the wire format**, because the wire format is a contract
  and a diff on it should be loud.
