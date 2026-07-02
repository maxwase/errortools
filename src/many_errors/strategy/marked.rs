//! Shared nested-list traversal behind [`List`](super::List) and
//! [`Bullets`](super::Bullets).
//!
//! Both shapes are the same rose-tree walk; they differ only in the row marker
//! (`"1. "` vs `"• "`) and two indent offsets, captured by [`Marker`]. The
//! geometry is tracked as a single `content` column (in `"  "` units): the
//! column that foreign content — leaf chains and group labels — re-indents to
//! when it embeds newlines. Structural writes (markers, headers) stay raw.

use core::{error::Error, fmt, marker::PhantomData};

use crate::{
    Format,
    indent::{Repeat, indented},
    many_errors::{ManyErrors, Node},
    with_context::WithContext,
};

use super::{ErrorCount, Label, LeafChain, NO_ERRORS};

/// Row marker and indent offsets distinguishing a [`List`](super::List) from
/// a [`Bullets`](super::Bullets) rendering.
pub(super) trait Marker {
    /// Row indent (in `"  "` units) of top-level children.
    const TOP_ROW: usize;
    /// Offset from a group's content column to its children's row indent.
    const GROUP_ROW_OFFSET: usize;

    /// Writes the `"\n{indent}{marker} "` row prefix for child `index`.
    fn write_marker(f: &mut fmt::Formatter<'_>, indent: usize, index: usize) -> fmt::Result;
}

/// Entry point: renders `errors` as a marked nested list.
///
/// - `None` writes `"no errors"`.
/// - `One` renders the lone child flush — no header, no marker (a lone error
///   reads better inline than as `"1. …"` / `"• …"`).
/// - `Many` writes the `"N errors:"` header, then one marked row per child at
///   [`Marker::TOP_ROW`].
pub(super) fn draw_marked_many<M, C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    M: Marker,
    E: Error,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    match errors {
        ManyErrors::None => f.write_str(NO_ERRORS),
        ManyErrors::One(node) => draw_marked_node::<M, C, E, GC, F, GF>(node, 0, f),
        ManyErrors::Many(nodes) => {
            write!(f, "{}:", ErrorCount(nodes.len()))?;
            draw_children::<M, C, E, GC, F, GF>(nodes, M::TOP_ROW, f)
        }
    }
}

/// Writes one marked row per node: the `M` marker at `row`, then the node with
/// its content column one unit past the marker.
fn draw_children<M, C, E, GC, F, GF>(
    nodes: &[Node<C, E, GC, F, GF>],
    row: usize,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    M: Marker,
    E: Error,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    for (index, node) in nodes.iter().enumerate() {
        M::write_marker(f, row, index)?;
        draw_marked_node::<M, C, E, GC, F, GF>(node, row + 1, f)?;
    }
    Ok(())
}

/// Renders one node whose foreign content re-indents to column `content`.
///
/// - `Leaf` → the whole pair on one logical line via [`LeafChain`]: `{w}`
///   (context/error through `F`) followed by its `": "`-joined source chain
///   (same output as [`OneLine`](crate::OneLine), without requiring
///   `C: Debug`).
/// - `Group` → the label through `GF`, then:
///   - empty group → `": no errors"`;
///   - single child → `": "` and recurse at the *same* content column (the
///     child sits inline after the colon, not on a new marked row);
///   - many children → `" (N errors):"` header, then marked rows at
///     `content + `[`Marker::GROUP_ROW_OFFSET`].
fn draw_marked_node<M, C, E, GC, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    content: usize,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    M: Marker,
    E: Error,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    match node {
        Node::Leaf(w) => indented(f, Repeat("  ", content), LeafChain(w)),
        Node::Group(w) => {
            indented(
                f,
                Repeat("  ", content),
                Label::<_, GF>(&w.context, PhantomData),
            )?;
            match w.errors.as_ref() {
                ManyErrors::None => write!(f, ": {NO_ERRORS}"),
                ManyErrors::One(inner) => {
                    write!(f, ": ")?;
                    draw_marked_node::<M, C, E, GC, F, GF>(inner, content, f)
                }
                ManyErrors::Many(nodes) => {
                    write!(f, " ({}):", ErrorCount(nodes.len()))?;
                    draw_children::<M, C, E, GC, F, GF>(nodes, content + M::GROUP_ROW_OFFSET, f)
                }
            }
        }
    }
}
