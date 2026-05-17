//! Aggregated, context-tagged errors from iterator/fold-style operations.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
};

use alloc::{vec, vec::Vec};

use crate::{
    AsDisplay, Format,
    with_context::{Colon, WithContext},
};

mod iter;
mod listing;

pub use listing::Listing;

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
    use super::*;
    use crate::{
        FormatError,
        tests::{Inner, Mid},
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

    // --- Display + Error ---

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
}
