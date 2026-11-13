---
name: structured-error-handling
description: >
  Conventions for designing typed Rust error enums with thiserror. Use whenever
  a task involves defining error types, choosing between #[source] and #[from],
  wording #[error] messages, deciding between a tuple variant, a struct variant
  and WithContext, map_err call-site patterns, #[error(transparent)], or
  asserting on errors in tests. Trigger on: thiserror, #[source], #[from],
  #[error(...)], error enum, Error variant, map_err, impl From, transparent,
  even when the task doesn't name a skill explicitly. See using-errortools for
  rendering, .context() and aggregation at runtime.
---

# structured-error-handling

How to shape error types so that one rendered chain identifies where the
failure happened and callers can branch on what matters. Rendering and the
`errortools` runtime are in `using-errortools`.

The model behind every rule: each layer's message says what *this* layer was
doing, the cause is linked with `#[source]`, and a renderer (`OneLine`,
`Chain`) joins the layers. A message never repeats its cause.

## Defining the type

**1. Derive `thiserror::Error` and `Debug`; one type per module, named
`Error`.** Outside it reads as `config::Error`, so a prefix adds nothing, and
hand-written `Display`/`Error` impls drift from the variants.

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error { /* ... */ }
```

**2. A single-variant enum is a struct.**

```rust
// BAD
pub enum Error { ReadFile(#[source] io::Error) }
// GOOD
#[derive(Debug, thiserror::Error)]
#[error("Failed to read file")]
pub struct Error(#[source] io::Error);
```

**3. Wrap a foreign error in a tuple variant with a message and `#[source]`.**

```rust
#[error("Failed to open config")]
ConfigOpen(#[source] io::Error),
```

**4. Context that nobody matches on goes in `WithContext`, not in the type.**
The test is "will a caller branch on this value?", not where the value came
from: a path, ID, index, attempt, or the value being converted is incidental
whether it was a parameter or computed in a loop. A field or wrapper type that
exists only to print it is noise.

```rust
// BAD: struct exists only to carry the offending value
#[derive(Debug, thiserror::Error)]
#[error("Index {index} exceeds u16 range")]
pub struct Error { index: u32, #[source] source: TryFromIntError }

// GOOD: "<value>: <error>"
pub type Error = errortools::WithContext<u32, TryFromIntError>;
let id = u16::try_from(value).context(value)?;

// BAD: variant exists only to attach a path
#[error("IO at {path}")]
IoAt { path: PathBuf, #[source] source: io::Error },

// GOOD: the variant names the operation, the pair carries the path.
// Keep the path out of the message; it renders "<path>: <io error>".
#[error("Failed to create file")]
Create(#[source] WithPath<PathBuf, io::Error>),
File::create(&path).context(path).map_err(Error::Create)?;
```

**5. Context that callers do match on goes in named fields of a struct
variant**, never formatted into a `String`, so it stays typed.

```rust
// BAD
#[error("Render template {}", _0)]
Render(String, #[source] tera::Error),
// GOOD
#[error("Render template {name}")]
Render { name: String, #[source] source: tera::Error },
```

**6. Do not print the source in the message.** `#[source]` already links it and
renderers walk the chain, so `{0}` prints it twice.

```rust
// BAD: "Read failed: disk full: disk full"
#[error("Read failed: {0}")] Read(#[source] io::Error),
// GOOD
#[error("Read failed")]      Read(#[source] io::Error),
```

The exception is a cause that cannot be a `#[source]` (it does not implement
`Error`, or must be serialized): then its text may go in the message. A
`ManyErrors` field is the common case, see `using-errortools`.

**7. Prefer specific variants to `Other(String)`.** A generic variant cannot be
matched and hides where it was produced. Use a `&'static str` payload only to
distinguish call sites of one failure kind.

```rust
#[error("Failed to join task '{0}'")]
TokioJoin(&'static str),
```

**8. Box large sources** so every `Result<T, Error>` stays small. `Box<E>` is
an `Error` when `E` is, so the chain is unchanged.

```rust
#[error("Render failed")]
Render(#[source] Box<SomeLargeError>),
```

## Message text

Start with a capital letter and end without punctuation. Messages are joined
with `": "` by `OneLine` and stacked by `Chain`, so each has to read as a
fragment at the head and in the middle of a chain: `Failed to load config`, not
`failed to load config.`. Foreign messages (`io::Error`) render verbatim at the
tail; leave them alone.

## Converting at the call site

**9. Pass the variant constructor to `map_err`.** Use a closure only to fill
extra fields.

```rust
// BAD
.map_err(|source| Error::Config { source })?
// GOOD
.map_err(Error::Config)?
```

**10. `#[from]` only when the source type alone identifies the operation.**
With `#[from]`, every `?` on that type lands in the same variant, so
`Db(#[from] sqlx::Error)` gives all queries one message and the chain no longer
says which failed. It is right for:

- a child module's `Error`, which already carries its own context (rule 12);
- a `WithContext` / `WithPath` pair produced by one operation in this module,
  which is what makes `.context(path)?` work without `map_err`.

Otherwise use `#[source]` and name the variant at the call site.

```rust
// BAD: every SQL error collapses to one variant
#[error("DB error")] Db(#[from] sqlx::Error),
// GOOD
#[error("Load user {id}")]
LoadUser { id: UserId, #[source] source: sqlx::Error },
```

**11. Do not hand-write `impl From<other::Error> for Error`.** It hides a
conversion that `#[from]` would show on the variant.

**12. `#[error(transparent)]` + `#[from]` at module boundaries.** A parent that
aggregates a child module's `Error` has nothing to add, and a wrapping message
would be a redundant layer. `transparent` forwards `Display` and `source()`.

```rust
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),

    // This layer adds context, so it is a normal variant.
    #[error("Failed to bind {addr}")]
    Bind { addr: SocketAddr, #[source] source: io::Error },
}
```

Do not add a second wrapper variant around an inner error that already has a
route; reuse it: `.ok_or(top::Error::from(inner::Error::Foo(ctx)))?`.
`transparent` does not forward `Suggest`
(`using-errortools/references/suggestions.md`).

## Panics, batches, opaque errors

**13. No `unwrap()` / `expect()` in production or library code.** Return a
variant; if a panic is truly unreachable, document the invariant under
`# Panics`.

```rust
// BAD
let cfg = load().unwrap();
// GOOD
let cfg = load().map_err(Error::Load)?;
```

**14. Do not silently skip failed items** (`let _ = process(item);`) unless the
API contract says so. Fail fast, or collect every failure with `ManyErrors`
(`using-errortools`, "Batches").

**15. No `anyhow` or `Box<dyn Error>` in production or library code**: callers
cannot branch on them. Both are fine in tests and throwaway scripts. To remove
them, see `migrating-from-unstructured`.

## Tests

**16. Assert the variant, and test the failure paths.** `is_err()` passes for
the wrong error, and a substring match passes for the wrong layer.

```rust
// BAD
assert!(result.is_err());
assert!(result.unwrap_err().to_string().contains("not found"));
// GOOD
assert!(matches!(result, Err(Error::Config(_))));
// GOOD, when the wording is the contract
assert_eq!(err.one_line().to_string(), "Failed to load config: app.toml: not found");
```
