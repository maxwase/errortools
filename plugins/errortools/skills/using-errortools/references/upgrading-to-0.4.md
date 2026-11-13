# Upgrading from errortools 0.3 to 0.4

Read this when code written for 0.3 fails to compile, or when asked to bump
the dependency. Bump `errortools = "0.4"`, fix the compile errors with the
table, then adopt the simplifications below.

| 0.3 | 0.4 |
|---|---|
| `Add<L, R>`, `WithSep<L, Sep, R>` | tuple `(L, R)`, `(L, Sep, R)` (arity 2 to 6) |
| `WithSpace<L, R>`, `WithNewLine<L, R>`, `WithColonSpace<L, R>` | `(L, Space, R)`, `(L, NewLine, R)`, `(L, ColonSpace, R)` |
| `WithNewLine<OneLine, Suggestion>` | `WithSuggestion` (no separator when there is no hint) |
| `WithContextColon::new(c, e)` | `WithContext::new(c, e)`, or `.context(c)` on the `Result` |
| `WithContextPathColon<C, E>` | `WithPath<C, E>` |
| free function `chain(&e)` | `sources(&e)` |
| `errs.joined()` | `errs.one_line()` |
| `errs.ok()` | `errs.is_empty()` |
| `impl_ref_format!`, hand-written `Format<&T>` forwarders | delete them; `FormattedRef` needs only `Format<T>` |
| return type `Formatted<&E, F>` from `one_line()` / `chain()` / `formatted()` | `FormattedRef<'_, E, F>` |
| `DisplaySwapDebug<Formatted<E, F>>` as the `MainResult` error | `MainError<E, F>` |
| `errortools::many_errors::{Ascii, Unicode}` | `errortools::{Ascii, Unicode}` |
| `errortools::path_display::DisplayPath` | `errortools::DisplayPath` |

Every public type is now at the crate root, except `Iter` / `IterMut` /
`IntoIter` (`many_errors`), `Colon` / `PathColon` (`with_context`) and `Repeat`
(`indent`). Old `separator::` and `with_context::` paths for the surviving
names still resolve.

## Behaviour changes that compile silently

- `WithSuggestion` and `MainResultWithSuggestion` no longer print a trailing
  newline for an error without a hint.
- `ManyErrors::map_err` and `map_context` keep the `F` / `GF` strategies. In
  0.3 they reset to the defaults; remove any `with_formats` call that restored
  them.
- `errs.chain()` on a concrete `ManyErrors` now renders the tree instead of
  the shallow summary.

## Simplifications to apply while there

- `.map_err(|e| WithPath::new(path, e))` becomes `.context(path)`, with `?`
  alone when the target variant is `#[from]`.
- In `main`, drop `.map_err(Error::from)` and double `map_err` hops whose only
  purpose was reaching the application error: `?` converts any `S` with
  `Error: From<S>`.
- Replace `to_string()` snapshots sent across threads or tasks with
  `e.detach()`, which keeps the chain.
- Replace partition-then-check loops over results with
  `ManyErrors::collect_results`.
