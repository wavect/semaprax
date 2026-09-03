//! Closed response documents for startup-selected candidate archive storage.

use std::collections::BTreeMap;

use serde_json::{json, Value};

pub(super) fn documents(capabilities: &Value) -> BTreeMap<String, Value> {
    if !capabilities["methods"].as_array().is_some_and(|methods| {
        methods
            .iter()
            .any(|method| method == "candidate/archive-store")
    }) {
        return BTreeMap::new();
    }
    let image_receipt = object(vec![
        ("kind", json!({"const":"image"})),
        ("subject_digest", digest()),
        ("stored_bytes", uint()),
        ("receipt_digest", digest()),
        ("image_digest", digest()),
        ("revision_store_entry", digest()),
        ("project_revision", digest()),
    ]);
    let candidate_receipt = object(vec![
        ("kind", json!({"const":"candidate"})),
        ("subject_digest", digest()),
        ("stored_bytes", uint()),
        ("archive_digest", digest()),
        ("candidate_digest", digest()),
        ("base_revision", digest()),
    ]);
    let draft_receipt = object(vec![
        ("kind", json!({"const":"draft"})),
        ("subject_digest", digest()),
        ("stored_bytes", uint()),
        ("archive_digest", digest()),
        ("draft_digest", digest()),
        ("base_revision", digest()),
    ]);
    let mut result = BTreeMap::new();
    result.insert(
        "urn:semaprax.semantic-retention-lifecycle-report.v1".into(),
        document(
            "semaprax.semantic-retention-lifecycle-report.v1",
            vec![
                ("successful_receipt_count", uint()),
                (
                    "successful_store_receipts",
                    json!({"type":"array","maxItems":96,"items":{"oneOf":[
                        image_receipt,candidate_receipt,draft_receipt
                    ]}}),
                ),
                (
                    "subject_store_status",
                    json!({"enum":[
                        "successful_receipts_precede_registry_attempt",
                        "successful_typed_store_receipt_was_supplied",
                        "no_successful_receipt_batch_accepted"
                    ]}),
                ),
                (
                    "registry_cursor_status",
                    json!({"enum":[
                        "advanced",
                        "registry_cursor_not_advanced",
                        "registry_cursor_not_advanced_stale",
                        "registry_cursor_not_advanced_pair_publication_uncertain",
                        "registry_cursor_uncertain_recovery_required",
                        "registry_attempt_blocked_reopen_required",
                        "no_registry_attempt_invalid_receipt_inventory",
                        "no_registry_attempt_receipt_capacity_exceeded",
                        "no_registry_attempt_receipt_projection_failed",
                        "registry_cursor_advanced_report_unavailable",
                        "registry_outcome_report_unavailable"
                    ]}),
                ),
                ("sequence", nullable(uint())),
                ("cursor_digest", nullable(digest())),
                (
                    "diagnostic_codes",
                    json!({"type":"array","maxItems":64,"items":text()}),
                ),
                (
                    "next_action",
                    json!({"enum":[
                        "continue_with_the_returned_exact_cursor",
                        "retry_with_a_bounded_nonempty_successful_receipt_batch",
                        "inspect_the_successful_typed_receipt_before_retry",
                        "recover_registry_and_reopen_with_an_exact_startup_expectation"
                    ]}),
                ),
                ("authority", json!({"const":"none"})),
                (
                    "nonclaims",
                    json!({"const":[
                        "successful_receipt_does_not_grant_subject_store_or_restore_authority",
                        "registry_checkpoint_does_not_apply_or_approve_the_GC_plan",
                        "registry_failure_does_not_undo_or_deny_prior_immutable_subject_storage",
                        "no_source_candidate_draft_image_approval_or_publication_state_is_changed",
                        "no_implicit_root_discovery_freshness_clock_mtime_or_access_frequency"
                    ]}),
                ),
            ],
        ),
    );
    let retention = json!({"oneOf":[
        object(vec![
            ("selected",json!({"const":false})),
            ("outcome",json!({"type":"null"})),
            ("status",json!({"const":"not_selected_before_frames"})),
        ]),
        object(vec![
            ("selected",json!({"const":true})),
            ("outcome",json!({"$ref":"urn:semaprax.semantic-retention-lifecycle-report.v1"})),
            ("status",json!({"const":"checkpoint_outcome_returned"})),
        ])
    ]});
    let schema = super::IMAGE_CANDIDATE_ARCHIVE_STORE_SCHEMA;
    result.insert(
        format!("urn:{schema}"),
        document(
            schema,
            vec![
                ("image_revision", digest()),
                ("candidate_revision", digest()),
                ("archive_digest", digest()),
                ("base_project_revision", digest()),
                ("stored_bytes", uint()),
                ("store_status", json!({"const":"immutable_archive_stored"})),
                ("retention_lifecycle", retention),
                ("source_authority", json!({"const":false})),
                ("approval_authority", json!({"const":false})),
                ("publication_authority", json!({"const":false})),
                ("restore_authority", json!({"const":false})),
                ("gc_authority", json!({"const":false})),
                (
                    "nonclaims",
                    json!({"const":[
                        "archive_store_success_does_not_make_the_candidate_current",
                        "retention_checkpoint_failure_does_not_undo_or_deny_archive_store_success",
                        "request_contains_no_store_or_registry_path_policy_or_authority",
                        "no_restore_delete_gc_approval_source_write_or_publication_operation"
                    ]}),
                ),
            ],
        ),
    );
    result
}

fn digest() -> Value {
    json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"})
}

fn text() -> Value {
    json!({"type":"string"})
}

fn uint() -> Value {
    json!({"type":"integer","minimum":0,"maximum":u64::MAX})
}

fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}

fn object(fields: Vec<(&str, Value)>) -> Value {
    let required = fields.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let properties = fields
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}

fn document(id: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut all = vec![("schema", json!({"const":id}))];
    all.extend(fields);
    let mut result = object(all);
    result["$id"] = json!(format!("urn:{id}"));
    result["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    result
}
