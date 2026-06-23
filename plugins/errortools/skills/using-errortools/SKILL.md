---
name: using-errortools
description: >
  How to use the errortools crate for formatting and context in Rust projects.
  Use whenever a task involves MainResult, FormatError, WithContext, WithPath,
  ManyErrors, custom Format strategies, or choosing how to render an error chain
  for a terminal, log, or user message. Trigger on: MainResult, FormatError,
  one_line(), chain(), formatted(), WithContext, WithPath, ManyErrors, Format
  trait, OneLine, Chain, Suggestion -- even when only one of these is mentioned.
  See structured-error-handling for designing the error types themselves.
---

# using-errortools

This skill covers the runtime surface of the `errortools` crate: rendering error
chains, attaching incidental context without new variants, aggregating batch
failures, and defining custom format strategies. For designing error enums and
`#[source]`/`#[from]` conventions, see `structured-error-handling`.

## Core API

| Item | Purpose |
|---|---|
| `MainResult<E, F = OneLine, T = ()>` | Return type for `fn main`. Renders `E` via strategy `F` instead of `Debug`. `T` lets `main` return `ExitCode`. |
| `OneLine` | Default strategy: error + sources joined with `": "`. |
| `Chain<C = Unicode>` | Per-error indented source-chain ladder (`└─`). |
| `Tree` / `List` / `Bullets` / `Joined` | Aggregate `ManyErrors` render shapes (needs `alloc`). |
| `FormatError` | Ext trait on any `&dyn Error`: `.one_line()`, `.chain()`, `.suggestion()`, `.formatted::<F>()`. |
| `Format<E>` trait | Implement on a unit type to define a custom strategy. |
| `chain(&dyn Error)` | Iterator over the error and its `source()` chain; use inside `Format` impls. |
| `Formatted<E, F>` | Wrapper whose `Display` runs strategy `F` over `E`. |
| `WithContext<C, E, F = Colon>` / `WithPath` | Tag an error with a context value (path, attempt, ID) without a wrapper variant. |
| `ManyErrors<C, E>` | Aggregate of context-tagged failures; render as tree/list/bullets/joined. |
| `Suggest` / `Suggestion` | Per-error "did you mean..." hints via `.suggestion()`. |
| `Add<L, R>` + `separator::*` | Compose two strategies, e.g. `WithNewLine<OneLine, Suggestion>`. |
| `DisplaySwapDebug<T>` | Swaps `Debug`/`Display`; powers `MainResult`. |

### Reference files

| Need | File |
|---|---|
| Choosing/combining formats, custom `Format`, `MainResult` internals | `references/formatting.md` |
| Attaching a path / attempt / ID to an error | `references/with-context.md` |
| Collecting and reporting many failures at once | `references/many-errors.md` |
| "Did you mean..." recovery hints | `references/suggestions.md` |

## Patterns

### `MainResult` for binary entrypoints

Replace the `if let Err(e) = run() { eprintln!(...); exit(1) }` dance with a
typed return. `MainResult` renders the chain via `Display`, so `Debug` never
reaches stderr.

```rust
use errortools::{MainResult, with_context::WithPath};
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Failed to load config")]
    Config(#[source] WithPath<&'static str, io::Error>),
}

fn main() -> MainResult<Error> {
    let config_path = "missing.toml";
    fs::read_to_string(config_path).map_err(|e| Error::Config(WithPath::new(config_path, e)))?;
    Ok(())
}
```

Output:

```text
Error: Failed to load config: missing.toml: No such file or directory (os error 2)
```
with exit code 1.

For the indented ladder: `fn main() -> MainResult<Error, Chain>`.

Switch the strategy globally by changing the type parameter on `MainResult`;
no call sites change. Details and composition in `references/formatting.md`.

**Rules:**
- **MUST** use `fn main() -> MainResult<Error>`. Never `Result<(), Error>`; that
  prints `Debug`.
- **MUST** confine `exit(1)` and `panic!()` to `main`. Business logic returns
  `Result`.
- **MUST** do graceful shutdown in `main` (join threads, close connections).
  Rely on scope for drop; avoid calling `drop(v)` manually.

### Ad-hoc logging mid-function

When you cannot propagate (inside a `tokio::spawn`, an event handler, a retry
loop), use `FormatError`. Never walk `source()` by hand.
Use **ONLY** if there's no way to return a `Result` to the caller. 

```rust
use errortools::FormatError;

if let Err(e) = do_thing().await {
    tracing::error!("do_thing failed: {}", e.one_line());
}
```

```rust
eprintln!("{}", e.chain());             // indented ladder for terminals
// eprintln!("{}", e.formatted::<F>()); // any custom strategy
```

### Custom format strategy

Implement `Format<E>` once per project, reuse everywhere. A chain-walking
strategy declares `E: Error` itself and walks with `chain`; never call
`source()` directly inside a `Format` impl.

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

// fn main() -> MainResult<MyError, Arrow> { ... }
// tracing::error!("{}", e.formatted::<Arrow>());
```

See `references/formatting.md` for composing strategies with `Add`/separators
and for `Chain<Ascii>`/connectors.

### Attaching incidental context

When a path, retry count, or record ID needs to travel with an error but callers
will not branch on it, use `WithContext` / `WithPath` instead of a wrapper variant.
`WithContext`'s `source()` skips its own inner error, so the chain never doubles.

```rust
// BAD -- variant exists only to staple a path on
#[error("IO at {path}")]
IoAt { path: PathBuf, #[source] source: io::Error },

// GOOD
File::create(&path).map_err(|e| WithPath::new(path, e))?;
```

Keep context in a real variant only when callers need to match on it. See
`references/with-context.md` for full usage.

### Collecting batch failures

Never silently skip failed items. Use `ManyErrors` to push `(context, error)`
pairs and report them all at once.

```rust
let mut errs = ManyErrors::new();
for item in &items {
    if let Err(e) = process(item) { errs.push(item.id, e); }
}
errs.into_result(())?;

// BEST
items.into_iter()
    .map(|item| process(item).map_err(|e| (item.id, e)))
    .collect::<ManyErrors<_, _>>()
    .into_result(())?;
```

See `references/many-errors.md` for nesting with `push_group` and render options.

## Choosing a format strategy

| Context | Strategy |
|---|---|
| CLI tools, default | `OneLine` (single tidy line, greppable) |
| Interactive terminals, deep chains | `Chain` (or `Chain<Ascii>`) |
| Structured logs (JSON, OpenTelemetry) | `OneLine`, one log line per error |
| A batch of independent failures | `Tree` / `List` / `Bullets` / `Joined` (`ManyErrors`) |
| Error + recovery hint | `WithNewLine<OneLine, Suggestion>` |
| Project house style | custom `Format` impl, applied uniformly |

## References

- API docs: <https://docs.rs/errortools>
- Runnable examples: <https://github.com/maxwase/errortools/tree/master/examples>
  (`one_line`, `chain`, `format_error`, `custom_format`, `transparent`,
  `with_context`, `many_errors`)
