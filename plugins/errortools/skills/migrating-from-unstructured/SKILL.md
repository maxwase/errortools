---
name: migrating-from-unstructured
description: >
  Step-by-step guide for moving Rust error handling from anyhow or unstructured
  patterns to typed thiserror enums and the errortools crate. Use whenever a
  task involves replacing anyhow, removing Box<dyn Error> or String errors,
  converting anyhow .context() calls, eliminating unwrap/expect from production
  code, or replacing manual eprintln + exit(1). Trigger on: anyhow, bail!,
  ensure!, anyhow::Context, Box<dyn Error>, String error, exit(1), unwrap(),
  expect() -- even mid-refactor. For upgrading errortools 0.3 to 0.4 use
  using-errortools instead.
---

# migrating-from-unstructured

Moves a codebase from `anyhow`, `Box<dyn Error>`, `String` errors, `unwrap`s
and hand-rolled reporting to typed errors. The steps are mechanical and each
is shippable on its own, so migrate module by module, leaves first.

Design rules for the new types are in `structured-error-handling`; the runtime
API is in `using-errortools`. This skill only maps old patterns to new ones.

## Target state

1. One exit: `fn main() -> MainResult<Error>`, reached by `?`. Errors from
   spawned tasks and handlers are joined or sent back to it; they are logged
   in place only when there is no way to return them.
2. One rendered chain identifies the code path that failed, because every
   layer contributes its own message. Callers can `match` on any layer.

## Step 1: dependencies

```toml
errortools = "0.4"
thiserror  = "2"
# anyhow = "1"   remove once the last use is gone
```

## Step 2: a typed `Error` per module

Replace `anyhow::Result<T>`, `Result<T, Box<dyn Error>>` and
`Result<T, String>` with `Result<T, Error>`, where `Error` is the module's own
enum. A formatted string becomes a variant with the cause as `#[source]`, not
embedded in the text.

```rust
// BEFORE
fn parse(s: &str) -> Result<u32, String> {
    s.parse().map_err(|e| format!("bad number: {e}"))
}

// AFTER
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bad number")]
    Parse(#[source] std::num::ParseIntError),
}

fn parse(s: &str) -> Result<u32, Error> {
    s.parse().map_err(Error::Parse)
}
```

A parent module takes a child's error with
`#[error(transparent)] Config(#[from] config::Error)`, so `?` keeps working
across module boundaries without `map_err`.

## Step 3: `anyhow`'s `.context(..)`

An `anyhow` context string does two jobs. Split them: the fixed part, which
names the operation, becomes the variant's message; the interpolated part,
which identifies the item, becomes an `errortools` context value.

```rust
// BEFORE
let text = fs::read_to_string(path)
    .with_context(|| format!("Failed to read config {}", path.display()))?;

// AFTER
#[error("Failed to read config")]
Read(#[from] WithPath<PathBuf, io::Error>),

let text = fs::read_to_string(path).with_context(|| path.to_owned())?;
```

```text
Error: Failed to read config: app.conf: No such file or directory (os error 2)
```

| `anyhow` call | Typed replacement |
|---|---|
| `.context("Read config")?` (no data) | `.map_err(Error::ReadConfig)?` |
| `.with_context(\|\| format!("Read {path}"))?` | variant holding `WithPath<..>` / `WithContext<..>`, then `.context(path)?` if it is `#[from]`, else `.context(path).map_err(Error::Variant)?` |
| `.context(..)` on an `Option` | `.ok_or(Error::MissingKey(key))?` |

While a module still imports `anyhow::Context`, importing
`errortools::ResultExt` as well makes `.context(..)` ambiguous. Convert a
module's calls together and drop the `anyhow` import in the same change.

## Step 4: `bail!`, `ensure!`, `anyhow!`

```rust
// BEFORE
ensure!(count > 0, "count must be positive, got {count}");

// AFTER
#[error("Count must be positive, got {count}")]
InvalidCount { count: i32 },

if count <= 0 {
    return Err(Error::InvalidCount { count });
}
```

## Step 5: `main`

```rust
// BEFORE
fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// AFTER: no run() wrapper, no eprintln!, no exit
fn main() -> errortools::MainResult<Error> {
    let count = config::load(Path::new("app.conf"))?;
    // ...
    Ok(())
}
```

`fn main() -> anyhow::Result<()>` changes the same way. `MainResult` prints the
whole chain on one line and exits with code 1; `?` converts through the
application error's `#[from]` impls.

## Step 6: hand-rolled source walks and `{:#}` / `{:?}` printing

```rust
// BEFORE
let mut cur: &dyn std::error::Error = &e;
while let Some(source) = cur.source() {
    eprintln!("  caused by: {source}");
    cur = source;
}
// or anyhow's: tracing::error!("{e:#}");

// AFTER
use errortools::FormatError;
tracing::error!("{}", e.one_line()); // single log line
eprintln!("{}", e.chain());          // indented ladder
```

## Step 7: `unwrap` / `expect`

Each one outside tests becomes a returned variant, or keeps a documented
invariant under `# Panics`.

```rust
// BEFORE
let val = map.get("count").unwrap();

// AFTER: carry what was missing
let key = "count";
let val = map.get(key).ok_or(Error::MissingKey(key))?;
```

## Step 8: swallowed failures

- `let _ = process(item);` in a loop, or `filter_map(Result::ok)`: collect
  with `ManyErrors::collect_results` (`using-errortools`, "Batches").
- `e.to_string()` sent through a channel, or `anyhow::Error` returned from a
  spawned task because the real error is not `Send`: send `e.detach()`
  (`using-errortools`, "Crossing a thread, task or channel").

## Tests

Tests may keep `unwrap`, panics and an opaque return type; that is the one
place they are fine. Assertions still follow `structured-error-handling`
rule 16: match the variant, and cover the failure paths.
