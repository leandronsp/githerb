# Typing discipline

Rust gives more than Go did, and still not everything. The compiler will not
stop a `String` from being passed where a sha was meant, will happily let a
struct literal build an invalid value, and `unwrap` turns a bad input into a
crash. So the discipline buys back what the compiler does not give, and every
rule here is backed by a lint that fails the build (`make lint` is
`cargo clippy --workspace --all-targets -- -D warnings` with pedantic on).

Three techniques carry most of the weight.

## 1. Named types, never bare primitives

A `&str` parameter accepts every other string in the program. Give the domain
value its own type and the compiler starts helping.

```rust
pub struct Sha(String);        // a commit, 40 hex characters
pub struct FilePath(String);   // a path inside the repository
pub struct Author(String);     // who wrote a record
```

`comment(sha: &Sha, file: &FilePath)` cannot be called with the arguments
swapped. `comment(a: &str, b: &str)` can, and will be, eventually.

The rule: if two values of the same underlying type mean different things,
they get different types. No exceptions for "it is obviously a string".

## 2. Illegal states unconstructible

Fields are private and the constructor is the only door.

```rust
pub struct Span { side: Side, start: u32, end: u32 }

impl Span {
    pub fn new(side: Side, start: u32, end: u32) -> Result<Self, Error> {
        // validate here, once, and nothing downstream ever checks again
    }
    pub fn start(&self) -> u32 { self.start }
}
```

Public fields are for data that crosses a wire and has already been validated
(the serde line structs), never for a domain value with rules. `Option<T>` says
optional; an empty string or a zero never does.

## 3. Closed sets are enums with an exhaustive match

```rust
pub enum Side { Old, New }
```

Every `match` on our own enums lists every variant. `clippy::wildcard_enum_match_arm`
is denied, so adding a third side breaks the build at every place that has to
care. Parse at the boundary, once (`Side::parse`); nothing inside re-checks.

## Errors

Errors are values, and a value with no type is a string in disguise.

- One `Error` enum per crate, hand-rolled: `Display` (lowercase, no trailing
  punctuation, carrying the offending value), `std::error::Error`, `From` at
  the crate boundary. No `thiserror`, no `anyhow`, no `Box<dyn Error>` as an
  error type.
- Callers match on variants, never on the message.
- `unwrap`, `expect` and `panic!` are denied in library code. A bad input is a
  `Result`; a broken invariant is still a `Result` with a variant that says so.
  Tests may unwrap.
- Never swallow: an error either propagates or is handled where the handling
  means something, and a handler that ignores one says why in a comment.

## Interfaces

- Accept `&T`/`&str`/`impl AsRef<..>`, return concrete types.
- A trait with one implementation and no test double is not an abstraction,
  it is indirection. Delete it.

## Banned outright

- `unsafe` (forbidden at the workspace level).
- `_ =>` on our own enums.
- `as` casts that can truncate; use `u32::try_from` and say what happens.
- `.clone()` to silence the borrow checker; borrow, or restructure.
- Boolean parameters; two call sites reading `f(true)` want an enum.
- `todo!`, `unimplemented!`, `dbg!` in committed code.
