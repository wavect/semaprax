//! Authentic descriptor pairs, not JSON forgeries or stale-digest failures.
//! The shared subject facts deliberately isolate API replay; they do not claim
//! that an edited real Project would retain its revision or graph digest.

use super::*;
use semaprax::project::FlatOwnedRecordApiDescriptor;
use serde_json::Value;

fn selected() -> Vec<String> {
    vec!["frame.info".to_owned()]
}

fn replace(source: &str, before: &str, after: &str, count: usize) -> String {
    assert_eq!(source.matches(before).count(), count);
    source.replace(before, after)
}

fn derive(source: &str) -> (hir::ResolvedProgram, FlatOwnedRecordApiDescriptor) {
    let program = resolve(source);
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &selected(), subject()).unwrap();
    assert_eq!(descriptor.exports().len(), 1);
    assert_eq!(descriptor.exports()[0].stable_id().as_str(), "frame.info");
    assert_eq!(descriptor.project_revision(), PROJECT_REVISION);
    let bytes = descriptor.canonical_bytes();
    assert_eq!(descriptor.digest(), digest(&bytes));
    assert_eq!(
        replay_flat_owned_record_api_descriptor(
            &program,
            &selected(),
            subject(),
            &bytes,
            &descriptor.digest(),
        )
        .unwrap(),
        descriptor
    );
    (program, descriptor)
}

fn facts(descriptor: &FlatOwnedRecordApiDescriptor) -> Value {
    serde_json::from_slice(&descriptor.canonical_bytes()).unwrap()
}

fn host_name(prefix: &str, identity: &str) -> String {
    let mut value = prefix.to_owned();
    for byte in identity.bytes() {
        use std::fmt::Write as _;
        write!(value, "{byte:02x}").unwrap();
    }
    value
}

fn rejects_other_hir(program: &hir::ResolvedProgram, descriptor: &FlatOwnedRecordApiDescriptor) {
    let bytes = descriptor.canonical_bytes();
    assert_eq!(descriptor.digest(), digest(&bytes));
    let error = replay_flat_owned_record_api_descriptor(
        program,
        &selected(),
        subject(),
        &bytes,
        &descriptor.digest(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-J113");
    assert_eq!(
        error.message,
        "flat owned-record descriptor does not replay against retained HIR"
    );
}

#[test]
fn authentic_record_and_parameter_fact_changes_reject_bidirectional_cross_replay() {
    let (original_program, original) = derive(SOURCE);
    let original_facts = facts(&original);
    let mut cases = Vec::new();

    let source = replace(
        SOURCE,
        "@id(\"frame.info.type\")",
        "@id(\"frame.info.changed-type\")",
        1,
    );
    let mut expected = original_facts.clone();
    expected["exports"][0]["result"]["record_id"] = "frame.info.changed-type".into();
    expected["exports"][0]["result"]["record_host_name"] =
        host_name("SpxRecordId", "frame.info.changed-type").into();
    cases.push(("record-id", source, expected));

    let source = replace(
        SOURCE,
        "@id(\"frame.info.payload\")",
        "@id(\"frame.info.changed-payload\")",
        1,
    );
    let mut expected = original_facts.clone();
    expected["exports"][0]["result"]["fields"][0]["stable_id"] =
        "frame.info.changed-payload".into();
    expected["exports"][0]["result"]["fields"][0]["host_name"] =
        host_name("spx_field_id_", "frame.info.changed-payload").into();
    cases.push(("field-id", source, expected));

    let source = replace(SOURCE, "kind: i64,", "kind: bool,", 1);
    let source = replace(&source, "kind: 7,", "kind: true,", 1);
    let mut expected = original_facts.clone();
    expected["exports"][0]["result"]["fields"][1]["type"] = "bool".into();
    cases.push(("field-type", source, expected));

    let source = replace(SOURCE, "kind: i64,", "kind: usize,", 1);
    let source = replace(&source, "kind: 7,", "kind: 7usize,", 1);
    let mut expected = original_facts.clone();
    expected["exports"][0]["result"]["fields"][1]["type"] = "usize".into();
    cases.push(("field-type-usize", source, expected));

    let source = replace(
        SOURCE,
        "    @id(\"frame.info.payload\") payload: Bytes,\n    @id(\"frame.info.kind\") kind: i64,\n    @id(\"frame.info.valid\") valid: bool,\n    @id(\"frame.info.size\") size: usize,",
        "    @id(\"frame.info.kind\") kind: i64,\n    @id(\"frame.info.valid\") valid: bool,\n    @id(\"frame.info.size\") size: usize,\n    @id(\"frame.info.payload\") payload: Bytes,",
        1,
    );
    let source = replace(
        &source,
        "        payload: bytes_copy(value),\n        kind: 7,\n        valid: valid,\n        size: byte_len(value),",
        "        kind: 7,\n        valid: valid,\n        size: byte_len(value),\n        payload: bytes_copy(value),",
        1,
    );
    let mut expected = original_facts.clone();
    let fields = expected["exports"][0]["result"]["fields"]
        .as_array_mut()
        .unwrap();
    let owned = fields.remove(0);
    fields.push(owned);
    for (ordinal, field) in fields.iter_mut().enumerate() {
        field["ordinal"] = ordinal.into();
    }
    cases.push(("field-order", source, expected));

    let source = replace(SOURCE, "payload:", "renamed_payload:", 2);
    let mut expected = original_facts.clone();
    expected["exports"][0]["result"]["fields"][0]["source_name"] = "renamed_payload".into();
    cases.push(("field-name", source, expected));

    let source = replace(SOURCE, "FrameInfo", "RenamedInfo", 3);
    let mut expected = original_facts.clone();
    expected["exports"][0]["result"]["record_source_name"] = "RenamedInfo".into();
    cases.push(("record-name", source, expected));

    let source = replace(SOURCE, "value: borrow Slice<u8>", "value: borrow str", 1);
    let source = replace(
        &source,
        "bytes_copy(value)",
        "bytes_copy(str_as_bytes(value))",
        1,
    );
    let source = replace(
        &source,
        "byte_len(value)",
        "byte_len(str_as_bytes(value))",
        1,
    );
    let mut expected = original_facts.clone();
    expected["exports"][0]["parameters"][0]["type"] = "borrow-str".into();
    cases.push(("parameter-type", source, expected));

    let source = replace(
        SOURCE,
        "value: borrow Slice<u8>",
        "renamed: borrow Slice<u8>",
        1,
    );
    let source = replace(&source, "bytes_copy(value)", "bytes_copy(renamed)", 1);
    let source = replace(&source, "byte_len(value)", "byte_len(renamed)", 1);
    let mut expected = original_facts.clone();
    expected["exports"][0]["parameters"][0]["source_name"] = "renamed".into();
    cases.push(("parameter-name", source, expected));

    assert_eq!(cases.len(), 9);
    for (label, source, expected) in cases {
        assert_ne!(source, SOURCE, "{label}");
        let (candidate_program, candidate) = derive(&source);
        // Permit only the explicitly authored fact delta, not arbitrary changes
        // to another field, signature, ordinal, limit, or subject binding.
        assert_eq!(facts(&candidate), expected, "{label}");
        assert_ne!(facts(&candidate), original_facts, "{label}");
        assert_eq!(
            candidate.carrier_plans()[0].owned_field_ordinal,
            if label == "field-order" { 3 } else { 0 }
        );
        assert_ne!(
            candidate.canonical_bytes(),
            original.canonical_bytes(),
            "{label}"
        );
        assert_ne!(candidate.digest(), original.digest(), "{label}");
        rejects_other_hir(&original_program, &candidate);
        rejects_other_hir(&candidate_program, &original);
    }
}

#[test]
fn function_display_name_and_body_changes_preserve_the_semantic_api_descriptor() {
    let (original_program, original) = derive(SOURCE);
    let original_function = original_program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "frame.info")
        .unwrap();
    for (rename, source) in [
        (
            true,
            replace(SOURCE, "fn frame_info(", "fn renamed_info(", 1),
        ),
        (false, replace(SOURCE, "kind: 7,", "kind: 8,", 1)),
    ] {
        let (candidate_program, candidate) = derive(&source);
        let function = candidate_program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "frame.info")
            .unwrap();
        if rename {
            assert_eq!(function.name, "renamed_info");
            assert_ne!(function.name, original_function.name);
        } else {
            assert_eq!(function.name, original_function.name);
            assert_ne!(function.body, original_function.body);
        }
        assert_eq!(candidate, original);
        assert_eq!(candidate.canonical_bytes(), original.canonical_bytes());
        assert_eq!(candidate.digest(), original.digest());
        for (program, descriptor) in [
            (&original_program, &candidate),
            (&candidate_program, &original),
        ] {
            assert_eq!(
                replay_flat_owned_record_api_descriptor(
                    program,
                    &selected(),
                    subject(),
                    &descriptor.canonical_bytes(),
                    &descriptor.digest(),
                )
                .unwrap(),
                *descriptor
            );
        }
    }
}
