use core::{fmt, marker::PhantomData};

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
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Add<L, R>(PhantomData<fn() -> (L, R)>);

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

pub mod separator {
    //! Separator strategies for [`Add`].
    //!
    //! Each is a [`Format`] tag that ignores its input and writes a fixed
    //! string. Because [`Format<E>`] no longer requires `E: Error`, these
    //! separators also compose with non-error formatters (e.g. the field
    //! extractors used by [`WithContext`](crate::WithContext)).
    //!
    //! This may be worth extending and separating into its own crate in
    //! future, but for now it just has a few simple built-in separators.

    use crate::{Add, Format};

    use core::fmt;

    /// [`Format`] strategy that writes a single line feed and ignores the input.
    ///
    /// Designed as a separator term inside [`Add`], e.g.
    /// `Add<OneLine, Add<NewLine, Tree>>`.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct NewLine;

    impl<E: ?Sized> Format<E> for NewLine {
        fn fmt(_: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("\n")
        }
    }

    /// [`Format`] strategy that writes a single space and ignores the input.
    ///
    /// Designed as a separator term inside [`Add`].
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct Space;

    impl<E: ?Sized> Format<E> for Space {
        fn fmt(_: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(" ")
        }
    }

    /// [`Format`] strategy that writes nothing.
    ///
    /// Useful as a no-op identity element when composing strategies with [`Add`].
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct Empty;

    impl<E: ?Sized> Format<E> for Empty {
        fn fmt(_: &E, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            Ok(())
        }
    }

    /// [`Format`] strategy that writes a colon (`":"`) and ignores the input.
    ///
    /// Pair with [`Space`] via [`ColonSpace`] for the common `": "` separator.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct Colon;

    impl<E: ?Sized> Format<E> for Colon {
        fn fmt(_: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(":")
        }
    }

    /// Convenience alias for `Add<Colon, Space>` — writes `": "`.
    pub type ColonSpace = Add<Colon, Space>;

    /// [`Add`] with an explicit separator slot: writes `L`, then `Sep`, then `R`.
    ///
    /// Equivalent to `Add<Add<L, Sep>, R>` — a thin convenience over manual
    /// nesting. Pair with the separators in this module:
    /// [`WithSpace<L, R>`](WithSpace) is `WithSep<L, Space, R>`,
    /// [`WithNewLine<L, R>`](WithNewLine) is `WithSep<L, NewLine, R>`,
    /// [`WithColonSpace<L, R>`](WithColonSpace) is `WithSep<L, ColonSpace, R>`.
    pub type WithSep<L, Sep, R> = Add<Add<L, Sep>, R>;

    /// `Add` of `L` and `R` with a [`Space`] between — equivalent to
    /// [`WithSep<L, Space, R>`](WithSep).
    pub type WithSpace<L, R> = WithSep<L, Space, R>;

    /// `Add` of `L` and `R` with a [`NewLine`] between — equivalent to
    /// [`WithSep<L, NewLine, R>`](WithSep).
    pub type WithNewLine<L, R> = WithSep<L, NewLine, R>;

    /// `Add` of `L` and `R` with [`ColonSpace`] between — equivalent to
    /// [`WithSep<L, ColonSpace, R>`](WithSep).
    pub type WithColonSpace<L, R> = WithSep<L, ColonSpace, R>;
}

#[cfg(test)]
mod tests {
    use core::error::Error;

    use thiserror::Error;

    use super::*;
    use crate::{Formatted, OneLine, Suggest, Suggestion, Tree, tests::ErrorInner};
    use separator::*;

    #[derive(Error, Debug)]
    enum SugError {
        #[error("env file missing")]
        NoEnv,
        #[error("something else")]
        Other,
    }

    impl Suggest for SugError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::NoEnv => f.write_str("Did you mean rename the .env.example file to .env?"),
                Self::Other => Ok(()),
            }
        }
    }

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
        assert_format::<crate::tests::Error, Add<OneLine, Tree>>();
        assert_format::<SugError, Add<Add<OneLine, separator::NewLine>, Suggestion>>();

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
        let error = crate::tests::Error::Two(ErrorInner::One);
        assert_eq!(
            Formatted::<_, Add<OneLine, NewLine>>::new(error).to_string(),
            "Two: One\n"
        );
    }

    #[test]
    fn test_nested_oneline_newline_suggestion() {
        let error = SugError::NoEnv;
        assert_eq!(
            Formatted::<_, Add<Add<OneLine, NewLine>, Suggestion>>::new(error).to_string(),
            "env file missing\nDid you mean rename the .env.example file to .env?"
        );
    }

    #[test]
    fn test_empty_rhs_keeps_separator() {
        let error = SugError::Other;
        assert_eq!(
            Formatted::<_, Add<Add<OneLine, NewLine>, Suggestion>>::new(error).to_string(),
            "something else\n"
        );
    }

    #[test]
    fn test_right_associated_nesting() {
        let error = crate::tests::Error::Two(ErrorInner::One);
        assert_eq!(
            Formatted::<_, Add<OneLine, Add<NewLine, OneLine>>>::new(error).to_string(),
            "Two: One\nTwo: One"
        );
    }

    #[test]
    fn test_space_between_repeats() {
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, Add<OneLine, Add<Space, OneLine>>>::new(error).to_string(),
            "One One"
        );
    }

    #[test]
    fn test_debug_prints_inner() {
        let add = Add::<OneLine, separator::NewLine>::default();
        assert_eq!(format!("{add:?}"), "Add(OneLine, NewLine)");
    }

    #[test]
    fn test_colon_space_alias() {
        // ColonSpace ignores the error and writes ": ".
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, Add<OneLine, Add<ColonSpace, OneLine>>>::new(error).to_string(),
            "One: One"
        );
    }

    #[test]
    fn test_with_space_alias() {
        use separator::WithSpace;
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, WithSpace<OneLine, OneLine>>::new(error).to_string(),
            "One One"
        );
    }

    #[test]
    fn test_with_newline_alias() {
        use separator::WithNewLine;
        let error = crate::tests::Error::Two(ErrorInner::One);
        assert_eq!(
            Formatted::<_, WithNewLine<OneLine, OneLine>>::new(error).to_string(),
            "Two: One\nTwo: One"
        );
    }

    #[test]
    fn test_with_colon_space_alias() {
        use separator::WithColonSpace;
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, WithColonSpace<OneLine, OneLine>>::new(error).to_string(),
            "One: One"
        );
    }

    #[test]
    fn test_add_sep_generic_alias() {
        use separator::Colon;
        let error = crate::tests::Error::One;
        assert_eq!(
            Formatted::<_, WithSep<OneLine, Colon, OneLine>>::new(error).to_string(),
            "One:One"
        );
    }
}
