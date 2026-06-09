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
///
/// Named `ColonChar` (not `Colon`) to avoid colliding with the
/// [`Colon`](crate::with_context::Colon) *pair* strategy — a misimport between
/// the two would compile but render each leaf as a bare `":"`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColonChar;

impl<E: ?Sized> Format<E> for ColonChar {
    fn fmt(_: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(":")
    }
}

/// Convenience alias for `Add<ColonChar, Space>` — writes `": "`.
pub type ColonSpace = Add<ColonChar, Space>;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Add, Formatted, OneLine,
        tests::{Error, Inner},
    };

    #[test]
    fn test_space_between_repeats() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, Add<OneLine, Add<Space, OneLine>>>::new(error).to_string(),
            "One One"
        );
    }

    #[test]
    fn test_colon_space_alias() {
        // ColonSpace ignores the error and writes ": ".
        let error = Error::One;
        assert_eq!(
            Formatted::<_, Add<OneLine, Add<ColonSpace, OneLine>>>::new(error).to_string(),
            "One: One"
        );
    }

    #[test]
    fn test_with_space_alias() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, WithSpace<OneLine, OneLine>>::new(error).to_string(),
            "One One"
        );
    }

    #[test]
    fn test_with_newline_alias() {
        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, WithNewLine<OneLine, OneLine>>::new(error).to_string(),
            "Two: InnerA\nTwo: InnerA"
        );
    }

    #[test]
    fn test_with_colon_space_alias() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, WithColonSpace<OneLine, OneLine>>::new(error).to_string(),
            "One: One"
        );
    }

    #[test]
    fn test_add_sep_generic_alias() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, WithSep<OneLine, ColonChar, OneLine>>::new(error).to_string(),
            "One:One"
        );
    }
}
