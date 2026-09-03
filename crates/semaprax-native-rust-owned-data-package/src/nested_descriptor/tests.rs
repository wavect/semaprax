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
