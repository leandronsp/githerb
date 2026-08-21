---
description: Typing discipline
globs: ["**/*.go"]
---

# Typing discipline

Go's compiler gives less than Elixir's inference and far less than Rust's
algebraic types. It will not tell you that a switch missed a case, it will hand
you a zero value for any struct, and it thinks every string is every other
string. So the discipline has to manufacture what the compiler will not, and
every rule here is backed by a linter that fails the build.

Three techniques carry most of the weight.

## 1. Named types, never bare primitives

A `string` parameter accepts every other string in the program. Give the domain
value its own type and the compiler starts helping.

```go
type SHA string      // a commit, 40 hex characters
type File string     // a path inside the repository
type Login string    // an account handle
```

`Repository(sha SHA, file File)` cannot be called with the arguments swapped.
`Repository(a, b string)` can, and will be, eventually.

The rule: if two values of the same underlying type mean different things,
they get different types. No exceptions for "it is obviously a string".

## 2. Illegal states unconstructible

Go hands out the zero value of any struct to anyone who writes `var x T`. The
only way to stop an invalid value existing is to make the fields unexported and
the constructor the only door.

```go
type Range struct {
    side  Side
    start int
    end   int
}

func NewRange(side Side, start, end int) (Range, error) {
    // validate here, once, and nothing downstream ever checks again
}

func (r Range) Start() int { return r.start }
```

Exported fields are for data that crosses a wire and has already been
validated, never for a domain value with rules. `exhaustruct` runs over the
domain package so a struct literal there cannot silently omit a field.

## 3. Closed sets are named types with an exhaustive switch

Go has no enums. `iota` constants are just integers and accept any integer.

```go
type Side string

const (
    SideOld Side = "old"
    SideNew Side = "new"
)

func ParseSide(raw string) (Side, error) { ... }
```

Every switch on a closed type is checked by `exhaustive`, so adding a third
side breaks the build at every place that has to care. That is the guarantee
Rust gives for free and it is worth a linter to buy it.

Parse at the boundary, once. `ParseSide` lives where untrusted input arrives;
nothing inside re-checks.

## Errors

Errors are values, and a value with no type is a string in disguise.

- Sentinel errors per package, compared with `errors.Is`, never with `==` and
  never by matching on the message.
- Structured failures get a type and `errors.As`.
- Wrap with `%w` and add what the caller does not already know. `wrapcheck`
  fails a bare error returned from another package.
- `err113` forbids `errors.New` at the call site: define it once as a package
  sentinel so callers can match it.
- Never `return nil, nil`. `nilnil` fails it. Absence is either an error or a
  documented zero value, and you have to say which.

## Interfaces

- Accept interfaces, return concrete types. `ireturn` fails a function that
  returns an interface.
- Define the interface where it is consumed, not where it is implemented. The
  domain declares the port it needs; the adapter simply happens to satisfy it.
- An interface with one implementation and no test double is not an
  abstraction, it is indirection. Delete it.

## Banned outright

- `any` and `interface{}` outside the decode boundary, where the payload
  genuinely has no shape yet.
- Naked type assertions. `forcetypeassert` fails `x.(T)`; use the two-value
  form and handle the failure.
- `panic` in library code. Panic is for a broken program, not a bad input.
- Pointer fields to mean optional. A pointer means shared or large, never
  maybe. If a value is optional, say so with a type that carries the answer.
