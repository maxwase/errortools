//! Context-tagged error pair.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

pub use crate::with_context::format::{Colon, ContextFormat, WithContextColon};

/// A context value paired with an error, rendered through a static
/// [`ContextFormat`] strategy.
///
/// `Display` delegates to `F`. [`Error::source`] returns the inner error's
/// source (skipping `error` itself, since the strategy already prints it), so
/// chain-walking strategies don't duplicate it.
///
/// `F` is a type-level tag (never instantiated). The `fn() -> F` inside
/// [`PhantomData`] avoids drop-check ownership of `F` and keeps the wrapper
/// `Send + Sync` regardless of `F`.
///
/// # Example
/// ```
/// use errortools::{FormatError, WithContextColon};
/// use std::io;
///
/// let err = io::Error::new(io::ErrorKind::NotFound, "file missing");
/// let ctx = WithContextColon::new("path/to/config", err);
/// assert_eq!(ctx.one_line().to_string(), "path/to/config: file missing");
/// ```
///
/// # Custom strategy
/// ```
/// use core::fmt::{self, Display, Formatter};
/// use errortools::{ContextFormat, WithContext};
///
/// struct Arrow;
/// impl ContextFormat for Arrow {
///     fn fmt<C: Display, E: Display>(c: &C, e: &E, f: &mut Formatter<'_>) -> fmt::Result {
///         write!(f, "{c} -> {e}")
///     }
/// }
///
/// let w = WithContext::<_, _, Arrow>::new("step", "boom");
/// assert_eq!(w.to_string(), "step -> boom");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WithContext<C, E, F = format::Colon> {
    /// The context value tagging this error (e.g. a file path or step number).
    pub context: C,
    /// The underlying error.
    pub error: E,

    _format: PhantomData<fn() -> F>,
}

impl<C, E, F> WithContext<C, E, F> {
    /// Creates a new [`WithContext`] pairing `context` with `error`.
    ///
    /// Use [`WithContextColon`] for the default `Colon` strategy and type inference on `new` without a turbofish.
    pub const fn new(context: C, error: E) -> Self {
        Self {
            context,
            error,
            _format: PhantomData,
        }
    }

    /// Switches the formatting strategy without touching the stored values.
    pub fn with_format<G: ContextFormat>(self) -> WithContext<C, E, G> {
        WithContext {
            context: self.context,
            error: self.error,
            _format: PhantomData,
        }
    }
}

impl<C, E, F> From<(C, E)> for WithContext<C, E, F> {
    fn from((context, error): (C, E)) -> Self {
        Self::new(context, error)
    }
}

/// Renders the pair via the strategy `F`.
impl<C: Display, E: Display, F: ContextFormat> Display for WithContext<C, E, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        F::fmt(&self.context, &self.error, f)
    }
}

/// Forwards to the fields' `Debug` rather than printing the `PhantomData` tag.
impl<C: Debug, E: Debug, F> Debug for WithContext<C, E, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithContext")
            .field("context", &self.context)
            .field("error", &self.error)
            .finish()
    }
}

impl<C, E, F> Error for WithContext<C, E, F>
where
    C: Display + Debug,
    E: Error + 'static,
    F: ContextFormat,
{
    /// Returns the inner error's source, skipping the inner error itself
    /// (already shown via [`Display`]) so chain-walking strategies don't
    /// duplicate it.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.error.source()
    }
}

mod format {
    //! Formatting strategies for [`WithContext`].

    use core::fmt::{self, Display, Formatter};

    use super::WithContext;

    /// Convenience alias for [`WithContext`] with the default [`Colon`] strategy.
    ///
    /// Use this when you don't need a custom format and want type inference on
    /// [`WithContext::new`] to work with a default strategy without a turbofish.
    pub type WithContextColon<C, E> = WithContext<C, E, Colon>;

    /// A static strategy for combining a context value and an error into a single
    /// [`Display`] line.
    ///
    /// Implement on a unit type to plug a custom rendering into
    /// [`WithContext`]. The error's source chain is still walked by
    /// [`FormatError`](crate::FormatError) strategies (`OneLine`, `Tree`, …); this
    /// trait only controls how the `(context, error)` pair itself is printed.
    pub trait ContextFormat {
        /// Writes `context` and `error` to `f` using the strategy.
        fn fmt<C: Display, E: Display>(
            context: &C,
            error: &E,
            f: &mut Formatter<'_>,
        ) -> fmt::Result;
    }

    /// Default [`ContextFormat`]: writes `"{context}: {error}"`.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct Colon;

    impl ContextFormat for Colon {
        fn fmt<C: Display, E: Display>(
            context: &C,
            error: &E,
            f: &mut Formatter<'_>,
        ) -> fmt::Result {
            write!(f, "{context}: {error}")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error as _, io};

    use thiserror::Error;

    use super::*;
    use crate::FormatError;

    #[derive(Error, Debug)]
    #[error("leaf error")]
    struct Leaf;

    #[derive(Error, Debug)]
    #[error("middle")]
    struct Middle(#[source] Leaf);

    struct Bracketed;
    impl ContextFormat for Bracketed {
        fn fmt<C: Display, E: Display>(c: &C, e: &E, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "[{c}] {e}")
        }
    }

    /// Caller-facing error in this test module. The `#[from]` impl is what
    /// drives `F = Bracketed` inference at the `?` site in [`returning_error`].
    #[derive(Error, Debug)]
    #[error("an error happened")]
    pub struct Error(#[from] WithContext<&'static str, Middle, Bracketed>);

    fn returning_middle() -> Result<(), Middle> {
        Err(Middle(Leaf))
    }

    /// Realistic use: a function tags an inner error with context via
    /// `map_err`, then `?` routes it through `#[from]` into the caller's
    /// error type.
    /// Most importantly, `F` is inferred from the `From` impl on `Error`.
    fn returning_error() -> Result<(), Error> {
        returning_middle().map_err(|e| WithContext::new("context", e))?;
        Ok(())
    }

    #[test]
    fn test_new_and_fields() {
        let w = format::WithContextColon::new("ctx", Leaf);
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_from_tuple() {
        let w: format::WithContextColon<&str, Leaf> = ("ctx", Leaf).into();
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_display_default_format() {
        let w = format::WithContextColon::new("step 3", Leaf);
        assert_eq!(w.to_string(), "step 3: leaf error");
    }

    #[test]
    fn test_source_skips_inner_error() {
        // Leaf has no source, so skipping it yields None.
        let w = format::WithContextColon::new("ctx", Leaf);
        assert!(w.source().is_none());

        // For Middle(Leaf), source must be Leaf — not Middle (which Display already shows).
        let w = format::WithContextColon::new("ctx", Middle(Leaf));
        let src = w.source().expect("source must be Some");
        assert_eq!(src.to_string(), "leaf error");
    }

    #[test]
    fn test_one_line_walks_full_chain() {
        let w = format::WithContextColon::new("ctx", Middle(Leaf));
        assert_eq!(w.one_line().to_string(), "ctx: middle: leaf error");
    }

    #[test]
    fn test_io_error_chain() {
        let io = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let w = format::WithContextColon::new("config", io);
        assert_eq!(w.one_line().to_string(), "config: file missing");
    }

    #[test]
    fn test_custom_format_strategy() {
        struct Arrow;
        impl ContextFormat for Arrow {
            fn fmt<C: Display, E: Display>(c: &C, e: &E, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{c} -> {e}")
            }
        }

        let w = WithContext::<_, _, Arrow>::new("step", Leaf);
        assert_eq!(w.to_string(), "step -> leaf error");
    }

    #[test]
    fn test_custom_format_affects_one_line() {
        let w = WithContext::<_, _, Bracketed>::new("ctx", Middle(Leaf));
        // Display: "[ctx] middle" — then chain appends ": leaf error" via source.
        assert_eq!(w.one_line().to_string(), "[ctx] middle: leaf error");
    }

    /// End-to-end: `map_err` wraps an inner error with [`WithContext`], `?`
    /// fires `From<WithContext<_, _, Bracketed>> for Error` (and pins `F`),
    /// and the full chain comes out via [`FormatError::one_line`] without any
    /// duplication thanks to `source` skipping the inner error.
    #[test]
    fn test_propagation_via_question_mark() {
        let err = returning_error().expect_err("returning_error must error");
        assert_eq!(err.to_string(), "an error happened");
        assert_eq!(
            err.one_line().to_string(),
            "an error happened: [context] middle: leaf error",
        );
    }
}
