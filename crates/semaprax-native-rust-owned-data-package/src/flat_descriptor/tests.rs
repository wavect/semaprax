use super::*;

#[test]
fn retained_root_names_replay_the_same_hand_authored_canonical_oracle() {
    let bytes = include_bytes!("../../../../tests/fixtures/flat_descriptor_retained_names.json");
    let value = replay(
        bytes,
        &flat_descriptor_digest(bytes),
        &["api.value".to_owned()],
    )
    .unwrap();
    assert!(value.exports[0].record_source_name.len() > 128);
    assert_eq!(value.exports[0].record_id, "R\nλ\u{8}\u{c}\u{7f}\u{85}");
    assert_eq!(value.exports[0].fields[0].stable_id, "");
    assert_eq!(render::canonical(&value), bytes.as_slice());
    let canonical = std::str::from_utf8(bytes).unwrap();
    for (exact, alternate) in [
        ("\\u0008", "\\b"),
        ("\\u000c", "\\f"),
        ("\\u007f", "\u{7f}"),
        ("\\u0085", "\u{85}"),
    ] {
        assert!(canonical.contains(exact));
        let alternate = canonical.replacen(exact, alternate, 1);
        assert!(replay(
            alternate.as_bytes(),
            &flat_descriptor_digest(alternate.as_bytes()),
            &["api.value".to_owned()]
        )
        .is_err());
    }
}

#[test]
fn retained_identities_are_nul_free_not_method_identifiers() {
    for id in ["", "Upper", "λ\n", &"x".repeat(129)] {
        let value = descriptor(vec![export(
            "api.first",
            id,
            &"Name".repeat(40),
            vec![field(
                "FIELD\tλ",
                &"field".repeat(40),
                FieldKind::OwnedBytes,
                0,
            )],
        )]);
        let bytes = render::canonical(&value);
        replay(
            &bytes,
            &flat_descriptor_digest(&bytes),
            &["api.first".to_owned()],
        )
        .unwrap();
    }
    let value = descriptor(vec![export(
        "api.first",
        "bad\0id",
        "Packet",
        vec![field("field", "bytes", FieldKind::OwnedBytes, 0)],
    )]);
    let bytes = render::canonical(&value);
    assert!(replay(
        &bytes,
        &flat_descriptor_digest(&bytes),
        &["api.first".to_owned()]
    )
    .is_err());
}

#[test]
fn retained_display_name_uses_the_exact_global_descriptor_bound() {
    let mut value = descriptor(vec![export(
        "api.first",
        "record",
        "P",
        vec![field("field", "bytes", FieldKind::OwnedBytes, 0)],
    )]);
    let overhead = render::canonical(&value).len();
    value.exports[0]
        .record_source_name
        .push_str(&"x".repeat(MAX_DESCRIPTOR_BYTES - overhead));
    let exact = render::canonical(&value);
    assert_eq!(exact.len(), MAX_DESCRIPTOR_BYTES);
    replay(
        &exact,
        &flat_descriptor_digest(&exact),
        &["api.first".to_owned()],
    )
    .unwrap();
    value.exports[0].record_source_name.push('x');
    let oversized = render::canonical(&value);
    assert_eq!(oversized.len(), MAX_DESCRIPTOR_BYTES + 1);
    assert!(replay(
        &oversized,
        &flat_descriptor_digest(&oversized),
        &["api.first".to_owned()]
    )
    .is_err());
}

#[test]
fn parameter_and_field_presentation_names_are_not_host_identifiers() {
    let mut value = descriptor(vec![export(
        "api.first",
        "record",
        "Packet",
        vec![field(
            "field",
            &"field".repeat(30),
            FieldKind::OwnedBytes,
            0,
        )],
    )]);
    value.exports[0]
        .parameters
        .push(crate::descriptor::Parameter {
            stable_id: "retained:parameter#0".to_owned(),
            source_name: "parameter".repeat(20),
            kind: crate::ParameterKind::I64,
        });
    let bytes = render::canonical(&value);
    replay(
        &bytes,
        &flat_descriptor_digest(&bytes),
        &["api.first".to_owned()],
    )
    .unwrap();
    value.exports[0].parameters[0].stable_id.push('\0');
    let bytes = render::canonical(&value);
    assert!(replay(
        &bytes,
        &flat_descriptor_digest(&bytes),
        &["api.first".to_owned()]
    )
    .is_err());
}

#[test]
fn generated_flat_record_sdk_closes_before_publication() {
    let descriptor = descriptor(vec![export(
        "fixture.value",
        "record.packet",
        "Packet",
        vec![
            field("field.count", "count", FieldKind::I64, 0),
            field("field.bytes", "bytes", FieldKind::OwnedBytes, 1),
            field("field.flag", "flag", FieldKind::Bool, 2),
        ],
    )]);
    let sources =
        crate::flat_render::render_sources(&descriptor, crate::HostTarget::current().unwrap());
    crate::tests::ffi_boundaries::run_boundary_fixture(
        7,
        &sources.lib_rs,
        &sources.ffi_rs,
        "spx_fixture_dot_value",
    );
}

fn field(id: &str, source: &str, kind: FieldKind, ordinal: usize) -> Field {
    Field {
        stable_id: id.to_owned(),
        source_name: source.to_owned(),
        host_name: host_field_name(id),
        kind,
        ordinal,
    }
}

fn export(id: &str, record_id: &str, record_source: &str, fields: Vec<Field>) -> Export {
    Export {
        stable_id: id.to_owned(),
        rust_method_name: rust_method_name(id).unwrap(),
        parameters: Vec::new(),
        record_id: record_id.to_owned(),
        record_source_name: record_source.to_owned(),
        record_host_name: host_record_name(record_id),
        fields,
    }
}

fn descriptor(exports: Vec<Export>) -> Descriptor {
    let digest = format!("sha256:{}", "0".repeat(64));
    Descriptor {
        project_revision: digest.clone(),
        workspace_revision: digest.clone(),
        project_graph_digest: digest,
        exports,
    }
}

#[test]
fn replay_requires_exact_canonical_bytes_even_with_a_reminted_digest() {
    let value = descriptor(vec![export(
        "api.first",
        "record.first",
        "Packet",
        vec![field("field.bytes", "bytes", FieldKind::OwnedBytes, 0)],
    )]);
    let canonical = super::render::canonical(&value);
    replay(
        &canonical,
        &flat_descriptor_digest(&canonical),
        &["api.first".to_owned()],
    )
    .unwrap();
    let mut drifted = canonical;
    drifted.splice(1..1, b" ".iter().copied());
    assert!(replay(
        &drifted,
        &flat_descriptor_digest(&drifted),
        &["api.first".to_owned()],
    )
    .is_err());
}

#[test]
fn replay_rejects_cross_export_record_identity_disagreement() {
    let same_display_name = descriptor(vec![
        export(
            "api.first",
            "record.first",
            "Packet",
            vec![field("field.first", "bytes", FieldKind::OwnedBytes, 0)],
        ),
        export(
            "api.second",
            "record.second",
            "Packet",
            vec![field("field.second", "bytes", FieldKind::OwnedBytes, 0)],
        ),
    ]);
    let bytes = super::render::canonical(&same_display_name);
    replay(
        &bytes,
        &flat_descriptor_digest(&bytes),
        &["api.first".to_owned(), "api.second".to_owned()],
    )
    .unwrap();

    let inconsistent = descriptor(vec![
        export(
            "api.first",
            "record.shared",
            "Packet",
            vec![field("field.first", "bytes", FieldKind::OwnedBytes, 0)],
        ),
        export(
            "api.second",
            "record.shared",
            "Packet",
            vec![
                field("field.flag", "flag", FieldKind::Bool, 0),
                field("field.first", "bytes", FieldKind::OwnedBytes, 1),
            ],
        ),
    ]);
    let bytes = super::render::canonical(&inconsistent);
    assert!(replay(
        &bytes,
        &flat_descriptor_digest(&bytes),
        &["api.first".to_owned(), "api.second".to_owned()],
    )
    .is_err());
}
