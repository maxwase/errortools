//! Shared test fixtures: error types and reusable [`Format`] strategies.
//!
//! Each module's `#[cfg(test)] mod tests` pulls types and formatters from
//! here so per-module tests stay focused on the unit under test rather than
//! re-declaring boilerplate.
#![cfg(test)]

use core::{
    error::Error as _,
    fmt::{self, Display, Formatter},
    hash::Hash,
};
use std::io;

use itertools::Itertools as _;
use thiserror::Error;

use super::*;

/// Inner leaf error used as the source for [`Error::Two`] / [`Error::Three`] and chain tests.
#[derive(Error, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Inner {
    #[error("InnerA")]
    A,
    #[error("InnerB")]
    B,
}

/// Middle error with two variants:
/// - [`Mid::Inner`]: wraps [`Inner`] as a source, display `"mid"`.
/// - [`Mid::Io`]: transparent wrapper around [`io::Error`], `From<io::Error>` provided.
#[derive(Error, Debug)]
pub enum Mid {
    #[error("mid")]
    Inner(#[source] Inner),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Top-level error covering all `#[source]`/`#[from]`/`#[error(transparent)]`/none combinations:
/// - [`Error::One`]: plain unit variant, no source.
/// - [`Error::Two`]: explicit `#[source]`; Display prints `"Two"` only.
/// - [`Error::Three`]: `#[from] io::Error`; Display prints `"Three"`.
/// - [`Error::WithCtx`]: wraps a [`WithContext`] as a source.
/// - [`Error::Many`]: wraps a [`ManyErrors`] as a source (alloc only).
/// - [`Error::Transparent`]: `#[error(transparent)]`; delegates Display and source to [`Mid`].
#[derive(Error, Debug)]
pub enum Error {
    #[error("One")]
    One,
    #[error("Two")]
    Two(#[source] Inner),
    #[error("Three")]
    Three(#[from] io::Error),
    #[error("WithCtx")]
    WithCtx(#[source] WithContext<&'static str, Inner>),
    #[cfg(feature = "alloc")]
    #[error("Many")]
    Many(#[source] ManyErrors<&'static str, Inner>),
    #[error(transparent)]
    Transparent(#[from] Mid),
}

impl Suggest for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::One => f.write_str("Try passing --help to see available options."),
            Self::Three(_) => {
                f.write_str("Check that the file path exists and permissions are correct.")
            }
            Self::Two(_) | Self::WithCtx(_) | Self::Transparent(_) => Ok(()),
            #[cfg(feature = "alloc")]
            Self::Many(_) => Ok(()),
        }
    }
}

/// Error with no suggestion, exercises the default [`Suggest`] impl.
#[derive(Error, Debug)]
#[error("plain")]
pub struct NoHint;

impl Suggest for NoHint {}

/// Reusable [`Format`] strategy: joins error chain with `" -> "`.
#[derive(Debug, Default)]
pub struct Arrow;
impl<E: core::error::Error> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(error).format(" -> "))
    }
}

/// Reusable [`Format`] strategy: uppercases the top-level error's `Display`.
#[derive(Debug, Default)]
pub struct Upper;
impl<E: core::error::Error + ?Sized> Format<E> for Upper {
    fn fmt(error: &E, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", error.to_string().to_uppercase())
    }
}

/// [`WithContext`] formatter producing `"[ctx] err"`.
#[derive(Debug, Default)]
pub struct Bracketed;
impl<C: Display, E: Display, F> Format<WithContext<C, E, F>> for Bracketed {
    fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", w.context, w.error)
    }
}

/// [`WithContext`] formatter producing `"ctx -> err"`.
#[derive(Debug, Default)]
pub struct WcArrow;
impl<C: Display, E: Display, F> Format<WithContext<C, E, F>> for WcArrow {
    fn fmt(w: &WithContext<C, E, F>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", w.context, w.error)
    }
}

// --- lib-level integration tests ---

fn _assert_derive_traits() {
    #[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
    struct DummyError;
    impl fmt::Display for DummyError {
        fn fmt(&self, _: &mut Formatter<'_>) -> fmt::Result {
            Ok(())
        }
    }
    impl core::error::Error for DummyError {}

    fn assert_all<T: Clone + Copy + Default + PartialEq + Eq + Hash + Send + Sync>() {}
    assert_all::<Formatted<DummyError, OneLine>>();
    assert_all::<Formatted<DummyError, Chain>>();
    assert_all::<DisplaySwapDebug<DummyError>>();
    assert_all::<OneLine>();
    assert_all::<Chain>();

    // The phantom strategy param must NOT leak a `Trait` bound: these must
    // compile even though `NoTraits` implements nothing.
    struct NoTraits;
    assert_all::<Formatted<DummyError, NoTraits>>();
    assert_all::<Chain<NoTraits>>();
    assert_all::<Add<NoTraits, NoTraits>>();
    #[cfg(feature = "alloc")]
    assert_all::<Tree<NoTraits, true>>();

    // `WithContext` has no `Default`, but its other auto-traits must still be
    // `F`-free.
    fn assert_no_default<T: Clone + Copy + PartialEq + Eq + Hash + Send + Sync>() {}
    assert_no_default::<WithContext<DummyError, DummyError, NoTraits>>();
}

#[test]
fn test_user_output() {
    assert_eq!(Error::One.one_line().to_string(), "One");
    assert_eq!(Error::Two(Inner::A).one_line().to_string(), "Two: InnerA");
    assert_eq!(
        Error::Three(io::Error::new(io::ErrorKind::PermissionDenied, "test"))
            .one_line()
            .to_string(),
        "Three: test"
    );
    // `#[from]` generates the conversion used at `?` boundaries.
    let from_io: Error = io::Error::other("boom").into();
    assert_eq!(from_io.one_line().to_string(), "Three: boom");
}

#[test]
fn test_dyn_error() {
    let error = Error::Two(Inner::A);

    let dyn_ref: &dyn core::error::Error = &error;
    assert_eq!(dyn_ref.one_line().to_string(), "Two: InnerA");

    let boxed: Box<dyn core::error::Error> = Box::new(Error::Two(Inner::B));
    assert_eq!(boxed.one_line().to_string(), "Two: InnerB");

    let send_sync: &(dyn core::error::Error + Send + Sync) = &error;
    assert_eq!(send_sync.one_line().to_string(), "Two: InnerA");
}

/// `Formatted`'s `Display` requires only `F: Format<E>` — non-error values
/// render fine through a compatible strategy.
#[test]
fn test_formatted_non_error_value() {
    assert_eq!(
        Formatted::<_, AsDisplay>::new("plain text").to_string(),
        "plain text"
    );
    assert_eq!(Formatted::<_, AsDisplay>::new(42).to_string(), "42");
}

#[test]
fn test_custom_format() {
    assert_eq!(Error::Two(Inner::A).formatted::<Upper>().to_string(), "TWO");
}

#[test]
fn test_with_ctx_variant() {
    let e = Error::WithCtx(WithContext::new("step", Inner::A));
    assert_eq!(e.to_string(), "WithCtx");
    assert_eq!(e.one_line().to_string(), "WithCtx: step: InnerA");
}

#[cfg(feature = "alloc")]
#[test]
fn test_many_variant() {
    let mut errs = ManyErrors::new();
    errs.push("a", Inner::A);
    errs.push("b", Inner::B);
    let e = Error::Many(errs);
    assert_eq!(e.to_string(), "Many");
    // ManyErrors is the source; one_line walks the chain and embeds
    // ManyErrors::Display, now the shallow single-line Summary.
    assert_eq!(
        e.one_line().to_string(),
        "Many: 2 errors: a: InnerA; b: InnerB"
    );
}

// --- transparent ---

#[test]
fn test_transparent_display_collapses_wrapper() {
    // The word "Transparent" never appears in rendered output.
    assert_eq!(Error::Transparent(Mid::Inner(Inner::A)).to_string(), "mid",);
    assert_eq!(
        Error::Transparent(Mid::Io(io::Error::other("disk full"))).to_string(),
        "disk full",
    );
    // Two transparent layers: Error::Transparent(Mid::Io(...)) drills to io message.
    assert_eq!(
        Error::Transparent(Mid::Io(io::Error::other("deep"))).to_string(),
        "deep",
    );
}

#[test]
fn test_transparent_source_delegates_not_wraps() {
    // source() is delegated, not wrapped — the transparent variant itself is
    // NOT a node in the source chain.

    // Mid::Inner(Inner::A).source() = Some(Inner::A).
    // Error::Transparent(Mid::Inner(Inner::A)).source() follows Mid's source.
    let e = Error::Transparent(Mid::Inner(Inner::A));
    let src = e.source().expect("Inner::A must be the source");
    assert_eq!(src.to_string(), "InnerA");

    // io::Error::other has no source; Mid::Io delegates, so None.
    let e2 = Error::Transparent(Mid::Io(io::Error::other("boom")));
    assert!(e2.source().is_none());

    // Double-transparent: Error::Transparent(Mid::Io) → mid.source() → None.
    let e3 = Error::Transparent(Mid::Io(io::Error::other("deep")));
    assert!(e3.source().is_none());
}

#[test]
fn test_transparent_chain_never_shows_wrapper_name() {
    // one_line and chain walk the chain. "Transparent" must not appear.
    let e = Error::Transparent(Mid::Inner(Inner::A));
    let one = e.one_line().to_string();
    let chain_out = e.chain().to_string();
    assert!(
        !one.contains("Transparent"),
        "one_line should not contain 'Transparent': {one}"
    );
    assert!(
        !chain_out.contains("Transparent"),
        "chain should not contain 'Transparent': {chain_out}"
    );
    assert_eq!(one, "mid: InnerA");
    assert_eq!(chain_out, "mid\n└─ InnerA");
}

#[test]
fn test_transparent_two_vs_transparent_same_source_different_display() {
    // Error::Two shows its own label; Error::Transparent(Mid::Inner) shows Mid's.
    // Same source (Inner::A), different top-level display.
    let two = Error::Two(Inner::A);
    let transp = Error::Transparent(Mid::Inner(Inner::A));

    assert_eq!(two.to_string(), "Two");
    assert_eq!(transp.to_string(), "mid");

    assert_eq!(two.one_line().to_string(), "Two: InnerA");
    assert_eq!(transp.one_line().to_string(), "mid: InnerA");

    assert_eq!(
        two.source().unwrap().to_string(),
        transp.source().unwrap().to_string(),
    );
}

#[test]
fn test_from_io_routes_differ_by_variant() {
    // io::Error -> Error via #[from] on Three: direct route.
    let via_three: Error = io::Error::other("direct").into();
    assert!(matches!(via_three, Error::Three(_)));
    assert_eq!(via_three.to_string(), "Three"); // NOT transparent — own display

    // io::Error -> Mid -> Error: two-hop From, lands in Transparent.
    let via_mid: Error = Mid::from(io::Error::other("via mid")).into();
    assert!(matches!(via_mid, Error::Transparent(Mid::Io(_))));
    assert_eq!(via_mid.to_string(), "via mid"); // transparent — io message

    // The two routes coexist; neither hides the other.
}

#[test]
fn test_suggest_not_delegated_through_transparent() {
    // #[error(transparent)] delegates Display + source — but NOT Suggest.
    // Suggest::fmt is always dispatched on the concrete outer type.
    let e = Error::Transparent(Mid::Inner(Inner::A));

    // Display is transparent (collapses to Mid's message).
    assert_eq!(e.to_string(), "mid");
    // Suggestion uses Error's Transparent arm — returns "".
    assert_eq!(e.suggestion().to_string(), "");

    // Control: hint-bearing variants still work.
    assert_ne!(Error::One.suggestion().to_string(), "");
    assert_ne!(
        Error::Three(io::Error::other("x")).suggestion().to_string(),
        "",
    );
}
