//! Per-error source-chain ladder renderer ([`Chain`]).
//!
//! This is distinct from [`Tree`](crate::many_errors::Tree), which renders
//! a branching *aggregate* of many errors. `Chain` renders a *single* error's
//! linear source chain as an indented ladder:
//!
//! ```text
//! top error
//! └─ source 1
//!    └─ source 2
//! ```

use core::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    iter,
    marker::PhantomData,
};

use itertools::Itertools;

use crate::{
    Format, chain,
    connectors::{Connectors, Unicode},
};

/// Per-error source-chain ladder format, drawn with a [`Connectors`] glyph set.
///
/// ```text
/// top error
/// └─ source 1
///    └─ source 2
/// ```
///
/// A linear chain is a degenerate tree — every node is an only-child — so it
/// uses only the "last child" branch glyph ([`Connectors::LAST`]) and the blank
/// continuation ([`Connectors::GAP`]). The marker is printed before each source
/// and the continuation is repeated `depth - 1` times. Swap the glyph set with
/// [`Ascii`](crate::Ascii) (or any custom [`Connectors`] impl) the same way
/// [`Tree`](crate::many_errors::Tree) does — one vocabulary serves both.
///
/// Use [`FormatError::chain`](crate::FormatError::chain) for the most common case.
/// For aggregate many-error rendering see [`Tree`](crate::many_errors::Tree).
pub struct Chain<C = Unicode>(PhantomData<fn() -> C>);

// Manual impls so the phantom connector `C` gets no `C: Trait` bound from
// derives (the `_format`-style doctrine; see `WithContext`).
impl<C> Default for Chain<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
impl<C> Clone for Chain<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C> Copy for Chain<C> {}
impl<C> PartialEq for Chain<C> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl<C> Eq for Chain<C> {}
impl<C> Hash for Chain<C> {
    fn hash<H: Hasher>(&self, _: &mut H) {}
}

/// Walks the source chain. Prints the top error on its own line, then each
/// source on a new line preceded by `(depth - 1)` repetitions of
/// [`Connectors::GAP`] followed by [`Connectors::LAST`].
impl<E: Error + ?Sized, C: Connectors> Format<E> for Chain<C> {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // &error: &&E; &&E coerces to &dyn Error via the blanket `impl<T: Error + ?Sized> Error for &T`.
        let formatted =
            chain(&error)
                .enumerate()
                .format_with("\n", |(depth, e), write| match depth {
                    0 => write(&format_args!("{e}")),
                    n => {
                        let pad = iter::repeat_n(C::GAP, n - 1).format("");
                        write(&format_args!("{pad}{}{e}", C::LAST))
                    }
                });
        write!(f, "{formatted}")
    }
}

/// Prints the connector type (instantiated via [`Default`]) instead of
/// `Chain(PhantomData)`.
impl<C: fmt::Debug + Default> fmt::Debug for Chain<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Chain").field(&C::default()).finish()
    }
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use itertools::Itertools;

    use crate::{
        Chain, Format, FormatError, Formatted, chain,
        connectors::{Ascii, Connectors, Unicode},
        tests::{Error, Inner},
    };

    #[test]
    fn test_chain_no_source() {
        let error = Error::One;
        assert_eq!(error.chain().to_string(), "One");
    }

    #[test]
    fn test_chain_one_source() {
        let error = Error::Two(Inner::A);
        assert_eq!(error.chain().to_string(), "Two\n└─ InnerA");
    }

    #[test]
    fn test_chain_nested() {
        let error = Error::Two(Inner::B);
        assert_eq!(error.chain().to_string(), "Two\n└─ InnerB");
    }

    #[test]
    fn test_chain_ascii() {
        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Chain<Ascii>>::new(error).to_string(),
            "Two\n`- InnerA"
        );
    }

    #[test]
    fn test_chain_custom_connectors() {
        struct Arrow;
        impl Connectors for Arrow {
            const LAST: &'static str = "|-> ";
            const GAP: &'static str = "  ";
        }

        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Chain<Arrow>>::new(error).to_string(),
            "Two\n|-> InnerA"
        );
    }

    #[test]
    fn test_chain_debug_default_params() {
        let c = Chain::<Unicode>::default();
        assert_eq!(format!("{c:?}"), "Chain(Unicode)");
    }

    #[test]
    fn test_chain_debug_custom_params() {
        let c = Chain::<Ascii>::default();
        assert_eq!(format!("{c:?}"), "Chain(Ascii)");
    }

    #[test]
    fn test_custom_chain_via_format() {
        struct AsciiChain;
        impl<E: core::error::Error + ?Sized> Format<E> for AsciiChain {
            fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let formatted = chain(&error)
                    .enumerate()
                    .format_with("\n", |(depth, e), write| match depth {
                        0 => write(&format_args!("{e}")),
                        n => write(&format_args!("{:width$}|-- {e}", "", width = (n - 1) * 2)),
                    });
                write!(f, "{formatted}")
            }
        }

        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, AsciiChain>::new(error).to_string(),
            "Two\n|-- InnerA"
        );
    }
}
