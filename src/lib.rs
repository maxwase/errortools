// #![doc = include_str!("../README.md")]

use core::{error::Error, fmt, iter, marker::PhantomData};

mod main_result;
mod oneline;
mod tree;

pub use main_result::MainResult;
pub use oneline::{FormatOneLine, OneLine};
pub use tree::{Tree, TreeIndent, TreeMarker};

/// A static strategy for formatting an error and its source chain.
///
/// Implement on a unit type to define a custom format. Use [`chain`] to walk
/// the error and its sources.
pub trait Format {
    fn fmt(error: &dyn Error, f: &mut fmt::Formatter<'_>) -> fmt::Result;
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
    fn one_line(&self) -> FormatOneLine<&Self> {
        FormatOneLine::new(self)
    }

    /// Formats the error as an indented tree of sources.
    fn tree(&self) -> Formatted<&Self, Tree> {
        Formatted::new(self)
    }

    /// Formats the error using a custom [`Format`] strategy.
    fn formatted<F: Format>(&self) -> Formatted<&Self, F> {
        Formatted::new(self)
    }
}

impl<E: Error + ?Sized> FormatError for E {}

/// An error wrapper that uses a static [`Format`] strategy for [`Display`].
pub struct Formatted<E, F: Format = OneLine>(E, PhantomData<F>);

impl<E, F: Format> Formatted<E, F> {
    pub fn new(error: E) -> Self {
        Formatted(error, PhantomData)
    }
}

impl<E: Error, F: Format> fmt::Display for Formatted<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        F::fmt(&self.0, f)
    }
}

impl<E: fmt::Debug, F: Format> fmt::Debug for Formatted<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl<E: Clone, F: Format> Clone for Formatted<E, F> {
    fn clone(&self) -> Self {
        Formatted(self.0.clone(), PhantomData)
    }
}

impl<E: Copy, F: Format> Copy for Formatted<E, F> {}

#[cfg(test)]
pub(crate) mod tests {
    use std::io;

    use thiserror::Error;

    use super::*;

    #[derive(Error, Debug)]
    pub enum Error {
        #[error("One")]
        One,
        #[error("Two")]
        Two(#[source] ErrorInner),
        #[error("Three")]
        Three(#[from] io::Error),
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
        impl Format for Upper {
            fn fmt(error: &dyn core::error::Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", error.to_string().to_uppercase())
            }
        }

        let error = Error::Two(ErrorInner::One);
        assert_eq!(error.formatted::<Upper>().to_string(), "TWO");
    }
}
