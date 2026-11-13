//! [`Detached`]: an owned snapshot of an error chain.

use alloc::{
    boxed::Box,
    string::{String, ToString as _},
    vec::Vec,
};
use core::{error::Error, fmt, iter};

/// An owned snapshot of an error and its source chain.
///
/// Each level stores the [`Display`](fmt::Display) text of one error; the
/// next level is the snapshot of its [`Error::source`]. The original error is
/// not kept, so a `Detached` is `Send + Sync + 'static + Clone` whatever the
/// original was. That makes it the carrier for an error that has to cross a
/// task, a channel, or the wire.
///
/// It is an [`Error`] itself, so every strategy that walks a source chain
/// ([`OneLine`](crate::OneLine), [`Chain`](crate::Chain), custom ones) renders
/// it exactly as it would render the original.
///
/// Take a snapshot with [`FormatError::detach`](crate::FormatError::detach),
/// or rebuild a chain from its messages with [`Detached::new`] and
/// [`Detached::with_source`].
///
/// ```
/// use errortools::{Detached, FormatError};
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("Failed to load config")]
/// struct LoadError(#[source] ParseError);
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("Unexpected token at line {0}")]
/// struct ParseError(usize);
///
/// let error = LoadError(ParseError(3));
/// let detached: Detached = error.detach();
///
/// assert_eq!(detached.to_string(), "Failed to load config");
/// assert_eq!(
///     detached.one_line().to_string(),
///     error.one_line().to_string()
/// );
/// assert_eq!(
///     detached,
///     Detached::new("Failed to load config")
///         .with_source(Detached::new("Unexpected token at line 3"))
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Detached {
    message: String,
    source: Option<Box<Detached>>,
}

impl Detached {
    /// Creates a snapshot of a single error with no source.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// Sets `source` as the next level of the chain, replacing any previous one.
    #[must_use]
    pub fn with_source(mut self, source: Detached) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// The `Display` text of this level, without its sources.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The next level of the chain; the typed twin of [`Error::source`].
    pub fn cause(&self) -> Option<&Detached> {
        self.source.as_deref()
    }

    /// Snapshots `error` and its whole source chain.
    pub(crate) fn snapshot<E: Error + ?Sized>(error: &E) -> Self {
        // Built from the innermost source outwards, so a long chain costs no
        // stack depth here.
        let sources: Vec<String> = iter::successors(error.source(), |e| Error::source(*e))
            .map(|e| e.to_string())
            .collect();
        let source = sources.into_iter().rev().fold(None, |source, message| {
            Some(Box::new(Detached { message, source }))
        });
        Self {
            message: error.to_string(),
            source,
        }
    }
}

/// Prints the top-level message only, like any other error; use a strategy
/// to render the chain.
impl fmt::Display for Detached {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for Detached {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString as _;
    use core::error::Error as _;

    use thiserror::Error;

    use super::Detached;
    use crate::{Chain, FormatError as _, tests::Inner};

    #[derive(Error, Debug)]
    #[error("mid")]
    struct Mid(#[source] Inner);

    #[derive(Error, Debug)]
    #[error("top")]
    struct Top(#[source] Mid);

    fn three_levels() -> Top {
        Top(Mid(Inner::A))
    }

    #[test]
    fn test_detach_renders_like_original_one_line() {
        let error = three_levels();
        let detached = error.detach();
        assert_eq!(detached.one_line().to_string(), "top: mid: InnerA");
        assert_eq!(
            detached.one_line().to_string(),
            error.one_line().to_string()
        );
    }

    #[test]
    fn test_detach_renders_like_original_chain() {
        let error = three_levels();
        let detached = error.detach();
        assert_eq!(detached.chain().to_string(), error.chain().to_string());
        assert_eq!(
            detached.formatted::<Chain<crate::Ascii>>().to_string(),
            error.formatted::<Chain<crate::Ascii>>().to_string()
        );
    }

    #[test]
    fn test_detach_two_levels_and_leaf() {
        let two = Mid(Inner::B).detach();
        assert_eq!(two.one_line().to_string(), "mid: InnerB");

        let leaf = Inner::A.detach();
        assert_eq!(leaf, Detached::new("InnerA"));
        assert!(leaf.source().is_none());
    }

    #[test]
    fn test_display_is_top_message_only() {
        let detached = three_levels().detach();
        assert_eq!(detached.to_string(), "top");
        assert_eq!(detached.message(), "top");
    }

    #[test]
    fn test_levels_are_reachable() {
        let detached = three_levels().detach();
        let mid = detached.cause().unwrap();
        assert_eq!(mid.message(), "mid");
        let leaf = mid.cause().unwrap();
        assert_eq!(leaf.message(), "InnerA");
        assert_eq!(leaf.cause(), None);
        assert_eq!(detached.source().unwrap().to_string(), "mid");
    }

    #[test]
    fn test_from_parts_equals_snapshot() {
        let built = Detached::new("top")
            .with_source(Detached::new("mid").with_source(Detached::new("InnerA")));
        assert_eq!(built, three_levels().detach());
        assert_eq!(built.one_line().to_string(), "top: mid: InnerA");
    }

    #[test]
    fn test_with_source_replaces_previous() {
        let detached = Detached::new("top")
            .with_source(Detached::new("old"))
            .with_source(Detached::new("new"));
        assert_eq!(detached.one_line().to_string(), "top: new");
    }

    #[test]
    fn test_clone_and_eq() {
        let detached = three_levels().detach();
        let cloned = detached.clone();
        assert_eq!(cloned, detached);
        assert_ne!(detached, Detached::new("top"));
        assert_ne!(detached, Mid(Inner::A).detach());
    }

    #[test]
    fn test_detach_dyn_error_and_detached_itself() {
        let error = three_levels();
        let dynamic: &(dyn core::error::Error + 'static) = &error;
        let detached = dynamic.detach();
        assert_eq!(detached, error.detach());
        assert_eq!(detached.detach(), detached);
    }

    #[test]
    fn test_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<Detached>();
    }
}
