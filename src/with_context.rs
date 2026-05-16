//! Context-tagged error pair.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

pub use crate::with_context::format::{Colon, ContextFormat, WithContextColon};
#[cfg(feature = "std")]
pub use crate::with_context::format::{PathColon, WithContextPathColon};

/// Convenience alias for [`WithContext`] with the default [`PathColon`] strategy.
#[cfg(feature = "std")]
pub type WithPath<C, E> = WithContext<C, E, PathColon>;

/// A context value paired with an error, rendered through a static
/// [`ContextFormat`] strategy.
///
/// `Display` delegates to `F`. [`Error::source`] returns the inner error's
/// source (skipping `error` itself, since the strategy already prints it), so
/// chain-walking strategies don't duplicate it.
///
/// # Example
/// ```
/// use errortools::{FormatError, with_context::WithContextColon};
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
/// use errortools::{WithContext, with_context::ContextFormat};
///
/// struct Arrow;
/// impl<C: Display, E: Display> ContextFormat<C, E> for Arrow {
///     fn fmt(c: &C, e: &E, f: &mut Formatter<'_>) -> fmt::Result {
///         write!(f, "{c} -> {e}")
///     }
/// }
///
/// let w = WithContext::<_, _, Arrow>::new(1, "boom");
/// assert_eq!(w.to_string(), "1 -> boom");
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
    pub fn with_format<G: ContextFormat<C, E>>(self) -> WithContext<C, E, G> {
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

/// Renders the pair via the strategy `F`. `C` and `E` have no `Display` bound
/// here — the strategy decides what each must implement.
impl<C, E, F: ContextFormat<C, E>> Display for WithContext<C, E, F> {
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
    C: Debug,
    E: Error + 'static,
    F: ContextFormat<C, E>,
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
    #[cfg(feature = "std")]
    use std::path::Path;

    use super::WithContext;

    /// Convenience alias for [`WithContext`] with the default [`Colon`] strategy.
    ///
    /// Use this when you don't need a custom format and want type inference on
    /// [`WithContext::new`] to work with a default strategy without a turbofish.
    pub type WithContextColon<C, E> = WithContext<C, E, Colon>;

    /// Convenience alias for [`WithContext`] with the [`PathColon`] strategy.
    /// Use this when your context is a path and you want it rendered via `Path::display`
    /// without needing to wrap it in [`DisplayPath`](crate::DisplayPath) or another newtype.
    #[cfg(feature = "std")]
    pub type WithContextPathColon<C, E> = WithContext<C, E, PathColon>;

    /// A static strategy for combining a context value and an error into a single
    /// [`Display`] line.
    ///
    /// The trait is parameterized over `C` and `E` so each strategy can declare
    /// its own bounds: [`Colon`] requires `Display` on both, [`PathColon`]
    /// requires `AsRef<Path>` on the context, and custom strategies can require
    /// whatever they need. The error's source chain is still walked by
    /// [`FormatError`](crate::FormatError) strategies (`OneLine`, `Tree`, …);
    /// this trait only controls how the `(context, error)` pair itself is printed.
    pub trait ContextFormat<C: ?Sized, E: ?Sized> {
        /// Writes `context` and `error` to `f` using the strategy.
        fn fmt(context: &C, error: &E, f: &mut Formatter<'_>) -> fmt::Result;
    }

    /// Default [`ContextFormat`]: writes `"{context}: {error}"` for any pair of
    /// `Display` values.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct Colon;

    impl<C: Display + ?Sized, E: Display + ?Sized> ContextFormat<C, E> for Colon {
        fn fmt(context: &C, error: &E, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{context}: {error}")
        }
    }

    /// Path-aware [`ContextFormat`]: writes `"{path}: {error}"` where `path` is
    /// rendered via [`Path::display`].
    ///
    /// `Path` and `PathBuf` don't implement [`Display`] (paths may not be valid
    /// UTF-8), so [`Colon`] won't accept them. `PathColon` plugs that gap
    /// without needing a wrapper newtype around the context value.
    #[cfg(feature = "std")]
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct PathColon;

    #[cfg(feature = "std")]
    impl<P: AsRef<Path> + ?Sized, E: Display + ?Sized> ContextFormat<P, E> for PathColon {
        fn fmt(path: &P, error: &E, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "{}: {error}", path.as_ref().display())
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
    impl<C: Display, E: Display> ContextFormat<C, E> for Bracketed {
        fn fmt(c: &C, e: &E, f: &mut Formatter<'_>) -> fmt::Result {
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
        impl<C: Display, E: Display> ContextFormat<C, E> for Arrow {
            fn fmt(c: &C, e: &E, f: &mut Formatter<'_>) -> fmt::Result {
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

    /// `PathColon` formats path contexts directly, without a wrapper newtype.
    #[cfg(feature = "std")]
    #[test]
    fn test_path_colon_strategy() {
        use std::path::{Path, PathBuf};

        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let w = WithContext::<_, _, PathColon>::new(PathBuf::from("a/b/c.txt"), io_err);
        assert_eq!(w.to_string(), "a/b/c.txt: file missing");

        // Works for borrowed paths too.
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let path: &Path = Path::new("a/b/c.txt");
        let w = WithContext::<_, _, PathColon>::new(path, io_err);
        assert_eq!(w.to_string(), "a/b/c.txt: file missing");
    }
}
