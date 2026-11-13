# Aggregating failures: `ManyErrors`

Read this when building an aggregate by hand, grouping failures, picking a
render shape, or writing a custom shape. The default pattern
(`collect_results` plus a variant that renders the tree) is in `SKILL.md`.
Needs the `alloc` feature.

`ManyErrors<C, E, GC = C, F = Colon, GF = AsDisplay>` is a tree of nodes: a
leaf is a `WithContext<C, E, F>`, a group is a label `GC` over a nested
`ManyErrors`. `F` renders leaves, `GF` renders group labels. It allocates only
when a second error arrives.

## Building

| Call | Effect |
|---|---|
| `ManyErrors::collect_results(pairs)` | `(C, Result<T, E>)` pairs into `Result<Vec<T>, Self>`; consumes all input |
| `pairs.collect::<ManyErrors<_, _>>()` | from `(C, E)`, `(C, Result<T, E>)`, `WithContext`, or `Node` items; also `extend` |
| `push(c, e)`, `push_result(c, result)`, `push_some(c, option)` | append one leaf (the latter two only on `Err` / `Some`) |
| `push_group(label, sub)`, `sub.into_group(label)` | nest an aggregate under a label |
| `a + b`, `a += (c, result)` | merge two aggregates, or add a leaf |
| `into_result(ok)` | `Ok(ok)` when empty, else `Err(self)` |
| `err()` | `None` when empty, else `Some(self)` |
| `is_empty()`, `len()` | `len` counts direct children |
| `map_err(f)`, `map_context(f)` | convert every leaf error or context; `F` and `GF` are kept |
| `with_formats::<F, GF>()` | switch leaf and label strategies |

```rust
fn check_ports(ports: &[&'static str]) -> Result<(), Error> {
    ports
        .iter()
        .map(|&p| (p, p.parse::<u16>()))
        .collect::<ManyErrors<_, _>>()
        .into_result(())
        .map_err(Error::Ports)
}
```

Grouping:

```rust
let mut east = ManyErrors::new();
east.push("i-0b2", DeployError::Upload(io::Error::other("disk full")));
east.push("i-0c3", DeployError::Refused);

let mut all: ManyErrors<&str, DeployError> = ManyErrors::new();
all.push_group("us-east-1", east);
all.push("eu-west-1", DeployError::Refused);
```

For `Path` / `PathBuf` contexts set the leaf strategy to `PathColon` and give
groups a `Display` label type, because the label defaults to `C` and paths have
no `Display`: `ManyErrors<PathBuf, io::Error, &str, PathColon>`.

## Rendering

The inherent helpers borrow and walk each leaf's source chain. For `all` above:

`all.tree()` (and `all.chain()`, the same rendering):

```text
2 errors:
├─ us-east-1 (2 errors):
│  ├─ i-0b2: Failed to upload artifact
│  │  └─ disk full
│  └─ i-0c3: Connection refused
└─ eu-west-1: Connection refused
```

`all.list()` (`all.bullets()` is the same outline with `•` markers):

```text
2 errors:
1. us-east-1 (2 errors):
    1. i-0b2: Failed to upload artifact: disk full
    2. i-0c3: Connection refused
2. eu-west-1: Connection refused
```

`all.one_line()`:

```text
2 errors: us-east-1 (2 errors: i-0b2: Failed to upload artifact: disk full; i-0c3: Connection refused); eu-west-1: Connection refused
```

`{all}` (`Display`) is the same line without the leaves' sources (no
`: disk full`), following the convention that `Display` is an error's own
message. An empty aggregate renders `no errors`.

Other parameters go through `formatted`: `all.formatted::<Tree<Ascii, false>>()`
uses ASCII connectors and drops the count header.

## Where the deep rendering is lost

`ManyErrors` implements `Error` with `source()` always `None`, since siblings
have no single cause. Consequences:

- Through `dyn Error`, a generic `E: Error`, or as a `#[source]` of another
  error, only the shallow `Display` line is visible. The inherent `one_line()`
  and `chain()` shadow the `FormatError` ones only on a concrete `ManyErrors`.
- So do not mark the field `#[source]` or `#[from]`. Render it in the variant
  message: `#[error("Deploy failed:\n{}", .0.tree())]` for terminals,
  `#[error("Bad ports: {}", .0.one_line())]` for one-line logs.
- To nest one aggregate inside another, use `push_group`, not a source chain.

Using it in an `Error` position also requires `C: Debug` and `GC: Debug`.
Rendering alone does not.

## Children

Iterating `&errs` yields `&Node`; `Node::Leaf(w)` exposes `w.context` and
`w.error`, `Node::Group(g)` exposes `g.context` and `g.errors`. Each node is an
`Error`, so `node.one_line()` works. The iterator types are
`errortools::many_errors::{Iter, IterMut, IntoIter}`.

## Custom shapes

Implement `Format<ManyErrors<..>>` on a unit type and render through
`errs.formatted::<Shape>()`. Reuse the crate's helpers so the output matches
the built-ins: `ErrorCount(n)` prints `"N errors"`, `LeafChain(w)` prints a
leaf with its `": "`-joined sources, `many_errors::strategy::NO_ERRORS` is the
empty marker, and `indent::indented` re-indents multi-line content.

```rust
use core::fmt::{self, Formatter};
use errortools::{ErrorCount, Format, LeafChain, ManyErrors, Node, WithContext};
use errortools::many_errors::strategy::NO_ERRORS;

struct Arrows;

impl<C, E, GC, F, GF> Format<ManyErrors<C, E, GC, F, GF>> for Arrows
where
    E: core::error::Error,
    F: Format<WithContext<C, E, F>>,
{
    fn fmt(errors: &ManyErrors<C, E, GC, F, GF>, f: &mut Formatter<'_>) -> fmt::Result {
        if errors.is_empty() {
            return f.write_str(NO_ERRORS);
        }
        write!(f, "{} ->", ErrorCount(errors.len()))?;
        for (i, node) in errors.iter().enumerate() {
            let sep = if i == 0 { " " } else { "; " };
            match node {
                Node::Leaf(w) => write!(f, "{sep}{}", LeafChain(w))?,
                Node::Group(g) => write!(f, "{sep}{}", ErrorCount(g.errors.len()))?,
            }
        }
        Ok(())
    }
}
// all.formatted::<Arrows>(): "2 errors -> 2 errors; eu-west-1: Connection refused"
```

To restyle only group labels, set `GF` to a `Format<GC>` strategy; it sees the
label alone, never the nested errors.
