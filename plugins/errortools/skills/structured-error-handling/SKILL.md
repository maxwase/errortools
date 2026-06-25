---
name: structured-error-handling
description: >
  Conventions for designing typed Rust error enums with thiserror. Use whenever
  a task involves defining error types, choosing between #[source] and #[from],
  structuring error variants with or without context fields, map_err call-site
  patterns, or deciding when to use a tuple variant vs a struct variant vs
  WithContext. Trigger on: thiserror, #[source], #[from], #[error(...)], error
  enum, Error variant, map_err, impl From, transparent, even when the task
  description doesn't name a skill explicitly. See using-errortools for rendering
  and context-attachment at runtime.
---

# structured-error-handling

This skill covers how to design error types in Rust: what shape variants should
take, when to chain vs wrap, and how to convert errors at call sites. It does
not cover formatting or the `errortools` crate runtime. See `using-errortools`
for that.

## Defining the error type

**1.** Derive `thiserror::Error` + `Debug`. One error type per module, named
`Error` (referenced as `feature::Error` from outside).

```rust
// GOOD
#[derive(Debug, thiserror::Error)]
pub enum Error { /* ... */ }

// BAD
#[derive(Debug)]
pub enum MyError { /* ... */ } // not named `Error`

impl std::fmt::Display for MyError { /* ... */ } // hand-rolled
impl std::error::Error for MyError { /* ... */ } // hand-rolled (or absent)
```

**2.** Collapse single-variant enums to structs — but first check the struct
earns its existence. If its **only** job is to staple one incidental value (an
index, ID, key, attempt, path) onto a foreign error and no caller will `match`
on it, don't write a struct at all: use `WithContext` / `WithPath` (rule 7).

```rust
// BAD
pub enum Error { ReadFile(#[source] io::Error) }
// GOOD
pub struct Error(#[source] io::Error);

// ALSO BAD -- a struct whose only purpose is to carry the offending value
#[derive(thiserror::Error)]
#[error("Index {index} exceeds u16 range")]
pub struct Error { index: u32, #[source] source: TryFromIntError }
let id = u16::try_from(value).map_err(|source| Error { index: value, source })?;

// GOOD -- tag the value onto the source; renders "<value>: <error>"
pub type Error = errortools::WithContext<u32, TryFromIntError>;
let id = u16::try_from(value).map_err(|source| WithContext::new(value, source))?;
```

**3.** Use a tuple variant when wrapping a foreign error with no extra context.

```rust
// GOOD
#[error("Failed to open config")]
ConfigOpen(#[source] io::Error),
```

**4.** Use a struct variant when extra context is needed **and a caller will
match on that context**. Put context in named fields, never inside the message
via `format!`. If the context is only ever rendered (never matched), don't add a
field — layer `WithContext` over the variant instead (rule 7).

```rust
// BAD
#[error("Render template {}", _0)]
Render(String, #[source] tera::Error),
// GOOD
#[error("Render template {name}")]
Render { name: String, #[source] source: tera::Error },
```

**5.** Never print the source inside the variant message. `#[source]` already
chains it. `OneLine` / `Chain` (from `using-errortools`) walk `source()` and
join automatically.

```rust
// VERY BAD -- causes double printing of the source error
#[error("Read failed: {0}")] Read(#[source] io::Error),
// GOOD
#[error("Read failed")]      Read(#[source] io::Error),
```
The only exception is when the source error is not `#[source]`-chained for some reason, e.g., a `Box<dyn Error>` that is not `#[from]`-converted. In that case, you can include the source message in the variant text, but avoid it as much as possible. This can be the case due to serialization constraints, or when the underlying error type does not implement `std::error::Error`.

**6.** Prefer specific variants over generic ones. Use `&'static str` payloads
only for truly one-off messages.

```rust
// BAD
Other(String),
// GOOD
#[error("Failed to join task '{0}'")]
TokioJoin(&'static str),
```

**7.** For incidental context that callers will never match on (a file path, a
retry count, a record ID, the offending value being converted), prefer
`WithContext` / `WithPath` over inventing a single-variant wrapper. The trigger
is **"will anyone branch on it?"**, never *where the value came from* — a value
produced inside the function (e.g. the element being converted in a loop) is
just as incidental as one passed in as a parameter. Basic usage and rendering
live in `using-errortools` →
"Attaching incidental context" and `references/with-context.md`; the design rule
below covers the case those don't -- needing a named variant *and* a path.

```rust
// BAD -- wrapper variant exists only to attach a path
#[error("IO at {path}")]
IoAt { path: PathBuf, #[source] source: io::Error },

// GOOD -- need a named variant (callers match on the operation)? Hold the
// WithPath<C, E> (path type AND error) and keep the path OUT of the message;
// it renders "<path>: <io error>" in the chain.
enum Error {
    #[error("IO error")]
    IoAt(#[source] WithPath<PathBuf, io::Error>),
}

File::create(&path).map_err(|e| Error::IoAt(WithPath::new(path, e)))?;
```

## Message text

**Start every `#[error("...")]` message with a capital letter, and never end it
with punctuation.** Messages chain with `: ` under `OneLine` and stack in a
ladder under `Chain`, so each one is a fragment that has to read cleanly both at
the head and in the middle of a chain.

```rust
// BAD
#[error("failed to load config")]   // lowercase
#[error("Failed to load config.")]  // trailing period
// GOOD
#[error("Failed to load config")]
```

Capitalize only your own text. Foreign error messages (`io::Error`,
`sqlx::Error`) render verbatim at the tail of the chain and keep whatever case
they ship with.

## Converting at the call site

**8.** Pass the variant constructor directly to `map_err` when the variant has no extra context fields. 
Use a closure only when you need to fill in extra fields.

```rust
// BAD
.map_err(|source| Error::Config { source })?
// GOOD
.map_err(Error::Config)?
```

**9.** Chain through existing variants rather than inventing new wrapper variants.

```rust
// BAD -- new top-level variant just to wrap an inner one
#[error("Inner")] InnerWrap(#[source] inner::Error),
// GOOD -- reuse via From
.ok_or(top::Error::from(inner::Error::Foo(ctx)))?
```

**10.** Do not put `#[from]` on context-less variants. `#[from]` is only
appropriate when the source error already carries the operation context on its
own.

```rust
// BAD -- every SQL error collapses to the same variant, losing context
#[error("DB error")] Db(#[from] sqlx::Error),
// GOOD
#[error("Load user {id}")]
LoadUser { id: UserId, #[source] source: sqlx::Error },
```

**11.** Do not hand-write `impl From<other::Error> for Error`. Use `#[source]`
or `#[from]` only.

**12.** `#[error(transparent)]` forwards the inner error's `Display` and
`source()` unchanged and adds no text of its own. Its job is **module-boundary
conversions**: when a parent module aggregates a child module's `Error` that
already carries full context, so a wrapping message would only add a redundant
layer. Pair it with `#[from]` so `?` lifts the child error with no `map_err`.

```rust
// child module already produces a fully-contextualized error
mod config {
    #[derive(Debug, thiserror::Error)]
    pub enum Error { /* ... */ }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    // GOOD -- pass config::Error straight through; `?` converts via #[from]
    #[error(transparent)]
    Config(#[from] config::Error),

    // a sibling variant that DOES add context stays a normal variant
    #[error("Failed to bind {addr}")]
    Bind { addr: SocketAddr, #[source] source: io::Error },
}
```

When to reach for it:
- **Use it** when the inner error is the whole story and the outer variant would
  add nothing -- the canonical case is re-exporting a child module's `Error`
  across a module boundary.
- **Do NOT use it** when this layer adds context the caller needs (an operation
  name, an ID, a path). Give that variant a real message and a `#[source]`
  field instead; reserve `#[from]` for context-less pass-throughs (rule 10).
- `transparent` collapses `Display`/`source` but is **not** transparent to
  `Suggest` -- the outer type's hint still wins. See
  `using-errortools/references/suggestions.md`.

## Panics

**13.** Do not `unwrap()` or `expect()` in production or library code. If
genuinely unavoidable, document the invariant under `# Panics`.

```rust
// BAD
let cfg = load().unwrap();
// GOOD
let cfg = load().map_err(config::Error::Load)?;
```

## Batch operations

**14.** Do not silently skip failed items unless the API contract says so. Either
fail fast or collect per-item errors with `ManyErrors`. The canonical
`ManyErrors` pattern (push vs `collect`, `into_result`, render shapes) lives in
`using-errortools` → "Collecting batch failures" and `references/many-errors.md`.

```rust
// BAD -- failures vanish
for item in items { let _ = process(item); }
```

## Large source errors

**15.** Box large source errors to keep the variant size small.

```rust
// BAD -- variant is as large as the biggest source error
#[error("Render failed")]
Render(#[source] SomeLargeError),

// GOOD
#[error("Render failed")]
Render(#[source] Box<SomeLargeError>),
```

`Box<E>` implements `std::error::Error` when `E: Error`, so `#[source]` chains
through the box transparently and `OneLine` / `Chain` still walk the full chain.

## `anyhow` / `Box<dyn Error>`

**16.** Avoid `anyhow` or `Box<dyn Error>` in production code or library code. Callers cannot
branch on variants, and the chain is opaque. Both are acceptable in tests and
temporary scripts. If the project currently uses `anyhow`, see
`migrating-from-unstructured`.

## Tests

**17.** Assert the exact error variant, not just `.is_err()`. Once a variant is
worth matching, the unhappy path is worth testing: cover the error cases, not
only the happy path.

```rust
// BAD
assert!(result.is_err());
assert!(result.unwrap_err().to_string().contains("config file not found"));

// GOOD
assert_eq!(result.unwrap_err().to_string(), "config file not found");
// BEST
assert_matches!(result, Err(Error::Config(_)));
```
