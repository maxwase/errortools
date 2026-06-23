# Attaching context: `WithContext`

Read this when an error needs a value carried alongside it (a file path, a step
number, a retry attempt, a record ID) and inventing a one-off wrapper variant
just to hold that value would be noise.

`WithContext<C, E, F = Colon>` pairs a context value `C` with an error `E` and
renders the pair through a `Format` strategy `F`. It is the idiomatic
alternative to single-variant wrapper enums whose only job is to staple a path
onto an `io::Error`.

The key behaviour: `WithContext`'s `Error::source()` returns the **inner
error's** source, skipping the inner error itself (its `Display` already prints
it). So chain-walking strategies (`OneLine`, `Chain`, the aggregate shapes)
never print the wrapped error twice.

## The default `Colon` strategy

`Colon` renders `"<context>: <error>"`. Use the `WithContextColon` alias to get
type inference on `new` without a turbofish:

```rust
use errortools::{FormatError, with_context::WithContextColon};
use std::io;

let err = io::Error::new(io::ErrorKind::NotFound, "file missing");
let ctx = WithContextColon::new("path/to/config", err);
assert_eq!(ctx.one_line().to_string(), "path/to/config: file missing");
```

Any `Display` context works. A retry attempt number, for instance:

```rust
use errortools::WithContext;
use std::{fs::File, io, path::Path, num::NonZeroUsize};

fn create_with_retry(
    path: &Path,
    attempts: NonZeroUsize,
) -> Result<File, WithContext<usize, io::Error>> {
    let last = attempts.get();
    for _ in 1..last {
        if let Ok(f) = File::create(path) { return Ok(f); }
    }
    // Tag the final failure with its attempt number → "<attempt>: <io error>".
    File::create(path).map_err(|e| WithContext::new(last, e))
}
```

## Paths: `PathColon` and `WithPath`

`PathColon` (std only) calls `Path::display` for you, so `&Path`/`PathBuf` go
straight in without a `Display` newtype. `WithPath<C, E>` names the path case
(`= WithContext<C, E, PathColon>`). Lift it into your error enum with `#[from]`:

```rust
use errortools::{MainResult, WithContext, with_context::WithPath};
use std::{fs::File, io, path::Path};

#[derive(Debug, thiserror::Error)]
#[error("Failed to create file")]
struct Error(#[from] WithPath<&'static Path, io::Error>);

fn main() -> MainResult<Error> {
    let path = Path::new("no/such/dir/foo.txt");
    File::create(path).map_err(|e| Error::from(WithContext::new(path, e)))?;
    Ok(())
}
```

```text
Error: Failed to create file: no/such/dir/foo.txt: No such file or directory (os error 2)
```

`#[from]` is what pins the format parameter: `WithContext::new(path, e)` infers
`PathColon` from the target `WithPath<...>`, no turbofish needed.

## Nesting context layers

Layers nest, and the chain reads outside-in. Wrap a
`WithContext<usize, io::Error>` (attempt) inside a `WithPath<&Path, ...>` (path)
and you get `"<path>: <attempt>: <io error>"`:

```rust
#[derive(Debug, thiserror::Error)]
enum FsError {
    #[error("Failed to create file")]
    Create(#[source] WithPath<std::path::PathBuf, WithContext<usize, std::io::Error>>),
}
```

The crate's [`with_context`](https://github.com/maxwase/errortools/blob/master/examples/with_context.rs)
example threads this end-to-end through `MainResult`.

## Customizing the rendering

`WithContext` formats through any `F: Format<WithContext<C, E, F>>`. Two ways to
change the look:

**1. Compose field extractors with a separator.** `Colon` is just
`WithColonSpace<ContextField, ErrorField>`. Swap the separator to change the
delimiter, since every extractor/separator is a `Format` tag:

```rust
use errortools::{WithContext, separator::WithSpace, with_context::{ContextField, ErrorField}};

// Same as Colon but a single space instead of ": ".
type SpacePair = WithSpace<ContextField, ErrorField>;
let w = WithContext::<_, _, SpacePair>::new("step", "boom");
assert_eq!(w.to_string(), "step boom");
```

The extractors: `ContextField` reads `w.context`, `ErrorField` reads `w.error`,
and `ContextPath` (std only) reads `w.context` via `Path::display`.

**2. Write a one-shot impl** when the layout is unusual. Pull fields off the
`&WithContext` directly and declare whatever bounds you need:

```rust
use core::fmt::{self, Display, Formatter};
use errortools::{Format, WithContext};

struct Arrow;
impl<C: Display, E: Display, WCF> Format<WithContext<C, E, WCF>> for Arrow {
    fn fmt(w: &WithContext<C, E, WCF>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", w.context, w.error)
    }
}

let w = WithContext::<_, _, Arrow>::new(1, "boom");
assert_eq!(w.to_string(), "1 -> boom");
```

> **Render the error's own text.** Because `Error::source` deliberately skips the
> inner error, chain-walking renderers assume your strategy already printed
> `w.error`. A strategy that omits it (context-only) silently drops the error
> text from every deep rendering.

`with_format::<NewF>()` switches the strategy on an existing value without
touching the stored fields. `From<(C, E)>` lets a `(context, error)` tuple
become a `WithContext` directly, handy with iterator adapters.

## When to reach for it (and when not)

- **Reach for it** instead of a single-variant wrapper struct/enum whose only
  purpose is to attach a path/ID/attempt to a foreign error. This keeps the
  source chain honest and avoids variant sprawl.
- **May use a real variant** when the context *is* the operation's identity
  and you'll match on it (e.g. `LoadUser { id, #[source] source }`). Context
  that callers branch on belongs in the enum; context that's purely for the
  message belongs in `WithContext`.
