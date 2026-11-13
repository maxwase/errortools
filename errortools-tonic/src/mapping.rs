//! Server side: error to [`Status`].

use std::{error::Error, num::NonZeroUsize};

use errortools::{FormatError as _, FormattedRef, Node, Suggest, Suggestion, sources};
use tonic::{
    Status,
    metadata::{BinaryMetadataValue, MetadataMap},
};
use tonic_types::{ErrorDetails, FieldViolation, HelpLink, StatusExt as _};

use crate::{Code, DetachedErrors, tree};

/// Name of the binary trailer that carries a nested aggregate.
///
/// See [`Mapping::with_tree`] for when it is sent and the crate docs for its
/// encoding.
pub const TREE_TRAILER: &str = "errortools-tree-bin";

/// Default cap for the [`TREE_TRAILER`] value: 4 KiB on the wire.
///
/// HTTP/2 peers and proxies commonly limit a whole header list to 8 or
/// 16 KiB, and the trailers also hold `grpc-message` and
/// `grpc-status-details-bin`, so the tree gets a quarter to a half of the
/// smallest common budget.
pub const DEFAULT_TREE_CAP: NonZeroUsize = NonZeroUsize::new(4096).unwrap();

/// How much of an error's text is sent to the client.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Detail {
    /// Only the top-level `Display` text of the error, and of each aggregate
    /// leaf. No source text leaves the server. This is the default.
    #[default]
    TopLevel,
    /// The whole source chain: `grpc-message` is the
    /// [`OneLine`](errortools::OneLine) rendering, a `google.rpc.DebugInfo`
    /// detail carries one stack entry per level, and aggregate leaves carry
    /// their chains too.
    ///
    /// Source text is written for operators, not for callers. It regularly
    /// contains file paths, host names, SQL and the like, and with this
    /// variant all of it reaches whoever calls the service. Use it between
    /// services you trust, not on a public API.
    Chain,
}

/// Configuration of the error to [`Status`] mapping.
///
/// The default sends the least: the code, the top-level message, the hint,
/// and the top-level text of aggregate leaves. Everything else is opted into.
///
/// ```
/// use errortools_tonic::{Detail, Mapping};
///
/// /// Used by handlers of an internal service.
/// const INTERNAL: Mapping = Mapping::detailed().with_tree();
///
/// assert_eq!(INTERNAL.detail(), Detail::Chain);
/// assert_eq!(Mapping::new().detail(), Detail::TopLevel);
/// assert_eq!(Mapping::new().tree_cap(), None);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Mapping {
    detail: Detail,
    tree_cap: Option<NonZeroUsize>,
}

impl Mapping {
    /// The default mapping: [`Detail::TopLevel`], no tree trailer.
    pub const fn new() -> Self {
        Self {
            detail: Detail::TopLevel,
            tree_cap: None,
        }
    }

    /// A mapping that sends the whole source chain; see [`Detail::Chain`]
    /// for what that exposes.
    pub const fn detailed() -> Self {
        Self::new().with_detail(Detail::Chain)
    }

    /// Sets how much error text is sent.
    pub const fn with_detail(mut self, detail: Detail) -> Self {
        self.detail = detail;
        self
    }

    /// Enables the [`TREE_TRAILER`] with the [`DEFAULT_TREE_CAP`].
    ///
    /// `google.rpc.BadRequest` is a flat list, so nested groups of a
    /// [`ManyErrors`](errortools::ManyErrors) are flattened there. The
    /// trailer keeps the structure (and, with [`Detail::Chain`], each leaf's
    /// chain) for clients that use this crate.
    pub const fn with_tree(self) -> Self {
        self.with_tree_cap(DEFAULT_TREE_CAP)
    }

    /// Enables the [`TREE_TRAILER`] with a custom cap.
    ///
    /// The cap is the size of the trailer value as it is sent, base64
    /// included. A tree that does not fit is not sent at all; it is never
    /// truncated. The flat `BadRequest` violations are sent either way.
    pub const fn with_tree_cap(mut self, cap: NonZeroUsize) -> Self {
        self.tree_cap = Some(cap);
        self
    }

    /// How much error text this mapping sends.
    pub const fn detail(&self) -> Detail {
        self.detail
    }

    /// The tree trailer cap, or `None` when the trailer is off.
    pub const fn tree_cap(&self) -> Option<NonZeroUsize> {
        self.tree_cap
    }

    /// Builds the [`Status`] for `error`.
    ///
    /// [`IntoStatus`] is the same conversion in method position.
    pub fn status<E: Error + Code + ?Sized>(&self, error: &E) -> Status {
        let mut details = ErrorDetails::new();
        let mut has_details = false;
        let mut metadata = MetadataMap::new();

        let message = match self.detail {
            Detail::TopLevel => error.to_string(),
            Detail::Chain => {
                let entries: Vec<String> = sources(&error).map(ToString::to_string).collect();
                details.set_debug_info(entries, "");
                has_details = true;
                error.one_line().to_string()
            }
        };

        if let Some(hint) = error.suggest().and_then(render_hint) {
            // `Help` only has links, so the hint is a link without a URL.
            details.set_help(vec![HelpLink::new(hint, "")]);
            has_details = true;
        }

        if let Some(errors) = error.many_errors().filter(|errors| !errors.is_empty()) {
            let mut violations = Vec::new();
            collect_violations(&errors, self.detail, "", &mut violations);
            details.set_bad_request(violations);
            has_details = true;

            if let Some(value) = self.tree_value(&errors) {
                metadata.insert_bin(TREE_TRAILER, value);
            }
        }

        // An empty detail set would still be encoded as a `google.rpc.Status`
        // that repeats the code and the message, so it is skipped.
        if has_details {
            Status::with_error_details_and_metadata(error.code(), message, details, metadata)
        } else {
            Status::with_metadata(error.code(), message, metadata)
        }
    }

    /// The trailer value for `errors`, when the trailer is on and the value
    /// fits the cap.
    fn tree_value(&self, errors: &DetachedErrors) -> Option<BinaryMetadataValue> {
        let cap = self.tree_cap?;
        let value = BinaryMetadataValue::from_bytes(&tree::encode(errors, self.detail)?);
        (value.as_encoded_bytes().len() <= cap.get()).then_some(value)
    }
}

/// Renders a hint; `None` when it is empty.
fn render_hint(hint: &dyn Suggest) -> Option<String> {
    let hint = FormattedRef::<_, Suggestion>::new(hint).to_string();
    (!hint.is_empty()).then_some(hint)
}

/// Flattens `errors` into field violations. A leaf inside groups gets the
/// group labels as a dot-separated prefix of its field.
fn collect_violations(
    errors: &DetachedErrors,
    detail: Detail,
    prefix: &str,
    out: &mut Vec<FieldViolation>,
) {
    let path = |context: &str| match prefix {
        "" => context.to_owned(),
        prefix => format!("{prefix}.{context}"),
    };
    for node in errors {
        match node {
            Node::Leaf(leaf) => {
                let description = match detail {
                    Detail::TopLevel => leaf.error.message().to_owned(),
                    Detail::Chain => leaf.error.one_line().to_string(),
                };
                out.push(FieldViolation::new(path(&leaf.context), description));
            }
            Node::Group(group) => {
                collect_violations(&group.errors, detail, &path(&group.context), out);
            }
        }
    }
}

/// Converts an error into a [`Status`]; implemented for every
/// [`Error`] that implements [`Code`].
///
/// ```
/// use errortools_tonic::{Code, IntoStatus};
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("Failed to load the profile")]
/// struct LoadError(#[source] std::io::Error);
///
/// impl Code for LoadError {}
///
/// fn load() -> Result<(), LoadError> {
///     Err(LoadError(std::io::Error::other("disk full")))
/// }
///
/// fn handler() -> Result<(), tonic::Status> {
///     load().map_err(IntoStatus::into_status)?;
///     Ok(())
/// }
///
/// let status = handler().unwrap_err();
/// assert_eq!(status.code(), tonic::Code::Internal);
/// assert_eq!(status.message(), "Failed to load the profile");
/// ```
pub trait IntoStatus: Sized {
    /// Converts with an explicit [`Mapping`].
    fn into_status_with(self, mapping: &Mapping) -> Status;

    /// Converts with the default [`Mapping`]: the code, the top-level
    /// message, the hint, and flat aggregate leaves. No source text is sent.
    fn into_status(self) -> Status {
        self.into_status_with(&Mapping::new())
    }

    /// Converts with [`Mapping::detailed`], which sends the whole source
    /// chain to the client. Read [`Detail::Chain`] before using it.
    fn into_detailed_status(self) -> Status {
        self.into_status_with(&Mapping::detailed())
    }
}

impl<E: Error + Code> IntoStatus for E {
    fn into_status_with(self, mapping: &Mapping) -> Status {
        mapping.status(&self)
    }
}
