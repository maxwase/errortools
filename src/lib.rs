#![cfg_attr(feature = "std", doc = include_str!("../README.md"))]
#![cfg_attr(
    not(feature = "std"),
    doc = "Quality of life utilities for error handling in Rust."
)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![warn(missing_docs)]

use core::{error::Error, fmt, iter, marker::PhantomData};

mod main_result;
mod oneline;
#[cfg(feature = "std")]
pub mod path_display;
mod suggestion;
mod tree;
pub mod with_context;

pub use main_result::{DisplaySwapDebug, MainResult};
pub use oneline::OneLine;
#[cfg(feature = "std")]
pub use path_display::DisplayPath;
pub use suggestion::{Suggest, Suggestion};
pub use tree::{Tree, TreeIndent, TreeMarker};
pub use with_context::WithContext;

/// A static strategy for formatting an error and its source chain.
///
/// The strategy is parameterized over the error type `E` so each strategy can declare its own bounds:
/// [`OneLine`] and [`Tree`] accept any `E: Error`,
/// while strategies like [`Suggestion`](crate::Suggestion) additionally require [`Suggestion`] on `E`.
///
/// Use [`chain`] to walk the error and its sources.
///
/// We cannot rely on `fmt::*` traits because:
/// 1. They accept &self
/// 1. `Error` is already bound by it
///
/// In theory, the [Error] bound can be removed, but it would create confusion when implementing custom strategies,
/// so it's better to keep it.
pub trait Format<E: Error + ?Sized> {
    /// Writes `error` and its source chain to `f` using the strategy.
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result;
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
    fn one_line(&self) -> Formatted<&Self, OneLine> {
        self.formatted::<OneLine>()
    }

    /// Formats the error as an indented tree of sources.
    fn tree(&self) -> Formatted<&Self, Tree> {
        self.formatted::<Tree>()
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
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Formatted<E, F = OneLine>(E, PhantomData<fn() -> F>);

impl<E, F> Formatted<E, F> {
    /// Wraps `error` so its `Display` impl uses the [`Format`] strategy `F`.
    pub const fn new(error: E) -> Self {
        Formatted(error, PhantomData)
    }
}

/// Renders the wrapped error via the strategy `F`.
impl<E: Error, F: Format<E>> fmt::Display for Formatted<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        F::fmt(&self.0, f)
    }
}

/// Forwards to the inner error's `Debug` rather than printing
/// `Formatted(.., PhantomData)`. Keeps `{:?}` output of wrapped errors readable.
impl<E: fmt::Debug, F> fmt::Debug for Formatted<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io;

    use thiserror::Error;

    use super::*;

    fn _assert_derive_traits() {
        #[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
        struct DummyError;
        impl fmt::Display for DummyError {
            fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
                Ok(())
            }
        }
        impl core::error::Error for DummyError {}

        fn assert_all<
            T: Clone + Copy + Default + PartialEq + Eq + core::hash::Hash + Send + Sync,
        >() {
        }
        assert_all::<Formatted<DummyError, OneLine>>();
        assert_all::<Formatted<DummyError, Tree>>();
        assert_all::<DisplaySwapDebug<DummyError>>();
        assert_all::<OneLine>();
        assert_all::<TreeMarker>();
        assert_all::<TreeIndent>();
        assert_all::<Tree>();
    }

    #[derive(Error, Debug)]
    pub enum Error {
        #[error("One")]
        One,
        #[error("Two")]
        Two(#[source] ErrorInner),
        #[error("Three")]
        Three(#[source] io::Error),
        #[error(transparent)]
        Four(#[from] ErrorInner),
    }

    #[derive(Error, Debug)]
    pub enum ErrorInner {
        #[error("One")]
        One,
        #[error("Two")]
        Two,
    }

    #[test]
    fn test_user_output() {
        let error = Error::One;
        assert_eq!(error.one_line().to_string(), "One");

        let error = Error::Two(ErrorInner::One);
        assert_eq!(error.one_line().to_string(), "Two: One");

        let error = Error::Three(io::Error::new(io::ErrorKind::PermissionDenied, "test"));
        assert_eq!(error.one_line().to_string(), "Three: test");

        let error = Error::Four(ErrorInner::Two);
        assert_eq!(error.one_line().to_string(), "Two");
    }

    #[test]
    fn test_combined() {
        let error = Error::One;
        let io_error = Error::Three(io::Error::new(io::ErrorKind::PermissionDenied, "test"));

        assert_eq!(error.one_line().to_string(), "One");

        assert_eq!(io_error.one_line().to_string(), "Three: test");
    }

    #[test]
    fn test_dyn_error() {
        let error = Error::Two(ErrorInner::One);

        let dyn_ref: &dyn core::error::Error = &error;
        assert_eq!(dyn_ref.one_line().to_string(), "Two: One");

        let boxed: Box<dyn core::error::Error> = Box::new(Error::Two(ErrorInner::Two));
        assert_eq!(boxed.one_line().to_string(), "Two: Two");

        let send_sync: &(dyn core::error::Error + Send + Sync) = &error;
        assert_eq!(send_sync.one_line().to_string(), "Two: One");
    }

    #[test]
    fn test_custom_format() {
        struct Upper;
        impl<E: core::error::Error + ?Sized> Format<E> for Upper {
            fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", error.to_string().to_uppercase())
            }
        }

        let error = Error::Two(ErrorInner::One);
        assert_eq!(error.formatted::<Upper>().to_string(), "TWO");
    }
}
