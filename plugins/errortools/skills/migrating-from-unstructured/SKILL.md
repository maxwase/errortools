---
name: migrating-from-unstructured
description: >
  Step-by-step guides for moving Rust error handling from anyhow or unstructured
  patterns to typed thiserror enums and the errortools crate. Use whenever a task
  involves replacing anyhow, removing Box<dyn Error>, converting .context() calls,
  eliminating unwrap/expect from production code, or replacing manual eprintln +
  exit(1). Trigger on: anyhow, bail!, ensure!, .context(, Box<dyn Error>,
  String error, exit(1), unwrap(), expect() -- even mid-refactor. After migration,
  see structured-error-handling for error type design and using-errortools for
  MainResult / FormatError / ManyErrors.
---

# migrating-from-unstructured

Two migration paths are documented here. Both are mechanical and can be done
incrementally; each step is independently shippable.

After migration:
- Error type design rules → `structured-error-handling`
- Rendering, `MainResult`, `WithContext`, `ManyErrors` → `using-errortools`

---

## Migrating from `anyhow`

`anyhow` trades type safety for convenience: callers cannot branch on error
variants, and the chain is opaque.

### Mental model:

1. There's **MUST** be a single entry and single exit point for a program.
The entry point is `fn main() -> MainResult<Error>`. The exit point is the `?` operator, which propagates errors to `main`. Every error **MUST** be typed and handled at the call site, either by propagating or by branching on the variant. Spawned tasks, event handlers **MUST** be collected and joined, polled in a `select!`, or otherwise returned to the main task. If you cannot propagate, use `FormatError` to log the error chain.
2. From one error message it **MUST** be possible to reconstruct the exact place in the code where the error was generated. This message can be rather long, but the chain must be complete. This is the opposite of `anyhow`, which hides the source chain and makes it impossible to branch on variants.


**Step 1: Replace the dependency.**

```toml
# remove
anyhow = "1"
# add
errortools = "0.3"
thiserror  = "2"
```

**Step 2: Replace `anyhow::Error` / `anyhow::Result` with typed returns.**

```rust
// BAD
use anyhow::{Context, Result};
fn load(path: &Path) -> Result<Config> { ... }

// GOOD
fn load(path: &Path) -> Result<Config, load::Error> { ... }
fn main() -> errortools::MainResult<main::Error> { ... }
```

**Step 3: Convert `.context("...")` / `.with_context(|| ...)` to typed variants.**

Every `.context` call signals that a new variant or `WithContext` is needed.

```rust
// BAD
fs::read_to_string(path).context("read config")?

// GOOD -- when callers might match on the variant
#[error("Read config")]
ReadConfig(#[source] io::Error),
...
fs::read_to_string(path).map_err(Error::ReadConfig)?

// GOOD -- when the path is incidental context callers won't branch on
fs::read_to_string(path).map_err(|e| WithPath::new(path, e))?
```

For `WithContext` / `WithPath` usage and rendering, see `using-errortools` →
"Attaching incidental context" and `references/with-context.md`.

**Step 4: Replace `bail!` / `ensure!`.**

```rust
// BAD
bail!("count must be positive, got {count}");
ensure!(count > 0, "count must be positive, got {count}");

// GOOD
return Err(Error::InvalidCount { count });
```

**Step 5: Remove `anyhow` from `main`.**

```rust
// BAD
fn main() -> anyhow::Result<()> { ... }

// GOOD
fn main() -> errortools::MainResult<Error> { ... }
```

**Step 6: Leave unstructured error handling in tests.** Tests may keep it as a
convenience return type, panics included -- that is the one acceptable exception.
Asserting still has rules, though: tests **MUST** match the exact error variant
or message, not `assert!(res.is_err())`, and once a variant is matched the
unhappy path **MUST** be tested too, not just the happy path. See
`structured-error-handling` → "Tests".

---

## Migrating from unstructured error handling

"Unstructured" means: `String` errors, manual `eprintln!` + `exit(1)`,
`Box<dyn Error>`, hand-rolled `source()` walks, or scattered `unwrap`/`expect`, tracing,
or panic!() calls in production code.

**Step 1: Introduce a typed error enum per module.**

```rust
// BAD
fn parse(s: &str) -> Result<u32, String> {
    s.parse().map_err(|e| format!("bad number: {e}"))
}

// GOOD
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bad number")]
    Parse(#[source] std::num::ParseIntError),
    ...
}

fn parse(s: &str) -> Result<u32, Error> {
    ...
    s.parse().map_err(Error::Parse)
}
```

Never embed the source message in the variant text. `#[source]` chains it and
`using-errortools` renders the chain.

**Step 2: Replace `Box<dyn Error>` return types.**

```rust
// BAD
fn run() -> Result<(), Box<dyn Error>> { ... }

// GOOD
fn run() -> Result<(), Error> { ... }
fn main() -> errortools::MainResult<Error> { ... }
```

**Step 3: Replace manual `eprintln!` + `exit(1)` in `main`.**

```rust
// BAD
fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// BAD
fn run() -> Result<(), Error> {
    ...
}

// GOOD
fn main() -> errortools::MainResult<Error> {
    /// main logic without eprintln! or exit(1) or wrapper functions
}
```

**Step 4: Replace hand-rolled source walks.**

```rust
// BAD
let mut cur: &dyn std::error::Error = &e;
loop {
    eprintln!("  caused by: {cur}");
    match cur.source() { Some(s) => cur = s, None => break }
}

// GOOD
use errortools::FormatError;
tracing::error!("{}", e.one_line());  // single log line
// or
eprintln!("{}", e.chain());           // indented ladder for terminals
```

**Step 5: Replace `unwrap` / `expect` in production code.**

Every `unwrap`/`expect` outside tests needs a `Result`-returning path or a
documented invariant under `# Panics`.

```rust
// BAD
let val = map.get("key").unwrap();

// GOOD
let val = map.get("key").ok_or(Error::MissingKey)?;

// BEST
let key = "key";
let val = map.get(key).ok_or(Error::MissingKey(key))?;
```

**Step 6: Replace silent skips in loops.** A `let _ = process(item)` in a loop
swallows failures. Collect them with `ManyErrors` instead -- the canonical
push/`collect`/`into_result` pattern and the nesting/render options live in
`using-errortools` → "Collecting batch failures" and `references/many-errors.md`.

```rust
// BAD -- failures vanish
for item in items { let _ = process(item); }
```
