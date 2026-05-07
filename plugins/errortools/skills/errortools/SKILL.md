---
name: errortools
description: Use when writing or refactoring Rust error-handling code — covers idiomatic source-chain design with `thiserror` and ad-hoc error logging.
---

# Rust error-handling skill

Apply this whenever you are designing error types, deciding how `main` returns errors, or formatting an error chain for users or logs in a Rust project. The `errortools` crate ([crates.io](https://crates.io/crates/errortools), [docs.rs](https://docs.rs/errortools)) provides the runtime pieces; this skill encodes the conventions for using it well.

## When to reach for it

- Binary's `main` currently does the `if let Err(e) = run() { eprintln!(...); exit(1) }` dance — replace with `MainResult`.
- You see `Error: Outer(Inner(Io(Os { ... })))` in output — that's `Debug` formatting bleeding through; switch to `MainResult` or `FormatError`.
- You need to log a full source chain on one line (structured logs) or as a tree (human terminal).
- You want a project-specific error format — implement `Format` once, reuse via `MainResult<E, MyFormat>` and `e.formatted::<MyFormat>()`.

If the project does not depend on `errortools`, add it to `Cargo.toml`:

```toml
[dependencies]
errortools = "0.1"
thiserror  = "2"
```

`errortools` is `no_std`-capable: disable the default `std` feature for embedded targets (`default-features = false`).

## Core API cheat sheet

| Item | Purpose |
|---|---|
| `MainResult<E, F = OneLine>` | Return type for `fn main`. Renders `E` via `Format` strategy `F` instead of `Debug`. |
| `OneLine` | Default strategy: joins error + sources with `": "`. |
| `Tree` | Indented multi-line strategy with `└──` connectors. |
| `Format` trait | Implement on a unit type to define a custom strategy. |
| `FormatError` ext trait | Adds `.one_line()`, `.tree()`, `.formatted::<F>()` to any `&dyn Error`. |
| `chain(&dyn Error)` | Iterator over the error and its `source()` chain — use inside `Format` impls. |
| `Formatted<E, F>` | Wrapper whose `Display` runs strategy `F` over `E`. |
| `DisplaySwapDebug<T>` | Swaps `Debug` and `Display`, so returning it from `main` prints the `Display` form. |

## Patterns

### Pattern: `MainResult` for binary entrypoints

```rust
use errortools::MainResult;
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("failed to load config")]
    Config(#[source] io::Error),
}

fn main() -> MainResult<Error> {
    fs::read_to_string("missing.toml").map_err(Error::Config)?;
    Ok(())
}
```

Output:

```text
Error: failed to load config: No such file or directory (os error 2)
```

For tree output, parameterise the strategy: `fn main() -> MainResult<Error, Tree>`.

### Pattern: ad-hoc logging mid-function

When you cannot return — e.g., inside a `tokio::spawn`, an event handler, or a retry loop — use `FormatError`:

```rust
use errortools::FormatError;

if let Err(e) = do_thing().await {
    tracing::error!("do_thing failed: {}", e.one_line());
}
```

Pick the strategy inline:

```rust
use errortools::{FormatError, Tree};
eprintln!("{}", e.formatted::<Tree>());
```

### Pattern: custom format strategy

Define once per project, reuse everywhere:

```rust
use core::{error::Error, fmt};
use errortools::{Format, FormatError, chain};
use itertools::Itertools;

pub struct Arrow;
impl Format for Arrow {
    fn fmt(error: &dyn Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(error).format(" -> "))
    }
}

// usage:
// fn main() -> MainResult<MyError, Arrow> { ... }
// tracing::error!("{}", e.formatted::<Arrow>());
```

`chain` walks `error.source()` repeatedly — never call `source()` by hand inside a `Format` impl.

## Error-type discipline

### Defining the error type

1. **MUST** derive `thiserror::Error` + `Debug`. One error type per module, named `Error` (used as `feature::Error` from outside).

   ```rust
   // GOOD
   #[derive(Debug, thiserror::Error)]
   pub enum Error { /* … */ }
   ```

2. **MUST** collapse single-variant enums to structs.

   ```rust
   // BAD
   pub enum Error { ReadFile(#[source] io::Error) }
   // GOOD
   pub struct ReadFile(#[source] io::Error);
   ```

3. **MUST** use a tuple variant when wrapping a foreign error with no extra context.

   ```rust
   // GOOD
   #[error("Failed to open config")]
   ConfigOpen(#[source] io::Error),
   ```

4. **MUST** use a struct variant when extra context is needed; put context in fields, never inside the message via `format!`.

   ```rust
   // BAD
   #[error("render template {}", name)]
   Render(String, #[source] tera::Error),
   // GOOD
   #[error("render template {name}")]
   Render { name: String, #[source] source: tera::Error },
   ```

5. **MUST NOT** print the source inside the variant message — `#[source]` already chains it. `OneLine` / `Tree` walk `source()` and join.

   ```rust
   // BAD
   #[error("read failed: {0}")] Read(#[source] io::Error),
   // GOOD
   #[error("read failed")]      Read(#[source] io::Error),
   ```

6. **PREFER** specific variants over generic ones. `&'static str` payloads only when the variant is one-off.

   ```rust
   // BAD
   Other(String),
   // GOOD
   #[error("Failed to join task '{0}'")]
   TokioJoin(&'static str),
   ```

### Converting at the call site

7. **MUST** pass the variant constructor directly to `map_err`.

   ```rust
   // BAD
   .map_err(|source| Error::Config { source })?
   // GOOD
   .map_err(Error::Config)?
   ```

8. **PREFER** chaining through existing variants over inventing new wrapper variants.

   ```rust
   // BAD — new top-level variant just to wrap an inner one
   #[error("inner")] InnerWrap(#[source] inner::Error),
   // GOOD — reuse via From
   .ok_or(top::Error::from(inner::Error::Foo(ctx)))?
   ```

9. **MUST NOT** put `#[from]` on context-less variants. `#[from]` is allowed only when the source error already carries the operation context.

   ```rust
   // BAD — every SQL collapses to one variant
   #[error("db")] Db(#[from] sqlx::Error),
   // GOOD
   #[error("load user {id}")]
   LoadUser { id: UserId, #[source] source: sqlx::Error },
   ```

10. **MUST NOT** hand-write `impl From<other::Error> for Error`. Use `#[source]` or `#[from]` only.

11. **PREFER** `#[error(transparent)]` + `#[from]` only when the inner error is the whole story (re-export wrappers).

### Logging mid-flow

12. **PREFER** `errortools::FormatError` when you cannot return — never walk `source()` by hand.

    ```rust
    // BAD
    let mut cur: &dyn Error = &e;
    while let Some(s) = cur.source() { /* … */ }
    // GOOD
    use errortools::FormatError;
    tracing::error!("do_thing: {}", e.one_line());
    ```

### Returning from `main`

13. **MUST** use `fn main() -> MainResult<Error>` (or `MainResult<Error, Tree>`). The strategy renders the chain via `Display`; `Debug` never reaches stderr.

    ```rust
    // BAD
    fn main() -> Result<(), Error> { … }
    // GOOD
    fn main() -> errortools::MainResult<Error> { … }
    ```

14. **MUST** confine `exit(1)` and `panic!()` to `main`. Business logic returns `Result`.

15. **MUST** do graceful shutdown in `main` (join threads, close connections). **AVOID** calling `drop(v)` manually — rely on scope.

### Panics

16. **MUST NOT** `unwrap()` / `expect()` in production or library code. If unavoidable, document it under `# Panics`.

    ```rust
    // BAD
    let cfg = load().unwrap();
    // GOOD
    let cfg = load().map_err(config::Error::Config)?;
    ```

### Batch operations

17. **MUST NOT** silently skip failed items unless the API contract says so — fail fast, or collect and report per-item errors.

### `anyhow` / `Box<dyn Error>`

18. **AVOID** `anyhow` or other dynamic error types outside tests / throwaway scripts. Production code uses explicit `thiserror` enums.

### Tests

19. **MUST** assert the exact error variant with arguments, not `.is_err()`.

    ```rust
    // BAD
    assert!(result.is_err());
    // GOOD
    assert!(matches!(result, Err(Error::Config(_))));
    ```

## Choosing a format strategy

| Context | Strategy |
|---|---|
| CLI tools, default | `OneLine` (single tidy line, greppable) |
| Interactive terminals where chains can be deep | `Tree` |
| Structured logs (JSON, OpenTelemetry) | `OneLine` — keep one log line per error |
| Project house style | Custom `Format` impl, applied uniformly |

Switch globally by changing the type parameter on `MainResult` — there is no need to touch call sites.

## References

- README: <https://github.com/maxwase/errortools/blob/master/README.md>
- Examples: <https://github.com/maxwase/errortools/tree/master/examples> (`one_line`, `tree`, `format_error`, `custom_format`, `transparent`)
- API docs: <https://docs.rs/errortools>
