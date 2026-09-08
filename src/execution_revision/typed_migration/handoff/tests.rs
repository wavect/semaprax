use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord};

fn handoff() -> Handoff {
    Handoff {
        migration: canonical(
            json!({"digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000","facts":{},"schema":"root"}),
        ),
        value: RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("state"),
            fields: vec![RetainedField {
                field: DeclarationId::new("count"),
                value: RetainedValue::I64(1),
            }],
        }),
        usage: CheckpointUsage {
            calls: 1,
            argument_bytes: 2,
            result_bytes: 3,
            reserved_fuel: 5,
        },
        iterations: 1,
        stages: 2,
        max_reserved_fuel: 5,
    }
}

#[test]
fn exact_roundtrip_requires_the_independently_trusted_digest() {
    let handoff = handoff();
    let document = handoff.canonical_json();
    assert_eq!(
        Handoff::decode(&document, &handoff.digest())
            .unwrap()
            .canonical_json(),
        document
    );
    assert!(Handoff::decode(
        &document,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    )
    .is_err());
}

#[test]
fn hostile_keys_refunds_kinds_and_noncanonical_numbers_fail_closed() {
    let handoff = handoff();
    let document = handoff.canonical_json();
    let digest = handoff.digest();
    assert!(Handoff::decode(&document.replacen("{", "{\"unknown\":0,", 1), &digest).is_err());
    assert!(Handoff::decode(
        &document.replacen("\"calls\":\"1\"", "\"calls\":\"01\"", 1),
        &digest
    )
    .is_err());
    assert!(Handoff::decode(
        &document.replacen("\"reserved_fuel\":\"5\"", "\"reserved_fuel\":\"6\"", 1),
        &digest
    )
    .is_err());
    assert!(Handoff::decode(
        &document.replacen("\"kind\":\"record\"", "\"kind\":\"bytes\"", 1),
        &digest
    )
    .is_err());
}
