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

## Chain format

Prefer a multi-line indented view of the source chain? Swap the format strategy:

```rust,no_run
use errortools::{Chain, MainResult};
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("failed to load config")]
    Config(#[source] io::Error),
}

fn main() -> MainResult<AppError, Chain> {
    let _ = fs::read_to_string("missing.toml").map_err(AppError::Config)?;
    Ok(())
}
```

```text
Error: failed to load config
└─ No such file or directory (os error 2)
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

Need a different look? `WithContext` formats through any `F: Format<WithContext<C, E, F>>`,
so there are two ways to customize it:

1. **Compose** with the built-in field extractors and separators:
   `type SpacePair = WithSpace<ContextField, ErrorField>;` swaps `": "`
   for a single space. Same recipe for any delimiter you can write as a
   `Format` tag.
2. **Write a one-shot impl** when the layout is unusual:
   `impl<C: Display, E: Display, F> Format<WithContext<C, E, F>> for MyFmt { ... }`.
   You declare your own bounds — `Colon` asks for `Display`, `PathColon` asks
   for `AsRef<Path>`, you ask for whatever you need.

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
use errortools::{Chain, FormatError};

if let Err(e) = do_thing() {
    eprintln!("{}", e.formatted::<Chain>());
    // outer
    // └─ middle
    //    └─ inner
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

## Combining strategies

`Add<L, R>` glues two `Format` strategies together. Both run against the same value, left then right. There's no built-in separator, drop a separator strategy (`NewLine`, `Space`, `Colon`, `ColonSpace`, `Empty`) in between, or reach for the three-arg `WithSep<L, Sep, R>` alias when you'd otherwise nest:

```rust,ignore
use errortools::{Formatted, OneLine, Suggestion, separator::{NewLine, WithSep}};

// Same as Add<Add<OneLine, NewLine>, Suggestion>. Renders:
// "<one-line chain>\n<top-level suggestion>"
type Brief = WithSep<OneLine, NewLine, Suggestion>;

eprintln!("{}", Formatted::<_, Brief>::new(err));
```

For the common separators there are zero-think aliases — `WithSpace<L, R>`,
`WithNewLine<L, R>`, `WithColonSpace<L, R>` — all in `errortools::separator`:

```rust,ignore
use errortools::{Formatted, OneLine, Suggestion, separator::WithNewLine};

type Brief = WithNewLine<OneLine, Suggestion>;
eprintln!("{}", Formatted::<_, Brief>::new(err));
```

Bounds compose: `Add<OneLine, Suggestion>` only implements `Format<E>` when
`E: Error + Suggest`, because `Suggestion`'s impl carries that bound.

The same combinator powers the `WithContext` default — `Colon` is just a type
alias for `WithColonSpace<ContextField, ErrorField>`, where
`ContextField`/`ErrorField` are extractor strategies that read the pair's
fields. To get a different delimiter, swap one piece:

```rust,ignore
use errortools::{WithContext, separator::WithSpace, with_context::{ContextField, ErrorField}};

type SpacePair = WithSpace<ContextField, ErrorField>;
let w = WithContext::<_, _, SpacePair>::new("step", "boom");
assert_eq!(w.to_string(), "step boom");
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

## Many errors at once

Some operations shouldn't stop at the first failure — validating a config, deploying to every region, parsing a batch. You want all of them, grouped and readable. That's `ManyErrors<C, E>`: a context-tagged collection you can render as a tree, list, or single line.

```rust,ignore
use errortools::ManyErrors;

let mut errs = ManyErrors::new();
errs.push("eu-west-1", RegionError::Refused);
errs.push("us-east-1", RegionError::Timeout);

errs.into_result(())?; // Ok if empty, Err(ManyErrors) otherwise
```

It costs nothing until it has to: `None` while empty, one inline slot for the first error, a `Vec` only once a second arrives. You can also collect straight from an iterator of `(context, error)` pairs or `WithContext` values — including itertools' `partition_result`.

Group related failures with `push_group` and the shapes nest. `tree()` gives the Unicode tree, walking each error's source chain:

```text
2 errors:
├─ us-east-1 (2 errors):
│  ├─ i-0a1: connection refused
│  └─ i-0b2: timed out: network partition
└─ eu-west-1: connection refused
```

The default `Display` (`{errs}`) is deliberately a shallow one-line *summary* — each error's own text, no source chains — so it's safe to embed in a message or log, following the Rust convention that an error's `Display` is its own message:

```text
2 errors: us-east-1 (2 errors: i-0a1: connection refused; i-0b2: timed out); eu-west-1: connection refused
```

For the full picture, the shapes are inherent helpers, no turbofish — `tree()` and `joined()` walk the source chains, `list()` and `bullets()` too:

```rust,ignore
println!("{}", errs.tree());      // Unicode tree (above)
println!("{}", errs.list());      // 1.  1.1.  2.
println!("{}", errs.bullets());   // • bulleted
println!("{}", errs.joined());    // ;-separated one line, parens around groups
```

For full control — ASCII connectors, no count header — go through `formatted`: `Formatted::<_, Tree<Ascii, false>>::new(&errs)`.

Group labels can differ from leaf contexts via the third parameter, `ManyErrors<C, E, GC>`, but `GC` defaults to `C`, so the common case stays two params.

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
| [`tree`](https://github.com/maxwase/errortools/blob/master/examples/tree.rs) | `MainResult<E, Chain>` for indented multi-line output |
| [`format_error`](https://github.com/maxwase/errortools/blob/master/examples/format_error.rs) | `FormatError` trait for ad-hoc formatting |
| [`custom_format`](https://github.com/maxwase/errortools/blob/master/examples/custom_format.rs) | A custom `Format` strategy |
| [`transparent`](https://github.com/maxwase/errortools/blob/master/examples/transparent.rs) | `#[error(transparent)]` pass-through with `#[from]` |
| [`with_context`](https://github.com/maxwase/errortools/blob/master/examples/with_context.rs) | `WithContext` tags an inner error with a context value, lifted via `#[from]` |
| [`many_errors`](https://github.com/maxwase/errortools/blob/master/examples/many_errors.rs) | `ManyErrors` collects nested, context-tagged failures and renders them as a tree |

Run with: `cargo run --example <name>`.

## Features

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Enables `itertools/use_std`. Disable for `no_std`. |
