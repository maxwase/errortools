# errortools

[![crates](https://img.shields.io/crates/v/errortools?style=for-the-badge)](https://crates.io/crates/errortools)
[![doc](https://img.shields.io/docsrs/errortools?style=for-the-badge)](https://docs.rs/errortools/latest/)

Tired of writing this in every project?

```rust,ignore
fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), MyError> { todo!() }
```

Because returning `Result` from `main` uses `Debug`, which gives you this:

```text
Error: Outer(Inner(Io(Os { code: 2, kind: NotFound, message: "No such file or directory" })))
```

We have a solution: **`MainResult`**.

## Example

```rust,no_run
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

The error and its full source chain print joined with `": "`. No `run()` wrapper, no manual loop.

## Tree format

Prefer a multi-line view? Swap the format strategy:

```rust,no_run
use errortools::{MainResult, Tree};
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("failed to load config")]
    Config(#[source] io::Error),
}

fn main() -> MainResult<AppError, Tree> {
    let _ = fs::read_to_string("missing.toml").map_err(AppError::Config)?;
    Ok(())
}
```

```text
Error: failed to load config
└── No such file or directory (os error 2)
```

## Adding context

Ever needed to wrap `io::Error` just to attach a path? Or keep a retry attempt around? That's what `WithContext<C, E>` is for. No more ad-hoc single-variant wrappers that mess up error chains. `WithContext` holds a context value next to an error. The pair displays through whatever strategy you pick: `Colon` by default, `PathColon` if the context is a path. `FormatError` skips the wrapped error itself when it walks the chain, so it never shows up twice.

`PathColon` calls `Path::display` for you, so `&Path` and `PathBuf` go
straight in. The `WithPath` alias names the type:

```rust,no_run
use errortools::{MainResult, WithContext, with_context::WithPath};
use std::{fs::File, io, path::Path};

#[derive(Debug, thiserror::Error)]
#[error("failed to create file")]
struct Error(#[from] WithPath<&'static Path, io::Error>);

fn main() -> MainResult<Error> {
    let path = Path::new("no/such/dir/foo.txt");
    File::create(path).map_err(|e| Error::from(WithContext::new(path, e)))?;
    Ok(())
}
```

```text
Error: failed to create file: no/such/dir/foo.txt: No such file or directory (os error 2)
```

Retry attempt numbers fit too. The default `Colon` strategy takes any
`Display` pair, and `usize` is `Display`:

```rust,ignore
fn create_with_retry(
    path: &Path,
    attempts: NonZeroUsize,
) -> Result<File, WithContext<usize, io::Error>> {
    let last = attempts.get();
    for _ in 1..last {
        if let Ok(f) = File::create(path) { return Ok(f); }
    }
    File::create(path).map_err(|e| WithContext::new(last, e))
}
```

You can nest the two: wrap a `WithContext<usize, io::Error>` inside a `WithPath<&Path, WithContext<usize, io::Error>>` and the chain prints `<path>: <attempt>: <io error>`.
The [`with_context`](https://github.com/maxwase/errortools/blob/master/examples/with_context.rs) example shows that through `MainResult` end-to-end.

Need a different look? Implement `ContextFormat<C, E>` (re-exported as
`errortools::with_context::ContextFormat`) on a unit type and plug it in
with `WithContext<C, E, MyFmt>`. The bounds are yours: `Colon` asks for
`Display`, `PathColon` asks for `AsRef<Path>`, you ask for whatever you
need.

## But why?

Countless hours of debugging with unordered error and debug logs that *may* mention the needed context (such as a path), simply because it felt like too much effort to write a wrapper type just to add it.

### My strong point

**It must be possible to pinpoint the exact location of an error from a single, perhaps rather long but informative, error message.**


## Logging in place

Sometimes you cannot return and need to log the full source chain right where
the error happens. The `FormatError` extension trait works on any error:

```rust,ignore
use errortools::FormatError;

if let Err(e) = do_thing() {
    tracing::error!("do_thing failed: {}", e.one_line());
    // do_thing failed: outer: middle: inner
}
```

For ad-hoc strategies, pick the format inline with `formatted::<F>()`:

```rust,ignore
use errortools::{FormatError, Tree};

if let Err(e) = do_thing() {
    eprintln!("{}", e.formatted::<Tree>());
    // outer
    // └── middle
    //     └── inner
}
```

## Custom formats

Implement the `Format<E>` trait on a unit type. `E` is generic so your strategy can require extra bounds on the error type (e.g. `Suggest` for the suggestion strategy):

```rust,ignore
use core::{error::Error, fmt};
use errortools::{Format, FormatError, chain};
use itertools::Itertools;

struct Arrow;
impl<E: Error + ?Sized> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(&error).format(" -> "))
    }
}

println!("{}", my_error.formatted::<Arrow>()); // outer -> middle -> inner
```

## Suggestions

For "Did you mean…" hints, implement `Suggest` on your error type and call
`error.suggestion()`:

```rust,ignore
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
            Self::Network => Ok(()),
        }
    }
}

eprintln!("{}\n{}", Error::NoConfig.one_line(), Error::NoConfig.suggestion());
// Config file missing
// Did you copy config.example.toml to config.toml?
```

Only the top-level error's hint is printed, the source chain isn't walked. This decision is intentional: The underlying hint may be irrelevant in the context of the top-level error, and printing it may just add noise.

The idea is that every error that is supposed to have a suggestion should implement `Suggest` and then later the top-level error's suggestion may concatenate the inner hint if it's relevant with nesting matching the error chain.

## How it works

`MainResult<E, F>` is a type alias:

```rust
use errortools::{DisplaySwapDebug, Formatted, OneLine};

pub type MainResult<E, F = OneLine, T = ()> = Result<T, DisplaySwapDebug<Formatted<E, F>>>;
```

`DisplaySwapDebug` swaps the `Debug` and `Display` impls of its inner type. When `main` prints the error via `Debug`, it ends up reaching the `Display` output instead, formatted by the chosen strategy. `?` converts your error automatically via the blanket `From` impl.

## Examples

Runnable examples in [`examples/`](https://github.com/maxwase/errortools/tree/master/examples):

| Example | What it shows |
|---|---|
| [`one_line`](https://github.com/maxwase/errortools/blob/master/examples/one_line.rs) | `MainResult` with default `OneLine` format |
| [`tree`](https://github.com/maxwase/errortools/blob/master/examples/tree.rs) | `MainResult<E, Tree>` for indented multi-line output |
| [`format_error`](https://github.com/maxwase/errortools/blob/master/examples/format_error.rs) | `FormatError` trait for ad-hoc formatting |
| [`custom_format`](https://github.com/maxwase/errortools/blob/master/examples/custom_format.rs) | A custom `Format` strategy |
| [`transparent`](https://github.com/maxwase/errortools/blob/master/examples/transparent.rs) | `#[error(transparent)]` pass-through with `#[from]` |
| [`with_context`](https://github.com/maxwase/errortools/blob/master/examples/with_context.rs) | `WithContext` tags an inner error with a context value, lifted via `#[from]` |

Run with: `cargo run --example <name>`.

## Features

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `itertools/use_std`. Disable for `no_std`. |
