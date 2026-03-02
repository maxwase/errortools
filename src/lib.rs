// #![doc = include_str!("../README.md")]

use core::{error::Error, fmt};

mod main_result;
mod oneline;

pub use main_result::MainResult;
pub use oneline::FormatOneLine;

/// A helper trait to format errors.
pub trait FormatError {
    /// Formats the error in a single line concatenated by `:`.
    fn one_line(&self) -> FormatOneLine<&Self> {
        FormatOneLine::new(self)
    }

    /// Formats the error for the user output.
    /// Combines both [FormatError::one_line] and [FormatError::suggestion] methods.
    fn user_output(&self) -> UserOutput<&Self> {
        UserOutput::new(self)
    }
}

impl<E: Error + ?Sized> FormatError for E {}

/// An error formatter that outputs the error in a single line with a suggestion on the next line.
/// It basically combines the [FormatOneLine] and [FormatSuggestion] methods.
#[derive(Debug, Clone, Copy)]
pub struct UserOutput<E: ?Sized>(E);

impl<E> UserOutput<E> {
    pub fn new(error: E) -> Self {
        UserOutput(error)
    }
}

impl<E: Error + ?Sized> fmt::Display for UserOutput<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.one_line())
    }
}

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

        assert_eq!(error.user_output().to_string(), "One");

        assert_eq!(io_error.user_output().to_string(), "Three: test");
    }
}
