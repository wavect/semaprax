//! Descriptive comparison of exact caller-declared external API contracts.
//! The declarations carry digests only and grant no provider, network,
//! runtime, consumer, conformance, publication, or deployment authority.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;
use crate::hir::IdentityOrigin;

use super::{
    external_api_contract_evidence, wire, ProjectCandidate,
    MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA,
};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_BASE_DECLARATION_SCHEMA: &str =
    "semaprax.project-candidate-external-api-contract-base-declaration.v1";
pub const PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-external-api-contract-delta.v1";
pub const MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_BYTES: usize = 2 * 1024 * 1024;

const BASE_DECLARATION_DOMAIN: &[u8] =
    b"semaprax.project-candidate-external-api-contract-base-declaration.v1\0";

#[derive(Clone, Copy)]
struct Contract<'a> {
    operation_digest: &'a str,
    schema_digest: &'a str,
}

impl ProjectCandidate {
    /// Compare two exact digest-only declarations over the retained base and
    /// candidate manifest exports. This reports identity and digest changes;
    /// it deliberately does not assess compatibility.
    pub fn external_api_contract_delta(
        &self,
        expected_candidate: &str,
        base_declaration_bytes: &[u8],
        expected_base_declaration_digest: &str,
        candidate_declaration_bytes: &[u8],
        expected_candidate_declaration_digest: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let base = authenticate_base_declaration(
            self,
            base_declaration_bytes,
            expected_base_declaration_digest,
        )?;
        let candidate = external_api_contract_evidence::authenticate_declaration(
            self,
            candidate_declaration_bytes,
            expected_candidate_declaration_digest,
        )?;
        let before = index(&base)?;
        let after = index(&candidate)?;
        let ids = before
            .keys()
            .chain(after.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        if ids.len() > crate::project::MAX_WEB_EXPORTS {
            return Err(capacity(
                "external API contract delta export inventory exceeds its bound",
            ));
        }

        let mut added = 0usize;
        let mut removed = 0usize;
        let mut changed = 0usize;
        let mut unchanged = 0usize;
        let mut contracts = Vec::with_capacity(ids.len());
        for export_id in ids {
            let base_contract = before.get(export_id).copied();
            let candidate_contract = after.get(export_id).copied();
            let (change, changed_facets) = match (base_contract, candidate_contract) {
                (None, Some(_)) => {
                    added += 1;
                    ("added", Vec::new())
                }
                (Some(_), None) => {
                    removed += 1;
                    ("removed", Vec::new())
                }
                (Some(left), Some(right))
                    if left.operation_digest == right.operation_digest
                        && left.schema_digest == right.schema_digest =>
                {
                    unchanged += 1;
                    ("unchanged", Vec::new())
                }
                (Some(left), Some(right)) => {
                    changed += 1;
                    let mut facets = Vec::with_capacity(2);
                    if left.operation_digest != right.operation_digest {
                        facets.push("operation_digest");
                    }
                    if left.schema_digest != right.schema_digest {
                        facets.push("schema_digest");
                    }
                    ("changed", facets)
                }
                (None, None) => unreachable!("identity originates in one declaration"),
            };
            contracts.push(json!({
                "export_id":export_id,
                "change":change,
                "changed_facets":changed_facets,
                "base":contract_value(base_contract),
                "candidate":contract_value(candidate_contract),
            }));
        }

        let value = json!({
            "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_SCHEMA,
            "candidate_revision":self.candidate_digest(),
            "base_project_revision":self.base.project_revision(),
            "project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),
            "workspace_revision":self.revision.workspace_revision(),
            "base_project_graph_digest":self.base.semantic_graph_digest(),
            "project_graph_digest":self.revision.semantic_graph_digest(),
            "base_declaration":{
                "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_BASE_DECLARATION_SCHEMA,
                "digest":expected_base_declaration_digest,
                "bytes":base_declaration_bytes.len(),
                "canonical_json":std::str::from_utf8(base_declaration_bytes)
                    .map_err(|_| invalid("base external API declaration is not UTF-8"))?,
                "scope":base["scope"],
            },
            "candidate_declaration":{
                "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA,
                "digest":expected_candidate_declaration_digest,
                "bytes":candidate_declaration_bytes.len(),
                "canonical_json":std::str::from_utf8(candidate_declaration_bytes)
                    .map_err(|_| invalid("candidate external API declaration is not UTF-8"))?,
                "scope":candidate["scope"],
            },
            "contracts":contracts,
            "inventory":{
                "base":before.len(),
                "candidate":after.len(),
                "added":added,
                "removed":removed,
                "changed":changed,
                "unchanged":unchanged,
            },
            "compatibility":"not_assessed",
            "comparison_scope":"caller_declared_export_identity_operation_digest_and_schema_digest_inventory_only",
            "evidence_class":"exact_base_and_candidate_digest_only_external_api_contract_declaration_delta",
            "provider_observation":false,
            "network_observation":false,
            "runtime_observation":false,
            "version_evidence":false,
            "conformance_evidence":false,
            "consumer_evidence":false,
            "source_authority":false,
            "filesystem_authority":false,
            "process_authority":false,
            "network_authority":false,
            "ambient_authority":false,
            "publication_authority":false,
            "deployment_authority":false,
            "nonclaims":[
                "not_a_compatibility_assessment",
                "added_or_removed_means_declared_contract_inventory_only_not_runtime_API_compatibility",
                "not_provider_network_runtime_version_or_conformance_evidence",
                "not_external_consumer_discovery_usage_or_migration_evidence",
                "no_endpoint_URL_secret_locator_or_ambient_authority",
                "no_source_filesystem_process_network_publication_or_deployment_authority"
            ]
        });
        super::super::image::render(
            value,
            false,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_BYTES,
        )
        .map_err(|_| capacity("external API contract delta exceeds its byte bound"))
    }
}

fn authenticate_base_declaration(
    candidate: &ProjectCandidate,
    bytes: &[u8],
    expected_digest: &str,
) -> Result<Value> {
    if bytes.is_empty()
        || bytes.len() > MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES
    {
        return Err(capacity(
            "base external API declaration is empty or exceeds its byte bound",
        ));
    }
    validate_digest(expected_digest)?;
    if wire::digest(BASE_DECLARATION_DOMAIN, bytes) != expected_digest {
        return Err(binding("base external API declaration digest disagrees"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("base external API declaration is not bounded valid JSON"))?;
    if wire::render(
        value.clone(),
        MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
    )?
    .as_bytes()
        != bytes
    {
        return Err(invalid(
            "base external API declaration requires exact canonical JSON bytes",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("base external API declaration must be an object"))?;
    require_keys(
        object,
        &["schema", "base_project_revision", "scope", "operations"],
        "base external API declaration has unknown or missing fields",
    )?;
    if value["schema"] != PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_BASE_DECLARATION_SCHEMA
        || value["base_project_revision"] != candidate.base.project_revision()
    {
        return Err(binding(
            "base external API declaration schema or Project binding disagrees",
        ));
    }
    authenticate_operations(
        &value,
        candidate.base.manifest().web_exports(),
        &candidate.base.semantic,
        "base",
    )?;
    Ok(value)
}

fn authenticate_operations(
    declaration: &Value,
    manifest_exports: &[String],
    semantic: &super::super::semantic::ProjectSemanticState,
    subject: &'static str,
) -> Result<()> {
    let scope = declaration["scope"]
        .as_object()
        .ok_or_else(|| invalid("external API declaration scope must be an object"))?;
    require_keys(
        scope,
        &["kind"],
        "external API declaration scope has unknown or missing fields",
    )?;
    let kind = scope["kind"]
        .as_str()
        .ok_or_else(|| invalid("external API declaration scope kind must be text"))?;
    if !matches!(kind, "manifest_exports" | "explicit_stable_exports") {
        return Err(invalid("external API declaration scope is unsupported"));
    }
    let operations = declaration["operations"]
        .as_array()
        .ok_or_else(|| invalid("external API operations must be an array"))?;
    if operations.is_empty() || operations.len() > crate::project::MAX_WEB_EXPORTS {
        return Err(capacity(
            "external API operation inventory is empty or exceeds its bound",
        ));
    }
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
        let export_id = object["export_id"]
            .as_str()
            .ok_or_else(|| invalid("external API export identity must be text"))?;
        if !manifest_set.contains(export_id)
            || semantic
                .rename_function(export_id)
                .is_none_or(|function| function.origin != IdentityOrigin::Explicit)
        {
            return Err(binding(match subject {
                "base" => "base external API operation is not one explicit manifest export",
                _ => "external API operation is not one explicit manifest export",
            }));
        }
        validate_digest(
            object["operation_digest"]
                .as_str()
                .ok_or_else(|| invalid("external API operation digest must be text"))?,
        )?;
        validate_digest(
            object["schema_digest"]
                .as_str()
                .ok_or_else(|| invalid("external API schema digest must be text"))?,
        )?;
        if kind == "explicit_stable_exports"
            && previous.is_some_and(|prior: &str| prior.as_bytes() >= export_id.as_bytes())
        {
            return Err(invalid(
                "explicit external API exports must be unique canonical identity order",
            ));
        }
        previous = Some(export_id);
        ids.push(export_id);
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
    Ok(())
}

fn index(declaration: &Value) -> Result<BTreeMap<&str, Contract<'_>>> {
    declaration["operations"]
        .as_array()
        .ok_or_else(|| invalid("authenticated external API operations are absent"))?
        .iter()
        .map(|row| {
            let export_id = row["export_id"]
                .as_str()
                .ok_or_else(|| invalid("authenticated external API export identity is absent"))?;
            let operation_digest = row["operation_digest"]
                .as_str()
                .ok_or_else(|| invalid("authenticated external API operation digest is absent"))?;
            let schema_digest = row["schema_digest"]
                .as_str()
                .ok_or_else(|| invalid("authenticated external API schema digest is absent"))?;
            Ok((
                export_id,
                Contract {
                    operation_digest,
                    schema_digest,
                },
            ))
        })
        .collect()
}

fn contract_value(contract: Option<Contract<'_>>) -> Value {
    contract.map_or(Value::Null, |contract| {
        json!({
            "operation_digest":contract.operation_digest,
            "schema_digest":contract.schema_digest,
        })
    })
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
            "external API contract delta digest is not canonical SHA-256",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G446", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G447", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G448", message)]
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

    fn declarations(candidate: &ProjectCandidate) -> ((Vec<u8>, String), (Vec<u8>, String)) {
        let exports = candidate.base.manifest().web_exports();
        let operations = exports
            .iter()
            .enumerate()
            .map(|(index, export_id)| {
                json!({
                    "export_id":export_id,
                    "operation_digest":digest(char::from(b'a' + u8::try_from(index).unwrap())),
                    "schema_digest":digest(char::from(b'f' - u8::try_from(index).unwrap())),
                })
            })
            .collect::<Vec<_>>();
        let base = wire::render(
            json!({
                "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_BASE_DECLARATION_SCHEMA,
                "base_project_revision":candidate.base.project_revision(),
                "scope":{"kind":"manifest_exports"},
                "operations":operations,
            }),
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let candidate_declaration = wire::render(
            json!({
                "schema":PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA,
                "candidate_revision":candidate.candidate_digest(),
                "scope":{"kind":"manifest_exports"},
                "operations":operations,
            }),
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let base_digest = wire::digest(BASE_DECLARATION_DOMAIN, base.as_bytes());
        let candidate_digest = wire::digest(
            b"semaprax.project-candidate-external-api-contract-declaration.v1\0",
            candidate_declaration.as_bytes(),
        );
        (
            (base.into_bytes(), base_digest),
            (candidate_declaration.into_bytes(), candidate_digest),
        )
    }

    #[test]
    fn exact_declarations_report_unchanged_without_compatibility_claim() {
        let candidate = candidate();
        let ((base, base_digest), (after, after_digest)) = declarations(&candidate);
        let report: Value = serde_json::from_str(
            &candidate
                .external_api_contract_delta(
                    candidate.candidate_digest(),
                    &base,
                    &base_digest,
                    &after,
                    &after_digest,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["compatibility"], "not_assessed");
        assert_eq!(report["inventory"]["changed"], 0);
        assert_eq!(
            report["inventory"]["unchanged"],
            candidate.base.manifest().web_exports().len()
        );
        for field in [
            "provider_observation",
            "network_observation",
            "runtime_observation",
            "version_evidence",
            "conformance_evidence",
            "consumer_evidence",
            "ambient_authority",
            "publication_authority",
            "deployment_authority",
        ] {
            assert_eq!(report[field], false);
        }
    }

    #[test]
    fn changed_digest_and_tampered_base_binding_are_descriptive_or_rejected() {
        let candidate = candidate();
        let ((base, base_digest), (after, _)) = declarations(&candidate);
        let mut changed: Value = serde_json::from_slice(&after).unwrap();
        changed["operations"][0]["schema_digest"] = json!(digest('9'));
        let changed = wire::render(
            changed,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let changed_digest = wire::digest(
            b"semaprax.project-candidate-external-api-contract-declaration.v1\0",
            changed.as_bytes(),
        );
        let report: Value = serde_json::from_str(
            &candidate
                .external_api_contract_delta(
                    candidate.candidate_digest(),
                    &base,
                    &base_digest,
                    changed.as_bytes(),
                    &changed_digest,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["inventory"]["changed"], 1);
        assert_eq!(
            report["contracts"][0]["changed_facets"],
            json!(["schema_digest"])
        );

        let mut stale: Value = serde_json::from_slice(&base).unwrap();
        stale["base_project_revision"] = json!(digest('0'));
        let stale = wire::render(
            stale,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let stale_digest = wire::digest(BASE_DECLARATION_DOMAIN, stale.as_bytes());
        assert_eq!(
            candidate
                .external_api_contract_delta(
                    candidate.candidate_digest(),
                    stale.as_bytes(),
                    &stale_digest,
                    changed.as_bytes(),
                    &changed_digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G448"
        );

        let mut open: Value = serde_json::from_slice(&base).unwrap();
        open["url"] = json!("https://forbidden.invalid");
        let open = wire::render(
            open,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let open_digest = wire::digest(BASE_DECLARATION_DOMAIN, open.as_bytes());
        assert_eq!(
            candidate
                .external_api_contract_delta(
                    candidate.candidate_digest(),
                    open.as_bytes(),
                    &open_digest,
                    changed.as_bytes(),
                    &changed_digest,
                )
                .unwrap_err()[0]
                .code,
            "SPX-G446"
        );
    }

    #[test]
    fn explicit_subsets_report_added_removed_and_unchanged_in_identity_order() {
        let candidate = candidate();
        let ((base, _), (after, _)) = declarations(&candidate);
        let mut base: Value = serde_json::from_slice(&base).unwrap();
        let mut after: Value = serde_json::from_slice(&after).unwrap();
        let operations = base["operations"].as_array().unwrap().clone();
        base["scope"]["kind"] = json!("explicit_stable_exports");
        base["operations"] = json!([operations[0].clone(), operations[1].clone()]);
        let operations = after["operations"].as_array().unwrap().clone();
        after["scope"]["kind"] = json!("explicit_stable_exports");
        after["operations"] = json!([operations[1].clone(), operations[2].clone()]);
        let base = wire::render(
            base,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let after = wire::render(
            after,
            MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
        )
        .unwrap();
        let base_digest = wire::digest(BASE_DECLARATION_DOMAIN, base.as_bytes());
        let after_digest = wire::digest(
            b"semaprax.project-candidate-external-api-contract-declaration.v1\0",
            after.as_bytes(),
        );
        let report: Value = serde_json::from_str(
            &candidate
                .external_api_contract_delta(
                    candidate.candidate_digest(),
                    base.as_bytes(),
                    &base_digest,
                    after.as_bytes(),
                    &after_digest,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["inventory"]["added"], 1);
        assert_eq!(report["inventory"]["removed"], 1);
        assert_eq!(report["inventory"]["unchanged"], 1);
        let rows = report["contracts"].as_array().unwrap();
        assert_eq!(rows[0]["change"], "removed");
        assert_eq!(rows[1]["change"], "unchanged");
        assert_eq!(rows[2]["change"], "added");
        assert!(rows
            .windows(2)
            .all(|pair| pair[0]["export_id"].as_str().unwrap().as_bytes()
                < pair[1]["export_id"].as_str().unwrap().as_bytes()));
    }
}
