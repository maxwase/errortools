//! Separator strategies for composing [`Format`] strategies in tuples.
//!
//! Each separator is a [`Format`] tag that ignores its input and writes a
//! fixed string. Because [`Format<E>`] does not require `E: Error`, these
//! separators also compose with non-error formatters (e.g. the field
//! extractors used by [`WithContext`](crate::WithContext)).
//!
//! - [`Lit`] writes one or two fixed characters; [`NewLine`], [`Space`],
//!   [`ColonChar`] and [`ColonSpace`] are aliases of it.
//! - [`Empty`] writes nothing.
//! - [`Then`] writes a separator only if the strategy after it produces output.

use core::{
    fmt::{self, Write},
    marker::PhantomData,
};

use derive_where::derive_where;

use crate::{Format, FormattedRef};

/// [`Format`] strategy that writes the character `A`, then `B`, and ignores
/// the input.
///
/// `B` defaults to `'\0'`, which means "no second character", so `Lit<' '>`
/// writes a single space and `Lit<'-', '>'>` writes `->`. Designed as a
/// separator element of a tuple strategy, e.g. `(OneLine, Lit<'-', '>'>, Chain)`.
///
/// `Debug` prints the literal as a string: `Lit("->")`.
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lit<const A: char, const B: char = '\0'>;

impl<const A: char, const B: char, E: ?Sized> Format<E> for Lit<A, B> {
    fn fmt(_: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char(A)?;
        if B != '\0' {
            f.write_char(B)?;
        }
        Ok(())
    }
}

/// Prints the characters as one escaped string literal, e.g. `Lit("\n")` or
/// `Lit(": ")`, since the aliases ([`NewLine`], ...) are not nameable from
/// inside the type.
impl<const A: char, const B: char> fmt::Debug for Lit<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tuple = f.debug_tuple("Lit");
        if B == '\0' {
            tuple.field(&format_args!("\"{}\"", A.escape_debug()));
        } else {
            tuple.field(&format_args!(
                "\"{}{}\"",
                A.escape_debug(),
                B.escape_debug()
            ));
        }
        tuple.finish()
    }
}

/// Separator that writes a single line feed.
pub type NewLine = Lit<'\n'>;

/// Separator that writes a single space.
pub type Space = Lit<' '>;

/// Separator that writes a colon (`":"`).
///
/// Named `ColonChar` (not `Colon`) to avoid colliding with the
/// [`Colon`](crate::with_context::Colon) *pair* strategy: a misimport between
/// the two would compile but render each leaf as a bare `":"`.
pub type ColonChar = Lit<':'>;

/// Separator that writes `": "`.
pub type ColonSpace = Lit<':', ' '>;

/// [`Format`] strategy that writes nothing.
///
/// Useful as a no-op identity element when composing strategies in a tuple.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Empty;

impl<E: ?Sized> Format<E> for Empty {
    fn fmt(_: &E, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

/// Lazy separator: writes `Sep`, then `R`, but only if `R` produces output.
///
/// A plain tuple such as `(OneLine, NewLine, Suggestion)` writes the newline
/// even when the suggestion is empty. `(OneLine, Then<NewLine, Suggestion>)`
/// leaves no trailing newline in that case.
///
/// `Sep` is written just before the first non-empty piece of `R`'s output, so
/// nothing is buffered or allocated. `R` renders through a fresh formatter, so
/// width, precision and other flags of the outer formatter do not reach it.
#[derive_where(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Then<Sep, R>(PhantomData<fn() -> (Sep, R)>);

/// Prints the inner strategy values (instantiated via [`Default`]) instead of
/// `Then(PhantomData)`.
impl<Sep: fmt::Debug + Default, R: fmt::Debug + Default> fmt::Debug for Then<Sep, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Then")
            .field(&Sep::default())
            .field(&R::default())
            .finish()
    }
}

impl<E, Sep, R> Format<E> for Then<Sep, R>
where
    E: ?Sized,
    Sep: Format<E>,
    R: Format<E>,
{
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut writer = SepFirst::<E, Sep> {
            inner: f,
            error,
            pending: true,
            sep: PhantomData,
        };
        write!(writer, "{}", FormattedRef::<E, R>::new(error))
    }
}

/// A [`fmt::Write`] adapter that writes `Sep` before the first non-empty
/// chunk and passes everything else through.
struct SepFirst<'a, 'b, E: ?Sized, Sep> {
    inner: &'a mut fmt::Formatter<'b>,
    error: &'a E,
    pending: bool,
    sep: PhantomData<fn() -> Sep>,
}

impl<E: ?Sized, Sep: Format<E>> Write for SepFirst<'_, '_, E, Sep> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.is_empty() {
            return Ok(());
        }
        if self.pending {
            self.pending = false;
            Sep::fmt(self.error, self.inner)?;
        }
        self.inner.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Formatted, OneLine, Suggestion,
        tests::{Error, Inner},
    };

    fn _assert_traits() {
        fn assert_all<
            T: Clone + Copy + Default + PartialEq + Eq + core::hash::Hash + Send + Sync,
        >() {
        }
        assert_all::<NewLine>();
        assert_all::<Space>();
        assert_all::<Lit<'-', '>'>>();
        assert_all::<Then<NewLine, Suggestion>>();
    }

    #[test]
    fn test_space_between_repeats() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, Space, OneLine)>::new(error).to_string(),
            "One One"
        );
    }

    #[test]
    fn test_newline_between_repeats() {
        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, (OneLine, NewLine, OneLine)>::new(error).to_string(),
            "Two: InnerA\nTwo: InnerA"
        );
    }

    #[test]
    fn test_colon_space_alias() {
        // ColonSpace ignores the error and writes ": ".
        let error = Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, ColonSpace, OneLine)>::new(error).to_string(),
            "One: One"
        );
    }

    #[test]
    fn test_colon_char_alias() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, ColonChar, OneLine)>::new(error).to_string(),
            "One:One"
        );
    }

    #[test]
    fn test_empty_writes_nothing() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, Empty, OneLine)>::new(error).to_string(),
            "OneOne"
        );
    }

    #[test]
    fn test_lit_two_chars() {
        assert_eq!(
            Formatted::<_, Lit<'-', '>'>>::new(Error::One).to_string(),
            "->"
        );
        assert_eq!(
            Formatted::<_, (OneLine, Lit<'-', '>'>, OneLine)>::new(Error::One).to_string(),
            "One->One"
        );
    }

    #[test]
    fn test_lit_debug() {
        assert_eq!(format!("{:?}", NewLine::default()), r#"Lit("\n")"#);
        assert_eq!(format!("{:?}", Space::default()), r#"Lit(" ")"#);
        assert_eq!(format!("{:?}", ColonChar::default()), r#"Lit(":")"#);
        assert_eq!(format!("{:?}", ColonSpace::default()), r#"Lit(": ")"#);
        assert_eq!(format!("{:?}", Lit::<'-', '>'>), r#"Lit("->")"#);
    }

    #[test]
    fn test_then_non_empty_rhs_writes_separator() {
        let error = Error::One;
        assert_eq!(
            Formatted::<_, (OneLine, Then<NewLine, Suggestion>)>::new(error).to_string(),
            "One\nTry passing --help to see available options."
        );
    }

    #[test]
    fn test_then_empty_rhs_skips_separator() {
        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, (OneLine, Then<NewLine, Suggestion>)>::new(error).to_string(),
            "Two: InnerA"
        );
        assert_eq!(
            Formatted::<_, Then<ColonSpace, Empty>>::new(Error::One).to_string(),
            ""
        );
    }

    /// Outer width flags reach a plain tuple element but not the right side
    /// of `Then`, which renders through a fresh formatter.
    #[test]
    fn test_then_does_not_forward_width() {
        /// Honours the outer width, like `Display` for `str` does.
        struct Padded;
        impl<E: ?Sized> Format<E> for Padded {
            fn fmt(_: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.pad("x")
            }
        }

        let in_tuple = format!("{:>4}", Formatted::<_, (Empty, Padded)>::new(Error::One));
        assert_eq!(in_tuple, "   x");
        let through_then = format!(
            "{:>4}",
            Formatted::<_, Then<Empty, Padded>>::new(Error::One)
        );
        assert_eq!(through_then, "x");
    }

    /// The separator is written once, even when the right side writes in
    /// several chunks.
    #[test]
    fn test_then_separator_written_once() {
        let error = Error::Two(Inner::A);
        assert_eq!(
            Formatted::<_, Then<Lit<'-', '>'>, OneLine>>::new(error).to_string(),
            "->Two: InnerA"
        );
    }

    #[test]
    fn test_then_debug_prints_inner() {
        let then = Then::<NewLine, Suggestion>::default();
        assert_eq!(format!("{then:?}"), r#"Then(Lit("\n"), Suggestion)"#);
    }
}
