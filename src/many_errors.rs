//! Aggregated, context-tagged errors from iterator/fold-style operations.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
    ops::ControlFlow,
};

use alloc::{vec, vec::Vec};

use crate::{Format, with_context::WithContext};

/// Zero or more context-tagged errors collected during an iterator/fold operation.
///
/// The three-variant split lets consumers pattern-match and render each case
/// differently — for example, printing a single error with its full chain or
/// listing all errors line-by-line with [`ManyErrors::list`].
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
pub enum ManyErrors<C, E> {
    /// No errors were recorded.
    #[default]
    None,
    /// Exactly one error was recorded.
    One(WithContext<C, E>),
    /// Two or more errors were recorded.
    Many(Vec<WithContext<C, E>>),
}

impl<C, E> ManyErrors<C, E> {
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
    pub fn push(&mut self, item: WithContext<C, E>) {
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

    /// Returns an iterator over references to each recorded [`WithContext`].
    pub fn iter(&self) -> Iter<'_, C, E> {
        Iter::new(self)
    }

    /// Returns a [`Display`](fmt::Display) adapter that renders each error on its
    /// own line using format strategy `F`.
    ///
    /// # Example
    /// ```
    /// use errortools::{ManyErrors, OneLine, WithContext};
    ///
    /// let mut errs = ManyErrors::<&str, std::io::Error>::new();
    /// errs.push(WithContext::new("a", std::io::Error::other("err a")));
    /// errs.push(WithContext::new("b", std::io::Error::other("err b")));
    /// let output = errs.list::<OneLine>().to_string();
    /// assert!(output.contains("a: err a"));
    /// assert!(output.contains("b: err b"));
    /// ```
    pub fn list<F: Format<WithContext<C, E>>>(&self) -> Listing<'_, C, E, F>
    where
        C: Display + Debug,
        E: Error + 'static,
    {
        Listing(self, PhantomData)
    }
}

// --- FromIterator ---

impl<C, E> FromIterator<WithContext<C, E>> for ManyErrors<C, E> {
    fn from_iter<I: IntoIterator<Item = WithContext<C, E>>>(iter: I) -> Self {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

impl<C, E> FromIterator<(C, E)> for ManyErrors<C, E> {
    fn from_iter<I: IntoIterator<Item = (C, E)>>(iter: I) -> Self {
        iter.into_iter().map(WithContext::from).collect()
    }
}

impl<C, E> FromIterator<ControlFlow<WithContext<C, E>, WithContext<C, E>>> for ManyErrors<C, E> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = ControlFlow<WithContext<C, E>, WithContext<C, E>>>,
    {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

impl<C, E> FromIterator<ControlFlow<(C, E), (C, E)>> for ManyErrors<C, E> {
    fn from_iter<I: IntoIterator<Item = ControlFlow<(C, E), (C, E)>>>(iter: I) -> Self {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

// --- Extend ---

impl<C, E> Extend<WithContext<C, E>> for ManyErrors<C, E> {
    fn extend<I: IntoIterator<Item = WithContext<C, E>>>(&mut self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }
}

impl<C, E> Extend<(C, E)> for ManyErrors<C, E> {
    fn extend<I: IntoIterator<Item = (C, E)>>(&mut self, iter: I) {
        self.extend(iter.into_iter().map(WithContext::from));
    }
}

/// `Continue(w)` records `w` and keeps iterating; `Break(w)` records `w` and stops.
impl<C, E> Extend<ControlFlow<WithContext<C, E>, WithContext<C, E>>> for ManyErrors<C, E> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ControlFlow<WithContext<C, E>, WithContext<C, E>>>,
    {
        for cf in iter {
            let stop = matches!(cf, ControlFlow::Break(_));
            let w = match cf {
                ControlFlow::Continue(w) | ControlFlow::Break(w) => w,
            };
            self.push(w);
            if stop {
                break;
            }
        }
    }
}

impl<C, E> Extend<ControlFlow<(C, E), (C, E)>> for ManyErrors<C, E> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ControlFlow<(C, E), (C, E)>>,
    {
        self.extend(iter.into_iter().map(|cf| match cf {
            ControlFlow::Continue(t) => ControlFlow::Continue(WithContext::from(t)),
            ControlFlow::Break(t) => ControlFlow::Break(WithContext::from(t)),
        }));
    }
}

// --- Display + Error ---

impl<C: Display, E: Display> Display for ManyErrors<C, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::One(p) => Display::fmt(&p.context, f),
            Self::Many(v) => write!(f, "{} errors", v.len()),
        }
    }
}

impl<C, E> Error for ManyErrors<C, E>
where
    C: Display + Debug,
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::None => None,
            // Skip the WithContext wrapper to avoid repeating the context in the chain.
            Self::One(p) => Some(&p.error),
            Self::Many(_) => None,
        }
    }
}

// --- Iter ---

/// Iterator over references to each [`WithContext`] in a [`ManyErrors`].
pub struct Iter<'a, C, E>(IterInner<'a, C, E>);

enum IterInner<'a, C, E> {
    Empty,
    One(Option<&'a WithContext<C, E>>),
    Many(core::slice::Iter<'a, WithContext<C, E>>),
}

impl<'a, C, E> Iter<'a, C, E> {
    fn new(many: &'a ManyErrors<C, E>) -> Self {
        Self(match many {
            ManyErrors::None => IterInner::Empty,
            ManyErrors::One(w) => IterInner::One(Some(w)),
            ManyErrors::Many(v) => IterInner::Many(v.iter()),
        })
    }
}

impl<'a, C, E> Iterator for Iter<'a, C, E> {
    type Item = &'a WithContext<C, E>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IterInner::Empty => None,
            IterInner::One(slot) => slot.take(),
            IterInner::Many(it) => it.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            IterInner::Empty => (0, Some(0)),
            IterInner::One(slot) => {
                let n = slot.is_some() as usize;
                (n, Some(n))
            }
            IterInner::Many(it) => it.size_hint(),
        }
    }
}

// --- Listing ---

/// Renders all errors in a [`ManyErrors`], one per line, each via strategy `F`.
///
/// Obtained from [`ManyErrors::list`].
pub struct Listing<'a, C, E, F = crate::OneLine>(&'a ManyErrors<C, E>, PhantomData<fn() -> F>);

impl<C, E, F> Display for Listing<'_, C, E, F>
where
    C: Display + Debug,
    E: Error + 'static,
    F: Format<WithContext<C, E>>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut it = self.0.iter();
        let Some(first) = it.next() else {
            return Ok(());
        };
        F::fmt(first, f)?;
        for p in it {
            writeln!(f)?;
            F::fmt(p, f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use thiserror::Error;

    use super::*;
    use crate::{FormatError, OneLine, Tree};

    #[derive(Error, Debug, Clone, PartialEq, Eq)]
    #[error("leaf")]
    struct Leaf;

    #[derive(Error, Debug, Clone, PartialEq, Eq)]
    #[error("mid")]
    struct Mid(#[source] Leaf);

    fn w(ctx: &'static str) -> WithContext<&'static str, Leaf> {
        WithContext::new(ctx, Leaf)
    }

    // --- push / variants ---

    #[test]
    fn test_new_is_none() {
        let e = ManyErrors::<&str, Leaf>::new();
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
        let mut e = ManyErrors::new();
        for i in 0..5u32 {
            e.push(WithContext::new(i, Leaf));
        }
        assert_eq!(e.len(), 5);
    }

    // --- into_result ---

    #[test]
    fn test_into_result_none_ok() {
        let e = ManyErrors::<&str, Leaf>::new();
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
        let errs: ManyErrors<&str, Leaf> = [w("a"), w("b"), w("c")].into_iter().collect();
        assert_eq!(errs.len(), 3);
    }

    #[test]
    fn test_collect_from_tuples() {
        let errs: ManyErrors<&str, Leaf> = [("a", Leaf), ("b", Leaf)].into_iter().collect();
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

        let results: Vec<Result<i32, (&str, Leaf)>> =
            vec![Ok(1), Err(("a", Leaf)), Ok(2), Err(("b", Leaf))];
        let (oks, errs): (Vec<i32>, ManyErrors<&str, Leaf>) =
            results.into_iter().partition_result();
        assert_eq!(oks, [1, 2]);
        assert_eq!(errs.len(), 2);
    }

    // --- ControlFlow ---

    #[test]
    fn test_control_flow_all_continue() {
        #[allow(clippy::type_complexity)]
        let items: Vec<ControlFlow<WithContext<&str, Leaf>, WithContext<&str, Leaf>>> =
            vec![ControlFlow::Continue(w("a")), ControlFlow::Continue(w("b"))];
        let errs: ManyErrors<&str, Leaf> = items.into_iter().collect();
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_control_flow_break_stops_and_records() {
        let mut count = 0usize;
        let iter = ["a", "b", "c", "d"].iter().map(|s| {
            count += 1;
            if *s == "b" {
                ControlFlow::Break(WithContext::new(*s, Leaf))
            } else {
                ControlFlow::Continue(WithContext::new(*s, Leaf))
            }
        });
        let errs: ManyErrors<&str, Leaf> = iter.collect();
        // "a" (continue), "b" (break) → stops; "c","d" not consumed
        assert_eq!(errs.len(), 2);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_control_flow_tuples() {
        #[allow(clippy::type_complexity)]
        let items: Vec<ControlFlow<(&str, Leaf), (&str, Leaf)>> = vec![
            ControlFlow::Continue(("a", Leaf)),
            ControlFlow::Break(("b", Leaf)),
        ];
        let errs: ManyErrors<&str, Leaf> = items.into_iter().collect();
        assert_eq!(errs.len(), 2);
    }

    // --- Display + Error ---

    #[test]
    fn test_display_none_is_empty() {
        let e = ManyErrors::<&str, Leaf>::new();
        assert_eq!(e.to_string(), "");
    }

    #[test]
    fn test_display_one_writes_context() {
        let mut e = ManyErrors::new();
        e.push(w("step 1"));
        assert_eq!(e.to_string(), "step 1");
    }

    #[test]
    fn test_display_many_writes_count() {
        let mut e = ManyErrors::new();
        e.push(w("a"));
        e.push(w("b"));
        e.push(w("c"));
        assert_eq!(e.to_string(), "3 errors");
    }

    #[test]
    fn test_source_none() {
        let e = ManyErrors::<&str, Leaf>::new();
        assert!(e.source().is_none());
    }

    #[test]
    fn test_source_one_skips_wrapper() {
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("ctx", Mid(Leaf)));
        // source should be &Mid, not &WithContext
        let src = e.source().expect("should have source");
        assert_eq!(src.to_string(), "mid");
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
        e.push(WithContext::new("ctx", Mid(Leaf)));
        assert_eq!(e.one_line().to_string(), "ctx: mid: leaf");
    }

    // --- iter ---

    #[test]
    fn test_iter_none() {
        let e = ManyErrors::<&str, Leaf>::new();
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

    // --- Listing ---

    #[test]
    fn test_listing_none_empty() {
        let e = ManyErrors::<&str, Leaf>::new();
        assert_eq!(e.list::<OneLine>().to_string(), "");
    }

    #[test]
    fn test_listing_one_formats_chain() {
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("ctx", Mid(Leaf)));
        assert_eq!(e.list::<OneLine>().to_string(), "ctx: mid: leaf");
    }

    #[test]
    fn test_listing_many_one_per_line() {
        let errs: ManyErrors<&str, Leaf> = [("a", Leaf), ("b", Leaf), ("c", Leaf)]
            .into_iter()
            .collect();
        let out = errs.list::<OneLine>().to_string();
        assert_eq!(out, "a: leaf\nb: leaf\nc: leaf");
    }

    #[test]
    fn test_listing_tree_strategy() {
        let mut e: ManyErrors<&str, Mid> = ManyErrors::new();
        e.push(WithContext::new("ctx", Mid(Leaf)));
        let out = e.list::<Tree>().to_string();
        assert!(out.contains("ctx"));
        assert!(out.contains("mid"));
        assert!(out.contains("leaf"));
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
