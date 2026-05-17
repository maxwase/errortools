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
- `Add<L, R>` — type-level combinator that runs two `Format` strategies in sequence. Compose with the new `separator` module (`NewLine`, `Space`, `Empty`, `Colon`, `ColonSpace`) to build strategies like `Add<Add<OneLine, NewLine>, Suggestion>` without writing a new impl. Bounds compose automatically — using `Suggestion` in the chain still requires `E: Suggest`.
- `WithSep<L, Sep, R>` alias — sugar for `Add<Add<L, Sep>, R>` so the separator reads in the middle. Plus pre-baked variants in `separator`: `WithSpace<L, R>`, `WithNewLine<L, R>`, `WithColonSpace<L, R>`.
- `separator::IfNonEmpty<Sep, Then>` — conditional combinator that writes `Sep` followed by `Then` only if `Then` produces non-empty output. Used internally by `WithSuggestion` so `MainResultWithSuggestion` doesn't print a trailing blank line when `Suggest::fmt` writes nothing.
- `with_context::ContextField`, `ErrorField`, and `ContextPath` (std-only) — `Format` extractor strategies that read fields of `WithContext`. Compose with separators to build custom pair strategies, e.g. `WithSep<ContextField, Space, ErrorField>` for a space-delimited pair.

### Changed

- **Breaking:** `Format` is now `Format<E: ?Sized>` (the `E: Error` bound on the trait is gone). Strategies that walk the source chain still need `E: Error` and declare it themselves. The motivation: composing strategies through non-error types (e.g. `WithContext`) required dropping the trait-level `Error` bound. Existing impls keep working — the bound just moves from the trait to each impl that needs it.
- `WithSuggestion<F, Sep>` (and therefore `MainResultWithSuggestion`) now skips the separator when `Suggest::fmt` writes nothing, so variants without a hint no longer leave a trailing newline in CLI output. Previous shape was `Add<Add<F, Sep>, Suggestion>`; new shape is `Add<F, IfNonEmpty<Sep, Suggestion>>`.

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
