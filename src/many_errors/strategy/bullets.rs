//! [`Bullets`]: render a [`ManyErrors`] as a bulleted (`•`) list.
//!
//! `depth: usize` carries the nesting level; the visual indent is reconstructed
//! lazily with `repeat_n("  ", depth).format("")` — no `String` allocation.

use core::{
    error::Error,
    fmt::{self, Display},
    iter,
};

use itertools::Itertools;

use crate::{
    Format,
    many_errors::{ManyErrors, Node, Subgroup},
    with_context::WithContext,
};

use super::{impl_aggregate_format, inline_sources};

/// Aggregate strategy that renders a [`ManyErrors`] as a bulleted (`•`) list.
///
/// # Output example
/// ```text
/// 3 errors:
///   • a: InnerA
///   • b: InnerB
///   • c: InnerC
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bullets;

impl_aggregate_format!(Bullets, |errors, f| draw_bullets_many::<C, E, GC, F, GF>(
    errors, 0, f
));

/// Render `errors` as a bulleted list at nesting `depth`.
///
/// - `None` writes nothing.
/// - `One` delegates to [`draw_bullets_node`] with `with_bullet = false`: a lone
///   error is printed flush, without a leading `•`.
/// - `Many` writes the `"N errors:"` header, then recurses into each child at
///   `depth + 1` with `with_bullet = true` so every child gets its own bullet.
fn draw_bullets_many<C, E, GC, F, GF>(
    errors: &ManyErrors<C, E, GC, F, GF>,
    depth: usize,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    match errors {
        ManyErrors::None => Ok(()),
        ManyErrors::One(node) => draw_bullets_node::<C, E, GC, F, GF>(node, depth, false, f),
        ManyErrors::Many(nodes) => {
            write!(f, "{} errors:", nodes.len())?;
            for node in nodes {
                draw_bullets_node::<C, E, GC, F, GF>(node, depth + 1, true, f)?;
            }
            Ok(())
        }
    }
}

/// Render a single node, optionally prefixed with its own bullet.
///
/// When `with_bullet` is set, first writes `"\n{indent}• "` where `indent` is
/// `depth` copies of `"  "` (lazy `repeat_n`, no allocation). Then:
/// - `Leaf` → `{w}` plus the inline source chain via [`inline_sources`];
/// - `Group` empty → `"{w}: (no errors)"`;
/// - `Group` single child → `"{w}: "` then recurse at the same `depth` with
///   `with_bullet = false`, so the child sits inline after the label;
/// - `Group` many children → `"{w} (N errors):"` header, then each child
///   recurses at `depth + 1` with its own bullet.
fn draw_bullets_node<C, E, GC, F, GF>(
    node: &Node<C, E, GC, F, GF>,
    depth: usize,
    with_bullet: bool,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    C: Display,
    E: Error + Display + 'static,
    F: Format<WithContext<C, E, F>>,
    GF: Format<Subgroup<C, E, GC, F, GF>>,
{
    if with_bullet {
        let indent = iter::repeat_n("  ", depth).format("");
        write!(f, "\n{indent}• ")?;
    }
    match node {
        Node::Leaf(w) => {
            write!(f, "{w}")?;
            inline_sources(w.error.source(), f)
        }
        Node::Group(w) => match w.error.as_ref() {
            ManyErrors::None => write!(f, "{w}: (no errors)"),
            ManyErrors::One(inner) => {
                write!(f, "{w}: ")?;
                draw_bullets_node::<C, E, GC, F, GF>(inner, depth, false, f)
            }
            ManyErrors::Many(nodes) => {
                write!(f, "{w} ({} errors):", nodes.len())?;
                for node in nodes {
                    draw_bullets_node::<C, E, GC, F, GF>(node, depth + 1, true, f)?;
                }
                Ok(())
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::many_errors::strategy::test_helpers::two_leaves;

    #[test]
    fn test_bullets_two_leaves() {
        let e = two_leaves();
        assert_eq!(
            e.bullets().to_string(),
            "2 errors:\n  • a: InnerA\n  • b: InnerB"
        );
    }
}
