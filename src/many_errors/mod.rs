//! Aggregated, context-tagged errors from iterator/fold-style operations.

use core::{
    error::Error,
    fmt::{self, Display, Formatter},
    hash::{Hash, Hasher},
};

use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
    Format,
    with_context::{Colon, ContextField, WithContext},
};

mod iter;
mod node;
mod strategy;

pub use crate::connectors::{Ascii, Connectors, TreeConnectors, Unicode};
pub use node::{Node, Subgroup};
pub use strategy::{Bullets, Inline, List, Tree};

/// Zero or more context-tagged errors, arranged as a rose tree.
///
/// Each child is a [`Node`]: either a leaf [`WithContext`] pair or a labeled
/// sub-group (another `ManyErrors`). The three-variant split avoids heap
/// allocation until a second error arrives.
///
/// [`Display`] renders via [`Tree`] (branching Unicode tree with a count
/// header). Other shapes — [`List`], [`Bullets`], [`Inline`] — are available
/// via the inherent helpers [`tree`](ManyErrors::tree),
/// [`list`](ManyErrors::list), [`bullets`](ManyErrors::bullets),
/// [`one_line`](ManyErrors::one_line), or via
/// [`FormatError::formatted`](crate::FormatError::formatted) for full generic
/// control (e.g. `Tree<Ascii, false>`).
///
/// All standard-trait impls are written manually so they do **not** add
/// `F: Trait` bounds (mirroring [`WithContext`]'s `PhantomData<fn() -> F>`).
///
/// # Context bounds
/// To put a `ManyErrors` in an [`Error`] position (e.g. as a `#[source]`, or to
/// render it via [`Display`]/[`Formatted`](crate::Formatted)), the leaf context
/// `C` **and** the group context `GC` must implement [`Debug`](core::fmt::Debug)
/// — not for display, but because [`Error`] requires `Debug` as a supertrait and
/// that bound propagates through the manual `Debug` impl. A custom group-context
/// type therefore needs a `Debug` derive even though only its [`Display`] is
/// printed.
///
/// # Example
/// ```
/// use errortools::ManyErrors;
/// use std::io;
///
/// let mut errs = ManyErrors::<&str, io::Error>::new();
/// assert!(errs.is_empty());
/// errs.push("step 1", io::Error::other("fail"));
/// assert_eq!(errs.len(), 1);
/// ```
#[derive(Default)]
pub enum ManyErrors<C, E, GC = C, F = Colon, GF = ContextField> {
    /// No errors recorded.
    #[default]
    None,
    /// Exactly one child.
    One(Node<C, E, GC, F, GF>),
    /// Two or more children.
    Many(Vec<Node<C, E, GC, F, GF>>),
}

// Manual trait impls so F/GF get no extra Trait bounds from derives.

impl<C: core::fmt::Debug, E: core::fmt::Debug, GC: core::fmt::Debug, F, GF> core::fmt::Debug
    for ManyErrors<C, E, GC, F, GF>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::One(n) => f.debug_tuple("One").field(n).finish(),
            Self::Many(v) => f.debug_list().entries(v.iter()).finish(),
        }
    }
}

impl<C: Clone, E: Clone, GC: Clone, F, GF> Clone for ManyErrors<C, E, GC, F, GF> {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::One(n) => Self::One(n.clone()),
            Self::Many(v) => Self::Many(v.clone()),
        }
    }
}

impl<C: PartialEq, E: PartialEq, GC: PartialEq, F, GF> PartialEq for ManyErrors<C, E, GC, F, GF> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::One(a), Self::One(b)) => a == b,
            (Self::Many(a), Self::Many(b)) => a == b,
            _ => false,
        }
    }
}

impl<C: Eq, E: Eq, GC: Eq, F, GF> Eq for ManyErrors<C, E, GC, F, GF> {}

impl<C: Hash, E: Hash, GC: Hash, F, GF> Hash for ManyErrors<C, E, GC, F, GF> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::None => {}
            Self::One(n) => n.hash(state),
            Self::Many(v) => v.hash(state),
        }
    }
}

// --- Core API ---

impl<C, E, GC, F, GF> ManyErrors<C, E, GC, F, GF> {
    /// Creates an empty `ManyErrors`.
    pub const fn new() -> Self {
        Self::None
    }

    /// Returns `true` if no errors have been recorded.
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns the number of direct children (leaves + sub-groups).
    pub const fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::One(_) => 1,
            Self::Many(v) => v.len(),
        }
    }

    /// Appends a leaf error with context, promoting `None → One → Many`.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push("step 1", std::io::Error::other("fail"));
    /// assert_eq!(errs.len(), 1);
    /// ```
    pub fn push(&mut self, context: C, error: E) {
        self.push_node(Node::Leaf(WithContext::new(context, error)));
    }

    /// Appends a named sub-group of errors.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    /// use std::io;
    ///
    /// let mut inner = ManyErrors::<&str, io::Error>::new();
    /// inner.push("a", io::Error::other("x"));
    ///
    /// let mut outer = ManyErrors::new();
    /// outer.push_group("region", inner);
    /// assert_eq!(outer.len(), 1);
    /// ```
    pub fn push_group(&mut self, context: GC, errors: Self) {
        self.push_node(Node::Group(WithContext::new(context, Box::new(errors))));
    }

    pub(crate) fn push_node(&mut self, node: Node<C, E, GC, F, GF>) {
        let prev = core::mem::take(self);
        *self = match prev {
            Self::None => Self::One(node),
            Self::One(first) => Self::Many(vec![first, node]),
            Self::Many(mut v) => {
                v.push(node);
                Self::Many(v)
            }
        };
    }

    /// Returns `Ok(ok)` if no errors were recorded, otherwise `Err(self)`.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let errs = ManyErrors::<&str, std::io::Error>::new();
    /// assert!(errs.into_result(42).is_ok());
    /// ```
    pub fn into_result<T>(self, ok: T) -> Result<T, Self> {
        match self {
            Self::None => Ok(ok),
            _ => Err(self),
        }
    }
}

// --- Inherent formatting helpers (no turbofish needed for common shapes) ---

impl<C, E, GC, F, GF> ManyErrors<C, E, GC, F, GF> {
    /// Renders as a branching Unicode tree with a count header (same as default [`Display`]).
    pub fn tree(&self) -> crate::Formatted<&Self, Tree> {
        crate::Formatted::new(self)
    }

    /// Renders as a dotted numbered list (`1.  1.1.  1.2.  2.`).
    pub fn list(&self) -> crate::Formatted<&Self, List> {
        crate::Formatted::new(self)
    }

    /// Renders as a bulleted list with `•` markers.
    pub fn bullets(&self) -> crate::Formatted<&Self, Bullets> {
        crate::Formatted::new(self)
    }

    /// Renders on a single line: `;`-separated siblings, parens around groups.
    pub fn one_line(&self) -> crate::Formatted<&Self, Inline> {
        crate::Formatted::new(self)
    }
}

/// Renders each error as a branching Unicode tree with a count header.
impl<C, E, GC, F, GF> Display for ManyErrors<C, E, GC, F, GF>
where
    C: Display,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        <Tree as Format<Self>>::fmt(self, f)
    }
}

impl<C, E, GC, F, GF> Error for ManyErrors<C, E, GC, F, GF>
where
    C: Display + core::fmt::Debug,
    GC: core::fmt::Debug,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    /// For [`Self::One`] holding a leaf, returns the inner error's source so
    /// chain-walking strategies don't duplicate it. Groups and `Many` variants
    /// have no single source.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::None | Self::Many(_) => None,
            Self::One(Node::Leaf(w)) => w.error.source(),
            Self::One(Node::Group(_)) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Inner, Mid};

    // --- push / push_group / variants ---

    #[test]
    fn test_new_is_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert!(matches!(e, ManyErrors::None));
        assert!(e.is_empty());
        assert_eq!(e.len(), 0);
    }

    #[test]
    fn test_push_none_to_one() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        assert!(matches!(e, ManyErrors::One(_)));
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn test_push_one_to_many() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::A);
        assert!(matches!(e, ManyErrors::Many(_)));
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn test_push_many_grows() {
        let mut e = ManyErrors::<u32, Inner, &str>::new();
        for i in 0..5u32 {
            e.push(i, Inner::A);
        }
        assert_eq!(e.len(), 5);
    }

    #[test]
    fn test_push_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);

        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("region", inner);
        assert_eq!(outer.len(), 1);
        assert!(matches!(outer, ManyErrors::One(Node::Group(_))));
    }

    #[test]
    fn test_push_leaf_and_group() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("leaf", Inner::A);
        let mut sub = ManyErrors::new();
        sub.push("sub-leaf", Inner::B);
        e.push_group("group", sub);
        assert_eq!(e.len(), 2);
    }

    // --- into_result ---

    #[test]
    fn test_into_result_none_ok() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.into_result(42), Ok(42));
    }

    #[test]
    fn test_into_result_one_err() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        assert!(e.into_result(()).is_err());
    }

    #[test]
    fn test_into_result_many_err() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::A);
        assert!(e.into_result(()).is_err());
    }

    // --- Error::source ---

    #[test]
    fn test_source_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert!(e.source().is_none());
    }

    #[test]
    fn test_source_one_leaf_skips_inner_error() {
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("ctx", Mid::Inner(Inner::A));
        let src = e.source().expect("should have source");
        assert_eq!(src.to_string(), "InnerA");
    }

    #[test]
    fn test_source_one_group_is_none() {
        let mut e = ManyErrors::<&str, Inner>::new();
        let mut sub = ManyErrors::new();
        sub.push("x", Inner::A);
        e.push_group("g", sub);
        assert!(e.source().is_none());
    }

    #[test]
    fn test_source_many_is_none() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::A);
        assert!(e.source().is_none());
    }

    // --- Display (Tree) ---

    #[test]
    fn test_display_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.to_string(), "");
    }

    #[test]
    fn test_display_single_leaf_no_header() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        // Single item: no count header
        assert_eq!(e.to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_display_two_leaves_with_header() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        assert_eq!(e.to_string(), "2 errors:\n├─ a: InnerA\n└─ b: InnerB");
    }

    #[test]
    fn test_display_nested_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);

        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("region", inner);
        outer.push("leaf", Inner::A);

        let s = outer.to_string();
        assert!(s.contains("2 errors:"), "got: {s}");
        assert!(s.contains("region (2 errors):"), "got: {s}");
        assert!(s.contains("leaf: InnerA"), "got: {s}");
    }

    #[test]
    fn test_one_line_single_leaf_walks_chain() {
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("ctx", Mid::Inner(Inner::A));
        assert_eq!(e.one_line().to_string(), "ctx: mid: InnerA");
    }
}
