use super::*;

fn descriptor() -> Descriptor {
    let inner = Record {
        stable_id: "inner".to_owned(),
        source_name: "Inner".to_owned(),
        host_name: host_record_name("inner"),
        fields: vec![Field {
            stable_id: "data".to_owned(),
            source_name: "data".to_owned(),
            host_name: host_field_name("data"),
            ordinal: 0,
            ty: FieldType::OwnedBytes,
        }],
    };
    let outer = Record {
        stable_id: "outer".to_owned(),
        source_name: "Outer".to_owned(),
        host_name: host_record_name("outer"),
        fields: ["left", "right"]
            .into_iter()
            .enumerate()
            .map(|(ordinal, id)| Field {
                stable_id: id.to_owned(),
                source_name: id.to_owned(),
                host_name: host_field_name(id),
                ordinal,
                ty: FieldType::Record("inner".to_owned()),
            })
            .collect(),
    };
    Descriptor {
        project_revision: format!("sha256:{}", "0".repeat(64)),
        workspace_revision: format!("sha256:{}", "1".repeat(64)),
        project_graph_digest: format!("sha256:{}", "2".repeat(64)),
        exports: vec![Export {
            stable_id: "api.run".to_owned(),
            rust_method_name: "spx_api_dot_run".to_owned(),
            parameters: Vec::new(),
            result_record_id: "outer".to_owned(),
            leaves: vec![
                Leaf {
                    path: vec!["left".to_owned(), "data".to_owned()],
                    ordinal: 0,
                    ty: FieldType::OwnedBytes,
                },
                Leaf {
                    path: vec!["right".to_owned(), "data".to_owned()],
                    ordinal: 1,
                    ty: FieldType::OwnedBytes,
                },
            ],
        }],
        records: vec![inner, outer],
    }
}

#[test]
fn shared_nominal_children_replay_as_distinct_owner_paths() {
    let expected = descriptor();
    let bytes = render::canonical(&expected);
    let digest = nested_descriptor_digest(&bytes);
    let replayed = replay(&bytes, &digest, &["api.run".to_owned()]).unwrap();
    assert_eq!(replayed, expected);
    assert_ne!(
        replayed.exports[0].leaves[0].path,
        replayed.exports[0].leaves[1].path
    );
}

#[test]
fn redigested_duplicate_owner_occurrence_is_rejected() {
    let bytes = render::canonical(&descriptor());
    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    value["exports"][0]["leaves"][1]["path"] = value["exports"][0]["leaves"][0]["path"].clone();
    let mut changed = serde_json::to_vec(&value).unwrap();
    changed.push(b'\n');
    let digest = nested_descriptor_digest(&changed);
    assert!(replay(&changed, &digest, &["api.run".to_owned()]).is_err());
}

fn redigest(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn reachable_empty_nested_record_replays_without_inventing_a_leaf() {
    let mut expected = descriptor();
    let empty = Record {
        stable_id: "empty".to_owned(),
        source_name: "Empty".to_owned(),
        host_name: host_record_name("empty"),
        fields: Vec::new(),
    };
    let outer = expected
        .records
        .iter_mut()
        .find(|record| record.stable_id == "outer")
        .unwrap();
    for field in &mut outer.fields {
        field.ordinal += 1;
    }
    outer.fields.insert(
        0,
        Field {
            stable_id: "empty-field".to_owned(),
            source_name: "empty".to_owned(),
            host_name: host_field_name("empty-field"),
            ordinal: 0,
            ty: FieldType::Record("empty".to_owned()),
        },
    );
    expected.records.insert(0, empty);

    let bytes = render::canonical(&expected);
    let replayed = replay(
        &bytes,
        &nested_descriptor_digest(&bytes),
        &["api.run".to_owned()],
    )
    .unwrap();
    assert_eq!(replayed, expected);
    assert_eq!(replayed.exports[0].leaves.len(), 2);
}

#[test]
fn redigested_record_table_path_and_type_forgeries_all_reject() {
    let canonical = render::canonical(&descriptor());
    let original: Value = serde_json::from_slice(&canonical).unwrap();
    let mut cases = Vec::new();

    let mut duplicate_record = original.clone();
    duplicate_record["records"][1]["stable_id"] = Value::String("inner".to_owned());
    cases.push(duplicate_record);

    let mut foreign_child = original.clone();
    foreign_child["records"][1]["fields"][0]["record_id"] = Value::String("foreign".to_owned());
    cases.push(foreign_child);

    let mut wrong_leaf_type = original.clone();
    wrong_leaf_type["exports"][0]["leaves"][0]["type"] = Value::String("i64".to_owned());
    cases.push(wrong_leaf_type);

    let mut truncated_path = original.clone();
    truncated_path["exports"][0]["leaves"][0]["path"] = serde_json::json!(["left"]);
    cases.push(truncated_path);

    let mut reordered_records = original;
    reordered_records["records"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    cases.push(reordered_records);

    for value in cases {
        let bytes = redigest(value);
        assert!(replay(
            &bytes,
            &nested_descriptor_digest(&bytes),
            &["api.run".to_owned()]
        )
        .is_err());
    }
}

#[test]
fn generated_multi_owner_runtime_includes_the_frozen_preflight_and_settlement_sequence() {
    let mut runtime = String::new();
    crate::owned_ffi_runtime::append_multi_owner_operations(&mut runtime);
    let preflight = runtime
        .find("total=total.checked_add(length).filter(|total|*total<=65536)")
        .unwrap();
    let allocate = runtime
        .find("let mut values=Vec::with_capacity(handles.len())")
        .unwrap();
    let copy = runtime
        .find("for(index,handle)in handles.iter().enumerate(){let bytes=&mut values[index]")
        .unwrap();
    let settle = runtime
        .find("for handle in handles{if unsafe{spx_owned_bytes_drop_v1")
        .unwrap();
    let publish = runtime.find("guard.armed=false;Ok(values)").unwrap();
    assert!(preflight < allocate && allocate < copy && copy < settle && settle < publish);
    assert!(!runtime.contains("<65536).ok_or"));
    assert!(!runtime.contains("<=65537"));
}
