use core::{error::Error, fmt, iter};

use itertools::Itertools;

use crate::{Format, chain};

/// Tree format with `└── ` markers and four-space indentation per depth.
///
/// ```text
/// top error
/// └── source 1
///     └── source 2
/// ```
///
/// For a different marker or indent, implement [`Format`] yourself using
/// [`chain`].
pub struct Tree;

impl Format for Tree {
    fn fmt(error: &dyn Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted =
            chain(error)
                .enumerate()
                .format_with("\n", |(depth, e), write| match depth {
                    0 => write(&format_args!("{e}")),
                    n => {
                        let indent = iter::repeat_n("    ", n - 1).format("");
                        write(&format_args!("{indent}└── {e}"))
                    }
                });
        write!(f, "{formatted}")
    }
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use itertools::Itertools;

    use crate::{
        Format, FormatError, Formatted, chain,
        tests::{Error, ErrorInner},
    };

    #[test]
    fn test_tree_no_source() {
        let error = Error::One;
        assert_eq!(error.tree().to_string(), "One");
    }

    #[test]
    fn test_tree_one_source() {
        let error = Error::Two(ErrorInner::One);
        assert_eq!(error.tree().to_string(), "Two\n└── One");
    }

    #[test]
    fn test_tree_nested() {
        let error = Error::Two(ErrorInner::Two);
        assert_eq!(error.tree().to_string(), "Two\n└── Two");
    }

    #[test]
    fn test_custom_tree_via_format() {
        struct AsciiTree;
        impl Format for AsciiTree {
            fn fmt(error: &dyn core::error::Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let formatted = chain(error)
                    .enumerate()
                    .format_with("\n", |(depth, e), write| match depth {
                        0 => write(&format_args!("{e}")),
                        n => write(&format_args!("{:width$}|-- {e}", "", width = (n - 1) * 2)),
                    });
                write!(f, "{formatted}")
            }
        }

        let error = Error::Two(ErrorInner::One);
        assert_eq!(
            Formatted::<_, AsciiTree>::new(error).to_string(),
            "Two\n|-- One"
        );
    }
}
