//! Aggregate format strategies for [`ManyErrors`]: [`Tree`], [`List`], [`Bullets`], [`Inline`].
//!
//! All strategies implement [`Format<ManyErrors<…>>`] (and the ref trampoline
//! [`Format<&ManyErrors<…>>`]) so they work with both `Display` and
//! [`Formatted`](crate::Formatted) wrappers.
//!
//! Group headers are rendered through the group's own label strategy `GF` via
//! `write!(f, "{w}")` (default [`ContextField`](crate::with_context::ContextField):
//! the label only); the structural ` (N errors):` / `: ` and children are added
//! by the aggregate strategy itself.

use core::{error::Error, fmt};

mod bullets;
mod inline;
mod list;
mod tree;

pub use bullets::Bullets;
pub use inline::Inline;
pub use list::List;
pub use tree::Tree;

/// Emits the `Format<ManyErrors<…>>` impl and its `Format<&ManyErrors<…>>` ref
/// trampoline for an aggregate strategy with no extra generic parameters. The
/// closure-like argument names the entry-point `draw_*_many` call.
macro_rules! impl_aggregate_format {
    ($strategy:ident, |$errors:ident, $f:ident| $call:expr) => {
        impl<C, E, GC, F, GF> $crate::Format<$crate::ManyErrors<C, E, GC, F, GF>> for $strategy
        where
            C: ::core::fmt::Display,
            E: ::core::error::Error + ::core::fmt::Display + 'static,
            F: $crate::Format<$crate::with_context::WithContext<C, E, F>>,
            GF: $crate::Format<$crate::many_errors::Subgroup<C, E, GC, F, GF>>,
        {
            fn fmt(
                $errors: &$crate::ManyErrors<C, E, GC, F, GF>,
                $f: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                $call
            }
        }

        impl<C, E, GC, F, GF> $crate::Format<&$crate::ManyErrors<C, E, GC, F, GF>> for $strategy
        where
            C: ::core::fmt::Display,
            E: ::core::error::Error + ::core::fmt::Display + 'static,
            F: $crate::Format<$crate::with_context::WithContext<C, E, F>>,
            GF: $crate::Format<$crate::many_errors::Subgroup<C, E, GC, F, GF>>,
        {
            fn fmt(
                errors: &&$crate::ManyErrors<C, E, GC, F, GF>,
                f: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                <Self as $crate::Format<$crate::ManyErrors<C, E, GC, F, GF>>>::fmt(errors, f)
            }
        }
    };
}

pub(crate) use impl_aggregate_format;

/// Write the source chain as `": {src1}: {src2}: ..."` on the current line.
///
/// Shared by the [`List`], [`Bullets`], and [`Inline`] leaf renderers.
pub(super) fn inline_sources(
    source: Option<&dyn Error>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let mut opt_src = source;
    while let Some(src) = opt_src {
        write!(f, ": {src}")?;
        opt_src = src.source();
    }
    Ok(())
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
