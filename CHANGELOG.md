# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
