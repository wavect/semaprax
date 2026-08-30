use super::*;
use crate::PackageErrorKind;

fn descriptor(utf8: bool, parameter_count: usize) -> Descriptor {
    let (schema, project_schema, result) = if utf8 {
        (
            PUBLIC_OWNED_UTF8_API_SCHEMA,
            PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
            ResultKind::OwnedUtf8,
        )
    } else {
        (
            PUBLIC_OWNED_DATA_API_SCHEMA,
            PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            ResultKind::OwnedBytes,
        )
    };
    Descriptor {
        schema,
        project_schema,
        project_revision: format!("sha256:{}", "1".repeat(64)),
        workspace_revision: format!("sha256:{}", "2".repeat(64)),
        project_graph_digest: format!("sha256:{}", "3".repeat(64)),
        exports: vec![Export {
            stable_id: "entry.a".to_owned(),
            rust_method_name: "spx_entry_dot_a".to_owned(),
            parameters: (0..parameter_count)
                .map(|ordinal| Parameter {
                    stable_id: format!("entry.a#value:param:{ordinal}"),
                    source_name: format!("arg_{ordinal}"),
                    kind: [
                        ParameterKind::I64,
                        ParameterKind::Bool,
                        ParameterKind::BorrowStr,
                        ParameterKind::BorrowSliceU8,
                    ][ordinal % 4],
                })
                .collect(),
            result,
        }],
    }
}

fn replay_fresh(value: &Descriptor) -> Result<Descriptor, PackageError> {
    let bytes = render(value);
    let digest = descriptor_digest_for_schema(value.schema, &bytes).unwrap();
    let selected = value
        .exports
        .iter()
        .map(|export| export.stable_id.clone())
        .collect::<Vec<_>>();
    replay(&bytes, &digest, &selected)
}

#[test]
fn distinct_parameter_identities_replay_at_admitted_counts_in_both_schemas() {
    for utf8 in [false, true] {
        for count in [0, 1, 2, MAX_PARAMETERS] {
            let value = descriptor(utf8, count);
            let replayed = replay_fresh(&value).unwrap();
            assert_eq!(replayed, value);
            assert_eq!(render(&replayed), render(&value));
        }
    }
}

#[test]
fn freshly_rehashed_duplicate_parameter_identities_reject_in_both_schemas() {
    for utf8 in [false, true] {
        for (count, first, duplicate) in [(2, 0, 1), (8, 0, 7), (8, 6, 7)] {
            let mut value = descriptor(utf8, count);
            let original_bytes = render(&value);
            let original_digest =
                descriptor_digest_for_schema(value.schema, &original_bytes).unwrap();
            replay_fresh(&value).unwrap();
            let parameters = &mut value.exports[0].parameters;
            parameters[duplicate].stable_id = parameters[first].stable_id.clone();
            assert_ne!(
                parameters[first].source_name,
                parameters[duplicate].source_name
            );

            // The renderer retains distinct canonical ordinals and every other
            // field. A fresh digest authenticates the mutation, not its validity.
            let bytes = render(&value);
            let digest = descriptor_digest_for_schema(value.schema, &bytes).unwrap();
            assert_ne!(digest, original_digest);
            let parsed: Value = serde_json::from_slice(&bytes).unwrap();
            let rows = parsed["exports"][0]["parameters"].as_array().unwrap();
            assert_eq!(rows[first]["ordinal"].as_u64(), Some(first as u64));
            assert_eq!(rows[duplicate]["ordinal"].as_u64(), Some(duplicate as u64));
            assert_eq!(rows[first]["stable_id"], rows[duplicate]["stable_id"]);
            assert_eq!(
                replay(&bytes, &digest, &["entry.a".to_owned()])
                    .unwrap_err()
                    .kind(),
                PackageErrorKind::Descriptor
            );
        }
    }
}

#[test]
fn parameter_identity_uniqueness_is_scoped_to_each_export() {
    // This tests structural descriptor replay, not whether a source HIR could
    // derive these cross-export parameter identities or authenticate a provider.
    for utf8 in [false, true] {
        let mut value = descriptor(utf8, MAX_PARAMETERS);
        let mut second = value.exports[0].clone();
        second.stable_id = "entry.b".to_owned();
        second.rust_method_name = "spx_entry_dot_b".to_owned();
        value.exports.push(second);
        assert_eq!(replay_fresh(&value).unwrap(), value);
    }
}
