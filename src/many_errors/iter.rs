// --- Iter ---

use core::ops::ControlFlow;

use crate::{
    ManyErrors,
    with_context::{Colon, ContextField, WithContext},
};

use super::Node;

impl<C, E, GC, F, GF> ManyErrors<C, E, GC, F, GF> {
    /// Returns an iterator over references to each direct [`Node`] child.
    pub fn iter(&self) -> Iter<'_, C, E, GC, F, GF> {
        Iter::new(self)
    }
}

impl<'a, C, E, GC, F, GF> IntoIterator for &'a ManyErrors<C, E, GC, F, GF> {
    type Item = &'a Node<C, E, GC, F, GF>;
    type IntoIter = Iter<'a, C, E, GC, F, GF>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over references to the direct [`Node`] children of a [`ManyErrors`].
pub struct Iter<'a, C, E, GC = C, F = Colon, GF = ContextField>(IterInner<'a, C, E, GC, F, GF>);

enum IterInner<'a, C, E, GC, F, GF> {
    Empty,
    One(Option<&'a Node<C, E, GC, F, GF>>),
    Many(core::slice::Iter<'a, Node<C, E, GC, F, GF>>),
}

impl<'a, C, E, GC, F, GF> Iter<'a, C, E, GC, F, GF> {
    fn new(many: &'a ManyErrors<C, E, GC, F, GF>) -> Self {
        Self(match many {
            ManyErrors::None => IterInner::Empty,
            ManyErrors::One(n) => IterInner::One(Some(n)),
            ManyErrors::Many(v) => IterInner::Many(v.iter()),
        })
    }
}

impl<'a, C, E, GC, F, GF> Iterator for Iter<'a, C, E, GC, F, GF> {
    type Item = &'a Node<C, E, GC, F, GF>;

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

// --- IterMut ---

impl<C, E, GC, F, GF> ManyErrors<C, E, GC, F, GF> {
    /// Returns an iterator over mutable references to each direct [`Node`] child.
    pub fn iter_mut(&mut self) -> IterMut<'_, C, E, GC, F, GF> {
        IterMut::new(self)
    }
}

impl<'a, C, E, GC, F, GF> IntoIterator for &'a mut ManyErrors<C, E, GC, F, GF> {
    type Item = &'a mut Node<C, E, GC, F, GF>;
    type IntoIter = IterMut<'a, C, E, GC, F, GF>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// Iterator over mutable references to the direct [`Node`] children of a [`ManyErrors`].
pub struct IterMut<'a, C, E, GC = C, F = Colon, GF = ContextField>(
    IterMutInner<'a, C, E, GC, F, GF>,
);

enum IterMutInner<'a, C, E, GC, F, GF> {
    Empty,
    One(Option<&'a mut Node<C, E, GC, F, GF>>),
    Many(core::slice::IterMut<'a, Node<C, E, GC, F, GF>>),
}

impl<'a, C, E, GC, F, GF> IterMut<'a, C, E, GC, F, GF> {
    fn new(many: &'a mut ManyErrors<C, E, GC, F, GF>) -> Self {
        Self(match many {
            ManyErrors::None => IterMutInner::Empty,
            ManyErrors::One(n) => IterMutInner::One(Some(n)),
            ManyErrors::Many(v) => IterMutInner::Many(v.iter_mut()),
        })
    }
}

impl<'a, C, E, GC, F, GF> Iterator for IterMut<'a, C, E, GC, F, GF> {
    type Item = &'a mut Node<C, E, GC, F, GF>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IterMutInner::Empty => None,
            IterMutInner::One(slot) => slot.take(),
            IterMutInner::Many(it) => it.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            IterMutInner::Empty => (0, Some(0)),
            IterMutInner::One(slot) => {
                let n = slot.is_some() as usize;
                (n, Some(n))
            }
            IterMutInner::Many(it) => it.size_hint(),
        }
    }
}

// --- IntoIter (owned) ---

impl<C, E, GC, F, GF> IntoIterator for ManyErrors<C, E, GC, F, GF> {
    type Item = Node<C, E, GC, F, GF>;
    type IntoIter = IntoIter<C, E, GC, F, GF>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(match self {
            ManyErrors::None => IntoIterInner::Empty,
            ManyErrors::One(n) => IntoIterInner::One(Some(n)),
            ManyErrors::Many(v) => IntoIterInner::Many(v.into_iter()),
        })
    }
}

/// Owning iterator over the direct [`Node`] children of a [`ManyErrors`],
/// produced by `into_iter` (moves each child out).
pub struct IntoIter<C, E, GC = C, F = Colon, GF = ContextField>(IntoIterInner<C, E, GC, F, GF>);

enum IntoIterInner<C, E, GC, F, GF> {
    Empty,
    One(Option<Node<C, E, GC, F, GF>>),
    Many(alloc::vec::IntoIter<Node<C, E, GC, F, GF>>),
}

impl<C, E, GC, F, GF> Iterator for IntoIter<C, E, GC, F, GF> {
    type Item = Node<C, E, GC, F, GF>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IntoIterInner::Empty => None,
            IntoIterInner::One(slot) => slot.take(),
            IntoIterInner::Many(it) => it.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            IntoIterInner::Empty => (0, Some(0)),
            IntoIterInner::One(slot) => {
                let n = slot.is_some() as usize;
                (n, Some(n))
            }
            IntoIterInner::Many(it) => it.size_hint(),
        }
    }
}

// --- FromIterator / Extend ---

impl<C, E, GC, F, GF> FromIterator<WithContext<C, E, F>> for ManyErrors<C, E, GC, F, GF> {
    fn from_iter<I: IntoIterator<Item = WithContext<C, E, F>>>(iter: I) -> Self {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

impl<C, E, GC, F, GF> FromIterator<(C, E)> for ManyErrors<C, E, GC, F, GF> {
    fn from_iter<I: IntoIterator<Item = (C, E)>>(iter: I) -> Self {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

impl<C, E, GC, F, GF> FromIterator<ControlFlow<WithContext<C, E, F>, WithContext<C, E, F>>>
    for ManyErrors<C, E, GC, F, GF>
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = ControlFlow<WithContext<C, E, F>, WithContext<C, E, F>>>,
    {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

impl<C, E, GC, F, GF> FromIterator<ControlFlow<(C, E), (C, E)>> for ManyErrors<C, E, GC, F, GF> {
    fn from_iter<I: IntoIterator<Item = ControlFlow<(C, E), (C, E)>>>(iter: I) -> Self {
        let mut me = Self::None;
        me.extend(iter);
        me
    }
}

// --- Extend ---

impl<C, E, GC, F, GF> Extend<WithContext<C, E, F>> for ManyErrors<C, E, GC, F, GF> {
    fn extend<I: IntoIterator<Item = WithContext<C, E, F>>>(&mut self, iter: I) {
        for item in iter {
            self.push_node(Node::Leaf(item));
        }
    }
}

impl<C, E, GC, F, GF> Extend<(C, E)> for ManyErrors<C, E, GC, F, GF> {
    fn extend<I: IntoIterator<Item = (C, E)>>(&mut self, iter: I) {
        for (context, error) in iter {
            self.push(context, error);
        }
    }
}

/// `Continue(w)` records `w` and keeps iterating; `Break(w)` records `w` and stops.
impl<C, E, GC, F, GF> Extend<ControlFlow<WithContext<C, E, F>, WithContext<C, E, F>>>
    for ManyErrors<C, E, GC, F, GF>
{
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ControlFlow<WithContext<C, E, F>, WithContext<C, E, F>>>,
    {
        for cf in iter {
            let stop = matches!(cf, ControlFlow::Break(_));
            let w = match cf {
                ControlFlow::Continue(w) | ControlFlow::Break(w) => w,
            };
            self.push_node(Node::Leaf(w));
            if stop {
                break;
            }
        }
    }
}

impl<C, E, GC, F, GF> Extend<ControlFlow<(C, E), (C, E)>> for ManyErrors<C, E, GC, F, GF> {
    fn extend<I: IntoIterator<Item = ControlFlow<(C, E), (C, E)>>>(&mut self, iter: I) {
        for cf in iter {
            let stop = matches!(cf, ControlFlow::Break(_));
            let (context, error) = match cf {
                ControlFlow::Continue(t) | ControlFlow::Break(t) => t,
            };
            self.push(context, error);
            if stop {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ManyErrors, Node, WithContext, tests::Inner};
    use itertools::Itertools as _;
    use std::{io, ops::ControlFlow};

    #[test]
    fn test_collect_from_with_context() {
        let wcs = [
            WithContext::<_, _, _>::new("a", Inner::A),
            WithContext::new("b", Inner::A),
            WithContext::new("c", Inner::A),
        ];
        let errs: ManyErrors<&str, Inner> = wcs.into_iter().collect();
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
        let mut e = ManyErrors::<&str, Inner>::new();
        e.extend([
            WithContext::new("a", Inner::A),
            WithContext::new("b", Inner::A),
        ]);
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn test_extend_from_tuples_via_partition_result() {
        let results: alloc::vec::Vec<Result<i32, (&str, Inner)>> =
            alloc::vec![Ok(1), Err(("a", Inner::A)), Ok(2), Err(("b", Inner::A))];
        let (oks, errs): (alloc::vec::Vec<i32>, ManyErrors<&str, Inner>) =
            results.into_iter().partition_result();
        assert_eq!(oks, [1, 2]);
        assert_eq!(errs.len(), 2);
    }

    type WcFlow = ControlFlow<WithContext<&'static str, Inner>, WithContext<&'static str, Inner>>;
    type TupleFlow = ControlFlow<(&'static str, Inner), (&'static str, Inner)>;

    #[test]
    fn test_control_flow_all_continue() {
        let items: alloc::vec::Vec<WcFlow> = alloc::vec![
            ControlFlow::Continue(WithContext::new("a", Inner::A)),
            ControlFlow::Continue(WithContext::new("b", Inner::A)),
        ];
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
        let items: alloc::vec::Vec<TupleFlow> = alloc::vec![
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
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        let items: alloc::vec::Vec<_> = e.iter().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].as_leaf().unwrap().context, "a");
    }

    #[test]
    fn test_iter_many() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::A);
        let ctxs: alloc::vec::Vec<_> = e.iter().map(|n| n.as_leaf().unwrap().context).collect();
        assert_eq!(ctxs, ["a", "b"]);
    }

    #[test]
    fn test_into_iter_ref() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::A);
        let mut ctxs = alloc::vec::Vec::new();
        for n in &e {
            ctxs.push(n.as_leaf().unwrap().context);
        }
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

    #[test]
    fn test_into_iter_owned() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::B);
        // Moves each node out — no borrow of `e` afterwards.
        let ctxs: alloc::vec::Vec<_> = e
            .into_iter()
            .map(|n| n.as_leaf().unwrap().context)
            .collect();
        assert_eq!(ctxs, ["a", "b"]);
    }

    #[test]
    fn test_into_iter_owned_one_and_none() {
        let one = {
            let mut e = ManyErrors::<&str, Inner>::new();
            e.push("solo", Inner::A);
            e
        };
        assert_eq!(one.into_iter().count(), 1);

        let none = ManyErrors::<&str, Inner>::new();
        assert_eq!(none.into_iter().count(), 0);
    }

    #[test]
    fn test_iter_mut_mutates_in_place() {
        let mut e = ManyErrors::<&str, Inner>::new();
        e.push("a", Inner::A);
        e.push("b", Inner::A);

        for node in &mut e {
            if let Node::Leaf(w) = node {
                w.context = "patched";
            }
        }

        let ctxs: alloc::vec::Vec<_> = e.iter().map(|n| n.as_leaf().unwrap().context).collect();
        assert_eq!(ctxs, ["patched", "patched"]);
    }
}
