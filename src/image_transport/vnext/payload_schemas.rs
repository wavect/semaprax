//! Exact transport-owned payload shapes. Compiler reports without a full shape
//! here remain explicitly unbundled instead of being represented as empty schemas.
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[path = "candidate_schemas.rs"]
mod candidate_schemas;

pub(super) fn digest() -> Value {
    json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"})
}
pub(super) fn text() -> Value {
    json!({"type":"string"})
}
pub(super) fn uint() -> Value {
    json!({"type":"integer","minimum":0,"maximum":u64::MAX})
}
pub(super) fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}
pub(super) fn array(value: Value) -> Value {
    json!({"type":"array","items":value})
}
pub(super) fn object(fields: Vec<(&str, Value)>) -> Value {
    let required = fields.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let properties = fields
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}
pub(super) fn document(id: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut all = vec![("schema", json!({"const":id}))];
    all.extend(fields);
    let mut result = object(all);
    result["$id"] = json!(format!("urn:{id}"));
    result["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    result
}
pub(super) fn documents(capabilities: &Value) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    let mut put = |id: &str, fields| {
        result.insert(format!("urn:{id}"), document(id, fields));
    };
    put(
        "semaprax.image-agent-workspace.v1",
        vec![
            ("state", json!({"const":"open"})),
            ("image_revision", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
        ],
    );
    put(
        "semaprax.image-workspace-refresh-preview.v1",
        vec![
            ("old_image_revision", digest()),
            ("observed_image_revision", digest()),
            ("observed_project_revision", digest()),
            ("workspace_revision", digest()),
            ("manifest_changed", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            ("current_state_replaced", json!({"const":false})),
            ("requires_explicit_refresh", json!({"const":true})),
        ],
    );
    put(
        "semaprax.image-workspace-refresh.v1",
        vec![
            ("old_image_revision", digest()),
            ("image_revision", digest()),
            ("old_project_revision", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("image_arc_reused", json!({"type":"boolean"})),
            ("retained_candidates", array(digest())),
            ("cleared_drafts", uint()),
            ("cleared_attempts", uint()),
            ("manifest_changed", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            ("recovery", json!({"const":"explicit_fresh_snapshot"})),
            ("nonclaims", array(text())),
        ],
    );
    put(
        "semaprax.image-candidate-handle.v1",
        vec![
            ("candidate_revision", digest()),
            ("project_revision", digest()),
            ("base_revision", digest()),
            ("report_bytes", uint()),
            ("source_authority", json!({"const":false})),
            ("tests", json!({"const":"not_run"})),
        ],
    );
    put(
        "semaprax.image-draft-handle.v1",
        vec![
            ("draft_revision", digest()),
            ("source_candidate_revision", digest()),
            ("report_bytes", uint()),
            ("source_authority", json!({"const":false})),
            ("buildable", json!({"const":false})),
        ],
    );
    for (id, handle) in [
        ("semaprax.image-candidate-discard.v1", "candidate_revision"),
        ("semaprax.image-draft-discard.v1", "draft_revision"),
        ("semaprax.image-attempt-discard.v1", "attempt_revision"),
    ] {
        put(
            id,
            vec![
                (handle, digest()),
                ("discarded", json!({"const":true})),
                ("source_unchanged", json!({"const":true})),
            ],
        );
    }
    put(
        "semaprax.project-candidate-attempt-summary.v1",
        vec![
            ("attempt_revision", digest()),
            ("base_candidate_revision", digest()),
            ("base_project_revision", digest()),
            ("state", json!({"const":"rejected"})),
            (
                "diagnostic_count",
                json!({"type":"integer","minimum":1,"maximum":256}),
            ),
            ("report_bytes", uint()),
            ("materializable", json!({"const":false})),
            ("checked_image", json!({"const":false})),
            ("source_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-candidate-attempt-outcome.v1",
        vec![
            ("status", json!({"enum":["accepted","rejected"]})),
            (
                "candidate",
                nullable(json!({"$ref":"urn:semaprax.image-candidate-handle.v1"})),
            ),
            (
                "attempt",
                nullable(json!({"$ref":"urn:semaprax.project-candidate-attempt-summary.v1"})),
            ),
        ],
    );
    put(
        "semaprax.image-candidate-validation.v1",
        vec![
            ("candidate_revision", digest()),
            ("independently_replayed", json!({"const":true})),
            ("source_reparsed", json!({"const":true})),
            ("project_profile_admitted", json!({"const":true})),
            ("tests", json!({"const":"not_run"})),
            ("target_execution", json!({"const":false})),
            ("commit_authority", json!({"const":false})),
        ],
    );
    for (id, handle, label, materializable, target) in [
        (
            "semaprax.image-candidate-report-chunk.v1",
            "candidate_revision",
            "report_schema",
            false,
            false,
        ),
        (
            "semaprax.image-candidate-recovery-chunk.v1",
            "candidate_revision",
            "capsule_schema",
            false,
            false,
        ),
        (
            "semaprax.image-attempt-report-chunk.v1",
            "attempt_revision",
            "report_schema",
            true,
            false,
        ),
        (
            "semaprax.image-semantic-delta-chunk.v1",
            "candidate_revision",
            "report_schema",
            false,
            true,
        ),
    ] {
        let mut fields = vec![
            (handle, digest()),
            (label, text()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ];
        if materializable {
            fields.push(("materializable", json!({"const":false})));
        }
        if target {
            fields.push(("target", nullable(text())));
        }
        put(id, fields);
    }
    put(
        "semaprax.image-draft-recovery-chunk.v1",
        vec![
            ("draft_revision", digest()),
            (
                "capsule_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA}),
            ),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
            ("materializable", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-declaration-dependencies-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":crate::project::IMAGE_DECLARATION_DEPENDENCIES_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("target", text()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-cleanup-dependencies-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":crate::project::IMAGE_CLEANUP_DEPENDENCIES_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("target", text()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-candidate-cleanup-dependencies-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("target", text()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    let dependency_source = object(vec![
        ("path", text()),
        ("module", text()),
        ("source_revision", digest()),
        ("source_digest", digest()),
    ]);
    let dependency_views = json!({"enum":["sites","callers","calls","members"]});
    put(
        "semaprax.image-dependency-summary.v1",
        vec![
            ("image_digest", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("target", text()),
            ("name", nullable(text())),
            ("kind", text()),
            ("source_binding", dependency_source.clone()),
            (
                "facets",
                json!({"type":"array","minItems":4,"maxItems":4,"items":object(vec![
                    ("view",dependency_views.clone()),("handle",digest()),("total_items",uint()),
                ])}),
            ),
            ("declared_test_root", text()),
            ("test_reachable", json!({"type":"boolean"})),
            ("source_authority", json!({"const":false})),
            ("evidence_owner", json!({"const":"retained_checked_hir"})),
            ("nonclaims", array(text())),
        ],
    );
    put(
        "semaprax.image-dependency-page.v1",
        vec![
            ("image_digest", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("target", text()),
            ("source_binding", dependency_source),
            ("view", dependency_views),
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
                json!({"type":"array","maxItems":128,"items":{"$ref":"urn:semaprax.image-dependency-item.v1"}}),
            ),
            ("source_authority", json!({"const":false})),
            ("evidence_owner", json!({"const":"retained_checked_hir"})),
        ],
    );
    put(
        "semaprax.image-artifact-delta-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":"semaprax.project-candidate-artifact-delta.v1"}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("target", json!({"type":"null"})),
            ("kind", json!({"enum":["web","npm","openapi","c"]})),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
            ("artifact_materialization", json!({"const":false})),
            ("target_execution", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-ownership-delta-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":"semaprax.project-candidate-ownership-delta.v1"}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-contract-delta-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":"semaprax.project-candidate-contract-delta.v1"}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-interface-delta-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":"semaprax.project-candidate-interface-delta.v1"}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-symbol-diagnostics-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":"semaprax.project-candidate-symbol-diagnostics.v1"}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("target", text()),
            ("report_revision", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
        ],
    );
    for id in [
        "semaprax.image-protocol-conformance-chunk.v1",
        "semaprax.image-interface-catalog-chunk.v1",
    ] {
        put(
            id,
            vec![
                ("report_schema", text()),
                ("image_revision", digest()),
                ("candidate_revision", nullable(digest())),
                ("target", nullable(text())),
                ("offset", uint()),
                ("total_bytes", uint()),
                ("chunk", text()),
                ("next_offset", nullable(uint())),
                ("source_authority", json!({"const":false})),
            ],
        );
    }
    for id in [
        "semaprax.image-target-admission-chunk.v1",
        "semaprax.image-artifact-projection-chunk.v1",
    ] {
        put(
            id,
            vec![
                ("report_schema", text()),
                ("image_revision", digest()),
                ("candidate_revision", nullable(digest())),
                ("target", nullable(text())),
                (
                    "kind",
                    nullable(json!({"enum":["web","npm","openapi","c"]})),
                ),
                ("offset", uint()),
                ("total_bytes", uint()),
                ("chunk", text()),
                ("next_offset", nullable(uint())),
                ("source_authority", json!({"const":false})),
                ("artifact_materialization", json!({"const":false})),
                ("target_execution", json!({"const":false})),
            ],
        );
    }
    put(
        "semaprax.image-source-commit-handle.v1",
        vec![
            ("state", json!({"const":"published"})),
            ("candidate_revision", digest()),
            ("approval_revision", digest()),
            ("report_revision", digest()),
            ("report_bytes", uint()),
            ("receipt_method", json!({"const":"candidate/commit-report"})),
            ("raw_working_tree_write", json!({"const":false})),
            (
                "source_commit_authority",
                json!({"const":"startup_fixed_host_git_policy"}),
            ),
        ],
    );
    put(
        "semaprax.image-source-commit-status.v1",
        vec![
            ("capability", json!({"const":"source_commit"})),
            (
                "authority",
                json!({"const":"startup_fixed_host_git_policy"}),
            ),
            (
                "state",
                json!({"enum":["available","published","publication_uncertain"]}),
            ),
            (
                "pending_approval",
                nullable(object(vec![
                    ("candidate_revision", digest()),
                    ("approval_revision", digest()),
                ])),
            ),
            ("report_revision", nullable(digest())),
            ("last_error_codes", array(text())),
            ("approval_via_request", json!({"const":false})),
            ("raw_working_tree_write", json!({"const":false})),
            ("host_state_only", json!({"const":true})),
        ],
    );
    put(
        "semaprax.image-source-commit-report-chunk.v1",
        vec![
            ("report_revision", digest()),
            ("report_schema", text()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("historical_publication_receipt", json!({"const":true})),
            ("current_source_admission", json!({"const":false})),
        ],
    );
    let descriptor = object(vec![
        ("method", text()),
        ("query", json!({"type":"boolean"})),
        ("capability", text()),
        ("request_schema", json!({"type":"object"})),
        ("success_response_schema", json!({"type":"object"})),
        ("error_response_schema", json!({"type":"object"})),
    ]);
    put(
        "semaprax.image-agent-query-catalog.v5",
        vec![
            ("protocol", json!({"const":super::VNEXT_PROTOCOL_SCHEMA})),
            ("queries", array(descriptor.clone())),
        ],
    );
    put(
        "semaprax.image-agent-instructions.v5",
        vec![
            ("protocol", json!({"const":super::VNEXT_PROTOCOL_SCHEMA})),
            ("instructions", text()),
        ],
    );
    put(
        "semaprax.image-agent-client.v5",
        vec![
            ("protocol", json!({"const":super::VNEXT_PROTOCOL_SCHEMA})),
            ("language", json!({"enum":["typescript","python","rust"]})),
            ("source", text()),
            ("io", json!({"const":false})),
            ("request_validation", text()),
            ("result_validation", text()),
            ("unbundled_payload_schemas", array(text())),
            ("typescript_integer_policy", text()),
            ("dependencies", array(text())),
        ],
    );
    put(
        "semaprax.image-agent-schemas.v5",
        vec![
            ("protocol", json!({"const":super::VNEXT_PROTOCOL_SCHEMA})),
            ("methods", array(descriptor)),
            ("documents", array(json!({"type":"object"}))),
            ("unbundled_payload_schemas", array(text())),
            ("request_schemas_complete", json!({"const":true})),
            ("payload_completeness", text()),
            (
                "wire_rules",
                object(vec![
                    ("unknown_parameters", text()),
                    ("optional_parameters", text()),
                    ("strings", text()),
                    ("request_ids", text()),
                    ("integer_bounds", text()),
                    ("max_request_bytes", uint()),
                    ("max_response_bytes", uint()),
                ]),
            ),
        ],
    );
    // Capabilities are immutable for one selected host profile, so this exact
    // constant is the strongest truthful schema, including nullable test policy.
    result.insert("urn:semaprax.image-agent-capabilities.v5".into(),json!({"$id":"urn:semaprax.image-agent-capabilities.v5","$schema":"https://json-schema.org/draft/2020-12/schema","const":capabilities}));
    result.extend(candidate_schemas::documents());
    for id in [
        "urn:semaprax.image-workspace-refresh.v1",
        "urn:semaprax.image-workspace-refresh-preview.v1",
    ] {
        // Startup opt-in only; omitted in the unchanged cold response shape.
        result.get_mut(id).expect("refresh schema")["properties"]["frontend_work"] = json!({"oneOf":[
            {"$ref":"urn:semaprax.project-frontend-cache-work.v1"},
            {"$ref":"urn:semaprax.project-semantic-cache-work.v1"}
        ]});
    }
    result
}
