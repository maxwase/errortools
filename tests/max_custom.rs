//! Maximum-customization integration test for the `ManyErrors` rendering path.
//!
//! `ManyErrors` lives behind the `alloc` feature, so the whole test is gated.
#![cfg(feature = "alloc")]
//!
//!
//! Nothing here relies on a crate-provided strategy or a defaulted generic:
//!
//! - every `ManyErrors` / `WithContext` type parameter is spelled out;
//! - the leaf error carries a **2-level** source chain;
//! - the leaf's own `WithContext` uses one custom strategy (`TopFmt`) while the
//!   `WithContext` one level deeper in the chain uses a *different* one
//!   (`InnerFmt`);
//! - group labels use a third custom strategy (`GroupFmt`);
//! - the tree is drawn with a hand-rolled `TreeConnectors` glyph set (`Pipes`),
//!   not `Unicode`/`Ascii`.

use core::fmt::{self, Display, Formatter};

use errortools::{Connectors, Format, ManyErrors, Tree, TreeConnectors, WithContext};
use pretty_assertions::assert_eq;
use thiserror::Error;

// ── Error types: a 2-level source chain sits under every leaf ──────────────────

#[derive(Debug, Error)]
#[error("disk full")]
struct Bottom;

#[derive(Debug, Error)]
#[error("write failed")]
struct MidErr(#[source] Bottom);

/// Top-level leaf error. Its source is an *inner* [`WithContext`] tagged with a
/// strategy (`InnerFmt`) different from the leaf's own (`TopFmt`).
#[derive(Debug, Error)]
#[error("operation failed")]
struct TopErr(#[source] WithContext<&'static str, MidErr, InnerFmt>);

// ── Three distinct, fully custom Format strategies ─────────────────────────────

/// Leaf `WithContext` strategy (top level inside `ManyErrors`): `"ctx ▸ err"`.
struct TopFmt;
impl<C: Display, E: Display, WCF> Format<WithContext<C, E, WCF>> for TopFmt {
    fn fmt(w: &WithContext<C, E, WCF>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ▸ {}", w.context, w.error)
    }
}

/// Inner `WithContext` strategy (one level deeper in the source chain):
/// `"ctx « err"`. Deliberately unlike `TopFmt` so the two are distinguishable
/// in the output.
struct InnerFmt;
impl<C: Display, E: Display, WCF> Format<WithContext<C, E, WCF>> for InnerFmt {
    fn fmt(w: &WithContext<C, E, WCF>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} « {}", w.context, w.error)
    }
}

/// A custom group-context type, distinct from the leaf context (`&str`).
#[derive(Debug)]
struct Region {
    code: &'static str,
    zone: u8,
}
impl Display for Region {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.code, self.zone)
    }
}

/// Group-label strategy: a label-only `Format<GC>` that wraps the label in braces.
struct GroupFmt;
impl<GC: Display> Format<GC> for GroupFmt {
    fn fmt(label: &GC, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{{{label}}}")
    }
}

// ── Custom tree connectors (no Unicode / Ascii) ────────────────────────────────

struct Pipes;
impl Connectors for Pipes {
    const LAST: &'static str = "\\__ ";
    const GAP: &'static str = "    ";
}
impl TreeConnectors for Pipes {
    const BRANCH: &'static str = "|__ ";
    const VERT: &'static str = "|   ";
}

// ── The fully-spelled aggregate type ───────────────────────────────────────────

// Heterogeneous: leaf context is `&str`, group context is the custom `Region`.
type Many = ManyErrors<&'static str, TopErr, Region, TopFmt, GroupFmt>;

/// A leaf error whose source chain is `TopErr → WithContext(InnerFmt) → MidErr → Bottom`.
fn nested(inner_ctx: &'static str) -> TopErr {
    TopErr(WithContext::new(inner_ctx, MidErr(Bottom)))
}

#[test]
fn fully_custom_tree() {
    // A group of two deep leaves, plus a sibling deep leaf at the top level.
    let mut inner: Many = ManyErrors::new();
    inner.push("config", nested("fsync"));
    inner.push("network", nested("connect"));

    let mut outer: Many = ManyErrors::new();
    outer.push_group(
        Region {
            code: "us-east",
            zone: 3,
        },
        inner,
    );
    outer.push("startup", nested("load"));

    // Custom connectors + explicit HEADER, no Display defaulting.
    let rendered = outer.formatted::<Tree<Pipes, true>>().to_string();

    let expected = "\
2 errors:
|__ {us-east#3} (2 errors):
|   |__ config ▸ operation failed
|   |   \\__ fsync « write failed
|   |       \\__ disk full
|   \\__ network ▸ operation failed
|       \\__ connect « write failed
|           \\__ disk full
\\__ startup ▸ operation failed
    \\__ load « write failed
        \\__ disk full";

    assert_eq!(rendered, expected);

    assert_eq!(
        outer.joined().to_string(),
        "2 errors: {us-east#3} (2 errors: config ▸ operation failed: fsync « write failed: disk full; network ▸ operation failed: connect « write failed: disk full); startup ▸ operation failed: load « write failed: disk full"
    );
}

// ── Malformed variant: error messages and strategies embed `\n` / `\t` ─────────
//
// The tree renderer re-indents every physical line of a node's content (and of
// each source) to its tree column, so embedded `\n`s no longer spill flush-left
// — continuation lines carry the ancestry prefix. Embedded `\t`s are passed
// through verbatim (no display-width handling). The expected strings are written
// multi-line (real newlines for the structural `\n`, `\t` escapes for the tabs)
// so the garbled layout is legible in source.

#[derive(Debug, Error)]
#[error("disk\n\tfull")] // newline + tab inside the message
struct BadBottom;

#[derive(Debug, Error)]
#[error("write\tfailed")] // tab inside the message
struct BadMid(#[source] BadBottom);

#[derive(Debug, Error)]
#[error("op\nfailed")] // newline inside the message
struct BadTop(#[source] WithContext<&'static str, BadMid, BadInnerFmt>);

/// Leaf strategy that injects a newline + tab between context and error.
struct BadTopFmt;
impl<C: Display, E: Display, WCF> Format<WithContext<C, E, WCF>> for BadTopFmt {
    fn fmt(w: &WithContext<C, E, WCF>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n\t-> {}", w.context, w.error)
    }
}

/// Inner strategy that injects a tab between context and error.
struct BadInnerFmt;
impl<C: Display, E: Display, WCF> Format<WithContext<C, E, WCF>> for BadInnerFmt {
    fn fmt(w: &WithContext<C, E, WCF>, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}\t=> {}", w.context, w.error)
    }
}

/// Group strategy (label-only `Format<GC>`) that leaves a trailing newline after the label.
struct BadGroupFmt;
impl<GC: Display> Format<GC> for BadGroupFmt {
    fn fmt(label: &GC, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[{label}]\n ")
    }
}

type BadMany = ManyErrors<&'static str, BadTop, Region, BadTopFmt, BadGroupFmt>;

fn bad_nested(inner_ctx: &'static str) -> BadTop {
    BadTop(WithContext::new(inner_ctx, BadMid(BadBottom)))
}

#[test]
fn malformed_messages_and_strategies() {
    let mut inner: BadMany = ManyErrors::new();
    inner.push("conf\tig", bad_nested("fsync"));
    inner.push("net\nwork", bad_nested("connect"));

    let mut outer: BadMany = ManyErrors::new();
    outer.push_group(
        Region {
            code: "us\teast",
            zone: 9,
        },
        inner,
    );
    outer.push("start\nup", bad_nested("load"));

    let rendered = outer.formatted::<Tree<Pipes, true>>().to_string();

    // Manual print: shows the actual garbled layout (run with `--nocapture`).
    println!("--- tree ---\n{rendered}");
    println!("--- one line ---\n{}", outer.joined());

    let expected_tree = "\
2 errors:
|__ [us\teast#9]
|     (2 errors):
|   |__ conf\tig
|   |   \t-> op
|   |   failed
|   |   \\__ fsync\t=> write\tfailed
|   |       \\__ disk
|   |           \tfull
|   \\__ net
|       work
|       \t-> op
|       failed
|       \\__ connect\t=> write\tfailed
|           \\__ disk
|               \tfull
\\__ start
    up
    \t-> op
    failed
    \\__ load\t=> write\tfailed
        \\__ disk
            \tfull";

    assert_eq!(rendered, expected_tree);

    // `joined` (the deep single-line strategy) keeps its own `; ` / `: `
    // separators and passes embedded control chars through untouched
    // (re-indentation only applies to the structural tree renderer).
    let expected_one_line = "\
2 errors: [us\teast#9]
  (2 errors: conf\tig
\t-> op
failed: fsync\t=> write\tfailed: disk
\tfull; net
work
\t-> op
failed: connect\t=> write\tfailed: disk
\tfull); start
up
\t-> op
failed: load\t=> write\tfailed: disk
\tfull";

    assert_eq!(outer.joined().to_string(), expected_one_line);
}

/// Same malformed fixture through [`List`](errortools::List): foreign content
/// (leaf chains, group labels) is re-indented to the list column; the
/// structural `\n{indent}{i}. ` markers stay raw.
#[test]
fn malformed_list() {
    let mut inner: BadMany = ManyErrors::new();
    inner.push("conf\tig", bad_nested("fsync"));
    inner.push("net\nwork", bad_nested("connect"));

    let mut outer: BadMany = ManyErrors::new();
    outer.push_group(
        Region {
            code: "us\teast",
            zone: 9,
        },
        inner,
    );
    outer.push("start\nup", bad_nested("load"));

    let rendered = outer.list().to_string();
    println!("--- list ---\n{rendered}");

    let expected = "\
2 errors:
1. [us\teast#9]
    (2 errors):
    1. conf\tig
      \t-> op
      failed: fsync\t=> write\tfailed: disk
      \tfull
    2. net
      work
      \t-> op
      failed: connect\t=> write\tfailed: disk
      \tfull
2. start
  up
  \t-> op
  failed: load\t=> write\tfailed: disk
  \tfull";

    assert_eq!(rendered, expected);
}

/// Same malformed fixture through [`Bullets`](errortools::Bullets): foreign
/// content is re-indented one unit past its bullet's row indent.
#[test]
fn malformed_bullets() {
    let mut inner: BadMany = ManyErrors::new();
    inner.push("conf\tig", bad_nested("fsync"));
    inner.push("net\nwork", bad_nested("connect"));

    let mut outer: BadMany = ManyErrors::new();
    outer.push_group(
        Region {
            code: "us\teast",
            zone: 9,
        },
        inner,
    );
    outer.push("start\nup", bad_nested("load"));

    let rendered = outer.bullets().to_string();
    println!("--- bullets ---\n{rendered}");

    let expected = "\
2 errors:
  • [us\teast#9]
      (2 errors):
    • conf\tig
      \t-> op
      failed: fsync\t=> write\tfailed: disk
      \tfull
    • net
      work
      \t-> op
      failed: connect\t=> write\tfailed: disk
      \tfull
  • start
    up
    \t-> op
    failed: load\t=> write\tfailed: disk
    \tfull";

    assert_eq!(rendered, expected);
}

/// A single deep leaf through [`Chain`](errortools::Chain): a source whose
/// message embeds `\n` re-indents its continuation lines to `depth × GAP`
/// (the top error stays flush — depth 0). Trailing-`\n` content leaves a
/// trailing prefix, same as the tree renderer.
#[test]
fn malformed_chain() {
    use errortools::FormatError as _;

    let error = bad_nested("fsync");
    let rendered = error.chain().to_string();
    println!("--- chain ---\n{rendered}");

    let expected = "\
op
failed
└─ fsync\t=> write\tfailed
   └─ disk
      \tfull";

    assert_eq!(rendered, expected);
}
