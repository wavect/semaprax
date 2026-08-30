//! V5 descriptions and clients derive from the exact host-selected registry.
use super::super::{
    method_description, text, Method, Operation, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
use super::{VNextPolicy, VNEXT_PROTOCOL_SCHEMA, VNEXT_RESULT_SCHEMA};
use crate::diagnostic::Diagnostic;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[path = "clients.rs"]
mod clients;
#[path = "payload_schemas.rs"]
mod payload_schemas;

const MAX_DISCOVERY_BYTES: usize = 900 * 1024;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) fn payload(
    method: &Method,
    params: &Map<String, Value>,
    methods: &[&'static Method],
    policy: &VNextPolicy,
    commit_enabled: bool,
) -> Result<Value> {
    let capabilities = capabilities(methods, policy, commit_enabled);
    let descriptors = methods
        .iter()
        .map(|method| descriptor(method, policy))
        .collect::<Vec<_>>();
    let result = match method.name {
        "protocol/capabilities" => capabilities,
        "protocol/instructions" => {
            json!({"schema":"semaprax.image-agent-instructions.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
            "instructions":"Read protocol/capabilities, protocol/schemas, then workspace/open. Only the host-selected methods exist. Every semantic request must bind the current exact image revision and any required candidate/draft/attempt digest. Optional means omitted, never null unless its schema explicitly permits null. Use workspace/refresh-preview with the currently bound image revision to observe a fresh Project revision without replacing session state; pass that observed_project_revision as expected_new_project_revision to workspace/refresh. Old semantic request tokens remain stale after replacement. Refresh is explicit and source drift never refreshes implicitly. Candidate preparation, diagnostics, interpreted tests, artifact projection, and source commit are distinct host-selected capabilities. Use only the selected catalogue and respect its limits. Schema bundle unbundled_payload_schemas explicitly identifies opaque payloads requiring their owning specification. Generated clients construct and decode messages only: they perform no I/O and cannot enable any capability. A source commit receipt is not permission to repeat publication."})
        }
        "query/catalog" => {
            json!({"schema":"semaprax.image-agent-query-catalog.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
            "queries":methods.iter().zip(&descriptors).filter(|(method,_)|method.query).map(|(_,descriptor)|descriptor).collect::<Vec<_>>()})
        }
        "protocol/schemas" => bundle(&descriptors, &capabilities)?,
        "protocol/client" => {
            let language = text(params, "language");
            let bundle = bundle(&descriptors, &capabilities)?;
            let source = clients::generate(language, &bundle)?;
            json!({"schema":"semaprax.image-agent-client.v5","protocol":VNEXT_PROTOCOL_SCHEMA,"language":language,
                "source":source,"io":false,"request_validation":"selected_descriptor_outer_parameters; nested_constructor_objects_require_compiler_admission",
                "result_validation":"envelope_and_bundled_payload_shapes; explicitly_unbundled_payloads_remain_opaque",
                "unbundled_payload_schemas":bundle["unbundled_payload_schemas"],
                "typescript_integer_policy":"reject_integers_outside_safe_integer_range; use_string_request_ids",
                "dependencies":match language { "rust"=>json!(["serde with derive","serde_json"]),"python"=>json!(["Python 3.11 standard library"]),_=>json!(["TypeScript ES2022","TextEncoder","structuredClone"])}})
        }
        _ => return Err(invalid("v5 discovery received a non-discovery method")),
    };
    bounded(result)
}

fn capabilities(methods: &[&Method], policy: &VNextPolicy, commit: bool) -> Value {
    let mut grants = vec!["semantic_read", "workspace_refresh"];
    if policy.candidate_prepare {
        grants.push("candidate_prepare");
    }
    if policy.diagnostics {
        grants.push("candidate_diagnostics");
    }
    if policy.test_policy.is_some() {
        grants.push("candidate_test");
    }
    if policy.build_enabled {
        grants.push("candidate_build");
    }
    if commit {
        grants.push("source_commit");
    }
    grants.sort();
    json!({"schema":"semaprax.image-agent-capabilities.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
        "capabilities":grants,"methods":methods.iter().map(|method|method.name).collect::<Vec<_>>(),
        "max_request_bytes":MAX_REQUEST_BYTES,"max_response_bytes":MAX_RESPONSE_BYTES,
        "source_authority":commit,"test_execution":policy.test_policy.is_some(),"target_execution":false,
        "artifact_projection":policy.build_enabled,"request_capability_changes":false,
        "test_policy":policy.test_policy.as_ref().map(|policy|json!({"max_steps":policy.max_steps(),"max_execution_bytes":policy.max_execution_bytes(),"max_report_bytes":policy.max_report_bytes(),"engine":"project_interpreter","request_overrides":false}))})
}

fn descriptor(method: &Method, policy: &VNextPolicy) -> Value {
    let mut value = method_description(method);
    let properties = &mut value["success_response_schema"]["properties"]["result"]["properties"];
    properties["schema"] = json!({"const":VNEXT_RESULT_SCHEMA});
    properties["protocol"] = json!({"const":VNEXT_PROTOCOL_SCHEMA});
    let payload = match method.name {
        "protocol/capabilities" => "semaprax.image-agent-capabilities.v5",
        "protocol/schemas" => "semaprax.image-agent-schemas.v5",
        "protocol/instructions" => "semaprax.image-agent-instructions.v5",
        "protocol/client" => "semaprax.image-agent-client.v5",
        "query/catalog" => "semaprax.image-agent-query-catalog.v5",
        "validation/catalog" if policy.test_policy.is_some() => {
            "semaprax.image-validation-catalog.v2"
        }
        _ => method.payload_schema,
    };
    properties["payload"] = if method.name == "hole/query" {
        json!({"oneOf":[{"$ref":"urn:semaprax.project-candidate-hole-context.v1"},{"$ref":"urn:semaprax.project-candidate-expression-hole-context.v1"}]})
    } else {
        json!({"$ref":format!("urn:{payload}")})
    };
    value["query"] = json!(method.query);
    value["capability"] = json!(match method.name {
        "workspace/refresh" | "workspace/refresh-preview" => "workspace_refresh",
        "candidate/test" => "candidate_test",
        "candidate/build" => "candidate_build",
        "candidate/commit" | "candidate/commit-report" | "source-commit/status" => "source_commit",
        name if name == "candidate/attempt" || name.starts_with("attempt/") =>
            "candidate_diagnostics",
        _ if matches!(method.operation, Operation::Candidate(_)) && !method.query =>
            "candidate_prepare",
        _ => "semantic_read",
    });
    value
}

fn bundle(descriptors: &[Value], capabilities: &Value) -> Result<Value> {
    let constructors: Value =
        serde_json::from_str(&crate::project::SemanticChange::constructor_schemas()?)
            .map_err(|_| invalid("compiler constructor schema bundle is invalid"))?;
    let mut documents = BTreeMap::new();
    for document in constructors["documents"]
        .as_array()
        .ok_or_else(|| invalid("constructor schema documents are absent"))?
    {
        let id = document["$id"]
            .as_str()
            .ok_or_else(|| invalid("constructor schema identity is absent"))?;
        documents.insert(id.to_owned(), document.clone());
    }
    for (id, document) in payload_schemas::documents(capabilities) {
        documents.insert(id, document);
    }
    for descriptor in descriptors {
        let method = descriptor["method"]
            .as_str()
            .ok_or_else(|| invalid("method descriptor lacks a name"))?;
        for field in [
            "request_schema",
            "success_response_schema",
            "error_response_schema",
        ] {
            let mut schema = descriptor[field].clone();
            let id = format!(
                "urn:semaprax.image-v5.{}:{}",
                field,
                method.replace('/', ".")
            );
            schema["$id"] = json!(id);
            documents.insert(id, schema);
        }
    }
    let mut references = BTreeSet::new();
    for document in documents.values() {
        references_in(document, &mut references);
    }
    // Chunk data is a string, so JSON Schema $ref traversal cannot discover its
    // owning report schema. List those report schemas explicitly as well.
    for descriptor in descriptors {
        let report = match descriptor["method"].as_str().unwrap_or("") {
            "candidate/query" => Some("semaprax.project-candidate.v1"),
            "candidate/recovery-export" => Some("semaprax.project-candidate-recovery.v1"),
            "attempt/query" => Some("semaprax.project-candidate-attempt.v1"),
            "candidate/semantic-delta" => Some("semaprax.project-candidate-semantic-delta.v1"),
            "candidate/semantic-delta-catalog" => {
                Some("semaprax.project-candidate-semantic-delta-catalog.v1")
            }
            "protocol/conformance" => Some(crate::project::IMAGE_PROTOCOL_CONFORMANCE_SCHEMA),
            "candidate/interface-catalog" => Some("semaprax.project-interface-change-catalog.v1"),
            "image/target-admission" => Some(crate::project::IMAGE_TARGET_ADMISSION_SCHEMA),
            "candidate/build" => Some(crate::project::IMAGE_ARTIFACT_PROJECTION_SCHEMA),
            "candidate/commit-report" => {
                Some(crate::project::PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA)
            }
            _ => None,
        };
        if let Some(report) = report {
            references.insert(format!("urn:{report}"));
        }
    }
    let unresolved = references
        .into_iter()
        .filter(|id| {
            !id.starts_with('#') && !documents.contains_key(id.split('#').next().unwrap_or(id))
        })
        .collect::<Vec<_>>();
    let mut request_refs = BTreeSet::new();
    for descriptor in descriptors {
        references_in(&descriptor["request_schema"], &mut request_refs);
    }
    if request_refs
        .iter()
        .any(|reference| unresolved.contains(reference))
    {
        return Err(invalid(
            "selected request descriptor has an unbundled constructor reference",
        ));
    }
    bounded(
        json!({"schema":"semaprax.image-agent-schemas.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
        "methods":descriptors,"documents":documents.into_values().collect::<Vec<_>>(),
        "unbundled_payload_schemas":unresolved,"request_schemas_complete":true,
        "payload_completeness":"only_bundled_documents_are_structurally_described; unbundled_payloads_require_owning_specification",
        "wire_rules":{"unknown_parameters":"reject","optional_parameters":"omit; null_rejected_unless_explicitly_nullable","strings":"UTF-8-byte_limits_and_control_character_rejection","request_ids":"unsigned_u64_or_nonempty_bounded_string; notifications_do_not_execute","integer_bounds":"exact_descriptor_minimum_and_maximum","max_request_bytes":MAX_REQUEST_BYTES,"max_response_bytes":MAX_RESPONSE_BYTES}}),
    )
}

fn references_in(value: &Value, output: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                output.insert(reference.to_owned());
            }
            for child in object.values() {
                references_in(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                references_in(child, output);
            }
        }
        _ => {}
    }
}
fn bounded(mut value: Value) -> Result<Value> {
    value.sort_all_objects();
    if serde_json::to_vec(&value)
        .map_err(|_| invalid("v5 discovery serialization failed"))?
        .len()
        > MAX_DISCOVERY_BYTES
    {
        return Err(vec![Diagnostic::io(
            "SPX-G289",
            "v5 discovery exceeds its bounded response payload",
        )]);
    }
    Ok(value)
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G288", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selected(policy: VNextPolicy) -> Value {
        let methods = super::super::methods(&policy, false);
        let capabilities = capabilities(&methods, &policy, false);
        bundle(
            &methods
                .iter()
                .map(|method| descriptor(method, &policy))
                .collect::<Vec<_>>(),
            &capabilities,
        )
        .unwrap()
    }
    #[test]
    fn selected_bundle_resolves_constructor_requests_and_marks_opaque_reports() {
        let bundle = selected(VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            build_enabled: true,
            ..VNextPolicy::default()
        });
        let ids = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|document| document["$id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains("urn:semaprax.semantic-change-intent.v1"));
        for id in [
            "urn:semaprax.image-workspace-refresh-preview.v1",
            "urn:semaprax.image-workspace-refresh.v1",
            "urn:semaprax.image-artifact-projection-chunk.v1",
        ] {
            assert!(ids.contains(id));
        }
        assert!(bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|schema| schema == "urn:semaprax.image-artifact-projection.v1"));
        let apply = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "candidate/apply-intent")
            .unwrap();
        assert_eq!(
            apply["request_schema"]["properties"]["params"]["additionalProperties"],
            false
        );
        let control = apply["request_schema"]["properties"]["id"]["oneOf"][1]["pattern"]
            .as_str()
            .unwrap();
        assert_eq!(control, r"^[^\u0000-\u001f\u007f-\u009f]+$");
    }
    #[test]
    fn optional_and_nullable_fields_remain_distinct_and_capabilities_do_not_expand() {
        let bundle = selected(VNextPolicy::default());
        assert!(!bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| method["method"] == "candidate/commit"
                || method["method"] == "candidate/apply-intent"));
        let context = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "image/context")
            .unwrap();
        let params = &context["request_schema"]["properties"]["params"];
        assert!(!params["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == "max_bytes"));
        assert_eq!(params["properties"]["max_bytes"]["type"], "integer");
        let chunk = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| document["$id"] == "urn:semaprax.image-target-admission-chunk.v1")
            .unwrap();
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == "candidate_revision"));
        assert_eq!(
            chunk["properties"]["candidate_revision"]["anyOf"][1]["type"],
            "null"
        );
    }
    #[test]
    fn generated_clients_have_typed_builders_bounds_and_actual_lf_escapes() {
        let bundle = selected(VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            build_enabled: true,
            ..VNextPolicy::default()
        });
        for language in ["typescript", "python", "rust"] {
            let source = clients::generate(language, &bundle).unwrap();
            assert!(source.contains("WorkspaceRefreshParams"));
            assert!(source.contains("request_workspace_refresh"));
            assert!(source.contains("decode_request_candidate_apply_intent"));
            assert!(source.contains("expected_new_project_revision"));
            assert!(source.contains("request byte bound"));
            assert!(!source.contains("request_candidate_commit("));
            assert!(source.len() < MAX_DISCOVERY_BYTES);
            match language {
                "typescript" => {
                    assert!(source.contains(r"return line+'\n';"));
                    assert!(source.contains("Number.isSafeInteger"));
                }
                "python" => {
                    assert!(source.contains(r"return line + '\n'"));
                    assert!(source.contains("NotRequired[int]"));
                }
                _ => {
                    assert!(source.contains(r#"Ok(line+"\n")"#));
                    assert!(source.contains("pub r#expected_new_project_revision: String"));
                }
            }
        }
    }

    #[test]
    fn candidate_payloads_and_optional_frontend_work_are_concrete() {
        let bundle = selected(VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            ..VNextPolicy::default()
        });
        let documents = bundle["documents"].as_array().unwrap();
        for id in [
            "semaprax.project-change-catalog.v1",
            "semaprax.project-candidate-comparison.v1",
            "semaprax.image-candidate-reconciliation.v1",
            "semaprax.project-candidate-rebase.v1",
            "semaprax.image-validation-catalog.v1",
            "semaprax.image-validation-catalog.v2",
            "semaprax.project-candidate-semantic-delta-catalog.v1",
            "semaprax.project-candidate-test-plan.v1",
        ] {
            let uri = format!("urn:{id}");
            let schema = documents.iter().find(|doc| doc["$id"] == uri).unwrap();
            assert_eq!(schema["additionalProperties"], false);
            assert_eq!(schema["properties"]["schema"]["const"], id);
            assert!(!bundle["unbundled_payload_schemas"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == &uri));
        }
        for id in [
            "urn:semaprax.image-workspace-refresh.v1",
            "urn:semaprax.image-workspace-refresh-preview.v1",
        ] {
            let schema = documents.iter().find(|doc| doc["$id"] == id).unwrap();
            assert!(!schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|name| name == "frontend_work"));
            assert_eq!(
                schema["properties"]["frontend_work"],
                json!({"$ref":"urn:semaprax.project-frontend-cache-work.v1"})
            );
        }
        let frontend = documents
            .iter()
            .find(|doc| doc["$id"] == "urn:semaprax.project-frontend-cache-work.v1")
            .unwrap();
        assert_eq!(
            frontend["properties"]["invalidated_sources"]["type"],
            "array"
        );
        let catalog = documents
            .iter()
            .find(|doc| doc["$id"] == "urn:semaprax.project-change-catalog.v1")
            .unwrap();
        let operations = catalog["properties"]["operations"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(operations.len(), 11);
        for kind in [
            "rename_declaration",
            "change_function_signature",
            "replace_function_body",
            "repair_diagnostic",
            "replace_expression",
            "add_contract",
            "add_declaration",
            "extract_function",
            "move_declaration",
            "add_record_field",
            "implement_interface",
        ] {
            let operation = operations
                .iter()
                .find(|schema| schema["properties"]["kind"]["const"] == kind)
                .unwrap();
            assert_eq!(operation["additionalProperties"], false);
        }
    }
}
