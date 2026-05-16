# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `WithContext<C, E, F>` — tag any error with a context value (path, attempt number, etc.). `Colon` is the default strategy; `PathColon` handles
  `Path`/`PathBuf` directly without a newtype. `WithPath<C, E>` aliases the path case. The wrapper's `Error::source` skips its inner error so chain walkers don't print it twice.
- `Suggest` trait for per-variant "Did you mean…" hints, and the `Suggestion` `Format` strategy that renders them. `error.suggestion()` prints the top-level hint via `Display`. Default `Suggest::fmt` writes nothing, so types only implement it for variants that have a hint.
- `DisplayPath` wrapper in the `path_display` module — drops `&Path`/`PathBuf` into any `Display` context.
- `T` generic on `MainResult<E, F, T = ()>` so `main` can return `ExitCode` or any `Termination` type.

### Changed

- **Breaking:** `Format` is now `Format<E: Error + ?Sized>` with
  `fn fmt(error: &E, ...)`. The old `&dyn Error` signature is gone. Custom
  strategies need to add the `<E>` parameter. The motivation: a strategy can
  now require extra bounds on `E` (the `Suggestion` strategy needs
  `E: Suggest`, which doesn't survive type erasure). See
  `examples/custom_format.rs` for the new shape.

### Removed

- **Breaking:** `FormatOneLine` type alias. Use `Formatted<E, OneLine>`.

## [0.1.0] - 2025-01-01

### Added

- `MainResult<E, F>` — drop-in `Result` for `fn main` that prints errors via
  `Display` (and the full source chain) instead of `Debug`.
- `OneLine` format — joins the error chain with `": "`.
- `Tree<M, I>` format — multi-line indented tree, marker and indent
  customizable via any `Display + Default` types.
- `Format` trait + `chain()` iterator for writing custom strategies.
- `FormatError` extension trait (`one_line()`, `tree()`, `formatted::<F>()`)
  on any `Error`, including `&dyn Error`.
- `DisplaySwapDebug<E>` wrapper that swaps `Debug`/`Display`.
- `no_std` support via `default-features = false`.

[Unreleased]: https://github.com/maxwase/errortools/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/maxwase/errortools/releases/tag/v0.1.0
