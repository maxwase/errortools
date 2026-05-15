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
impl Format for OneLine {
    fn fmt(error: &dyn Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(error).format(": "))
    }
}

#[cfg(test)]
mod tests {
    use core::fmt;
    use std::io;

    use itertools::Itertools;

    use crate::{
        Format, FormatError, Formatted, OneLine, chain,
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
        assert_eq!(Formatted::<_, OneLine>::new(Error::One).to_string(), "One");

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

    #[test]
    fn test_custom_separator_via_format() {
        struct Arrow;
        impl Format for Arrow {
            fn fmt(error: &dyn core::error::Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", chain(error).format(" -> "))
            }
        }

        let error = Error::Two(ErrorInner::One);
        assert_eq!(Formatted::<_, Arrow>::new(error).to_string(), "Two -> One");
    }
}
