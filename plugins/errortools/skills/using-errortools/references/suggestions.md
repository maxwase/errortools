# Suggestions: "Did you mean..." hints

Read this when an error should carry a recovery hint for the user ("copy
`config.example.toml` to `config.toml`", "pass `--help`", "check the path
exists") separate from the error message itself.

Implement the `Suggest` trait on your error type to give per-variant hints, then
render them with `error.suggestion()` (or the `Suggestion` `Format` strategy).

```rust
use core::fmt;
use errortools::{FormatError, Suggest};

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
            Self::NoConfig => f.write_str("Did you copy config.example.toml to config.toml?"),
            Self::Network => Ok(()), // no hint for this variant
        }
    }
}

eprintln!("{}\n{}", Error::NoConfig.one_line(), Error::NoConfig.suggestion());
// Config file missing
// Did you copy config.example.toml to config.toml?
```

`Suggest::fmt` defaults to writing nothing, so a type only needs arms for the
variants that actually have a hint.

## Top-level only, by design

`error.suggestion()` prints **only the top-level error's** hint; the source
chain is not walked. This is intentional: an inner error's hint is often
irrelevant once it has been wrapped, and printing it would just add noise. The
convention is that each error that should suggest something implements `Suggest`,
and a top-level error's hint can deliberately concatenate an inner hint when it's
relevant, with nesting that matches the error chain.

`Suggest` is dispatched on the concrete outer type, so it is **not** delegated
through `#[error(transparent)]` either: `transparent` collapses `Display` and
`source`, but the outer type's `Suggest` impl always wins.

## Pairing the error with its hint

A bare `error.suggestion()` is just the hint. To show the error *and* its hint,
compose the two strategies with `Add` (see `formatting.md`). The
`WithNewLine<OneLine, Suggestion>` alias puts the hint on its own line:

```rust
use errortools::{Formatted, OneLine, Suggestion, separator::WithNewLine};

type Brief = WithNewLine<OneLine, Suggestion>;
eprintln!("{}", Formatted::<_, Brief>::new(err));
// <one-line chain>
// <top-level suggestion>
```

`Add` writes both sides unconditionally, so a variant with no hint still emits
the separator (a trailing newline), predictable for line-oriented output.

## In `main`: `MainResultWithSuggestion`

To print the error and its hint straight out of `main`, return
`MainResultWithSuggestion<E>` instead of `MainResult<E>`. It is
`MainResult<E, WithSuggestion<F, NewLine>>`: the error via `F` (default
`OneLine`), a newline, then the top-level suggestion:

```rust
use errortools::{MainResultWithSuggestion, Suggest};

fn main() -> MainResultWithSuggestion<Error> {
    do_work()?; // any Err with Suggest renders as "<chain>\n<hint>"
    Ok(())
}
```

Customize the layout with the `WithSuggestion<F, Sep>` type alias (e.g. a space
separator, or a different error strategy `F`), or drop down to `MainResult<E, _>`
with your own `Add`-composed strategy. The `T` success generic still applies, so
`MainResultWithSuggestion<E, OneLine, ExitCode>` works too.
