//! Display-adapter wrapper for [`Path`]-like values.
//! This is an experimental helper module. Prefer defining printing strategies
//! that call `Path::display` directly, e.g. via [`ContextFormat<C, E>`](crate::with_context::ContextFormat) for [`WithContext`](crate::WithContext).
//! See [`WithPath`](crate::with_context::WithPath) to get the idea.

use core::fmt;
use std::path::Path;

/// Wrapper that gives a [`Path`]-like value a [`fmt::Display`] impl (via
/// [`Path::display`]) so it can be used in contexts that require `Display`,
/// e.g. as the context slot of [`WithContext`](crate::WithContext) under the
/// default [`Colon`](crate::with_context::Colon) strategy. Prefer
/// [`PathColon`](crate::with_context::PathColon) when you only need a path-aware strategy.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayPath<T>(T);

impl<P: AsRef<Path>> fmt::Display for DisplayPath<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.as_ref().display().fmt(f)
    }
}

impl<P: AsRef<Path>> fmt::Debug for DisplayPath<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.as_ref().fmt(f)
    }
}

impl<T> From<T> for DisplayPath<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
