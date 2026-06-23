---
name: rust-error-handling
description: >
  Router for Rust error-handling skills. Use by default whenever a Rust task
  touches errors in any form: Result, unwrap, expect, thiserror, anyhow,
  Box<dyn Error>, error logging, or the errortools crate. Routes to the right
  sub-skill: structured-error-handling for designing error types and source
  chains, using-errortools for MainResult / FormatError / WithContext / ManyErrors,
  migrating-from-unstructured for moving away from anyhow or scattered unwraps.
  When in doubt, load this skill first.
---

# Rust error-handling

Three skills cover the full error-handling surface. Load the one that matches
your task; they cross-link where they overlap.

| Task | Skill |
|---|---|
| Design error enums, chain sources, choose `#[source]` vs `#[from]`, `map_err` conventions | `structured-error-handling` |
| Use `MainResult`, `FormatError`, `WithContext`, `ManyErrors`, custom format strategies | `using-errortools` |
| Migrate from `anyhow` or scattered `unwrap`/`expect`/`Box<dyn Error>` | `migrating-from-unstructured` |

Most tasks touch more than one area, so load both if needed.

## Adding the dependency

If the project does not depend on `errortools` yet:

```toml
[dependencies]
errortools = "0.3"
thiserror  = "2"
```

`errortools` is `no_std`-capable; pass `default-features = false` for embedded
targets. The `alloc` feature (implied by `std`) gates `ManyErrors` and the
aggregate render shapes.

## References

- API docs: <https://docs.rs/errortools>
- README: <https://github.com/maxwase/errortools/blob/master/README.md>
- Runnable examples: <https://github.com/maxwase/errortools/tree/master/examples>
