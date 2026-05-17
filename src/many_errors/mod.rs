//! Aggregated, context-tagged errors from iterator/fold-style operations.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

use alloc::{vec, vec::Vec};

use crate::{
    AsDisplay, Format,
    with_context::{Colon, WithContext},
};

mod iter;

/// Zero or more context-tagged errors collected during an iterator/fold operation.
///
/// The three-variant split lets consumers pattern-match on the empty / single /
/// multiple cases. [`Display`] renders each recorded [`WithContext`] via the
/// strategy `WithContextFormat`, one per line — mirroring [`WithContext`]'s
/// strategy-dispatched Display. The default `WithContextFormat = Colon` produces
/// `"{context}: {error}"` per item.
///
/// # Example
/// ```
/// use errortools::{ManyErrors, WithContext};
/// use std::path::PathBuf;
///
/// let mut errs = ManyErrors::<PathBuf, std::io::Error>::new();
/// assert!(errs.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum ManyErrors<C, E, WithContextFormat = Colon> {
    /// No errors were recorded.
    #[default]
    None,
    /// Exactly one error was recorded.
    One(WithContext<C, E, WithContextFormat>),
    /// Two or more errors were recorded.
    Many(Vec<WithContext<C, E, WithContextFormat>>),
}

impl<C, E, WithContextFormat> ManyErrors<C, E, WithContextFormat> {
    /// Creates an empty `ManyErrors`.
    pub const fn new() -> Self {
        Self::None
    }

    /// Returns `true` if no errors have been recorded.
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns the number of recorded errors.
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::One(_) => 1,
            Self::Many(v) => v.len(),
        }
    }

    /// Appends a tagged error, promoting `None → One → Many` as needed.
    ///
    /// # Example
    /// ```
    /// use errortools::{ManyErrors, WithContext};
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push(WithContext::new("step 1", std::io::Error::other("fail")));
    /// assert_eq!(errs.len(), 1);
    /// ```
    pub fn push(&mut self, item: WithContext<C, E, WithContextFormat>) {
        let prev = core::mem::take(self);
        *self = match prev {
            Self::None => Self::One(item),
            Self::One(first) => Self::Many(vec![first, item]),
            Self::Many(mut v) => {
                v.push(item);
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

/// Renders each recorded error on its own line. Each item is rendered via its
/// own [`Display`] (and thus its own type-level strategy `WithContextFormat`), since this
/// Display impl routes through [`Listing<AsDisplay>`].
impl<C, E, WithContextFormat> Display for ManyErrors<C, E, WithContextFormat>
where
    WithContextFormat: Format<WithContext<C, E, WithContextFormat>>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        <Listing<AsDisplay> as Format<Self>>::fmt(self, f)
    }
}

/// Aggregate strategy that renders each item in a [`ManyErrors`] on its own
/// line, formatting each via the per-item strategy `G`.
///
/// The default `G = AsDisplay` defers to each item's own [`Display`] (and
/// thus its own type-level strategy `WithContextFormat`). Pass a concrete `G` (e.g.
/// [`OneLine`](crate::OneLine) or [`Tree`](crate::Tree)) to override per-item
/// rendering.
///
/// Listing is implemented for both `ManyErrors<C, E, WithContextFormat>` and
/// `&ManyErrors<C, E, WithContextFormat>` so it can be used directly inside this module's
/// `Display` and via the [`Formatted`](crate::Formatted) wrapper (which holds
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

impl<C, E, WithContextFormat> Error for ManyErrors<C, E, WithContextFormat>
where
    C: Debug,
    E: Error + 'static,
    WithContextFormat: Format<WithContext<C, E, WithContextFormat>> + Debug,
{
    /// For [`Self::One`], skips the inner error (already shown via Display) and
    /// returns its source so chain-walking strategies don't duplicate it.
    /// [`Self::Many`] has no single source — the chain ends here.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::None | Self::Many(_) => None,
            Self::One(p) => p.error.source(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, ops::ControlFlow};

    use super::*;
    use crate::{
        FormatError, OneLine, Tree,
        tests::{Inner, Mid, WcArrow},
    };

    fn w(ctx: &'static str) -> WithContext<&'static str, Inner> {
        WithContext::new(ctx, Inner::A)
    }

    // --- push / variants ---

    #[test]
    fn test_new_is_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert!(matches!(e, ManyErrors::None));
        assert!(e.is_empty());
        assert_eq!(e.len(), 0);
    }

    #[test]
    fn test_push_none_to_one() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        assert!(matches!(e, ManyErrors::One(_)));
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn test_push_one_to_many() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        e.push(w("b"));
        assert!(matches!(e, ManyErrors::Many(_)));
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn test_push_many_grows() {
        let mut e: ManyErrors<u32, Inner> = ManyErrors::new();
        for i in 0..5u32 {
            e.push(WithContext::new(i, Inner::A));
        }
        assert_eq!(e.len(), 5);
    }

    // --- into_result ---

    #[test]
    fn test_into_result_none_ok() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.into_result(42), Ok(42));
    }

    #[test]
    fn test_into_result_one_err() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        assert!(e.into_result(()).is_err());
    }

    #[test]
    fn test_into_result_many_err() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        e.push(w("b"));
        assert!(e.into_result(()).is_err());
    }

    // --- FromIterator / Extend ---

    #[test]
    fn test_collect_from_with_context() {
        let errs: ManyErrors<&str, Inner> = [w("a"), w("b"), w("c")].into_iter().collect();
        assert_eq!(errs.len(), 3);
    }

    #[test]
    fn test_collect_from_tuples() {
        let errs: ManyErrors<&str, Inner> =
            [("a", Inner::A), ("b", Inner::A)].into_iter().collect();
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_extend_from_with_context() {
        let mut e = ManyErrors::new();
        e.extend([w("a"), w("b")]);
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn test_extend_from_tuples_via_partition_result() {
        use itertools::Itertools as _;

        let results: Vec<Result<i32, (&str, Inner)>> =
            vec![Ok(1), Err(("a", Inner::A)), Ok(2), Err(("b", Inner::A))];
        let (oks, errs): (Vec<i32>, ManyErrors<&str, Inner>) =
            results.into_iter().partition_result();
        assert_eq!(oks, [1, 2]);
        assert_eq!(errs.len(), 2);
    }

    // --- ControlFlow ---

    #[test]
    fn test_control_flow_all_continue() {
        #[allow(clippy::type_complexity)]
        let items: Vec<ControlFlow<WithContext<&str, Inner>, WithContext<&str, Inner>>> =
            vec![ControlFlow::Continue(w("a")), ControlFlow::Continue(w("b"))];
        let errs: ManyErrors<&str, Inner> = items.into_iter().collect();
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_control_flow_break_stops_and_records() {
        let mut count = 0usize;
        let iter = ["a", "b", "c", "d"].iter().map(|s| {
            count += 1;
            if *s == "b" {
                ControlFlow::Break(WithContext::new(*s, Inner::A))
            } else {
                ControlFlow::Continue(WithContext::new(*s, Inner::A))
            }
        });
        let errs: ManyErrors<&str, Inner> = iter.collect();
        // "a" (continue), "b" (break) → stops; "c","d" not consumed
        assert_eq!(errs.len(), 2);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_control_flow_tuples() {
        #[allow(clippy::type_complexity)]
        let items: Vec<ControlFlow<(&str, Inner), (&str, Inner)>> = vec![
            ControlFlow::Continue(("a", Inner::A)),
            ControlFlow::Break(("b", Inner::A)),
        ];
        let errs: ManyErrors<&str, Inner> = items.into_iter().collect();
        assert_eq!(errs.len(), 2);
    }

    // --- Display + Error ---

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

    #[test]
    fn test_source_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert!(e.source().is_none());
    }

    #[test]
    fn test_source_one_skips_inner_error() {
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("ctx", Mid::Inner(Inner::A)));
        // Display already shows "ctx: mid"; source returns Mid's source (&Inner::A)
        // so chain walkers don't repeat "mid".
        let src = e.source().expect("should have source");
        assert_eq!(src.to_string(), "InnerA");
    }

    #[test]
    fn test_source_many_is_none() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        e.push(w("b"));
        assert!(e.source().is_none());
    }

    #[test]
    fn test_one_line_one_walks_chain() {
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("ctx", Mid::Inner(Inner::A)));
        assert_eq!(e.one_line().to_string(), "ctx: mid: InnerA");
    }

    // --- iter ---

    #[test]
    fn test_iter_none() {
        let e = ManyErrors::<&str, Inner>::new();
        assert_eq!(e.iter().count(), 0);
    }

    #[test]
    fn test_iter_one() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        let items: Vec<_> = e.iter().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].context, "a");
    }

    #[test]
    fn test_iter_many() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        e.push(w("b"));
        let ctxs: Vec<_> = e.iter().map(|w| w.context).collect();
        assert_eq!(ctxs, ["a", "b"]);
    }

    // --- io::Error integration ---

    #[test]
    fn test_io_errors_via_collect() {
        let paths = ["missing.txt", "also_missing.txt"];
        let errs: ManyErrors<&str, io::Error> = paths
            .iter()
            .filter_map(|p| std::fs::read(p).err().map(|e| WithContext::new(*p, e)))
            .collect();
        assert_eq!(errs.len(), 2);
    }
}
