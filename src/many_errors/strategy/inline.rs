//! [`Inline`]: render a [`ManyErrors`] on a single line.

use core::{
    error::Error,
    fmt::{self, Display},
};

use crate::{
    Format,
    many_errors::{ManyErrors, Node, Subgroup},
    with_context::WithContext,
};

use super::{impl_aggregate_format, inline_sources};

/// Aggregate strategy that renders a [`ManyErrors`] on a single line.
///
/// Siblings are separated by `"; "`. Nested groups are wrapped in parens.
///
/// # Output example
/// ```text
/// 3 errors: a: InnerA; b: InnerB; c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Inline;

impl_aggregate_format!(Inline, |errors, f| draw_inline_many::<C, E, GC, F, GF>(
    errors, f
));

/// Render `errors` on the current line, no newlines.
///
/// - `None` writes nothing.
/// - `One` delegates to [`draw_inline_node`] with no header.
/// - `Many` writes the `"N errors: "` header, then each child separated by
///   `"; "`. A `first` flag suppresses the separator before the first child so
///   it isn't led by a stray `"; "`.
fn draw_inline_many<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    match errors {
        ManyErrors::None => Ok(()),
        ManyErrors::One(node) => draw_inline_node::<C, E, GC, F, GF>(node, f),
        ManyErrors::Many(nodes) => {
            write!(f, "{} errors: ", nodes.len())?;
            let mut first = true;
            for node in nodes {
                if !first {
                    write!(f, "; ")?;
                }
                first = false;
                draw_inline_node::<C, E, GC, F, GF>(node, f)?;
            }
            Ok(())
        }
    }
}

/// Render a single node inline.
///
/// - `Leaf` → `{w}` plus its source chain via [`inline_sources`].
/// - `Group` → `"{w} ("`, the nested aggregate rendered recursively by
///   [`draw_inline_many`], then a closing `")"`. Parens bracket each nested
///   group so depth stays unambiguous on one line.
fn draw_inline_node<C, E, GC, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    match node {
        Node::Leaf(w) => {
            write!(f, "{w}")?;
            inline_sources(w.error.source(), f)
        }
        Node::Group(w) => {
            write!(f, "{w} (")?;
            draw_inline_many::<C, E, GC, F, GF>(w.error.as_ref(), f)?;
            write!(f, ")")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ManyErrors;
    use crate::many_errors::strategy::test_helpers::two_leaves;
    use crate::tests::{Inner, Mid};

    #[test]
    fn test_inline_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.one_line().to_string(), "");
    }

    #[test]
    fn test_inline_single() {
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("ctx", Mid::Inner(Inner::A));
        assert_eq!(e.one_line().to_string(), "ctx: mid: InnerA");
    }

    #[test]
    fn test_inline_two() {
        let e = two_leaves();
        let s = e.one_line().to_string();
        assert!(s.contains("2 errors:"), "got: {s}");
        assert!(s.contains("a: InnerA"), "got: {s}");
        assert!(s.contains("b: InnerB"), "got: {s}");
        assert!(s.contains("; "), "got: {s}");
    }
}
