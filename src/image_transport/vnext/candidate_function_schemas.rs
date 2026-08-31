//! Closed candidate function-navigation envelopes; compiler facet values stay opaque.
use super::{array, digest, document, nullable, object, text, uint};
use crate::project::{
    PROJECT_CANDIDATE_FUNCTION_FACET_ITEM_SCHEMA, PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA,
    PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const NONCLAIMS: [&str; 5] = [
    "not_a_candidate_semantic_delta_or_behavioral_change",
    "only_functions_present_in_the_final_candidate_hir_are_selectable",
    "no_runtime_liveness_test_coverage_or_external_dynamic_callers",
    "no_persistent_derived_image_candidate_or_cursor_retention",
    "no_source_execution_or_publication_authority",
];

fn facet() -> Value {
    json!({"enum":[
        "signature","contracts","callers","ownership","loans","cleanup",
        "relationships","data-access","unsafe-boundaries"
    ]})
}

fn cursor() -> Value {
    json!({"type":"string","maxLength":128,"x-max-utf8-bytes":128})
}

pub(super) fn documents() -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    let handle = object(vec![("facet", facet()), ("handle", digest())]);
    result.insert(
        format!("urn:{PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA}"),
        document(
            PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA,
            vec![
                ("image_revision", digest()),
                ("project_revision", digest()),
                ("id", text()),
                ("name", text()),
                ("path", text()),
                ("module", text()),
                ("source_revision", digest()),
                (
                    "span",
                    object(vec![
                        ("start", uint()),
                        ("end", uint()),
                        ("line", uint()),
                        ("column", uint()),
                    ]),
                ),
                ("parameter_count", uint()),
                ("return_type_id", text()),
                ("effects", array(text())),
                ("requires_count", uint()),
                ("ensures_count", uint()),
                (
                    "facets",
                    json!({"type":"array","minItems":9,"maxItems":9,"items":handle}),
                ),
                (
                    "evidence_class",
                    json!({"const":"descriptive_projection_of_validated_hir"}),
                ),
                ("source_authority", json!({"const":false})),
                ("target_execution", json!({"const":false})),
                ("candidate_revision", digest()),
                ("base_project_revision", digest()),
                ("workspace_revision", digest()),
                ("project_graph_digest", digest()),
                ("candidate_retained", json!({"const":false})),
                ("execution", json!({"const":false})),
                ("publication_authority", json!({"const":false})),
                ("nonclaims", json!({"const":NONCLAIMS})),
            ],
        ),
    );
    result.insert(
        format!("urn:{PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA}"),
        document(
            PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA,
            vec![
                ("image_revision", digest()),
                ("project_revision", digest()),
                ("target", text()),
                ("facet", facet()),
                ("handle", digest()),
                ("path", text()),
                ("source_revision", digest()),
                ("offset", uint()),
                ("total_items", uint()),
                (
                    "items",
                    json!({"type":"array","maxItems":128,"items":{"$ref":format!("urn:{PROJECT_CANDIDATE_FUNCTION_FACET_ITEM_SCHEMA}")}}),
                ),
                ("next_cursor", nullable(cursor())),
                (
                    "evidence_class",
                    json!({"const":"descriptive_projection_of_validated_hir"}),
                ),
                ("candidate_revision", digest()),
                ("base_project_revision", digest()),
                ("workspace_revision", digest()),
                ("project_graph_digest", digest()),
                ("cursor", nullable(cursor())),
                (
                    "page_size",
                    json!({"type":"integer","minimum":1,"maximum":128}),
                ),
                (
                    "max_bytes",
                    json!({"type":"integer","minimum":1024,"maximum":1048576}),
                ),
                ("source_authority", json!({"const":false})),
                ("target_execution", json!({"const":false})),
                ("candidate_retained", json!({"const":false})),
                ("execution", json!({"const":false})),
                ("publication_authority", json!({"const":false})),
                ("nonclaims", json!({"const":NONCLAIMS})),
            ],
        ),
    );
    result
}
