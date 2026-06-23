---
name: errortools
description: Use when writing, refactoring, or reviewing Rust error-handling code with `thiserror` and the `errortools` crate — designing error enums and source chains, returning errors from `main`, attaching context like paths/IDs/retry counts, aggregating many failures, formatting chains for users or logs, or adding "did you mean" suggestions. Reach for this whenever a Rust task touches error types, `#[from]`/`#[source]`, `main` returning a `Result`, error logging, or `errortools`/`MainResult`/`FormatError` — even when not named explicitly.
---

# Rust error-handling skill

Apply this whenever you are designing error types, deciding how `main` returns
errors, attaching context to an error, aggregating a batch of failures, or
formatting an error chain for users or logs in a Rust project. The `errortools`
crate ([crates.io](https://crates.io/crates/errortools),
[docs.rs](https://docs.rs/errortools)) provides the runtime pieces; this skill
encodes the conventions for using it well.

## When to reach for it

- A binary's `main` does the `if let Err(e) = run() { eprintln!(...); exit(1) }`
  dance — replace it with `MainResult`.
- You see `Error: Outer(Inner(Io(Os { ... })))` in output — that's `Debug`
  formatting bleeding through; switch to `MainResult`.
- You need a full source chain on one line (structured logs) or as an indented
  ladder (human terminal).
- You're tempted to add a single-variant wrapper just to attach a path or an
  attempt number — reach for `WithContext` instead.
- An operation should report *all* failures, not just the first — use `ManyErrors`.
- You want a project-specific error format — implement `Format` once and reuse it
  via `MainResult<E, MyFormat>` and `e.formatted::<MyFormat>()`.

If the project does not depend on `errortools`, add it to `Cargo.toml`:

```toml
[dependencies]
errortools = "0.3"
thiserror  = "2"
```

`errortools` is `no_std`-capable: disable the default `std` feature for embedded
targets (`default-features = false`). The `alloc` feature (implied by `std`)
gates `ManyErrors` and the aggregate shapes.

## Core API cheat sheet

| Item | Purpose |
|---|---|
| `MainResult<E, F = OneLine, T = ()>` | Return type for `fn main`. Renders `E` via strategy `F` instead of `Debug`. `T` lets `main` return `ExitCode`. |
| `OneLine` | Default strategy: error + sources joined with `": "`. |
| `Chain<C = Unicode>` | Per-error indented source-chain ladder (`└─`). *(was named `Tree` in ≤ 0.2)* |
| `Tree` / `List` / `Bullets` / `Joined` | Aggregate `ManyErrors` shapes (needs `alloc`). |
| `FormatError` | Ext trait on any `&dyn Error`: `.one_line()`, `.chain()`, `.suggestion()`, `.formatted::<F>()`. |
| `Format<E>` trait | Implement on a unit type to define a custom strategy (`Format<E: ?Sized>`). |
| `chain(&dyn Error)` | Iterator over the error and its `source()` chain — use inside `Format` impls. |
| `Formatted<E, F>` | Wrapper whose `Display` runs strategy `F` over `E`. |
| `WithContext<C, E, F = Colon>` / `WithPath` | Tag an error with a context value (path, attempt, ID) without a wrapper variant. |
| `ManyErrors<C, E>` | Aggregate of context-tagged failures; render as tree/list/bullets/joined (needs `alloc`). |
| `Suggest` / `Suggestion` | Per-error "did you mean…" hints via `.suggestion()`. |
| `Add<L, R>` + `separator::*` | Compose two strategies, e.g. `WithNewLine<OneLine, Suggestion>`. |
| `DisplaySwapDebug<T>` | Swaps `Debug`/`Display`; powers `MainResult`. |

### Going deeper

The error-type discipline below is the spine and applies to almost every task.
For the larger subsystems, read the matching reference file when the task calls
for it:

| Need | Reference |
|---|---|
| Choosing/combining formats, custom `Format`, `MainResult` internals | `references/formatting.md` |
| Attaching a path / attempt / ID to an error | `references/with-context.md` |
| Collecting and reporting many failures at once | `references/many-errors.md` |
| "Did you mean…" recovery hints | `references/suggestions.md` |

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

For the indented ladder, parameterise the strategy: `fn main() -> MainResult<Error, Chain>`.

### Pattern: ad-hoc logging mid-function

When you cannot return — inside a `tokio::spawn`, an event handler, a retry loop
— use `FormatError`. Never walk `source()` by hand.

```rust
use errortools::FormatError;

if let Err(e) = do_thing().await {
    tracing::error!("do_thing failed: {}", e.one_line());
}
```

Pick the strategy inline:

```rust
use errortools::FormatError;
eprintln!("{}", e.chain());            // indented ladder
// eprintln!("{}", e.formatted::<F>()); // any custom strategy F
```

### Pattern: custom format strategy

Implement `Format<E>` once per project, reuse everywhere. The trait bounds
nothing on `E`; a chain-walking strategy declares `E: Error` itself and walks
with `chain`:

```rust
use core::{error::Error, fmt};
use errortools::{Format, chain};

pub struct Arrow;
impl<E: Error + ?Sized> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in chain(&error).enumerate() {
            if i > 0 { f.write_str(" -> ")?; }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

// fn main() -> MainResult<MyError, Arrow> { … }
// tracing::error!("{}", e.formatted::<Arrow>());
```

`chain` walks `error.source()` repeatedly — never call `source()` by hand inside
a `Format` impl. See `references/formatting.md` for composing strategies with
`Add`/separators and for `Chain<Ascii>`/connectors.

## Error-type discipline

### Defining the error type

1. **MUST** derive `thiserror::Error` + `Debug`. One error type per module, named
   `Error` (used as `feature::Error` from outside).

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

4. **MUST** use a struct variant when extra context is needed; put context in
   fields, never inside the message via `format!`.

   ```rust
   // BAD
   #[error("render template {}", name)]
   Render(String, #[source] tera::Error),
   // GOOD
   #[error("render template {name}")]
   Render { name: String, #[source] source: tera::Error },
   ```

5. **PREFER** `WithContext` / `WithPath` to attach *incidental* context — a path,
   a retry attempt, a record ID — over inventing a single-variant wrapper whose
   only job is to hold it. `WithContext`'s `source()` skips its own inner error,
   so the chain never doubles up. Keep context in a real variant only when callers
   branch on it. See `references/with-context.md`.

   ```rust
   // BAD — a wrapper variant that exists only to staple a path on
   #[error("io at {path}")]
   IoAt { path: PathBuf, #[source] source: io::Error },
   // GOOD
   File::create(&path).map_err(|e| WithContext::new(path, e))?;
   ```

6. **MUST NOT** print the source inside the variant message — `#[source]` already
   chains it. `OneLine` / `Chain` walk `source()` and join.

   ```rust
   // BAD
   #[error("read failed: {0}")] Read(#[source] io::Error),
   // GOOD
   #[error("read failed")]      Read(#[source] io::Error),
   ```

7. **PREFER** specific variants over generic ones. `&'static str` payloads only
   when the variant is one-off.

   ```rust
   // BAD
   Other(String),
   // GOOD
   #[error("Failed to join task '{0}'")]
   TokioJoin(&'static str),
   ```

### Converting at the call site

8. **MUST** pass the variant constructor directly to `map_err`.

   ```rust
   // BAD
   .map_err(|source| Error::Config { source })?
   // GOOD
   .map_err(Error::Config)?
   ```

9. **PREFER** chaining through existing variants over inventing new wrapper variants.

   ```rust
   // BAD — new top-level variant just to wrap an inner one
   #[error("inner")] InnerWrap(#[source] inner::Error),
   // GOOD — reuse via From
   .ok_or(top::Error::from(inner::Error::Foo(ctx)))?
   ```

10. **MUST NOT** put `#[from]` on context-less variants. `#[from]` is allowed only
    when the source error already carries the operation context.

    ```rust
    // BAD — every SQL collapses to one variant
    #[error("db")] Db(#[from] sqlx::Error),
    // GOOD
    #[error("load user {id}")]
    LoadUser { id: UserId, #[source] source: sqlx::Error },
    ```

11. **MUST NOT** hand-write `impl From<other::Error> for Error`. Use `#[source]`
    or `#[from]` only.

12. **PREFER** `#[error(transparent)]` + `#[from]` only when the inner error is the
    whole story (re-export wrappers).

### Logging mid-flow

13. **PREFER** `errortools::FormatError` when you cannot return — never walk
    `source()` by hand.

    ```rust
    // BAD
    let mut cur: &dyn Error = &e;
    while let Some(s) = cur.source() { /* … */ }
    // GOOD
    use errortools::FormatError;
    tracing::error!("do_thing: {}", e.one_line());
    ```

### Returning from `main`

14. **MUST** use `fn main() -> MainResult<Error>` (or `MainResult<Error, Chain>`).
    The strategy renders the chain via `Display`; `Debug` never reaches stderr.

    ```rust
    // BAD
    fn main() -> Result<(), Error> { … }
    // GOOD
    fn main() -> errortools::MainResult<Error> { … }
    ```

15. **MUST** confine `exit(1)` and `panic!()` to `main`. Business logic returns
    `Result`.

16. **MUST** do graceful shutdown in `main` (join threads, close connections).
    **AVOID** calling `drop(v)` manually — rely on scope.

### Panics

17. **MUST NOT** `unwrap()` / `expect()` in production or library code. If
    unavoidable, document it under `# Panics`.

    ```rust
    // BAD
    let cfg = load().unwrap();
    // GOOD
    let cfg = load().map_err(config::Error::Config)?;
    ```

### Batch operations

18. **MUST NOT** silently skip failed items unless the API contract says so — fail
    fast, or collect and report per-item errors. `ManyErrors` is the tool for the
    collect-and-report case: push `(context, error)` pairs (or `push_group` for
    nesting), then `into_result(ok)`. See `references/many-errors.md`.

    ```rust
    // BAD — swallow failures
    for item in items { let _ = process(item); }
    // GOOD
    let mut errs = ManyErrors::new();
    for item in &items {
        if let Err(e) = process(item) { errs.push(item.id, e); }
    }
    errs.into_result(())?;
    ```

### `anyhow` / `Box<dyn Error>`

19. **AVOID** `anyhow` or other dynamic error types outside tests / throwaway
    scripts. Production code uses explicit `thiserror` enums.

### Tests

20. **MUST** assert the exact error variant with arguments, not `.is_err()`.

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
| Interactive terminals, deep chains | `Chain` (or `Chain<Ascii>`) |
| Structured logs (JSON, OpenTelemetry) | `OneLine` — one log line per error |
| A batch of independent failures | `Tree` / `List` / `Bullets` / `Joined` (`ManyErrors`) |
| Error + recovery hint | `WithNewLine<OneLine, Suggestion>` |
| Project house style | custom `Format` impl, applied uniformly |

Switch globally by changing the type parameter on `MainResult` — no call sites
change. Details and composition in `references/formatting.md`.

## References

- README: <https://github.com/maxwase/errortools/blob/master/README.md>
- Runnable examples: <https://github.com/maxwase/errortools/tree/master/examples>
  (`one_line`, `chain`, `format_error`, `custom_format`, `transparent`,
  `with_context`, `many_errors`)
- API docs: <https://docs.rs/errortools>
