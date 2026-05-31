//! [`List`]: render a [`ManyErrors`] as a numbered list.
//!
//! `depth: usize` carries the nesting level; the visual indent is reconstructed
//! lazily with `repeat_n("  ", depth).format("")` — no `String` allocation.

use core::{
    error::Error,
    fmt::{self, Display},
    iter,
};

use itertools::Itertools;

use crate::{
    Format,
    many_errors::{ManyErrors, Node, Subgroup},
    with_context::WithContext,
};

use super::{impl_aggregate_format, inline_sources};

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
/// - `None` writes nothing.
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
    C: Display,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    match errors {
        ManyErrors::None => Ok(()),
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
/// - `Leaf` writes the context/error pair (`{w}`), then appends its source
///   chain inline via [`inline_sources`] (`": src1: src2"`) — lists keep each
///   entry on one logical line.
/// - `Group` writes the label, then:
///   - empty group → `"{w}: (no errors)"`;
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
        Node::Group(w) => match w.error.as_ref() {
            ManyErrors::None => write!(f, "{w}: (no errors)"),
            ManyErrors::One(inner) => {
                write!(f, "{w}: ")?;
                draw_list_node::<C, E, GC, F, GF>(inner, depth, f)
            }
            ManyErrors::Many(nodes) => {
                write!(f, "{w} ({} errors):", nodes.len())?;
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
    use crate::many_errors::strategy::test_helpers::two_leaves;

    #[test]
    fn test_list_two_leaves() {
        let e = two_leaves();
        let s = e.list().to_string();
        assert!(s.contains("2 errors:"), "got: {s}");
        assert!(s.contains("1. a: InnerA"), "got: {s}");
        assert!(s.contains("2. b: InnerB"), "got: {s}");
    }
}
