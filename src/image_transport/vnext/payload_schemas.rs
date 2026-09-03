//! Exact transport-owned payload shapes. Compiler reports without a full shape
//! here remain explicitly unbundled instead of being represented as empty schemas.
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[path = "candidate_function_schemas.rs"]
mod candidate_function_schemas;
#[path = "candidate_schemas.rs"]
mod candidate_schemas;
#[path = "function_instance_schemas.rs"]
mod function_instance_schemas;
#[path = "function_reference_schemas.rs"]
mod function_reference_schemas;
#[path = "hole_navigation_schemas.rs"]
mod hole_navigation_schemas;
#[path = "merge_preview_schemas.rs"]
mod merge_preview_schemas;
#[path = "package_schemas.rs"]
mod package_schemas;

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
fn blind_spot_ledger() -> Value {
    json!({
        "type":"array",
        "minItems":3,
        "maxItems":3,
        "items":blind_spot(),
    })
}
fn blind_spot() -> Value {
    object(vec![
        (
            "domain",
            json!({"enum":[
                "deployment_configuration",
                "generated_file_provenance",
                "external_api_and_deployed_runtime_contracts",
            ]}),
        ),
        ("evidence_status", json!({"const":"absent"})),
        ("absent_evidence", text()),
        (
            "source_binding",
            object(vec![
                (
                    "kind",
                    json!({"const":"exact_retained_project_revision_and_manifest_source_inventory"}),
                ),
                ("project_revision", digest()),
            ]),
        ),
        ("nonclaim", text()),
    ])
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
        super::VNEXT_APPLICATION_ERROR_DATA_SCHEMA,
        vec![(
            "diagnostics",
            json!({"type":"array","minItems":1,"items":object(vec![
                ("code", text()),
                ("severity", json!({"enum":["error","warning"]})),
                ("message", text()),
                ("path", nullable(text())),
                (
                    "location",
                    nullable(object(vec![
                        ("line", uint()),
                        ("column", uint()),
                        ("start", uint()),
                        ("end", uint()),
                    ])),
                ),
                ("help", nullable(text())),
            ])}),
        )],
    );
    put(
        "semaprax.image-analysis-coverage.v1",
        vec![
            ("image_revision", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("project_graph_digest", digest()),
            (
                "manifest",
                object(vec![
                    ("schema", text()),
                    ("profile", nullable(text())),
                    ("entry", text()),
                    ("test_module", text()),
                    (
                        "source_paths",
                        json!({"type":"array","maxItems":16,"items":text()}),
                    ),
                    ("web_exports", array(text())),
                    ("capabilities", array(text())),
                ]),
            ),
            (
                "sources",
                json!({"type":"array","maxItems":16,"items":object(vec![
                    ("path",text()),("module",text()),("source_revision",digest()),
                    ("source_digest",digest()),("source_graph_schema",text()),
                ])}),
            ),
            (
                "inventory",
                object(
                    [
                        "source_modules",
                        "functions",
                        "function_templates",
                        "function_instances",
                        "nominal_types",
                        "interfaces",
                        "interface_imports",
                    ]
                    .into_iter()
                    .map(|name| (name, json!({"type":"integer","minimum":0,"maximum":65536})))
                    .collect(),
                ),
            ),
            (
                "external_contracts",
                json!({"type":"array","maxItems":65536,"items":object(vec![
                    ("path",text()),("module",text()),("interface_id",text()),("import_id",text()),
                    ("name",text()),("import_key",text()),("native_rust",json!({"type":"boolean"})),
                    ("effects",array(text())),("required_authority",array(text())),
                ])}),
            ),
            (
                "areas",
                json!({"type":"array","minItems":8,"maxItems":8,"items":object(vec![
                    ("area",json!({"enum":["declared_source_inputs","declared_external_contracts",
                        "deployment_configuration","generated_file_provenance","generated_artifacts",
                        "external_api_behavior","runtime_environment","external_consumers"]})),
                    ("status",json!({"enum":["known","partial","not_inspected"]})),
                    ("basis",text()),("limitations",array(text())),("required_evidence",array(text())),
                ])}),
            ),
            ("blind_spots", blind_spot_ledger()),
            ("source_authority", json!({"const":false})),
            ("external_io", json!({"const":false})),
            ("execution", json!({"const":false})),
            (
                "evidence_class",
                json!({"const":"retained_source_analysis_boundary_inventory"}),
            ),
            ("nonclaims", array(text())),
        ],
    );
    put(
        "semaprax.project-candidate-analysis-coverage.v1",
        vec![
            ("image_revision", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("candidate_revision", digest()),
            ("base_project_revision", digest()),
            ("project_graph_digest", digest()),
            (
                "manifest",
                object(vec![
                    ("schema", text()),
                    ("profile", nullable(text())),
                    ("entry", text()),
                    ("test_module", text()),
                    (
                        "source_paths",
                        json!({"type":"array","maxItems":16,"items":text()}),
                    ),
                    ("web_exports", array(text())),
                    ("capabilities", array(text())),
                ]),
            ),
            (
                "sources",
                json!({"type":"array","maxItems":16,"items":object(vec![
                    ("path",text()),("module",text()),("source_revision",digest()),
                    ("source_digest",digest()),("source_graph_schema",text()),
                ])}),
            ),
            (
                "inventory",
                object(
                    [
                        "source_modules",
                        "functions",
                        "function_templates",
                        "function_instances",
                        "nominal_types",
                        "interfaces",
                        "interface_imports",
                    ]
                    .into_iter()
                    .map(|name| (name, json!({"type":"integer","minimum":0,"maximum":65536})))
                    .collect(),
                ),
            ),
            (
                "external_contracts",
                json!({"type":"array","maxItems":65536,"items":object(vec![
                    ("path",text()),("module",text()),("interface_id",text()),("import_id",text()),
                    ("name",text()),("import_key",text()),("native_rust",json!({"type":"boolean"})),
                    ("effects",array(text())),("required_authority",array(text())),
                ])}),
            ),
            (
                "areas",
                json!({"type":"array","minItems":8,"maxItems":8,"items":object(vec![
                    ("area",json!({"enum":["declared_source_inputs","declared_external_contracts",
                        "deployment_configuration","generated_file_provenance","generated_artifacts",
                        "external_api_behavior","runtime_environment","external_consumers"]})),
                    ("status",json!({"enum":["known","partial","not_inspected"]})),
                    ("basis",text()),("limitations",array(text())),("required_evidence",array(text())),
                ])}),
            ),
            ("blind_spots", blind_spot_ledger()),
            ("source_authority", json!({"const":false})),
            ("external_io", json!({"const":false})),
            ("execution", json!({"const":false})),
            ("candidate_retained", json!({"const":false})),
            ("publication_authority", json!({"const":false})),
            (
                "evidence_class",
                json!({"const":"retained_source_analysis_boundary_inventory"}),
            ),
            ("nonclaims", array(text())),
        ],
    );
    put(
        "semaprax.image-candidate-deployment-contract-evidence-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("declaration_digest", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("report_sha256", digest()),
            ("source_authority", json!({"const":false})),
            ("external_io", json!({"const":false})),
            ("environment_observation", json!({"const":false})),
            ("deployment_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-candidate-external-api-contract-evidence-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("declaration_digest", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("report_sha256", digest()),
            ("source_authority", json!({"const":false})),
            ("external_io", json!({"const":false})),
            ("network_observation", json!({"const":false})),
            ("provider_observation", json!({"const":false})),
            ("runtime_observation", json!({"const":false})),
            ("conformance_evidence", json!({"const":false})),
            ("ambient_authority", json!({"const":false})),
            ("deployment_authority", json!({"const":false})),
        ],
    );
    put(
        "semaprax.image-candidate-generated-file-provenance-evidence-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            ("declaration_digest", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("report_sha256", digest()),
            ("source_authority", json!({"const":false})),
            ("filesystem_scan", json!({"const":false})),
            ("generator_execution", json!({"const":false})),
            ("artifact_materialization", json!({"const":false})),
            ("runtime_observation", json!({"const":false})),
            ("deployment_authority", json!({"const":false})),
        ],
    );
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
    let review_text = json!({
        "type":"string",
        "maxLength":crate::project::MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES,
        "x-max-utf8-bytes":crate::project::MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES,
    });
    let mut review_files = array(object(vec![
        (
            "path",
            json!({"type":"string","minLength":1,"maxLength":240,"x-max-utf8-bytes":240}),
        ),
        ("base_source", review_text.clone()),
        ("candidate_source", review_text.clone()),
        ("base_digest", digest()),
        ("candidate_digest", digest()),
        ("source_diff", review_text),
        ("source_diff_digest", digest()),
    ]));
    review_files["maxItems"] = json!(16);
    put(
        crate::project::PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
        vec![
            ("base_project_revision", digest()),
            ("candidate_project_revision", digest()),
            ("candidate_revision", digest()),
            ("source_authority", json!({"const":false})),
            ("files", review_files),
            ("report_revision", digest()),
        ],
    );
    let review_offset = json!({
        "type":"integer","minimum":0,
        "maximum":crate::project::MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES,
    });
    put(
        "semaprax.image-source-review-chunk.v1",
        vec![
            ("image_revision", digest()),
            ("candidate_revision", digest()),
            (
                "report_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA}),
            ),
            ("offset", review_offset.clone()),
            ("total_bytes", review_offset.clone()),
            (
                "chunk",
                json!({"type":"string","maxLength":65536,"x-max-utf8-bytes":65536}),
            ),
            ("next_offset", nullable(review_offset)),
            ("source_authority", json!({"const":false})),
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
    put(
        "semaprax.image-draft-rebase.v1",
        vec![
            ("selected_candidate_revision", digest()),
            (
                "draft",
                json!({"$ref":"urn:semaprax.image-draft-handle.v1"}),
            ),
            (
                "report",
                json!({"$ref":"urn:semaprax.project-candidate-draft-rebase.v2"}),
            ),
        ],
    );
    put(
        "semaprax.image-draft-merge.v1",
        vec![
            ("left_draft_revision", digest()),
            ("right_draft_revision", digest()),
            (
                "draft",
                json!({"$ref":"urn:semaprax.image-draft-handle.v1"}),
            ),
            (
                "report",
                json!({"$ref":"urn:semaprax.project-candidate-draft-merge.v2"}),
            ),
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
                json!({"enum":[crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,crate::project::PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA]}),
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
        "semaprax.image-draft-archive-chunk.v1",
        vec![
            (
                "archive_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA}),
            ),
            ("image_revision", digest()),
            ("archive_revision", digest()),
            ("draft_revision", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            ("chunk", text()),
            ("next_offset", nullable(uint())),
            ("source_authority", json!({"const":false})),
            ("approval_authority", json!({"const":false})),
            ("trusted_hir", json!({"const":false})),
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
            ("source_binding", dependency_source.clone()),
            ("view", dependency_views.clone()),
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
        "semaprax.project-candidate-dependency-summary.v1",
        vec![
            ("image_digest", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("candidate_revision", digest()),
            ("base_project_revision", digest()),
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
            ("candidate_retained", json!({"const":false})),
            ("execution", json!({"const":false})),
            ("publication_authority", json!({"const":false})),
            ("evidence_owner", json!({"const":"retained_checked_hir"})),
            ("nonclaims", array(text())),
        ],
    );
    put(
        "semaprax.project-candidate-dependency-page.v1",
        vec![
            ("image_digest", digest()),
            ("project_revision", digest()),
            ("workspace_revision", digest()),
            ("candidate_revision", digest()),
            ("base_project_revision", digest()),
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
            ("candidate_retained", json!({"const":false})),
            ("execution", json!({"const":false})),
            ("publication_authority", json!({"const":false})),
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
        "semaprax.image-analysis-artifact-evidence-chunk.v1",
        vec![
            (
                "report_schema",
                json!({"const":"semaprax.project-candidate-analysis-artifact-evidence.v1"}),
            ),
            ("report_sha256", digest()),
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
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "workspace/retained-subjects")
    }) {
        let candidate = object(vec![
            ("candidate_revision", digest()),
            ("base_project_revision", digest()),
            ("project_revision", digest()),
            ("retained_report_bytes", uint()),
            ("has_retained_drafts", json!({"type":"boolean"})),
            ("has_retained_attempts", json!({"type":"boolean"})),
            ("detail_method", json!({"const":"candidate/query"})),
            ("discard_method", json!({"const":"candidate/discard"})),
        ]);
        let draft = object(vec![
            ("draft_revision", digest()),
            ("source_candidate_revision", digest()),
            ("source_candidate_retained", json!({"type":"boolean"})),
            ("state", json!({"enum":["incomplete","ready_to_complete"]})),
            (
                "unresolved_hole_count",
                json!({"type":"integer","minimum":0,"maximum":16}),
            ),
            ("retained_report_bytes", uint()),
            ("detail_method", json!({"const":"hole/recovery-export"})),
            ("discard_method", json!({"const":"hole/discard"})),
        ]);
        let attempt = object(vec![
            ("attempt_revision", digest()),
            ("base_candidate_revision", digest()),
            ("base_project_revision", digest()),
            ("base_candidate_retained", json!({"type":"boolean"})),
            ("state", json!({"const":"rejected"})),
            (
                "diagnostic_count",
                json!({"type":"integer","minimum":1,"maximum":256}),
            ),
            ("retained_report_bytes", uint()),
            ("detail_method", json!({"const":"attempt/query"})),
            ("discard_method", json!({"const":"attempt/discard"})),
        ]);
        result.insert(
            "urn:semaprax.image-retained-subjects.v1".into(),
            document(
                "semaprax.image-retained-subjects.v1",
                vec![
                    ("image_revision", digest()),
                    (
                        "candidates",
                        json!({"type":"array","maxItems":16,"items":candidate}),
                    ),
                    (
                        "drafts",
                        json!({"type":"array","maxItems":16,"items":draft}),
                    ),
                    (
                        "attempts",
                        json!({"type":"array","maxItems":16,"items":attempt}),
                    ),
                    ("retained_report_bytes", uint()),
                    (
                        "limits",
                        object(vec![
                            ("max_candidates", json!({"const":16})),
                            ("max_drafts", json!({"const":16})),
                            ("max_attempts", json!({"const":16})),
                            ("max_retained_report_bytes", json!({"const":268435456})),
                            ("max_inventory_bytes", json!({"const":65536})),
                        ]),
                    ),
                    ("source_authority", json!({"const":false})),
                    ("artifact_materialization", json!({"const":false})),
                    ("execution", json!({"const":false})),
                    ("publication_authority", json!({"const":false})),
                    (
                        "nonclaims",
                        json!({"const":[
                            "session_inventory_is_not_persistent_storage",
                            "registry_association_is_not_ownership_or_current_candidate_validity",
                            "drafts_and_rejected_attempts_are_not_checked_candidates",
                            "references_grant_no_source_execution_materialization_or_publication_authority"
                        ]}),
                    ),
                ],
            ),
        );
    }
    if !capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "candidate/analysis-coverage")
    }) {
        result.remove("urn:semaprax.project-candidate-analysis-coverage.v1");
    }
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "candidate/test-task-start")
    }) {
        let common = || {
            vec![
                ("image_revision", digest()),
                ("project_revision", digest()),
                ("candidate_revision", digest()),
                ("task_revision", digest()),
                ("source_authority", json!({"const":false})),
                (
                    "authority",
                    object(vec![
                        ("source_write", json!({"const":false})),
                        ("process", json!({"const":false})),
                        ("network", json!({"const":false})),
                        ("target_runtime", json!({"const":false})),
                        ("publication", json!({"const":false})),
                    ]),
                ),
                (
                    "blind_spots",
                    json!({"const":["native_and_wasm_runtime","deployment_configuration",
                        "generated_artifacts","external_api_behavior","runtime_environment",
                        "external_consumers"]}),
                ),
            ]
        };
        let status = |cancel: bool| {
            let mut fields = common();
            fields.extend([
                (
                    "state",
                    json!({"enum":["queued","running","completed","cancelled","failed"]}),
                ),
                ("terminal", json!({"type":"boolean"})),
                ("cancellation_requested", json!({"type":"boolean"})),
                ("report_digest", nullable(digest())),
                ("passed", nullable(json!({"type":"boolean"}))),
                ("before_step", nullable(uint())),
                ("steps_used", nullable(uint())),
                ("max_steps", uint()),
                (
                    "diagnostics",
                    array(object(vec![
                        ("code", text()),
                        ("severity", json!({"enum":["error","warning"]})),
                        ("message", text()),
                        ("path", nullable(text())),
                        (
                            "location",
                            nullable(object(vec![
                                ("line", uint()),
                                ("column", uint()),
                                ("start", uint()),
                                ("end", uint()),
                            ])),
                        ),
                        ("help", nullable(text())),
                    ])),
                ),
            ]);
            if cancel {
                fields.push(("cancel_observed", json!({"const":true})));
            }
            fields
        };
        result.insert(
            "urn:semaprax.image-candidate-test-task-start.v1".into(),
            document("semaprax.image-candidate-test-task-start.v1", status(false)),
        );
        result.insert(
            "urn:semaprax.image-candidate-test-task-status.v1".into(),
            document(
                "semaprax.image-candidate-test-task-status.v1",
                status(false),
            ),
        );
        result.insert(
            "urn:semaprax.image-candidate-test-task-cancel.v1".into(),
            document("semaprax.image-candidate-test-task-cancel.v1", status(true)),
        );
        let mut result_fields = common();
        result_fields.extend([
            (
                "report_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_TEST_REPORT_SCHEMA}),
            ),
            ("report_digest", digest()),
            ("offset", uint()),
            ("total_bytes", uint()),
            (
                "chunk",
                json!({"type":"string","maxLength":524288,"x-max-utf8-bytes":524288}),
            ),
            ("next_offset", nullable(uint())),
            ("complete", json!({"type":"boolean"})),
        ]);
        result.insert(
            "urn:semaprax.image-candidate-test-task-result-chunk.v1".into(),
            document(
                "semaprax.image-candidate-test-task-result-chunk.v1",
                result_fields,
            ),
        );
    }
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "candidate/function-summary")
    }) {
        result.extend(candidate_function_schemas::documents());
    }
    if !capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "candidate/dependency-summary")
    }) {
        result.remove("urn:semaprax.project-candidate-dependency-summary.v1");
        result.remove("urn:semaprax.project-candidate-dependency-page.v1");
    }
    result.extend(candidate_schemas::documents());
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "image/function-instances")
    }) {
        result.extend(function_instance_schemas::documents());
    }
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "image/function-reference-export")
    }) {
        result.extend(function_reference_schemas::documents());
    }
    result.extend(hole_navigation_schemas::documents());
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "workspace/read-batch")
    }) {
        let mut request = object(vec![(
            "frames",
            json!({
                "type":"array","minItems":1,"maxItems":16,
                "items":{"type":"string","maxLength":65536,"x-max-utf8-bytes":65536}
            }),
        )]);
        request["$id"] = json!("urn:semaprax.image-read-batch-request.v1");
        request["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        result.insert("urn:semaprax.image-read-batch-request.v1".into(), request);
        result.insert(
            "urn:semaprax.image-read-batch.v1".into(),
            document("semaprax.image-read-batch.v1", vec![
                ("responses", json!({"type":"array","minItems":1,"maxItems":16,
                    "items":nullable(json!({"type":"string","maxLength":1048576,"x-max-utf8-bytes":1048576}))})),
                ("source_authority", json!({"const":false})),
            ]),
        );
    }
    if capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "candidate/merge-preview")
    }) {
        result.insert(
            "urn:semaprax.project-candidate-merge-preview.v1".into(),
            merge_preview_schemas::schema(),
        );
    }
    if capabilities["methods"]
        .as_array()
        .is_some_and(|methods| methods.iter().any(|method| method == "package/summary"))
    {
        result.extend(package_schemas::documents());
    }
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
