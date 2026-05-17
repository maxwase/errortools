// --- Iter ---

use crate::{ManyErrors, WithContext, with_context::Colon};

impl<C, E, WithContextFormat> ManyErrors<C, E, WithContextFormat> {
    /// Returns an iterator over references to each recorded [`WithContext`].
    pub fn iter(&self) -> Iter<'_, C, E, WithContextFormat> {
        Iter::new(self)
    }
}

/// Iterator over references to each [`WithContext`] in a [`ManyErrors`].
pub struct Iter<'a, C, E, WithContextFormat = Colon>(IterInner<'a, C, E, WithContextFormat>);

enum IterInner<'a, C, E, WithContextFormat> {
    Empty,
    One(Option<&'a WithContext<C, E, WithContextFormat>>),
    Many(core::slice::Iter<'a, WithContext<C, E, WithContextFormat>>),
}

impl<'a, C, E, WithContextFormat> Iter<'a, C, E, WithContextFormat> {
    fn new(many: &'a ManyErrors<C, E, WithContextFormat>) -> Self {
        Self(match many {
            ManyErrors::None => IterInner::Empty,
            ManyErrors::One(w) => IterInner::One(Some(w)),
            ManyErrors::Many(v) => IterInner::Many(v.iter()),
        })
    }
}

impl<'a, C, E, WithContextFormat> Iterator for Iter<'a, C, E, WithContextFormat> {
    type Item = &'a WithContext<C, E, WithContextFormat>;

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

mod from_iter {
    use core::ops::ControlFlow;

    use super::*;

    impl<C, E, WithContextFormat> FromIterator<WithContext<C, E, WithContextFormat>>
        for ManyErrors<C, E, WithContextFormat>
    {
        fn from_iter<I: IntoIterator<Item = WithContext<C, E, WithContextFormat>>>(
            iter: I,
        ) -> Self {
            let mut me = Self::None;
            me.extend(iter);
            me
        }
    }

    impl<C, E, WithContextFormat> FromIterator<(C, E)> for ManyErrors<C, E, WithContextFormat> {
        fn from_iter<I: IntoIterator<Item = (C, E)>>(iter: I) -> Self {
            iter.into_iter().map(WithContext::from).collect()
        }
    }

    impl<C, E, WithContextFormat>
        FromIterator<
            ControlFlow<WithContext<C, E, WithContextFormat>, WithContext<C, E, WithContextFormat>>,
        > for ManyErrors<C, E, WithContextFormat>
    {
        fn from_iter<I>(iter: I) -> Self
        where
            I: IntoIterator<
                Item = ControlFlow<
                    WithContext<C, E, WithContextFormat>,
                    WithContext<C, E, WithContextFormat>,
                >,
            >,
        {
            let mut me = Self::None;
            me.extend(iter);
            me
        }
    }

    impl<C, E, WithContextFormat> FromIterator<ControlFlow<(C, E), (C, E)>>
        for ManyErrors<C, E, WithContextFormat>
    {
        fn from_iter<I: IntoIterator<Item = ControlFlow<(C, E), (C, E)>>>(iter: I) -> Self {
            let mut me = Self::None;
            me.extend(iter);
            me
        }
    }

    // --- Extend ---

    impl<C, E, WithContextFormat> Extend<WithContext<C, E, WithContextFormat>>
        for ManyErrors<C, E, WithContextFormat>
    {
        fn extend<I: IntoIterator<Item = WithContext<C, E, WithContextFormat>>>(
            &mut self,
            iter: I,
        ) {
            // TODO: Optimize
            for item in iter {
                self.push(item);
            }
        }
    }

    impl<C, E, WithContextFormat> Extend<(C, E)> for ManyErrors<C, E, WithContextFormat> {
        fn extend<I: IntoIterator<Item = (C, E)>>(&mut self, iter: I) {
            self.extend(iter.into_iter().map(WithContext::from));
        }
    }

    /// `Continue(w)` records `w` and keeps iterating; `Break(w)` records `w` and stops.
    impl<C, E, WithContextFormat>
        Extend<
            ControlFlow<WithContext<C, E, WithContextFormat>, WithContext<C, E, WithContextFormat>>,
        > for ManyErrors<C, E, WithContextFormat>
    {
        fn extend<I>(&mut self, iter: I)
        where
            I: IntoIterator<
                Item = ControlFlow<
                    WithContext<C, E, WithContextFormat>,
                    WithContext<C, E, WithContextFormat>,
                >,
            >,
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

    impl<C, E, WithContextFormat> Extend<ControlFlow<(C, E), (C, E)>>
        for ManyErrors<C, E, WithContextFormat>
    {
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
}

#[cfg(test)]
mod tests {
    use crate::{ManyErrors, WithContext, tests::Inner};
    use itertools::Itertools as _;
    use std::{io, ops::ControlFlow};

    fn w(ctx: &'static str) -> WithContext<&'static str, Inner> {
        WithContext::new(ctx, Inner::A)
    }

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
        let results: Vec<Result<i32, (&str, Inner)>> =
            vec![Ok(1), Err(("a", Inner::A)), Ok(2), Err(("b", Inner::A))];
        let (oks, errs): (Vec<i32>, ManyErrors<&str, Inner>) =
            results.into_iter().partition_result();
        assert_eq!(oks, [1, 2]);
        assert_eq!(errs.len(), 2);
    }

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
