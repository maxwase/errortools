//! A single child of a [`ManyErrors`]: a leaf error-with-context, or a named sub-group.

use core::{
    error::Error,
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
/// [`Display`] renders the group standalone as `"{label} (summary)"`: the label via
/// the label strategy `GroupFormat` (default [`AsDisplay`]: the label's own `Display`),
/// then a shallow one-line summary of the nested errors — the same shape the parent
/// [`ManyErrors`] uses for a group. Inside an aggregate strategy ([`Tree`](crate::Tree) /
/// [`List`](crate::List) / …) only the label is taken from `GroupFormat`; the strategy
/// owns the nested layout itself, so this `Display` is never used there. That split is
/// why `GroupFormat` is bound label-only [`Format<GroupContext>`](Format) and never sees
/// `errors`: a label formatter that rendered the children would double-render under a strategy.
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

    /// Switches the leaf and group-label strategies without touching the
    /// stored values, rebuilding the nested tree recursively (O(n), one new
    /// box per group).
    pub fn with_formats<NewF, NewGF>(self) -> Subgroup<C, E, GroupContext, NewF, NewGF>
    where
        NewF: Format<WithContext<C, E, NewF>>,
        NewGF: Format<GroupContext>,
    {
        Subgroup {
            context: self.context,
            errors: Box::new(self.errors.with_formats()),
            _label: PhantomData,
        }
    }
}

/// Standalone group rendering: label via `GroupFormat`, then the nested errors as a
/// shallow one-line summary in parens (`"{label} (…)"`) — matching how the parent
/// [`ManyErrors`] renders a group. Aggregate strategies don't use this; they take the
/// label from `GroupFormat` directly and lay out the children themselves.
impl<C, E, GroupContext, F, GroupFormat> Display for Subgroup<C, E, GroupContext, F, GroupFormat>
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GroupFormat: Format<GroupContext>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        GroupFormat::fmt(&self.context, f)?;
        write!(f, " (")?;
        Display::fmt(&*self.errors, f)?;
        write!(f, ")")
    }
}

impl<C, E, GroupContext, F, GroupFormat> Error for Subgroup<C, E, GroupContext, F, GroupFormat>
where
    C: fmt::Debug,
    GroupContext: fmt::Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GroupFormat: Format<GroupContext>,
{
    /// Always `None`: an aggregate of independent sibling errors has no single
    /// linear cause (matching [`ManyErrors`]). Inspect `errors` directly, or
    /// render the full chains via an aggregate strategy.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
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

impl<C, E, GroupContext, F, GroupFormat> From<Subgroup<C, E, GroupContext, F, GroupFormat>>
    for Node<C, E, GroupContext, F, GroupFormat>
{
    fn from(group: Subgroup<C, E, GroupContext, F, GroupFormat>) -> Self {
        Node::Group(group)
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

    /// Returns the group's [`Subgroup`], or `None` for a leaf.
    ///
    /// The label is `&self.context`; the nested errors are `&self.errors`.
    pub fn as_group(&self) -> Option<&Subgroup<C, E, GroupContext, F, GroupFormat>> {
        match self {
            Node::Group(w) => Some(w),
            Node::Leaf(_) => None,
        }
    }

    /// Switches the leaf and group-label strategies without touching the
    /// stored values (a group rebuilds its nested tree recursively).
    pub fn with_formats<NewF, NewGF>(self) -> Node<C, E, GroupContext, NewF, NewGF>
    where
        NewF: Format<WithContext<C, E, NewF>>,
        NewGF: Format<GroupContext>,
    {
        match self {
            Node::Leaf(w) => Node::Leaf(w.with_format()),
            Node::Group(group) => Node::Group(group.with_formats()),
        }
    }
}

// --- Display / Error (so iterated children are usable as errors directly) ---

/// Renders the child standalone: a leaf via its own [`WithContext`] `Display`
/// (the pair through `F`), a group via [`Subgroup`]'s `Display`
/// (`"{label} (…shallow summary…)"`).
impl<C, E, GroupContext, F, GroupFormat> Display for Node<C, E, GroupContext, F, GroupFormat>
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GroupFormat: Format<GroupContext>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Node::Leaf(w) => Display::fmt(w, f),
            Node::Group(group) => Display::fmt(group, f),
        }
    }
}

impl<C, E, GroupContext, F, GroupFormat> Error for Node<C, E, GroupContext, F, GroupFormat>
where
    C: fmt::Debug,
    GroupContext: fmt::Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GroupFormat: Format<GroupContext>,
{
    /// A leaf delegates to [`WithContext`]'s `source` (which skips the inner
    /// error itself — `Display` already shows it); a group returns `None` (an
    /// aggregate of independent siblings has no single linear cause, matching
    /// [`ManyErrors`]).
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Node::Leaf(w) => Error::source(w),
            Node::Group(_) => None,
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

    /// The accessors return `None` for the mismatched variant.
    #[test]
    fn test_accessor_mismatch_is_none() {
        let leaf: N = Node::from(("ctx", Inner::A));
        assert!(leaf.as_group().is_none());

        let group: N = Node::Group(Subgroup::new("region", ManyErrors::new()));
        assert!(group.as_leaf().is_none());
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

    #[test]
    fn test_node_from_subgroup() {
        let node: N = Subgroup::new("region", ManyErrors::new()).into();
        assert!(!node.is_leaf());
        assert_eq!(node.as_group().unwrap().context, "region");
    }

    /// `Node` renders standalone: a leaf via its pair strategy, a group via
    /// `Subgroup`'s `"{label} (…)"`.
    #[test]
    fn test_node_display() {
        use crate::tests::Mid;

        let leaf: Node<&str, Mid> = Node::from(("ctx", Mid::Inner(Inner::A)));
        assert_eq!(leaf.to_string(), "ctx: mid");

        let mut inner = ManyErrors::<&str, Mid>::new();
        inner.push("x", Mid::Inner(Inner::A));
        let group: Node<&str, Mid> = Subgroup::new("g", inner).into();
        assert_eq!(group.to_string(), "g (x: mid)");
    }

    /// A leaf `Node` is an error whose source skips the inner error (already
    /// displayed); a group's source is `None`.
    #[test]
    fn test_node_error_source() {
        use crate::FormatError as _;
        use crate::tests::Mid;

        let leaf: Node<&str, Mid> = Node::from(("ctx", Mid::Inner(Inner::A)));
        assert_eq!(
            leaf.source().expect("leaf has a source").to_string(),
            "InnerA"
        );
        assert_eq!(leaf.one_line().to_string(), "ctx: mid: InnerA");

        let group: Node<&str, Mid> = Subgroup::new("g", ManyErrors::new()).into();
        assert!(group.source().is_none());
    }

    #[test]
    fn test_subgroup_error_source_is_none() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        let group = Subgroup::new("g", inner);
        assert!(group.source().is_none());
        assert_eq!(group.to_string(), "g (x: InnerA)");
    }

    /// `with_formats` swaps both strategies on a nested tree without touching values.
    #[test]
    fn test_with_formats_rebuilds_tree() {
        use crate::tests::WcArrow;

        struct Bracket;
        impl<GC: core::fmt::Display> crate::Format<GC> for Bracket {
            fn fmt(label: &GC, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "[{label}]")
            }
        }

        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::B);
        outer.push_group("region", inner);
        assert_eq!(
            outer.to_string(),
            "2 errors: leaf: InnerB; region (x: InnerA)"
        );

        let swapped: ManyErrors<&str, Inner, &str, WcArrow, Bracket> = outer.with_formats();
        assert_eq!(
            swapped.to_string(),
            "2 errors: leaf -> InnerB; [region] (x -> InnerA)"
        );
    }

    /// A [`Subgroup`] extracted from the enum renders losslessly: label **and** a
    /// shallow summary of the nested errors, matching the parent's group rendering.
    #[test]
    fn test_group_display_is_lossless() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);
        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push_group("region", inner);

        let group = outer.iter().next().unwrap().as_group().unwrap();
        assert_eq!(group.to_string(), "region (2 errors: x: InnerA; y: InnerB)");
    }
}
