# Recovery hints: `Suggest`

Read this when an error should tell the user what to do next ("copy
`config.example.toml` to `config.toml`", "pass `--help`"), separately from the
message that says what went wrong.

Implement `Suggest` on the error type. The default body writes nothing, so only
variants with a hint need an arm.

```rust
use core::fmt;
use errortools::{MainResultWithSuggestion, Suggest};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Config file missing")]
    NoConfig,
    #[error("Network down")]
    Network,
}

impl Suggest for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfig => f.write_str("Copy config.example.toml to config.toml"),
            Self::Network => Ok(()), // no hint for this variant
        }
    }
}

fn main() -> MainResultWithSuggestion<Error> {
    Err(Error::NoConfig)?;
    Ok(())
}
```

```text
Error: Config file missing
Copy config.example.toml to config.toml
```

`Error::Network` prints `Error: Network down` with no trailing blank line.

## Rendering

| Call | Output |
|---|---|
| `fn main() -> MainResultWithSuggestion<E, F = OneLine, T = ()>` | error via `F`, newline, hint |
| `e.suggestion()` | the hint alone; empty string when there is none |
| `e.formatted::<WithSuggestion<F, Sep>>()` | error via `F`, `Sep`, hint; `Sep` is skipped when there is no hint |

`WithSuggestion<F = OneLine, Sep = NewLine>` is the tuple
`(F, Then<Sep, Suggestion>)`; use `MainResult<E, WithSuggestion<Chain>>` or
your own tuple for other layouts (see `formatting.md`).

## Only the top-level error's hint is used

`Suggestion` calls `Suggest` on the value it is given and does not walk
`source()`. An inner error's hint is often wrong once the error is wrapped, so
the outer type decides: its `Suggest` impl can call the inner one for the
variants where the inner hint still applies.

```rust
Self::Config(inner) => Suggest::fmt(inner, f),
```

This also holds for `#[error(transparent)]` variants: `transparent` forwards
`Display` and `source()`, not `Suggest`. Because dispatch is on the concrete
type, the strategies need `E: Suggest`; a type without an impl fails to compile
with "`E` has no suggestion to render", and an empty `impl Suggest for E {}` is
enough to opt in.
