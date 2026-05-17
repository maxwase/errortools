use core::{
    fmt::{self, Formatter},
    marker::PhantomData,
};

use crate::{AsDisplay, Format};

use super::{ManyErrors, WithContext};

/// Aggregate strategy that renders each item in a [`ManyErrors`] on its own
/// line, formatting each via the per-item strategy `IndividualErrorFormat`.
///
/// The default `G = AsDisplay` defers to each item's own [`fmt::Display`] (and
/// thus its own type-level strategy `WithContextFormat`). Pass a concrete `IndividualErrorFormat` (e.g.
/// [`OneLine`](crate::OneLine) or [`Tree`](crate::Tree)) to override per-item
/// rendering.
///
/// Listing is implemented for both [`ManyErrors<C, E, WithContextFormat>`] and
/// [`&ManyErrors<C, E, WithContextFormat>`](crate::ManyErrors) so it can be used directly inside this module's
/// [`fmt::Display`] and via the [`Formatted`](crate::Formatted) wrapper (which holds
/// a reference) from [`FormatError::formatted`](crate::FormatError::formatted).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Listing<IndividualErrorFormat = AsDisplay>(PhantomData<fn() -> IndividualErrorFormat>);

impl<C, E, WithContextFormat, IndividualErrorFormat> Format<ManyErrors<C, E, WithContextFormat>>
    for Listing<IndividualErrorFormat>
where
    IndividualErrorFormat: Format<WithContext<C, E, WithContextFormat>>,
{
    fn fmt(error: &ManyErrors<C, E, WithContextFormat>, f: &mut Formatter<'_>) -> fmt::Result {
        let mut it = error.iter();
        let Some(first) = it.next() else {
            return Ok(());
        };
        IndividualErrorFormat::fmt(first, f)?;
        for p in it {
            writeln!(f)?;
            IndividualErrorFormat::fmt(p, f)?;
        }
        Ok(())
    }
}

/// Trampoline so [`Formatted<&ManyErrors<_>, Listing<IndividualErrorFormat>>`](crate::Formatted)
/// (the type produced by `e.formatted::<Listing<_>>()`) can dispatch through
/// the owned impl above.
impl<C, E, WithContextFormat, IndividualErrorFormat> Format<&ManyErrors<C, E, WithContextFormat>>
    for Listing<IndividualErrorFormat>
where
    IndividualErrorFormat: Format<WithContext<C, E, WithContextFormat>>,
{
    fn fmt(error: &&ManyErrors<C, E, WithContextFormat>, f: &mut Formatter<'_>) -> fmt::Result {
        <Self as Format<ManyErrors<C, E, WithContextFormat>>>::fmt(error, f)
    }
}

#[cfg(test)]
mod tests {
    use super::Listing;
    use crate::{
        FormatError, ManyErrors, OneLine, Tree, WithContext,
        tests::{Inner, Mid, WcArrow},
    };

    #[test]
    fn test_format_zero_errors() {
        let e = ManyErrors::<&str, Inner>::new();

        // Display (default Listing<AsDisplay>).
        assert_eq!(e.to_string(), "");
        // Explicit Listing variants — all empty.
        assert_eq!(e.formatted::<Listing>().to_string(), "");
        assert_eq!(e.formatted::<Listing<OneLine>>().to_string(), "");
        assert_eq!(e.formatted::<Listing<Tree>>().to_string(), "");
    }

    #[test]
    fn test_format_one_error() {
        // Mid → Inner so OneLine / Tree have a chain to walk.
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("ctx", Mid::Inner(Inner::A)));

        // Default WithContextFormat = Colon → "{context}: {error}".
        assert_eq!(e.to_string(), "ctx: mid");
        assert_eq!(e.formatted::<Listing>().to_string(), "ctx: mid");
        // Listing<OneLine> walks the chain.
        assert_eq!(
            e.formatted::<Listing<OneLine>>().to_string(),
            "ctx: mid: InnerA"
        );
        assert_eq!(
            e.formatted::<Listing<Tree>>().to_string(),
            "ctx: mid\n└── InnerA",
        );

        // Per-item WithContextFormat override (WcArrow) — affects items' own
        // Display, which is what Listing<AsDisplay> defers to.
        let mut a: ManyErrors<&str, Mid, _> = ManyErrors::new();
        a.push(WithContext::<_, _, WcArrow>::new(
            "ctx",
            Mid::Inner(Inner::A),
        ));
        assert_eq!(a.to_string(), "ctx -> mid");
        assert_eq!(a.formatted::<Listing>().to_string(), "ctx -> mid");
        // Listing<OneLine> does NOT fully override: OneLine walks the Error
        // chain, whose first element is the WithContext itself — and that
        // WithContext's Display still fires its own F=WcArrow. Limitation.
        assert_eq!(
            a.formatted::<Listing<OneLine>>().to_string(),
            "ctx -> mid: InnerA",
        );
        assert_eq!(
            a.formatted::<Listing<Tree>>().to_string(),
            "ctx -> mid\n└── InnerA",
        );
    }

    #[test]
    fn test_format_many_errors() {
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("a", Mid::Inner(Inner::A)));
        e.push(WithContext::new("b", Mid::Inner(Inner::A)));
        e.push(WithContext::new("c", Mid::Inner(Inner::A)));

        assert_eq!(e.to_string(), "a: mid\nb: mid\nc: mid");
        assert_eq!(e.formatted::<Listing>().to_string(), e.to_string());
        assert_eq!(
            e.formatted::<Listing<OneLine>>().to_string(),
            "a: mid: InnerA\nb: mid: InnerA\nc: mid: InnerA",
        );
        assert_eq!(
            e.formatted::<Listing<Tree>>().to_string(),
            "a: mid\n└── InnerA\nb: mid\n└── InnerA\nc: mid\n└── InnerA",
        );

        // WcArrow override on items.
        let mut a: ManyErrors<&str, Mid, _> = ManyErrors::new();
        a.push(WithContext::<_, _, WcArrow>::new("a", Mid::Inner(Inner::A)));
        a.push(WithContext::<_, _, WcArrow>::new("b", Mid::Inner(Inner::A)));
        assert_eq!(a.to_string(), "a -> mid\nb -> mid");
        assert_eq!(
            a.formatted::<Listing<OneLine>>().to_string(),
            "a -> mid: InnerA\nb -> mid: InnerA",
        );
        assert_eq!(
            a.formatted::<Listing<Tree>>().to_string(),
            "a -> mid\n└── InnerA\nb -> mid\n└── InnerA",
        );
    }
}
