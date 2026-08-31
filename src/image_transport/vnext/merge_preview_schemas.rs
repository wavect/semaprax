//! Closed descriptive preview shapes; result digests grant no retained handle.
use super::{digest, document, nullable, object};
use serde_json::{json, Value};

pub(super) fn schema() -> Value {
    let accepted = object(vec![
        ("status", json!({"const":"accepted"})),
        ("result_project_revision", digest()),
        ("result_candidate_revision", digest()),
        (
            "shared_history_prefix",
            json!({"type":"integer","minimum":0,"maximum":32}),
        ),
        (
            "source_file_count",
            json!({"type":"integer","minimum":0,"maximum":16}),
        ),
        (
            "source_bytes",
            json!({"type":"integer","minimum":0,"maximum":16777216}),
        ),
    ]);
    let rejected = object(vec![
        ("status", json!({"const":"rejected"})),
        (
            "diagnostics",
            json!({"type":"array","minItems":1,"maxItems":64,"items":object(vec![
                ("code",json!({"type":"string","maxLength":16384,"x-max-utf8-bytes":16384})),
                ("message",json!({"type":"string","maxLength":16384,"x-max-utf8-bytes":16384})),
            ])}),
        ),
        (
            "interpretation",
            json!({"const":"merge_rejected_not_proof_of_incompatibility"}),
        ),
    ]);
    let direction = json!({"oneOf":[accepted,rejected]});
    document(
        "semaprax.project-candidate-merge-preview.v1",
        vec![
            ("base_revision", digest()),
            ("left_candidate_revision", digest()),
            ("right_candidate_revision", digest()),
            ("left_then_right", direction.clone()),
            ("right_then_left", direction),
            ("same_source", nullable(json!({"type":"boolean"}))),
            ("tests", json!({"const":"not_run"})),
            ("source_authority", json!({"const":false})),
            ("candidate_retained", json!({"const":false})),
            (
                "validation",
                json!({"const":"ordinary_merge_with_full_candidate_admission"}),
            ),
            (
                "nonclaims",
                json!({"const":[
                    "not_behavioral_equivalence",
                    "not_runtime_or_test_execution",
                    "not_external_consumer_compatibility",
                    "not_permission_to_publish_or_retain_candidates",
                    "directional_rejection_may_be_a_conservative_or_capacity_limit",
                ]}),
            ),
        ],
    )
}
