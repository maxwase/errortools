---
name: using-errortools
description: >
  How to use the errortools crate (0.4) to report, tag, aggregate and transport
  Rust errors. Use whenever a task involves returning errors from main
  (including async main), attaching a path / ID / attempt to an error with
  .context(), logging an error chain, collecting failures from a batch or a set
  of tasks, sending an error across a thread, task or channel, "did you mean"
  hints, custom error formats, or upgrading errortools 0.3 code. Trigger on:
  MainResult, MainError, ResultExt, .context(, .with_context(, WithContext,
  WithPath, FormatError, one_line(), chain(), formatted(), detach(), Detached,
  ManyErrors, collect_results, Format trait, OneLine, Chain, Tree, Suggest,
  Suggestion, Then, Lit, sources() -- even when only one is mentioned. See
  structured-error-handling for designing the error types themselves.
---

# using-errortools

The runtime surface of `errortools` 0.4. Error type design (`#[source]` vs
`#[from]`, message wording) is in `structured-error-handling`. Everything named
here is exported from the crate root unless a module path is shown.

| Need | Reach for |
|---|---|
| Report an error from `main` | `MainResult<E, F = OneLine, T = ()>` |
| Attach a path, ID or attempt to an error | `.context(c)?` / `.with_context(\|\| c)?` (`ResultExt`) producing `WithContext<C, E>` / `WithPath<C, E>` |
| Print a chain where you cannot return it | `FormatError`: `.one_line()`, `.chain()`, `.formatted::<F>()` |
| Move an error across a thread, task or channel | `.detach()` producing `Detached` |
| Report every failure of a batch | `ManyErrors<C, E>`, `ManyErrors::collect_results` |
| Show a recovery hint | `Suggest` + `MainResultWithSuggestion<E>` |
| House format | a tuple of strategies, or `impl Format<E>` |

| Detail | File |
|---|---|
| Strategies, tuples, `Then`, custom `Format`, `Formatted` vs `FormattedRef`, connectors | `references/formatting.md` |
| `WithContext` nesting, custom pair formats, inference failures | `references/with-context.md` |
| `ManyErrors` building, groups, render shapes, custom shapes | `references/many-errors.md` |
| `Suggest` hints | `references/suggestions.md` |
| 0.3 code that no longer compiles | `references/upgrading-to-0.4.md` |

## `main` returns `MainResult`

`fn main() -> Result<(), E>` prints the error with `Debug`. `MainResult` prints
it through a format strategy instead and exits with code 1. `?` converts any
error `S` with `E: From<S>` in one hop, so `main` needs no `map_err` for errors
the application error has a `#[from]` for.

```rust
use errortools::{MainResult, ResultExt, WithPath};
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Failed to load config")]
    Config(#[from] WithPath<&'static str, io::Error>),
}

fn main() -> MainResult<Error> {
    let path = "missing.toml";
    let _config = fs::read_to_string(path).context(path)?;
    Ok(())
}
```

```text
Error: Failed to load config: missing.toml: No such file or directory (os error 2)
```

- The second parameter picks the strategy for the whole program:
  `MainResult<Error, Chain>` prints the indented ladder. No call site changes.
- The third is the success type: `MainResult<Error, OneLine, ExitCode>`.
- Async works with any runtime, because the wrappers are `Send + Sync`:
  `#[tokio::main] async fn main() -> MainResult<Error>`.
- Keep `exit` and `panic!` out of business logic. Return `Result` up to `main`
  so destructors run and there is one place that reports.

## Attaching context: `.context(..)?`

A path, record ID or attempt number that callers will not `match` on belongs in
a `WithContext`, not in a new variant. `.context(c)` pairs the error with `c`;
`.with_context(|| c)` builds `c` only on failure (use it when `c` allocates).
It renders `"<context>: <error>"`, and its `source()` skips the inner error, so
chain renderers never print the error twice.

The pair's format (`Colon` for `Display` contexts, `PathColon` for paths) is
never written at the call site. It is inferred from where the value lands:

```rust
#[derive(Debug, thiserror::Error)]
enum Error {
    // One operation yields this pair type: #[from], so `?` is enough.
    #[error("Failed to open input")]
    Open(#[from] WithPath<PathBuf, io::Error>),
    // A second operation with the same pair type: name the variant.
    #[error("Failed to open output")]
    OpenOutput(#[source] WithPath<PathBuf, io::Error>),
}

let i = File::open(input).with_context(|| input.to_owned())?;
let o = File::open(output).with_context(|| output.to_owned()).map_err(Error::OpenOutput)?;
```

```text
Failed to open input: in.txt: No such file or directory (os error 2)
```

Use `WithPath<C, E>` whenever the context is a `Path`/`PathBuf`: they have no
`Display`, and `WithPath` renders them with `Path::display`. If `.context(..)?`
fails with "type annotations needed" or "`From<WithContext<..>>` is not
implemented", see `references/with-context.md`.

## Logging where you cannot return

Inside a retry loop or an event handler the error cannot propagate. Render it
with `FormatError` (implemented for every `Error`, including `dyn Error`)
instead of walking `source()` by hand, and only there: an error that can be
returned should be.

```rust
use errortools::FormatError;

if let Err(e) = do_thing() {
    tracing::error!("do_thing failed: {}", e.one_line()); // a: b: c
    eprintln!("{}", e.chain());                           // indented ladder
}
```

These borrow the error (`FormattedRef`), so the error stays usable afterwards.

## Crossing a thread, task or channel: `Detached`

A typed error is often not `Send`, not `Clone`, or not `'static`. `detach()`
snapshots the `Display` text of every level into a `Detached`, which is
`Send + Sync + 'static + Clone` and is itself an `Error` with the same chain,
so it renders identically and fits a `#[source]` field.

```rust
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Worker failed")]
    Worker(#[source] Detached),
    #[error("Failed to join worker")]
    Join(#[source] tokio::task::JoinError),
}

let worker = tokio::spawn(async { "x".parse::<u32>().map_err(|e| JobError(e).detach()) });
worker.await.map_err(Error::Join)?.map_err(Error::Worker)?;
```

```text
Error: Worker failed: Failed to parse job: invalid digit found in string
```

The snapshot keeps text only, so the receiver cannot `match` on variants. When
the error type is already `Send + 'static`, send it as is and keep the types.
`Detached::new(msg).with_source(..)` rebuilds a chain from messages (for
example one received over the wire); `.message()` and `.cause()` read it back.

## Batches: `ManyErrors`

Do not stop at the first failure of independent items and do not skip failures.
`collect_results` takes `(context, Result)` pairs and returns every `Ok` value
or the aggregate of every failure. It fits `join_all` output or a drained
`JoinSet`.

```rust
#[derive(Debug, thiserror::Error)]
enum Error {
    // No #[source]: an aggregate has no single cause. Render it in the message.
    #[error("Deploy failed:\n{}", .0.tree())]
    Deploy(ManyErrors<&'static str, DeployError>),
}

fn deploy_all(hosts: &[&'static str]) -> Result<Vec<u32>, Error> {
    ManyErrors::collect_results(hosts.iter().map(|&h| (h, deploy(h)))).map_err(Error::Deploy)
}
```

```text
Deploy failed:
2 errors:
├─ i-0b2: Failed to upload artifact
│  └─ disk full
└─ i-0c3: Connection refused
```

When there are no `Ok` values to keep, collect the pairs and call
`.into_result(())`. Shapes (`tree`, `list`, `bullets`, `one_line`), groups and
merging are in `references/many-errors.md`.

## Choosing a format

| Output | Strategy |
|---|---|
| CLI default, structured logs | `OneLine`: `a: b: c`, one greppable line |
| Interactive terminal, deep chains | `Chain` (`Chain<Ascii>` without box glyphs) |
| Error plus hint | `MainResultWithSuggestion<E>` / `WithSuggestion<F, Sep>` |
| Aggregate | `tree()` multi-line, `one_line()` single line |
| House style | a tuple such as `(Chain, Then<NewLine, Suggestion>)`, or `impl Format<E>` |

Define a house format once as a `type` alias and name it in `MainResult` and
`formatted::<F>()`, so the style changes in one place.

## Pitfalls

- Printing a source inside its parent's message (`#[error("Read failed: {0}")]`
  with `#[source]`) prints it twice under every strategy.
- `{e}` on any error prints the top message only. Use `.one_line()` or
  `.chain()` when the chain is wanted.
- On a concrete `ManyErrors`, `.one_line()` and `.chain()` render every child.
  Behind `dyn Error` or as a `#[source]` it shows a one-line summary without
  the children's sources, so embed it in the variant message as above.
- `ColonChar` / `ColonSpace` are separators; `with_context::Colon` is the pair
  strategy. They are not interchangeable.
- An unsupported strategy and value pair fails where the wrapper is displayed,
  with a message such as "the strategy `X` cannot format `Y`" or "`Y` has no
  suggestion to render". Fix the strategy or add the missing impl; do not
  reach for `to_string()` workarounds.
