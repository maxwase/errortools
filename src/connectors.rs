//! Box-drawing glyph sets shared by the [`Chain`](crate::Chain) source-chain
//! ladder and the `Tree` aggregate renderer (requires the `alloc` feature).
//!
//! A linear chain is a degenerate tree: every node is an only-child, so it
//! always renders as a "last" child with a blank continuation under it. That's
//! exactly the [`Connectors`] pair. A branching tree additionally needs the
//! sibling glyphs, which live on the [`TreeConnectors`] supertrait. One glyph
//! type ([`Unicode`], [`Ascii`]) implements both, so `Chain<Ascii>` and
//! `Tree<Ascii>` share a single vocabulary.

/// The two glyphs a linear source-chain ladder needs: the branch prefix before
/// each source, and the blank continuation under it.
///
/// [`Chain`](crate::Chain) renders every node as an only-child, so it only ever
/// uses [`LAST`](Connectors::LAST) and [`GAP`](Connectors::GAP). Branching
/// trees pick up the sibling glyphs via the [`TreeConnectors`] supertrait.
///
/// `Chain<C>` is a strategy only when `C` is a glyph set:
/// ```compile_fail,E0277
/// use errortools::{Chain, FormatError, OneLine};
/// use std::io;
///
/// // `OneLine` is a strategy, not a glyph set.
/// let error = io::Error::other("boom");
/// println!("{}", error.formatted::<Chain<OneLine>>());
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a set of chain connectors",
    label = "`{Self}` does not implement `Connectors`",
    note = "use `Unicode` or `Ascii`, or implement `Connectors` for `{Self}`"
)]
pub trait Connectors {
    /// Prefix for a last (or only) child: `"└─ "` (Unicode).
    const LAST: &'static str;
    /// Blank continuation under a last child: `"   "`.
    const GAP: &'static str;
}

/// The full box-drawing set a branching `Tree` aggregate renderer
/// needs: the [`Connectors`] pair plus the sibling glyphs for non-last children.
///
/// A glyph set that implements only [`Connectors`] can draw a `Chain` but not
/// a `Tree`:
#[cfg_attr(feature = "alloc", doc = "```compile_fail,E0277")]
#[cfg_attr(not(feature = "alloc"), doc = "```ignore")]
/// use errortools::{Connectors, ManyErrors, Tree};
///
/// struct Dots;
/// impl Connectors for Dots {
///     const LAST: &'static str = ".. ";
///     const GAP: &'static str = "   ";
/// }
///
/// let errors = ManyErrors::<&str, std::io::Error>::new();
/// println!("{}", errors.formatted::<Tree<Dots>>());
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a set of tree connectors",
    label = "`{Self}` does not implement `TreeConnectors`",
    note = "use `Unicode` or `Ascii`, or implement `TreeConnectors` (and `Connectors`) for `{Self}`"
)]
pub trait TreeConnectors: Connectors {
    /// Prefix for a non-last child: `"├─ "` (Unicode).
    const BRANCH: &'static str;
    /// Continuation bar under a non-last child: `"│  "` (Unicode).
    const VERT: &'static str;
}

/// Unicode box-drawing connectors (default).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Unicode;

impl Connectors for Unicode {
    const LAST: &'static str = "└─ ";
    const GAP: &'static str = "   ";
}

impl TreeConnectors for Unicode {
    const BRANCH: &'static str = "├─ ";
    const VERT: &'static str = "│  ";
}

/// ASCII-only connectors for environments that can't render Unicode box art.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ascii;

impl Connectors for Ascii {
    const LAST: &'static str = "`- ";
    const GAP: &'static str = "   ";
}

impl TreeConnectors for Ascii {
    const BRANCH: &'static str = "|- ";
    const VERT: &'static str = "|  ";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_glyphs() {
        assert_eq!(Unicode::BRANCH, "├─ ");
        assert_eq!(Unicode::LAST, "└─ ");
        assert_eq!(Unicode::VERT, "│  ");
        assert_eq!(Unicode::GAP, "   ");
    }

    #[test]
    fn test_ascii_glyphs() {
        assert_eq!(Ascii::BRANCH, "|- ");
        assert_eq!(Ascii::LAST, "`- ");
        assert_eq!(Ascii::VERT, "|  ");
        assert_eq!(Ascii::GAP, "   ");
    }
}
