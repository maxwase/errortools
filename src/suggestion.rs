use core::{error::Error, fmt};

use crate::Format;

/// A suggestion for how to fix an error.
///
/// Only the top-level error's hint is printed, the source chain isn't walked.
/// This decision is intentional: The underlying hint may be irrelevant in the context of the top-level error, and printing it may just add noise.
/// The idea is that every error that is supposed to have a suggestion should implement [`Suggest`]
/// and then later the top-level error's suggestion may concatenate the inner hint if it's relevant
/// with nesting matching the error chain.
///
/// Implement on an error type to provide a per-variant hint (e.g.
/// `"Did you mean to rename .env.example to .env?"`). The default impl writes
/// nothing, so types only need to implement it for the variants that have a
/// hint.
///
/// Render via [`FormatError::suggestion`](crate::FormatError::suggestion),
/// which dispatches through the [`Suggestion`] strategy.
pub trait Suggest {
    /// Writes the suggestion text for `self` to `f`. The default writes
    /// nothing.
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

/// Similar blanket impl as in fmt::Display
impl<T: Suggest + ?Sized> Suggest for &T {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Suggest::fmt(*self, f)
    }
}

/// [`Format`] strategy that renders the top-level error's [`Suggestion`] hint.
///
/// The source chain is not walked — only the wrapped error's own hint is
/// printed. Pair with [`FormatError::suggestion`](crate::FormatError::suggestion)
/// (returns `Formatted<&Self, Suggestion>`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Suggestion;

impl<E: Error + Suggest + ?Sized> Format<E> for Suggestion {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Suggest::fmt(error, f)
    }
}

#[cfg(test)]
mod tests {
    use thiserror::Error;

    use super::*;
    use crate::FormatError;

    #[derive(Error, Debug)]
    pub enum SugError {
        #[error("env file missing")]
        NoEnv,
        #[error("something else")]
        Other,
    }

    impl Suggest for SugError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::NoEnv => f.write_str("Did you mean rename the .env.example file to .env?"),
                Self::Other => Ok(()),
            }
        }
    }

    #[derive(Error, Debug)]
    #[error("plain")]
    struct NoHint;

    impl Suggest for NoHint {}

    #[test]
    fn renders_variant_hint() {
        let error = SugError::NoEnv;
        assert_eq!(
            error.suggestion().to_string(),
            "Did you mean rename the .env.example file to .env?"
        );
    }

    #[test]
    fn renders_empty_for_variant_without_hint() {
        let error = SugError::Other;
        assert_eq!(error.suggestion().to_string(), "");
    }

    #[test]
    fn default_impl_writes_nothing() {
        let error = NoHint;
        assert_eq!(error.suggestion().to_string(), "");
    }

    #[test]
    fn debug_forwards_to_inner() {
        let error = SugError::NoEnv;
        assert_eq!(format!("{:?}", error.suggestion()), "NoEnv");
    }

    #[test]
    fn one_line_still_works_on_suggestion_types() {
        let error = SugError::NoEnv;
        assert_eq!(error.one_line().to_string(), "env file missing");
    }
}
