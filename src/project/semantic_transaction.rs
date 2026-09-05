//! Bounded, authority-free Universal Semantic Transaction v1 kernel.
//!
//! V1 admits exactly one display-name rename over one immutable canonical
//! semantic workspace revision. Validation derives a Project candidate and
//! deterministic evidence; it never writes source or grants publication authority.

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{ProjectCandidate, ProjectRevision, SemanticChange, SEMANTIC_CHANGE_REQUIREMENTS};

pub const SEMANTIC_TRANSACTION_SCHEMA: &str = "semaprax.semantic-transaction.v1";
pub const SEMANTIC_TRANSACTION_IMPACT_SCHEMA: &str = "semaprax.semantic-transaction-impact.v1";
pub const SEMANTIC_TRANSACTION_REVIEW_SCHEMA: &str = "semaprax.semantic-transaction-review.v1";
pub const SEMANTIC_TRANSACTION_RESULT_SCHEMA: &str = "semaprax.semantic-transaction-result.v1";
pub const SEMANTIC_TRANSACTION_EVIDENCE_SCHEMA: &str = "semaprax.semantic-transaction-evidence.v1";
pub const MAX_SEMANTIC_TRANSACTION_BYTES: usize = 1024 * 1024;
pub const MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES: usize = 96 * 1024 * 1024;

const INTENT_DOMAIN: &[u8] = b"semaprax.semantic-transaction.intent.digest.v1\0";
const IMPACT_DOMAIN: &[u8] = b"semaprax.semantic-transaction.impact.digest.v1\0";
const REVIEW_DOMAIN: &[u8] = b"semaprax.semantic-transaction.review.digest.v1\0";
const RESULT_DOMAIN: &[u8] = b"semaprax.semantic-transaction.result.digest.v1\0";
const VALIDATION: &[&str] = &[
    "canonical_source_round_trip",
    "complete_project_admission",
    "ownership_and_cleanup",
    "native_and_wasm_emission",
    "canonical_workspace_revision",
];

/// The first typed operation in the universal transaction envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransactionRenameDisplayName {
    target: String,
    expected_old_value: String,
    new_value: String,
}

impl SemanticTransactionRenameDisplayName {
    pub fn new(
        target: impl Into<String>,
        expected_old_value: impl Into<String>,
        new_value: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            expected_old_value: expected_old_value.into(),
            new_value: new_value.into(),
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn expected_old_value(&self) -> &str {
        &self.expected_old_value
    }
    pub fn new_value(&self) -> &str {
        &self.new_value
    }

    fn value(&self) -> Value {
        json!({
            "expected_old_value": self.expected_old_value,
            "kind": "rename_display_name",
            "new_value": self.new_value,
            "target": self.target,
        })
    }
}

/// Canonical intent bound to one exact composite semantic workspace revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransaction {
    expected_workspace_revision: String,
    operation: SemanticTransactionRenameDisplayName,
    json: String,
    digest: String,
}

impl SemanticTransaction {
    pub fn rename_display_name(
        expected_workspace_revision: &str,
        operation: SemanticTransactionRenameDisplayName,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_workspace_revision)?;
        validate_text(&operation.target, 4096, "transaction target is not bounded")?;
        validate_text(
            &operation.expected_old_value,
            128,
            "expected display name is not bounded",
        )?;
        validate_text(&operation.new_value, 128, "new display name is not bounded")?;
        let json = render(
            json!({
                "expected_workspace_revision": expected_workspace_revision,
                "invariants": SEMANTIC_CHANGE_REQUIREMENTS,
                "operations": [operation.value()],
                "requested_authority": "none",
                "requested_validation": VALIDATION,
                "schema": SEMANTIC_TRANSACTION_SCHEMA,
            }),
            MAX_SEMANTIC_TRANSACTION_BYTES,
        )?;
        let digest = digest(INTENT_DOMAIN, json.as_bytes());
        Ok(Self {
            expected_workspace_revision: expected_workspace_revision.to_owned(),
            operation,
            json,
            digest,
        })
    }

    /// Admit only the exact canonical closed v1 envelope.
    pub fn from_json(bytes: &[u8]) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_SEMANTIC_TRANSACTION_BYTES {
            return Err(capacity("semantic transaction exceeds its byte limit"));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("semantic transaction is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("semantic transaction is not an object"))?;
        let keys = [
            "expected_workspace_revision",
            "invariants",
            "operations",
            "requested_authority",
            "requested_validation",
            "schema",
        ];
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(invalid("semantic transaction has an invalid field set"));
        }
        let operations = value["operations"]
            .as_array()
            .filter(|items| items.len() == 1)
            .ok_or_else(|| invalid("semantic transaction v1 requires exactly one operation"))?;
        let operation = operations[0]
            .as_object()
            .ok_or_else(|| invalid("semantic transaction operation is not an object"))?;
        let operation_keys = ["expected_old_value", "kind", "new_value", "target"];
        if operation.len() != operation_keys.len()
            || operation_keys
                .iter()
                .any(|key| !operation.contains_key(*key))
            || operations[0]["kind"] != "rename_display_name"
        {
            return Err(invalid(
                "semantic transaction operation is not RenameDisplayName v1",
            ));
        }
        let text = |key: &str| {
            operations[0][key]
                .as_str()
                .ok_or_else(|| invalid("semantic transaction operation text is invalid"))
        };
        let transaction = Self::rename_display_name(
            value["expected_workspace_revision"]
                .as_str()
                .ok_or_else(|| invalid("semantic transaction expected revision is invalid"))?,
            SemanticTransactionRenameDisplayName::new(
                text("target")?,
                text("expected_old_value")?,
                text("new_value")?,
            ),
        )?;
        if transaction.json.as_bytes() != bytes {
            return Err(invalid(
                "semantic transaction is not the exact canonical v1 envelope",
            ));
        }
        Ok(transaction)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn intent(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn expected_workspace_revision(&self) -> &str {
        &self.expected_workspace_revision
    }
    pub fn operation(&self) -> &SemanticTransactionRenameDisplayName {
        &self.operation
    }

    /// Validate from immutable compiler state and mint deterministic evidence.
    pub fn validate(
        &self,
        base: Arc<ProjectRevision>,
    ) -> Result<SemanticTransactionArtifacts, Vec<Diagnostic>> {
        let base_workspace = base.canonical_workspace_revision()?;
        if base_workspace.workspace_revision() != self.expected_workspace_revision {
            return Err(stale(
                "semantic transaction expected workspace revision is stale",
            ));
        }
        require_canonical_comment_free_sources(&base)?;
        require_function_preconditions(&base, &self.operation)?;

        let initial = ProjectCandidate::open(Arc::clone(&base), base.project_revision())?;
        let change = SemanticChange::new(
            base.project_revision(),
            &json!({
                "kind": "rename_declaration",
                "name": self.operation.new_value,
                "target": self.operation.target,
            }),
        )?;
        let candidate = initial.apply(initial.candidate_digest(), &change)?;
        require_canonical_comment_free_sources(candidate.revision())?;
        let candidate_workspace = candidate.revision().canonical_workspace_revision()?;
        let source_review_text = candidate.source_review(candidate.candidate_digest())?;
        let source_review: Value = serde_json::from_str(&source_review_text)
            .map_err(|_| invalid("candidate source review is not valid JSON"))?;
        let before = base
            .semantic
            .image_symbol(&self.operation.target)
            .ok_or_else(|| stale("transaction target disappeared from the base revision"))?;
        let after = candidate
            .revision()
            .semantic
            .image_symbol(&self.operation.target)
            .ok_or_else(|| stale("transaction target disappeared from the candidate revision"))?;
        if before.get("name").and_then(Value::as_str)
            != Some(self.operation.expected_old_value.as_str())
            || after.get("name").and_then(Value::as_str) != Some(self.operation.new_value.as_str())
        {
            return Err(stale(
                "RenameDisplayName result does not match its exact name preconditions",
            ));
        }
        if base_workspace.manifest_digest() != candidate_workspace.manifest_digest()
            || base_workspace.dependency_lock_digest()
                != candidate_workspace.dependency_lock_digest()
            || base_workspace.authority_policies() != candidate_workspace.authority_policies()
            || base_workspace.target_profiles() != candidate_workspace.target_profiles()
        {
            return Err(stale(
                "RenameDisplayName changed manifest, dependency, authority, or target facts",
            ));
        }

        let impact = render(
            json!({
                "base_workspace_revision": base_workspace.workspace_revision(),
                "candidate_workspace_revision": candidate_workspace.workspace_revision(),
                "classification": "descriptive_compiler_projection",
                "display_name": {"before": before["name"], "after": after["name"]},
                "identity": {"preserved": true, "target": self.operation.target},
                "nonclaims": ["not_behavioral_equivalence", "not_runtime_execution", "no_authority"],
                "schema": SEMANTIC_TRANSACTION_IMPACT_SCHEMA,
                "transaction_digest": self.digest,
            }),
            MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES,
        )?;
        let impact_digest = digest(IMPACT_DOMAIN, impact.as_bytes());
        let review = render(
            json!({
                "authority": {"granted": false, "requested": "none"},
                "impact_digest": impact_digest,
                "review": {
                    "candidate_rebuilt_from_canonical_source": true,
                    "exact_old_value_precondition": true,
                    "stable_identity_preserved": true,
                    "trivia_preservation": "comment_free_canonical_base_and_candidate",
                },
                "schema": SEMANTIC_TRANSACTION_REVIEW_SCHEMA,
                "transaction_digest": self.digest,
            }),
            MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES,
        )?;
        let review_digest = digest(REVIEW_DOMAIN, review.as_bytes());
        let candidate_value: Value = serde_json::from_str(candidate.to_json())
            .map_err(|_| invalid("candidate evidence is not valid JSON"))?;
        let result = render(
            json!({
                "authority": {"commit_performed": false, "granted": false},
                "base": {
                    "project_revision": base.project_revision(),
                    "workspace_revision": base_workspace.workspace_revision(),
                },
                "candidate": {
                    "evidence": candidate_value,
                    "project_revision": candidate.revision().project_revision(),
                    "revision": candidate.candidate_digest(),
                    "workspace_revision": candidate_workspace.workspace_revision(),
                },
                "operation_results": [{
                    "kind": "rename_display_name", "outcome": "validated",
                    "target": self.operation.target, "old_value": self.operation.expected_old_value,
                    "new_value": self.operation.new_value,
                }],
                "schema": SEMANTIC_TRANSACTION_RESULT_SCHEMA,
                "source_review": source_review,
                "transaction_digest": self.digest,
                "validation": VALIDATION.iter().map(|name| ((*name).to_owned(), Value::Bool(true))).collect::<serde_json::Map<String, Value>>(),
            }),
            MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES,
        )?;
        let result_digest = digest(RESULT_DOMAIN, result.as_bytes());
        let evidence = render(
            json!({
                "artifacts": {
                    "impact": {"digest": impact_digest, "value": parse_value(&impact)?},
                    "intent": {"digest": self.digest, "value": parse_value(&self.json)?},
                    "result": {"digest": result_digest, "value": parse_value(&result)?},
                    "review": {"digest": review_digest, "value": parse_value(&review)?},
                },
                "authority": false,
                "schema": SEMANTIC_TRANSACTION_EVIDENCE_SCHEMA,
            }),
            MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES,
        )?;

        Ok(SemanticTransactionArtifacts {
            candidate,
            impact,
            impact_digest,
            review,
            review_digest,
            result,
            result_digest,
            evidence,
        })
    }

    /// Freshly rederive the transaction and exact-compare its complete evidence.
    pub fn replay(
        base: Arc<ProjectRevision>,
        transaction_bytes: &[u8],
        evidence_bytes: &[u8],
    ) -> Result<SemanticTransactionArtifacts, Vec<Diagnostic>> {
        if evidence_bytes.len() > MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES {
            return Err(capacity(
                "semantic transaction evidence exceeds its byte limit",
            ));
        }
        let transaction = Self::from_json(transaction_bytes)?;
        let artifacts = transaction.validate(base)?;
        if artifacts.evidence.as_bytes() != evidence_bytes {
            return Err(stale("semantic transaction evidence failed exact replay"));
        }
        Ok(artifacts)
    }
}

/// Authority-free products of one fully validated transaction.
pub struct SemanticTransactionArtifacts {
    candidate: ProjectCandidate,
    impact: String,
    impact_digest: String,
    review: String,
    review_digest: String,
    result: String,
    result_digest: String,
    evidence: String,
}

impl SemanticTransactionArtifacts {
    pub fn candidate(&self) -> &ProjectCandidate {
        &self.candidate
    }
    pub fn impact(&self) -> &str {
        &self.impact
    }
    pub fn impact_digest(&self) -> &str {
        &self.impact_digest
    }
    pub fn review(&self) -> &str {
        &self.review
    }
    pub fn review_digest(&self) -> &str {
        &self.review_digest
    }
    pub fn result(&self) -> &str {
        &self.result
    }
    pub fn result_digest(&self) -> &str {
        &self.result_digest
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
}

fn require_function_preconditions(
    revision: &ProjectRevision,
    operation: &SemanticTransactionRenameDisplayName,
) -> Result<(), Vec<Diagnostic>> {
    let eligibility = rename_display_name_eligibility(revision, &operation.target)?;
    let Some(old_value) = eligibility.expected_old_value.as_deref() else {
        return Err(invalid(
            "RenameDisplayName v1 requires an explicit function target",
        ));
    };
    if old_value != operation.expected_old_value {
        return Err(stale(
            "RenameDisplayName expected old value does not match the base",
        ));
    }
    if !eligibility.explicit_identity || !eligibility.monomorphic || !eligibility.non_main {
        return Err(invalid(
            "RenameDisplayName v1 requires an explicit monomorphic non-main function",
        ));
    }
    if !eligibility.unique_function || operation.expected_old_value == operation.new_value {
        return Err(invalid(
            "RenameDisplayName v1 requires one target and a changed display name",
        ));
    }
    Ok(())
}

/// Shared, read-only classifier used by transaction validation and installed
/// operation discovery. It advertises eligibility, never a particular new name.
pub(super) struct RenameDisplayNameEligibility {
    pub(super) expected_old_value: Option<String>,
    pub(super) comment_free_canonical_workspace: bool,
    pub(super) explicit_identity: bool,
    pub(super) monomorphic: bool,
    pub(super) non_main: bool,
    unique_function: bool,
}

impl RenameDisplayNameEligibility {
    pub(super) fn available(&self) -> bool {
        self.unique_function
            && self.comment_free_canonical_workspace
            && self.explicit_identity
            && self.monomorphic
            && self.non_main
    }
}

pub(super) fn rename_display_name_eligibility(
    revision: &ProjectRevision,
    target: &str,
) -> Result<RenameDisplayNameEligibility, Vec<Diagnostic>> {
    let expected_old_value = revision
        .semantic
        .rename_function(target)
        .map(|function| function.name.clone());
    let mut matches = 0usize;
    let mut explicit_identity = false;
    let mut monomorphic = false;
    let mut non_main = false;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id == target {
                matches += 1;
                explicit_identity = function.explicit_id;
                monomorphic = function.type_parameters.is_empty();
                non_main = function.name != "main";
            }
        }
    }
    let comment_free_canonical_workspace = revision.sources().iter().all(|source| {
        crate::parse_with_comments(source.source(), Path::new(source.path())).is_ok_and(
            |(program, comments)| {
                comments.items.is_empty() && crate::format::canonical(&program) == source.source()
            },
        )
    });
    Ok(RenameDisplayNameEligibility {
        expected_old_value,
        comment_free_canonical_workspace,
        explicit_identity,
        monomorphic,
        non_main,
        unique_function: matches == 1,
    })
}

fn require_canonical_comment_free_sources(
    revision: &ProjectRevision,
) -> Result<(), Vec<Diagnostic>> {
    for source in revision.sources() {
        let (program, comments) =
            crate::parse_with_comments(source.source(), Path::new(source.path()))
                .map_err(|error| vec![error])?;
        if !comments.items.is_empty() || crate::format::canonical(&program) != source.source() {
            return Err(invalid(
                "semantic transaction v1 requires comment-free canonical source",
            ));
        }
    }
    Ok(())
}

fn parse_value(source: &str) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(source)
        .map_err(|_| invalid("semantic transaction artifact is invalid JSON"))
}

fn validate_text(value: &str, max: usize, message: &'static str) -> Result<(), Vec<Diagnostic>> {
    if value.is_empty() || value.len() > max || value.contains('\0') {
        Err(invalid(message))
    } else {
        Ok(())
    }
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "semantic transaction revision is not a canonical SHA-256 digest",
        ));
    }
    Ok(())
}

fn render(mut value: Value, limit: usize) -> Result<String, Vec<Diagnostic>> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("semantic transaction artifact could not be rendered"))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(capacity(
            "semantic transaction artifact exceeds its byte limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid("semantic transaction artifact is not UTF-8"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G525", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G526", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G527", message)]
}
