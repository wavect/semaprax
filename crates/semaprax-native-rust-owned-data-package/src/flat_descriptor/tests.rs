use super::*;

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
    let host_collision = descriptor(vec![
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
    let bytes = super::render::canonical(&host_collision);
    assert!(replay(
        &bytes,
        &flat_descriptor_digest(&bytes),
        &["api.first".to_owned(), "api.second".to_owned()],
    )
    .is_err());

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
