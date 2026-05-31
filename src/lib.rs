#![cfg_attr(feature = "std", doc = include_str!("../README.md"))]
#![cfg_attr(
    not(feature = "std"),
    doc = "Quality of life utilities for error handling in Rust."
)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    iter,
    marker::PhantomData,
};

mod add;
mod chain;
mod connectors;
mod main_result;
#[cfg(feature = "alloc")]
pub mod many_errors;
mod oneline;
#[cfg(feature = "std")]
pub mod path_display;
mod suggestion;
pub mod with_context;

pub use add::{Add, separator};
pub use chain::Chain;
pub use connectors::{Ascii, Connectors, TreeConnectors, Unicode};
pub use main_result::{DisplaySwapDebug, MainResult, MainResultWithSuggestion, WithSuggestion};
#[cfg(feature = "alloc")]
pub use many_errors::{Bullets, Inline, List, ManyErrors, Node, Subgroup, Tree};
pub use oneline::Flat;
#[cfg(feature = "std")]
pub use path_display::DisplayPath;
pub use suggestion::{Suggest, Suggestion};
pub use with_context::WithContext;

/// A static strategy for formatting a value to a [`fmt::Formatter`].
///
/// Usually, the error is traversed via [`chain`] to format the entire source chain,
/// but this is not required — the strategy can choose to ignore the chain or format
/// non-error types as well.
/// For example, an implementation of
/// [`Format<WithContext<C, E, WithContextFormat>>`] can format the context
/// and error fields of [`WithContext`] with field extractors like
/// [`ContextField`](crate::with_context::ContextField) and [`ErrorField`](crate::with_context::ErrorField)
/// without walking the source chain at all.
///
/// `E` is the value being formatted; each strategy declares its own bounds:
/// [`Flat`] and [`Chain`] require `E: Error`, [`Suggestion`] additionally
/// requires [`Suggest`], and field extractors like
/// [`ContextField`](crate::with_context::ContextField) require `E` to be a
/// specific shape. The trait itself imposes nothing beyond `?Sized` so
/// strategies can format non-error pairs (e.g. [`WithContext`]).
///
/// We cannot rely on `fmt::*` traits because:
/// 1. They accept &self
/// 1. `Error` already bounds `Display` as a supertrait, which would block composing strategies through types like [`WithContext`].
///
/// # `Debug` convention across this crate
/// Strategy tags carry their configuration only at the type level (in a phantom
/// `PhantomData<fn() -> _>`), so their `Debug` is hand-written:
/// - **Pure-strategy types** ([`Chain`], [`Add`], [`Tree`], and [`Formatted`])
///   materialize the phantom marker via [`Default`] and print its configuration
///   — these impls bound the marker `…: Debug + Default`, while their
///   auto-traits ([`Clone`]/[`Copy`]/[`PartialEq`]/[`Eq`]/[`Hash`]) stay free of
///   any marker bound.
/// - **Payload types** ([`WithContext`], [`ManyErrors`](crate::ManyErrors),
///   [`Node`](crate::Node)) print their own name and fields, hiding the phantom
///   strategy. Thin display adapters ([`DisplayPath`]) instead stay transparent
///   to mirror their target's `Debug`.
pub trait Format<E: ?Sized> {
    /// Writes `error` and its source chain to `f` using the strategy.
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Sentinel [`Format`] strategy that delegates to the value's own [`fmt::Display`]
/// impl.
///
/// Useful as a default in strategy-aware wrappers when per-item formatting
/// should defer to each item's own `Display` (and thus its own type-level
/// strategy) rather than being overridden.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AsDisplay;

impl<T: fmt::Display + ?Sized> Format<T> for AsDisplay {
    fn fmt(value: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(value, f)
    }
}

/// Iterator over an error and its source chain.
///
/// The first item is `error` itself; subsequent items come from
/// [`Error::source`].
pub fn chain<'a>(error: &'a dyn Error) -> impl Iterator<Item = &'a dyn Error> + 'a {
    iter::successors(Some(error), |e| Error::source(*e))
}

/// A helper trait to format errors.
pub trait FormatError {
    /// Formats the error in a single line concatenated by `: `.
    fn one_line(&self) -> Formatted<&Self, Flat> {
        self.formatted::<Flat>()
    }

    /// Formats the error as an indented source-chain ladder.
    ///
    /// For aggregate many-error rendering (branching tree) see
    /// [`ManyErrors::tree`](crate::many_errors::ManyErrors::tree).
    fn chain(&self) -> Formatted<&Self, Chain> {
        self.formatted::<Chain>()
    }

    /// Renders the error's [`Suggestion`] hint. Only the top-level error is
    /// printed; the source chain is not walked.
    fn suggestion(&self) -> Formatted<&Self, Suggestion>
    where
        Self: Suggest,
    {
        self.formatted::<Suggestion>()
    }

    /// Formats the error using a custom [`Format`] strategy.
    fn formatted<F>(&self) -> Formatted<&Self, F> {
        Formatted::new(self)
    }
}

impl<E: Error + ?Sized> FormatError for E {}

/// An error wrapper that uses a static [`Format`] strategy for [`fmt::Display`].
///
/// `F` is a type-level tag (never instantiated). The `fn() -> F` inside
/// [`PhantomData`] avoids drop-check ownership of `F` and makes the wrapper
/// `Send + Sync` regardless of `F`.
pub struct Formatted<E, F = Flat>(E, PhantomData<fn() -> F>);

// Manual impls bounding only the real `E`, never the phantom strategy `F`
// (the `_format`-style doctrine; see `WithContext`).
impl<E: Clone, F> Clone for Formatted<E, F> {
    fn clone(&self) -> Self {
        Formatted(self.0.clone(), PhantomData)
    }
}
impl<E: Copy, F> Copy for Formatted<E, F> {}
impl<E: Default, F> Default for Formatted<E, F> {
    fn default() -> Self {
        Formatted(E::default(), PhantomData)
    }
}
impl<E: PartialEq, F> PartialEq for Formatted<E, F> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<E: Eq, F> Eq for Formatted<E, F> {}
impl<E: Hash, F> Hash for Formatted<E, F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<E, F> Formatted<E, F> {
    /// Wraps `error` so its `Display` impl uses the [`Format`] strategy `F`.
    pub const fn new(error: E) -> Self {
        Formatted(error, PhantomData)
    }
}

/// Renders the wrapped error via the strategy `F`.
/// These genetic bounds actually define whether a strategy can be used to format a given error type
/// Any error type can be put into a strategy, but not every can actually be formatted.
/// That's why it's possible to construct, but get a compiler error when trying to call [`fmt::Display`] on the combination.
impl<E: Error, F: Format<E>> fmt::Display for Formatted<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        F::fmt(&self.0, f)
    }
}

/// Surfaces both the wrapped error and the active strategy (materialized via
/// [`Default`], like [`Chain`]/[`Add`]/[`Tree`]) rather than printing
/// `Formatted(.., PhantomData)`. The `F: Debug + Default` bound applies to this
/// `Debug` impl only — the auto-trait impls above stay free of any `F` bound.
impl<E: fmt::Debug, F: fmt::Debug + Default> fmt::Debug for Formatted<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Formatted")
            .field("error", &self.0)
            .field("format", &F::default())
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod tests;
