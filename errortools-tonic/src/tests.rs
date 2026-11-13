use std::{fmt, io, num::NonZeroUsize};

use errortools::{Detached, FormatError as _, ManyErrors, Suggest, sources};
use pretty_assertions::assert_eq;
use thiserror::Error;
use tonic::Status;
use tonic_types::{FieldViolation, StatusExt as _};

use crate::{
    Code, DEFAULT_TREE_CAP, DetachedErrors, Detail, ErrorStatusExt as _, IntoStatus as _, Mapping,
    TREE_TRAILER, detach_leaves, tree,
};

#[derive(Error, Debug)]
#[error("connection refused")]
struct Leaf;

#[derive(Error, Debug)]
#[error("Failed to query the database")]
struct Mid(#[source] Leaf);

/// Uses every default of [`Code`].
#[derive(Error, Debug)]
#[error("Failed to load the profile")]
struct Top(#[source] Mid);

impl Code for Top {}

fn top() -> Top {
    Top(Mid(Leaf))
}

/// Overrides the code and opts in to the hint.
#[derive(Error, Debug)]
enum Api {
    #[error("User {0} does not exist")]
    NoSuchUser(u64),
    #[error("Storage is down")]
    Storage(#[source] Mid),
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

#[derive(Error, Debug)]
#[error("Invalid value")]
struct Invalid(#[source] io::Error);

type Fields = ManyErrors<&'static str, Invalid>;

fn invalid(reason: &'static str) -> Invalid {
    Invalid(io::Error::other(reason))
}

fn flat() -> Fields {
    let mut errors = Fields::new();
    errors.push("name", invalid("empty"));
    errors.push("age", invalid("not a number"));
    errors
}

fn nested() -> Fields {
    let mut address = Fields::new();
    address.push("zip", invalid("too short"));
    address.push_group(
        "geo",
        Fields::from(("lat", Err::<(), _>(invalid("out of range")))),
    );
    let mut errors = flat();
    errors.push_group("address", address);
    errors
}

/// Holds an aggregate and exposes it explicitly.
#[derive(Error, Debug)]
#[error("Invalid request")]
struct Exposed(#[source] Fields);

impl Code for Exposed {
    fn code(&self) -> tonic::Code {
        tonic::Code::FailedPrecondition
    }

    fn many_errors(&self) -> Option<DetachedErrors> {
        Some(detach_leaves(&self.0))
    }
}

/// Holds an aggregate only as a source.
#[derive(Error, Debug)]
#[error("Invalid request")]
struct Buried(#[source] Fields);

impl Code for Buried {}

fn violations(status: &Status) -> Vec<(String, String)> {
    status
        .get_details_bad_request()
        .map(|bad_request| {
            bad_request
                .field_violations
                .into_iter()
                .map(
                    |FieldViolation {
                         field, description, ..
                     }| (field, description),
                )
                .collect()
        })
        .unwrap_or_default()
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(field, description)| ((*field).to_owned(), (*description).to_owned()))
        .collect()
}

fn tree_cap(bytes: usize) -> Mapping {
    Mapping::detailed().with_tree_cap(NonZeroUsize::new(bytes).unwrap())
}

// --- code ---

#[test]
fn test_code_defaults_to_internal() {
    assert_eq!(Code::code(&top()), tonic::Code::Internal);
    assert_eq!(top().into_status().code(), tonic::Code::Internal);
    assert_eq!(top().into_detailed_status().code(), tonic::Code::Internal);
}

#[test]
fn test_code_override() {
    assert_eq!(
        Api::NoSuchUser(7).into_status().code(),
        tonic::Code::NotFound
    );
    assert_eq!(
        Api::Storage(Mid(Leaf)).into_detailed_status().code(),
        tonic::Code::Unavailable
    );
}

#[test]
fn test_code_through_reference() {
    let error = Api::NoSuchUser(7);
    assert_eq!(Code::code(&&error), tonic::Code::NotFound);
    assert!(Code::suggest(&&error).is_some());
    assert_eq!((&error).into_status().code(), tonic::Code::NotFound);
}

#[test]
fn test_many_errors_code_is_invalid_argument() {
    assert_eq!(flat().into_status().code(), tonic::Code::InvalidArgument);
    assert_eq!(
        Exposed(flat()).into_status().code(),
        tonic::Code::FailedPrecondition
    );
}

// --- message ---

#[test]
fn test_detailed_message_is_one_line_rendering() {
    let error = top();
    let expected = error.one_line().to_string();
    assert_eq!(
        expected,
        "Failed to load the profile: Failed to query the database: connection refused"
    );
    assert_eq!(error.into_detailed_status().message(), expected);
}

#[test]
fn test_default_message_is_top_level_only() {
    assert_eq!(top().into_status().message(), "Failed to load the profile");
}

// --- DebugInfo ---

#[test]
fn test_debug_info_entries_equal_chain() {
    let error = top();
    let chain: Vec<String> = sources(&error).map(ToString::to_string).collect();
    assert_eq!(chain.len(), 3);

    let info = error
        .into_detailed_status()
        .get_details_debug_info()
        .unwrap();
    assert_eq!(info.stack_entries, chain);
    assert_eq!(info.detail, "");
}

#[test]
fn test_debug_info_single_level() {
    let info = Api::NoSuchUser(7)
        .into_detailed_status()
        .get_details_debug_info()
        .unwrap();
    assert_eq!(info.stack_entries, ["User 7 does not exist"]);
}

#[test]
fn test_default_sends_no_source_text() {
    let status = top().into_status();
    assert!(status.get_details_debug_info().is_none());
    // Nothing to say beyond code and message, so no details at all.
    assert!(status.details().is_empty());
    assert!(status.metadata().is_empty());

    let status = Exposed(flat()).into_status_with(&Mapping::new().with_tree());
    assert!(status.get_details_debug_info().is_none());
    let leaked = |bytes: &[u8]| bytes.windows(5).any(|window| window == b"empty");
    assert!(!leaked(status.details()));
    let trailer = status.metadata().get_bin(TREE_TRAILER).unwrap();
    assert!(!leaked(&trailer.to_bytes().unwrap()));
}

// --- BadRequest ---

#[test]
fn test_bad_request_violations_for_many_errors() {
    assert_eq!(
        violations(&flat().into_detailed_status()),
        pairs(&[
            ("name", "Invalid value: empty"),
            ("age", "Invalid value: not a number"),
        ])
    );
    assert_eq!(
        violations(&flat().into_status()),
        pairs(&[("name", "Invalid value"), ("age", "Invalid value")])
    );
}

#[test]
fn test_bad_request_flattens_groups_with_dotted_fields() {
    assert_eq!(
        violations(&nested().into_detailed_status()),
        pairs(&[
            ("name", "Invalid value: empty"),
            ("age", "Invalid value: not a number"),
            ("address.zip", "Invalid value: too short"),
            ("address.geo.lat", "Invalid value: out of range"),
        ])
    );
}

#[test]
fn test_bad_request_for_exposed_aggregate() {
    let status = Exposed(flat()).into_detailed_status();
    assert_eq!(violations(&status).len(), 2);
    // The wrapper's own chain still ends at the aggregate's shallow summary.
    assert_eq!(
        status.get_details_debug_info().unwrap().stack_entries,
        [
            "Invalid request",
            "2 errors: name: Invalid value; age: Invalid value"
        ]
    );
}

#[test]
fn test_buried_aggregate_gets_no_violations() {
    let status = Buried(nested()).into_status_with(&Mapping::detailed().with_tree());
    assert!(status.get_details_bad_request().is_none());
    assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
    assert_eq!(status.to_many_errors(), None);
}

#[test]
fn test_no_bad_request_without_or_with_empty_aggregate() {
    assert!(
        top()
            .into_detailed_status()
            .get_details_bad_request()
            .is_none()
    );

    let status = Exposed(Fields::new()).into_status_with(&Mapping::detailed().with_tree());
    assert!(status.get_details_bad_request().is_none());
    assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
}

// --- Help ---

#[test]
fn test_help_carries_hint() {
    let status = Api::NoSuchUser(7).into_status();
    let help = status.get_details_help().unwrap();
    assert_eq!(help.links.len(), 1);
    assert_eq!(
        help.links[0].description,
        "List the users to find a valid id"
    );
    assert_eq!(help.links[0].url, "");
    assert_eq!(
        status.hint().as_deref(),
        Some("List the users to find a valid id")
    );
}

#[test]
fn test_no_help_for_empty_hint() {
    let status = Api::Storage(Mid(Leaf)).into_detailed_status();
    assert!(status.get_details_help().is_none());
    assert_eq!(status.hint(), None);
}

#[test]
fn test_no_help_without_opt_in() {
    let status = top().into_detailed_status();
    assert!(status.get_details_help().is_none());
    assert_eq!(status.hint(), None);
}

// --- tree trailer ---

#[test]
fn test_trailer_absent_unless_opted_in() {
    for mapping in [Mapping::new(), Mapping::detailed()] {
        let status = nested().into_status_with(&mapping);
        assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
        assert!(status.metadata().is_empty());
    }
    assert!(nested().into_status().metadata().is_empty());
    assert!(nested().into_detailed_status().metadata().is_empty());
}

#[test]
fn test_trailer_present_when_opted_in() {
    let status = nested().into_status_with(&Mapping::detailed().with_tree());
    let trailer = status.metadata().get_bin(TREE_TRAILER).unwrap();
    assert!(trailer.as_encoded_bytes().len() <= DEFAULT_TREE_CAP.get());
    assert_eq!(
        tree::decode(&trailer.to_bytes().unwrap()),
        Some(detach_leaves(&nested()))
    );
}

#[test]
fn test_trailer_dropped_over_cap_not_truncated() {
    let exact = nested()
        .into_status_with(&Mapping::detailed().with_tree())
        .metadata()
        .get_bin(TREE_TRAILER)
        .unwrap()
        .as_encoded_bytes()
        .len();

    let fits = nested().into_status_with(&tree_cap(exact));
    assert_eq!(
        fits.metadata()
            .get_bin(TREE_TRAILER)
            .unwrap()
            .as_encoded_bytes()
            .len(),
        exact
    );

    let over = nested().into_status_with(&tree_cap(exact - 1));
    assert!(over.metadata().get_bin(TREE_TRAILER).is_none());
    assert!(over.metadata().is_empty());
    // The standard details are unaffected by the cap.
    assert_eq!(violations(&over), violations(&fits));
    assert_eq!(violations(&over).len(), 4);
}

#[test]
fn test_large_aggregate_over_default_cap_is_dropped() {
    let errors: ManyErrors<String, Invalid> = (0..500)
        .map(|index| (format!("field-{index}"), invalid("not a number")))
        .collect();
    let status = errors.into_status_with(&Mapping::detailed().with_tree());
    assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
    assert_eq!(violations(&status).len(), 500);
}

#[test]
fn test_too_deep_tree_is_dropped() {
    let deep = (0..40).fold(flat(), |errors, _| errors.into_group("group"));
    let status = deep.into_status_with(&Mapping::detailed().with_tree());
    assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
    assert_eq!(violations(&status).len(), 2);

    let shallow = (0..32).fold(flat(), |errors, _| errors.into_group("group"));
    let expected = detach_leaves(&shallow);
    let status = shallow.into_status_with(&Mapping::detailed().with_tree());
    assert_eq!(status.to_many_errors(), Some(expected));
}

#[test]
fn test_decode_rejects_malformed_input() {
    let valid = tree::encode(&detach_leaves(&nested()), Detail::Chain).unwrap();
    assert!(tree::decode(&valid).is_some());

    // Every strict prefix is truncated data.
    for len in 0..valid.len() {
        assert_eq!(tree::decode(&valid[..len]), None, "prefix of {len} bytes");
    }

    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(tree::decode(&trailing), None);

    let mut version = valid.clone();
    version[0] = 2;
    assert_eq!(tree::decode(&version), None);

    // One node with an unknown tag and an empty context.
    assert_eq!(tree::decode(&[1, 1, 7, 0]), None);
    // A leaf with no messages.
    assert_eq!(tree::decode(&[1, 1, 0, 0, 0]), None);
    // A string that is not UTF-8.
    assert_eq!(tree::decode(&[1, 1, 0, 1, 0xff, 1, 0]), None);
    // A count that overflows, and a huge count with no data behind it.
    assert_eq!(
        tree::decode(&[
            1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f
        ]),
        None
    );
    assert_eq!(tree::decode(&[1, 0xff, 0xff, 0xff, 0x7f]), None);
    // Groups nested past the depth limit.
    let mut deep = vec![1];
    deep.extend([1, 1, 0].repeat(40));
    deep.push(0);
    assert_eq!(tree::decode(&deep), None);

    // The empty tree is valid.
    assert_eq!(tree::decode(&[1, 0]), Some(DetachedErrors::new()));
}

// --- client ---

#[test]
fn test_round_trip_renders_like_original() {
    let error = top();
    let one_line = error.one_line().to_string();
    let chain = error.chain().to_string();
    let snapshot = error.detach();

    let detached = error.into_detailed_status().to_detached();
    assert_eq!(detached.one_line().to_string(), one_line);
    assert_eq!(detached.chain().to_string(), chain);
    assert_eq!(detached, snapshot);
}

#[test]
fn test_round_trip_survives_http_trailers() {
    let error = Api::Storage(Mid(Leaf));
    let chain = error.chain().to_string();
    let sent = error.into_detailed_status();

    let mut headers = http::HeaderMap::new();
    sent.add_header(&mut headers).unwrap();
    let received = Status::from_header_map(&headers).unwrap();

    assert_eq!(received.code(), tonic::Code::Unavailable);
    assert_eq!(received.to_detached().chain().to_string(), chain);
}

#[test]
fn test_fallback_to_message_without_details() {
    let status = Status::new(tonic::Code::Internal, "plain message");
    assert_eq!(status.to_detached(), Detached::new("plain message"));
    assert_eq!(status.to_many_errors(), None);
    assert_eq!(status.hint(), None);
}

#[test]
fn test_fallback_for_default_mapping() {
    let detached = top().into_status().to_detached();
    assert_eq!(detached, Detached::new("Failed to load the profile"));
}

#[test]
fn test_fallback_when_details_are_garbage_or_have_no_entries() {
    let garbage = Status::with_details(
        tonic::Code::Internal,
        "plain message",
        vec![0xff, 0xff, 0xff].into(),
    );
    assert_eq!(garbage.to_detached(), Detached::new("plain message"));
    assert_eq!(garbage.to_many_errors(), None);
    assert_eq!(garbage.hint(), None);

    let empty = Status::with_error_details(
        tonic::Code::Internal,
        "plain message",
        tonic_types::ErrorDetails::with_debug_info(Vec::new(), "detail only"),
    );
    assert_eq!(empty.to_detached(), Detached::new("plain message"));
}

#[test]
fn test_fallback_keeps_one_line_when_details_are_stripped() {
    let error = top();
    let one_line = error.one_line().to_string();
    let sent = error.into_detailed_status();
    // What is left when a proxy drops `grpc-status-details-bin`.
    let stripped = Status::new(sent.code(), sent.message());
    assert_eq!(stripped.to_detached().one_line().to_string(), one_line);
}

#[test]
fn test_many_errors_round_trip_through_trailer() {
    let errors = nested();
    let tree = errors.tree().to_string();

    let status = errors.into_status_with(&Mapping::detailed().with_tree());
    let received = status.to_many_errors().unwrap();
    assert_eq!(received, detach_leaves(&nested()));
    assert_eq!(received.tree().to_string(), tree);
}

#[test]
fn test_many_errors_from_trailer_with_top_level_detail() {
    let status = nested().into_status_with(&Mapping::new().with_tree());
    let received = status.to_many_errors().unwrap();

    let mut geo = DetachedErrors::new();
    geo.push("lat".to_owned(), Detached::new("Invalid value"));
    let mut address = DetachedErrors::new();
    address.push("zip".to_owned(), Detached::new("Invalid value"));
    address.push_group("geo".to_owned(), geo);
    let mut expected = DetachedErrors::new();
    expected.push("name".to_owned(), Detached::new("Invalid value"));
    expected.push("age".to_owned(), Detached::new("Invalid value"));
    expected.push_group("address".to_owned(), address);
    assert_eq!(received, expected);
}

#[test]
fn test_many_errors_fall_back_to_flat_violations() {
    let received = nested().into_detailed_status().to_many_errors().unwrap();

    let expected: DetachedErrors = [
        ("name", "Invalid value: empty"),
        ("age", "Invalid value: not a number"),
        ("address.zip", "Invalid value: too short"),
        ("address.geo.lat", "Invalid value: out of range"),
    ]
    .into_iter()
    .map(|(field, description)| (field.to_owned(), Detached::new(description)))
    .collect();
    assert_eq!(received, expected);
}

#[test]
fn test_invalid_trailer_falls_back_to_violations() {
    let mut status = flat().into_detailed_status();
    status.metadata_mut().insert_bin(
        TREE_TRAILER,
        tonic::metadata::BinaryMetadataValue::from_bytes(b"\x09garbage"),
    );
    assert_eq!(status.to_many_errors().unwrap().len(), 2);
}

// --- mapping ---

#[test]
fn test_mapping_defaults_and_builders() {
    assert_eq!(Mapping::default(), Mapping::new());
    assert_eq!(Mapping::new().detail(), Detail::TopLevel);
    assert_eq!(Mapping::new().tree_cap(), None);
    assert_eq!(Mapping::detailed().detail(), Detail::Chain);
    assert_eq!(
        Mapping::new().with_tree().tree_cap(),
        Some(DEFAULT_TREE_CAP)
    );
    assert_eq!(DEFAULT_TREE_CAP.get(), 4096);
    assert_eq!(
        Mapping::detailed().with_detail(Detail::TopLevel),
        Mapping::new()
    );
}

#[test]
fn test_status_from_unsized_error() {
    #[derive(Error, Debug)]
    #[error("dynamic")]
    struct Dynamic(#[source] Leaf);
    impl Code for Dynamic {}

    trait Both: std::error::Error + Code {}
    impl Both for Dynamic {}

    let error: Box<dyn Both> = Box::new(Dynamic(Leaf));
    let status = Mapping::detailed().status(&*error);
    assert_eq!(status.message(), "dynamic: connection refused");
}

// --- chain length limit ---

/// A chain of `levels` messages, built without recursion.
fn long_chain(levels: usize) -> Detached {
    (1..levels).fold(Detached::new("level"), |source, _| {
        Detached::new("level").with_source(source)
    })
}

/// One leaf with an empty context and `levels` empty messages, as it would
/// arrive from a server.
fn long_chain_trailer(levels: usize) -> Vec<u8> {
    let mut errors = DetachedErrors::new();
    errors.push(String::new(), Detached::new(""));
    let mut bytes = tree::encode(&errors, Detail::Chain).unwrap();
    // Version, one node, leaf tag, empty context; then the chain.
    assert_eq!(bytes, [1, 1, 0, 0, 1, 0]);
    bytes.truncate(4);
    let mut count = levels;
    while count >= 0x80 {
        bytes.push((count & 0x7f) as u8 | 0x80);
        count >>= 7;
    }
    bytes.push(count as u8);
    bytes.extend(std::iter::repeat_n(0, levels));
    bytes
}

#[test]
fn test_decode_rejects_overlong_chain() {
    let longest = tree::decode(&long_chain_trailer(tree::MAX_CHAIN)).unwrap();
    assert_eq!(longest.len(), 1);

    assert_eq!(tree::decode(&long_chain_trailer(tree::MAX_CHAIN + 1)), None);
    // Without the limit this input builds a chain whose drop overflows the
    // stack and aborts the process.
    assert_eq!(tree::decode(&long_chain_trailer(1_000_000)), None);
}

#[test]
fn test_overlong_chain_in_trailer_falls_back_to_violations() {
    let mut status = flat().into_detailed_status();
    status.metadata_mut().insert_bin(
        TREE_TRAILER,
        tonic::metadata::BinaryMetadataValue::from_bytes(&long_chain_trailer(1_000_000)),
    );
    assert_eq!(status.to_many_errors().unwrap().len(), 2);
}

#[test]
fn test_to_detached_falls_back_for_overlong_debug_info() {
    let with_entries = |count: usize| {
        Status::with_error_details(
            tonic::Code::Internal,
            "plain message",
            tonic_types::ErrorDetails::with_debug_info(vec![String::from("level"); count], ""),
        )
    };

    let longest = with_entries(tree::MAX_CHAIN).to_detached();
    assert_eq!(sources(&longest).count(), tree::MAX_CHAIN);
    assert_eq!(longest, long_chain(tree::MAX_CHAIN));

    assert_eq!(
        with_entries(tree::MAX_CHAIN + 1).to_detached(),
        Detached::new("plain message")
    );
    // Without the limit dropping this result overflows the stack.
    assert_eq!(
        with_entries(1_000_000).to_detached(),
        Detached::new("plain message")
    );
}

#[test]
fn test_trailer_dropped_for_overlong_leaf_chain() {
    let aggregate = |levels: usize| {
        let mut errors = ManyErrors::<&str, Detached>::new();
        errors.push("field", long_chain(levels));
        errors
    };
    // A cap far above the size, so only the chain length decides.
    let mapping = tree_cap(1 << 20);

    let status = aggregate(tree::MAX_CHAIN).into_status_with(&mapping);
    assert_eq!(
        status.to_many_errors(),
        Some(detach_leaves(&aggregate(tree::MAX_CHAIN)))
    );

    let status = aggregate(tree::MAX_CHAIN + 1).into_status_with(&mapping);
    assert!(status.metadata().get_bin(TREE_TRAILER).is_none());
    assert_eq!(violations(&status).len(), 1);
    // Only the top level decides how much is sent, so the default mapping
    // still sends the tree.
    let status = aggregate(tree::MAX_CHAIN + 1).into_status_with(&Mapping::new().with_tree());
    assert!(status.metadata().get_bin(TREE_TRAILER).is_some());
}
