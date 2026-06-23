# Formatting & rendering strategies

How `errortools` turns an error and its source chain into text. Read this when
choosing how `main` reports an error, how you log a chain mid-flow, or when you
need a project-specific format.

The mental model: a **`Format<E>` strategy** is a unit type that knows how to
write some value `E` to a `fmt::Formatter`. You rarely instantiate one — you
name it as a type parameter (`MainResult<E, Chain>`) or hand it to a wrapper
(`e.formatted::<Chain>()`). `Formatted<E, F>` is the wrapper whose `Display`
runs strategy `F` over `E`.

## Returning from `main`: `MainResult`

`MainResult<E, F = OneLine, T = ()>` is a drop-in `Result` for `fn main`. When
`main` returns `Err`, the runtime prints it via `Debug` — `MainResult` arranges
for that `Debug` print to emit `F`'s `Display`-style output instead of the raw
derived `Debug`. `?` converts your error in automatically.

```rust
use errortools::MainResult;
use std::{fs, io};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("failed to load config")]
    Config(#[source] ConfigError),
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("failed to read file")]
    Read(#[source] io::Error),
}

fn main() -> MainResult<AppError> {
    fs::read_to_string("does-not-exist.toml")
        .map_err(ConfigError::Read)
        .map_err(AppError::Config)?;
    Ok(())
}
```

```text
Error: failed to load config: failed to read file: No such file or directory (os error 2)
```

Swap the strategy with the second type parameter — no call sites change:

```rust
fn main() -> errortools::MainResult<AppError, errortools::Chain> { /* … */ }
```

```text
Error: failed to load config
└─ failed to read file
   └─ No such file or directory (os error 2)
```

The third parameter `T` is the success type (default `()`). Return an
`ExitCode` (or any `Termination`) when you need a custom exit status:

```rust
use std::process::ExitCode;
fn main() -> errortools::MainResult<AppError, errortools::OneLine, ExitCode> {
    // … on success:
    Ok(ExitCode::SUCCESS)
}
```

## Logging mid-flow: `FormatError`

When you cannot return — inside a `tokio::spawn`, an event loop, a retry — the
`FormatError` extension trait renders any `&dyn Error` on the spot. Never walk
`source()` by hand.

```rust
use errortools::FormatError;

if let Err(e) = do_thing() {
    tracing::error!("do_thing failed: {}", e.one_line());
    // do_thing failed: outer: middle: inner
}
```

`FormatError` methods:

| Method | Strategy | Output |
|---|---|---|
| `.one_line()` | `OneLine` | error + sources joined by `": "` |
| `.chain()` | `Chain` | indented source-chain ladder (`└─`) |
| `.suggestion()` | `Suggestion` | top-level "did you mean…" hint (needs `Suggest`) — see `suggestions.md` |
| `.formatted::<F>()` | any `F` | render with an arbitrary strategy |

Pick a strategy inline:

```rust
use errortools::{Chain, FormatError};

if let Err(e) = do_thing() {
    eprintln!("{}", e.formatted::<Chain>());
    // outer
    // └─ middle
    //    └─ inner
}
```

## `OneLine` vs `Chain`

| Strategy | Shape | Glyphs |
|---|---|---|
| `OneLine` | `outer: middle: inner` | — |
| `Chain<C = Unicode>` | indented ladder, one source per line | `└─ ` + `   ` (Unicode) |

`Chain` is parameterised by a `Connectors` glyph set. Use `Ascii` where Unicode
box art won't render:

```rust
use errortools::{Ascii, Chain, Formatted};
println!("{}", Formatted::<_, Chain<Ascii>>::new(err));
// outer
// `- middle
//    `- inner
```

`Connectors` exposes `LAST`/`GAP`; the branching `TreeConnectors` supertrait
adds `BRANCH`/`VERT` for the aggregate `Tree`. Both `Unicode` and `Ascii`
implement both, so `Chain<Ascii>` and `Tree<Ascii>` look consistent.

## Custom `Format` strategy

Implement `Format<E>` on a unit type. The trait imposes **no bound** on `E`
(`Format<E: ?Sized>`); a strategy that walks the source chain declares
`E: Error` itself. Walk the chain with `chain(&error)` — never call `source()`
by hand.

```rust
use core::{error::Error, fmt};
use errortools::{Format, FormatError, chain};
use itertools::Itertools; // consumer needs the `itertools` crate for `.format`

struct Arrow;

impl<E: Error + ?Sized> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `&error` is `&&E`; it coerces to `&dyn Error`.
        write!(f, "{}", chain(&error).format(" -> "))
    }
}

// fn main() -> MainResult<MyError, Arrow> { … }
println!("{}", my_error.formatted::<Arrow>()); // outer -> middle -> inner
```

Without the `itertools` dependency, fold the chain by hand:

```rust
fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, e) in chain(&error).enumerate() {
        if i > 0 { f.write_str(" -> ")?; }
        write!(f, "{e}")?;
    }
    Ok(())
}
```

Define a custom strategy once per project and reuse it everywhere via the type
parameter — there is one place to change the house style.

## Composing strategies: `Add` + separators

`Add<L, R>` runs two strategies against the same value, left then right. There
is no implicit separator — drop a separator strategy in the middle, or use the
`WithSep<L, Sep, R>` alias so the separator reads in order:

```rust
use errortools::{Formatted, OneLine, Suggestion, separator::{NewLine, WithSep}};

// Same as Add<Add<OneLine, NewLine>, Suggestion>.
// Renders "<one-line chain>\n<top-level suggestion>".
type Brief = WithSep<OneLine, NewLine, Suggestion>;
eprintln!("{}", Formatted::<_, Brief>::new(err));
```

Zero-think aliases for the common separators live in `errortools::separator`:
`WithSpace<L, R>`, `WithNewLine<L, R>`, `WithColonSpace<L, R>`. Separator tags
themselves: `NewLine`, `Space`, `Empty`, `ColonChar`, `ColonSpace`.

```rust
use errortools::{Formatted, OneLine, Suggestion, separator::WithNewLine};
type Brief = WithNewLine<OneLine, Suggestion>;
eprintln!("{}", Formatted::<_, Brief>::new(err));
```

Bounds compose automatically: `Add<OneLine, Suggestion>` only implements
`Format<E>` when `E: Error + Suggest`, because `Suggestion`'s own impl carries
the `Suggest` bound. `Add` writes both sides unconditionally — if `R` produces
nothing (a `Suggestion` variant with no hint), the separator is still written.

This same combinator powers defaults elsewhere: `WithContext`'s `Colon` is
`WithColonSpace<ContextField, ErrorField>` (see `with-context.md`).

## Choosing a strategy

| Context | Strategy |
|---|---|
| CLI tools, default | `OneLine` — single greppable line |
| Interactive terminals, deep chains | `Chain` (or `Chain<Ascii>`) |
| Structured logs (JSON, OTel) | `OneLine` — one log line per error |
| Aggregate of many failures | `Tree` / `List` / `Bullets` / `Joined` (see `many-errors.md`) |
| Error + fix hint | `WithNewLine<OneLine, Suggestion>` (see `suggestions.md`) |
| Project house style | a custom `Format` impl, applied uniformly |

## How `MainResult` works

```rust
pub type MainResult<E, F = OneLine, T = ()> =
    Result<T, DisplaySwapDebug<Formatted<E, F>>>;
```

`DisplaySwapDebug<T>` swaps a type's `Debug` and `Display` impls. `main` prints
the returned error via `Debug`; the swap routes that to the inner `Display`,
which is `Formatted<E, F>` running strategy `F`. The blanket `From<E>` impl is
what lets `?` lift your error into the wrapper.
