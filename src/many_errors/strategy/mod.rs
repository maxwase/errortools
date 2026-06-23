//! Aggregate format strategies for [`ManyErrors`](crate::ManyErrors): [`Tree`], [`List`], [`Bullets`], [`Joined`].
//!
//! All strategies implement [`Format<ManyErrors<…>>`] (and the ref trampoline
//! [`Format<&ManyErrors<…>>`]) so they work with both `Display` and
//! [`Formatted`](crate::Formatted) wrappers.
//!
//! `Summary` is the crate-internal shallow strategy backing the default
//! [`Display`]: own text only, no source chains.
//!
//! # Authoring a custom aggregate strategy
//!
//! A custom strategy is a unit type implementing
//! [`Format<ManyErrors<…>>`](crate::Format). Match on the public
//! [`ManyErrors`](crate::ManyErrors) and [`Node`](crate::Node) variants to walk
//! the tree, and reuse the public
//! helpers here so the output stays consistent with the built-in shapes:
//! [`ErrorCount`] for the `"N errors"` count phrase, [`NO_ERRORS`] for the empty
//! marker, and [`LeafChain`] to render a leaf together with its `": "`-joined
//! source chain (without forcing a `C: Debug` bound). Re-indent any embedded
//! newlines with [`indented`](crate::indent::indented). Pair the impl with the
//! [`impl_ref_format!`](crate::impl_ref_format) macro to get the `&T` trampoline
//! for free.
//!
//! Group headers are rendered through the group's own label strategy `GF` via
//! `write!(f, "{w}")` (default [`AsDisplay`](crate::AsDisplay): the label's own
//! `Display`). `GF` is a label-only [`Format<GC>`](crate::Format); the structural
//! ` (N errors):` / `: ` and the children are added by the aggregate strategy
//! itself, which owns all nested layout.
//!
//! ```
//! use core::fmt::{self, Formatter};
//! use errortools::{Format, ManyErrors, Node};
//! use errortools::many_errors::strategy::{ErrorCount, LeafChain, NO_ERRORS};
//!
//! // A flat strategy: "<count> errors -> leaf; leaf; …", leaves only.
//! struct Arrows;
//! impl<C, E, GC, F, GF> Format<ManyErrors<C, E, GC, F, GF>> for Arrows
//! where
//!     E: core::error::Error + 'static,
//!     F: Format<errortools::WithContext<C, E, F>>,
//! {
//!     fn fmt(errors: &ManyErrors<C, E, GC, F, GF>, f: &mut Formatter<'_>) -> fmt::Result {
//!         if errors.is_empty() {
//!             return f.write_str(NO_ERRORS);
//!         }
//!         write!(f, "{} ->", ErrorCount(errors.len()))?;
//!         for (i, node) in errors.iter().enumerate() {
//!             let sep = if i == 0 { " " } else { "; " };
//!             if let Node::Leaf(w) = node {
//!                 write!(f, "{sep}{}", LeafChain(w))?;
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//! errortools::impl_ref_format!(Arrows);
//!
//! let mut errs = ManyErrors::<&str, std::io::Error>::new();
//! errs.push("a.txt", std::io::Error::other("missing"));
//! errs.push("b.txt", std::io::Error::other("denied"));
//! assert_eq!(
//!     errs.formatted::<Arrows>().to_string(),
//!     "2 errors -> a.txt: missing; b.txt: denied",
//! );
//! ```

use core::{
    error::Error,
    fmt::{self, Display},
};

use crate::{Format, chain, with_context::WithContext};

mod bullets;
mod list;
mod marked;
mod one_line;
mod tree;

pub use bullets::Bullets;
pub use list::List;
pub use one_line::Joined;
pub(crate) use one_line::Summary;
pub use tree::Tree;

/// Emits the ref trampoline `Format<&T> where Self: Format<T>` for a strategy:
/// any `&T` formats like `T`, so `&ManyErrors` (and deeper references) work in
/// [`Formatted`](crate::Formatted) without a dedicated impl per reference
/// level. Extra generic parameters of the strategy go after the type, e.g.
/// `impl_ref_format!(Tree<Conn, HEADER>, Conn, const HEADER: bool)`.
///
/// Pair this with a `Format<ManyErrors<…>>` impl when authoring a custom
/// aggregate strategy so it works behind a reference (the form
/// [`Formatted`](crate::Formatted) wraps).
///
/// # Why a per-type macro and not one blanket impl
///
/// The tempting `impl<S, T: ?Sized> Format<&T> for S where S: Format<T>` is
/// rejected by coherence: [`AsDisplay`](crate::AsDisplay)'s
/// `impl<T: Display + ?Sized> Format<T>` already matches reference types
/// (`&U: Display` whenever `U: Display`), so `AsDisplay: Format<&U>` would be
/// provable two ways and stable Rust cannot prove the two disjoint. Removing it
/// would need specialization or negative bounds (neither stable).
///
/// This forwarding exists only because [`Formatted`](crate::Formatted) stores
/// its value *by value* while the borrowing constructors (`tree`, `formatted`,
/// …) hand it a `&ManyErrors`. A borrow-only `Formatted` would drop the need
/// for all but pathological multi-reference cases — at the cost of owned
/// wrappers. So the macro is the price of that ownership choice, not a wart to
/// be removed by a future compiler.
#[macro_export]
macro_rules! impl_ref_format {
    ($strategy:ty $(, $($gen:tt)*)?) => {
        impl<T: ?Sized $(, $($gen)*)?> $crate::Format<&T> for $strategy
        where
            Self: $crate::Format<T>,
        {
            fn fmt(error: &&T, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                <Self as $crate::Format<T>>::fmt(*error, f)
            }
        }
    };
}

/// Emits the `Format<ManyErrors<…>>` impl and a generic `Format<&T>` ref
/// trampoline for an aggregate strategy with no extra generic parameters.
///
/// The closure-like argument names the entry-point `draw_*` call.
///
/// Note no `C: Display`/`C: Debug` bound: leaves render through `F`, group
/// labels through `GF` — the strategies decide what each context must
/// implement (so e.g. a `PathBuf` context works with
/// [`PathColon`](crate::with_context::PathColon)).
macro_rules! impl_aggregate_format {
    ($strategy:ident, |$errors:ident, $f:ident| $call:expr) => {
        impl<C, E, GC, F, GF> $crate::Format<$crate::ManyErrors<C, E, GC, F, GF>> for $strategy
        where
            E: ::core::error::Error + 'static,
            F: $crate::Format<$crate::with_context::WithContext<C, E, F>>,
            GF: $crate::Format<GC>,
        {
            fn fmt(
                $errors: &$crate::ManyErrors<C, E, GC, F, GF>,
                $f: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                $call
            }
        }

        $crate::impl_ref_format!($strategy);
    };
}

pub(crate) use impl_aggregate_format;

/// Rendered when an aggregate (or a group) has no children.
pub const NO_ERRORS: &str = "no errors";

/// `"N errors"` — the count phrase every aggregate header builds on
/// (`"N errors:"`, `" (N errors):"`, `"N errors: "`). One definition keeps the
/// wording identical across [`Tree`], [`List`], [`Bullets`], [`Joined`], the
/// default `Display`, and any custom strategy that reuses it.
pub struct ErrorCount(pub usize);

impl Display for ErrorCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} errors", self.0)
    }
}

/// Displays a leaf pair (`{w}` via its strategy `F`) followed by its
/// `": "`-joined source chain.
///
/// Output-identical to routing the leaf through [`OneLine`](crate::OneLine):
/// [`WithContext`]'s `Error::source` skips the inner error (already printed by
/// `F`), so both walks start at the same place. Unlike `OneLine`, this does not
/// go through `WithContext: Error`, so it imposes no `C: Debug` bound.
///
/// Reuse this in a custom aggregate strategy to render a leaf node's
/// [`WithContext`] pair identically to the built-in shapes.
pub struct LeafChain<'a, C, E, F>(pub &'a WithContext<C, E, F>);

impl<C, E, F> Display for LeafChain<'_, C, E, F>
where
    E: Error,
    F: Format<WithContext<C, E, F>>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        F::fmt(self.0, f)?;
        // `chain` yields the error itself first; skipping it leaves exactly
        // the sources (`WithContext::source` starts past the inner error too).
        for src in chain(&self.0.error).skip(1) {
            write!(f, ": {src}")?;
        }
        Ok(())
    }
}

/// Renders a group label through its label strategy `GF`, wrapped so it can be
/// handed to [`indented`](crate::indent::indented) (whose re-indentation needs
/// a single [`Display`] value). This is the label-only path — the group's own
/// [`Display`] would also summarize the nested errors, which the aggregate
/// strategy draws itself.
pub(crate) struct Label<'a, GC: ?Sized, GF>(pub &'a GC, pub core::marker::PhantomData<fn() -> GF>);

impl<GC: ?Sized, GF: Format<GC>> Display for Label<'_, GC, GF> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        GF::fmt(self.0, f)
    }
}

#[cfg(test)]
pub(super) mod test_helpers {
    use crate::ManyErrors;
    use crate::tests::{Inner, Mid};

    pub fn two_leaves() -> ManyErrors<&'static str, Inner> {
        let mut e = ManyErrors::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        e
    }

    pub fn with_chain() -> ManyErrors<&'static str, Mid> {
        let mut e = ManyErrors::new();
        e.push("a", Mid::Inner(Inner::A));
        e.push("b", Mid::Inner(Inner::B));
        e
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::{self, Display, Formatter};

    use super::LeafChain;
    use crate::tests::{Inner, Mid};
    use crate::{Format, FormatError, Formatted, ManyErrors, WithContext};

    use super::test_helpers::two_leaves;

    /// The generic ref trampoline forwards through any number of reference levels.
    #[test]
    fn test_ref_trampoline_double_reference() {
        let e = two_leaves();
        let direct = e.list().to_string();
        assert_eq!(Formatted::<_, super::List>::new(&&e).to_string(), direct);
        assert_eq!(
            Formatted::<_, super::Tree>::new(&&e).to_string(),
            e.tree().to_string()
        );
    }

    /// `LeafChain` is output-identical to `OneLine` on the same `WithContext`.
    #[test]
    fn test_leaf_chain_equals_one_line() {
        let w = WithContext::<_, _, crate::with_context::Colon>::new("ctx", Mid::Inner(Inner::A));
        assert_eq!(LeafChain(&w).to_string(), w.one_line().to_string());

        let no_source = WithContext::<_, _, crate::with_context::Colon>::new("ctx", Inner::A);
        assert_eq!(
            LeafChain(&no_source).to_string(),
            no_source.one_line().to_string()
        );
    }

    /// A `Display`-only (non-`Debug`) context renders through every shape.
    #[test]
    fn test_non_debug_context_renders() {
        struct NoDebug(&'static str);
        impl Display for NoDebug {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.write_str(self.0)
            }
        }

        let mut e = ManyErrors::<NoDebug, Inner>::new();
        e.push(NoDebug("a"), Inner::A);
        e.push(NoDebug("b"), Inner::B);

        assert_eq!(e.to_string(), "2 errors: a: InnerA; b: InnerB");
        assert_eq!(
            e.tree().to_string(),
            "2 errors:\n├─ a: InnerA\n└─ b: InnerB"
        );
        assert_eq!(
            e.list().to_string(),
            "2 errors:\n1. a: InnerA\n2. b: InnerB"
        );
        assert_eq!(
            e.bullets().to_string(),
            "2 errors:\n  • a: InnerA\n  • b: InnerB"
        );
        assert_eq!(e.joined().to_string(), "2 errors: a: InnerA; b: InnerB");
    }

    /// A non-`Display` context (`PathBuf`) renders when `F` knows how to print
    /// it ([`PathColon`](crate::with_context::PathColon)) — no `C: Display`
    /// bound anywhere in the aggregate path.
    #[cfg(feature = "std")]
    #[test]
    fn test_non_display_context_via_path_colon() {
        use crate::with_context::PathColon;
        use std::{io, path::PathBuf};

        let mut e = ManyErrors::<PathBuf, io::Error, &str, PathColon>::new();
        e.push(PathBuf::from("a.txt"), io::Error::other("missing"));
        e.push(PathBuf::from("b.txt"), io::Error::other("denied"));

        assert_eq!(e.to_string(), "2 errors: a.txt: missing; b.txt: denied");
        assert_eq!(
            e.tree().to_string(),
            "2 errors:\n├─ a.txt: missing\n└─ b.txt: denied"
        );
        assert_eq!(
            e.list().to_string(),
            "2 errors:\n1. a.txt: missing\n2. b.txt: denied"
        );
        assert_eq!(
            e.joined().to_string(),
            "2 errors: a.txt: missing; b.txt: denied"
        );
    }

    /// A user marker built per the documented recipe formats through the
    /// unbounded inherent [`ManyErrors::formatted`].
    #[test]
    fn test_user_marker_via_recipe() {
        struct Count;
        impl<C, E, GC, F, GF> Format<ManyErrors<C, E, GC, F, GF>> for Count {
            fn fmt(errors: &ManyErrors<C, E, GC, F, GF>, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{} direct children", errors.len())
            }
        }
        // The public macro supplies the `&T` ref trampoline a custom strategy needs.
        impl_ref_format!(Count);

        let e = two_leaves();
        assert_eq!(e.formatted::<Count>().to_string(), "2 direct children");
    }
}
