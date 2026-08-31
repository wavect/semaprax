//! Closed wrappers for candidate impact navigation; heterogeneous compiler
//! node and edge interiors remain explicitly unbundled.
use super::payload_schemas::{digest, document, nullable, object, text, uint};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const ITEM: &str = "urn:semaprax.project-candidate-impact-item.v1";
const NONCLAIMS: [&str; 7] = [
    "not_a_candidate_semantic_delta_or_behavioral_change",
    "potential_reverse_dependencies_over_the_existing_six_edge_families_only",
    "not_runtime_liveness_test_coverage_or_external_consumer_compatibility",
    "no_repair_ranking_or_intent_correctness",
    "no_persistent_image_index_or_candidate_retention",
    "bounded_or_truncated_inventory_is_not_complete_impact",
    "no_source_execution_or_publication_authority",
];

pub(super) fn documents() -> BTreeMap<String, Value> {
    let target = object(vec![
        ("kind", json!({"const":"declaration"})),
        ("id", text()),
        ("declaration_kind", nullable(text())),
        ("identity_origin", nullable(text())),
        ("path", nullable(text())),
        ("module", nullable(text())),
    ]);
    let query = object(vec![
        ("direction", json!({"const":"reverse"})),
        (
            "depth",
            json!({"type":"integer","minimum":0,"maximum":1024}),
        ),
        (
            "max_bytes",
            json!({"type":"integer","minimum":4096,"maximum":16777216}),
        ),
        (
            "max_nodes",
            json!({"type":"integer","minimum":1,"maximum":8208}),
        ),
    ]);
    let truncation = object(vec![
        ("truncated", json!({"type":"boolean"})),
        (
            "reasons",
            json!({"type":"array","maxItems":3,"uniqueItems":true,
                "items":{"enum":["max_depth","max_nodes","max_bytes"]}}),
        ),
        ("omitted_known_nodes", uint()),
        ("deferred_known_nodes", uint()),
    ]);
    let budget = object(vec![
        ("used_nodes", uint()),
        ("used_edges", uint()),
        ("used_depth", uint()),
        ("used_builder_bytes", uint()),
        ("used_output_bytes", uint()),
    ]);
    let common = || {
        vec![
            ("candidate_revision", digest()),
            ("base_project_revision", digest()),
            ("project_schema", text()),
            ("project", text()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("project_graph_digest", digest()),
            ("target", target.clone()),
            ("artifact_digest", digest()),
            ("query", query.clone()),
            ("truncation", truncation.clone()),
            ("budget", budget.clone()),
        ]
    };
    let mut summary = common();
    summary.extend([
        (
            "facets",
            json!({"type":"array","minItems":3,"maxItems":3,"items":object(vec![
                ("view",json!({"enum":["affected","dependency_edges","frontier"]})),
                ("handle",digest()),("total_items",uint()),
            ])}),
        ),
        ("source_authority", json!({"const":false})),
        ("execution", json!({"const":false})),
        ("publication_authority", json!({"const":false})),
        ("candidate_retained", json!({"const":false})),
        ("nonclaims", json!({"const":NONCLAIMS})),
    ]);
    let mut page = common();
    page.extend([
        (
            "view",
            json!({"enum":["affected","dependency_edges","frontier"]}),
        ),
        ("handle", digest()),
        ("cursor", nullable(text())),
        ("offset", uint()),
        ("total_items", uint()),
        (
            "page_size",
            json!({"type":"integer","minimum":1,"maximum":128}),
        ),
        (
            "max_bytes",
            json!({"type":"integer","minimum":1024,"maximum":1048576}),
        ),
        ("next_cursor", nullable(text())),
        (
            "items",
            json!({"type":"array","maxItems":128,"items":{"$ref":ITEM}}),
        ),
        ("source_authority", json!({"const":false})),
        ("execution", json!({"const":false})),
        ("publication_authority", json!({"const":false})),
        ("candidate_retained", json!({"const":false})),
        ("nonclaims", json!({"const":NONCLAIMS})),
    ]);
    [
        ("semaprax.project-candidate-impact-summary.v1", summary),
        ("semaprax.project-candidate-impact-page.v1", page),
    ]
    .into_iter()
    .map(|(id, fields)| (format!("urn:{id}"), document(id, fields)))
    .collect()
}
