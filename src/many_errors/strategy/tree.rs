//! [`Tree`]: render a [`ManyErrors`] as a branching box-drawing tree.
//!
//! No `String` allocations. The ancestry path is encoded as `levels: Vec<bool>`,
//! one bool per ancestor depth — `true` if that ancestor was the last child,
//! `false` otherwise. At each write the VERT/GAP prefix is reconstructed from
//! `levels` via an itertools lazy format — O(depth) work, zero heap per line.

use core::{
    error::Error,
    fmt::{self, Display},
    marker::PhantomData,
};

use derive_where::derive_where;

use alloc::vec::Vec;

use itertools::Itertools;

use crate::{
    Format,
    connectors::{TreeConnectors, Unicode},
    many_errors::{ManyErrors, Node},
    with_context::WithContext,
};

/// Aggregate strategy that renders a [`ManyErrors`] as a branching tree.
///
/// Generic parameters:
/// - `Conn`: box-drawing character set ([`Unicode`] by default).
/// - `HEADER`: whether to print `"N errors:"` for levels with 2+ children (`true` by default).
///
/// # Output example (defaults)
/// ```text
/// 2 errors:
/// ├─ us-east-1 (2 errors):
/// │  ├─ i-0a1: connection timed out
/// │  └─ i-0b2: connection refused
/// └─ eu-west-1: quota exceeded
/// ```
#[derive_where(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Tree<Conn = Unicode, const HEADER: bool = true>(PhantomData<fn() -> Conn>);

impl<Conn: fmt::Debug + Default, const HEADER: bool> fmt::Debug for Tree<Conn, HEADER> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tree")
            .field("connectors", &Conn::default())
            .field("header", &HEADER)
            .finish()
    }
}

impl<C, GC, E, F, GF, Conn, const HEADER: bool> Format<ManyErrors<C, E, GC, F, GF>>
    for Tree<Conn, HEADER>
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
    Conn: TreeConnectors,
{
    fn fmt(errors: &ManyErrors<C, E, GC, F, GF>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // One Vec allocation per fmt call, shared across all recursive descent.
        let mut levels = Vec::new();
        draw_many::<Conn, C, GC, E, F, GF>(errors, &mut levels, HEADER, f)
    }
}

impl<C, GC, E, F, GF, Conn, const HEADER: bool> Format<&ManyErrors<C, E, GC, F, GF>>
    for Tree<Conn, HEADER>
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
    Conn: TreeConnectors,
{
    fn fmt(errors: &&ManyErrors<C, E, GC, F, GF>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as Format<ManyErrors<C, E, GC, F, GF>>>::fmt(errors, f)
    }
}

/// Lazily renders an ancestry prefix: one `VERT`/`GAP` per `levels` entry (a
/// bar for ancestors with siblings below, blank otherwise), then `extra`
/// trailing `GAP`s. Reusable and allocation-free.
struct Pad<'a, Conn> {
    levels: &'a [bool],
    extra: usize,
    _conn: PhantomData<fn() -> Conn>,
}

impl<Conn: TreeConnectors> Display for Pad<'_, Conn> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &last in self.levels {
            f.write_str(if last { Conn::GAP } else { Conn::VERT })?;
        }
        for _ in 0..self.extra {
            f.write_str(Conn::GAP)?;
        }
        Ok(())
    }
}

/// A [`fmt::Write`] adapter that re-emits `prefix` after every newline, so a
/// node whose rendered content spans multiple physical lines keeps the tree
/// indent instead of spilling flush-left. Streams line-by-line — no allocation.
struct Indented<'a, 'b, P: Display> {
    inner: &'a mut fmt::Formatter<'b>,
    prefix: P,
}

impl<P: Display> fmt::Write for Indented<'_, '_, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut lines = s.split('\n');
        if let Some(first) = lines.next() {
            self.inner.write_str(first)?;
        }
        for line in lines {
            write!(self.inner, "\n{}", self.prefix)?;
            self.inner.write_str(line)?;
        }
        Ok(())
    }
}

/// Writes `content` to `f`, re-indenting any embedded newlines to the prefix
/// `Pad { levels, extra }` so multi-line content stays under the tree.
fn indented<Conn: TreeConnectors>(
    f: &mut fmt::Formatter<'_>,
    levels: &[bool],
    extra: usize,
    content: impl Display,
) -> fmt::Result {
    use fmt::Write as _;
    let prefix = Pad::<Conn> {
        levels,
        extra,
        _conn: PhantomData,
    };
    write!(Indented { inner: f, prefix }, "{content}")
}

/// Renders a group label through its label strategy `GF`, wrapped so it can be
/// handed to [`indented`] (whose re-indentation needs a single [`Display`] value).
/// This is the label-only path — the group's own [`Display`] would also summarize
/// the nested errors, which the tree draws itself as branches.
struct Label<'a, GC: ?Sized, GF>(&'a GC, PhantomData<fn() -> GF>);

impl<GC: ?Sized, GF: Format<GC>> Display for Label<'_, GC, GF> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        GF::fmt(self.0, f)
    }
}

/// Draw `errors` at the current indentation level.
fn draw_many<Conn, C, GC, E, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    levels: &mut Vec<bool>,
    show_header: bool,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
    Conn: TreeConnectors,
{
    match errors {
        ManyErrors::None => write!(f, "no errors"),
        ManyErrors::One(node) => draw_node::<Conn, C, GC, E, F, GF>(node, levels, f),
        ManyErrors::Many(nodes) => {
            let pre_first = if show_header {
                write!(f, "{} errors:", nodes.len())?;
                "\n"
            } else {
                ""
            };
            draw_children::<Conn, C, GC, E, F, GF>(nodes, levels, pre_first, f)
        }
    }
}

/// Draw a slice of 2+ nodes, reconstructing each visual prefix lazily from `levels`.
fn draw_children<Conn, C, GC, E, F, GF>(
    nodes: &[Node<C, E, GC, F, GF>],
    levels: &mut Vec<bool>,
    pre_first: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
    Conn: TreeConnectors,
{
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { Conn::LAST } else { Conn::BRANCH };
        let sep = if i == 0 { pre_first } else { "\n" };
        // Reconstruct ancestor prefix lazily — no allocation.
        let pad = levels
            .iter()
            .map(|&l| if l { Conn::GAP } else { Conn::VERT })
            .format("");
        write!(f, "{sep}{pad}{connector}")?;
        levels.push(is_last);
        draw_node::<Conn, C, GC, E, F, GF>(node, levels, f)?;
        levels.pop();
    }
    Ok(())
}

/// Draw a single node (content after the connector has already been written).
fn draw_node<Conn, C, GC, E, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    levels: &mut Vec<bool>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
    Conn: TreeConnectors,
{
    match node {
        Node::Leaf(w) => {
            indented::<Conn>(f, levels, 0, w)?;
            draw_error_chain::<Conn>(w.error.source(), levels, f)
        }
        Node::Group(w) => {
            let label = Label::<_, GF>(&w.context, PhantomData);
            match w.errors.as_ref() {
                ManyErrors::None => {
                    indented::<Conn>(f, levels, 0, format_args!("{label}: no errors"))
                }
                ManyErrors::One(inner) => {
                    indented::<Conn>(f, levels, 0, format_args!("{label}: "))?;
                    draw_node::<Conn, C, GC, E, F, GF>(inner, levels, f)
                }
                ManyErrors::Many(nodes) => {
                    indented::<Conn>(
                        f,
                        levels,
                        0,
                        format_args!("{label} ({} errors):", nodes.len()),
                    )?;
                    draw_children::<Conn, C, GC, E, F, GF>(nodes, levels, "\n", f)
                }
            }
        }
    }
}

/// Walk a single error's source chain, drawing each source below `levels` prefix.
fn draw_error_chain<Conn: TreeConnectors>(
    source: Option<&dyn Error>,
    levels: &[bool],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let mut opt_src = source;
    let mut depth = 0usize;
    while let Some(src) = opt_src {
        let pad = Pad::<Conn> {
            levels,
            extra: depth,
            _conn: PhantomData,
        };
        write!(f, "\n{pad}{}", Conn::LAST)?;
        // Source content aligns one connector-width past `pad`; re-indent any
        // embedded newlines to that column.
        indented::<Conn>(f, levels, depth + 1, src)?;
        depth += 1;
        opt_src = src.source();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Formatted, ManyErrors,
        connectors::{Ascii, Unicode},
        many_errors::strategy::test_helpers::{two_leaves, with_chain},
        tests::Inner,
    };

    #[test]
    fn test_tree_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.tree().to_string(), "no errors");
    }

    #[test]
    fn test_tree_empty_group() {
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", ManyErrors::new());
        assert_eq!(outer.tree().to_string(), "g: no errors");
    }

    #[test]
    fn test_tree_single_leaf() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        assert_eq!(e.tree().to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_tree_two_leaves_unicode() {
        let e = two_leaves();
        assert_eq!(
            e.tree().to_string(),
            "2 errors:\n├─ a: InnerA\n└─ b: InnerB"
        );
    }

    #[test]
    fn test_tree_ascii() {
        let e = two_leaves();
        assert_eq!(
            Formatted::<_, Tree<Ascii>>::new(&e).to_string(),
            "2 errors:\n|- a: InnerA\n`- b: InnerB"
        );
    }

    #[test]
    fn test_tree_no_header() {
        let e = two_leaves();
        assert_eq!(
            Formatted::<_, Tree<Unicode, false>>::new(&e).to_string(),
            "├─ a: InnerA\n└─ b: InnerB"
        );
    }

    #[test]
    fn test_tree_with_source_chain() {
        let e = with_chain();
        let s = e.tree().to_string();
        assert!(s.contains("├─ a: mid"), "got: {s}");
        assert!(s.contains("│  └─ InnerA"), "got: {s}");
        assert!(s.contains("└─ b: mid"), "got: {s}");
        assert!(s.contains("   └─ InnerB"), "got: {s}");
    }

    #[test]
    fn test_tree_nested_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);

        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("region", inner);
        outer.push("leaf", Inner::A);

        let s = outer.tree().to_string();
        assert!(s.contains("2 errors:"), "got: {s}");
        assert!(s.contains("region (2 errors):"), "got: {s}");
        assert!(s.contains("x: InnerA"), "got: {s}");
        assert!(s.contains("y: InnerB"), "got: {s}");
        assert!(s.contains("leaf: InnerA"), "got: {s}");
    }

    /// Heterogeneous split: group labels are `usize`, leaf contexts are `&str`.
    #[test]
    fn test_tree_heterogeneous_group_label() {
        let mut inner = ManyErrors::<&str, Inner, usize>::new();
        inner.push("x", Inner::A);

        let mut outer = ManyErrors::<&str, Inner, usize>::new();
        outer.push_group(7, inner);
        outer.push("leaf", Inner::B);

        let s = outer.tree().to_string();
        assert!(s.contains("7: x: InnerA"), "got: {s}");
        assert!(s.contains("leaf: InnerB"), "got: {s}");
    }

    /// A custom `GF` is actually applied to group labels. `GF` is a label-only
    /// [`Format<GC>`] — it receives the bare label and cannot reach the nested errors.
    #[test]
    fn test_tree_custom_group_format() {
        // Brackets the group label.
        struct Bracket;
        impl<GC: Display> Format<GC> for Bracket {
            fn fmt(label: &GC, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "[{label}]")
            }
        }

        let mut inner = ManyErrors::<&str, Inner, &str, crate::with_context::Colon, Bracket>::new();
        inner.push("x", Inner::A);

        let mut outer = ManyErrors::<&str, Inner, &str, crate::with_context::Colon, Bracket>::new();
        outer.push_group("region", inner);

        assert_eq!(outer.tree().to_string(), "[region]: x: InnerA");
    }
}
