# Formatting strategies

Read this when the built-in `OneLine` / `Chain` output is not what the task
needs, or when a format type fails to compile.

A strategy is a type that implements `Format<E>`; it is named as a type
parameter and never instantiated. `Format<E>` puts no bound on `E`: each
strategy declares what it needs (`OneLine` and `Chain` need `E: Error`,
`Suggestion` needs `E: Suggest`).

The examples use this error:

```rust
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Failed to load config")]
    Config(#[source] ConfigError),   // ConfigError::Read(#[source] io::Error)
    #[error("Network down")]
    Network,
}
// impl Suggest for Error: Config => "Copy config.example.toml to config.toml"
```

## Built-in strategies

| Strategy | Output |
|---|---|
| `OneLine` | `Failed to load config: Failed to read file: disk full` |
| `Chain<C = Unicode>` | one source per line under `└─ `; `Chain<Ascii>` uses `` `- `` |
| `Suggestion` | the top-level error's hint only (see `suggestions.md`) |
| `AsDisplay` | the value's own `Display` |
| `Tree`, `List`, `Bullets`, `Joined` | `ManyErrors` shapes (see `many-errors.md`) |
| `Lit<A, B>`, `Empty`, `Then<Sep, R>` | separators, below |

```rust
println!("{}", e.formatted::<Chain<Ascii>>());
```

```text
Failed to load config
`- Failed to read file
   `- disk full
```

`Chain` takes its glyphs from a `Connectors` impl (`LAST`, `GAP`);
`TreeConnectors` adds `BRANCH` and `VERT` for `Tree`. `Unicode` and `Ascii`
implement both. Implement them on a unit type for other glyphs.

## Composing with tuples

A tuple of two to six strategies is a strategy: each element writes in order
against the same value, and the tuple implements `Format<E>` exactly when every
element does. Separators are strategies that ignore the value:

| Separator | Writes |
|---|---|
| `NewLine`, `Space`, `ColonChar`, `ColonSpace` | `"\n"`, `" "`, `":"`, `": "` |
| `Lit<'-', '>'>` | any one or two characters, here `->` |
| `Empty` | nothing |
| `Then<Sep, R>` | `Sep` then `R`, but nothing at all when `R` writes nothing |

```rust
use errortools::{Chain, FormatError, Lit, NewLine, OneLine, Space, Suggestion, Then};

type Report = (Chain, Then<NewLine, Suggestion>);
type Tagged = (Lit<'!'>, Space, OneLine);

println!("{}", e.formatted::<Report>());
println!("{}", e.formatted::<Tagged>());
```

```text
Failed to load config
└─ Failed to read file
   └─ disk full
Copy config.example.toml to config.toml
! Failed to load config: Failed to read file: disk full
```

Put an optional trailing part behind `Then`. Tuple elements write
unconditionally, so `(OneLine, NewLine, Suggestion)` renders `Error::Network`
(no hint) as `"Network down\n"`, while `(OneLine, Then<NewLine, Suggestion>)`
renders `"Network down"`. That second tuple is the alias
`WithSuggestion<F = OneLine, Sep = NewLine>`.

## Custom `Format` impl

Write an impl when a tuple cannot express the layout. Walk the chain with
`sources(&error)`, which yields the error and then each `source()`.

```rust
use core::{error::Error, fmt};
use errortools::{Format, sources};

struct Arrow;

impl<E: Error + ?Sized> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `&error` is `&&E`, which coerces to `&dyn Error` even for unsized `E`.
        for (i, e) in sources(&error).enumerate() {
            if i > 0 {
                f.write_str(" -> ")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

// fn main() -> MainResult<Error, Arrow>
println!("{}", e.formatted::<Arrow>());
```

```text
Failed to load config -> Failed to read file -> disk full
```

## Wrappers

| Type | Holds | Comes from |
|---|---|---|
| `FormattedRef<'_, E, F>` | `&E` | `e.one_line()`, `e.chain()`, `e.suggestion()`, `e.formatted::<F>()` |
| `Formatted<E, F = OneLine>` | `E` | `Formatted::new(e)`; `.inner()`, `.into_inner()` |
| `MainError<E, F = OneLine>` | `E` | `?` inside `fn main() -> MainResult<E, F>`; `.inner()`, `.into_inner()` |

Use the borrowing helpers by default. Use `Formatted` when the rendered value
has to own the error, for example to return `impl Display` from a function or
to store it. Both implement `Display` only when `F: Format<E>`, so a wrong pair
is a compile error at the `{}` site, not at construction.

`MainResult<E, F, T>` is `Result<T, MainError<E, F>>`. `MainError` renders `E`
through `F` for both `Debug` and `Display`, which is why the runtime's
`Debug` print of a failed `main` shows the formatted chain. Its
`From<S> where E: From<S>` impl is what lets `?` convert in one hop. Since it
is generic over every source error, `MainError` cannot implement `Error`
itself; it is a return type for `main`, not a value to nest in other errors.

All wrappers and strategy markers are `Send + Sync` regardless of `F`, so they
can be held across `.await`.
