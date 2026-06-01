//! A single child of a [`ManyErrors`]: a leaf error-with-context, or a named sub-group.

use core::{
    fmt::{self, Display, Formatter},
    marker::PhantomData,
};

use alloc::boxed::Box;

use derive_where::derive_where;

use crate::with_context::{Colon, WithContext};
use crate::{AsDisplay, Format};

use super::ManyErrors;

/// The payload of a [`Node::Group`]: a label `GroupContext` paired with the boxed nested
/// [`ManyErrors`].
///
/// [`Display`] renders the **label only**, through the label strategy `GroupFormat`
/// (default [`AsDisplay`]: the label's own `Display`). The nested `errors` are
/// *not* rendered here — the active aggregate strategy ([`Tree`](crate::Tree) /
/// [`List`](crate::List) / …) owns their structural layout. That is why `GroupFormat` is
/// bound [`Format<GroupContext>`](Format) and never sees `errors`: a label formatter that
/// also rendered the children would double-render (and shatter) tree/list output.
#[derive_where(Clone, PartialEq, Eq, Hash, Debug; C, E, GroupContext)]
pub struct Subgroup<C, E, GroupContext = C, F = Colon, GroupFormat = AsDisplay> {
    /// The group label.
    pub context: GroupContext,
    /// The boxed nested errors (boxed to break the recursion with [`ManyErrors`]).
    pub errors: Box<ManyErrors<C, E, GroupContext, F, GroupFormat>>,

    /// Grounds the `GroupFormat` label strategy: it otherwise appears only inside the
    /// recursive `errors`, leaving its variance undeterminable. Mirrors
    /// [`WithContext`]'s `PhantomData<fn() -> F>`.
    #[derive_where(skip(Debug))]
    _label: PhantomData<fn() -> GroupFormat>,
}

impl<C, E, GroupContext, F, GroupFormat> Subgroup<C, E, GroupContext, F, GroupFormat> {
    /// Creates a sub-group pairing `context` (the label) with nested `errors`.
    pub fn new(
        context: GroupContext,
        errors: ManyErrors<C, E, GroupContext, F, GroupFormat>,
    ) -> Self {
        Self {
            context,
            errors: Box::new(errors),
            _label: PhantomData,
        }
    }
}

/// Renders the **label only**, via the label strategy `GroupFormat`. The nested errors
/// are laid out by the active aggregate strategy, not here.
impl<C, E, GroupContext, F, GroupFormat> Display for Subgroup<C, E, GroupContext, F, GroupFormat>
where
    GroupFormat: Format<GroupContext>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        GroupFormat::fmt(&self.context, f)
    }
}

/// A child of a [`ManyErrors`]: either a leaf error paired with context, or a
/// named sub-group of further errors.
///
/// Each variant renders through its own [`Format`](crate::Format) strategy:
/// - [`Leaf`](Node::Leaf): a leaf context `C` paired with error `E`, formatted
///   by `F` (default [`Colon`]: `"{context}: {error}"`).
/// - [`Group`](Node::Group): a [`Subgroup`] — a label `GroupContext` paired with the boxed
///   nested [`ManyErrors`]. The label is formatted by `GroupFormat` (default
///   [`AsDisplay`](crate::AsDisplay): the label's own `Display`); the nested
///   errors' layout is owned by the aggregate strategy, so `GroupFormat` is a label-only
///   [`Format<GroupContext>`](Format) and never touches them.
///
/// The standard-trait impls bound only `C`/`E`/`GroupContext` — never the `F`/`GroupFormat` marker
/// params (mirroring [`WithContext`]'s `PhantomData<fn() -> F>`).
#[derive_where(Clone, PartialEq, Eq, Hash, Debug; C, E, GroupContext)]
pub enum Node<C, E, GroupContext = C, F = Colon, GroupFormat = AsDisplay> {
    /// A leaf: one context-tagged error.
    Leaf(WithContext<C, E, F>),
    /// A named sub-group: a label paired with a boxed nested [`ManyErrors`].
    Group(Subgroup<C, E, GroupContext, F, GroupFormat>),
}

// --- Conversions ---

impl<C, E, GroupContext, F, GroupFormat> From<WithContext<C, E, F>>
    for Node<C, E, GroupContext, F, GroupFormat>
{
    fn from(w: WithContext<C, E, F>) -> Self {
        Node::Leaf(w)
    }
}

impl<C, E, GroupContext, F, GroupFormat> From<(C, E)> for Node<C, E, GroupContext, F, GroupFormat> {
    fn from((context, error): (C, E)) -> Self {
        Node::Leaf(WithContext::new(context, error))
    }
}

// --- Methods ---

impl<C, E, GroupContext, F, GroupFormat> Node<C, E, GroupContext, F, GroupFormat> {
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
    pub fn as_group(&self) -> Option<&Subgroup<C, E, GroupContext, F, GroupFormat>> {
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

    type N = Node<&'static str, Inner, &'static str, Colon, AsDisplay>;

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
        let node: N = Node::Group(Subgroup::new("region", ManyErrors::new()));
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
        let node: N = Node::Group(Subgroup::new("grp", ManyErrors::new()));
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
        let node: N = Node::Group(Subgroup::new("grp", ManyErrors::new()));
        let s = format!("{node:?}");
        assert!(s.contains("Group"));
        assert!(s.contains("grp"));
    }
}
