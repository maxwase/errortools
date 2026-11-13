//! Tuples as composite [`Format`] strategies.
//!
//! A tuple of strategies is itself a strategy: each element writes in order
//! against the same value, so `(OneLine, NewLine, Suggestion)` renders the
//! one-line chain, a newline, then the suggestion hint. Bounds compose
//! automatically: the tuple implements [`Format<E>`] exactly when every
//! element does.
//!
//! Every element writes unconditionally. Wrap a trailing pair in
//! [`Then`](crate::separator::Then) to drop a separator whose right side
//! turns out empty.

use core::fmt;

use crate::Format;

/// Implements [`Format<E>`] for one tuple arity; elements write left to right.
macro_rules! impl_tuple_format {
    ($($T:ident),+) => {
        impl<E: ?Sized, $($T: Format<E>),+> Format<E> for ($($T,)+) {
            fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                $($T::fmt(error, f)?;)+
                Ok(())
            }
        }
    };
}

impl_tuple_format!(A, B);
impl_tuple_format!(A, B, C);
impl_tuple_format!(A, B, C, D);
impl_tuple_format!(A, B, C, D, G);
impl_tuple_format!(A, B, C, D, G, H);

#[cfg(test)]
mod tests {
    use core::error::Error;

    use crate::{
        Chain, Format, Formatted, OneLine, Suggestion,
        separator::{ColonChar, ColonSpace, Empty, NewLine, Space},
        tests::Inner,
    };

    fn _assert_traits() {
        fn assert_all<
            T: Clone + Copy + Default + PartialEq + Eq + core::hash::Hash + Send + Sync,
        >() {
        }
        assert_all::<(OneLine, NewLine)>();
        assert_all::<(OneLine, NewLine, Suggestion)>();

        fn assert_format<E: ?Sized, F: Format<E>>() {}
        assert_format::<crate::tests::Error, (OneLine, NewLine)>();
        assert_format::<crate::tests::Error, (OneLine, Chain)>();
        assert_format::<crate::tests::Error, (OneLine, NewLine, Suggestion)>();
        assert_format::<crate::tests::Error, (OneLine, Space, OneLine, Space)>();
        assert_format::<crate::tests::Error, (OneLine, Space, OneLine, Space, OneLine)>();
        assert_format::<dyn Error, (OneLine, Space, OneLine, Space, OneLine, Empty)>();

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
            Formatted::<_, (OneLine, NewLine)>::new(error).to_string(),
            "Two: InnerA\n"
        );
    }

    #[test]
    fn test_three_tuple_oneline_newline_suggestion() {
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, NewLine, Suggestion)>::new(error).to_string(),
            "One\nTry passing --help to see available options."
        );
    }

    /// A plain tuple writes every element; only `Then` drops a separator.
    #[test]
    fn test_empty_last_element_keeps_separator() {
        let error = crate::tests::Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, (OneLine, NewLine, Suggestion)>::new(error).to_string(),
            "Two: InnerA\n"
        );
    }

    #[test]
    fn test_nested_tuple() {
        let error = crate::tests::Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, (OneLine, (NewLine, OneLine))>::new(error).to_string(),
            "Two: InnerA\nTwo: InnerA"
        );
    }

    #[test]
    fn test_six_tuple_writes_in_order() {
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, Space, OneLine, ColonChar, OneLine, ColonSpace)>::new(error)
                .to_string(),
            "One One:One: "
        );
    }

    #[test]
    fn test_debug_prints_elements() {
        assert_eq!(
            format!("{:?}", <(OneLine, Empty, Suggestion)>::default()),
            "(OneLine, Empty, Suggestion)"
        );
    }
}
