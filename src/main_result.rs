//! [`MainResult`] and [`MainError`]: returning a formatted error from `main`.

use core::{error::Error, fmt, marker::PhantomData};

use derive_where::derive_where;

use crate::separator::{NewLine, Then};

use super::{Format, OneLine};

/// A result type for `fn main` whose error is a [`MainError`], rendered via the strategy `F`.
///
/// The format strategy `F` defaults to [`OneLine`]; pass [`Chain`](crate::Chain) or a custom [`Format`]
/// to change how the error is rendered when `main` returns `Err`.
/// The success type `T` defaults to `()`; pass `ExitCode` or another type to return from `main`.
///
/// `?` converts any error that `E` implements [`From`] for, so a source error
/// reaches `main` in one step:
/// ```
/// use errortools::MainResult;
/// use std::{fs, io};
///
/// #[derive(Debug, thiserror::Error)]
/// enum AppError {
///     #[error("Failed to read config")]
///     Io(#[from] io::Error),
/// }
///
/// fn run() -> MainResult<AppError> {
///     fs::read_to_string("no/such/dir/config.toml")?; // io::Error -> AppError -> MainError
///     Ok(())
/// }
///
/// assert!(format!("{:?}", run().unwrap_err()).starts_with("Failed to read config: "));
/// ```
pub type MainResult<E, F = OneLine, T = ()> = core::result::Result<T, MainError<E, F>>;

/// A [`MainResult`] that renders the error's [`Suggestion`](crate::Suggestion) after the error itself.
///
/// See [`MainResult`] for details on the type parameters.
/// The suggestion is rendered after the error, separated by a newline. To customize the separator, use `MainResult` with a custom `Format` that combines the error and suggestion as desired.
/// If the error has no suggestion (the hint renders empty), the newline is not printed either.
pub type MainResultWithSuggestion<E, F = OneLine, T = ()> =
    core::result::Result<T, MainError<E, WithSuggestion<F>>>;

/// The error half of [`MainResult`]: owns an error `E` and renders it via the
/// static [`Format`] strategy `F`.
///
/// `main` prints a returned error via [`fmt::Debug`], so both `Debug` and
/// [`fmt::Display`] of this wrapper run `F` over the error and produce the
/// same human-readable text.
///
/// `F` is a type-level tag (never instantiated), as in [`Formatted`](crate::Formatted).
// Must never implement `core::error::Error` (nor may `Formatted`): the blanket
// `From<S: Error>` impl below would then overlap with std's `From<T> for T`.
#[derive_where(Clone, Copy, Default, PartialEq, Eq, Hash; E)]
#[must_use = "a `MainError` should be returned from `main` or displayed"]
pub struct MainError<E, F = OneLine>(E, PhantomData<fn() -> F>);

impl<E, F> MainError<E, F> {
    /// Wraps `error` so that it is rendered via the [`Format`] strategy `F`.
    pub const fn new(error: E) -> Self {
        MainError(error, PhantomData)
    }

    /// Returns a reference to the wrapped error.
    pub const fn inner(&self) -> &E {
        &self.0
    }

    /// Unwraps the error, discarding the strategy.
    pub fn into_inner(self) -> E {
        self.0
    }
}

/// Renders the wrapped error via the strategy `F`. This is what `main` prints
/// when it returns `Err`.
impl<E, F: Format<E>> fmt::Debug for MainError<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        F::fmt(&self.0, f)
    }
}

/// Renders the wrapped error via the strategy `F`, same as [`fmt::Debug`].
impl<E, F: Format<E>> fmt::Display for MainError<E, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        F::fmt(&self.0, f)
    }
}

/// Lets `?` lift any source error `S` that `E` can be built from, so both
/// `E` itself and errors one `From` hop away from `E` convert.
impl<S: Error, E: From<S>, F> From<S> for MainError<E, F> {
    fn from(source: S) -> Self {
        MainError::new(E::from(source))
    }
}

/// A helper type to combine an error format strategy `F` with a suggestion, separated by `Sep`.
/// Used by `MainResultWithSuggestion` to render the error and suggestion together.
/// `F` defaults to [`OneLine`] and `Sep` defaults to a newline, but you can customize both to achieve different layouts.
///
/// Equivalent to `(F, Then<Sep, Suggestion>)`: [`Then`] writes the separator
/// only when the suggestion renders non-empty, so an error without a hint
/// leaves no trailing separator.
pub type WithSuggestion<F = OneLine, Sep = NewLine> = (F, Then<Sep, crate::Suggestion>);

/// Wrapper that swaps an inner type's [`fmt::Debug`] and [`fmt::Display`] impls.
///
///
/// ### Use-case
/// `main` prints the returned error via [`fmt::Debug`]. Wrapping a `Display`
/// type in `DisplaySwapDebug` makes that `Debug` print produce the `Display`
/// output instead. [`MainError`] is the purpose-built wrapper for `main`; this
/// one is a standalone utility for any other `Debug`-printing context.
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use = "a formatting wrapper does nothing unless printed"]
pub struct DisplaySwapDebug<T>(T);

impl<T> From<T> for DisplaySwapDebug<T> {
    fn from(value: T) -> Self {
        DisplaySwapDebug(value)
    }
}

impl<T> DisplaySwapDebug<T> {
    /// Wraps `value`, swapping its `Debug` and `Display` impls.
    pub const fn new(value: T) -> Self {
        DisplaySwapDebug(value)
    }

    /// Returns a reference to the wrapped value.
    pub const fn inner(&self) -> &T {
        &self.0
    }

    /// Unwraps the value.
    pub fn into_inner(self) -> T {
        self.0
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

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use crate::{
        Formatted,
        separator::Space,
        tests::{Error, Inner},
    };

    /// `From<io::Error>` only; drives the double-hop `?` in [`double_hop`].
    #[derive(Debug, thiserror::Error)]
    enum AppError {
        #[error("Failed to read config")]
        Io(#[from] io::Error),
    }

    fn failing_io() -> Result<(), io::Error> {
        Err(io::Error::new(io::ErrorKind::NotFound, "file missing"))
    }

    /// `?` goes `io::Error -> AppError -> MainError<AppError>` with no `map_err`.
    fn double_hop() -> MainResult<AppError> {
        failing_io()?;
        Ok(())
    }

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
        let inner = Formatted::<_, OneLine>::new(Error::Two(Inner::A));
        let wrapped = DisplaySwapDebug::new(inner);
        // Debug of DisplaySwapDebug = Display of inner = OneLine chain.
        assert_eq!(format!("{wrapped:?}"), "Two: InnerA");
        // Display of DisplaySwapDebug = Debug of inner Formatted = error + strategy.
        assert_eq!(
            wrapped.to_string(),
            "Formatted { error: Two(A), format: OneLine }"
        );
    }

    #[test]
    fn test_main_result() {
        let err = test_return().unwrap_err();
        // Debug is what `main` prints; Display renders the same text.
        assert_eq!(format!("{err:?}"), "One");
        assert_eq!(err.to_string(), "One");
    }

    #[test]
    fn test_main_result_double_hop_question_mark() {
        let err = double_hop().unwrap_err();
        assert_eq!(format!("{err:?}"), "Failed to read config: file missing");
        assert!(matches!(err.inner(), AppError::Io(e) if e.kind() == io::ErrorKind::NotFound));
    }

    #[test]
    fn test_main_error_accessors() {
        let err = MainError::<_, OneLine>::new(Error::Two(Inner::A));
        assert!(matches!(err.inner(), Error::Two(Inner::A)));
        assert!(matches!(err.into_inner(), Error::Two(Inner::A)));
    }

    #[test]
    fn test_main_error_custom_strategy() {
        fn run() -> MainResult<Error, crate::Chain> {
            Err(Error::Two(Inner::A))?;
            Ok(())
        }

        assert_eq!(format!("{:?}", run().unwrap_err()), "Two\n└─ InnerA");
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
    fn test_with_suggestion_empty_hint_skips_separator() {
        let formatted = Formatted::<_, WithSuggestion>::new(Error::Two(Inner::A));
        assert_eq!(formatted.to_string(), "Two: InnerA");
    }

    #[test]
    fn test_main_result_with_suggestion_hintless_has_no_trailing_newline() {
        fn run() -> MainResultWithSuggestion<Error> {
            Err(Error::Two(Inner::A))?;
            Ok(())
        }

        // Debug is what `main` prints.
        assert_eq!(format!("{:?}", run().unwrap_err()), "Two: InnerA");
    }

    #[test]
    fn test_with_suggestion_custom_separator() {
        let formatted = Formatted::<_, WithSuggestion<OneLine, Space>>::new(Error::One);
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
        // Debug of MainError runs the strategy = error chain + \n + hint.
        assert_eq!(
            format!("{wrapped:?}"),
            "One\nTry passing --help to see available options."
        );
        // Display renders the same text as Debug.
        assert_eq!(
            wrapped.to_string(),
            "One\nTry passing --help to see available options."
        );
    }

    #[test]
    fn test_main_result_with_suggestion_exit_code() {
        use std::process::ExitCode;

        fn main_with_error(err: bool) -> MainResultWithSuggestion<Error, OneLine, ExitCode> {
            if err {
                Err(Error::One)?;
            }
            Ok(ExitCode::SUCCESS)
        }

        assert_eq!(main_with_error(false).unwrap(), ExitCode::SUCCESS);
        let wrapped = main_with_error(true).unwrap_err();
        assert_eq!(
            format!("{wrapped:?}"),
            "One\nTry passing --help to see available options."
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
        assert_eq!(format!("{:?}", main_with_error(true).unwrap_err()), "One");
    }
}
