//! Multiline re-indentation for layout-owning strategies.
//!
//! Foreign content (error messages, custom strategies) may embed `\n`. A
//! strategy that owns a 2-D layout ([`Tree`](crate::Tree),
//! [`List`](crate::List), [`Bullets`](crate::Bullets), [`Chain`](crate::Chain))
//! writes such content through [`indented`] so every physical line stays under
//! the strategy's own column instead of spilling flush-left. Streams
//! line-by-line — no allocation, available without `alloc`.
//!
//! These are the same primitives the built-in shapes use; they are public so a
//! custom [`Format`](crate::Format) strategy can re-indent its own foreign
//! content identically.
//!
//! # Design note
//!
//! The `Indented` adapter below is a hand-rolled slice of two well-known
//! things: std's own
//! internal `core::fmt::builders::PadAdapter` (which indents nested `{:#?}`),
//! and the `nest`/`line` of a Wadler-style pretty-printer. The *concept* —
//! re-emit a continuation prefix after each newline — is forced by the goal
//! (stream foreign content into a 2-D layout) and survives any redesign. The
//! *encoding* is contingent: a `Doc` algebra or a scoped prefix-stack writer
//! could absorb [`indented`] and [`Repeat`] (and `Tree`'s `Pad`) into one
//! mechanism, but that relocates the complexity (allocation, a `Doc` to
//! maintain) rather than removing it. [`Repeat`] (uniform unit × N, used by
//! `Chain`/`List`/`Bullets`) and `Tree`'s `Pad` (heterogeneous per-level bars
//! vs gaps) are two genuine prefix *shapes*, not redundancy.

use core::fmt::{self, Display, Write};

/// A [`fmt::Write`] adapter that re-emits `prefix` after every newline, so
/// content spanning multiple physical lines keeps its column.
struct Indented<'a, 'b, P: Display> {
    inner: &'a mut fmt::Formatter<'b>,
    prefix: P,
}

impl<P: Display> Write for Indented<'_, '_, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut lines = s.split('\n');
        if let Some(first) = lines.next() {
            self.inner.write_str(first)?;
        }
        for line in lines {
            write!(self.inner, "\n{}", self.prefix)?;
            self.inner.write_str(line)?;
        }
        Ok(())
    }
}

/// Writes `content` to `f`, re-indenting any embedded newlines to `prefix`.
pub fn indented(
    f: &mut fmt::Formatter<'_>,
    prefix: impl Display,
    content: impl Display,
) -> fmt::Result {
    write!(Indented { inner: f, prefix }, "{content}")
}

/// [`Display`]-able repetition of a unit string — a reusable continuation
/// prefix for [`indented`] (unlike `itertools`' one-shot `format`, this can be
/// formatted once per embedded newline).
///
/// `Repeat(unit, n)` renders `unit` written `n` times.
pub struct Repeat(pub &'static str, pub usize);

impl Display for Repeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.1 {
            f.write_str(self.0)?;
        }
        Ok(())
    }
}
