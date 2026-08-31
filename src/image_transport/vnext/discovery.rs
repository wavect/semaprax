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
    let mut result = match method.name {
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
    if method.name == "protocol/instructions" {
        let mut instructions = result["instructions"].as_str().unwrap_or("").to_owned();
        instructions.push_str(" Use image/dependencies with the current image_revision and a stable declaration target to inspect bounded compiler-derived reverse dependency facts. It is read-only and grants no candidate or execution authority. Read UTF8 chunks from offset zero using next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the complete report is bounded to 8 MiB. The heterogeneous dependency report remains explicitly unbundled; its closed chunk envelope does not prove transitive runtime effects.");
        instructions.push_str(" Use image/cleanup-dependencies with the current image_revision and a stable declaration target to inspect compiler-derived relationships to ordered cleanup facts. Read UTF-8 chunks from offset zero using next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the report is bounded to 8 MiB. The heterogeneous report is explicitly unbundled. This immutable read grants no candidate, execution, physical cleanup or source authority and does not prove runtime liveness or destruction order.");
        if methods
            .iter()
            .any(|method| method.name == "candidate/cleanup-dependencies")
        {
            instructions.push_str(" With candidate_prepare, use candidate/cleanup-dependencies with candidate_revision and target for before/after cleanup dependency facts against the original source base. Keep image_revision, candidate_revision and target fixed while reassembling offset/next_offset chunks; chunk_bytes is 1024 through 65536 (default 16384), report bound 8 MiB. The heterogeneous report remains explicitly unbundled. This read is excluded from immutable image batches and grants no execution or publication authority.");
        }
        instructions.push_str(" For compact navigation, first call image/dependency-summary, then pass the selected sites, callers, calls or members facet handle to image/dependency-page. Keep image_revision, target, view, page_size and max_bytes fixed while following next_cursor; omit cursor on the first page. Page size is 1 through 128 (default 32), and max_bytes is 1024 through 1048576 (default 65536). Handles and cursors are bound compiler references, not authority; do not synthesize or reuse them with another target or image. Page wrappers are closed, but heterogeneous item facts remain explicitly unbundled.");
        if methods
            .iter()
            .any(|method| method.name == "candidate/interface-delta")
        {
            instructions.push_str(" Use candidate/interface-delta to compare whole-candidate static interface bindings, every affected member and retained direct-call dependencies; this does not prove runtime dispatch or behavior.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/contract-delta")
        {
            instructions.push_str(" Use candidate/contract-delta with candidate_revision to review whole-candidate contract changes and changed functions used by predicates against the candidate's original base. There is no target parameter. Reassemble the exact UTF-8 report using offset and next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the report is bounded to 8 MiB. Its heterogeneous compiler report remains explicitly unbundled. Descriptive changes do not prove predicate implication, behavioral equivalence, or execution and grant no source authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/ownership-delta")
        {
            instructions.push_str(" Use candidate/ownership-delta with candidate_revision to compare whole-candidate ownership, loan, and cleanup compiler facts against the candidate's original base. There is no target parameter. Reassemble UTF-8 chunks using offset and next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the report is bounded to 8 MiB. Full compiler plan payloads remain explicitly unbundled. These descriptive facts do not establish runtime liveness, destruction traces, backend execution, or publication authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/artifact-delta")
        {
            instructions.push_str(" Use candidate/build or candidate/artifact-delta only when candidate_build is granted. Bind candidate_revision and select kind web, npm, openapi or c for an independently replayed pathless artifact projection or its before/after comparison against the original base. OpenAPI projects supported manifest-selected export signatures as schema documents; it does not host or execute an HTTP service or establish external compatibility. C projects supported manifest-selected scalar exports into native-emitter-derived C declarations; it does not compile or link a C consumer or establish a general foreign ABI. Candidate replay precedes builds; each side has a fixed 16 MiB build limit that requests cannot override. Reassemble reports using offset and next_offset, with chunk_bytes 1024 through 65536 (default 16384); projection reports are bounded to 1 MiB and delta reports to 8 MiB. Heterogeneous reports remain explicitly unbundled. No artifact paths are written, package manager or target executable is run, or publication authority granted.");
        }
        if methods
            .iter()
            .any(|method| method.name == "hole/recovery-export")
        {
            if methods
                .iter()
                .any(|method| method.name == "hole/open-contract-expression")
            {
                instructions.push_str(" Use candidate/contract-expression-catalog for authenticated requires/ensures selections, then hole/open-contract-expression with candidate_revision, target, expression_id, hole_id and optional draft_revision. Phase and predicate ordinal are compiler-derived, never source spans or request paths. Existing hole/open-expression remains body-only. Use hole/query for the selected contract scope; result is available only in ensures. Fill through hole/fill, then complete only after every body and contract hole is resolved. Failed fills preserve the draft; selections are reauthenticated after successful fills. Context and catalogue reports remain explicitly unbundled and confer no validity, execution or publication authority.");
            }
            instructions.push_str(" Use hole/recovery-export to save prior valid history and pending selectors. hole/recovery-restore requires the same exact original source base and returns only a draft; every remaining hole must still be filled before completion. Recovery does not restore approvals or implicitly rebase after source edits.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/symbol-diagnostics")
        {
            instructions.push_str(" Use candidate/symbol-diagnostics for retained rejected intentions matching an exact candidate and target. Start at offset zero, then pass expected_report_revision from that first chunk for every nonzero offset. If that report revision becomes stale, restart at zero. Empty results do not mean the candidate has no diagnostics; rejected spans are not verified candidate locations.");
        }
        result["instructions"] = json!(instructions);
    }
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
        json!({"oneOf":[{"$ref":"urn:semaprax.project-candidate-hole-context.v1"},{"$ref":"urn:semaprax.project-candidate-expression-hole-context.v1"},{"$ref":"urn:semaprax.project-candidate-contract-expression-hole-context.v1"}]})
    } else {
        json!({"$ref":format!("urn:{payload}")})
    };
    value["query"] = json!(method.query);
    value["capability"] = json!(match method.name {
        "workspace/refresh" | "workspace/refresh-preview" => "workspace_refresh",
        "candidate/test" => "candidate_test",
        "candidate/build" | "candidate/artifact-delta" => "candidate_build",
        "candidate/interface-delta"
        | "candidate/contract-delta"
        | "candidate/ownership-delta"
        | "candidate/cleanup-dependencies"
        | "candidate/contract-expression-catalog"
        | "hole/open-contract-expression"
        | "hole/recovery-export"
        | "hole/recovery-restore" => "candidate_prepare",
        "candidate/commit" | "candidate/commit-report" | "source-commit/status" => "source_commit",
        name if name == "candidate/attempt"
            || name == "candidate/symbol-diagnostics"
            || name.starts_with("attempt/") =>
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
    if descriptors.iter().any(|descriptor| {
        matches!(
            descriptor["method"].as_str(),
            Some("hole/recovery-export" | "hole/recovery-restore")
        )
    }) {
        let schema = draft_recovery_schema();
        documents.insert(
            format!(
                "urn:{}",
                crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA
            ),
            schema,
        );
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
            "image/dependencies" => Some(crate::project::IMAGE_DECLARATION_DEPENDENCIES_SCHEMA),
            "image/cleanup-dependencies" => Some(crate::project::IMAGE_CLEANUP_DEPENDENCIES_SCHEMA),
            "candidate/cleanup-dependencies" => {
                Some(crate::project::PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA)
            }
            "hole/recovery-export" => Some(crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA),
            "attempt/query" => Some("semaprax.project-candidate-attempt.v1"),
            "candidate/semantic-delta" => Some("semaprax.project-candidate-semantic-delta.v1"),
            "candidate/semantic-delta-catalog" => {
                Some("semaprax.project-candidate-semantic-delta-catalog.v1")
            }
            "protocol/conformance" => Some(crate::project::IMAGE_PROTOCOL_CONFORMANCE_SCHEMA),
            "candidate/interface-catalog" => Some("semaprax.project-interface-change-catalog.v1"),
            "candidate/interface-delta" => Some("semaprax.project-candidate-interface-delta.v1"),
            "candidate/contract-delta" => Some("semaprax.project-candidate-contract-delta.v1"),
            "candidate/ownership-delta" => Some("semaprax.project-candidate-ownership-delta.v1"),
            "candidate/symbol-diagnostics" => {
                Some("semaprax.project-candidate-symbol-diagnostics.v1")
            }
            "image/target-admission" => Some(crate::project::IMAGE_TARGET_ADMISSION_SCHEMA),
            "candidate/build" => Some(crate::project::IMAGE_ARTIFACT_PROJECTION_SCHEMA),
            "candidate/artifact-delta" => Some("semaprax.project-candidate-artifact-delta.v1"),
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

fn draft_recovery_schema() -> Value {
    use payload_schemas::{digest, document, object};
    let id = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"});
    let target = json!({"type":"string","minLength":1,"maxLength":4096,"x-max-utf8-bytes":4096});
    let body = object(vec![
        ("kind", json!({"const":"function_body"})),
        ("hole_id", id.clone()),
        ("target", target.clone()),
    ]);
    let expression = object(vec![
        ("kind", json!({"const":"expression"})),
        ("hole_id", id.clone()),
        ("target", target.clone()),
        ("expression_id", target.clone()),
    ]);
    let contract = object(vec![
        ("kind", json!({"const":"contract_expression"})),
        ("hole_id", id),
        ("target", target.clone()),
        ("expression_id", target),
    ]);
    document(
        crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
        vec![
            (
                "compiler",
                json!({"const":{
                    "package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),
                    "compatibility":crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY,
                }}),
            ),
            ("base_revision", digest()),
            (
                "draft_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_DRAFT_SCHEMA}),
            ),
            (
                "candidate_recovery",
                json!({"$ref":"urn:semaprax.project-candidate-recovery.v1"}),
            ),
            (
                "holes",
                json!({"type":"array","maxItems":crate::project::MAX_PROJECT_CANDIDATE_HOLES,
            "items":{"oneOf":[body,expression,contract]},"x-sorted-by":"hole_id","x-unique-hole-id":true}),
            ),
            ("draft_digest", digest()),
            ("capsule_digest", digest()),
        ],
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
    fn dependency_query_is_read_only_with_closed_chunks_and_opaque_facts() {
        let bundle = selected(VNextPolicy::default());
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "image/dependencies")
            .unwrap();
        assert_eq!(method["capability"], "semantic_read");
        assert_eq!(method["query"], true);
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
        assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
        assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
        let chunk = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| {
                document["$id"] == "urn:semaprax.image-declaration-dependencies-chunk.v1"
            })
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(chunk["properties"]["source_authority"]["const"], false);
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .contains(&json!("target")));
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .contains(&json!("image_revision")));
        assert!(bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!("urn:semaprax.image-declaration-dependencies.v1")));
    }
    #[test]
    fn cleanup_dependencies_are_v5_read_only_with_closed_chunks_and_generated_clients() {
        let bundle = selected(VNextPolicy::default());
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "image/cleanup-dependencies")
            .unwrap();
        assert_eq!(method["capability"], "semantic_read");
        assert_eq!(method["query"], true);
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"].as_object().unwrap().len(), 4);
        assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
        assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
        assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
        assert_eq!(params["required"], json!(["image_revision", "target"]));
        let chunk = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| document["$id"] == "urn:semaprax.image-cleanup-dependencies-chunk.v1")
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(chunk["properties"]["source_authority"]["const"], false);
        assert_eq!(
            chunk["properties"]["report_schema"]["const"],
            crate::project::IMAGE_CLEANUP_DEPENDENCIES_SCHEMA
        );
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .contains(&json!("image_revision")));
        assert!(chunk["required"]
            .as_array()
            .unwrap()
            .contains(&json!("target")));
        assert!(bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!("urn:semaprax.image-cleanup-dependencies.v1")));
        for language in ["typescript", "python", "rust"] {
            let source = clients::generate(language, &bundle).unwrap();
            assert!(source.contains("image/cleanup-dependencies"));
            assert!(source.contains("semaprax.image-cleanup-dependencies-chunk.v1"));
        }
        for test_enabled in [false, true] {
            assert!(
                !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                    .iter()
                    .any(|method| method.name == "image/cleanup-dependencies")
            );
        }
    }
    #[test]
    fn candidate_cleanup_dependencies_require_candidate_grant_and_exact_target_binding() {
        let readonly = selected(VNextPolicy::default());
        assert!(!readonly["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| method["method"] == "candidate/cleanup-dependencies"));
        let bundle = selected(VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        });
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "candidate/cleanup-dependencies")
            .unwrap();
        assert_eq!(method["capability"], "candidate_prepare");
        assert_eq!(method["query"], true);
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"].as_object().unwrap().len(), 5);
        assert_eq!(
            params["required"],
            json!(["image_revision", "candidate_revision", "target"])
        );
        assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
        let chunk = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| {
                document["$id"] == "urn:semaprax.image-candidate-cleanup-dependencies-chunk.v1"
            })
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(chunk["properties"]["source_authority"]["const"], false);
        assert_eq!(
            chunk["properties"]["report_schema"]["const"],
            crate::project::PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA
        );
        for field in ["image_revision", "candidate_revision", "target"] {
            assert!(chunk["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field)));
        }
        assert!(bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(
                "urn:semaprax.project-candidate-cleanup-dependencies.v1"
            )));
        for language in ["typescript", "python", "rust"] {
            let source = clients::generate(language, &bundle).unwrap();
            assert!(source.contains("candidate/cleanup-dependencies"));
            assert!(source.contains("semaprax.image-candidate-cleanup-dependencies-chunk.v1"));
        }
        for test_enabled in [false, true] {
            assert!(
                !crate::image_transport::candidates::diagnostics::methods(test_enabled)
                    .iter()
                    .any(|method| method.name == "candidate/cleanup-dependencies")
            );
        }
    }
    #[test]
    fn draft_recovery_is_v5_candidate_only_with_closed_replay_schema() {
        let readonly = selected(VNextPolicy::default());
        assert!(!readonly["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| method["method"]
                .as_str()
                .unwrap()
                .starts_with("hole/recovery-")));
        let bundle = selected(VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        });
        for name in ["hole/recovery-export", "hole/recovery-restore"] {
            let descriptor = bundle["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|method| method["method"] == name)
                .unwrap();
            assert_eq!(descriptor["capability"], "candidate_prepare");
            assert_eq!(descriptor["query"], name == "hole/recovery-export");
        }
        let capsule_id = format!(
            "urn:{}",
            crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA
        );
        let capsule = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| document["$id"] == capsule_id)
            .unwrap();
        assert_eq!(capsule["additionalProperties"], false);
        assert_eq!(
            capsule["properties"]["compiler"]["const"]["compatibility"],
            crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY
        );
        assert_eq!(
            capsule["required"],
            json!([
                "schema",
                "compiler",
                "base_revision",
                "draft_schema",
                "candidate_recovery",
                "holes",
                "draft_digest",
                "capsule_digest"
            ])
        );
        assert_eq!(
            capsule["properties"]["candidate_recovery"]["$ref"],
            "urn:semaprax.project-candidate-recovery.v1"
        );
        let holes = &capsule["properties"]["holes"];
        assert_eq!(holes["maxItems"], 16);
        let kinds = holes["items"]["oneOf"].as_array().unwrap();
        assert_eq!(kinds.len(), 3);
        assert_eq!(kinds[0]["required"], json!(["kind", "hole_id", "target"]));
        assert_eq!(
            kinds[1]["required"],
            json!(["kind", "hole_id", "target", "expression_id"])
        );
        assert_eq!(
            kinds[2]["required"],
            json!(["kind", "hole_id", "target", "expression_id"])
        );
        assert_eq!(
            kinds[2]["properties"]["kind"]["const"],
            "contract_expression"
        );
        for kind in kinds {
            assert_eq!(kind["additionalProperties"], false);
        }
        assert!(!bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(capsule_id)));
        for test_enabled in [false, true] {
            assert!(!crate::image_transport::candidates::methods(test_enabled)
                .iter()
                .any(|method| method.name.starts_with("hole/recovery-")));
        }
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
            "urn:semaprax.image-interface-delta-chunk.v1",
            "urn:semaprax.image-symbol-diagnostics-chunk.v1",
        ] {
            assert!(ids.contains(id));
        }
        assert!(bundle["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|schema| schema == "urn:semaprax.image-artifact-projection.v1"));
        for report in [
            "urn:semaprax.project-candidate-interface-delta.v1",
            "urn:semaprax.project-candidate-symbol-diagnostics.v1",
        ] {
            assert!(bundle["unbundled_payload_schemas"]
                .as_array()
                .unwrap()
                .contains(&json!(report)));
        }
        let diagnostics = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "candidate/symbol-diagnostics")
            .unwrap();
        assert_eq!(diagnostics["capability"], "candidate_diagnostics");
        let interface = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == "candidate/interface-delta")
            .unwrap();
        assert_eq!(interface["capability"], "candidate_prepare");
        let params = &diagnostics["request_schema"]["properties"]["params"];
        assert!(!params["required"]
            .as_array()
            .unwrap()
            .contains(&json!("expected_report_revision")));
        assert_eq!(
            params["properties"]["expected_report_revision"]["type"],
            "string"
        );
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
    fn artifact_kinds_use_existing_build_grant_and_closed_client_schemas() {
        for policy in [
            VNextPolicy::default(),
            VNextPolicy {
                candidate_prepare: true,
                ..VNextPolicy::default()
            },
        ] {
            let bundle = selected(policy);
            for name in ["candidate/build", "candidate/artifact-delta"] {
                assert!(!bundle["methods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|method| method["method"] == name));
            }
        }
        let bundle = selected(VNextPolicy {
            candidate_prepare: true,
            build_enabled: true,
            ..VNextPolicy::default()
        });
        for name in ["candidate/build", "candidate/artifact-delta"] {
            let method = bundle["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|method| method["method"] == name)
                .unwrap();
            assert_eq!(method["capability"], "candidate_build");
            assert_eq!(method["query"], false);
            let params = &method["request_schema"]["properties"]["params"];
            assert_eq!(params["additionalProperties"], false);
            assert_eq!(
                params["properties"]["kind"]["enum"],
                json!(["web", "npm", "openapi", "c"])
            );
            assert_eq!(params["properties"].as_object().unwrap().len(), 5);
            assert!(params["properties"].get("max_build_bytes").is_none());
            assert!(params["properties"].get("path").is_none());
        }
        for id in [
            "urn:semaprax.image-artifact-projection-chunk.v1",
            "urn:semaprax.image-artifact-delta-chunk.v1",
        ] {
            let chunk = bundle["documents"]
                .as_array()
                .unwrap()
                .iter()
                .find(|document| document["$id"] == id)
                .unwrap();
            assert_eq!(chunk["additionalProperties"], false);
            let kind = &chunk["properties"]["kind"];
            let choices = kind
                .get("enum")
                .or_else(|| kind["anyOf"][0].get("enum"))
                .unwrap();
            assert_eq!(choices, &json!(["web", "npm", "openapi", "c"]));
            for field in [
                "source_authority",
                "artifact_materialization",
                "target_execution",
            ] {
                assert_eq!(chunk["properties"][field]["const"], false);
            }
        }
        for language in ["typescript", "python", "rust"] {
            let source = clients::generate(language, &bundle).unwrap();
            assert!(source.contains("openapi"));
            assert!(source.contains("request_candidate_build"));
            assert!(source.contains("request_candidate_artifact_delta"));
        }
        for test_enabled in [false, true] {
            for method in crate::image_transport::candidates::diagnostics::methods(test_enabled) {
                assert!(!serde_json::to_string(&method_description(method))
                    .unwrap()
                    .contains("openapi"));
            }
        }
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
            assert!(source.contains("request_candidate_interface_delta"));
            assert!(source.contains("request_candidate_contract_delta"));
            assert!(source.contains("request_candidate_ownership_delta"));
            assert!(source.contains("request_candidate_artifact_delta"));
            assert!(source.contains("request_candidate_contract_expression_catalog"));
            assert!(source.contains("request_hole_open_contract_expression"));
            assert!(source.contains("request_candidate_symbol_diagnostics"));
            assert!(source.contains("expected_report_revision"));
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
                json!({"oneOf":[
                    {"$ref":"urn:semaprax.project-frontend-cache-work.v1"},
                    {"$ref":"urn:semaprax.project-semantic-cache-work.v1"}
                ]})
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
        assert_eq!(
            frontend["properties"]["work"]["properties"]["checked_HIR_reused"],
            json!({"const":0})
        );
        let semantic = documents
            .iter()
            .find(|doc| doc["$id"] == "urn:semaprax.project-semantic-cache-work.v1")
            .unwrap();
        assert_eq!(
            semantic["properties"]["schema"]["const"],
            "semaprax.project-semantic-cache-work.v1"
        );
        assert_eq!(
            semantic["properties"]["work"]["properties"]["checked_HIR_reused"],
            json!({"type":"integer","minimum":0,"maximum":16})
        );
        let catalog = documents
            .iter()
            .find(|doc| doc["$id"] == "urn:semaprax.project-change-catalog.v1")
            .unwrap();
        let operations = catalog["properties"]["operations"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(operations.len(), 12);
        for kind in [
            "rename_declaration",
            "change_function_signature",
            "replace_function_body",
            "repair_diagnostic",
            "replace_expression",
            "replace_contract_expression",
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
