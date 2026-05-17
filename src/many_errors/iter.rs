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
    use std::ops::ControlFlow;

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
