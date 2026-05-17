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
/// `Display` delegates to `WithContextFormat::fmt(self, f)`, so any
/// `WithContextFormat: Format<WithContext<C, E, WithContextFormat>>`
/// can format the pair. Strategies are usually built by composing the field
/// extractors [`ContextField`] / [`ErrorField`] (or `ContextPath` when
/// `C: AsRef<Path>`, requires `std`) with separator strategies via
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
/// ## Custom strategy via an impl of `Format<WithContext<...>> for YourStrategy`
/// ```
/// use core::fmt::{self, Display, Formatter};
/// use errortools::{Format, WithContext};
///
/// struct Arrow;
/// impl<C: Display, E: Display, WithContextFormat> Format<WithContext<C, E, WithContextFormat>> for Arrow {
///     fn fmt(w: &WithContext<C, E, WithContextFormat>, f: &mut Formatter<'_>) -> fmt::Result {
///         write!(f, "{} -> {}", w.context, w.error)
///     }
/// }
///
/// let w = WithContext::<_, _, Arrow>::new(1, "boom");
/// assert_eq!(w.to_string(), "1 -> boom");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WithContext<C, E, WithContextFormat = Colon> {
    /// The context value tagging this error (e.g. a file path or step number).
    pub context: C,
    /// The underlying error.
    pub error: E,

    _format: PhantomData<fn() -> WithContextFormat>,
}

impl<C, E, WithContextFormat> WithContext<C, E, WithContextFormat> {
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
    pub fn with_format<NewSelfFormat>(self) -> WithContext<C, E, NewSelfFormat>
    where
        NewSelfFormat: Format<WithContext<C, E, NewSelfFormat>>,
    {
        WithContext {
            context: self.context,
            error: self.error,
            _format: PhantomData,
        }
    }
}

impl<C, E, WithContextFormat> From<(C, E)> for WithContext<C, E, WithContextFormat> {
    fn from((context, error): (C, E)) -> Self {
        Self::new(context, error)
    }
}

/// Renders the pair via the strategy `WithContextFormat`. `C` and `E` have
/// no `Display` bound here — the strategy decides what each must implement.
impl<C, E, WithContextFormat> Display for WithContext<C, E, WithContextFormat>
where
    WithContextFormat: Format<Self>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        WithContextFormat::fmt(self, f)
    }
}

/// Forwards to the fields' `Debug` rather than printing the `PhantomData` tag.
impl<C: Debug, E: Debug, WithContextFormat> Debug for WithContext<C, E, WithContextFormat> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithContext")
            .field("context", &self.context)
            .field("error", &self.error)
            .finish()
    }
}

impl<C, E, WithContextFormat> Error for WithContext<C, E, WithContextFormat>
where
    C: Debug,
    E: Error + 'static,
    WithContextFormat: Format<Self>,
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

    impl<C: Display, E, WithContextFormat> Format<WithContext<C, E, WithContextFormat>>
        for ContextField
    {
        fn fmt(w: &WithContext<C, E, WithContextFormat>, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(&w.context, f)
        }
    }

    /// [`Format`] extractor that prints the `error` field via `Display`.
    ///
    /// Counterpart to [`ContextField`]. See [`Colon`] for the canonical use.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct ErrorField;

    impl<C, E: Display, WithContextFormat> Format<WithContext<C, E, WithContextFormat>> for ErrorField {
        fn fmt(w: &WithContext<C, E, WithContextFormat>, f: &mut Formatter<'_>) -> fmt::Result {
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
    impl<P: AsRef<Path>, E, WithContextFormat> Format<WithContext<P, E, WithContextFormat>>
        for ContextPath
    {
        fn fmt(w: &WithContext<P, E, WithContextFormat>, f: &mut Formatter<'_>) -> fmt::Result {
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
    use std::io;

    use thiserror::Error;

    use super::*;
    use crate::{
        FormatError,
        tests::{Bracketed, Inner, Mid, WcArrow},
    };

    /// Caller-facing error in this test module. The `#[from]` impl is what
    /// drives `WithContextFormat = Bracketed` inference at the `?` site in [`returning_error`].
    #[derive(Error, Debug)]
    #[error("an error happened")]
    pub struct PropError(#[from] WithContext<&'static str, Mid, Bracketed>);

    fn returning_middle() -> Result<(), Mid> {
        Err(Mid::Inner(Inner::A))
    }

    /// Realistic use: a function tags an inner error with context via
    /// `map_err`, then `?` routes it through `#[from]` into the caller's
    /// error type.
    /// Most importantly, `WithContextFormat` is inferred from the `From` impl on `Error`.
    fn returning_error() -> Result<(), PropError> {
        returning_middle().map_err(|e| WithContext::new("context", e))?;
        Ok(())
    }

    #[test]
    fn test_new_and_fields() {
        let w = WithContextColon::new("ctx", Inner::A);
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_from_tuple() {
        let w: WithContextColon<&str, Inner> = ("ctx", Inner::A).into();
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_display_default_format() {
        let w = WithContextColon::new("step 3", Inner::A);
        assert_eq!(w.to_string(), "step 3: InnerA");
    }

    #[test]
    fn test_source_skips_inner_error() {
        // Inner::A has no source, so skipping it yields None.
        let w = WithContextColon::new("ctx", Inner::A);
        assert!(w.source().is_none());

        // For Mid::Inner(Inner::A), source must be Inner — not Mid (which Display already shows).
        let w = WithContextColon::new("ctx", Mid::Inner(Inner::A));
        let src = w.source().expect("source must be Some");
        assert_eq!(src.to_string(), "InnerA");
    }

    #[test]
    fn test_one_line_walks_full_chain() {
        let w = WithContextColon::new("ctx", Mid::Inner(Inner::A));
        assert_eq!(w.one_line().to_string(), "ctx: mid: InnerA");
    }

    #[test]
    fn test_io_error_chain() {
        let io = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let w = WithContextColon::new("config", io);
        assert_eq!(w.one_line().to_string(), "config: file missing");
    }

    #[test]
    fn test_custom_format_strategy() {
        let w = WithContext::<_, _, WcArrow>::new("step", Inner::A);
        assert_eq!(w.to_string(), "step -> InnerA");
    }

    #[test]
    fn test_custom_format_affects_one_line() {
        let w = WithContext::<_, _, Bracketed>::new("ctx", Mid::Inner(Inner::A));
        // Display: "[ctx] mid" — then chain appends ": InnerA" via source.
        assert_eq!(w.one_line().to_string(), "[ctx] mid: InnerA");
    }

    /// End-to-end: `map_err` wraps an inner error with [`WithContext`], `?`
    /// fires `From<WithContext<_, _, Bracketed>> for PropError` (and pins
    /// `WithContextFormat`), and the full chain comes out via
    /// [`FormatError::one_line`] without any duplication thanks to `source`
    /// skipping the inner error.
    #[test]
    fn test_propagation_via_question_mark() {
        let err = returning_error().expect_err("returning_error must error");
        assert_eq!(err.to_string(), "an error happened");
        assert_eq!(
            err.one_line().to_string(),
            "an error happened: [context] mid: InnerA",
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

        let w = WithContext::<_, _, SpacePair>::new("ctx", Inner::A);
        assert_eq!(w.to_string(), "ctx InnerA");
    }
}
