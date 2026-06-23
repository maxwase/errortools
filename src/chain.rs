//! Per-error source-chain ladder renderer ([`Chain`]).
//!
//! This is distinct from `Tree` (requires the `alloc` feature), which renders
//! a branching *aggregate* of many errors. `Chain` renders a *single* error's
//! linear source chain as an indented ladder:
//!
//! ```text
//! top error
//! └─ source 1
//!    └─ source 2
//! ```

use core::{error::Error, fmt, marker::PhantomData};

use derive_where::derive_where;

use crate::{
    Format, chain,
    connectors::{Connectors, Unicode},
    indent::{Repeat, indented},
};

/// Per-error source-chain ladder format, drawn with a [`Connectors`] glyph set.
///
/// ```text
/// top error
/// └─ source 1
///    └─ source 2
/// ```
///
/// # `Chain` vs `Tree`
///
/// `Chain` formats a **single** error's linear source chain — one error, one
/// straight line of causes. `Tree` (requires `alloc`) formats a **`ManyErrors`
/// aggregate** — many independent errors, each with their own source chain,
/// arranged as a branching tree. They share the same [`Connectors`] glyph
/// vocabulary so `Chain<Ascii>` and `Tree<Ascii>` look consistent.
///
/// A linear chain is a degenerate tree — every node is an only-child — so it
/// uses only the "last child" branch glyph ([`Connectors::LAST`]) and the blank
/// continuation ([`Connectors::GAP`]). The marker is printed before each source
/// and the continuation is repeated `depth - 1` times.
///
/// Use [`FormatError::chain`](crate::FormatError::chain) for the most common case.
/// For aggregate many-error rendering see `Tree` (requires the `alloc` feature).
///
/// An aggregate (`ManyErrors`, requires the `alloc` feature) buried in the source chain
/// renders as one shallow summary line and stops the walk (its `source()` is
/// `None`, and branching can't be recovered through `dyn Error`) — lift it
/// into a `push_group` of an outer aggregate for deep rendering.
#[derive_where(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Chain<C = Unicode>(PhantomData<fn() -> C>);

/// Walks the source chain. Prints the top error on its own line, then each
/// source on a new line preceded by `(depth - 1)` repetitions of
/// [`Connectors::GAP`] followed by [`Connectors::LAST`].
///
/// A source whose message embeds `\n` is re-indented: continuation lines carry
/// `depth` repetitions of [`Connectors::GAP`], keeping them under the ladder
/// instead of spilling flush-left.
impl<E: Error + ?Sized, C: Connectors> Format<E> for Chain<C> {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // &error: &&E; &&E coerces to &dyn Error via the blanket `impl<T: Error + ?Sized> Error for &T`.
        for (depth, e) in chain(&error).enumerate() {
            match depth {
                0 => write!(f, "{e}")?,
                n => {
                    write!(f, "\n{}{}", Repeat(C::GAP, n - 1), C::LAST)?;
                    indented(f, Repeat(C::GAP, n), e)?;
                }
            }
        }
        Ok(())
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
