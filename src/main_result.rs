use core::error::Error;
use core::fmt;

use super::{Format, Formatted, OneLine};

/// A result type that wraps an error with [Formatted] and [DisplaySwapDebug] to output from the `main` function.
///
/// The format strategy `F` defaults to [`OneLine`]; pass [`crate::Tree`] or a custom [`Format`]
/// to change how the error is rendered when `main` returns `Err`.
pub type MainResult<E, F = OneLine> = core::result::Result<(), DisplaySwapDebug<Formatted<E, F>>>;

#[derive(Copy, Clone)]
pub struct DisplaySwapDebug<E>(E);

impl<E> From<E> for DisplaySwapDebug<E> {
    fn from(value: E) -> Self {
        DisplaySwapDebug(value)
    }
}

impl<E> DisplaySwapDebug<E> {
    pub fn new(error: E) -> Self {
        DisplaySwapDebug(error)
    }
}

impl<D: fmt::Debug> fmt::Display for DisplaySwapDebug<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

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
}
