use core::{error::Error, fmt, iter, marker::PhantomData};

use itertools::Itertools;

use crate::{Format, chain};

/// Default tree branch marker: `"└── "`.
#[derive(Default, Debug)]
pub struct TreeMarker;

impl fmt::Display for TreeMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("└── ")
    }
}

/// Default tree indent: four spaces.
#[derive(Default, Debug)]
pub struct TreeIndent;

impl fmt::Display for TreeIndent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("    ")
    }
}

/// Tree format with a configurable marker and indent.
///
/// ```text
/// top error
/// └── source 1
///     └── source 2
/// ```
///
/// The marker (`└── ` by default) is printed before each source, and the
/// indent (four spaces by default) is repeated `depth - 1` times. Any types
/// implementing [`Display`](fmt::Display) and [`Default`] can be substituted
/// to customize the rendering.
#[derive(Debug)]
pub struct Tree<M = TreeMarker, I = TreeIndent>(PhantomData<fn() -> (M, I)>);

impl<M, I> Format for Tree<M, I>
where
    M: fmt::Display + Default,
    I: fmt::Display + Default,
{
    fn fmt(error: &dyn Error, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let marker = M::default();
        let indent = I::default();
        let formatted =
            chain(error)
                .enumerate()
                .format_with("\n", |(depth, e), write| match depth {
                    0 => write(&format_args!("{e}")),
                    n => {
                        let pad = iter::repeat_n(&indent, n - 1).format("");
                        write(&format_args!("{pad}{marker}{e}"))
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
        Format, FormatError, Formatted, Tree, chain,
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
    fn test_tree_custom_marker_and_indent() {
        #[derive(Default)]
        struct Arrow;
        impl fmt::Display for Arrow {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("|-> ")
            }
        }

        #[derive(Default)]
        struct TwoSpace;
        impl fmt::Display for TwoSpace {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("  ")
            }
        }

        let error = Error::Two(ErrorInner::One);
        assert_eq!(
            Formatted::<_, Tree<Arrow, TwoSpace>>::new(error).to_string(),
            "Two\n|-> One"
        );
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
