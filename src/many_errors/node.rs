//! A single child of a [`ManyErrors`]: a leaf error-with-context, or a named sub-group.

use core::{
    fmt::{self, Debug},
    hash::{Hash, Hasher},
};

use alloc::boxed::Box;

use crate::with_context::{Colon, ContextField, WithContext};

use super::ManyErrors;

/// The payload of a [`Node::Group`]: a label `GC` paired with the boxed nested
/// [`ManyErrors`], rendered through the label strategy `GF`.
pub type Subgroup<C, E, GC, F, GF> = WithContext<GC, Box<ManyErrors<C, E, GC, F, GF>>, GF>;

/// A child of a [`ManyErrors`]: either a leaf error paired with context, or a
/// named sub-group of further errors.
///
/// Both variants reuse [`WithContext`], so leaves and groups are symmetric and
/// each renders through its own [`Format`](crate::Format) strategy:
/// - [`Leaf`](Node::Leaf): a leaf context `C` paired with error `E`, formatted
///   by `F` (default [`Colon`]: `"{context}: {error}"`).
/// - [`Group`](Node::Group): a label `GC` paired with a boxed nested
///   [`ManyErrors`], formatted by `GF` (default [`ContextField`]: label only).
///
/// The group's `errors` are boxed to break the mutual recursion with
/// [`ManyErrors`]. All standard-trait impls (`Clone`, `PartialEq`, `Eq`,
/// `Hash`, `Debug`) are written manually — not derived — so they do **not** add
/// `F`/`GF`: `Trait` bounds (mirroring [`WithContext`]'s `PhantomData<fn() -> F>`).
pub enum Node<C, E, GC = C, F = Colon, GF = ContextField> {
    /// A leaf: one context-tagged error.
    Leaf(WithContext<C, E, F>),
    /// A named sub-group: a label paired with a boxed nested [`ManyErrors`].
    Group(Subgroup<C, E, GC, F, GF>),
}

// --- Manual trait impls (no F/GF: Trait bound) ---

impl<C: Clone, E: Clone, GC: Clone, F, GF> Clone for Node<C, E, GC, F, GF> {
    fn clone(&self) -> Self {
        match self {
            Node::Leaf(w) => Node::Leaf(w.clone()),
            Node::Group(w) => Node::Group(w.clone()),
        }
    }
}

impl<C: PartialEq, E: PartialEq, GC: PartialEq, F, GF> PartialEq for Node<C, E, GC, F, GF> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Node::Leaf(a), Node::Leaf(b)) => a == b,
            (Node::Group(a), Node::Group(b)) => a == b,
            _ => false,
        }
    }
}

impl<C: Eq, E: Eq, GC: Eq, F, GF> Eq for Node<C, E, GC, F, GF> {}

impl<C: Hash, E: Hash, GC: Hash, F, GF> Hash for Node<C, E, GC, F, GF> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Node::Leaf(w) => w.hash(state),
            Node::Group(w) => w.hash(state),
        }
    }
}

impl<C: Debug, E: Debug, GC: Debug, F, GF> Debug for Node<C, E, GC, F, GF> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Leaf(w) => f.debug_tuple("Leaf").field(w).finish(),
            Node::Group(w) => f.debug_tuple("Group").field(w).finish(),
        }
    }
}

// --- Conversions ---

impl<C, E, GC, F, GF> From<WithContext<C, E, F>> for Node<C, E, GC, F, GF> {
    fn from(w: WithContext<C, E, F>) -> Self {
        Node::Leaf(w)
    }
}

impl<C, E, GC, F, GF> From<(C, E)> for Node<C, E, GC, F, GF> {
    fn from((context, error): (C, E)) -> Self {
        Node::Leaf(WithContext::new(context, error))
    }
}

// --- Methods ---

impl<C, E, GC, F, GF> Node<C, E, GC, F, GF> {
    /// Returns `true` if this is a [`Node::Leaf`].
    pub fn is_leaf(&self) -> bool {
        matches!(self, Node::Leaf(_))
    }

    /// Returns the leaf's [`WithContext`] pair, or `None` for a group.
    pub fn as_leaf(&self) -> Option<&WithContext<C, E, F>> {
        match self {
            Node::Leaf(w) => Some(w),
            Node::Group(_) => None,
        }
    }

    /// Returns the group's labeled [`WithContext`], or `None` for a leaf.
    ///
    /// The label is `&self.context`; the nested errors are `&*self.error`.
    pub fn as_group(&self) -> Option<&Subgroup<C, E, GC, F, GF>> {
        match self {
            Node::Group(w) => Some(w),
            Node::Leaf(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Inner;

    type N = Node<&'static str, Inner, &'static str, Colon, ContextField>;

    #[test]
    fn test_leaf_from_with_context() {
        let w = WithContext::<_, _, Colon>::new("ctx", Inner::A);
        let node: N = Node::from(w);
        assert!(node.is_leaf());
        assert_eq!(node.as_leaf().unwrap().context, "ctx");
    }

    #[test]
    fn test_leaf_from_tuple() {
        let node: N = Node::from(("ctx", Inner::A));
        assert!(node.is_leaf());
        assert_eq!(node.as_leaf().unwrap().context, "ctx");
    }

    #[test]
    fn test_group_context() {
        let node: N = Node::Group(WithContext::new("region", Box::new(ManyErrors::new())));
        assert!(!node.is_leaf());
        assert_eq!(node.as_group().unwrap().context, "region");
    }

    #[test]
    fn test_clone_leaf() {
        let node: N = Node::from(("ctx", Inner::A));
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }

    #[test]
    fn test_clone_group() {
        let node: N = Node::Group(WithContext::new("grp", Box::new(ManyErrors::new())));
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }

    #[test]
    fn test_debug_leaf() {
        let node: N = Node::from(("ctx", Inner::A));
        let s = format!("{node:?}");
        assert!(s.contains("Leaf"));
        assert!(s.contains("ctx"));
    }

    #[test]
    fn test_debug_group() {
        let node: N = Node::Group(WithContext::new("grp", Box::new(ManyErrors::new())));
        let s = format!("{node:?}");
        assert!(s.contains("Group"));
        assert!(s.contains("grp"));
    }
}
