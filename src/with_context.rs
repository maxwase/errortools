//! Context-tagged error pair.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

use crate::Format;

pub use crate::with_context::format::{Colon, ContextField, ErrorField, WithContextColon};
#[cfg(feature = "std")]
pub use crate::with_context::format::{ContextPath, PathColon, WithContextPathColon};

/// Convenience alias for [`WithContext`] with the default [`PathColon`] strategy.
#[cfg(feature = "std")]
pub type WithPath<C, E> = WithContext<C, E, PathColon>;

/// A context value paired with an error, rendered through a static
/// [`Format`] strategy.
///
/// `Display` delegates to `F::fmt(self, f)`, so any `F: Format<WithContext<C, E, F>>`
/// can format the pair. Strategies are usually built by composing the field
/// extractors [`ContextField`] / [`ErrorField`] (or [`ContextPath`] when
/// `C: AsRef<Path>`) with separator strategies via
/// [`Add`](crate::Add) / [`WithSep`](crate::separator::WithSep), e.g. the default
/// [`Colon`] is [`WithColonSpace<ContextField, ErrorField>`](crate::separator::WithColonSpace).
///
/// [`Error::source`] returns the inner error's source (skipping `error` itself,
/// since the strategy already prints it), so chain-walking strategies don't
/// duplicate it.
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
/// # Custom formatting
/// There are 2 ways to customize the formatting strategy:
///
/// ## Custom strategy via composition of field extractors and separators
/// ```
/// use errortools::{WithContext, separator::WithSpace, with_context::{ContextField, ErrorField}};
///
/// // Same as `Colon` but uses a single space instead of ": ".
/// type SpacePair = WithSpace<ContextField, ErrorField>;
/// let w = WithContext::<_, _, SpacePair>::new("step", "boom");
/// assert_eq!(w.to_string(), "step boom");
/// ```
///
/// ## Custom strategy via a an impl of `Format<WithContext<...>> for YourStrategy`
/// ```
/// use core::fmt::{self, Display, Formatter};
/// use errortools::{Format, WithContext};
///
/// struct Arrow;
/// impl<C: Display, E: Display, F> Format<WithContext<C, E, F>> for Arrow {
///     fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
///         write!(f, "{} -> {}", w.context, w.error)
///     }
/// }
///
/// let w = WithContext::<_, _, Arrow>::new(1, "boom");
/// assert_eq!(w.to_string(), "1 -> boom");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WithContext<C, E, F = Colon> {
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
    pub fn with_format<G>(self) -> WithContext<C, E, G>
    where
        G: Format<WithContext<C, E, G>>,
    {
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
impl<C, E, F> Display for WithContext<C, E, F>
where
    F: Format<Self>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        F::fmt(self, f)
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
    F: Format<Self>,
{
    /// Returns the inner error's source, skipping the inner error itself
    /// (already shown via [`Display`]) so chain-walking strategies don't
    /// duplicate it.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.error.source()
    }
}

mod format {
    //! Field extractors and pre-composed strategies for [`WithContext`].

    use core::fmt::{self, Display, Formatter};
    #[cfg(feature = "std")]
    use std::path::Path;

    #[allow(unused_imports)] // referenced from doc links
    use crate::add::separator::{ColonSpace, WithSep};
    use crate::{Format, add::separator::WithColonSpace};

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

    /// [`Format`] extractor that prints the `context` field via `Display`.
    ///
    /// Compose with [`ErrorField`] and a separator to build pair strategies:
    /// [`WithSep<ContextField, ColonSpace, ErrorField>`](WithSep) is exactly [`Colon`].
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct ContextField;

    impl<C: Display, E, F> Format<WithContext<C, E, F>> for ContextField {
        fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(&w.context, f)
        }
    }

    /// [`Format`] extractor that prints the `error` field via `Display`.
    ///
    /// Counterpart to [`ContextField`]. See [`Colon`] for the canonical use.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct ErrorField;

    impl<C, E: Display, F> Format<WithContext<C, E, F>> for ErrorField {
        fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(&w.error, f)
        }
    }

    /// [`Format`] extractor that prints the `context` field via [`Path::display`].
    ///
    /// `Path` and `PathBuf` don't implement [`Display`] (paths may not be valid
    /// UTF-8), so [`ContextField`] won't accept them. `ContextPath` plugs that
    /// gap without needing a wrapper newtype around the context value.
    #[cfg(feature = "std")]
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct ContextPath;

    #[cfg(feature = "std")]
    impl<P: AsRef<Path>, E, F> Format<WithContext<P, E, F>> for ContextPath {
        fn fmt(w: &WithContext<P, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
            w.context.as_ref().display().fmt(f)
        }
    }

    /// Default pair strategy: writes `"{context}: {error}"` for any pair of
    /// `Display` values.
    ///
    /// Equivalent to [`WithColonSpace<ContextField, ErrorField>`](WithColonSpace).
    pub type Colon = WithColonSpace<ContextField, ErrorField>;

    /// Path-aware pair strategy: writes `"{path}: {error}"` where `path` is
    /// rendered via [`Path::display`].
    ///
    /// Equivalent to [`WithColonSpace<ContextPath, ErrorField>`](WithColonSpace).
    #[cfg(feature = "std")]
    pub type PathColon = WithColonSpace<ContextPath, ErrorField>;
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

    /// Custom one-shot strategy: `[ctx] err`.
    struct Bracketed;
    impl<C: Display, E: Display, F> Format<WithContext<C, E, F>> for Bracketed {
        fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "[{}] {}", w.context, w.error)
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
        let w = WithContextColon::new("ctx", Leaf);
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_from_tuple() {
        let w: WithContextColon<&str, Leaf> = ("ctx", Leaf).into();
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_display_default_format() {
        let w = WithContextColon::new("step 3", Leaf);
        assert_eq!(w.to_string(), "step 3: leaf error");
    }

    #[test]
    fn test_source_skips_inner_error() {
        // Leaf has no source, so skipping it yields None.
        let w = WithContextColon::new("ctx", Leaf);
        assert!(w.source().is_none());

        // For Middle(Leaf), source must be Leaf — not Middle (which Display already shows).
        let w = WithContextColon::new("ctx", Middle(Leaf));
        let src = w.source().expect("source must be Some");
        assert_eq!(src.to_string(), "leaf error");
    }

    #[test]
    fn test_one_line_walks_full_chain() {
        let w = WithContextColon::new("ctx", Middle(Leaf));
        assert_eq!(w.one_line().to_string(), "ctx: middle: leaf error");
    }

    #[test]
    fn test_io_error_chain() {
        let io = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let w = WithContextColon::new("config", io);
        assert_eq!(w.one_line().to_string(), "config: file missing");
    }

    #[test]
    fn test_custom_format_strategy() {
        struct Arrow;
        impl<C: Display, E: Display, F> Format<WithContext<C, E, F>> for Arrow {
            fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{} -> {}", w.context, w.error)
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

    /// Compose a custom delimiter without writing a new Format impl.
    #[test]
    fn test_composed_separator() {
        use crate::separator::WithSpace;
        type SpacePair = WithSpace<ContextField, ErrorField>;

        let w = WithContext::<_, _, SpacePair>::new("ctx", Leaf);
        assert_eq!(w.to_string(), "ctx leaf error");
    }
}
