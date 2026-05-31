use core::error::Error;
use core::fmt;

use crate::separator::{NewLine, WithSep};

use super::{Flat, Format, Formatted};

/// A result type that wraps an error with [Formatted] and [DisplaySwapDebug] to output from the `main` function.
///
/// The format strategy `F` defaults to [`Flat`]; pass [`crate::Tree`] or a custom [`Format`]
/// to change how the error is rendered when `main` returns `Err`.
/// The success type `T` defaults to `()`; pass `ExitCode` or another type to return from `main`.
pub type MainResult<E, F = Flat, T = ()> =
    core::result::Result<T, DisplaySwapDebug<Formatted<E, F>>>;

/// A result type that wraps an error with [Formatted] and [DisplaySwapDebug] to output from the `main` function, with an additional suggestion.
///
/// See [`MainResult`] for details on the type parameters.
/// The suggestion is rendered after the error, separated by a newline. To customize the separator, use `MainResult` with a custom `Format` that combines the error and suggestion as desired.
/// If [`Suggestion::fmt`](crate::Suggestion::fmt) produces an empty string, the separator is still printed.
pub type MainResultWithSuggestion<E, F = Flat, T = ()> =
    core::result::Result<T, DisplaySwapDebug<Formatted<E, WithSuggestion<F, NewLine>>>>;

/// A helper type to combine an error format strategy `F` with a suggestion, separated by `Sep`.
/// Used by `MainResultWithSuggestion` to render the error and suggestion together.
/// `F` defaults to [`Flat`] and `Sep` defaults to a newline, but you can customize both to achieve different layouts.
///
/// Equivalent to [`WithSep<F, Sep, Suggestion>`].
/// If [`Suggestion::fmt`](crate::Suggestion::fmt) produces an empty string, the separator is still printed.
pub type WithSuggestion<F = Flat, Sep = NewLine> = WithSep<F, Sep, crate::Suggestion>;

/// Wrapper that swaps an inner type's [`fmt::Debug`] and [`fmt::Display`] impls.
///
///
/// ### Use-case
/// `main` prints the returned error via [`fmt::Debug`]. Wrapping a `Display`
/// type in `DisplaySwapDebug` makes that `Debug` print produce the `Display`
/// output instead — used by [`MainResult`] to render the error chain cleanly.
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplaySwapDebug<T>(T);

impl<T> From<T> for DisplaySwapDebug<T> {
    fn from(value: T) -> Self {
        DisplaySwapDebug(value)
    }
}

impl<T> DisplaySwapDebug<T> {
    /// Wraps `error`, swapping its `Debug` and `Display` impls.
    pub fn new(value: T) -> Self {
        DisplaySwapDebug(value)
    }
}

/// Prints the inner value's `Debug` representation. This is `Display` only
/// because the wrapper's purpose is to feed a `Debug`-printing context (`main`)
/// with `Display`-flavored output via the [`fmt::Debug`] impl below.
impl<D: fmt::Debug> fmt::Display for DisplaySwapDebug<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

/// Prints the inner value's `Display` representation. Used by `main` when it
/// formats a returned error via `Debug`, yielding human-readable output.
impl<D: fmt::Display> fmt::Debug for DisplaySwapDebug<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<E: Error, F: Format<E>> From<E> for DisplaySwapDebug<Formatted<E, F>> {
    fn from(value: E) -> Self {
        DisplaySwapDebug::new(Formatted::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        separator::Space,
        tests::{Error, Inner},
    };

    struct Foo;

    impl fmt::Debug for Foo {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("Debug")
        }
    }

    impl fmt::Display for Foo {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("Display")
        }
    }

    fn test_return() -> MainResult<Error> {
        Err(Error::One)?;
        Ok(())
    }

    #[test]
    fn test_swap() {
        let result = DisplaySwapDebug::new(Foo);
        assert_eq!(format!("{result:?}"), "Display");
        assert_eq!(result.to_string(), "Debug");
    }

    #[test]
    fn test_swap_with_formatted() {
        let inner = Formatted::<_, Flat>::new(Error::Two(Inner::A));
        let wrapped = DisplaySwapDebug::new(inner);
        // Debug of DisplaySwapDebug = Display of inner = Flat chain.
        assert_eq!(format!("{wrapped:?}"), "Two: InnerA");
        // Display of DisplaySwapDebug = Debug of inner Formatted = error + strategy.
        assert_eq!(
            wrapped.to_string(),
            "Formatted { error: Two(A), format: Flat }"
        );
    }

    #[test]
    fn test_main_result() {
        assert_eq!(
            DisplaySwapDebug::new(test_return().unwrap_err()).to_string(),
            "One"
        );

        assert_eq!(
            DisplaySwapDebug::new(&DisplaySwapDebug::new(Formatted::<_, Flat>::new(
                Error::One
            )))
            .to_string(),
            "One"
        );
    }

    #[test]
    fn test_with_suggestion_renders_error_then_hint() {
        let formatted = Formatted::<_, WithSuggestion>::new(Error::One);
        assert_eq!(
            formatted.to_string(),
            "One\nTry passing --help to see available options."
        );
    }

    #[test]
    fn test_with_suggestion_empty_hint_keeps_separator() {
        let formatted = Formatted::<_, WithSuggestion>::new(Error::Two(Inner::A));
        assert_eq!(formatted.to_string(), "Two: InnerA\n");
    }

    #[test]
    fn test_with_suggestion_custom_separator() {
        let formatted = Formatted::<_, WithSuggestion<Flat, Space>>::new(Error::One);
        assert_eq!(
            formatted.to_string(),
            "One Try passing --help to see available options."
        );
    }

    #[test]
    fn test_main_result_with_suggestion_question_mark() {
        fn run(err: bool) -> MainResultWithSuggestion<Error> {
            if err {
                Err(Error::One)?;
            }
            Ok(())
        }

        run(false).unwrap();
        let wrapped = run(true).unwrap_err();
        // Debug of DisplaySwapDebug forwards to inner Display = error chain + \n + hint.
        assert_eq!(
            format!("{wrapped:?}"),
            "One\nTry passing --help to see available options."
        );
        // Display of DisplaySwapDebug = Debug of inner Formatted = error + strategy.
        assert_eq!(
            wrapped.to_string(),
            "Formatted { error: One, format: Add(Add(Flat, NewLine), Suggestion) }"
        );
    }

    #[test]
    fn test_main_result_with_suggestion_exit_code() {
        use std::process::ExitCode;

        fn main_with_error(err: bool) -> MainResultWithSuggestion<Error, Flat, ExitCode> {
            if err {
                Err(Error::One)?;
            }
            Ok(ExitCode::SUCCESS)
        }

        assert_eq!(main_with_error(false).unwrap(), ExitCode::SUCCESS);
        let wrapped = main_with_error(true).unwrap_err();
        assert_eq!(
            wrapped.0.to_string(),
            "One\nTry passing --help to see available options."
        );
    }

    #[test]
    fn test_main_result_with_exit_code() {
        use std::process::ExitCode;

        fn main_with_error(err: bool) -> MainResult<Error, Flat, ExitCode> {
            if err {
                Err(Error::One)?;
            }
            Ok(ExitCode::SUCCESS)
        }

        assert_eq!(main_with_error(false).unwrap(), ExitCode::SUCCESS);
        assert_eq!(main_with_error(true).unwrap_err().0.to_string(), "One");
    }
}
