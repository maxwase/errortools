//! Single-line strategies: [`Joined`] (deep, walks source chains) and
//! [`Summary`] (shallow, own text only — the default [`Display`]).
//!
//! Both share one traversal ([`draw_one_line_many`]) parameterized by a leaf
//! renderer, so they differ in exactly one place: how a leaf is printed.
//! `Joined` routes leaves through the chain-walking [`OneLine`] (needs
//! `C: Debug`, via `WithContext: Error`); `Summary` prints the leaf's own text
//! `{w}` only (no `Debug`, keeping `ManyErrors: Display` at `C: Display`).

use core::{
    error::Error,
    fmt::{self, Display},
};

use crate::{
    Format, OneLine,
    many_errors::{ManyErrors, Node, Subgroup},
    with_context::WithContext,
};

use super::impl_aggregate_format;

/// Aggregate strategy that renders a [`ManyErrors`] on a single line, walking
/// each leaf's source chain via the per-error [`OneLine`] strategy.
///
/// Siblings are separated by `"; "`, nested groups wrapped in parens.
///
/// # Output example
/// ```text
/// 3 errors: a: InnerA; b: InnerB; c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Joined;

impl_aggregate_format!(Joined, [+ ::core::fmt::Debug], |errors, f| draw_joined::<
    C,
    E,
    GC,
    F,
    GF,
>(errors, f));

/// Shallow single-line strategy backing the default [`Display`] of
/// [`ManyErrors`]: each error's own text only, **no source chains**.
///
/// Siblings are separated by `"; "`, nested groups wrapped in parens.
///
/// # Output example
/// ```text
/// 2 errors: leaf: InnerA; region (2 errors: x: InnerA; y: InnerB)
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Summary;

impl_aggregate_format!(Summary, [], |errors, f| draw_summary::<C, E, GC, F, GF>(
    errors, f
));

/// [`Joined`] entry point: leaves walk their source chain via [`OneLine`].
fn draw_joined<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display + fmt::Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    draw_one_line_many(errors, <OneLine as Format<WithContext<C, E, F>>>::fmt, f)
}

/// [`Summary`] entry point: leaves print their own text only.
fn draw_summary<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    draw_one_line_many(errors, |w, f| write!(f, "{w}"), f)
}

/// Shared single-line traversal, generic over the per-leaf renderer `leaf`.
///
/// - `None` writes nothing.
/// - `None` writes `"no errors"`.
/// - `One` delegates to [`draw_one_line_node`] with no header.
/// - `Many` writes the `"N errors: "` header, then each child separated by
///   `"; "` (a `first` flag suppresses the leading separator).
fn draw_one_line_many<C, E, GC, F, GF, L>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    leaf: L,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
    L: Fn(&WithContext<C, E, F>, &mut fmt::Formatter<'_>) -> fmt::Result + Copy,
{
    match errors {
        ManyErrors::None => write!(f, "no errors"),
        ManyErrors::One(node) => draw_one_line_node(node, leaf, f),
        ManyErrors::Many(nodes) => {
            write!(f, "{} errors: ", nodes.len())?;
            let mut first = true;
            for node in nodes {
                if !first {
                    write!(f, "; ")?;
                }
                first = false;
                draw_one_line_node(node, leaf, f)?;
            }
            Ok(())
        }
    }
}

/// Render a single node on the current line.
///
/// - `Leaf` → `leaf(w, f)`.
/// - `Group` → `"{w} ("`, the nested aggregate via [`draw_one_line_many`], then
///   `")"`. Parens keep depth unambiguous, and an empty group falls out as
///   `"{w} (no errors)"` with no special case.
fn draw_one_line_node<C, E, GC, F, GF, L>(
    node: &Node<C, E, GC, F, GF>,
    leaf: L,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
    L: Fn(&WithContext<C, E, F>, &mut fmt::Formatter<'_>) -> fmt::Result + Copy,
{
    match node {
        Node::Leaf(w) => leaf(w, f),
        Node::Group(w) => {
            write!(f, "{w} (")?;
            draw_one_line_many(w.error.as_ref(), leaf, f)?;
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
