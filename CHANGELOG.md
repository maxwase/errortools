# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-06-23

### Added

- `ManyErrors<C, E, GC, F, GF>` (requires `alloc`) — aggregates context-tagged errors as a rose tree. Costs nothing until needed: `None` while empty, one inline slot for the first error, a `Vec` only from the second. Implements `Error`, `Add`, `FromIterator`, and `Extend` for `Node`, `WithContext`, `(C, E)`, and `ControlFlow` pairs. `into_result(ok)` converts to `Ok`/`Err` in one call.
- `Node<C, E, GC, F, GF>` and `Subgroup<C, E, GC, F, GF>` (requires `alloc`) — the two child variants of `ManyErrors`: leaf `WithContext` pairs and labeled sub-groups.
- `Iter`, `IterMut`, `IntoIter` (requires `alloc`) — iterator types over the direct children of a `ManyErrors`, returned by `iter()`, `iter_mut()`, and `into_iter()`.
- `Tree<Conn, HEADER>`, `List`, `Bullets`, `Joined` (requires `alloc`) — aggregate rendering strategies. `Tree` draws a branching Unicode tree walking each leaf's source chain. `List` renders a numbered outline. `Bullets` uses `•` markers. `Joined` serializes to a single `;`-separated line. All are available via inherent helpers (`errs.tree()`, etc.) and via `errs.formatted::<F>()` for full generic control.
- `Connectors` / `TreeConnectors` traits with the `Unicode` and `Ascii` glyph sets, shared by `Chain` and the aggregate `Tree` (so `Chain<Ascii>` and `Tree<Ascii>` look consistent).

### Changed

- **Breaking:** `Tree<M, I>` (the per-error source-chain ladder) renamed to `Chain<C>`. The type-parameter API changed too: the old `Marker + Indent` pair is replaced by a single `Connectors` impl (e.g. `Chain<Ascii>`). The `Tree` name is now used for the `ManyErrors` aggregate branching renderer.
- **Breaking:** `FormatError::tree()` renamed to `FormatError::chain()`, matching the `Tree` → `Chain` strategy rename.
- **Breaking:** the `separator::Colon` tag (added in 0.2.0) renamed to `separator::ColonChar`, to avoid colliding with the `with_context::Colon` *pair* strategy — a misimport between the two would compile but render each leaf as a bare `":"`.

## [0.2.0] - 2026-05-17

### Added

- `WithContext<C, E, F>` — tag any error with a context value (path, attempt number, etc.). `Colon` is the default strategy; `PathColon` handles
  `Path`/`PathBuf` directly without a newtype. `WithPath<C, E>` aliases the path case. The wrapper's `Error::source` skips its inner error so chain walkers don't print it twice.
- `Suggest` trait for per-variant "Did you mean…" hints, and the `Suggestion` `Format` strategy that renders them. `error.suggestion()` prints the top-level hint via `Display`. Default `Suggest::fmt` writes nothing, so types only implement it for variants that have a hint.
- `DisplayPath` wrapper in the `path_display` module — drops `&Path`/`PathBuf` into any `Display` context.
- `T` generic on `MainResult<E, F, T = ()>` so `main` can return `ExitCode` or any `Termination` type.
- `Add<L, R>` — type-level combinator that runs two `Format` strategies in sequence. Compose with the new `separator` module (`NewLine`, `Space`, `Empty`, `Colon`, `ColonSpace`) to build strategies like `Add<Add<OneLine, NewLine>, Suggestion>` without writing a new impl. Bounds compose automatically — using `Suggestion` in the chain still requires `E: Suggest`.
- `WithSep<L, Sep, R>` alias — sugar for `Add<Add<L, Sep>, R>` so the separator reads in the middle. Plus pre-baked variants in `separator`: `WithSpace<L, R>`, `WithNewLine<L, R>`, `WithColonSpace<L, R>`.
- `with_context::ContextField`, `ErrorField`, and `ContextPath` (std-only) — `Format` extractor strategies that read fields of `WithContext`. Compose with separators to build custom pair strategies, e.g. `WithSep<ContextField, Space, ErrorField>` for a space-delimited pair.

### Changed

- **Breaking:** `Format` is now `Format<E: ?Sized>` (the `E: Error` bound on the trait is gone). Strategies that walk the source chain still need `E: Error` and declare it themselves. The motivation: composing strategies through non-error types (e.g. `WithContext`) required dropping the trait-level `Error` bound. Existing impls keep working — the bound just moves from the trait to each impl that needs it.
- **Breaking:** `with_context::Colon` and `with_context::PathColon` are now `type` aliases over `Add` rather than dedicated structs: `Colon = Add<Add<ContextField, ColonSpace>, ErrorField>`. The `WithContext::<_, _, Colon>` / `WithContextColon` / `WithPath` surface is unchanged; only code that named the strategies as `struct Colon;` (e.g. an explicit `impl ContextFormat for Colon`) is affected.
- **Breaking:** `WithContext`'s `Display`, `Error`, and `with_format` bounds switched from `F: ContextFormat<C, E>` to `F: Format<WithContext<C, E, F>>`.

### Removed

- **Breaking:** `FormatOneLine` type alias. Use `Formatted<E, OneLine>`.
- **Breaking:** `with_context::ContextFormat<C, E>` trait. Custom pair strategies now `impl<C, E, F> Format<WithContext<C, E, F>> for MyFmt` and pull fields off `&WithContext` directly. The pre-composed `Colon` / `PathColon` aliases cover the previous defaults.

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

[Unreleased]: https://github.com/maxwase/errortools/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/maxwase/errortools/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/maxwase/errortools/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maxwase/errortools/releases/tag/v0.1.0
