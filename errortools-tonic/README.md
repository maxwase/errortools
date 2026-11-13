# errortools-tonic

Maps [`errortools`](https://docs.rs/errortools) errors onto gRPC
[`tonic::Status`](https://docs.rs/tonic/latest/tonic/struct.Status.html) and back.

By default only the status code, the error's top-level message, its hint and
the top-level message of each aggregate leaf are sent. The source chain is
not sent: no source text and no `DebugInfo` leave the server unless the
handler opts in with `into_detailed_status()` or `Detail::Chain`. See
[What the client gets to see](#what-the-client-gets-to-see).

The mapping uses the standard gRPC carriers, so a client in any language can
read the result, and adds one private trailer only for what they cannot express.

| Carrier | Content |
|---|---|
| `grpc-status` | `Code::code()`, `Internal` unless overridden |
| `grpc-message` | The top-level message, or the `OneLine` rendering with `Detail::Chain` |
| `google.rpc.DebugInfo` | The source chain, one stack entry per level. Only with `Detail::Chain` |
| `google.rpc.BadRequest` | The leaves of a `ManyErrors`: field is the context, description is the error |
| `google.rpc.Help` | The `Suggest` hint, as a link without a URL |
| `errortools-tree-bin` | The nested `ManyErrors` structure. Opt-in and size-capped |

The three `google.rpc` messages travel in `grpc-status-details-bin` through
[`tonic-types`](https://docs.rs/tonic-types).

## Server

Implement `Code` for the error type (an empty impl maps to `Internal`), then
convert in the handler with `IntoStatus`:

```rust
use errortools::Suggest;
use errortools_tonic::{Code, IntoStatus};
use std::{fmt, io};
use tonic::{Response, Status};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("User {0} does not exist")]
    NoSuchUser(u64),
    #[error("Failed to load the profile")]
    Storage(#[source] io::Error),
}

impl Code for Error {
    fn code(&self) -> tonic::Code {
        match self {
            Error::NoSuchUser(_) => tonic::Code::NotFound,
            Error::Storage(_) => tonic::Code::Internal,
        }
    }

    // Opt in to sending the `Suggest` hint.
    fn suggest(&self) -> Option<&dyn Suggest> {
        Some(self)
    }
}

impl Suggest for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSuchUser(_) => f.write_str("List the users to find a valid id"),
            Error::Storage(_) => Ok(()),
        }
    }
}

fn load_profile(id: u64) -> Result<String, Error> {
    Err(Error::NoSuchUser(id))
}

// The body of a tonic service method.
fn get_profile(id: u64) -> Result<Response<String>, Status> {
    let profile = load_profile(id).map_err(IntoStatus::into_status)?;
    Ok(Response::new(profile))
}

let status = get_profile(7).unwrap_err();
assert_eq!(status.code(), tonic::Code::NotFound);
assert_eq!(status.message(), "User 7 does not exist");
```

For a bare `?`, add the conversion yourself. The orphan rule keeps this crate
from providing it:

```rust
# use errortools_tonic::{Code, IntoStatus};
# #[derive(Debug, thiserror::Error)]
# #[error("boom")]
# struct MyError;
# impl Code for MyError {}
impl From<MyError> for tonic::Status {
    fn from(error: MyError) -> Self {
        error.into_status()
    }
}
```

## What the client gets to see

Error sources are written for operators. They regularly contain file paths,
host names, SQL and similar internals, so the default is to keep them on the
server:

- `into_status()` sends the code, the **top-level** message, the hint, and the
  top-level message of each aggregate leaf. No source text is sent, and no
  `DebugInfo`.
- `into_detailed_status()`, or a `Mapping` with `Detail::Chain`, sends the whole
  chain: `grpc-message` becomes the `OneLine` rendering, `DebugInfo` carries one
  entry per level, and aggregate leaves carry their chains. Everything in the
  chain becomes readable by every caller. Use it between services you trust,
  not on a public API.

Top-level messages, hints and leaf contexts are always sent, so they must be
fit for callers in either mode.

A `Mapping` is a `const` value, so a service usually defines one and uses it
everywhere:

```rust
use errortools_tonic::{Code, IntoStatus, Mapping};

const INTERNAL: Mapping = Mapping::detailed().with_tree();

# #[derive(Debug, thiserror::Error)]
# #[error("boom")]
# struct MyError;
# impl Code for MyError {}
let status = MyError.into_status_with(&INTERNAL);
```

## Client

`ErrorStatusExt` reads the data back. The chain becomes a
[`Detached`](https://docs.rs/errortools/latest/errortools/struct.Detached.html),
which renders through every errortools strategy like the original error did:

```rust
use errortools::FormatError;
use errortools_tonic::{Code, ErrorStatusExt, IntoStatus};
use std::io;

#[derive(Debug, thiserror::Error)]
#[error("Failed to load the profile")]
struct LoadError(#[source] io::Error);

impl Code for LoadError {}

// Server, an internal service that shares its chains.
let status = LoadError(io::Error::other("disk full")).into_detailed_status();

// Client.
let error = status.to_detached();
assert_eq!(
    error.one_line().to_string(),
    "Failed to load the profile: disk full"
);
assert_eq!(status.hint(), None);
```

When the status has no `DebugInfo` (the default mapping, a server that does not
use this crate, or a proxy that dropped the details), `to_detached` falls back
to a single level holding the status message.

## Aggregates

A `ManyErrors` implements `Code` itself: it maps to `InvalidArgument` and its
leaves become `BadRequest` field violations. Leaves inside nested groups get
the group labels as a dot-separated prefix of the field.

```rust
use errortools::ManyErrors;
use errortools_tonic::{ErrorStatusExt, IntoStatus};
use std::io;

let mut errors = ManyErrors::<&str, io::Error>::new();
errors.push("name", io::Error::other("must not be empty"));
errors.push("age", io::Error::other("must be a number"));

let status = errors.into_status();
assert_eq!(status.code(), tonic::Code::InvalidArgument);

let received = status.to_many_errors().unwrap();
assert_eq!(received.len(), 2);
```

Only the top-level error is asked for an aggregate. A wrapper type exposes the
`ManyErrors` it holds by overriding `Code::many_errors` (see `detach_leaves`).
A `ManyErrors` that is only reachable as a source of another error is not
found, because it cannot be seen through `dyn Error`; it travels as its
one-line summary inside the chain.

### The tree trailer

`BadRequest` is flat. `Mapping::with_tree` additionally sends the aggregate in
the binary trailer `errortools-tree-bin`, which keeps nested groups (and leaf
chains with `Detail::Chain`); `to_many_errors` prefers it when it is present
and valid.

The trailer is off by default and capped at 4 KiB on the wire
(`DEFAULT_TREE_CAP`, base64 included), because HTTP/2 peers and proxies
commonly limit a whole header list to 8 or 16 KiB and a response that exceeds
the limit fails as a whole. A tree over the cap is not sent at all; it is
never truncated. The flat violations are sent either way. The cap covers only
this trailer: a very long chain or a very large aggregate in the standard
details is not limited by this crate.

Encoding, version 1. Counts are unsigned LEB128 and strings are UTF-8:

```text
tree   := 0x01 nodes
nodes  := count node*
node   := 0x00 string chain      leaf: context, then its messages
        | 0x01 string nodes      group: label, then its children
chain  := count string*          1 to 1024 messages, outermost first
string := count byte*
```

Groups nest at most 32 levels deep and a leaf chain holds at most 1024
messages; a tree beyond either limit is not sent, and one that arrives is
ignored. `to_detached` applies the same 1024 limit to the `DebugInfo` entries
and falls back to the status message beyond it.
