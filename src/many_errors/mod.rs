//! Aggregated, context-tagged errors from iterator/fold-style operations.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
};

use derive_where::derive_where;

use alloc::{vec, vec::Vec};

use crate::{
    AsDisplay, Format,
    with_context::{Colon, WithContext},
};

mod add;
mod iter;
mod node;
pub mod strategy;

pub use crate::connectors::{Ascii, Connectors, TreeConnectors, Unicode};
pub use iter::{IntoIter, Iter, IterMut};
pub use node::{Node, Subgroup};
pub use strategy::{Bullets, Joined, List, Tree};

/// Zero or more context-tagged errors, arranged as a rose tree.
///
/// Each child is a [`Node`]: either a leaf [`WithContext`] pair or a labeled
/// sub-group (another `ManyErrors`). The three-variant split avoids heap
/// allocation until a second error arrives.
///
/// [`Display`] renders a shallow single-line summary (each error's own text, no
/// source chains). Source-walking shapes — [`Tree`], [`List`], [`Bullets`],
/// [`Joined`] — are available via the inherent helpers
/// [`tree`](ManyErrors::tree), [`list`](ManyErrors::list),
/// [`bullets`](ManyErrors::bullets), [`joined`](ManyErrors::joined), or via
/// [`FormatError::formatted`](crate::FormatError::formatted) for full generic
/// control (e.g. `Tree<Ascii, false>`).
///
/// Note that the generic per-error helpers
/// [`one_line`](crate::FormatError::one_line) /
/// [`chain`](crate::FormatError::chain) walk [`Error::source`], which is
/// always `None` here — on a `ManyErrors` they render exactly the shallow
/// `Display` text. For deep aggregate rendering use
/// [`joined`](ManyErrors::joined) / [`tree`](ManyErrors::tree) instead.
///
/// # Customizing group rendering
/// Two independent levers:
/// - **Label decoration** — the group-label strategy `GF` is a label-only
///   [`Format<GC>`](crate::Format) (default [`AsDisplay`]).
///   Set it to wrap or restyle just the label; it composes with every built-in
///   shape, including [`tree`](ManyErrors::tree) (the label is re-indented under
///   the tree prefix). `GF` never sees the nested errors — laying those out is
///   the aggregate strategy's job, so a `GF` that rendered them would
///   double-render and break the layout.
/// - **Whole layout** — for full control over label, separators, and nesting,
///   implement [`Format<ManyErrors<…>>`](crate::Format) for your own marker
///   (exactly like [`Tree`]/[`List`]) plus a one-line ref forwarder, and render
///   via [`formatted`](ManyErrors::formatted):
///
/// ```
/// use core::fmt::{self, Display, Formatter};
/// use errortools::{Format, ManyErrors, Node};
///
/// /// Renders each direct child on its own `"- "` line (shallow).
/// struct Dashed;
///
/// impl<C: Display, E: Display, GC: Display, F, GF> Format<ManyErrors<C, E, GC, F, GF>> for Dashed {
///     fn fmt(errors: &ManyErrors<C, E, GC, F, GF>, f: &mut Formatter<'_>) -> fmt::Result {
///         for node in errors {
///             match node {
///                 Node::Leaf(w) => writeln!(f, "- {}: {}", w.context, w.error)?,
///                 Node::Group(g) => writeln!(f, "- {} ({} nested)", g.context, g.errors.len())?,
///             }
///         }
///         Ok(())
///     }
/// }
///
/// // The ref forwarder lets `Formatted<&ManyErrors<…>, Dashed>` render too.
/// impl<T: ?Sized> Format<&T> for Dashed
/// where
///     Dashed: Format<T>,
/// {
///     fn fmt(errors: &&T, f: &mut Formatter<'_>) -> fmt::Result {
///         <Self as Format<T>>::fmt(*errors, f)
///     }
/// }
///
/// let mut errs = ManyErrors::<&str, std::io::Error>::new();
/// errs.push("config", std::io::Error::other("missing"));
/// errs.push("network", std::io::Error::other("refused"));
/// assert_eq!(
///     errs.formatted::<Dashed>().to_string(),
///     "- config: missing\n- network: refused\n"
/// );
/// ```
///
/// All standard-trait impls are written manually so they do **not** add
/// `F: Trait` bounds (mirroring [`WithContext`]'s `PhantomData<fn() -> F>`).
///
/// # Context bounds
/// Rendering itself bounds neither `C` nor `GC`: leaves go through `F`, group
/// labels through `GF`, and those strategies decide what each context must
/// implement (the default [`Colon`]/[`AsDisplay`] need
/// [`Display`](core::fmt::Display); a path context works with
/// [`PathColon`](crate::with_context::PathColon) and no `Display` at all).
/// Only putting a `ManyErrors` in an [`Error`] *position* (e.g. as a
/// `#[source]`) additionally requires `C: Debug` and `GC: Debug`, because
/// [`Error`] has `Debug` as a supertrait and the bound propagates through the
/// manual `Debug` impl.
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
#[derive_where(Clone, PartialEq, Eq, Hash; C, E, GC)]
#[derive_where(Default)]
pub enum ManyErrors<C, E, GC = C, F = Colon, GF = AsDisplay> {
    /// No errors recorded.
    #[derive_where(default)]
    None,
    /// Exactly one child.
    One(Node<C, E, GC, F, GF>),
    /// Two or more children.
    Many(Vec<Node<C, E, GC, F, GF>>),
}

// `Debug` stays manual: `Many` renders as a bare list (not a `Many(..)` tuple)
// and `None` prints as the bare string — custom output, not a std-shaped derive.
impl<C: Debug, E: Debug, GC: Debug, F, GF> Debug for ManyErrors<C, E, GC, F, GF> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::One(n) => f.debug_tuple("One").field(n).finish(),
            Self::Many(v) => f.debug_list().entries(v.iter()).finish(),
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

    /// Appends a leaf error with context if `option` is `Some`; no-op on `None`.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push_some("step 1", None);
    /// assert!(errs.is_empty());
    /// errs.push_some("step 2", Some(std::io::Error::other("fail")));
    /// assert_eq!(errs.len(), 1);
    /// ```
    pub fn push_some(&mut self, context: C, option: Option<E>) {
        if let Some(error) = option {
            self.push(context, error);
        }
    }

    /// Appends a leaf error if `result` is `Err`; no-op on `Ok`.
    ///
    /// Named version of `*self += (context, result)` for discoverability.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push_result("step 1", Ok::<(), _>(()));
    /// assert!(errs.is_empty());
    /// errs.push_result("step 2", Err(std::io::Error::other("fail")));
    /// assert_eq!(errs.len(), 1);
    /// ```
    pub fn push_result<T>(&mut self, context: C, result: Result<T, E>) {
        if let Err(error) = result {
            self.push(context, error);
        }
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
        self.push_node(Node::Group(Subgroup::new(context, errors)));
    }

    /// Wraps `self` as a named sub-group inside a fresh `ManyErrors`.
    ///
    /// Builder-style complement to [`push_group`](Self::push_group): instead of
    /// attaching a child to an existing parent, this creates the parent and
    /// returns it.  Empty `self` produces a group with zero leaves.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let mut inner = ManyErrors::<&str, std::io::Error>::new();
    /// inner.push("a", std::io::Error::other("x"));
    ///
    /// let outer = inner.into_group("region");
    /// assert_eq!(outer.len(), 1); // one group node
    /// ```
    pub fn into_group(self, context: GC) -> Self {
        let mut outer = Self::None;
        outer.push_group(context, self);
        outer
    }

    /// Appends a child [`Node`] directly, promoting `None → One → Many`.
    ///
    /// Accepts anything convertible into a [`Node`]: a `(C, E)` pair, a
    /// [`WithContext`], a [`Subgroup`], or a `Node` itself — the general form
    /// behind [`push`](Self::push) / [`push_group`](Self::push_group).
    pub fn push_node(&mut self, node: impl Into<Node<C, E, GC, F, GF>>) {
        let prev = core::mem::take(self);
        *self = match prev {
            Self::None => Self::One(node.into()),
            Self::One(first) => Self::Many(vec![first, node.into()]),
            Self::Many(mut v) => {
                v.push(node.into());
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

    /// Returns `Some(())` if no errors were recorded, `None` otherwise.
    ///
    /// Mirrors [`Result::ok`]: discards the errors and signals whether the
    /// operation succeeded. For the inverse (keeping the errors) use
    /// [`err`](Self::err); to turn into a `Result` with a success value use
    /// [`into_result`](Self::into_result).
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let ok = ManyErrors::<&str, std::io::Error>::new();
    /// assert_eq!(ok.ok(), Some(()));
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push("step", std::io::Error::other("fail"));
    /// assert_eq!(errs.ok(), None);
    /// ```
    pub fn ok(self) -> Option<()> {
        matches!(self, Self::None).then_some(())
    }

    /// Returns `Some(self)` if errors were recorded, `None` if empty.
    ///
    /// Mirrors [`Result::err`]: preserves the errors and discards the success
    /// signal. Useful when you want to handle errors only when present, without
    /// needing a success value (compare [`into_result`](Self::into_result)).
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let ok = ManyErrors::<&str, std::io::Error>::new();
    /// assert!(ok.err().is_none());
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push("step", std::io::Error::other("fail"));
    /// assert!(errs.err().is_some());
    /// ```
    pub fn err(self) -> Option<Self> {
        match self {
            Self::None => None,
            other => Some(other),
        }
    }

    /// Transforms every leaf error by applying `f`, returning a new
    /// `ManyErrors` with the same context and group structure.
    ///
    /// Mirrors [`Result::map_err`]. Format strategies are reset to defaults
    /// for the new error type. Group contexts (`GC`) are left unchanged; to
    /// map those, use [`map_context`](Self::map_context) after (or before) this.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let mut errs = ManyErrors::<&str, i32>::new();
    /// errs.push("a", 1);
    /// errs.push("b", 2);
    /// let mapped = errs.map_err(|n| n.to_string());
    /// assert_eq!(mapped.len(), 2);
    /// ```
    pub fn map_err<E2>(self, mut f: impl FnMut(E) -> E2) -> ManyErrors<C, E2, GC> {
        map_err_many(self, &mut f)
    }

    /// Transforms every leaf context by applying `f`, returning a new
    /// `ManyErrors` with the same error and group-context structure.
    ///
    /// Mirrors [`Result::map`] (the "non-error" half of the pair). Only leaf
    /// contexts (`C`) are mapped; group labels (`GC`) are unchanged. Format
    /// strategies are reset to defaults for the new context type.
    ///
    /// # Example
    /// ```
    /// use errortools::ManyErrors;
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push("config", std::io::Error::other("missing"));
    /// let mapped = errs.map_context(|ctx| format!("step:{ctx}"));
    /// assert_eq!(mapped.len(), 1);
    /// ```
    pub fn map_context<C2>(self, mut f: impl FnMut(C) -> C2) -> ManyErrors<C2, E, GC> {
        map_context_many(self, &mut f)
    }

    /// Switches the leaf strategy `F` and group-label strategy `GF` without
    /// touching the stored values, rebuilding the tree recursively (O(n), one
    /// new box per group). The aggregate counterpart of
    /// [`WithContext::with_format`].
    pub fn with_formats<NewF, NewGF>(self) -> ManyErrors<C, E, GC, NewF, NewGF>
    where
        NewF: Format<WithContext<C, E, NewF>>,
        NewGF: Format<GC>,
    {
        match self {
            Self::None => ManyErrors::None,
            Self::One(node) => ManyErrors::One(node.with_formats()),
            Self::Many(nodes) => {
                ManyErrors::Many(nodes.into_iter().map(Node::with_formats).collect())
            }
        }
    }
}

// --- Inherent formatting helpers (no turbofish needed for common shapes) ---

impl<C, E, GC, F, GF> ManyErrors<C, E, GC, F, GF> {
    /// Renders as a branching Unicode tree with a count header, walking each leaf's source chain.
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

    /// Renders on a single line: `;`-separated siblings, parens around groups,
    /// walking each leaf's source chain.
    ///
    /// # Shadowing [`FormatError::one_line`](crate::FormatError::one_line)
    ///
    /// The trait method uses [`OneLine`](crate::OneLine), which walks
    /// [`Error::source`] — but `ManyErrors::source` is always `None`, so it
    /// would only emit the shallow [`Display`] summary. This inherent method
    /// wins at the call site and produces a meaningful aggregate rendering
    /// instead. The trait method remains reachable as
    /// `FormatError::one_line(&errs)` if the `OneLine` behavior is needed.
    pub fn one_line(&self) -> crate::Formatted<&Self, Joined> {
        crate::Formatted::new(self)
    }

    /// Renders on a single line: `;`-separated siblings, parens around groups.
    ///
    /// Alias for [`one_line`](Self::one_line); prefer that name for
    /// consistency with [`FormatError`](crate::FormatError).
    pub fn joined(&self) -> crate::Formatted<&Self, Joined> {
        crate::Formatted::new(self)
    }

    /// Wraps `self` for rendering with an arbitrary aggregate [`Format`]
    /// strategy `F2` — the escape hatch behind [`tree`](Self::tree)/
    /// [`list`](Self::list)/… for custom markers or non-default generics
    /// (e.g. `Tree<Ascii, false>`).
    ///
    /// # This is not [`FormatError::formatted`](crate::FormatError::formatted)
    ///
    /// The trait method comes from the blanket `impl FormatError for E: Error`,
    /// so calling it requires `ManyErrors: Error` — which drags in `C: Debug`
    /// and `GC: Debug` (the [`Error`] supertrait), even though no strategy
    /// needs them to render. This inherent method is completely unbounded
    /// (and `const`): wrapping always works; whether the combination can
    /// actually print is decided by `F2`'s own [`Format`] bounds at the
    /// `Display` call site.
    ///
    /// At a call site the inherent method always wins over the trait method,
    /// and when both apply they produce the identical
    /// [`Formatted`](crate::Formatted) value — so there is nothing to choose:
    /// `errs.formatted::<F2>()` just also compiles for aggregates that aren't
    /// errors themselves (a non-`Debug` context type, for example). The trait
    /// method remains reachable as `FormatError::formatted(&errs)` if you need
    /// to prove the `Error` bound.
    pub const fn formatted<F2>(&self) -> crate::Formatted<&Self, F2> {
        crate::Formatted::new(self)
    }
}

/// Renders a shallow, single-line summary: `"N errors: child1; child2; …"`,
/// each child's own text only (no source chains). This is the Rust-convention
/// error message; for source-walking shapes use [`tree`](ManyErrors::tree),
/// [`joined`](ManyErrors::joined), [`list`](ManyErrors::list), or
/// [`bullets`](ManyErrors::bullets).
impl<C, E, GC, F, GF> Display for ManyErrors<C, E, GC, F, GF>
where
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        <strategy::Summary as Format<Self>>::fmt(self, f)
    }
}

impl<C, E, GC, F, GF> Error for ManyErrors<C, E, GC, F, GF>
where
    C: Debug,
    GC: Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<GC>,
{
    /// Always `None`: an aggregate of independent sibling errors has no single
    /// linear cause, so it exposes nothing through [`Error::source`]. Inspect
    /// the children directly, or render the full chains via a strategy
    /// ([`tree`](Self::tree), [`joined`](Self::joined), …).
    ///
    /// Consequently, a `ManyErrors` buried in another error's source chain
    /// (e.g. as a `#[source]`) renders as one shallow [`Display`] line under
    /// chain-walking strategies like [`OneLine`](crate::OneLine) /
    /// [`Chain`](crate::Chain), and the walk stops there — a generic strategy
    /// cannot see branches through `dyn Error`. To render an aggregate's
    /// children deeply, lift it into a [`push_group`](Self::push_group) of an
    /// outer `ManyErrors` instead of chaining it as a source.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

// --- map_err / map_context helpers ---
// Free functions so Subgroup (in node.rs) does not need access to a private
// method on ManyErrors; WithContext and Subgroup fields are all `pub`.

fn map_err_node<C, E, GC, F, GF, E2>(
    node: Node<C, E, GC, F, GF>,
    f: &mut impl FnMut(E) -> E2,
) -> Node<C, E2, GC> {
    match node {
        Node::Leaf(w) => Node::Leaf(WithContext::new(w.context, f(w.error))),
        Node::Group(g) => Node::Group(Subgroup::new(g.context, map_err_many(*g.errors, f))),
    }
}

fn map_err_many<C, E, GC, F, GF, E2>(
    me: ManyErrors<C, E, GC, F, GF>,
    f: &mut impl FnMut(E) -> E2,
) -> ManyErrors<C, E2, GC> {
    match me {
        ManyErrors::None => ManyErrors::None,
        ManyErrors::One(n) => ManyErrors::One(map_err_node(n, f)),
        ManyErrors::Many(v) => {
            let mut out = Vec::with_capacity(v.len());
            for n in v {
                out.push(map_err_node(n, f));
            }
            ManyErrors::Many(out)
        }
    }
}

fn map_context_node<C, E, GC, F, GF, C2>(
    node: Node<C, E, GC, F, GF>,
    f: &mut impl FnMut(C) -> C2,
) -> Node<C2, E, GC> {
    match node {
        Node::Leaf(w) => Node::Leaf(WithContext::new(f(w.context), w.error)),
        Node::Group(g) => Node::Group(Subgroup::new(g.context, map_context_many(*g.errors, f))),
    }
}

fn map_context_many<C, E, GC, F, GF, C2>(
    me: ManyErrors<C, E, GC, F, GF>,
    f: &mut impl FnMut(C) -> C2,
) -> ManyErrors<C2, E, GC> {
    match me {
        ManyErrors::None => ManyErrors::None,
        ManyErrors::One(n) => ManyErrors::One(map_context_node(n, f)),
        ManyErrors::Many(v) => {
            let mut out = Vec::with_capacity(v.len());
            for n in v {
                out.push(map_context_node(n, f));
            }
            ManyErrors::Many(out)
        }
    }
}

/// Converts `(context, Ok(_))` into an empty `ManyErrors` and
/// `(context, Err(e))` into a single-leaf `ManyErrors`.
impl<C, E, GC, F, GF, T> From<(C, Result<T, E>)> for ManyErrors<C, E, GC, F, GF> {
    fn from((context, result): (C, Result<T, E>)) -> Self {
        match result {
            Ok(_) => Self::None,
            Err(error) => {
                let mut me = Self::None;
                me.push(context, error);
                me
            }
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
    fn test_source_one_leaf_is_none() {
        // An aggregate has no single linear cause, even with one leaf.
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("ctx", Mid::Inner(Inner::A));
        assert!(e.source().is_none());
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

    // --- Debug (custom: `None` bare, `One(..)` tuple, `Many` bare list) ---

    #[test]
    fn test_debug_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(format!("{e:?}"), "None");
    }

    #[test]
    fn test_debug_one_is_tuple() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        assert_eq!(
            format!("{e:?}"),
            r#"One(Leaf(WithContext { context: "ctx", error: A, .. }))"#
        );
    }

    #[test]
    fn test_debug_many_is_bare_list() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        // Bare list, not a `Many(..)` tuple wrapper.
        assert_eq!(
            format!("{e:?}"),
            r#"[Leaf(WithContext { context: "a", error: A, .. }), Leaf(WithContext { context: "b", error: B, .. })]"#
        );
    }

    // --- with_formats ---

    #[test]
    fn test_with_formats_none_stays_none() {
        let e = ManyErrors::<&str, Inner>::new();
        let swapped: ManyErrors<&str, Inner, &str, crate::tests::WcArrow> = e.with_formats();
        assert!(matches!(swapped, ManyErrors::None));
    }

    // --- Display (Summary: shallow, single-line, no source chains) ---

    #[test]
    fn test_display_empty() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.to_string(), "no errors");
    }

    #[test]
    fn test_display_single_leaf_no_header() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("ctx", Inner::A);
        // Single item: no count header
        assert_eq!(e.to_string(), "ctx: InnerA");
    }

    #[test]
    fn test_display_two_leaves() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        assert_eq!(e.to_string(), "2 errors: a: InnerA; b: InnerB");
    }

    /// Default Display does not walk a leaf's source chain.
    #[test]
    fn test_display_does_not_walk_source() {
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("a", Mid::Inner(Inner::A));
        e.push("b", Mid::Inner(Inner::B));
        let s = e.to_string();
        assert_eq!(s, "2 errors: a: mid; b: mid");
        assert!(!s.contains("InnerA"), "source must not be walked: {s}");
    }

    #[test]
    fn test_display_nested_group() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);
        inner.push("y", Inner::B);

        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::A);
        outer.push_group("region", inner);

        assert_eq!(
            outer.to_string(),
            "2 errors: leaf: InnerA; region (2 errors: x: InnerA; y: InnerB)"
        );
    }

    #[test]
    fn test_one_line_single_leaf_walks_chain() {
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("ctx", Mid::Inner(Inner::A));
        assert_eq!(e.one_line().to_string(), "ctx: mid: InnerA");
        assert_eq!(e.joined().to_string(), e.one_line().to_string());
    }

    /// `one_line()` walks leaf chains; `FormatError::one_line` stops at Display.
    #[test]
    fn test_one_line_shadows_format_error_trait() {
        use crate::FormatError;
        let mut e = ManyErrors::<&str, Mid>::new();
        e.push("a", Mid::Inner(Inner::A));
        e.push("b", Mid::Inner(Inner::B));
        // inherent: walks chains
        assert_eq!(
            e.one_line().to_string(),
            "2 errors: a: mid: InnerA; b: mid: InnerB"
        );
        // trait (shallow): stops at ManyErrors::source() = None
        assert_eq!(
            FormatError::one_line(&e).to_string(),
            "2 errors: a: mid; b: mid"
        );
    }

    // --- ok / err ---

    #[test]
    fn test_ok_empty_is_some() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.ok(), Some(()));
    }

    #[test]
    fn test_ok_with_errors_is_none() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        assert_eq!(e.ok(), None);
    }

    #[test]
    fn test_err_empty_is_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert!(e.err().is_none());
    }

    #[test]
    fn test_err_with_errors_is_some() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        let errs = e.err().expect("should be Some");
        assert_eq!(errs.len(), 1);
    }

    // --- map_err ---

    #[test]
    fn test_map_err_none_stays_none() {
        let e = ManyErrors::<&str, Inner>::new();
        let mapped: ManyErrors<&str, String> = e.map_err(|err| err.to_string());
        assert!(mapped.is_empty());
    }

    #[test]
    fn test_map_err_transforms_leaves() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        // Map Inner → Mid to stay within Error-implementing types.
        let mapped = e.map_err(Mid::Inner);
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped.to_string(), "2 errors: a: mid; b: mid");
    }

    #[test]
    fn test_map_err_recurses_into_groups() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);

        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::B);
        outer.push_group("region", inner);

        let mapped = outer.map_err(Mid::Inner);
        assert_eq!(mapped.len(), 2);
        // Group structure preserved, errors replaced.
        assert_eq!(
            mapped.to_string(),
            "2 errors: leaf: mid; region (x: mid)"
        );
    }

    // --- map_context ---

    #[test]
    fn test_map_context_none_stays_none() {
        let e = ManyErrors::<&str, Inner>::new();
        // GC stays &str (unchanged); only leaf C changes.
        let mapped: ManyErrors<String, Inner, &str> = e.map_context(|ctx| ctx.to_uppercase());
        assert!(mapped.is_empty());
    }

    #[test]
    fn test_map_context_transforms_leaves() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        let mapped = e.map_context(|ctx| ctx.to_uppercase());
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped.to_string(), "2 errors: A: InnerA; B: InnerB");
    }

    #[test]
    fn test_map_context_recurses_into_groups_leaves_only() {
        let mut inner = ManyErrors::<&str, Inner>::new();
        inner.push("x", Inner::A);

        let mut outer = ManyErrors::<&str, Inner>::new();
        outer.push("leaf", Inner::B);
        outer.push_group("region", inner);

        let mapped = outer.map_context(|ctx| ctx.to_uppercase());
        // group label "region" (GC) unchanged; leaf contexts uppercased
        assert_eq!(
            mapped.to_string(),
            "2 errors: LEAF: InnerB; region (X: InnerA)"
        );
    }

    // --- From<(C, Result<T, E>)> ---

    #[test]
    fn test_from_result_err_produces_one_leaf() {
        let e: ManyErrors<&str, Inner> = ("ctx", Err::<(), _>(Inner::A)).into();
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn test_from_result_ok_produces_empty() {
        let e: ManyErrors<&str, Inner> = ("ctx", Ok::<(), Inner>(())).into();
        assert!(e.is_empty());
    }
}
