use core::{error::Error, fmt, iter, marker::PhantomData};

use itertools::Itertools;

use crate::{Format, chain};

/// Default tree branch marker: `"└── "`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TreeMarker;

/// Writes the literal `"└── "`.
impl fmt::Display for TreeMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("└── ")
    }
}

/// Default tree indent: four spaces.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TreeIndent;

/// Writes four spaces.
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
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tree<M = TreeMarker, I = TreeIndent>(PhantomData<fn() -> (M, I)>);

/// Walks the source chain. Prints the top error on its own line, then each
/// source on a new line preceded by `(depth - 1)` repetitions of `I` followed
/// by `M`.
impl<E: Error + ?Sized, M, I> Format<E> for Tree<M, I>
where
    M: fmt::Display + Default,
    I: fmt::Display + Default,
{
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let marker = M::default();
        let indent = I::default();
        let formatted =
            chain(&error)
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

/// Prints the marker/indent values (instantiated via [`Default`]) instead of
/// `Tree(PhantomData)`.
impl<M: fmt::Debug + Default, I: fmt::Debug + Default> fmt::Debug for Tree<M, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Tree")
            .field(&M::default())
            .field(&I::default())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use itertools::Itertools;

    use crate::{
        Format, FormatError, Formatted, Tree, TreeIndent, TreeMarker, chain,
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
    fn test_tree_marker_debug() {
        assert_eq!(format!("{:?}", TreeMarker), "TreeMarker");
        assert_eq!(format!("{:?}", TreeIndent), "TreeIndent");
    }

    #[test]
    fn test_tree_debug_default_params() {
        let tree = Tree::<TreeMarker, TreeIndent>::default();
        assert_eq!(format!("{tree:?}"), "Tree(TreeMarker, TreeIndent)");
    }

    #[test]
    fn test_tree_debug_custom_params() {
        #[derive(Debug, Default)]
        struct Arrow;
        #[derive(Debug, Default)]
        struct TwoSpace;
        let tree = Tree::<Arrow, TwoSpace>::default();
        assert_eq!(format!("{tree:?}"), "Tree(Arrow, TwoSpace)");
    }

    #[test]
    fn test_custom_tree_via_format() {
        struct AsciiTree;
        impl<E: core::error::Error + ?Sized> Format<E> for AsciiTree {
            fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let formatted = chain(&error)
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
