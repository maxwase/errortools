//! Single-line strategies: [`Joined`] (deep, walks source chains) and
//! [`Summary`] (shallow, own text only — the default [`Display`]).
//!
//! Both share one traversal ([`draw_one_line_many`]) parameterized by a leaf
//! renderer, so they differ in exactly one place: how a leaf is printed.
//! `Joined` walks each leaf's source chain via [`LeafChain`]; `Summary` prints
//! the leaf's own text `{w}` only. Neither bounds the context `C` at all —
//! leaves render through `F`, labels through `GF`.

use core::{error::Error, fmt};

use crate::{
    Format,
    many_errors::{ManyErrors, Node},
    with_context::WithContext,
};

use super::{ErrorCount, LeafChain, NO_ERRORS, impl_aggregate_format, impl_ref_format};

/// Aggregate strategy that renders a [`ManyErrors`] on a single line, walking
/// each leaf's source chain (joined with `": "`, like the per-error
/// [`OneLine`](crate::OneLine) strategy).
///
/// Siblings are separated by `"; "`, nested groups wrapped in parens.
///
/// Single-line is a layout intent, not a sanitization guarantee: control
/// characters (`\n`, `\t`, …) embedded in messages or strategies pass through
/// verbatim. For layouts that re-indent embedded newlines use
/// [`Tree`](super::Tree) / [`List`](super::List) / [`Bullets`](super::Bullets).
///
/// # Output example
/// ```text
/// 3 errors: a: InnerA; b: InnerB; c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Joined;

impl_aggregate_format!(Joined, |errors, f| draw_joined::<C, E, GC, F, GF>(
    errors, f
));

/// Shallow single-line strategy backing the default [`Display`] of
/// [`ManyErrors`]: each error's own text only, **no source chains**.
///
/// Siblings are separated by `"; "`, nested groups wrapped in parens.
/// Control characters embedded in messages pass through verbatim (same
/// passthrough policy as [`Joined`]).
///
/// # Output example
/// ```text
/// 2 errors: leaf: InnerA; region (2 errors: x: InnerA; y: InnerB)
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Summary;

impl_aggregate_format!(Summary, |errors, f| draw_summary::<C, E, GC, F, GF>(
    errors, f
));

/// Shared single-line traversal, generic over the per-node renderer `node`.
///
/// - `None` writes `"no errors"`.
/// - `One` renders the sole child with no header.
/// - `Many` writes the `"N errors: "` header, then each child separated by
///   `"; "` (a `first` flag suppresses the leading separator).
fn draw_one_line_many<C, E, GC, F, GF, N>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    node: N,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    N: Fn(&Node<C, E, GC, F, GF>, &mut fmt::Formatter<'_>) -> fmt::Result + Copy,
{
    match errors {
        ManyErrors::None => f.write_str(NO_ERRORS),
        ManyErrors::One(child) => node(child, f),
        ManyErrors::Many(nodes) => {
            write!(f, "{}: ", ErrorCount(nodes.len()))?;
            let mut first = true;
            for child in nodes {
                if !first {
                    write!(f, "; ")?;
                }
                first = false;
                node(child, f)?;
            }
            Ok(())
        }
    }
}

/// [`Summary`] entry point: render each child via its own [`Display`] — shallow,
/// no source chains. A group child goes through
/// [`Subgroup`](crate::Subgroup)'s own `Display` (`"{label} (…summary…)"`),
/// the single definition of a group's summary form.
fn draw_summary<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    draw_one_line_many(errors, summary_node::<C, E, GC, F, GF>, f)
}

/// Shallow per-node renderer: each child via its own `Display` (the leaf's
/// `WithContext`, or the group's [`Subgroup`](crate::Subgroup)).
fn summary_node<C, E, GC, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    match node {
        Node::Leaf(w) => write!(f, "{w}"),
        Node::Group(w) => write!(f, "{w}"),
    }
}

/// [`Joined`] entry point: leaves walk their source chain via [`OneLine`]; groups
/// recurse deep through the same renderer.
fn draw_joined<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    draw_one_line_many(errors, joined_node::<C, E, GC, F, GF>, f)
}

/// Deep per-node renderer: a leaf walks its source chain via [`LeafChain`]; a group
/// is `"{label} ("`, the nested aggregate via [`draw_one_line_many`] (recursing
/// through this same renderer), then `")"`. An empty group falls out as
/// `"{label} (no errors)"` with no special case.
fn joined_node<C, E, GC, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    match node {
        Node::Leaf(w) => write!(f, "{}", LeafChain(w)),
        Node::Group(w) => {
            GF::fmt(&w.context, f)?;
            write!(f, " (")?;
            draw_one_line_many(w.errors.as_ref(), joined_node::<C, E, GC, F, GF>, f)?;
            write!(f, ")")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ManyErrors;
    use crate::many_errors::strategy::test_helpers::{two_leaves, with_chain};
    use crate::tests::{Inner, Mid};

    /// `{leaf, group{x, y}}` — the standard nested fixture, leaf first.
    fn nested() -> ManyErrors<&'static str, Inner> {
        let mut inner = ManyErrors::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);
        let mut outer = ManyErrors::new();
        outer.push("leaf", Inner::A);
        outer.push_group("region", inner);
        outer
    }

    // --- Joined (deep) ---

    #[test]
    fn test_joined_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.joined().to_string(), "no errors");
    }

    #[test]
    fn test_joined_single_leaf_no_header() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        assert_eq!(e.joined().to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_joined_two_leaves() {
        assert_eq!(
            two_leaves().joined().to_string(),
            "2 errors: a: InnerA; b: InnerB"
        );
    }

    /// Deep: leaf source chains are walked and joined with `": "`.
    #[test]
    fn test_joined_walks_source_chain() {
        assert_eq!(
            with_chain().joined().to_string(),
            "2 errors: a: mid: InnerA; b: mid: InnerB"
        );
    }

    #[test]
    fn test_joined_nested_group() {
        assert_eq!(
            nested().joined().to_string(),
            "2 errors: leaf: InnerA; region (2 errors: x: InnerA; y: InnerB)"
        );
    }

    #[test]
    fn test_joined_single_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", inner);
        assert_eq!(outer.joined().to_string(), "g (x: InnerA)");
    }

    #[test]
    fn test_joined_empty_group() {
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", ManyErrors::new());
        assert_eq!(outer.joined().to_string(), "g (no errors)");
    }

    // --- Summary (shallow, the default Display) ---

    #[test]
    fn test_summary_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.to_string(), "no errors");
    }

    #[test]
    fn test_summary_single_leaf_no_header() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        assert_eq!(e.to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_summary_two_leaves() {
        assert_eq!(two_leaves().to_string(), "2 errors: a: InnerA; b: InnerB");
    }

    /// Shallow: a leaf's source is NOT walked (`mid`, not `mid: InnerA`).
    #[test]
    fn test_summary_does_not_walk_source() {
        let s = with_chain().to_string();
        assert_eq!(s, "2 errors: a: mid; b: mid");
        assert!(!s.contains("InnerA"), "source must not be walked: {s}");
    }

    #[test]
    fn test_summary_nested_group() {
        assert_eq!(
            nested().to_string(),
            "2 errors: leaf: InnerA; region (2 errors: x: InnerA; y: InnerB)"
        );
    }

    #[test]
    fn test_summary_single_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", inner);
        assert_eq!(outer.to_string(), "g (x: InnerA)");
    }

    #[test]
    fn test_summary_empty_group() {
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", ManyErrors::new());
        assert_eq!(outer.to_string(), "g (no errors)");
    }

    /// Heterogeneous: group labels are `usize`, leaf contexts are `&str`.
    #[test]
    fn test_summary_heterogeneous_group_label() {
        let mut inner = ManyErrors::<&str, Inner, usize>::new();
        inner.push("x", Inner::A);
        let mut outer = ManyErrors::<&str, Inner, usize>::new();
        outer.push("leaf", Inner::B);
        outer.push_group(7, inner);
        assert_eq!(outer.to_string(), "2 errors: leaf: InnerB; 7 (x: InnerA)");
    }

    /// A leaf whose error carries a source is still shallow under Summary.
    #[test]
    fn test_summary_single_leaf_with_source() {
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("ctx", Mid::Inner(Inner::A));
        assert_eq!(e.to_string(), "ctx: mid");
    }
}
