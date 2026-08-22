# Testing

TDD, and the cycle is not done at green. Break the line the test is supposed to
protect and confirm that exact test fails. A test that survives the mutation was
not testing anything.

- **Unit tests live with the code**: `#[cfg(test)] mod tests { use super::*; }`
  at the bottom of the module. Names are behaviour sentences with no `test_`
  prefix: `a_reply_never_blocks_landing`, not `test_reply_1`. A failure should
  read as a sentence about what broke.
- **`assert_eq!` over `assert!`**, because it prints both sides. Tests return
  `Result<(), Error>` and use `?` where a constructor can fail.
- **Standard library first.** `assert_eq!` plus `PartialEq` derives cover
  nearly everything here. Reach for a helper only when the assertion is
  genuinely unreadable without one.
- **The core is tested without a repository.** If a test in `crates/review`
  wants a git repo on disk, the boundary leaked and the fix is in the source,
  not in the test. `clippy.toml` there will refuse the I/O anyway.
- **The adapters are tested against a real repository.** A temp dir, a real
  `git init`, the real binary. Integration tests go in `crates/<crate>/tests/`.
  A fake git proves nothing about git.
- **No `thread::sleep` for synchronisation.** Wait on a channel, a `Condvar`,
  a `JoinHandle` or a condition. Polling a child process exit is the one place
  the OS forces a short interval.
- **Golden files for the wire format**, in `crates/review/tests/golden/`,
  because the wire format is a contract and a diff on it should be loud.
- **The gate is `make check`**: `cargo fmt --check`, clippy with warnings as
  errors, every test in the workspace. Run it when a change set is done; never
  commit with it red. While iterating, scope to the crate you touched:
  `cargo test -p review`.
