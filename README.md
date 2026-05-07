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

The error and its full source chain are joined with `": "` — no boilerplate, no `run()` wrapper, no manual loop.

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

Implement the `Format` trait on a unit type:

```rust,ignore
use core::{error::Error, fmt};
use errortools::{Format, FormatError, chain};
use itertools::Itertools;

struct Arrow;
impl Format for Arrow {
    fn fmt(error: &dyn Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(error).format(" -> "))
    }
}

println!("{}", my_error.formatted::<Arrow>()); // outer -> middle -> inner
```

## How it works

`MainResult<E, F>` is a type alias:

```rust
use errortools::{DisplaySwapDebug, Formatted, OneLine};

pub type MainResult<E, F = OneLine> = Result<(), DisplaySwapDebug<Formatted<E, F>>>;
```

`DisplaySwapDebug` swaps the `Debug` and `Display` impls of its inner type, so when `main` prints the error via `Debug`, you actually get its `Display` output — formatted by the chosen strategy. `?` converts your error automatically via the blanket `From` impl.

## Examples

Runnable examples in [`examples/`](https://github.com/maxwase/errortools/tree/master/examples):

| Example | What it shows |
|---|---|
| [`one_line`](https://github.com/maxwase/errortools/blob/master/examples/one_line.rs) | `MainResult` with default `OneLine` format |
| [`tree`](https://github.com/maxwase/errortools/blob/master/examples/tree.rs) | `MainResult<E, Tree>` for indented multi-line output |
| [`format_error`](https://github.com/maxwase/errortools/blob/master/examples/format_error.rs) | `FormatError` trait for ad-hoc formatting |
| [`custom_format`](https://github.com/maxwase/errortools/blob/master/examples/custom_format.rs) | A custom `Format` strategy |
| [`transparent`](https://github.com/maxwase/errortools/blob/master/examples/transparent.rs) | `#[error(transparent)]` pass-through with `#[from]` |

Run with: `cargo run --example <name>`.

## Features

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `itertools/use_std`. Disable for `no_std`. |
