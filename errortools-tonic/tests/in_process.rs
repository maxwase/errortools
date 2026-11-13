//! Runs a real tonic server and client in one process and checks that the
//! mapping survives an actual gRPC round trip.
//!
//! The transport is in memory: the client's connector makes a
//! `tokio::io::duplex` pair for every connection, keeps one end and hands the
//! other to the server through a channel that the server reads as its stream
//! of incoming connections. HTTP/2 runs over that pair exactly as it would
//! over TCP, so no port is bound.
//!
//! The service is written by hand on `tonic::server::Grpc` with a codec that
//! sends strings as raw UTF-8, so nothing here needs `protoc`. The request
//! names a scenario and the server fails with that scenario's error.
//!
//! Every scenario is called in two ways, because tonic sends a status in two
//! different places:
//!
//! * a unary call that fails gets a "trailers-only" response, where the
//!   status travels in the HTTP/2 response headers;
//! * a server stream that yields a message and then fails has already sent
//!   its headers, so the status travels in real HTTP/2 trailers.

use std::{
    convert::Infallible,
    fmt,
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{Buf as _, BufMut as _};
use errortools::{Detached, FormatError as _, ManyErrors, Suggest, sources};
use errortools_tonic::{
    Code, DetachedErrors, ErrorStatusExt as _, IntoStatus as _, Mapping, TREE_TRAILER,
    detach_leaves,
};
use http::uri::PathAndQuery;
use hyper_util::rt::TokioIo;
use pretty_assertions::assert_eq;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    Request, Response, Status,
    body::Body,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
    metadata::KeyAndValueRef,
    transport::{Channel, Endpoint, Server, Uri},
};
use tonic_types::StatusExt as _;
use tower::{Service, service_fn};

// --- the errors the server fails with ---

#[derive(Error, Debug)]
#[error("connection refused")]
struct Refused;

#[derive(Error, Debug)]
#[error("Failed to query the database")]
struct Query(#[source] Refused);

#[derive(Error, Debug)]
enum Api {
    #[error("User {0} does not exist")]
    NoSuchUser(u64),
    #[error("Storage is down")]
    Storage(#[source] Query),
}

impl Code for Api {
    fn code(&self) -> tonic::Code {
        match self {
            Api::NoSuchUser(_) => tonic::Code::NotFound,
            Api::Storage(_) => tonic::Code::Unavailable,
        }
    }

    fn suggest(&self) -> Option<&dyn Suggest> {
        Some(self)
    }
}

impl Suggest for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Api::NoSuchUser(_) => f.write_str("List the users to find a valid id"),
            Api::Storage(_) => Ok(()),
        }
    }
}

fn storage() -> Api {
    Api::Storage(Query(Refused))
}

#[derive(Error, Debug)]
#[error("Invalid value")]
struct Invalid(#[source] io::Error);

type Fields = ManyErrors<&'static str, Invalid>;

fn invalid(reason: &'static str) -> Invalid {
    Invalid(io::Error::other(reason))
}

/// Two leaves and a group that holds a leaf and another group.
fn nested() -> Fields {
    let mut geo = Fields::new();
    geo.push("lat", invalid("out of range"));
    let mut address = Fields::new();
    address.push("zip", invalid("too short"));
    address.push_group("geo", geo);
    let mut errors = Fields::new();
    errors.push("name", invalid("empty"));
    errors.push("age", invalid("not a number"));
    errors.push_group("address", address);
    errors
}

/// Every piece of source text in the errors above. None of it may reach the
/// client under the default mapping.
const SOURCE_TEXT: [&str; 6] = [
    "connection refused",
    "Failed to query the database",
    "empty",
    "not a number",
    "too short",
    "out of range",
];

/// The server's whole behaviour: the status for a scenario, or `None` when
/// the scenario succeeds.
fn failure(scenario: &str) -> Option<Status> {
    let tree = Mapping::detailed().with_tree();
    Some(match scenario {
        "ok" => return None,
        "storage-default" => storage().into_status(),
        "storage-detailed" => storage().into_detailed_status(),
        "no-such-user" => Api::NoSuchUser(7).into_status(),
        "fields-default" => nested().into_status(),
        "fields-default-tree" => nested().into_status_with(&Mapping::new().with_tree()),
        "fields-detailed" => nested().into_detailed_status(),
        "fields-detailed-tree" => nested().into_status_with(&tree),
        other => Status::unimplemented(format!("unknown scenario {other}")),
    })
}

// --- a codec that needs no protobuf ---

/// Sends a `String` as its UTF-8 bytes.
#[derive(Debug, Clone, Copy, Default)]
struct Utf8Codec;

impl Codec for Utf8Codec {
    type Encode = String;
    type Decode = String;
    type Encoder = Self;
    type Decoder = Self;

    fn encoder(&mut self) -> Self {
        Self
    }

    fn decoder(&mut self) -> Self {
        Self
    }
}

impl Encoder for Utf8Codec {
    type Item = String;
    type Error = Status;

    fn encode(&mut self, item: String, dst: &mut EncodeBuf<'_>) -> Result<(), Status> {
        dst.put_slice(item.as_bytes());
        Ok(())
    }
}

impl Decoder for Utf8Codec {
    type Item = String;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<String>, Status> {
        let bytes = src.copy_to_bytes(src.remaining());
        String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|error| Status::invalid_argument(error.to_string()))
    }
}

// --- the service ---

const UNARY_PATH: &str = "/in_process.Scenarios/Unary";
const STREAM_PATH: &str = "/in_process.Scenarios/Stream";
/// What a successful call answers, and what a stream yields before failing.
const GREETING: &str = "hello";

type ScenarioStream = tokio_stream::Iter<std::vec::IntoIter<Result<String, Status>>>;

async fn unary(request: Request<String>) -> Result<Response<String>, Status> {
    match failure(request.get_ref()) {
        Some(status) => Err(status),
        None => Ok(Response::new(GREETING.to_owned())),
    }
}

/// Yields one message, then the scenario's failure. The message forces the
/// response headers out first, so the status has to go into the trailers.
async fn stream(request: Request<String>) -> Result<Response<ScenarioStream>, Status> {
    let mut items = vec![Ok(GREETING.to_owned())];
    items.extend(failure(request.get_ref()).map(Err));
    Ok(Response::new(tokio_stream::iter(items)))
}

/// What generated code would be for a service with the two methods above.
#[derive(Debug, Clone, Copy)]
struct Scenarios;

impl Service<http::Request<Body>> for Scenarios {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        Box::pin(async move {
            let mut grpc = tonic::server::Grpc::new(Utf8Codec);
            Ok(match request.uri().path() {
                UNARY_PATH => grpc.unary(service_fn(unary), request).await,
                STREAM_PATH => grpc.server_streaming(service_fn(stream), request).await,
                _ => Status::unimplemented("no such method").into_http(),
            })
        })
    }
}

// --- the transport ---

/// Starts the server and connects a client to it over in-memory pipes.
async fn connect() -> Channel {
    let (connections, incoming) = mpsc::channel::<io::Result<tokio::io::DuplexStream>>(1);

    tokio::spawn(async move {
        Server::builder()
            .serve_with_incoming(Scenarios, ReceiverStream::new(incoming))
            .await
            .expect("the server runs until the test ends");
    });

    let connector = service_fn(move |_: Uri| {
        let connections = connections.clone();
        async move {
            let (client, server) = tokio::io::duplex(64 * 1024);
            connections
                .send(Ok(server))
                .await
                .map_err(|_| io::Error::other("the server is gone"))?;
            Ok::<_, io::Error>(TokioIo::new(client))
        }
    });

    // The address is never resolved; the connector ignores it.
    Endpoint::from_static("http://in-process.invalid")
        .connect_with_connector(connector)
        .await
        .expect("the in-memory connection is made")
}

/// Where the status of a failed call travels.
#[derive(Debug, Clone, Copy)]
enum Carrier {
    /// A failed unary call: the status is in the response headers.
    TrailersOnly,
    /// A stream that fails after a message: the status is in the trailers.
    Trailers,
}

const CARRIERS: [Carrier; 2] = [Carrier::TrailersOnly, Carrier::Trailers];

/// Calls `scenario` and returns the status the client received.
async fn call(channel: &Channel, carrier: Carrier, scenario: &str) -> Status {
    let mut client = tonic::client::Grpc::new(channel.clone());
    client.ready().await.expect("the channel is ready");
    let request = Request::new(scenario.to_owned());

    match carrier {
        Carrier::TrailersOnly => client
            .unary::<_, String, _>(request, PathAndQuery::from_static(UNARY_PATH), Utf8Codec)
            .await
            .expect_err("the scenario fails"),
        Carrier::Trailers => {
            let mut messages = client
                .server_streaming(request, PathAndQuery::from_static(STREAM_PATH), Utf8Codec)
                .await
                .expect("the stream starts before it fails")
                .into_inner();
            // The message arrives first, which proves the headers were sent
            // without a status and the failure comes from the trailers.
            let first: Option<String> = messages.message().await.expect("the first message");
            assert_eq!(first.as_deref(), Some(GREETING));
            messages.message().await.expect_err("the scenario fails")
        }
    }
}

/// Runs `check` on the status of `scenario` as received through each carrier.
async fn for_each_carrier(scenario: &str, check: impl Fn(&Status)) {
    let channel = connect().await;
    for carrier in CARRIERS {
        let status = call(&channel, carrier, scenario).await;
        // A panic inside `check` does not say which carrier it was.
        eprintln!("checking {scenario} received through {carrier:?}");
        check(&status);
    }
}

// --- helpers for the assertions ---

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// Whether `needle` appears anywhere in what the client received: the
/// message, the encoded details, or any metadata value (binary values are
/// searched after base64 decoding).
fn received_anywhere(status: &Status, needle: &str) -> bool {
    contains(status.message().as_bytes(), needle)
        || contains(status.details(), needle)
        || status.metadata().iter().any(|entry| match entry {
            KeyAndValueRef::Ascii(_, value) => contains(value.as_encoded_bytes(), needle),
            KeyAndValueRef::Binary(_, value) => {
                let bytes = value.to_bytes().expect("binary metadata is valid base64");
                contains(&bytes, needle)
            }
        })
}

fn assert_no_source_text(status: &Status) {
    for text in SOURCE_TEXT {
        assert!(
            !received_anywhere(status, text),
            "source text {text:?} reached the client: {status:?}"
        );
    }
}

fn flat(leaves: &[(&str, &str)]) -> DetachedErrors {
    leaves
        .iter()
        .map(|(field, description)| ((*field).to_owned(), Detached::new(*description)))
        .collect()
}

// --- the tests ---

#[tokio::test]
async fn test_successful_calls_work() {
    let channel = connect().await;
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();

    let response: Response<String> = client
        .unary(
            Request::new("ok".to_owned()),
            PathAndQuery::from_static(UNARY_PATH),
            Utf8Codec,
        )
        .await
        .unwrap();
    assert_eq!(response.get_ref(), GREETING);

    client.ready().await.unwrap();
    let mut messages = client
        .server_streaming(
            Request::new("ok".to_owned()),
            PathAndQuery::from_static(STREAM_PATH),
            Utf8Codec,
        )
        .await
        .unwrap()
        .into_inner();
    let first: Option<String> = messages.message().await.unwrap();
    assert_eq!(first.as_deref(), Some(GREETING));
    assert_eq!(messages.message().await.unwrap(), None);
}

#[tokio::test]
async fn test_detailed_chain_round_trip() {
    let error = storage();
    let entries: Vec<String> = sources(&error).map(ToString::to_string).collect();
    assert_eq!(
        entries,
        [
            "Storage is down",
            "Failed to query the database",
            "connection refused"
        ]
    );

    for_each_carrier("storage-detailed", |status| {
        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert_eq!(
            status.message(),
            "Storage is down: Failed to query the database: connection refused"
        );
        assert_eq!(status.message(), error.one_line().to_string());

        let info = status.get_details_debug_info().expect("DebugInfo is sent");
        assert_eq!(info.stack_entries, entries);
        assert_eq!(info.detail, "");

        // The rebuilt chain renders like the server's error did, and is the
        // same value as a snapshot taken on the server.
        let detached = status.to_detached();
        assert_eq!(detached.chain().to_string(), error.chain().to_string());
        assert_eq!(detached, error.detach());

        // This error's hint is empty, so none is sent, and it is no aggregate.
        assert_eq!(status.hint(), None);
        assert_eq!(status.to_many_errors(), None);
    })
    .await;
}

#[tokio::test]
async fn test_default_mapping_sends_no_source_text() {
    for_each_carrier("storage-default", |status| {
        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert_eq!(status.message(), "Storage is down");
        assert!(status.get_details_debug_info().is_none());
        // Code and message say it all, so no details are sent at all.
        assert!(status.details().is_empty());
        assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
        assert_no_source_text(status);

        // The client falls back to the one level it was given.
        assert_eq!(status.to_detached(), Detached::new("Storage is down"));
    })
    .await;
}

#[tokio::test]
async fn test_default_mapping_sends_no_leaf_sources() {
    let top_level = [
        ("name", "Invalid value"),
        ("age", "Invalid value"),
        ("address.zip", "Invalid value"),
        ("address.geo.lat", "Invalid value"),
    ];

    for_each_carrier("fields-default", |status| {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.get_details_debug_info().is_none());
        assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
        assert_no_source_text(status);
        assert_eq!(status.to_many_errors(), Some(flat(&top_level)));
    })
    .await;

    // Turning the tree trailer on adds structure, not source text.
    for_each_carrier("fields-default-tree", |status| {
        assert!(status.get_details_debug_info().is_none());
        assert!(status.metadata().get_bin(TREE_TRAILER).is_some());
        assert_no_source_text(status);

        // The same shape as `nested()`, with every leaf cut to its own message.
        let mut geo = DetachedErrors::new();
        geo.push("lat".to_owned(), Detached::new("Invalid value"));
        let mut address = DetachedErrors::new();
        address.push("zip".to_owned(), Detached::new("Invalid value"));
        address.push_group("geo".to_owned(), geo);
        let mut expected = DetachedErrors::new();
        expected.push("name".to_owned(), Detached::new("Invalid value"));
        expected.push("age".to_owned(), Detached::new("Invalid value"));
        expected.push_group("address".to_owned(), address);
        assert_eq!(status.to_many_errors(), Some(expected));
    })
    .await;
}

#[tokio::test]
async fn test_hint_round_trip() {
    for_each_carrier("no-such-user", |status| {
        assert_eq!(status.code(), tonic::Code::NotFound);
        assert_eq!(status.message(), "User 7 does not exist");
        assert_eq!(
            status.hint().as_deref(),
            Some("List the users to find a valid id")
        );
        // The hint is the only detail under the default mapping.
        assert!(status.get_details_debug_info().is_none());
        assert_eq!(status.to_many_errors(), None);
    })
    .await;
}

#[tokio::test]
async fn test_aggregate_comes_back_flat_through_bad_request() {
    let expected = flat(&[
        ("name", "Invalid value: empty"),
        ("age", "Invalid value: not a number"),
        ("address.zip", "Invalid value: too short"),
        ("address.geo.lat", "Invalid value: out of range"),
    ]);

    for_each_carrier("fields-detailed", |status| {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        // An aggregate has no source of its own, so even the detailed
        // message is its `Display` summary, which names each leaf's own
        // message only. The leaf chains are in the violations.
        assert_eq!(
            status.message(),
            "3 errors: name: Invalid value; age: Invalid value; \
             address (2 errors: zip: Invalid value; geo (lat: Invalid value))"
        );
        assert_eq!(status.message(), nested().to_string());
        assert!(status.metadata().get_bin(TREE_TRAILER).is_none());

        let violations = status
            .get_details_bad_request()
            .expect("BadRequest is sent")
            .field_violations;
        assert_eq!(violations.len(), 4);

        assert_eq!(status.to_many_errors().as_ref(), Some(&expected));
    })
    .await;
}

#[tokio::test]
async fn test_aggregate_comes_back_nested_through_tree_trailer() {
    let sent = nested();
    let expected = detach_leaves(&sent);

    for_each_carrier("fields-detailed-tree", |status| {
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.metadata().get_bin(TREE_TRAILER).is_some());

        let received = status.to_many_errors().expect("the tree trailer is sent");
        // Two leaves and the `address` group, not four flat leaves.
        assert_eq!(received.len(), 3);
        assert_eq!(received, expected);
        assert_eq!(received.tree().to_string(), sent.tree().to_string());

        // The flat violations are still there for clients in other languages.
        let violations = status
            .get_details_bad_request()
            .expect("BadRequest is sent")
            .field_violations;
        assert_eq!(violations.len(), 4);
    })
    .await;
}
