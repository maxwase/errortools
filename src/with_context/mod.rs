//! Context-tagged error pair.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

use derive_where::derive_where;

use crate::Format;

mod format;

pub use crate::with_context::format::{Colon, ContextField, ErrorField};
#[cfg(feature = "std")]
pub use crate::with_context::format::{ContextPath, PathColon};

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
/// a tuple, e.g. the default [`Colon`] is
/// `(ContextField, ColonSpace, ErrorField)` with
/// [`ColonSpace`](crate::separator::ColonSpace) from [`separator`](crate::separator).
///
/// [`Error::source`] returns the inner error's source (skipping `error` itself,
/// since the strategy already prints it), so chain-walking strategies don't
/// duplicate it.
///
/// The standard-trait impls (`Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`) bound
/// only `C`/`E`, so they do **not** impose `WithContextFormat: Trait` bounds.
///
/// # Example
/// ```
/// use errortools::{FormatError, ResultExt, WithContext};
/// use std::io;
///
/// fn load() -> Result<(), WithContext<&'static str, io::Error>> {
///     let read = Err(io::Error::new(io::ErrorKind::NotFound, "file missing"));
///     read.context("path/to/config")
/// }
///
/// let err = load().unwrap_err();
/// assert_eq!(err.one_line().to_string(), "path/to/config: file missing");
/// ```
/// # Custom formatting
/// There are 2 ways to customize the formatting strategy:
///
/// ## Custom strategy via composition of field extractors and separators
/// ```
/// use errortools::{WithContext, separator::Space, with_context::{ContextField, ErrorField}};
///
/// // Same as `Colon` but uses a single space instead of ": ".
/// type SpacePair = (ContextField, Space, ErrorField);
/// let w = WithContext::<_, _, SpacePair>::new("step", "boom");
/// assert_eq!(w.to_string(), "step boom");
/// ```
///
/// ## Custom strategy via an impl of `Format<WithContext<...>> for YourStrategy`
///
/// The strategy should render the error's own text (`w.error`):
/// [`Error::source`] deliberately skips the inner error, so chain-walking
/// renderers ([`OneLine`](crate::OneLine), [`Chain`](crate::Chain), the
/// aggregate shapes) assume the strategy already printed it — a strategy that
/// omits it (e.g. context-only) silently drops the error text from every deep
/// rendering.
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
#[derive_where(Clone, Copy, PartialEq, Eq, Hash, Debug; C, E)]
pub struct WithContext<C, E, WithContextFormat = Colon> {
    /// The context value tagging this error (e.g. a file path or step number).
    pub context: C,
    /// The underlying error.
    pub error: E,

    #[derive_where(skip(Debug))]
    _format: PhantomData<fn() -> WithContextFormat>,
}

impl<C, E, WithContextFormat> WithContext<C, E, WithContextFormat> {
    /// Creates a new [`WithContext`] pairing `context` with `error`.
    ///
    /// The strategy is inferred from the expected type. On a `Result`, prefer
    /// [`ResultExt::context`] over `map_err(|e| WithContext::new(..))`.
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

impl<C, E, WithContextFormat> Error for WithContext<C, E, WithContextFormat>
where
    C: Debug,
    E: Error,
    WithContextFormat: Format<Self>,
{
    /// Returns the inner error's source, skipping the inner error itself
    /// (already shown via [`Display`]) so chain-walking strategies don't
    /// duplicate it.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.error.source()
    }
}

/// Extension methods that tag the error of a [`Result`] with a context value.
///
/// The format strategy `F` is never named at the call site: it is inferred
/// from the `From` target at the `?` site or from the declared return type.
///
/// ```
/// use errortools::{FormatError, ResultExt, WithContext};
/// use std::{fs, io};
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("Failed to load config")]
/// struct ConfigError(#[from] WithContext<&'static str, io::Error>);
///
/// fn load(path: &'static str) -> Result<String, ConfigError> {
///     Ok(fs::read_to_string(path).context(path)?)
/// }
///
/// let err = load("no/such/dir/config.toml").unwrap_err();
/// assert!(err.one_line().to_string().starts_with("Failed to load config: no/such/dir/config.toml: "));
/// ```
pub trait ResultExt<T, E> {
    /// Pairs the error with `context`, yielding a [`WithContext`] error.
    fn context<C, F>(self, context: C) -> Result<T, WithContext<C, E, F>>;

    /// Like [`context`](ResultExt::context), but builds the context only on `Err`.
    fn with_context<C, F>(self, f: impl FnOnce() -> C) -> Result<T, WithContext<C, E, F>>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn context<C, F>(self, context: C) -> Result<T, WithContext<C, E, F>> {
        self.map_err(|error| WithContext::new(context, error))
    }

    fn with_context<C, F>(self, f: impl FnOnce() -> C) -> Result<T, WithContext<C, E, F>> {
        self.map_err(|error| WithContext::new(f(), error))
    }
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
    /// [`ResultExt::context`], then `?` routes it through `#[from]` into the
    /// caller's error type.
    /// Most importantly, `WithContextFormat` is inferred from the `From` impl on `Error`.
    fn returning_error() -> Result<(), PropError> {
        returning_middle().context("context")?;
        Ok(())
    }

    #[test]
    fn test_new_and_fields() {
        let w = WithContext::<_, _, Colon>::new("ctx", Inner::A);
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_from_tuple() {
        let w: WithContext<&str, Inner> = ("ctx", Inner::A).into();
        assert_eq!(w.context, "ctx");
    }

    #[test]
    fn test_display_default_format() {
        let w = WithContext::<_, _, Colon>::new("step 3", Inner::A);
        assert_eq!(w.to_string(), "step 3: InnerA");
    }

    #[test]
    fn test_source_skips_inner_error() {
        // Inner::A has no source, so skipping it yields None.
        let w = WithContext::<_, _, Colon>::new("ctx", Inner::A);
        assert!(w.source().is_none());

        // For Mid::Inner(Inner::A), source must be Inner — not Mid (which Display already shows).
        let w = WithContext::<_, _, Colon>::new("ctx", Mid::Inner(Inner::A));
        let src = w.source().expect("source must be Some");
        assert_eq!(src.to_string(), "InnerA");
    }

    #[test]
    fn test_one_line_walks_full_chain() {
        let w = WithContext::<_, _, Colon>::new("ctx", Mid::Inner(Inner::A));
        assert_eq!(w.one_line().to_string(), "ctx: mid: InnerA");
    }

    #[test]
    fn test_io_error_chain() {
        let io = io::Error::new(io::ErrorKind::NotFound, "file missing");
        let w = WithContext::<_, _, Colon>::new("config", io);
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

    /// End-to-end: `context` wraps an inner error with [`WithContext`], `?`
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

    /// `.context(path)?` into an enum with several `#[from]` variants: the
    /// only `From<WithContext<..>>` impl pins `F = PathColon`, no annotation.
    #[cfg(feature = "std")]
    #[test]
    fn test_context_infers_path_strategy_at_question_mark() {
        use std::path::Path;

        #[derive(Error, Debug)]
        enum LoadError {
            #[error("Failed to load config")]
            Read(#[from] WithPath<&'static Path, io::Error>),
            #[error("Bad config")]
            Parse(#[from] Inner),
        }

        fn load(path: &'static Path) -> Result<(), LoadError> {
            let read: Result<(), io::Error> =
                Err(io::Error::new(io::ErrorKind::NotFound, "file missing"));
            read.context(path)?;
            Err(Inner::A)?
        }

        let err = load(Path::new("no/such/dir/config.toml")).expect_err("load must error");
        assert!(matches!(err, LoadError::Read(_)));
        assert_eq!(
            err.one_line().to_string(),
            "Failed to load config: no/such/dir/config.toml: file missing",
        );
    }

    #[test]
    fn test_context_infers_strategy_from_return_type() {
        fn run() -> Result<(), WithContext<&'static str, Mid, Bracketed>> {
            returning_middle().context("ctx")
        }

        assert_eq!(
            run().unwrap_err().one_line().to_string(),
            "[ctx] mid: InnerA"
        );
    }

    #[test]
    fn test_with_context_is_lazy() {
        fn run(result: Result<u8, Inner>) -> Result<u8, WithContext<&'static str, Inner>> {
            result.with_context(|| "step 3")
        }

        let ok: Result<u8, WithContext<&str, Inner>> =
            Ok::<u8, Inner>(1).with_context(|| unreachable!("context built on Ok"));
        assert_eq!(ok, Ok(1));
        assert_eq!(
            run(Err(Inner::A)).unwrap_err().to_string(),
            "step 3: InnerA"
        );
    }
}
