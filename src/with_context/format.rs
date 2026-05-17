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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WithContext, separator::WithSpace, tests::Inner};
    use std::io;

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
        type SpacePair = WithSpace<ContextField, ErrorField>;

        let w = WithContext::<_, _, SpacePair>::new("ctx", Inner::A);
        assert_eq!(w.to_string(), "ctx InnerA");
    }
}
