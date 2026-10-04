//! Exact transport-owned payload shapes. Compiler reports without a full shape
//! here remain explicitly unbundled instead of being represented as empty schemas.
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[path = "candidate_function_schemas.rs"]
mod candidate_function_schemas;
#[path = "candidate_schemas.rs"]
mod candidate_schemas;
#[path = "explorer_schemas.rs"]
mod explorer_schemas;
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
#[path = "payload_schemas/document_bundle.rs"]
mod document_bundle;
pub(super) use document_bundle::documents;
