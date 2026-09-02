//! Closed schemas for exact-revision function references and their resolution.
use super::{array, digest, document, nullable, object, text, uint};
use crate::project::{
    IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA, IMAGE_FUNCTION_REFERENCE_SCHEMA,
    IMAGE_FUNCTION_SUMMARY_SCHEMA,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn facet() -> Value {
    json!({"enum":[
        "signature","contracts","callers","ownership","loans","cleanup",
        "relationships","data-access","unsafe-boundaries"
    ]})
}

fn span() -> Value {
    object(vec![
        ("start", uint()),
        ("end", uint()),
        ("line", uint()),
        ("column", uint()),
    ])
}

fn function_summary() -> Value {
    object(vec![
        ("schema", json!({"const":IMAGE_FUNCTION_SUMMARY_SCHEMA})),
        ("image_revision", digest()),
        ("project_revision", digest()),
        ("id", text()),
        ("name", text()),
        ("path", text()),
        ("module", text()),
        ("source_revision", digest()),
        ("span", span()),
        ("parameter_count", uint()),
        ("return_type_id", text()),
        ("effects", array(text())),
        ("requires_count", uint()),
        ("ensures_count", uint()),
        (
            "facets",
            json!({"type":"array","minItems":9,"maxItems":9,"items":object(vec![
                ("facet",facet()),("handle",digest()),
            ])}),
        ),
        (
            "evidence_class",
            json!({"const":"descriptive_projection_of_validated_hir"}),
        ),
        ("source_authority", json!({"const":false})),
        ("target_execution", json!({"const":false})),
    ])
}

pub(super) fn documents() -> BTreeMap<String, Value> {
    let source = object(vec![
        ("path", text()),
        ("module", text()),
        ("source_revision", digest()),
        ("source_digest", digest()),
    ]);
    BTreeMap::from([
        (
            format!("urn:{IMAGE_FUNCTION_REFERENCE_SCHEMA}"),
            document(
                IMAGE_FUNCTION_REFERENCE_SCHEMA,
                vec![
                    ("reference_revision", digest()),
                    ("image_revision", digest()),
                    ("project_revision", digest()),
                    ("workspace_revision", digest()),
                    ("project_graph_digest", digest()),
                    ("target_kind", json!({"const":"function"})),
                    (
                        "target",
                        json!({"type":"string","minLength":1,"maxLength":4096,"x-max-utf8-bytes":4096}),
                    ),
                    ("facet", nullable(facet())),
                    ("source", source),
                    ("source_authority", json!({"const":false})),
                    ("execution", json!({"const":false})),
                    ("publication_authority", json!({"const":false})),
                    (
                        "nonclaims",
                        json!({"const":[
                            "integrity_and_staleness_binding_not_capability_or_secret",
                            "exact_revision_only_no_automatic_migration",
                            "no_hir_graph_source_or_handle_facts_trusted_from_reference",
                            "no_source_execution_candidate_retention_or_publication_authority",
                            "no_persistent_server_state_or_general_session_recovery"
                        ]}),
                    ),
                ],
            ),
        ),
        (
            format!("urn:{IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA}"),
            document(
                IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA,
                vec![
                    ("reference_revision", digest()),
                    ("image_revision", digest()),
                    ("project_revision", digest()),
                    ("workspace_revision", digest()),
                    ("project_graph_digest", digest()),
                    (
                        "target",
                        json!({"type":"string","minLength":1,"maxLength":4096,"x-max-utf8-bytes":4096}),
                    ),
                    ("facet", nullable(facet())),
                    ("function_summary", function_summary()),
                    ("facet_handle", nullable(digest())),
                    ("source_authority", json!({"const":false})),
                    ("execution", json!({"const":false})),
                    ("publication_authority", json!({"const":false})),
                    (
                        "nonclaims",
                        json!({"const":[
                            "resolved_only_against_exact_current_image_and_source_provenance",
                            "function_summary_and_facet_handle_freshly_derived_not_trusted_from_reference",
                            "no_cursor_persistence_or_automatic_migration",
                            "no_source_execution_candidate_retention_or_publication_authority",
                            "no_ranking_or_general_session_recovery"
                        ]}),
                    ),
                ],
            ),
        ),
    ])
}
