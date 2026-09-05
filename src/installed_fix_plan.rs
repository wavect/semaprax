//! Installed and exact-current-source Fix Plan v1 projections.
//!
//! The only admitted plan reuses Diagnostic Repair v1 discovery for SPX-S103.
//! Planning never instantiates a patch, writes source, or grants authority.

use std::path::Path;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::repair::{self, DiagnosticRepairQuery};

pub const INSTALLED_FIX_PLAN_CATALOG_SCHEMA: &str = "semaprax.installed-fix-plan-catalog.v1";
pub const CURRENT_SOURCE_FIX_PLAN_SCHEMA: &str = "semaprax.current-source-fix-plan.v1";
pub const MAX_INSTALLED_FIX_PLAN_CATALOG_BYTES: usize = 1024 * 1024;
pub const MAX_CURRENT_SOURCE_FIX_PLAN_BYTES: usize = 64 * 1024 * 1024;

const CATALOG_DOMAIN: &[u8] = b"semaprax.installed-fix-plan-catalog.payload.digest.v1\0";
const PLAN_DOMAIN: &[u8] = b"semaprax.current-source-fix-plan.payload.digest.v1\0";
const REPAIR_REPORT_DOMAIN: &[u8] = b"semaprax.current-source-fix-plan.repair-report.digest.v1\0";
const DIAGNOSTIC_REPAIR_SCHEMA: &str = "semaprax.diagnostic-repair.v1";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// The closed request inventory for current-source Fix Plan v1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixPlanRequest {
    target: String,
}

impl FixPlanRequest {
    pub fn assign_function_id(automatic_function_id: impl Into<String>) -> Result<Self> {
        let target = automatic_function_id.into();
        DiagnosticRepairQuery::assign_function_id(target.clone()).map_err(|error| vec![error])?;
        Ok(Self { target })
    }

    pub fn kind(&self) -> &'static str {
        "assign_function_id"
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    fn repair_query(&self) -> Result<DiagnosticRepairQuery> {
        DiagnosticRepairQuery::assign_function_id(self.target.clone()).map_err(|error| vec![error])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledFixPlanCatalog {
    json: String,
    digest: String,
}

impl InstalledFixPlanCatalog {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixPlan {
    json: String,
    digest: String,
    repair_report: String,
    repair_report_digest: String,
}

impl FixPlan {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn repair_report(&self) -> &str {
        &self.repair_report
    }
    pub fn repair_report_digest(&self) -> &str {
        &self.repair_report_digest
    }

    /// Re-run held-source discovery and exact-compare the complete plan.
    pub fn replay_current_source(
        source_path: &Path,
        request: &FixPlanRequest,
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        if bytes.len() > MAX_CURRENT_SOURCE_FIX_PLAN_BYTES {
            return Err(capacity("current-source fix plan exceeds its byte limit"));
        }
        validate_digest(expected_digest)?;
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("current-source fix plan is not valid JSON"))?;
        if value["schema"] != CURRENT_SOURCE_FIX_PLAN_SCHEMA
            || canonical(&value)?.as_bytes() != bytes
        {
            return Err(invalid("current-source fix plan is not canonical"));
        }
        if hash(PLAN_DOMAIN, payload_bytes(&value)?.as_bytes()) != expected_digest {
            return Err(stale("current-source fix plan digest is stale"));
        }
        let derived = current_source_fix_plan(source_path, request)?;
        if derived.digest() != expected_digest || derived.to_json().as_bytes() != bytes {
            return Err(stale("current-source fix plan failed exact replay"));
        }
        Ok(derived)
    }
}

/// Exact installed inventory. V1 advertises only the already specified
/// SPX-S103 identity-rebase discovery and its explicit instantiation input.
pub fn installed_fix_plan_catalog() -> Result<InstalledFixPlanCatalog> {
    let payload = json!({
        "authority": false,
        "compiler": compiler()?,
        "operations": [{
            "classification": "breaking_identity_rebase",
            "diagnostic": "SPX-S103",
            "kind": "assign_function_id",
            "plan_availability": "requires_exact_current_source_and_automatic_function_id",
            "required_instantiation_input": {
                "name": "persistent_id",
                "required": true,
                "type": "persistent_declaration_id",
            },
            "source_report_schema": DIAGNOSTIC_REPAIR_SCHEMA,
        }],
        "limits": {"max_document_bytes":MAX_INSTALLED_FIX_PLAN_CATALOG_BYTES},
        "nonclaims": [
            "not_general_diagnostic_repair",
            "no_automatic_repair_selection_or_ranking",
            "catalog_does_not_claim_a_current_source_is_eligible",
            "planning_does_not_instantiate_or_apply_a_patch",
            "no_source_filesystem_process_network_or_publication_authority",
            "no_host_capability_grant",
        ],
    });
    let (json, digest) = document(
        INSTALLED_FIX_PLAN_CATALOG_SCHEMA,
        CATALOG_DOMAIN,
        payload,
        MAX_INSTALLED_FIX_PLAN_CATALOG_BYTES,
    )?;
    Ok(InstalledFixPlanCatalog { json, digest })
}

/// Discover one plan from an explicitly selected, currently authenticated
/// source. Diagnostic Repair performs the held-source read and final drift
/// check. This wrapper never calls its instantiation route.
pub fn current_source_fix_plan(source_path: &Path, request: &FixPlanRequest) -> Result<FixPlan> {
    let catalog = installed_fix_plan_catalog()?;
    let repair_report = repair::query(source_path, &request.repair_query()?)?;
    if repair_report.len() > MAX_CURRENT_SOURCE_FIX_PLAN_BYTES {
        return Err(capacity(
            "diagnostic repair report exceeds the fix-plan byte limit",
        ));
    }
    let repair_value: Value = serde_json::from_str(&repair_report)
        .map_err(|_| invalid("diagnostic repair report is not valid JSON"))?;
    if repair_value["schema"] != DIAGNOSTIC_REPAIR_SCHEMA {
        return Err(invalid("diagnostic repair report has an unexpected schema"));
    }
    if repair_value["query"]["kind"] != request.kind()
        || repair_value["query"]["target"] != request.target()
        || repair_value["diagnostic"]["code"] != "SPX-S103"
    {
        return Err(unavailable(
            "diagnostic repair report does not match the requested fix plan",
        ));
    }
    let repair_report_digest = hash(REPAIR_REPORT_DOMAIN, repair_report.as_bytes());
    let catalog_value: Value = serde_json::from_str(catalog.to_json())
        .map_err(|_| invalid("installed fix-plan catalog is not valid JSON"))?;
    let payload = json!({
        "authority": false,
        "catalog": {"digest":catalog.digest(),"value":catalog_value},
        "compiler": compiler()?,
        "diagnostic": "SPX-S103",
        "plan": {
            "classification": "breaking_identity_rebase",
            "kind": request.kind(),
            "required_instantiation_input": "persistent_id",
            "status": "repair_available_requires_explicit_instantiation_input",
            "target": request.target(),
        },
        "repair_discovery": {
            "digest": repair_report_digest,
            "value": repair_value,
        },
        "limits": {"max_document_bytes":MAX_CURRENT_SOURCE_FIX_PLAN_BYTES},
        "nonclaims": [
            "not_general_diagnostic_repair",
            "not_a_patch_or_repair_instantiation",
            "not_a_claim_that_any_persistent_id_will_validate",
            "no_automatic_repair_selection_or_ranking",
            "no_source_write_commit_or_publication_authority",
            "no_process_network_secret_key_or_host_capability_grant",
        ],
        "source_binding": {
            "base_revision": repair_value["base_revision"],
            "source_digest": repair_value["source"]["digest"],
            "validation": "diagnostic_repair_held_source_final_recheck",
        },
    });
    let (json, digest) = document(
        CURRENT_SOURCE_FIX_PLAN_SCHEMA,
        PLAN_DOMAIN,
        payload,
        MAX_CURRENT_SOURCE_FIX_PLAN_BYTES,
    )?;
    Ok(FixPlan {
        json,
        digest,
        repair_report,
        repair_report_digest,
    })
}

fn compiler() -> Result<Value> {
    Ok(json!({
        "binary_identity_claimed": false,
        "build_commit": validated_commit(option_env!("SEMAPRAX_BUILD_COMMIT"))?,
        "package": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

fn validated_commit(commit: Option<&str>) -> Result<Option<&str>> {
    match commit {
        None => Ok(None),
        Some(value)
            if value.len() == 40
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            Ok(Some(value))
        }
        Some(_) => Err(invalid(
            "installed fix-plan build commit is not 40 lowercase hexadecimal characters",
        )),
    }
}

fn document(
    schema: &'static str,
    domain: &[u8],
    payload: Value,
    limit: usize,
) -> Result<(String, String)> {
    let payload = canonical(&payload)?;
    let digest = hash(domain, payload.as_bytes());
    let payload: Value = serde_json::from_str(&payload)
        .map_err(|_| invalid("fix-plan payload is not valid JSON"))?;
    let json = canonical(&json!({"digest":digest,"payload":payload,"schema":schema}))?;
    if json.len() > limit {
        return Err(capacity("fix-plan document exceeds its byte limit"));
    }
    Ok((json, digest))
}

fn payload_bytes(document: &Value) -> Result<String> {
    document
        .get("payload")
        .ok_or_else(|| invalid("fix-plan document lacks a payload"))
        .and_then(canonical)
}

fn canonical(value: &Value) -> Result<String> {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
                let mut result = Map::new();
                for key in keys {
                    result.insert(key.clone(), sorted(&object[key]));
                }
                Value::Object(result)
            }
            other => other.clone(),
        }
    }
    let mut output = serde_json::to_string(&sorted(value))
        .map_err(|_| invalid("fix-plan document cannot be serialized"))?;
    output.push('\n');
    Ok(output)
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid("fix-plan digest is not canonical"));
    }
    Ok(())
}

fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G544", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G545", message)]
}
fn unavailable(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G546", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G547", message)]
}
