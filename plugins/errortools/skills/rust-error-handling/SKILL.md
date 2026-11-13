---
name: rust-error-handling
description: >
  Router for Rust error-handling skills. Use by default whenever a Rust task
  touches errors in any form: Result, unwrap, expect, thiserror, anyhow,
  Box<dyn Error>, error logging, or the errortools crate. Routes to the right
  sub-skill: structured-error-handling for designing error types and source
  chains, using-errortools for MainResult / .context() / FormatError /
  ManyErrors / Detached and for upgrading errortools 0.3 code to 0.4,
  migrating-from-unstructured for moving away from anyhow or scattered unwraps.
  When in doubt, load this skill first.
---

# Rust error handling

Three skills cover the surface. Load the one that matches the task; most tasks
touch two, so load both when in doubt.

| Task | Skill |
|---|---|
| Design error enums, choose `#[source]` vs `#[from]`, word messages, convert at call sites, test errors | `structured-error-handling` |
| Report from `main`, attach context with `.context(..)?`, log a chain, collect batch failures, move an error across a task or channel, custom formats | `using-errortools` |
| Replace `anyhow`, `Box<dyn Error>`, `String` errors, `unwrap`/`expect`, `eprintln!` + `exit(1)` | `migrating-from-unstructured` |
| Code written against errortools 0.3 no longer compiles | `using-errortools`, then its `references/upgrading-to-0.4.md` |

## Adding the dependency

```toml
[dependencies]
errortools = "0.4"
thiserror  = "2"
```

`errortools` works in `no_std` with `default-features = false`. The `alloc`
feature (implied by `std`) gates `ManyErrors` and `Detached`; `std` gates the
path helpers (`WithPath`, `ContextPath`, `DisplayPath`).

## References

- API docs: <https://docs.rs/errortools>
- Runnable examples: <https://github.com/maxwase/errortools/tree/master/examples>
