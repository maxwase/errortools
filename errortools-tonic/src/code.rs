//! The [`Code`] trait: what an error type tells the mapping about itself.

use std::{error::Error, fmt::Display};

use errortools::{Detached, FormatError as _, ManyErrors, Node, Suggest};

/// An owned aggregate: string contexts and [`Detached`] leaves.
///
/// This is the shape a [`ManyErrors`] takes on its way to the wire and the
/// shape the client gets back from
/// [`ErrorStatusExt::to_many_errors`](crate::ErrorStatusExt::to_many_errors).
/// It is an ordinary [`ManyErrors`], so `tree()`, `list()` and the other
/// renderings work on it.
pub type DetachedErrors = ManyErrors<String, Detached>;

/// Describes how an error maps onto a gRPC status.
///
/// Shaped like [`Suggest`]: every method has a default, so an empty impl is
/// enough to make an error convertible with
/// [`IntoStatus`](crate::IntoStatus), and a type overrides only what it needs.
///
/// Like [`Suggest`], the trait is dispatched on the top-level error only. The
/// source chain is never searched for a code, a hint or an aggregate.
///
/// ```
/// use errortools_tonic::Code;
///
/// #[derive(Debug, thiserror::Error)]
/// enum Error {
///     #[error("User {0} does not exist")]
///     NoSuchUser(u64),
///     #[error("Failed to query the database")]
///     Database(#[source] std::io::Error),
/// }
///
/// impl Code for Error {
///     fn code(&self) -> tonic::Code {
///         match self {
///             Error::NoSuchUser(_) => tonic::Code::NotFound,
///             Error::Database(_) => tonic::Code::Internal,
///         }
///     }
/// }
///
/// assert_eq!(Error::NoSuchUser(7).code(), tonic::Code::NotFound);
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no gRPC status mapping",
    label = "`{Self}` does not implement `errortools_tonic::Code`",
    note = "implement `Code` for `{Self}`; an empty impl maps it to `tonic::Code::Internal`"
)]
pub trait Code {
    /// The gRPC status code (`grpc-status`). Defaults to
    /// [`tonic::Code::Internal`].
    fn code(&self) -> tonic::Code {
        tonic::Code::Internal
    }

    /// The error's [`Suggest`] hint, sent as a `google.rpc.Help` detail.
    ///
    /// Defaults to `None`. A type that implements [`Suggest`] opts in by
    /// returning `Some(self)`. A hint that renders as an empty string is not
    /// sent.
    fn suggest(&self) -> Option<&dyn Suggest> {
        None
    }

    /// The aggregate this error stands for, sent as `google.rpc.BadRequest`
    /// field violations (and as the tree trailer when that is enabled).
    ///
    /// Defaults to `None`. [`ManyErrors`] itself returns its leaves. A
    /// wrapper that holds a [`ManyErrors`] exposes it explicitly with
    /// [`detach_leaves`]; an aggregate that is only reachable through
    /// [`Error::source`] is never found, because [`ManyErrors`] cannot be
    /// seen through `dyn Error`. It then travels as its one-line summary.
    fn many_errors(&self) -> Option<DetachedErrors> {
        None
    }
}

impl<T: Code + ?Sized> Code for &T {
    fn code(&self) -> tonic::Code {
        (**self).code()
    }

    fn suggest(&self) -> Option<&dyn Suggest> {
        (**self).suggest()
    }

    fn many_errors(&self) -> Option<DetachedErrors> {
        (**self).many_errors()
    }
}

/// A bare aggregate maps to [`tonic::Code::InvalidArgument`], the code that
/// `google.rpc.BadRequest` is meant to accompany, and exposes its leaves.
///
/// Wrap the aggregate in your own error type and implement [`Code`] for that
/// type when the failures are not the caller's fault.
impl<C: Display, E: Error, GC: Display, F, GF> Code for ManyErrors<C, E, GC, F, GF> {
    fn code(&self) -> tonic::Code {
        tonic::Code::InvalidArgument
    }

    fn many_errors(&self) -> Option<DetachedErrors> {
        Some(detach_leaves(self))
    }
}

/// Copies an aggregate into its owned wire shape: contexts become strings
/// through [`Display`], every leaf becomes a [`Detached`] snapshot of its
/// source chain, and nested groups keep their structure.
///
/// Use it to implement [`Code::many_errors`] for a wrapper type:
///
/// ```
/// use errortools::ManyErrors;
/// use errortools_tonic::{Code, DetachedErrors, detach_leaves};
///
/// #[derive(Debug, thiserror::Error)]
/// #[error("Invalid request")]
/// struct Invalid(ManyErrors<&'static str, std::num::ParseIntError>);
///
/// impl Code for Invalid {
///     fn code(&self) -> tonic::Code {
///         tonic::Code::InvalidArgument
///     }
///
///     fn many_errors(&self) -> Option<DetachedErrors> {
///         Some(detach_leaves(&self.0))
///     }
/// }
/// ```
pub fn detach_leaves<C: Display, E: Error, GC: Display, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
) -> DetachedErrors {
    let mut detached = DetachedErrors::new();
    for node in errors {
        match node {
            Node::Leaf(leaf) => detached.push(leaf.context.to_string(), leaf.error.detach()),
            Node::Group(group) => {
                detached.push_group(group.context.to_string(), detach_leaves(&group.errors));
            }
        }
    }
    detached
}
