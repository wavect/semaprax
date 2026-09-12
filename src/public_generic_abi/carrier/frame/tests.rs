//! Coverage for [Public Generic Carrier v1's canonical carrier
//! bytes](../../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#canonical-carrier-bytes):
//! golden byte-determinism, the required frame hostile-input matrix
//! (duplicate/missing/extra/reordered leaf, truncation at every byte
//! boundary, trailing bytes, noncanonical length, each first-over-bound
//! case), and — driven through the real parser, resolver, descriptor
//! producer, and independent descriptor verifier, never a hand-built
//! fixture — [`CarrierFrameBinding::from_verified_descriptor`]'s own
//! requirement that only a real `VerifiedPublicGenericDescriptor` can
//! produce a trusted binding.

use std::path::Path;

use super::*;
use crate::hir::{self, ResolvedProgram};
use crate::parse;
use crate::public_generic_abi::descriptor::producer;
use crate::public_generic_abi::descriptor::verify::{
    verify_public_generic_descriptor, VerificationOptions,
};

// ---------------------------------------------------------------------
// A real verified descriptor fixture
// ---------------------------------------------------------------------
//
// This is a THIRD independent copy of the program-root digest algorithm
// [Public Generic Descriptor
// v1](../../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#derivation-from-checked-facts-the-producer)
// documents, alongside the producer's own `declaration_identity_preimage`
// and the verifier's own `recompute_program_root_digest` — deliberately not
// shared with either, for the same reason the verifier module gives for not
// sharing with the producer: this is test-fixture bootstrapping, not
// trust-boundary logic, so duplicating it costs nothing and buys the same
// "a bug in one collection doesn't silently satisfy another" property one
// more time, for the one caller (this test module) with no other way to
// obtain a real `VerifiedPublicGenericDescriptor`.

const REVISION: &str = "carrier-frame-test-revision-1";
const EXPORT_ID: &str = "frame.split";

/// `Pair<Leaf, i64>` in, `Leaf` out: input and result are deliberately
/// *different* instances (mirroring the descriptor verifier's own `split`
/// fixture) so `input_facts()` and `result_facts()` genuinely differ —
/// same-shaped input/output would make the two directions' facts
/// coincidentally identical and hide a "always reads input, ignores the
/// requested direction" bug.
const SOURCE: &str = r#"
module test.public_generic_carrier_frame;

@id("frame.leaf")
record Leaf {
    @id("frame.leaf.head")
    head: Bytes,
}

@id("frame.pair")
record Pair<T, U> {
    @id("frame.pair.left")
    left: T,
    @id("frame.pair.right")
    right: U,
}

@id("frame.split")
fn split(value: own Pair<Leaf, i64>) -> Leaf {
    match own value {
        Pair { left: Leaf { head: payload }, right: _count } => Leaf { head: payload },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("carrier-frame.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn program_root_digest(program: &ResolvedProgram) -> String {
    let mut ids: Vec<&str> = Vec::new();
    ids.extend(
        program
            .types
            .iter()
            .map(|declaration| declaration.id.as_str()),
    );
    ids.extend(
        program
            .function_templates
            .iter()
            .map(|declaration| declaration.id.as_str()),
    );
    ids.extend(
        program
            .functions
            .iter()
            .map(|function| function.id.as_str()),
    );
    ids.extend(
        program
            .function_instances
            .iter()
            .map(|instance| instance.id.as_str()),
    );
    ids.sort_unstable();
    ids.dedup();

    let mut preimage = Vec::new();
    frame(&mut preimage, &(ids.len() as u64).to_le_bytes());
    for id in ids {
        frame(&mut preimage, id.as_bytes());
    }
    digest(
        b"semaprax.public-generic-descriptor.v1.program-root\0",
        &preimage,
    )
}

fn verified_descriptor() -> VerifiedPublicGenericDescriptor {
    let program = resolved(SOURCE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, EXPORT_ID).unwrap();
    let root = program_root_digest(&program);
    verify_public_generic_descriptor(
        &program,
        REVISION,
        EXPORT_ID,
        &root,
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap()
}

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

fn sample_leaves() -> Vec<CarrierLeaf> {
    vec![
        CarrierLeaf::new("leaf.0", LeafKind::Bytes, b"hello".to_vec()),
        CarrierLeaf::new("leaf.1", LeafKind::Bytes, b"world!!".to_vec()),
    ]
}

/// The exact `leaf_inventory_digest` [`sample_binding`] itself derives for
/// `["leaf.0", "leaf.1"]` — computed the same way
/// [`CarrierFrameBinding::new`] does internally, so [`sample_frame`] and
/// [`sample_binding`] are bound to each other by construction rather than by
/// two independently hand-picked placeholder strings that happen to need to
/// agree.
fn sample_leaf_inventory_digest() -> String {
    digest(
        LEAF_INVENTORY_DOMAIN,
        &framed_leaf_paths(&["leaf.0".to_owned(), "leaf.1".to_owned()]),
    )
}

fn sample_frame() -> LogicalCarrierFrame {
    LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        sample_leaf_inventory_digest(),
        sample_leaves(),
    )
}

fn sample_binding() -> CarrierFrameBinding {
    CarrierFrameBinding::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        vec!["leaf.0".to_owned(), "leaf.1".to_owned()],
    )
}

// ---------------------------------------------------------------------
// Golden bytes, determinism, and round trip
// ---------------------------------------------------------------------

#[test]
fn encode_is_deterministic() {
    assert_eq!(sample_frame().encode(), sample_frame().encode());
    assert_eq!(
        sample_frame().carrier_facts_digest(),
        sample_frame().carrier_facts_digest()
    );
}

#[test]
fn minimal_valid_frame_round_trips() {
    let frame = LogicalCarrierFrame::new(
        Direction::Result,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        Vec::new(),
    );
    let bytes = frame.encode();
    let decoded = parse_bounded(&bytes).expect("a zero-leaf frame is still a valid frame");
    assert_eq!(decoded, frame);
    assert_eq!(decoded.leaves().len(), 0);
    assert_eq!(decoded.total_payload_length(), 0);
}

#[test]
fn frame_with_a_zero_length_leaf_round_trips() {
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        vec![CarrierLeaf::new("leaf.0", LeafKind::Bytes, Vec::new())],
    );
    let decoded = parse_bounded(&frame.encode()).expect("a zero-length Bytes leaf must parse");
    assert_eq!(decoded.leaves()[0].payload(), b"");
}

#[test]
fn frame_with_an_embedded_zero_byte_round_trips() {
    let payload = vec![1u8, 0u8, 2u8, 0u8, 0u8, 3u8];
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        vec![CarrierLeaf::new("leaf.0", LeafKind::Bytes, payload.clone())],
    );
    let decoded = parse_bounded(&frame.encode()).unwrap();
    assert_eq!(decoded.leaves()[0].payload(), payload.as_slice());
}

#[test]
fn full_round_trip_preserves_every_field() {
    let original = sample_frame();
    let decoded = parse_bounded(&original.encode()).expect("a well-formed frame must decode");
    assert_eq!(decoded, original);
    assert_eq!(decoded.direction(), Direction::Input);
    assert_eq!(decoded.leaves()[0].path(), "leaf.0");
    assert_eq!(decoded.leaves()[0].payload(), b"hello");
    assert_eq!(decoded.leaves()[1].path(), "leaf.1");
    assert_eq!(decoded.leaves()[1].payload(), b"world!!");
}

// ---------------------------------------------------------------------
// Bounds and first-over-bound cases
// ---------------------------------------------------------------------

#[test]
fn a_leaf_at_the_max_bytes_per_leaf_bound_is_admitted_and_one_over_is_refused() {
    let at_bound = vec![CarrierLeaf::new(
        "leaf.0",
        LeafKind::Bytes,
        vec![7u8; MAX_BYTES_PER_LEAF],
    )];
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        at_bound,
    );
    parse_bounded(&frame.encode()).expect("exactly the per-leaf byte bound must be admitted");

    let over_bound = vec![CarrierLeaf::new(
        "leaf.0",
        LeafKind::Bytes,
        vec![7u8; MAX_BYTES_PER_LEAF + 1],
    )];
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        over_bound,
    );
    let error = parse_bounded(&frame.encode())
        .expect_err("one byte over the per-leaf bound must be refused");
    assert_eq!(error.code, CARRIER_CAPACITY);
}

#[test]
fn total_payload_one_under_the_bound_is_admitted() {
    // Completes the one-under/exactly-at/one-over triple for the
    // total-payload bound: `total_payload_at_the_bound_with_max_sized_leaves_is_admitted`
    // below proves exactly-at, and
    // `a_declared_total_payload_length_over_the_bound_is_refused_before_any_leaf_is_read`
    // proves one-over, but neither exercises one byte *under* the bound with
    // real leaf data. The last leaf is one byte short of
    // `MAX_BYTES_PER_LEAF` so the real, decoded total is exactly
    // `MAX_TOTAL_PAYLOAD_BYTES - 1`, never trusting a declared field alone.
    assert_eq!(
        MAX_OWNED_LEAVES_PER_INSTANCE * MAX_BYTES_PER_LEAF,
        MAX_TOTAL_PAYLOAD_BYTES,
        "this fixture assumes the three bounds are related exactly this way"
    );
    let leaves: Vec<CarrierLeaf> = (0..MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|index| {
            let len = if index + 1 == MAX_OWNED_LEAVES_PER_INSTANCE {
                MAX_BYTES_PER_LEAF - 1
            } else {
                MAX_BYTES_PER_LEAF
            };
            CarrierLeaf::new(format!("leaf.{index}"), LeafKind::Bytes, vec![9u8; len])
        })
        .collect();
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        leaves,
    );
    let decoded = parse_bounded(&frame.encode())
        .expect("one byte under the total-payload bound must be admitted");
    assert_eq!(
        decoded.total_payload_length(),
        MAX_TOTAL_PAYLOAD_BYTES as u64 - 1
    );
}

#[test]
fn total_payload_at_the_bound_with_max_sized_leaves_is_admitted() {
    // `MAX_TOTAL_PAYLOAD_BYTES` (16 MiB) equals exactly
    // `MAX_OWNED_LEAVES_PER_INSTANCE * MAX_BYTES_PER_LEAF` (256 * 64 KiB), so
    // this is the one construction that reaches the total-payload bound
    // exactly without first tripping the leaf-count or per-leaf bound —
    // a heavier, distinct positive case from
    // `leaf_count_at_the_bound_is_admitted_and_one_over_is_refused`'s own
    // zero-length leaves.
    assert_eq!(
        MAX_OWNED_LEAVES_PER_INSTANCE * MAX_BYTES_PER_LEAF,
        MAX_TOTAL_PAYLOAD_BYTES,
        "this fixture assumes the three bounds are related exactly this way"
    );
    let leaves: Vec<CarrierLeaf> = (0..MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|index| {
            CarrierLeaf::new(
                format!("leaf.{index}"),
                LeafKind::Bytes,
                vec![9u8; MAX_BYTES_PER_LEAF],
            )
        })
        .collect();
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        leaves,
    );
    let decoded =
        parse_bounded(&frame.encode()).expect("exactly the total-payload bound must be admitted");
    assert_eq!(
        decoded.total_payload_length(),
        MAX_TOTAL_PAYLOAD_BYTES as u64
    );
}

#[test]
fn a_declared_total_payload_length_over_the_bound_is_refused_before_any_leaf_is_read() {
    // Given the exact relationship pinned above, the only way to observe
    // the declared-total-payload-length check firing on its own — never
    // reachable through real leaf bytes once the leaf-count and per-leaf
    // bounds are already enforced — is a header that lies before any leaf
    // is even read: zero leaves declared, but a `total_payload_length`
    // header one byte over the bound. This proves the header-level check
    // runs independently of, and before, any leaf allocation.
    let mut bytes = Vec::new();
    frame(&mut bytes, CARRIER_SCHEMA.as_bytes());
    frame(&mut bytes, b"input");
    frame(&mut bytes, b"sha256:d");
    frame(&mut bytes, b"sha256:e");
    frame(&mut bytes, b"sha256:i");
    frame(&mut bytes, b"sha256:l");
    bytes.extend_from_slice(&0u64.to_le_bytes()); // leaf_count = 0
    bytes.extend_from_slice(&(MAX_TOTAL_PAYLOAD_BYTES as u64 + 1).to_le_bytes());
    let computed_digest = digest(FRAME_DOMAIN, &bytes);
    frame(&mut bytes, computed_digest.as_bytes());
    let error = parse_bounded(&bytes)
        .expect_err("a declared total payload length over the bound must be refused");
    assert_eq!(error.code, CARRIER_CAPACITY);
}

#[test]
fn leaf_count_at_the_bound_is_admitted_and_one_over_is_refused() {
    let at_bound: Vec<CarrierLeaf> = (0..MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|index| CarrierLeaf::new(format!("leaf.{index}"), LeafKind::Bytes, Vec::new()))
        .collect();
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        at_bound,
    );
    parse_bounded(&frame.encode()).expect("exactly the leaf-count bound must be admitted");

    // One over the bound: tamper the encoded `leaf_count` header directly,
    // since `LogicalCarrierFrame::new` has no separate admission check of
    // its own (that is `parse_bounded`'s job, exercised here) — this proves
    // the wire-level bound independent of whether a well-behaved caller
    // would ever construct that many leaves in memory first.
    let over_bound: Vec<CarrierLeaf> = (0..=MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|index| CarrierLeaf::new(format!("leaf.{index}"), LeafKind::Bytes, Vec::new()))
        .collect();
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        over_bound,
    );
    let error = parse_bounded(&frame.encode())
        .expect_err("one leaf over the leaf-count bound must be refused");
    assert_eq!(error.code, CARRIER_CAPACITY);
}

// ---------------------------------------------------------------------
// Hostile leaf-inventory shapes
// ---------------------------------------------------------------------

#[test]
fn a_duplicate_leaf_path_is_rejected_at_parse_time() {
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:d",
        "sha256:e",
        "sha256:i",
        "sha256:l",
        vec![
            CarrierLeaf::new("leaf.0", LeafKind::Bytes, b"a".to_vec()),
            CarrierLeaf::new("leaf.0", LeafKind::Bytes, b"b".to_vec()),
        ],
    );
    let error = parse_bounded(&frame.encode()).expect_err("a duplicate leaf path must never parse");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn a_missing_leaf_is_rejected_by_the_binding_plan() {
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        sample_leaf_inventory_digest(),
        vec![CarrierLeaf::new(
            "leaf.0",
            LeafKind::Bytes,
            b"hello".to_vec(),
        )],
    );
    let error = sample_binding()
        .validate_frame(&frame)
        .expect_err("a frame missing a required leaf must be rejected");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);
    assert!(
        error.message.contains("leaf sequence"),
        "must fail on the leaf-sequence check specifically, not an earlier field: {}",
        error.message
    );
}

#[test]
fn an_extra_leaf_is_rejected_by_the_binding_plan() {
    let mut leaves = sample_leaves();
    leaves.push(CarrierLeaf::new(
        "leaf.2",
        LeafKind::Bytes,
        b"extra".to_vec(),
    ));
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        sample_leaf_inventory_digest(),
        leaves,
    );
    let error = sample_binding()
        .validate_frame(&frame)
        .expect_err("a frame with an extra leaf must be rejected");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);
    assert!(
        error.message.contains("leaf sequence"),
        "must fail on the leaf-sequence check specifically, not an earlier field: {}",
        error.message
    );
}

#[test]
fn a_reordered_leaf_sequence_is_rejected_by_the_binding_plan() {
    let mut leaves = sample_leaves();
    leaves.reverse();
    let frame = LogicalCarrierFrame::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        sample_leaf_inventory_digest(),
        leaves,
    );
    let error = sample_binding()
        .validate_frame(&frame)
        .expect_err("a reordered leaf sequence must be rejected even though the set is identical");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);
    assert!(
        error.message.contains("leaf sequence"),
        "must fail on the leaf-sequence check specifically, not an earlier field: {}",
        error.message
    );
}

// ---------------------------------------------------------------------
// Cross-paired semantic binding ("reminted carrier digest with the wrong
// semantic binding")
// ---------------------------------------------------------------------

#[test]
fn a_self_consistent_frame_bound_to_the_wrong_descriptor_direction_or_endpoint_is_rejected() {
    let frame = sample_frame();
    // The frame decodes cleanly: its own digest is internally consistent.
    let decoded = parse_bounded(&frame.encode()).expect("the frame itself is well-formed");

    let wrong_direction = CarrierFrameBinding::new(
        Direction::Result,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        vec!["leaf.0".to_owned(), "leaf.1".to_owned()],
    );
    let error = wrong_direction
        .validate_frame(&decoded)
        .expect_err("a frame bound to a different direction must be rejected");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    let wrong_descriptor = CarrierFrameBinding::new(
        Direction::Input,
        "sha256:a-different-descriptor",
        "sha256:endpoint-fixture",
        "sha256:instance-fixture",
        vec!["leaf.0".to_owned(), "leaf.1".to_owned()],
    );
    let error = wrong_descriptor
        .validate_frame(&decoded)
        .expect_err("a frame bound to a different descriptor must be rejected");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    let wrong_endpoint = CarrierFrameBinding::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:a-different-endpoint",
        "sha256:instance-fixture",
        vec!["leaf.0".to_owned(), "leaf.1".to_owned()],
    );
    let error = wrong_endpoint
        .validate_frame(&decoded)
        .expect_err("a frame bound to a different endpoint must be rejected");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    let wrong_instance = CarrierFrameBinding::new(
        Direction::Input,
        "sha256:descriptor-fixture",
        "sha256:endpoint-fixture",
        "sha256:a-different-instance",
        vec!["leaf.0".to_owned(), "leaf.1".to_owned()],
    );
    let error = wrong_instance
        .validate_frame(&decoded)
        .expect_err("a frame bound to a different instance must be rejected");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    // The correct plan still accepts it.
    sample_binding()
        .validate_frame(&decoded)
        .expect("the correctly bound plan must still accept the frame");
}

#[test]
fn a_tampered_frame_fails_the_self_digest_check_before_any_binding_is_considered() {
    let original = sample_frame();
    // `body_preimage`'s last bytes are exactly the last leaf's raw payload
    // (nothing follows the leaf loop in the preimage), so flipping its very
    // last byte deterministically corrupts payload content without ever
    // touching a length header or field boundary.
    let mut body = original.body_preimage();
    let last = body.len() - 1;
    body[last] ^= 0xFF;
    // Append the *original*, now-stale digest: the tampered body's own
    // recomputed digest can no longer equal it, so only the self-check can
    // be what rejects this, never a length/framing failure.
    let mut bytes = body;
    frame(&mut bytes, original.carrier_facts_digest().as_bytes());
    let error =
        parse_bounded(&bytes).expect_err("a tampered frame must fail its own digest self-check");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);
}

// ---------------------------------------------------------------------
// Framing hostility: truncation, trailing bytes, noncanonical length
// ---------------------------------------------------------------------

#[test]
fn truncation_at_every_byte_boundary_is_rejected() {
    let bytes = sample_frame().encode();
    for cut in 0..bytes.len() {
        let truncated = &bytes[..cut];
        assert!(
            parse_bounded(truncated).is_err(),
            "truncating to {cut} of {} bytes must never parse",
            bytes.len()
        );
    }
    // The untruncated bytes still parse, so the loop above is exercising
    // real failure, not a codec that never succeeds.
    parse_bounded(&bytes).expect("the full, untruncated frame must still parse");
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = sample_frame().encode();
    bytes.push(0);
    let error =
        parse_bounded(&bytes).expect_err("trailing bytes after a well-formed frame must fail");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn an_unknown_direction_is_rejected() {
    let mut bytes = Vec::new();
    frame(&mut bytes, CARRIER_SCHEMA.as_bytes());
    frame(&mut bytes, b"sideways");
    frame(&mut bytes, b"sha256:d");
    frame(&mut bytes, b"sha256:e");
    frame(&mut bytes, b"sha256:i");
    frame(&mut bytes, b"sha256:l");
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    let computed_digest = digest(FRAME_DOMAIN, &bytes);
    frame(&mut bytes, computed_digest.as_bytes());
    let error = parse_bounded(&bytes).expect_err("an unknown direction must be rejected");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn an_unknown_schema_is_rejected() {
    let mut bytes = Vec::new();
    frame(&mut bytes, b"semaprax.some-other-carrier.v9");
    frame(&mut bytes, b"input");
    frame(&mut bytes, b"sha256:d");
    frame(&mut bytes, b"sha256:e");
    frame(&mut bytes, b"sha256:i");
    frame(&mut bytes, b"sha256:l");
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    let computed_digest = digest(FRAME_DOMAIN, &bytes);
    frame(&mut bytes, computed_digest.as_bytes());
    let error = parse_bounded(&bytes).expect_err("an unknown schema must be rejected");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn an_oversized_length_claim_is_rejected() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(u64::MAX).to_le_bytes());
    bytes.extend_from_slice(b"short");
    let error =
        parse_bounded(&bytes).expect_err("an oversized length claim must never allocate or parse");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn a_noncanonical_declared_total_payload_length_is_rejected() {
    // A self-consistent frame is built, then its declared
    // `total_payload_length` header is corrupted to no longer match the
    // actual sum of leaf payload lengths, and the trailing digest is
    // recomputed over the corrupted body so the self-digest check itself
    // cannot be what catches this — only the explicit declared-vs-actual
    // total check can.
    let frame_value = sample_frame();
    let mut body = frame_value.body_preimage();
    // `total_payload_length` sits right after the six framed identity
    // fields' worth of bytes and the 8-byte leaf_count; locate it by
    // reconstructing the offset the encoder itself used.
    let mut offset = 0usize;
    for _ in 0..6 {
        let (_field, next) = read_frame(&body, offset, usize::MAX).unwrap();
        offset = next;
    }
    offset += 8; // skip leaf_count
    let declared = u64::from_le_bytes(body[offset..offset + 8].try_into().unwrap());
    body[offset..offset + 8].copy_from_slice(&(declared + 1).to_le_bytes());
    let recomputed_digest = digest(FRAME_DOMAIN, &body);
    frame(&mut body, recomputed_digest.as_bytes());

    let error = parse_bounded(&body)
        .expect_err("a declared total payload length that disagrees with the actual sum must fail");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

// ---------------------------------------------------------------------
// A real VerifiedPublicGenericDescriptor-derived binding
// ---------------------------------------------------------------------

#[test]
fn from_verified_descriptor_derives_a_binding_that_accepts_the_matching_frame() {
    let descriptor = verified_descriptor();
    let input_binding =
        CarrierFrameBinding::from_verified_descriptor(&descriptor, Direction::Input);
    assert_eq!(
        input_binding.leaf_paths(),
        descriptor.input_facts().owned_leaves.as_slice(),
        "the binding's leaf inventory must be exactly the verified descriptor's own \
         canonical owned-leaf paths, never re-derived or reordered"
    );

    // Build a frame the same way a real caller would: identity facts and
    // leaf inventory taken straight from the same verified descriptor the
    // binding itself was derived from, so `validate_frame` has a genuine
    // matching frame to accept, not a hand-picked fixture.
    let leaves: Vec<CarrierLeaf> = input_binding
        .leaf_paths()
        .iter()
        .map(|path| CarrierLeaf::new(path.clone(), LeafKind::Bytes, b"payload".to_vec()))
        .collect();
    let bound_frame = LogicalCarrierFrame::new(
        Direction::Input,
        descriptor.descriptor_digest(),
        digest(ENDPOINT_IDENTITY_DOMAIN, descriptor.export_id().as_bytes()),
        descriptor.input_facts().instance_digest.clone(),
        digest(
            LEAF_INVENTORY_DOMAIN,
            &framed_leaf_paths(&descriptor.input_facts().owned_leaves),
        ),
        leaves,
    );
    let decoded = parse_bounded(&bound_frame.encode()).expect("a freshly encoded frame must parse");
    input_binding
        .validate_frame(&decoded)
        .expect("a frame built from the verified descriptor's own facts must validate");
}

#[test]
fn from_verified_descriptor_reads_facts_for_the_requested_direction_not_always_input() {
    let descriptor = verified_descriptor();
    // `frame.split` (`Pair<Leaf, i64>` in, `Leaf` out) was chosen precisely
    // so input and result are genuinely different instances: this assertion
    // would hold trivially, proving nothing, for a same-shaped export.
    assert_ne!(
        descriptor.input_facts().term,
        descriptor.result_facts().term,
        "the fixture export must own genuinely different input/result instances"
    );

    let input_binding =
        CarrierFrameBinding::from_verified_descriptor(&descriptor, Direction::Input);
    let result_binding =
        CarrierFrameBinding::from_verified_descriptor(&descriptor, Direction::Result);

    assert_eq!(input_binding.direction(), Direction::Input);
    assert_eq!(result_binding.direction(), Direction::Result);
    // Pin against a copy-paste bug that always reads `input_facts()`
    // regardless of the requested `direction` argument: each binding's own
    // leaf-path inventory must equal the *matching* direction's own
    // `owned_leaves`, not the other one's, and — because the two directions
    // now genuinely differ — a bug that ignored `direction` would make one
    // of these two assertions fail.
    assert_eq!(
        input_binding.leaf_paths(),
        descriptor.input_facts().owned_leaves.as_slice()
    );
    assert_eq!(
        result_binding.leaf_paths(),
        descriptor.result_facts().owned_leaves.as_slice()
    );
    assert_ne!(input_binding.leaf_paths(), result_binding.leaf_paths());
    assert_ne!(input_binding, result_binding);
}
