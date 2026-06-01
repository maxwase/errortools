//! [`List`]: render a [`ManyErrors`] as a numbered list.
//!
//! `depth: usize` carries the nesting level; the visual indent is reconstructed
//! lazily with `repeat_n("  ", depth).format("")` — no `String` allocation.

use core::{
    error::Error,
    fmt::{self, Debug, Display},
    iter,
};

use itertools::Itertools;

use crate::{
    Format, OneLine,
    many_errors::{ManyErrors, Node},
    with_context::WithContext,
};

use super::impl_aggregate_format;

/// Aggregate strategy that renders a [`ManyErrors`] as a numbered list.
///
/// # Output example
/// ```text
/// 3 errors:
///   1. a: InnerA
///   2. b: InnerB
///   3. c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct List;

impl_aggregate_format!(List, |errors, f| draw_list_many::<C, E, GC, F, GF>(
    errors, 0, f
));

/// Render `errors` as a numbered list at nesting `depth`.
///
/// - `None` writes `"no errors"`.
/// - `One` delegates straight to [`draw_list_node`] with no header or number
///   (a lone error reads better inline than as `1. ...`).
/// - `Many` writes the `"N errors:"` header, then one `"{indent}{i}. "` prefix
///   per child before recursing. `indent` is `depth` copies of `"  "`, built
///   lazily via `repeat_n` so no `String` is allocated.
///
/// Children recurse at `depth + 1` so their own nested groups indent one step
/// further than this level's numbers.
fn draw_list_many<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    depth: usize,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display + Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    match errors {
        ManyErrors::None => write!(f, "no errors"),
        ManyErrors::One(node) => draw_list_node::<C, E, GC, F, GF>(node, depth, f),
        ManyErrors::Many(nodes) => {
            write!(f, "{} errors:", nodes.len())?;
            for (i, node) in nodes.iter().enumerate() {
                let indent = iter::repeat_n("  ", depth).format("");
                write!(f, "\n{indent}{}. ", i + 1)?;
                draw_list_node::<C, E, GC, F, GF>(node, depth + 1, f)?;
            }
            Ok(())
        }
    }
}

/// Render a single node; the `"{i}. "` prefix has already been written by the
/// caller.
///
/// - `Leaf` renders the whole pair on one logical line via the [`OneLine`]
///   strategy: `{w}` (context/error through `F`) followed by its source chain
///   joined with `": "` — `WithContext`'s own `Display`/`Error::source` give
///   exactly that.
/// - `Group` writes the label, then:
///   - empty group → `"{w}: no errors"`;
///   - single child → `"{w}: "` and recurse at the *same* `depth` (the child
///     is rendered inline after the colon, not as a new numbered row);
///   - many children → `"{w} (N errors):"` header, then a fresh numbered list
///     at `depth + 1`, recursing into children at `depth + 2`.
fn draw_list_node<C, E, GC, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    depth: usize,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display + Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    match node {
        Node::Leaf(w) => <OneLine as Format<_>>::fmt(w, f),
        Node::Group(w) => match w.errors.as_ref() {
            ManyErrors::None => {
                GF::fmt(&w.context, f)?;
                write!(f, ": no errors")
            }
            ManyErrors::One(inner) => {
                GF::fmt(&w.context, f)?;
                write!(f, ": ")?;
                draw_list_node::<C, E, GC, F, GF>(inner, depth, f)
            }
            ManyErrors::Many(nodes) => {
                GF::fmt(&w.context, f)?;
                write!(f, " ({} errors):", nodes.len())?;
                for (i, node) in nodes.iter().enumerate() {
                    let indent = iter::repeat_n("  ", depth + 1).format("");
                    write!(f, "\n{indent}{}. ", i + 1)?;
                    draw_list_node::<C, E, GC, F, GF>(node, depth + 2, f)?;
                }
                Ok(())
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::ManyErrors;
    use crate::many_errors::strategy::test_helpers::{two_leaves, with_chain};
    use crate::tests::Inner;

    #[test]
    fn test_list_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.list().to_string(), "no errors");
    }

    #[test]
    fn test_list_single_leaf_no_header() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        assert_eq!(e.list().to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_list_two_leaves() {
        assert_eq!(
            two_leaves().list().to_string(),
            "2 errors:\n1. a: InnerA\n2. b: InnerB"
        );
    }

    /// Leaves walk their source chain via `OneLine`.
    #[test]
    fn test_list_walks_source_chain() {
        let s = with_chain().list().to_string();
        assert!(s.contains("1. a: mid: InnerA"), "got: {s}");
        assert!(s.contains("2. b: mid: InnerB"), "got: {s}");
    }

    #[test]
    fn test_list_nested_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::A);
        outer.push_group("region", inner);

        assert_eq!(
            outer.list().to_string(),
            "2 errors:\n1. leaf: InnerA\n2. region (2 errors):\n    1. x: InnerA\n    2. y: InnerB"
        );
    }

    #[test]
    fn test_list_empty_group() {
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", ManyErrors::new());
        assert_eq!(outer.list().to_string(), "g: no errors");
    }
}
