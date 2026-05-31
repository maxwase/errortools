use core::{error::Error, fmt};

use crate::Format;

/// A suggestion for how to fix an error.
///
/// Only the top-level error's hint is printed, the source chain isn't walked.
/// This decision is intentional: The underlying hint may be irrelevant in the context of the top-level error, and printing it may just add noise.
/// The idea is that every error that is supposed to have a suggestion should implement [`Suggest`]
/// and then later the top-level error's suggestion may concatenate the inner hint if it's relevant
/// with nesting matching the error chain.
///
/// Implement on an error type to provide a per-variant hint (e.g.
/// `"Did you mean to rename .env.example to .env?"`). The default impl writes
/// nothing, so types only need to implement it for the variants that have a
/// hint.
///
/// Render via [`FormatError::suggestion`](crate::FormatError::suggestion),
/// which dispatches through the [`Suggestion`] strategy.
pub trait Suggest {
    /// Writes the suggestion text for `self` to `f`. The default writes
    /// nothing.
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

/// Similar blanket impl as in fmt::Display
impl<T: Suggest + ?Sized> Suggest for &T {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Suggest::fmt(*self, f)
    }
}

/// [`Format`] strategy that renders the top-level error's [`Suggestion`] hint.
///
/// The source chain is not walked — only the wrapped error's own hint is
/// printed. Pair with [`FormatError::suggestion`](crate::FormatError::suggestion)
/// (returns `Formatted<&Self, Suggestion>`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Suggestion;

impl<E: Error + Suggest + ?Sized> Format<E> for Suggestion {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Suggest::fmt(error, f)
    }
}

#[cfg(test)]
mod tests {
    //! Tests are organized around one invariant:
    //!
    //! **`Suggest` is a caller-level annotation — it is never auto-delegated
    //! through the source chain or through `#[error(transparent)]`.**
    //!
    //! `#[error(transparent)]` collapses `Display` and `source()`, but
    //! `Suggest::fmt` is always dispatched on the *concrete outer type*.
    //! Chain length, source depth, and transparent wrappers are all irrelevant.

    use core::error::Error as _;
    use std::io;

    use crate::{
        Add, FormatError,
        separator::NewLine,
        tests::{Error, Inner, Mid, NoHint},
    };

    // --- baseline: hint vs no-hint ---

    #[test]
    fn hint_variant_renders_message() {
        // Error::One has a hint; Error::Three has a different hint.
        assert_eq!(
            Error::One.suggestion().to_string(),
            "Try passing --help to see available options.",
        );
        assert_eq!(
            Error::Three(io::Error::other("x")).suggestion().to_string(),
            "Check that the file path exists and permissions are correct.",
        );
    }

    #[test]
    fn no_hint_variant_renders_empty_string() {
        assert_eq!(Error::Two(Inner::A).suggestion().to_string(), "");
        assert_eq!(
            Error::Transparent(Mid::Inner(Inner::A))
                .suggestion()
                .to_string(),
            ""
        );
    }

    #[test]
    fn default_impl_writes_nothing() {
        // NoHint uses the default impl — suggestion() must be callable and empty.
        assert_eq!(NoHint.suggestion().to_string(), "");
    }

    #[test]
    fn debug_of_formatted_suggestion_forwards_to_inner_debug() {
        // Formatted<_, Suggestion> forwards Debug to the inner error's Debug,
        // not to Suggestion (which is a zero-size tag type).
        assert_eq!(format!("{:?}", Error::One.suggestion()), "One");
        assert_eq!(format!("{:?}", NoHint.suggestion()), "NoHint");
    }

    // --- suggestion is orthogonal to Display ---

    #[test]
    fn one_line_and_suggestion_are_independent_strategies() {
        let e = Error::One;
        // one_line walks the source chain; suggestion ignores it.
        assert_eq!(e.one_line().to_string(), "One");
        assert_eq!(
            e.suggestion().to_string(),
            "Try passing --help to see available options.",
        );
        // They can be composed via Add.
        assert_eq!(
            e.formatted::<Add<crate::Flat, Add<NewLine, crate::Suggestion>>>()
                .to_string(),
            "One\nTry passing --help to see available options.",
        );
    }

    // --- suggestion does NOT walk the source chain ---

    #[test]
    fn suggestion_ignores_source_chain_depth() {
        // Error::Two has a source (Inner::A); its Suggest arm returns "".
        let with_source = Error::Two(Inner::A);
        assert_eq!(with_source.one_line().to_string(), "Two: InnerA");
        assert_eq!(with_source.suggestion().to_string(), "");

        // Longer chain: Error::Transparent → Mid::Inner → Inner::A.
        let with_chain = Error::Transparent(Mid::Inner(Inner::A));
        assert_eq!(with_chain.one_line().to_string(), "mid: InnerA");
        assert_eq!(with_chain.suggestion().to_string(), "");
    }

    #[test]
    fn suggestion_fires_even_with_no_source() {
        // Error::One has no source — prove chain depth is not required.
        assert!(Error::One.source().is_none());
        assert_ne!(Error::One.suggestion().to_string(), "");
    }

    // --- suggestion is NOT delegated through transparent ---

    #[test]
    fn transparent_collapses_display_but_not_suggestion() {
        // Error::Transparent is #[error(transparent)] — display collapses to Mid's.
        // But Suggest::fmt is dispatched on Error, not on Mid.
        // Error's Transparent arm returns "".
        let with_inner = Error::Transparent(Mid::Inner(Inner::A));
        let with_io = Error::Transparent(Mid::Io(io::Error::other("io error")));

        // Display collapsed through transparent.
        assert_eq!(with_inner.to_string(), "mid");
        assert_eq!(with_io.to_string(), "io error");

        // Suggestion is NOT collapsed — outer type's impl always wins.
        assert_eq!(with_inner.suggestion().to_string(), "");
        assert_eq!(with_io.suggestion().to_string(), "");
    }

    #[test]
    fn double_transparent_display_collapses_suggestion_stays_at_outermost() {
        // Error::Transparent(Mid::Io(io_err)):
        //   display = io message (two transparent layers)
        //   source  = None (io::Error::other has none)
        //   suggestion = "" (Error's Transparent arm, not delegated)
        let e = Error::Transparent(Mid::Io(io::Error::other("deep io")));
        assert_eq!(e.to_string(), "deep io");
        assert!(e.source().is_none());
        assert_eq!(e.suggestion().to_string(), "");
        // one_line has no chain to walk — just the one display string.
        assert_eq!(e.one_line().to_string(), "deep io");
    }

    #[test]
    fn hint_and_no_hint_variants_coexist_in_same_type() {
        // Error has both hint-bearing (One, Three) and silent (Two, Transparent) variants.
        // The match arm in Suggest controls everything — no cross-variant leakage.
        assert_ne!(Error::One.suggestion().to_string(), "");
        assert_eq!(Error::Two(Inner::A).suggestion().to_string(), "");
        assert_ne!(
            Error::Three(io::Error::other("x")).suggestion().to_string(),
            "",
        );
        assert_eq!(
            Error::Transparent(Mid::Inner(Inner::A))
                .suggestion()
                .to_string(),
            "",
        );
    }

    // --- ref delegation: impl Suggest for &T ---

    #[test]
    fn suggest_blanket_impl_works_on_shared_ref() {
        // impl<T: Suggest + ?Sized> Suggest for &T delegates to T.
        let e = Error::One;
        let r: &Error = &e;
        // &Error: core::error::Error (via blanket) + Suggest (via blanket on &T).
        assert_eq!(
            r.suggestion().to_string(),
            "Try passing --help to see available options.",
        );
        let no: &NoHint = &NoHint;
        assert_eq!(no.suggestion().to_string(), "");
    }
}
