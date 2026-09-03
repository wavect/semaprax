//! Caller-declared deployment configuration contracts attached to one exact
//! candidate analysis boundary. A declaration describes expected key shapes;
//! it never supplies values, locators, observed environment state, or authority.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;
use crate::hir::IdentityOrigin;

use super::{wire, ProjectCandidate, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_SCHEMA: &str =
    "semaprax.project-candidate-deployment-contract-declaration.v1";
pub const PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA: &str =
    "semaprax.project-candidate-deployment-contract-evidence.v1";
pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_BYTES: usize = 65_536;
pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEYS: usize = 64;
pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEY_BYTES: usize = 128;

const DECLARATION_DOMAIN: &[u8] =
    b"semaprax.project-candidate-deployment-contract-declaration.v1\0";
const AREA_ORDER: [&str; 8] = [
    "declared_source_inputs",
    "declared_external_contracts",
    "deployment_configuration",
    "generated_file_provenance",
    "generated_artifacts",
    "external_api_behavior",
    "runtime_environment",
    "external_consumers",
];

impl ProjectCandidate {
    /// Attach one explicit caller declaration to the analysis boundary for this
    /// exact candidate. The declaration contains key shapes only and is neither
    /// environment observation nor permission to read or deploy anything.
    pub fn analysis_deployment_contract_evidence(
        &self,
        expected_candidate: &str,
        declaration_bytes: &[u8],
        expected_declaration_digest: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let declaration =
            authenticate_declaration(self, declaration_bytes, expected_declaration_digest)?;
        let mut coverage: Value =
            serde_json::from_str(&self.analysis_coverage(expected_candidate)?)
                .map_err(|_| invalid("candidate analysis coverage is not compiler JSON"))?;
        validate_coverage(self, &coverage)?;

        let object = coverage
            .as_object_mut()
            .ok_or_else(|| invalid("candidate analysis coverage is not an object"))?;
        object.insert(
            "schema".into(),
            json!(PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA),
        );
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_explicit_caller_deployment_contract_declaration"),
        );

        let areas = object
            .get_mut("areas")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage areas are absent"))?;
        if areas.len() != AREA_ORDER.len()
            || areas
                .iter()
                .zip(AREA_ORDER)
                .any(|(row, name)| row["area"].as_str() != Some(name))
        {
            return Err(invalid(
                "candidate analysis coverage area inventory is not canonical",
            ));
        }
        let deployment = &mut areas[2];
        if *deployment
            != json!({
                "area":"deployment_configuration",
                "status":"not_inspected",
                "basis":"no_deployment_configuration_inputs_or_io",
                "limitations":[
                    "environment_variables_secrets_routing_and_infrastructure_are_not_discovered",
                    "manifest_capabilities_are_not_deployment_state"
                ],
                "required_evidence":[
                    "explicit_authenticated_deployment_inputs_with_separate_analysis_and_authority"
                ]
            })
        {
            return Err(invalid(
                "candidate analysis coverage deployment boundary is unexpected",
            ));
        }
        *deployment = json!({
            "area":"deployment_configuration",
            "status":"partial",
            "basis":"caller_supplied_canonical_configuration_contract_bound_to_exact_candidate_and_manifest_exports",
            "limitations":[
                "declaration_is_not_observed_environment_or_deployed_configuration_state",
                "configuration_key_shapes_supply_no_values_secrets_paths_urls_or_provider_locators",
                "no_artifact_runtime_external_api_or_consumer_verification",
                "no_deployment_execution_freshness_or_drift_observation"
            ],
            "required_evidence":[
                "authorized_environment_observation_bound_to_this_candidate_and_declaration",
                "artifact_runtime_external_api_and_deployment_conformance_evidence"
            ]
        });

        let blind_spots = object
            .get_mut("blind_spots")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis blind-spot inventory is absent"))?;
        let matching = blind_spots
            .iter_mut()
            .filter(|row| row["domain"] == "deployment_configuration")
            .collect::<Vec<_>>();
        if matching.len() != 1
            || *matching[0]
                != json!({
                    "domain":"deployment_configuration",
                    "evidence_status":"absent",
                    "absent_evidence":"no_authenticated_deployment_configuration_evidence",
                    "source_binding":{
                        "kind":"exact_retained_project_revision_and_manifest_source_inventory",
                        "project_revision":self.revision.project_revision()
                    },
                    "nonclaim":"not_evidence_that_no_deployment_contract_exists"
                })
        {
            return Err(invalid(
                "candidate deployment blind-spot boundary is unexpected",
            ));
        }
        *matching.into_iter().next().expect("one deployment row") = json!({
            "domain":"deployment_configuration",
            "evidence_status":"partial",
            "basis":"authenticated_caller_declaration_only",
            "source_binding":{
                "kind":"exact_candidate_revision_manifest_exports_and_canonical_declaration_bytes",
                "candidate_revision":self.candidate_digest(),
                "project_revision":self.revision.project_revision(),
                "declaration_digest":expected_declaration_digest
            },
            "limitations":[
                "caller_declaration_is_not_environment_observation",
                "declaration_does_not_verify_deployed_values_artifacts_runtime_or_external_apis"
            ],
            "nonclaim":"not_evidence_that_the_declared_configuration_is_present_current_or_used"
        });

        let nonclaims = object
            .get_mut("nonclaims")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage nonclaims are absent"))?;
        for value in [
            "caller_declaration_not_environment_observation_or_deployed_state",
            "no_artifact_runtime_external_api_or_consumer_verification",
            "no_deployment_execution_freshness_drift_or_conformance_proof",
            "no_filesystem_network_secret_input_or_locator_authority",
            "no_source_approval_publication_or_deployment_authority",
        ] {
            if nonclaims.iter().any(|row| row.as_str() == Some(value)) {
                return Err(invalid(
                    "candidate deployment nonclaim inventory is duplicated",
                ));
            }
            nonclaims.push(json!(value));
        }
        object.insert(
            "deployment_contract_declaration".into(),
            json!({
                "schema":PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_SCHEMA,
                "digest":expected_declaration_digest,
                "bytes":declaration_bytes.len(),
                "canonical_json":std::str::from_utf8(declaration_bytes)
                    .map_err(|_| invalid("deployment declaration is not UTF-8"))?,
                "candidate_revision":self.candidate_digest(),
                "manifest_exports":declaration["manifest_exports"],
                "configuration":declaration["configuration"],
                "authentication":"exact_canonical_bytes_digest_candidate_and_unique_manifest_export_join",
                "source_authority":false,
                "environment_observation":false,
                "deployment_authority":false
            }),
        );
        super::super::image::render(
            coverage,
            false,
            MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_BYTES,
        )
        .map_err(|_| capacity("candidate deployment contract evidence exceeds its byte bound"))
    }
}

fn authenticate_declaration(
    candidate: &ProjectCandidate,
    bytes: &[u8],
    expected_digest: &str,
) -> Result<Value> {
    if bytes.is_empty() || bytes.len() > MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_BYTES
    {
        return Err(capacity(
            "deployment contract declaration is empty or exceeds its byte bound",
        ));
    }
    validate_digest(expected_digest)?;
    if wire::digest(DECLARATION_DOMAIN, bytes) != expected_digest {
        return Err(binding("deployment contract declaration digest disagrees"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("deployment contract declaration is not bounded valid JSON"))?;
    if wire::render(
        value.clone(),
        MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_BYTES,
    )?
    .as_bytes()
        != bytes
    {
        return Err(invalid(
            "deployment contract declaration requires exact canonical JSON bytes",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("deployment contract declaration must be an object"))?;
    require_keys(
        object,
        &[
            "schema",
            "candidate_revision",
            "manifest_exports",
            "configuration",
        ],
        "deployment contract declaration has unknown or missing fields",
    )?;
    if value["schema"] != PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_SCHEMA
        || value["candidate_revision"] != candidate.candidate_digest()
    {
        return Err(binding(
            "deployment contract declaration schema or candidate binding disagrees",
        ));
    }

    let exports = value["manifest_exports"]
        .as_array()
        .ok_or_else(|| invalid("deployment contract manifest exports must be an array"))?;
    if exports.is_empty() || exports.len() > crate::project::MAX_WEB_EXPORTS {
        return Err(capacity(
            "deployment contract manifest export inventory is empty or exceeds its bound",
        ));
    }
    let mut selected = Vec::with_capacity(exports.len());
    let mut seen = BTreeSet::new();
    for export in exports {
        let id = export
            .as_str()
            .ok_or_else(|| invalid("deployment contract manifest export must be text"))?;
        if !seen.insert(id)
            || candidate
                .revision
                .semantic
                .rename_function(id)
                .is_none_or(|function| function.origin != IdentityOrigin::Explicit)
        {
            return Err(binding(
                "deployment contract export is repeated or lacks one explicit candidate function",
            ));
        }
        selected.push(id);
    }
    if selected
        != candidate
            .revision
            .manifest()
            .web_exports()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    {
        return Err(binding(
            "deployment contract exports differ from the exact manifest inventory",
        ));
    }

    let configuration = value["configuration"]
        .as_array()
        .ok_or_else(|| invalid("deployment configuration contract must be an array"))?;
    if configuration.is_empty()
        || configuration.len() > MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEYS
    {
        return Err(capacity(
            "deployment configuration contract is empty or exceeds its key bound",
        ));
    }
    let mut previous = None;
    for row in configuration {
        let object = row
            .as_object()
            .ok_or_else(|| invalid("deployment configuration row must be an object"))?;
        require_keys(
            object,
            &["key", "type", "required"],
            "deployment configuration row has unknown or missing fields",
        )?;
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("deployment configuration key must be text"))?;
        if key.is_empty()
            || key.len() > MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEY_BYTES
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
            || previous.is_some_and(|prior: &str| prior.as_bytes() >= key.as_bytes())
        {
            return Err(invalid(
                "deployment configuration keys must be unique canonical bounded tokens",
            ));
        }
        previous = Some(key);
        if !matches!(
            object.get("type").and_then(Value::as_str),
            Some("string" | "integer" | "boolean")
        ) || object.get("required").and_then(Value::as_bool).is_none()
        {
            return Err(invalid(
                "deployment configuration row has an unsupported key shape",
            ));
        }
    }
    Ok(value)
}

fn validate_coverage(candidate: &ProjectCandidate, coverage: &Value) -> Result<()> {
    if coverage["schema"] != PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA
        || coverage["candidate_revision"] != candidate.candidate_digest()
        || coverage["base_project_revision"] != candidate.base.project_revision()
        || coverage["project_revision"] != candidate.revision.project_revision()
        || coverage["workspace_revision"] != candidate.revision.workspace_revision()
        || coverage["project_graph_digest"] != candidate.revision.semantic_graph_digest()
        || coverage["evidence_class"] != "retained_source_analysis_boundary_inventory"
        || coverage["manifest"]["web_exports"] != json!(candidate.revision.manifest().web_exports())
        || coverage["source_authority"] != false
        || coverage["external_io"] != false
        || coverage["execution"] != false
        || coverage["candidate_retained"] != false
        || coverage["publication_authority"] != false
    {
        return Err(binding(
            "candidate analysis coverage and deployment declaration bindings disagree",
        ));
    }
    Ok(())
}

fn require_keys(object: &Map<String, Value>, keys: &[&str], message: &'static str) -> Result<()> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(message));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "deployment contract declaration digest is not canonical SHA-256",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G424", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G425", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G426", message)]
}
