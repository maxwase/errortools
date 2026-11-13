# Attaching context: `WithContext`

Read this for nesting context layers, changing how the pair renders, or fixing
an inference error at a `.context(..)` call. The basic pattern is in
`SKILL.md`; the design rule (context vs variant) is in
`structured-error-handling`.

`WithContext<C, E, F = Colon>` has public fields `context` and `error` and a
type-level pair strategy `F`. `WithPath<C, E>` is `WithContext<C, E, PathColon>`.
`ResultExt` adds to every `Result<T, E>`:

```rust
fn context<C, F>(self, context: C) -> Result<T, WithContext<C, E, F>>;
fn with_context<C, F>(self, f: impl FnOnce() -> C) -> Result<T, WithContext<C, E, F>>;
```

## How `F` is inferred

`F` is a free parameter of `.context`, fixed by the type the value flows into:

| The result flows into | Works because |
|---|---|
| `?` with exactly one `#[from] WithContext<C, E, _>` for this `C`, `E` on the target error | the single `From` impl fixes `F` |
| `.map_err(Error::Variant)` | the variant's field type fixes `F` |
| a function returning `Result<T, WithContext<C, E>>` | the return type fixes `F` |

Two compile errors come from this:

- `type annotations needed ... cannot infer type of the type parameter F`: the
  target has two `#[from]` variants for the same `C` and `E` with different
  formats (for example `WithPath<&str, io::Error>` and
  `WithContext<&str, io::Error>`). Use `.map_err(Error::Variant)` for one.
- `the trait From<WithContext<&str, io::Error, _>> is not implemented for
  Error`: the variant holds the pair as `#[source]`, not `#[from]`. Add
  `.map_err(Error::Variant)` or change it to `#[from]`.

Outside a `Result`, `WithContext::new(context, error)` infers `F` the same way,
and `(context, error).into()` works too.

## As the whole error type

When a function has one failure and the only thing to add is the offending
value, an alias is enough:

```rust
type IdError = WithContext<u32, TryFromIntError>;

fn narrow(value: u32) -> Result<u16, IdError> {
    u16::try_from(value).context(value)
}
// narrow(70_000): "70000: number too large to fit in target type"
```

## Nesting layers

Layers nest and read outside-in. Apply `.context` innermost first:

```rust
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Failed to create file")]
    Create(#[source] WithPath<PathBuf, WithContext<usize, io::Error>>),
}

return Err(e).context(attempt).context(path.to_owned()).map_err(Error::Create);
```

```text
Failed to create file: no/such/dir/foo.txt: 3: No such file or directory (os error 2)
```

A bare number in a chain is hard to read. If the reader would not know what
`3` means, give the context a `Display` newtype (`Attempt(3)` printing
`attempt 3`) or make it a named field of a real variant.

## Custom pair formats

`Colon` is the tuple `(ContextField, ColonSpace, ErrorField)` and `PathColon`
is `(ContextPath, ColonSpace, ErrorField)`. `Colon` and `PathColon` live in
`errortools::with_context`; the extractors are at the crate root. Build another
layout from the same parts:

```rust
use errortools::{ContextField, ErrorField, Lit, Space, WithContext};

type ArrowPair = (ContextField, Space, Lit<'-', '>'>, Space, ErrorField);

let w = WithContext::<_, _, ArrowPair>::new("step 2", "boom");
assert_eq!(w.to_string(), "step 2 -> boom");
```

In an error enum, name the format in the field type
(`WithContext<&'static str, io::Error, ArrowPair>`) and `.context(..)` infers
it. When tuples are not enough, implement the strategy for the pair:

```rust
use core::fmt::{self, Display, Formatter};
use errortools::{Format, WithContext};

struct Bracketed;
impl<C: Display, E: Display, WCF> Format<WithContext<C, E, WCF>> for Bracketed {
    fn fmt(w: &WithContext<C, E, WCF>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", w.context, w.error) // "[1] boom"
    }
}
```

A pair strategy has to print `w.error`. `WithContext::source()` skips the inner
error on the assumption that the pair's `Display` already showed it, so a
context-only strategy drops the error text from every chain rendering.

`w.with_format::<NewF>()` switches the strategy of an existing value.
