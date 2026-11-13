//! Client side: [`Status`] back to errortools types.

use errortools::Detached;
use tonic::Status;
use tonic_types::StatusExt as _;

use crate::{DetachedErrors, TREE_TRAILER, tree};

mod sealed {
    pub trait Sealed {}
    impl Sealed for tonic::Status {}
}

/// Reads errortools data back out of a [`Status`].
///
/// A foreign trait cannot be implemented for two foreign types, so this is an
/// extension trait and not `TryFrom<&Status> for Detached`. It is sealed.
///
/// ```
/// use errortools::FormatError;
/// use errortools_tonic::{Code, ErrorStatusExt, IntoStatus};
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("Failed to load the profile")]
/// struct LoadError(#[source] std::io::Error);
///
/// impl Code for LoadError {}
///
/// let error = LoadError(std::io::Error::other("disk full"));
/// let expected = error.chain().to_string();
/// let status = error.into_detailed_status();
///
/// // On the client:
/// let detached = status.to_detached();
/// assert_eq!(detached.chain().to_string(), expected);
/// ```
pub trait ErrorStatusExt: sealed::Sealed {
    /// Rebuilds the server's error chain.
    ///
    /// The levels come from the stack entries of a `google.rpc.DebugInfo`
    /// detail. Without one (the server did not send the chain, the details do
    /// not decode, or a proxy dropped them) the result is a single level
    /// holding [`Status::message`]. This never fails.
    ///
    /// A detail with more than 1024 entries is treated as absent. A chain is
    /// dropped recursively, so an unbounded one from an untrusted server
    /// could overflow the stack; a real chain is nowhere near that long.
    ///
    /// The entries are trusted to be an error chain. A server that does not
    /// use this crate may put a real stack trace there, and that is what the
    /// result will then hold.
    fn to_detached(&self) -> Detached;

    /// Rebuilds the server's aggregate, if the status carries one.
    ///
    /// A valid [`TREE_TRAILER`] wins and gives the nested structure. Otherwise
    /// the `google.rpc.BadRequest` field violations give a flat aggregate,
    /// one leaf per violation. `None` when there is neither.
    fn to_many_errors(&self) -> Option<DetachedErrors>;

    /// The server's hint: the description of the first `google.rpc.Help`
    /// link that has no URL.
    fn hint(&self) -> Option<String>;
}

impl ErrorStatusExt for Status {
    fn to_detached(&self) -> Detached {
        self.get_details_debug_info()
            .and_then(|info| tree::detached_from_messages(info.stack_entries))
            .unwrap_or_else(|| Detached::new(self.message()))
    }

    fn to_many_errors(&self) -> Option<DetachedErrors> {
        let tree = self
            .metadata()
            .get_bin(TREE_TRAILER)
            .and_then(|value| value.to_bytes().ok())
            .and_then(|bytes| tree::decode(&bytes));
        tree.or_else(|| {
            let violations = self.get_details_bad_request()?.field_violations;
            (!violations.is_empty()).then(|| {
                violations
                    .into_iter()
                    .map(|violation| (violation.field, Detached::new(violation.description)))
                    .collect()
            })
        })
    }

    fn hint(&self) -> Option<String> {
        let links = self.get_details_help()?.links;
        links
            .into_iter()
            .find(|link| link.url.is_empty())
            .map(|link| link.description)
    }
}
