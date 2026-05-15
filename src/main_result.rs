use core::error::Error;
use core::fmt;

use super::{Format, Formatted, OneLine};

/// A result type that wraps an error with [Formatted] and [DisplaySwapDebug] to output from the `main` function.
///
/// The format strategy `F` defaults to [`OneLine`]; pass [`crate::Tree`] or a custom [`Format`]
/// to change how the error is rendered when `main` returns `Err`.
/// The success type `T` defaults to `()`; pass `ExitCode` or another type to return from `main`.
pub type MainResult<E, F = OneLine, T = ()> =
    core::result::Result<T, DisplaySwapDebug<Formatted<E, F>>>;

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

impl<E: Error, F: Format> From<E> for DisplaySwapDebug<Formatted<E, F>> {
    fn from(value: E) -> Self {
        DisplaySwapDebug::new(Formatted::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Error;

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
        let inner = Formatted::<_, OneLine>::new(Error::Two(crate::tests::ErrorInner::One));
        let wrapped = DisplaySwapDebug::new(inner);
        // Debug of DisplaySwapDebug = Display of inner = OneLine chain.
        assert_eq!(format!("{wrapped:?}"), "Two: One");
        // Display of DisplaySwapDebug = Debug of inner = forwarded to error's Debug.
        assert_eq!(wrapped.to_string(), "Two(One)");
    }

    #[test]
    fn test_main_result() {
        assert_eq!(
            DisplaySwapDebug::new(test_return().unwrap_err()).to_string(),
            "One"
        );

        assert_eq!(
            DisplaySwapDebug::new(&DisplaySwapDebug::new(Formatted::<_, OneLine>::new(
                Error::One
            )))
            .to_string(),
            "One"
        );
    }

    #[test]
    fn test_main_result_with_exit_code() {
        use std::process::ExitCode;

        fn main_with_error(err: bool) -> MainResult<Error, OneLine, ExitCode> {
            if err {
                Err(Error::One)?;
            }
            Ok(ExitCode::SUCCESS)
        }

        assert_eq!(main_with_error(false).unwrap(), ExitCode::SUCCESS);
        assert_eq!(main_with_error(true).unwrap_err().0.to_string(), "One");
    }
}
