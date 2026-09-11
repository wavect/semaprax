//! Golden byte-determinism and hostile-input coverage for the Public
//! Generic Descriptor v1 reference codec. Every fixture here is
//! hand-constructed, not derived from checked HIR — see the module's own
//! doc comment for why that is this round's deliberate scope.

use super::*;
use crate::public_generic_abi::frame;

fn sample_input() -> InstanceBinding {
    InstanceBinding {
        term: "@11:sample.pair<bytes,bool>".to_owned(),
        instance_digest: "sha256:input-digest-fixture".to_owned(),
    }
}

fn sample_result() -> InstanceBinding {
    InstanceBinding {
        term: "@11:sample.pair<bytes,i64>".to_owned(),
        instance_digest: "sha256:result-digest-fixture".to_owned(),
    }
}

fn sample() -> DescriptorV1 {
    DescriptorV1::new(
        "sample.transform",
        "transform",
        "sha256:program-root-fixture",
        "sha256:source-projection-fixture",
        "sha256:public-surface-fixture",
        sample_input(),
        sample_result(),
    )
}

#[test]
fn encode_is_deterministic() {
    assert_eq!(sample().encode(), sample().encode());
    assert_eq!(sample().identity_digest(), sample().identity_digest());
}

#[test]
fn encode_decode_round_trips() {
    let original = sample();
    let decoded = decode(&original.encode()).expect("well-formed bytes decode");
    assert_eq!(decoded, original);
    assert_eq!(decoded.identity_digest(), original.identity_digest());
}

#[test]
fn a_display_rename_changes_wire_bytes_but_not_identity() {
    let original = sample();
    let renamed = original.clone().with_export_name("transform_v2");

    assert_ne!(
        original.encode(),
        renamed.encode(),
        "wire bytes carry the name"
    );
    assert_eq!(
        original.identity_digest(),
        renamed.identity_digest(),
        "a rename must not move identity"
    );

    // Replay against the old trusted value still succeeds: only the
    // presentation-only trailing field changed.
    let replayed = replay(&renamed.encode(), &original).expect("a rename alone still replays");
    assert_eq!(replayed.export_name(), "transform_v2");
}

#[test]
fn replay_accepts_an_identical_candidate() {
    let trusted = sample();
    let replayed = replay(&trusted.encode(), &trusted).expect("identical bytes replay");
    assert_eq!(replayed, trusted);
}

#[test]
fn decode_rejects_truncated_bytes() {
    let mut bytes = sample().encode();
    bytes.truncate(bytes.len() - 3);
    let error = decode(&bytes).expect_err("truncated bytes must not decode");
    assert_eq!(error.code, MALFORMED_DESCRIPTOR);
}

#[test]
fn decode_rejects_trailing_bytes() {
    let mut bytes = sample().encode();
    bytes.push(0xFF);
    let error = decode(&bytes).expect_err("trailing bytes must not decode");
    assert_eq!(error.code, MALFORMED_DESCRIPTOR);
    assert!(error.message.contains("trailing bytes"));
}

#[test]
fn decode_rejects_an_unknown_schema_literal() {
    let mut bytes = Vec::new();
    frame(&mut bytes, b"semaprax.some-other-schema.v7");
    frame(&mut bytes, BOUNDARY_PROFILE_SCHEMA.as_bytes());
    frame(&mut bytes, PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA.as_bytes());
    for _ in 0..8 {
        frame(&mut bytes, b"placeholder");
    }
    let error = decode(&bytes).expect_err("an unknown schema must not decode");
    assert_eq!(error.code, MALFORMED_DESCRIPTOR);
    assert!(error.message.contains("unknown descriptor schema"));
}

#[test]
fn decode_rejects_an_oversized_length_claim() {
    let mut bytes = Vec::new();
    // A length prefix claiming far more than the buffer actually holds.
    bytes.extend_from_slice(&(u64::MAX).to_le_bytes());
    bytes.extend_from_slice(b"short");
    let error = decode(&bytes).expect_err("an oversized length claim must not decode");
    assert_eq!(error.code, MALFORMED_DESCRIPTOR);
}

#[test]
fn decode_rejects_bytes_reordered_from_the_canonical_field_order() {
    // Swap the first two fields (schema, boundary_profile). The bytes still
    // frame cleanly, but the schema no longer reads as the descriptor schema
    // literal, so this must still fail closed rather than silently accepting
    // a different field order.
    let mut bytes = Vec::new();
    frame(&mut bytes, BOUNDARY_PROFILE_SCHEMA.as_bytes());
    frame(&mut bytes, DESCRIPTOR_SCHEMA.as_bytes());
    frame(&mut bytes, PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA.as_bytes());
    for _ in 0..8 {
        frame(&mut bytes, b"placeholder");
    }
    let error = decode(&bytes).expect_err("a reordered descriptor must not decode");
    assert_eq!(error.code, MALFORMED_DESCRIPTOR);
}

#[test]
fn decode_rejects_a_total_size_over_the_wire_bound() {
    let oversized = vec![0u8; MAX_DESCRIPTOR_WIRE_BYTES + 1];
    let error = decode(&oversized).expect_err("an over-bound blob must not decode");
    assert_eq!(error.code, DESCRIPTOR_CAPACITY);
}

#[test]
fn replay_rejects_a_cross_paired_descriptor_on_every_bound_field() {
    type Mutator = fn(DescriptorV1) -> DescriptorV1;

    let trusted = sample();
    let mutators: Vec<(&str, Mutator)> = vec![
        ("export_id", |mut d: DescriptorV1| {
            d.export_id = "different.export".to_owned();
            d
        }),
        ("program_root_digest", |mut d: DescriptorV1| {
            d.program_root_digest = "sha256:different-program-root".to_owned();
            d
        }),
        ("source_projection_digest", |mut d: DescriptorV1| {
            d.source_projection_digest = "sha256:different-source-projection".to_owned();
            d
        }),
        ("public_surface_digest", |mut d: DescriptorV1| {
            d.public_surface_digest = "sha256:different-public-surface".to_owned();
            d
        }),
        ("input.term", |mut d: DescriptorV1| {
            d.input.term = "@4:different<>".to_owned();
            d
        }),
        ("input.instance_digest", |mut d: DescriptorV1| {
            d.input.instance_digest = "sha256:different-input-digest".to_owned();
            d
        }),
        ("result.term", |mut d: DescriptorV1| {
            d.result.term = "@4:different<>".to_owned();
            d
        }),
        ("result.instance_digest", |mut d: DescriptorV1| {
            d.result.instance_digest = "sha256:different-result-digest".to_owned();
            d
        }),
    ];
    for (field, mutate) in mutators {
        let cross_paired = mutate(trusted.clone());
        let error = replay(&cross_paired.encode(), &trusted)
            .expect_err(&format!("a mismatched {field} must not replay"));
        assert_eq!(error.code, DESCRIPTOR_REPLAY_MISMATCH, "field: {field}");
    }
}

#[test]
fn replay_rejects_a_stale_boundary_profile_or_type_grammar_version() {
    let trusted = sample();
    let mut stale_profile = trusted.clone();
    stale_profile.boundary_profile = "semaprax.public-generic-boundary-profile.v2".to_owned();
    let error = replay(&stale_profile.encode(), &trusted)
        .expect_err("a stale boundary profile must not replay");
    assert_eq!(error.code, DESCRIPTOR_VERSION_MISMATCH);

    let mut stale_grammar = trusted.clone();
    stale_grammar.type_grammar_schema = "semaprax.public-generic-type-grammar.v2".to_owned();
    let error = replay(&stale_grammar.encode(), &trusted)
        .expect_err("a stale type grammar version must not replay");
    assert_eq!(error.code, DESCRIPTOR_VERSION_MISMATCH);
}

#[test]
fn instance_binding_reuses_the_grammar_facts_rather_than_reinventing_them() {
    let facts = InstanceFacts {
        term: "@11:sample.pair<bytes,bool>".to_owned(),
        term_digest: "sha256:term-fixture".to_owned(),
        template: crate::public_generic_type::TemplateIdentity {
            declaration: "sample.pair".to_owned(),
            name: "Pair".to_owned(),
            arity: 2,
            parameters: Vec::new(),
            digest: "sha256:template-fixture".to_owned(),
        },
        arguments: Vec::new(),
        fields: Vec::new(),
        owned_leaves: vec!["@4:left".to_owned()],
        instance_digest: "sha256:instance-fixture".to_owned(),
    };
    let binding = InstanceBinding::from_facts(&facts);
    assert_eq!(binding.term, facts.term);
    assert_eq!(binding.instance_digest, facts.instance_digest);
}
