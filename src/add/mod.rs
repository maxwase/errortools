use core::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use crate::Format;

/// Combines two [`Format`] strategies, rendering `L` then `R` against the same value.
///
/// `Add` is a type-level combinator: both strategies are tag types, never
/// instantiated. The combined strategy implements [`Format<E>`] when both
/// `L` and `R` do. Bounds compose automatically, so `Add<OneLine, Suggestion>`
/// requires `E: Suggest` because [`Suggestion`](crate::Suggestion) does.
///
/// There is no built-in separator. Use [`NewLine`](separator::NewLine) or
/// [`Space`](separator::Space) (or any custom [`Format`] tag) as the middle term:
///
/// ```text
/// Add<Add<OneLine, NewLine>, Suggestion>
/// ```
///
/// renders the one-line chain, a newline, then the top-level suggestion hint.
///
/// `Add` writes both sides unconditionally — if `R` produces no output (e.g.
/// a [`Suggestion`](crate::Suggestion) variant without a hint), the separator
/// is still written.
pub struct Add<L, R>(PhantomData<fn() -> (L, R)>);

// Manual impls so the phantom strategies `L`/`R` get no `Trait` bound from
// derives (the `_format`-style doctrine; see `WithContext`).
impl<L, R> Default for Add<L, R> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
impl<L, R> Clone for Add<L, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<L, R> Copy for Add<L, R> {}
impl<L, R> PartialEq for Add<L, R> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl<L, R> Eq for Add<L, R> {}
impl<L, R> Hash for Add<L, R> {
    fn hash<H: Hasher>(&self, _: &mut H) {}
}

/// Prints the inner strategy values (instantiated via [`Default`]) instead of
/// `Add(PhantomData)`.
impl<L: fmt::Debug + Default, R: fmt::Debug + Default> fmt::Debug for Add<L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Add")
            .field(&L::default())
            .field(&R::default())
            .finish()
    }
}

impl<E, L, R> Format<E> for Add<L, R>
where
    E: ?Sized,
    L: Format<E>,
    R: Format<E>,
{
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        L::fmt(error, f)?;
        R::fmt(error, f)
    }
}

pub mod separator;

#[cfg(test)]
mod tests {
    use core::error::Error;

    use super::*;
    use crate::{Chain, Formatted, OneLine, Suggestion, tests::Inner};
    use separator::*;

    fn _assert_traits() {
        fn assert_all<
            T: Clone + Copy + Default + PartialEq + Eq + core::hash::Hash + Send + Sync,
        >() {
        }
        assert_all::<Add<OneLine, separator::NewLine>>();
        assert_all::<Add<Add<OneLine, separator::NewLine>, Suggestion>>();
        assert_all::<separator::NewLine>();
        assert_all::<separator::Space>();

        fn assert_format<E: ?Sized, F: Format<E>>() {}
        assert_format::<crate::tests::Error, Add<OneLine, separator::NewLine>>();
        assert_format::<crate::tests::Error, Add<OneLine, Chain>>();
        assert_format::<crate::tests::Error, Add<Add<OneLine, separator::NewLine>, Suggestion>>();

        // Confirm Error bound still gates the leaf strategy, just not the trait.
        fn assert_oneline<E: Error + ?Sized>()
        where
            OneLine: Format<E>,
        {
        }
        assert_oneline::<crate::tests::Error>();
    }

    #[test]
    fn test_one_line_plus_newline() {
        let error = crate::tests::Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Add<OneLine, NewLine>>::new(error).to_string(),
            "Two: InnerA\n"
        );
    }

    #[test]
    fn test_nested_oneline_newline_suggestion() {
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, Add<Add<OneLine, NewLine>, Suggestion>>::new(error).to_string(),
            "One\nTry passing --help to see available options."
        );
    }

    #[test]
    fn test_empty_rhs_keeps_separator() {
        let error = crate::tests::Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Add<Add<OneLine, NewLine>, Suggestion>>::new(error).to_string(),
            "Two: InnerA\n"
        );
    }

    #[test]
    fn test_right_associated_nesting() {
        let error = crate::tests::Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Add<OneLine, Add<NewLine, OneLine>>>::new(error).to_string(),
            "Two: InnerA\nTwo: InnerA"
        );
    }

    #[test]
    fn test_debug_prints_inner() {
        let add = Add::<OneLine, separator::NewLine>::default();
        assert_eq!(format!("{add:?}"), "Add(OneLine, NewLine)");
    }
}
