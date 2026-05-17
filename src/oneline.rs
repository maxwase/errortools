use core::{error::Error, fmt};

use itertools::Itertools;

use crate::{Format, chain};

/// One-line format. Joins the error and its sources with `": "`.
///
/// For a different separator (or any per-element formatting), implement
/// [`Format`] yourself using [`chain`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OneLine;

/// Walks the source chain and joins each error's `Display` output with `": "`.
impl<E: Error + ?Sized> Format<E> for OneLine {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(&error).format(": "))
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use crate::{
        FormatError, Formatted, OneLine,
        tests::{Arrow, Error, Inner},
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
        assert_eq!(Formatted::<_, OneLine>::new(Error::One).to_string(), "One");

        let error = Error::Two(Inner::A);
        assert_eq!(error.one_line().to_string(), "Two: InnerA");
        assert_eq!(format!("{:?}", error.one_line()), "Two(A)");
    }

    #[test]
    fn test_from() {
        // `#[from] io::Error` provides both the From impl and the source link.
        let error: Error = io::Error::other("test").into();
        assert_eq!(error.one_line().to_string(), "Three: test");
    }

    #[test]
    fn test_custom_separator_via_format() {
        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Arrow>::new(error).to_string(),
            "Two -> InnerA"
        );
    }
}
