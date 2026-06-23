# Aggregating many failures: `ManyErrors`

Read this when an operation shouldn't stop at the first failure — validating a
whole config, deploying to every region, parsing a batch — and you want to
report all of them, grouped and readable, instead of just the first. (Requires
the `alloc` feature, on by default.)

`ManyErrors<C, E>` is a context-tagged collection arranged as a rose tree. It
costs nothing until it has to: `None` while empty, one inline slot for the first
error, a `Vec` only once a second arrives.

## Building it

```rust
use errortools::ManyErrors;

let mut errs = ManyErrors::new();
errs.push("eu-west-1", RegionError::Refused);   // (context, error) leaf
errs.push("us-east-1", RegionError::Refused);

errs.into_result(())?; // Ok(()) if empty, Err(ManyErrors) otherwise
```

| Method | Effect |
|---|---|
| `new()` / `is_empty()` / `len()` | construct / inspect |
| `push(context, error)` | append a leaf, promoting `None → One → Many` |
| `push_group(label, sub)` | append a named nested `ManyErrors` |
| `push_node(node)` | append anything `Into<Node>` — a `(C, E)` pair, `WithContext`, `Subgroup`, or `Node` |
| `into_result(ok)` | `Ok(ok)` if empty, else `Err(self)` |
| `with_formats::<F, GF>()` | swap leaf + group-label strategies |

You can also collect straight from an iterator of `(context, error)` pairs or
`WithContext` values — `ManyErrors` implements `FromIterator` and `Extend`,
which composes with itertools' `partition_result`:

```rust
let errs: ManyErrors<&str, io::Error> = nodes.into_iter().collect();
```

Group related failures with `push_group`, and the shapes nest:

```rust
use errortools::ManyErrors;

let mut east = ManyErrors::new();
east.push("i-0a1", RegionError::Refused);
east.push("i-0b2", RegionError::Refused);

let mut all: ManyErrors<&str, RegionError> = ManyErrors::new();
all.push_group("us-east-1", east);
all.push("eu-west-1", RegionError::Refused);
```

## Rendering shapes

The shapes are **inherent helpers** — no turbofish — and they walk each leaf's
source chain:

```rust
println!("{}", all.tree());      // Unicode branching tree, with count header
println!("{}", all.list());      // numbered outline (1.  1.1.  2.)
println!("{}", all.bullets());   // • bulleted
println!("{}", all.joined());    // ;-separated single line, parens around groups
```

`tree()`:

```text
2 errors:
├─ us-east-1 (2 errors):
│  ├─ i-0a1: connection refused
│  └─ i-0b2: connection refused
└─ eu-west-1: connection refused
```

The default `Display` (`{all}`) is deliberately a **shallow one-line summary** —
each error's own text, no source chains — so it's safe to embed in a message or
log, following the Rust convention that an error's `Display` is its own message:

```text
2 errors: us-east-1 (2 errors: i-0a1: connection refused; i-0b2: connection refused); eu-west-1: connection refused
```

For full control — ASCII connectors, no count header — go through the inherent
`formatted` with explicit generics. `Tree<Conn, HEADER>`:

```rust
use errortools::{Formatted, many_errors::{Ascii, Tree}};
println!("{}", Formatted::<_, Tree<Ascii, false>>::new(&all)); // ASCII, no header
println!("{}", all.formatted::<Tree<Ascii, false>>());          // same, shorter
```

## Two `formatted` methods — use the inherent one

`errs.formatted::<F>()` resolves to an **inherent** method that is completely
unbounded (and `const`): wrapping always compiles; whether the combination can
print is decided by `F`'s own `Format` bounds at the `Display` call site.

The `FormatError::formatted` *trait* method also exists, but calling it requires
`ManyErrors: Error`, which drags `C: Debug` and `GC: Debug` onto your context
types (the `Error` supertrait) even though no strategy needs them to render. At a
call site the inherent method wins automatically and produces the identical
value — so just write `errs.formatted::<F>()`. A `PathBuf` context with
`PathColon` then renders in every shape even though `PathBuf` has no `Display`.

The same rule runs through the whole path: no shape demands anything from a
context directly. Leaves print through the leaf strategy `F`, group labels
through the label strategy `GF`, and those decide the bounds.

## Children are errors too

```rust
use errortools::FormatError;

// Iterate and log each child, or stick one in a #[source].
for node in &errs {
    tracing::warn!("{}", node.one_line());
}
```

## Footgun: `one_line()` / `chain()` on an aggregate

A `ManyErrors` *is* an `Error`, so `errs.one_line()` and `errs.chain()` compile —
but they print only the shallow summary, because `Error::source()` is always
`None` (an aggregate has no single linear cause). The deep versions are
`joined()` (one line) and `tree()` (multi-line).

Same logic in reverse: a `ManyErrors` buried in another error's `#[source]`
chain shows up as **one summary line** under `OneLine`/`Chain`, and the walk
stops there — branching can't be recovered through `dyn Error`. If you want its
branches rendered, lift it into a `push_group` of an outer `ManyErrors` instead
of chaining it as a source.

## Custom aggregate shapes

Need your own layout? Implement `Format<ManyErrors<…>>` and match on the public
`ManyErrors` / `Node` variants, plus a small `&T` ref-forwarder so
`Formatted<&ManyErrors<…>, _>` works:

```rust
use core::fmt::{self, Display, Formatter};
use errortools::{Format, ManyErrors, Node};

struct Dashed;

impl<C: Display, E: Display, GC: Display, F, GF> Format<ManyErrors<C, E, GC, F, GF>> for Dashed {
    fn fmt(errors: &ManyErrors<C, E, GC, F, GF>, f: &mut Formatter<'_>) -> fmt::Result {
        for node in errors {
            match node {
                Node::Leaf(w) => writeln!(f, "- {}: {}", w.context, w.error)?,
                Node::Group(g) => writeln!(f, "- {} ({} nested)", g.context, g.errors.len())?,
            }
        }
        Ok(())
    }
}

impl<T: ?Sized> Format<&T> for Dashed where Dashed: Format<T> {
    fn fmt(errors: &&T, f: &mut Formatter<'_>) -> fmt::Result {
        <Self as Format<T>>::fmt(*errors, f)
    }
}
```

To keep output consistent with the built-ins, reuse their public helpers rather
than reinventing them: `many_errors::strategy::{ErrorCount, LeafChain,
NO_ERRORS}`, `indent::indented` for re-indenting multiline content, and the
`impl_ref_format!` macro for the `&T` trampoline. See the `many_errors::strategy`
module docs for a worked example.

Group labels can differ from leaf contexts via the third parameter
`ManyErrors<C, E, GC>`, but `GC` defaults to `C`, so the common case stays two
params. Label decoration is a separate lever: the group-label strategy `GF`
(default `AsDisplay`) is label-only and never sees the nested errors — laying
those out is the aggregate strategy's job.
