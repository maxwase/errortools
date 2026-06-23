//! [`List`]: render a [`ManyErrors`](crate::ManyErrors) as a numbered list.
//!
//! The traversal lives in [`super::marked`]; `List` only contributes the
//! `"{i}. "` row marker and its indent offsets.

use core::fmt;

use crate::indent::Repeat;

use super::impl_aggregate_format;
use super::marked::{Marker, draw_marked_many};

/// Aggregate strategy that renders a [`ManyErrors`](crate::ManyErrors) as a
/// numbered list.
///
/// # Output example
/// ```text
/// 3 errors:
/// 1. a: InnerA
/// 2. b: InnerB
/// 3. c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct List;

/// Numbered rows: top-level rows sit flush; a group's rows indent one unit
/// past the group's content column.
impl Marker for List {
    const TOP_ROW: usize = 0;
    const GROUP_ROW_OFFSET: usize = 1;

    fn write_marker(f: &mut fmt::Formatter<'_>, indent: usize, index: usize) -> fmt::Result {
        write!(f, "\n{}{}. ", Repeat("  ", indent), index + 1)
    }
}

impl_aggregate_format!(List, |errors, f| draw_marked_many::<Self, C, E, GC, F, GF>(
    errors, f
));

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

    /// A single-child sub-group recurses inline after `": "` (no marked row for
    /// the lone child), keeping the same content column.
    #[test]
    fn test_list_single_child_group_inline() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::A);
        outer.push_group("region", inner);

        assert_eq!(
            outer.list().to_string(),
            "2 errors:\n1. leaf: InnerA\n2. region: x: InnerA"
        );
    }
}
