# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-11-13

### Added

- Tuple strategies: tuples of 2 to 6 `Format` strategies implement `Format` and run left to right against the same value, e.g. `(OneLine, NewLine, Suggestion)` or `(ContextField, ColonSpace, ErrorField)`.
- `separator::Lit<const A: char, const B: char>`: writes one or two fixed characters, e.g. `Lit<'-', '>'>` writes `->`. `NewLine`, `Space`, `ColonChar` and `ColonSpace` are aliases of it.
- `separator::Then<Sep, R>`: writes `Sep` and then `R`, but only when `R` renders something.
- `FormattedRef<'a, E, F>`: the borrowing twin of `Formatted`. Its `Display` needs only `F: Format<E>`, and `E` may be unsized (`dyn Error`).
- `MainError<E, F>`: the error half of `MainResult`. `From<S>` is implemented for every `S: Error` with `E: From<S>`, so `?` in `main` converts `E` itself and any error `E` has a `From` impl for, in one hop.
- `ResultExt::{context, with_context}` on `Result`: tags the error with a context value and yields a `WithContext`. The format strategy is inferred from the `From` target at the `?` or from the return type.
- `Detached` (requires `alloc`): an owned, cloneable, `Send + Sync + 'static` snapshot of an error chain, taken with `FormatError::detach()` or built with `Detached::new` and `Detached::with_source`. It implements `Error` and renders through every strategy like the original.
- `ManyErrors::collect_results`: splits `(context, Result)` pairs into `Ok(Vec<T>)` or the aggregate of every failure.
- `ManyErrors`: `FromIterator` and `Extend` accept `(C, Result<T, E>)`; `Ok` items are skipped.
- `ManyErrors::detach()`: inherent method that snapshots the deep one-line rendering, shadowing the shallow `FormatError::detach()`. `errs.map_err(|e| e.detach())` keeps the structure instead.
- `ManyErrors::chain()`: inherent method, same rendering as `tree()`. Together with the inherent `one_line()` it shadows the shallow `FormatError` method of the same name on a concrete `ManyErrors`.
- `inner()` and `into_inner()` on `Formatted`, `MainError`, `DisplaySwapDebug` and `DisplayPath`.
- `DisplayPath::new`, a `const fn` constructor next to the existing `From` impl.
- `#[diagnostic::on_unimplemented]` messages on `Format`, `Suggest`, `Connectors` and `TreeConnectors` that name the strategy and the value type.
- Crate-root exports for the types used in signatures: `Lit`, `Then`, `NewLine`, `Space`, `ColonChar`, `ColonSpace`, `Empty`, `ContextField`, `ErrorField`, `ContextPath`, `WithPath`, `ResultExt`, `ErrorCount`, `LeafChain`.
- Static assertions that the wrappers are `Send + Sync` whatever the strategy type.

### Changed

- **Breaking:** `FormatError::{one_line, chain, suggestion, formatted}` and the `ManyErrors` helpers (`tree`, `list`, `bullets`, `one_line`, `formatted`) return `FormattedRef<'_, Self, F>` instead of `Formatted<&Self, F>`. Code that only displays the result is unaffected.
- **Breaking:** `MainResult<E, F, T>` is `Result<T, MainError<E, F>>` instead of `Result<T, DisplaySwapDebug<Formatted<E, F>>>`. The same change applies to `MainResultWithSuggestion`.
- **Breaking:** `WithSuggestion<F, Sep>` is `(F, Then<Sep, Suggestion>)`. An error whose hint renders empty no longer prints a trailing separator.
- **Breaking:** `with_context::Colon` and `with_context::PathColon` are tuples: `(ContextField, ColonSpace, ErrorField)` and `(ContextPath, ColonSpace, ErrorField)`.
- **Breaking:** `NewLine`, `Space`, `ColonChar` and `ColonSpace` are type aliases of `Lit` instead of separate structs.
- **Breaking:** the free function `chain()` is renamed `sources()`, so it no longer shares a name with `FormatError::chain()` and the `Chain` strategy.
- **Breaking:** `ManyErrors::map_err` and `ManyErrors::map_context` keep the `F` and `GF` format parameters instead of resetting them to the defaults.
- **Breaking:** `Iter`, `IterMut` and `IntoIter` are no longer exported at the crate root; they stay under `many_errors`.
- **Breaking:** `path_display` is a private module; `DisplayPath` is exported at the crate root only. `many_errors` no longer re-exports `Ascii`, `Unicode`, `Connectors` and `TreeConnectors`.
- `Suggestion` formats any `E: Suggest`; the `E: Error` bound is gone.
- The `Error` impls and rendering of `WithContext`, `ManyErrors`, `Node` and `Subgroup` no longer require `E: 'static`.
- `DisplaySwapDebug::new` is a `const fn`, like the constructors of the other wrappers. `Formatted`, `FormattedRef`, `MainError`, `DisplaySwapDebug` and `WithContext` are `#[must_use]`.
- `itertools` is a dev-dependency only. The crate's single runtime dependency is `derive-where`.

### Removed

- **Breaking:** `Add<L, R>` and the `separator` aliases `WithSep`, `WithSpace`, `WithNewLine`, `WithColonSpace`. Use tuples.
- **Breaking:** the `impl_ref_format!` macro and the `Format<&T>` forwarders on the built-in strategies. `FormattedRef` makes them unnecessary.
- **Breaking:** `From<E> for DisplaySwapDebug<Formatted<E, F>>`. `MainError` carries the `?` conversion now.
- **Breaking:** `with_context::WithContextColon` and `with_context::WithContextPathColon`.
- **Breaking:** `ManyErrors::joined()` and `ManyErrors::ok()`.

### Migration

| 0.3 | 0.4 |
|---|---|
| `Add<L, R>` | `(L, R)` |
| `Add<Add<A, B>, C>` | `(A, B, C)` |
| `WithSep<L, Sep, R>` | `(L, Sep, R)` |
| `WithSpace<L, R>` | `(L, Space, R)` |
| `WithNewLine<L, R>` | `(L, NewLine, R)` |
| `WithColonSpace<L, R>` | `(L, ColonSpace, R)` |
| `impl_ref_format!(MyFmt)` | delete it; implement `Format<T>` only |
| `impl Format<&T> for MyFmt` | delete it; implement `Format<T>` only |
| `Formatted<&E, F>` as a return type of the borrowing helpers | `FormattedRef<'_, E, F>` |
| `DisplaySwapDebug<Formatted<E, F>>` | `MainError<E, F>` |
| `DisplaySwapDebug::from(Formatted::new(e))` | `MainError::new(e)` |
| `.map_err(AppError::from)?` in `main` | `?` |
| `.map_err(\|e\| WithContext::new(ctx, e))` | `.context(ctx)` |
| `WithContextColon<C, E>` | `WithContext<C, E>` |
| `WithContextPathColon<C, E>` | `WithPath<C, E>` |
| `errortools::chain(&error)` | `errortools::sources(&error)` |
| `errs.joined()` | `errs.one_line()` |
| `errs.ok()` | `errs.is_empty()` (`.then_some(())` for the `Option`) |
| `errortools::{Iter, IterMut, IntoIter}` | `errortools::many_errors::{Iter, IterMut, IntoIter}` |
| `errortools::path_display::DisplayPath` | `errortools::DisplayPath` |
| `errortools::many_errors::{Ascii, Unicode, Connectors, TreeConnectors}` | `errortools::{Ascii, Unicode, Connectors, TreeConnectors}` |
| `impl<E: Error + Suggest> Format<E>` bound copied from `Suggestion` | `E: Suggest` is enough |

## [0.3.1] - 2026-06-25

### Added

- `core::ops::Add` and `AddAssign` on `ManyErrors` for another `ManyErrors`, a `(C, E)` pair, a `(C, Result<T, E>)` pair (`Ok` is skipped), and an `Option` of any of those (`None` is skipped).
- `ManyErrors::push_some` and `ManyErrors::push_result`: push a leaf only for `Some` / `Err`.
- `ManyErrors::into_group`: wraps the aggregate as a labeled sub-group of a new `ManyErrors`.
- `ManyErrors::ok` and `ManyErrors::err`: `Option` views of the aggregate, mirroring `Result::ok` / `Result::err`.
- `ManyErrors::map_err` and `ManyErrors::map_context`: map every leaf error or leaf context, groups included.
- `ManyErrors::one_line()`: inherent method that renders like `joined()` and shadows the shallow `FormatError::one_line`.
- `From<(C, Result<T, E>)> for ManyErrors`.

### Changed

- `ManyErrors` docs: a warning that `FormatError::one_line` / `FormatError::chain` do not walk the aggregate, and the recommended way to embed a `ManyErrors` in an error enum.

## [0.3.0] - 2026-06-23

### Added

- `ManyErrors<C, E, GC, F, GF>` (requires `alloc`) -- aggregates context-tagged errors as a rose tree. Costs nothing until needed: `None` while empty, one inline slot for the first error, a `Vec` only from the second. Implements `Error`, `Add`, `FromIterator`, and `Extend` for `Node`, `WithContext`, `(C, E)`, and `ControlFlow` pairs. `into_result(ok)` converts to `Ok`/`Err` in one call.
- `Node<C, E, GC, F, GF>` and `Subgroup<C, E, GC, F, GF>` (requires `alloc`) -- the two child variants of `ManyErrors`: leaf `WithContext` pairs and labeled sub-groups.
- `Iter`, `IterMut`, `IntoIter` (requires `alloc`) -- iterator types over the direct children of a `ManyErrors`, returned by `iter()`, `iter_mut()`, and `into_iter()`.
- `Tree<Conn, HEADER>`, `List`, `Bullets`, `Joined` (requires `alloc`) -- aggregate rendering strategies. `Tree` draws a branching Unicode tree walking each leaf's source chain. `List` renders a numbered outline. `Bullets` uses `•` markers. `Joined` serializes to a single `;`-separated line. All are available via inherent helpers (`errs.tree()`, etc.) and via `errs.formatted::<F>()` for full generic control.
- `Connectors` / `TreeConnectors` traits with the `Unicode` and `Ascii` glyph sets, shared by `Chain` and the aggregate `Tree` (so `Chain<Ascii>` and `Tree<Ascii>` look consistent).

### Changed

- **Breaking:** `Tree<M, I>` (the per-error source-chain ladder) renamed to `Chain<C>`. The type-parameter API changed too: the old `Marker + Indent` pair is replaced by a single `Connectors` impl (e.g. `Chain<Ascii>`). The `Tree` name is now used for the `ManyErrors` aggregate branching renderer.
- **Breaking:** `FormatError::tree()` renamed to `FormatError::chain()`, matching the `Tree` → `Chain` strategy rename.
- **Breaking:** the `separator::Colon` tag (added in 0.2.0) renamed to `separator::ColonChar`, to avoid colliding with the `with_context::Colon` *pair* strategy -- a misimport between the two would compile but render each leaf as a bare `":"`.

## [0.2.0] - 2026-05-17

### Added

- `WithContext<C, E, F>` -- tag any error with a context value (path, attempt number, etc.). `Colon` is the default strategy; `PathColon` handles
  `Path`/`PathBuf` directly without a newtype. `WithPath<C, E>` aliases the path case. The wrapper's `Error::source` skips its inner error so chain walkers don't print it twice.
- `Suggest` trait for per-variant "Did you mean..." hints, and the `Suggestion` `Format` strategy that renders them. `error.suggestion()` prints the top-level hint via `Display`. Default `Suggest::fmt` writes nothing, so types only implement it for variants that have a hint.
- `DisplayPath` wrapper in the `path_display` module -- drops `&Path`/`PathBuf` into any `Display` context.
- `T` generic on `MainResult<E, F, T = ()>` so `main` can return `ExitCode` or any `Termination` type.
- `Add<L, R>` -- type-level combinator that runs two `Format` strategies in sequence. Compose with the new `separator` module (`NewLine`, `Space`, `Empty`, `Colon`, `ColonSpace`) to build strategies like `Add<Add<OneLine, NewLine>, Suggestion>` without writing a new impl. Bounds compose automatically -- using `Suggestion` in the chain still requires `E: Suggest`.
- `WithSep<L, Sep, R>` alias -- sugar for `Add<Add<L, Sep>, R>` so the separator reads in the middle. Plus pre-baked variants in `separator`: `WithSpace<L, R>`, `WithNewLine<L, R>`, `WithColonSpace<L, R>`.
- `with_context::ContextField`, `ErrorField`, and `ContextPath` (std-only) -- `Format` extractor strategies that read fields of `WithContext`. Compose with separators to build custom pair strategies, e.g. `WithSep<ContextField, Space, ErrorField>` for a space-delimited pair.

### Changed

- **Breaking:** `Format` is now `Format<E: ?Sized>` (the `E: Error` bound on the trait is gone). Strategies that walk the source chain still need `E: Error` and declare it themselves. The motivation: composing strategies through non-error types (e.g. `WithContext`) required dropping the trait-level `Error` bound. Existing impls keep working -- the bound just moves from the trait to each impl that needs it.
- **Breaking:** `with_context::Colon` and `with_context::PathColon` are now `type` aliases over `Add` rather than dedicated structs: `Colon = Add<Add<ContextField, ColonSpace>, ErrorField>`. The `WithContext::<_, _, Colon>` / `WithContextColon` / `WithPath` surface is unchanged; only code that named the strategies as `struct Colon;` (e.g. an explicit `impl ContextFormat for Colon`) is affected.
- **Breaking:** `WithContext`'s `Display`, `Error`, and `with_format` bounds switched from `F: ContextFormat<C, E>` to `F: Format<WithContext<C, E, F>>`.

### Removed

- **Breaking:** `FormatOneLine` type alias. Use `Formatted<E, OneLine>`.
- **Breaking:** `with_context::ContextFormat<C, E>` trait. Custom pair strategies now `impl<C, E, F> Format<WithContext<C, E, F>> for MyFmt` and pull fields off `&WithContext` directly. The pre-composed `Colon` / `PathColon` aliases cover the previous defaults.

## [0.1.0] - 2025-01-01

### Added

- `MainResult<E, F>` -- drop-in `Result` for `fn main` that prints errors via
  `Display` (and the full source chain) instead of `Debug`.
- `OneLine` format -- joins the error chain with `": "`.
- `Tree<M, I>` format -- multi-line indented tree, marker and indent
  customizable via any `Display + Default` types.
- `Format` trait + `chain()` iterator for writing custom strategies.
- `FormatError` extension trait (`one_line()`, `tree()`, `formatted::<F>()`)
  on any `Error`, including `&dyn Error`.
- `DisplaySwapDebug<E>` wrapper that swaps `Debug`/`Display`.
- `no_std` support via `default-features = false`.

[Unreleased]: https://github.com/maxwase/errortools/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/maxwase/errortools/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/maxwase/errortools/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/maxwase/errortools/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/maxwase/errortools/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/maxwase/errortools/releases/tag/v0.1.0
