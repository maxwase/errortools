//! Context-tagged error pair.

use core::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
};

use derive_where::derive_where;

use crate::Format;

mod format;

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
/// The standard-trait impls (`Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`) bound
/// only `C`/`E`, so they do **not** impose `WithContextFormat: Trait` bounds.
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
}
