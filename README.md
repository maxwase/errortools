# errortools

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
    Config(#[from] io::Error),
}

fn main() -> MainResult<AppError, Tree> {
    let _ = fs::read_to_string("missing.toml").map_err(AppError::from)?;
    Ok(())
}
```

```text
Error: failed to load config
└── No such file or directory (os error 2)
```

## Use with `&dyn Error`

The `FormatError` extension trait works on any error:

```rust,ignore
use errortools::FormatError;

let e: &dyn core::error::Error = &my_error;
eprintln!("{}", e.one_line());
eprintln!("{}", e.tree());
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
