use crate::ManyErrors;

/// Merges all top-level nodes from `rhs` into `self`.
impl<C, E, GC, F, GF> core::ops::Add for ManyErrors<C, E, GC, F, GF> {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self {
        self.extend(rhs);
        self
    }
}

/// Appends a single leaf `(context, error)`.
impl<C, E, GC, F, GF> core::ops::Add<(C, E)> for ManyErrors<C, E, GC, F, GF> {
    type Output = Self;

    fn add(mut self, (context, error): (C, E)) -> Self {
        self.push(context, error);
        self
    }
}

/// Appends `error` if `result` is `Err`; leaves `self` unchanged on `Ok`.
impl<C, E, GC, F, GF, T> core::ops::Add<(C, Result<T, E>)> for ManyErrors<C, E, GC, F, GF> {
    type Output = Self;

    fn add(mut self, (context, result): (C, Result<T, E>)) -> Self {
        if let Err(error) = result {
            self.push(context, error);
        }
        self
    }
}

/// Appends `item` if `option` is `Some`; leaves `self` unchanged on `None`.
///
/// `I` can be anything that `ManyErrors` already accepts via `Add`:
/// `(C, E)`, `(C, Result<T, E>)`, or another `ManyErrors`.
impl<C, E, GC, F, GF, I> core::ops::Add<Option<I>> for ManyErrors<C, E, GC, F, GF>
where
    ManyErrors<C, E, GC, F, GF>: core::ops::Add<I, Output = ManyErrors<C, E, GC, F, GF>>,
{
    type Output = Self;

    fn add(self, option: Option<I>) -> Self {
        match option {
            Some(item) => self + item,
            None => self,
        }
    }
}

impl<C, E, GC, F, GF> core::ops::AddAssign for ManyErrors<C, E, GC, F, GF> {
    fn add_assign(&mut self, rhs: Self) {
        self.extend(rhs);
    }
}

impl<C, E, GC, F, GF> core::ops::AddAssign<(C, E)> for ManyErrors<C, E, GC, F, GF> {
    fn add_assign(&mut self, (context, error): (C, E)) {
        self.push(context, error);
    }
}

impl<C, E, GC, F, GF, T> core::ops::AddAssign<(C, Result<T, E>)> for ManyErrors<C, E, GC, F, GF> {
    fn add_assign(&mut self, (context, result): (C, Result<T, E>)) {
        if let Err(error) = result {
            self.push(context, error);
        }
    }
}

/// Appends `item` if `option` is `Some`; leaves `self` unchanged on `None`.
impl<C, E, GC, F, GF, I> core::ops::AddAssign<Option<I>> for ManyErrors<C, E, GC, F, GF>
where
    Self: core::ops::AddAssign<I>,
{
    fn add_assign(&mut self, option: Option<I>) {
        if let Some(item) = option {
            *self += item;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::Inner;

    use super::*;

    // --- Add ---

    #[test]
    fn test_add_many_errors_merges_nodes() {
        let mut a = ManyErrors::<&str, Inner>::new();
        a.push("a", Inner::A);
        let mut b = ManyErrors::<&str, Inner>::new();
        b.push("b", Inner::B);
        b.push("c", Inner::A);
        let merged = a + b;
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_add_many_errors_with_empty() {
        let mut a = ManyErrors::<&str, Inner>::new();
        a.push("a", Inner::A);
        let merged = a + ManyErrors::new();
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_add_tuple_pushes_leaf() {
        let errs = ManyErrors::<&str, Inner>::new() + ("ctx", Inner::A);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_result_err_pushes() {
        let errs = ManyErrors::<&str, Inner>::new() + ("ctx", Err::<(), _>(Inner::A));
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_result_ok_skips() {
        let errs = ManyErrors::<&str, Inner>::new() + ("ctx", Ok::<(), _>(()));
        assert!(errs.is_empty());
    }

    #[test]
    fn test_add_result_chain() {
        let results: alloc::vec::Vec<Result<(), Inner>> =
            alloc::vec![Ok(()), Err(Inner::A), Ok(()), Err(Inner::B)];
        let errs = results
            .into_iter()
            .enumerate()
            .fold(ManyErrors::<usize, Inner>::new(), |acc, (i, r)| {
                acc + (i, r)
            });
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_add_option_tuple_some_pushes() {
        let errs = ManyErrors::<&str, Inner>::new() + Some(("ctx", Inner::A));
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_option_tuple_none_skips() {
        let errs = ManyErrors::<&str, Inner>::new() + None::<(&str, Inner)>;
        assert!(errs.is_empty());
    }

    #[test]
    fn test_add_option_result_some_err_pushes() {
        let errs = ManyErrors::<&str, Inner>::new() + Some(("ctx", Err::<(), _>(Inner::A)));
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_option_result_some_ok_skips() {
        let errs = ManyErrors::<&str, Inner>::new() + Some(("ctx", Ok::<(), Inner>(())));
        assert!(errs.is_empty());
    }

    #[test]
    fn test_add_option_many_errors_some_merges() {
        let mut other = ManyErrors::<&str, Inner>::new();
        other.push("b", Inner::B);
        let errs = ManyErrors::<&str, Inner>::new() + Some(other);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_option_many_errors_none_skips() {
        let errs = ManyErrors::<&str, Inner>::new() + None::<ManyErrors<&str, Inner>>;
        assert!(errs.is_empty());
    }

    // --- AddAssign ---

    #[test]
    fn test_add_assign_many_errors() {
        let mut a = ManyErrors::<&str, Inner>::new();
        a.push("a", Inner::A);
        let mut b = ManyErrors::<&str, Inner>::new();
        b.push("b", Inner::B);
        b.push("c", Inner::A);
        a += b;
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn test_add_assign_tuple() {
        let mut errs = ManyErrors::<&str, Inner>::new();
        errs += ("ctx", Inner::A);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_assign_result_err() {
        let mut errs = ManyErrors::<&str, Inner>::new();
        errs += ("ctx", Err::<(), _>(Inner::A));
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_assign_result_ok() {
        let mut errs = ManyErrors::<&str, Inner>::new();
        errs += ("ctx", Ok::<(), Inner>(()));
        assert!(errs.is_empty());
    }

    #[test]
    fn test_add_assign_option_some_pushes() {
        let mut errs = ManyErrors::<&str, Inner>::new();
        errs += Some(("ctx", Inner::A));
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_add_assign_option_none_skips() {
        let mut errs = ManyErrors::<&str, Inner>::new();
        errs += None::<(&str, Inner)>;
        assert!(errs.is_empty());
    }
}
