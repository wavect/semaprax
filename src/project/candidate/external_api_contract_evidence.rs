//! Digest-only external API contract declarations attached to one exact
//! candidate analysis boundary. Declarations carry no endpoint, provider,
//! credential, runtime observation, network authority, or conformance proof.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;
use crate::hir::IdentityOrigin;

use super::{wire, ProjectCandidate, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA: &str =
    "semaprax.project-candidate-external-api-contract-declaration.v1";
pub const PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA: &str =
    "semaprax.project-candidate-external-api-contract-evidence.v1";
pub const MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES: usize = 128 * 1024;
pub const MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;

const DECLARATION_DOMAIN: &[u8] =
    b"semaprax.project-candidate-external-api-contract-declaration.v1\0";
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
    /// Attach caller-declared operation and schema digests for exact manifest
    /// exports or a canonical nonempty subset of explicit manifest exports.
    /// The declaration is comparison data only and cannot identify or contact
    /// a provider.
    pub fn analysis_external_api_contract_evidence(
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
            json!(PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA),
        );
        object.insert(
            "evidence_class".into(),
            json!("retained_source_and_explicit_digest_only_external_api_contract_declaration"),
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
        let external_api = &mut areas[5];
        if *external_api
            != json!({
                "area":"external_api_behavior",
                "status":"not_inspected",
                "basis":"no_external_service_or_native_provider_execution",
                "limitations":[
                    "availability_versions_authentication_and_remote_side_effects_are_unknown",
                    "source_effect_declarations_do_not_inspect_remote_systems"
                ],
                "required_evidence":["explicit_external_contract_version_and_conformance_evidence"]
            })
        {
            return Err(invalid(
                "candidate external API analysis boundary is unexpected",
            ));
        }
        *external_api = json!({
            "area":"external_api_behavior",
            "status":"partial",
            "basis":"caller_supplied_digest_only_operation_and_schema_contract_bound_to_exact_candidate_exports",
            "limitations":[
                "declaration_is_not_provider_network_or_runtime_observation",
                "operation_and_schema_digests_are_declared_not_remotely_conformed",
                "no_endpoint_url_secret_locator_version_availability_authentication_or_side_effect_evidence"
            ],
            "required_evidence":[
                "independently_authenticated_provider_contract_bound_to_declared_digests",
                "authorized_runtime_conformance_evidence_bound_to_this_candidate"
            ]
        });

        let blind_spots = object
            .get_mut("blind_spots")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis blind-spot inventory is absent"))?;
        let matching = blind_spots
            .iter_mut()
            .filter(|row| row["domain"] == "external_api_and_deployed_runtime_contracts")
            .collect::<Vec<_>>();
        if matching.len() != 1
            || *matching[0]
                != json!({
                    "domain":"external_api_and_deployed_runtime_contracts",
                    "evidence_status":"absent",
                    "absent_evidence":"no_authenticated_external_provider_or_deployed_runtime_contract_evidence",
                    "source_binding":{
                        "kind":"exact_retained_project_revision_and_manifest_source_inventory",
                        "project_revision":self.revision.project_revision()
                    },
                    "nonclaim":"not_evidence_that_no_external_api_or_runtime_contract_exists"
                })
        {
            return Err(invalid(
                "candidate external API blind-spot boundary is unexpected",
            ));
        }
        *matching.into_iter().next().expect("one external API row") = json!({
            "domain":"external_api_and_deployed_runtime_contracts",
            "evidence_status":"partial",
            "basis":"authenticated_caller_digest_declaration_only",
            "source_binding":{
                "kind":"exact_candidate_revision_and_selected_explicit_manifest_exports",
                "candidate_revision":self.candidate_digest(),
                "project_revision":self.revision.project_revision(),
                "declaration_digest":expected_declaration_digest
            },
            "limitations":[
                "declaration_is_not_external_provider_or_deployed_runtime_evidence",
                "declared_digests_do_not_prove_network_behavior_or_conformance"
            ],
            "nonclaim":"not_evidence_that_the_declared_external_API_is_present_current_reachable_or_conformant"
        });

        let nonclaims = object
            .get_mut("nonclaims")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("candidate analysis coverage nonclaims are absent"))?;
        for value in [
            "external_API_contract_is_caller_declaration_not_provider_evidence",
            "no_network_provider_runtime_availability_or_conformance_evidence",
            "no_URL_secret_endpoint_locator_or_ambient_authority",
            "no_filesystem_process_publication_or_deployment_authority",
            "only_external_API_behavior_coverage_is_advanced_to_partial",
        ] {
            if nonclaims.iter().any(|row| row.as_str() == Some(value)) {
                return Err(invalid(
                    "candidate external API nonclaim inventory is duplicated",
                ));
            }
            nonclaims.push(json!(value));
        }
        object.insert(
            "external_api_contract_declaration".into(),
            json!({
                "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA,
                "digest":expected_declaration_digest,
                "bytes":declaration_bytes.len(),
                "canonical_json":std::str::from_utf8(declaration_bytes)
                    .map_err(|_| invalid("external API declaration is not UTF-8"))?,
                "candidate_revision":self.candidate_digest(),
                "scope":declaration["scope"],
                "operations":declaration["operations"],
                "authentication":"exact_canonical_bytes_digest_candidate_and_explicit_manifest_export_join",
                "network_observation":false,
                "provider_observation":false,
                "runtime_observation":false,
                "conformance_evidence":false,
                "ambient_authority":false
            }),
        );
        super::super::image::render(
            coverage,
            false,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_BYTES,
        )
        .map_err(|_| capacity("candidate external API contract evidence exceeds its byte bound"))
    }
}

fn authenticate_declaration(
    candidate: &ProjectCandidate,
    bytes: &[u8],
    expected_digest: &str,
) -> Result<Value> {
    if bytes.is_empty()
        || bytes.len() > MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES
    {
        return Err(capacity(
            "external API declaration is empty or exceeds its byte bound",
        ));
    }
    validate_digest(expected_digest)?;
    if wire::digest(DECLARATION_DOMAIN, bytes) != expected_digest {
        return Err(binding("external API declaration digest disagrees"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("external API declaration is not bounded valid JSON"))?;
    if wire::render(
        value.clone(),
        MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
    )?
    .as_bytes()
        != bytes
    {
        return Err(invalid(
            "external API declaration requires exact canonical JSON bytes",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("external API declaration must be an object"))?;
    require_keys(
        object,
        &["schema", "candidate_revision", "scope", "operations"],
        "external API declaration has unknown or missing fields",
    )?;
    if value["schema"] != PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA
        || value["candidate_revision"] != candidate.candidate_digest()
    {
        return Err(binding(
            "external API declaration schema or candidate binding disagrees",
        ));
    }
    let scope = value["scope"]
        .as_object()
        .ok_or_else(|| invalid("external API declaration scope must be an object"))?;
    require_keys(
        scope,
        &["kind"],
        "external API declaration scope has unknown or missing fields",
    )?;
    let kind = scope
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("external API declaration scope kind must be text"))?;
    if !matches!(kind, "manifest_exports" | "explicit_stable_exports") {
        return Err(invalid("external API declaration scope is unsupported"));
    }

    let operations = value["operations"]
        .as_array()
        .ok_or_else(|| invalid("external API operations must be an array"))?;
    if operations.is_empty() || operations.len() > crate::project::MAX_WEB_EXPORTS {
        return Err(capacity(
            "external API operation inventory is empty or exceeds its bound",
        ));
    }
    let manifest_exports = candidate.revision.manifest().web_exports();
    let manifest_set = manifest_exports
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut ids = Vec::with_capacity(operations.len());
    let mut previous = None;
    for row in operations {
        let object = row
            .as_object()
            .ok_or_else(|| invalid("external API operation row must be an object"))?;
        require_keys(
            object,
            &["export_id", "operation_digest", "schema_digest"],
            "external API operation row has unknown or missing fields",
        )?;
        let id = object
            .get("export_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("external API export identity must be text"))?;
        if !manifest_set.contains(id)
            || candidate
                .revision
                .semantic
                .rename_function(id)
                .is_none_or(|function| function.origin != IdentityOrigin::Explicit)
        {
            return Err(binding(
                "external API operation is not one explicit manifest export",
            ));
        }
        validate_digest(
            object
                .get("operation_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("external API operation digest must be text"))?,
        )?;
        validate_digest(
            object
                .get("schema_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("external API schema digest must be text"))?,
        )?;
        if kind == "explicit_stable_exports"
            && previous.is_some_and(|prior: &str| prior.as_bytes() >= id.as_bytes())
        {
            return Err(invalid(
                "explicit external API exports must be unique canonical identity order",
            ));
        }
        previous = Some(id);
        ids.push(id);
    }
    if kind == "manifest_exports"
        && ids
            != manifest_exports
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
    {
        return Err(binding(
            "manifest-scoped external API declaration must cover the complete exact export inventory",
        ));
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
            "candidate analysis coverage and external API declaration bindings disagree",
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
            "external API declaration digest is not canonical SHA-256",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G433", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G434", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G435", message)]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn candidate() -> ProjectCandidate {
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
        let revision = super::super::super::load_snapshot(&manifest)
            .unwrap()
            .retain_revision();
        ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap()
    }

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn declaration(candidate: &ProjectCandidate, kind: &str, ids: &[&str]) -> (Vec<u8>, String) {
        let operations = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                json!({
                    "export_id":id,
                    "operation_digest":digest(char::from(b'a' + u8::try_from(index).unwrap())),
                    "schema_digest":digest(char::from(b'f' - u8::try_from(index).unwrap())),
                })
            })
            .collect::<Vec<_>>();
        let rendered = wire::render(
            json!({
                "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA,
                "candidate_revision":candidate.candidate_digest(),
                "scope":{"kind":kind},
                "operations":operations,
            }),
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let digest = wire::digest(DECLARATION_DOMAIN, rendered.as_bytes());
        (rendered.into_bytes(), digest)
    }

    fn area<'a>(value: &'a Value, name: &str) -> &'a Value {
        value["areas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["area"] == name)
            .unwrap()
    }

    #[test]
    fn complete_manifest_declaration_marks_only_external_api_behavior_partial() {
        let candidate = candidate();
        let ids = candidate
            .revision()
            .manifest()
            .web_exports()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let (bytes, declaration_digest) = declaration(&candidate, "manifest_exports", &ids);
        let report: Value = serde_json::from_str(
            &candidate
                .analysis_external_api_contract_evidence(
                    candidate.candidate_digest(),
                    &bytes,
                    &declaration_digest,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            report["schema"],
            PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA
        );
        assert_eq!(area(&report, "external_api_behavior")["status"], "partial");
        for name in [
            "declared_source_inputs",
            "declared_external_contracts",
            "deployment_configuration",
            "generated_file_provenance",
            "generated_artifacts",
            "runtime_environment",
            "external_consumers",
        ] {
            assert_ne!(area(&report, name)["status"], "partial");
        }
        let attachment = &report["external_api_contract_declaration"];
        assert_eq!(attachment["digest"], declaration_digest);
        assert_eq!(
            attachment["operations"].as_array().unwrap().len(),
            ids.len()
        );
        for field in [
            "network_observation",
            "provider_observation",
            "runtime_observation",
            "conformance_evidence",
            "ambient_authority",
        ] {
            assert_eq!(attachment[field], false);
        }
        assert!(report["nonclaims"].as_array().unwrap().contains(&json!(
            "no_URL_secret_endpoint_locator_or_ambient_authority"
        )));
    }

    #[test]
    fn explicit_stable_manifest_subset_is_canonical_and_partial() {
        let candidate = candidate();
        let mut ids = candidate
            .revision()
            .manifest()
            .web_exports()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let (bytes, declaration_digest) =
            declaration(&candidate, "explicit_stable_exports", &ids[..1]);
        let report: Value = serde_json::from_str(
            &candidate
                .analysis_external_api_contract_evidence(
                    candidate.candidate_digest(),
                    &bytes,
                    &declaration_digest,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(area(&report, "external_api_behavior")["status"], "partial");
        assert_eq!(
            report["external_api_contract_declaration"]["operations"][0]["export_id"],
            ids[0]
        );
    }

    #[test]
    fn incomplete_unknown_or_open_declarations_fail_closed() {
        let candidate = candidate();
        let exports = candidate.revision().manifest().web_exports();
        let first = exports[0].as_str();

        let (incomplete, digest) = declaration(&candidate, "manifest_exports", &[first]);
        assert_eq!(
            candidate
                .analysis_external_api_contract_evidence(
                    candidate.candidate_digest(),
                    &incomplete,
                    &digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G435"
        );

        let (unknown, digest) =
            declaration(&candidate, "explicit_stable_exports", &["unknown.external"]);
        assert_eq!(
            candidate
                .analysis_external_api_contract_evidence(
                    candidate.candidate_digest(),
                    &unknown,
                    &digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G435"
        );

        let (bytes, _) = declaration(&candidate, "explicit_stable_exports", &[first]);
        let mut open: Value = serde_json::from_slice(&bytes).unwrap();
        open["url"] = json!("https://forbidden.invalid");
        let open = wire::render(
            open,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let digest = wire::digest(DECLARATION_DOMAIN, open.as_bytes());
        assert_eq!(
            candidate
                .analysis_external_api_contract_evidence(
                    candidate.candidate_digest(),
                    open.as_bytes(),
                    &digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G433"
        );
    }
}
