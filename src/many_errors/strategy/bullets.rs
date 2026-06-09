//! [`Bullets`]: render a [`ManyErrors`](crate::ManyErrors) as a bulleted (`•`) list.
//!
//! The traversal lives in [`super::marked`]; `Bullets` only contributes the
//! `"• "` row marker and its indent offsets.

use core::fmt;

use crate::indent::Repeat;

use super::marked::{Marker, draw_marked_many};
use super::{impl_aggregate_format, impl_ref_format};

/// Aggregate strategy that renders a [`ManyErrors`](crate::ManyErrors) as a
/// bulleted (`•`) list.
///
/// # Output example
/// ```text
/// 3 errors:
///   • a: InnerA
///   • b: InnerB
///   • c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bullets;

/// Bulleted rows: top-level rows indent one unit; a group's rows sit at the
/// group's content column (right under its label).
impl Marker for Bullets {
    const TOP_ROW: usize = 1;
    const GROUP_ROW_OFFSET: usize = 0;

    fn write_marker(f: &mut fmt::Formatter<'_>, indent: usize, _index: usize) -> fmt::Result {
        write!(f, "\n{}• ", Repeat("  ", indent))
    }
}

impl_aggregate_format!(
    Bullets,
    |errors, f| draw_marked_many::<Self, C, E, GC, F, GF>(errors, f)
);

#[cfg(test)]
mod tests {
    use crate::ManyErrors;
    use crate::many_errors::strategy::test_helpers::{two_leaves, with_chain};
    use crate::tests::Inner;

    #[test]
    fn test_bullets_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.bullets().to_string(), "no errors");
    }

    #[test]
    fn test_bullets_single_leaf_no_bullet() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        assert_eq!(e.bullets().to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_bullets_two_leaves() {
        assert_eq!(
            two_leaves().bullets().to_string(),
            "2 errors:\n  • a: InnerA\n  • b: InnerB"
        );
    }

    /// Leaves walk their source chain via `OneLine`.
    #[test]
    fn test_bullets_walks_source_chain() {
        let s = with_chain().bullets().to_string();
        assert!(s.contains("• a: mid: InnerA"), "got: {s}");
        assert!(s.contains("• b: mid: InnerB"), "got: {s}");
    }

    #[test]
    fn test_bullets_nested_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::A);
        outer.push_group("region", inner);

        assert_eq!(
            outer.bullets().to_string(),
            "2 errors:\n  • leaf: InnerA\n  • region (2 errors):\n    • x: InnerA\n    • y: InnerB"
        );
    }

    #[test]
    fn test_bullets_empty_group() {
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("g", ManyErrors::new());
        assert_eq!(outer.bullets().to_string(), "g: no errors");
    }
}
