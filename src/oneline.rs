use core::{error::Error, fmt};

/// A formatter that outputs the error in a single line concatenated by `:`.
pub struct FormatOneLine<E>(E);

impl<E> FormatOneLine<E> {
    pub fn new(error: E) -> Self {
        FormatOneLine(error)
    }
}

impl<E: Error> fmt::Display for FormatOneLine<E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut error = &self.0 as &dyn Error;

        fmt::Display::fmt(error, fmt)?;

        while let Some(source) = error.source() {
            write!(fmt, ": {source}")?;
            error = source;
        }
        Ok(())
    }
}

impl<E: fmt::Debug> fmt::Debug for FormatOneLine<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use crate::{
        FormatError,
        oneline::FormatOneLine,
        tests::{Error, ErrorInner},
    };

    #[test]
    fn test_io_error() {
        let error = io::Error::other("test");
        assert_eq!(error.one_line().to_string(), "test");

        let error = io::Error::other(error);
        assert_eq!(error.one_line().to_string(), "test");
    }

    #[test]
    fn test_one_line_variants() {
        let error = Error::One;
        assert_eq!(error.one_line().to_string(), "One");
        assert_eq!(format!("{:?}", error.one_line()), "One");
        assert_eq!(FormatOneLine::new(Error::One).to_string(), "One");

        let error = Error::Two(ErrorInner::One);
        assert_eq!(error.one_line().to_string(), "Two: One");
        assert_eq!(format!("{:?}", error.one_line()), "Two(One)");
    }

    #[test]
    fn test_from() {
        let error = Error::Three(io::Error::other("test"));
        assert_eq!(error.one_line().to_string(), "Three: test");
        assert_eq!(
            format!("{:?}", error.one_line()),
            "Three(Custom { kind: Other, error: \"test\" })"
        );

        let error = Error::Four(ErrorInner::Two);
        assert_eq!(error.one_line().to_string(), "Two");
        assert_eq!(format!("{:?}", error.one_line()), "Four(Two)");
    }
}
