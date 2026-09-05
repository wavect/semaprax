//! Bounded, authority-free Universal Semantic Transaction v1 kernel.
//!
//! V1 admits exactly one typed operation over one immutable canonical semantic
//! workspace revision. Validation derives a Project candidate and deterministic
//! evidence; it never writes source or grants publication authority.

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{
    ProgramRoot, ProjectCandidate, ProjectRevision, SemanticChange, SEMANTIC_CHANGE_REQUIREMENTS,
};

mod add_declaration;
pub(super) use add_declaration::add_declaration_eligibility;
pub use add_declaration::SemanticTransactionAddDeclaration;
use add_declaration::{
    require_add_declaration_preconditions, require_source_preserving_declaration_addition,
};

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

/// Replace one complete function body block while preserving all source bytes
/// outside its authenticated span. The replacement uses the existing closed
/// Project Candidate expression grammar and is admitted only after a complete
/// Project rebuild.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransactionReplaceBlock {
    target: String,
    expected_old_block: String,
    replacement: Value,
}

/// Append one typed predicate after authenticating the complete ordered
/// contract inventory of the selected function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransactionAddContract {
    target: String,
    expected_old_contract: Value,
    phase: String,
    predicate: Value,
}

impl SemanticTransactionAddContract {
    pub fn new(
        target: impl Into<String>,
        expected_old_contract: Value,
        phase: impl Into<String>,
        predicate: Value,
    ) -> Self {
        Self {
            target: target.into(),
            expected_old_contract,
            phase: phase.into(),
            predicate,
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn expected_old_contract(&self) -> &Value {
        &self.expected_old_contract
    }
    pub fn phase(&self) -> &str {
        &self.phase
    }
    pub fn predicate(&self) -> &Value {
        &self.predicate
    }

    fn value(&self) -> Value {
        json!({
            "expected_old_contract": self.expected_old_contract,
            "kind": "add_contract",
            "phase": self.phase,
            "predicate": self.predicate,
            "target": self.target,
        })
    }
}

impl SemanticTransactionReplaceBlock {
    pub fn new(
        target: impl Into<String>,
        expected_old_block: impl Into<String>,
        replacement: Value,
    ) -> Self {
        Self {
            target: target.into(),
            expected_old_block: expected_old_block.into(),
            replacement,
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn expected_old_block(&self) -> &str {
        &self.expected_old_block
    }
    pub fn replacement(&self) -> &Value {
        &self.replacement
    }

    fn value(&self) -> Value {
        json!({
            "expected_old_block": self.expected_old_block,
            "kind": "replace_block",
            "replacement": self.replacement,
            "target": self.target,
        })
    }
}

/// Closed operation algebra for the one-operation v1 envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticTransactionOperation {
    RenameDisplayName(SemanticTransactionRenameDisplayName),
    ReplaceBlock(SemanticTransactionReplaceBlock),
    AddContract(SemanticTransactionAddContract),
    AddDeclaration(SemanticTransactionAddDeclaration),
}

impl SemanticTransactionOperation {
    pub fn target(&self) -> &str {
        match self {
            Self::RenameDisplayName(operation) => operation.target(),
            Self::ReplaceBlock(operation) => operation.target(),
            Self::AddContract(operation) => operation.target(),
            Self::AddDeclaration(operation) => operation.target(),
        }
    }

    fn value(&self) -> Value {
        match self {
            Self::RenameDisplayName(operation) => operation.value(),
            Self::ReplaceBlock(operation) => operation.value(),
            Self::AddContract(operation) => operation.value(),
            Self::AddDeclaration(operation) => operation.value(),
        }
    }
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
    operation: SemanticTransactionOperation,
    json: String,
    digest: String,
}

impl SemanticTransaction {
    pub fn rename_display_name(
        expected_workspace_revision: &str,
        operation: SemanticTransactionRenameDisplayName,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::new(
            expected_workspace_revision,
            SemanticTransactionOperation::RenameDisplayName(operation),
        )
    }

    pub fn replace_block(
        expected_workspace_revision: &str,
        operation: SemanticTransactionReplaceBlock,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::new(
            expected_workspace_revision,
            SemanticTransactionOperation::ReplaceBlock(operation),
        )
    }

    pub fn add_contract(
        expected_workspace_revision: &str,
        operation: SemanticTransactionAddContract,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::new(
            expected_workspace_revision,
            SemanticTransactionOperation::AddContract(operation),
        )
    }

    pub fn add_declaration(
        expected_workspace_revision: &str,
        operation: SemanticTransactionAddDeclaration,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::new(
            expected_workspace_revision,
            SemanticTransactionOperation::AddDeclaration(operation),
        )
    }

    fn new(
        expected_workspace_revision: &str,
        operation: SemanticTransactionOperation,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_workspace_revision)?;
        validate_text(
            operation.target(),
            4096,
            "transaction target is not bounded",
        )?;
        if let SemanticTransactionOperation::RenameDisplayName(operation) = &operation {
            validate_text(
                &operation.expected_old_value,
                128,
                "expected display name is not bounded",
            )?;
            validate_text(&operation.new_value, 128, "new display name is not bounded")?;
        }
        if let SemanticTransactionOperation::ReplaceBlock(operation) = &operation {
            validate_text(
                &operation.expected_old_block,
                MAX_SEMANTIC_TRANSACTION_BYTES / 2,
                "expected function block is not bounded",
            )?;
            if operation
                .replacement
                .get("kind")
                .and_then(Value::as_str)
                .is_none()
            {
                return Err(invalid(
                    "ReplaceBlock v1 requires a typed body-expression replacement",
                ));
            }
        }
        if let SemanticTransactionOperation::AddContract(operation) = &operation {
            if !matches!(operation.phase.as_str(), "requires" | "ensures") {
                return Err(invalid("AddContract phase must be requires or ensures"));
            }
            require_contract_snapshot_shape(&operation.expected_old_contract)?;
            if operation
                .predicate
                .get("kind")
                .and_then(Value::as_str)
                .is_none()
            {
                return Err(invalid(
                    "AddContract v1 requires a typed predicate-expression constructor",
                ));
            }
        }
        if let SemanticTransactionOperation::AddDeclaration(operation) = &operation {
            operation.validate_shape()?;
        }
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
        let text = |key: &str| {
            operations[0][key]
                .as_str()
                .ok_or_else(|| invalid("semantic transaction operation text is invalid"))
        };
        let revision = value["expected_workspace_revision"]
            .as_str()
            .ok_or_else(|| invalid("semantic transaction expected revision is invalid"))?;
        let transaction = match operations[0]["kind"].as_str() {
            Some("rename_display_name") => {
                let keys = ["expected_old_value", "kind", "new_value", "target"];
                if operation.len() != keys.len()
                    || keys.iter().any(|key| !operation.contains_key(*key))
                {
                    return Err(invalid(
                        "semantic transaction RenameDisplayName field set is invalid",
                    ));
                }
                Self::rename_display_name(
                    revision,
                    SemanticTransactionRenameDisplayName::new(
                        text("target")?,
                        text("expected_old_value")?,
                        text("new_value")?,
                    ),
                )?
            }
            Some("replace_block") => {
                let keys = ["expected_old_block", "kind", "replacement", "target"];
                if operation.len() != keys.len()
                    || keys.iter().any(|key| !operation.contains_key(*key))
                {
                    return Err(invalid(
                        "semantic transaction ReplaceBlock field set is invalid",
                    ));
                }
                Self::replace_block(
                    revision,
                    SemanticTransactionReplaceBlock::new(
                        text("target")?,
                        text("expected_old_block")?,
                        operations[0]["replacement"].clone(),
                    ),
                )?
            }
            Some("add_contract") => {
                let keys = [
                    "expected_old_contract",
                    "kind",
                    "phase",
                    "predicate",
                    "target",
                ];
                if operation.len() != keys.len()
                    || keys.iter().any(|key| !operation.contains_key(*key))
                {
                    return Err(invalid(
                        "semantic transaction AddContract field set is invalid",
                    ));
                }
                Self::add_contract(
                    revision,
                    SemanticTransactionAddContract::new(
                        text("target")?,
                        operations[0]["expected_old_contract"].clone(),
                        text("phase")?,
                        operations[0]["predicate"].clone(),
                    ),
                )?
            }
            Some("add_declaration") => {
                let keys = ["declaration", "expected_old_module", "kind", "target"];
                if operation.len() != keys.len()
                    || keys.iter().any(|key| !operation.contains_key(*key))
                {
                    return Err(invalid(
                        "semantic transaction AddDeclaration field set is invalid",
                    ));
                }
                Self::add_declaration(
                    revision,
                    SemanticTransactionAddDeclaration::new(
                        text("target")?,
                        operations[0]["expected_old_module"].clone(),
                        operations[0]["declaration"].clone(),
                    ),
                )?
            }
            _ => {
                return Err(invalid(
                    "semantic transaction operation kind is unsupported",
                ))
            }
        };
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
    pub fn operation(&self) -> &SemanticTransactionOperation {
        &self.operation
    }

    pub(super) fn rename_operation(&self) -> Option<&SemanticTransactionRenameDisplayName> {
        match &self.operation {
            SemanticTransactionOperation::RenameDisplayName(operation) => Some(operation),
            SemanticTransactionOperation::ReplaceBlock(_) => None,
            SemanticTransactionOperation::AddContract(_) => None,
            SemanticTransactionOperation::AddDeclaration(_) => None,
        }
    }

    /// Validate from immutable compiler state and mint deterministic evidence.
    pub fn validate(
        &self,
        base: Arc<ProjectRevision>,
    ) -> Result<SemanticTransactionArtifacts, Vec<Diagnostic>> {
        let base_workspace = base.canonical_workspace_revision()?;
        let base_program_root = base_workspace.program_root()?;
        if base_program_root.workspace_revision() != self.expected_workspace_revision {
            return Err(stale(
                "semantic transaction expected workspace revision is stale",
            ));
        }
        require_canonical_comment_free_sources(&base)?;
        let initial = ProjectCandidate::open(Arc::clone(&base), base.project_revision())?;
        let change = match &self.operation {
            SemanticTransactionOperation::RenameDisplayName(operation) => {
                require_rename_preconditions(&base, operation)?;
                SemanticChange::new(
                    base.project_revision(),
                    &json!({
                        "kind": "rename_declaration",
                        "name": operation.new_value,
                        "target": operation.target,
                    }),
                )?
            }
            SemanticTransactionOperation::ReplaceBlock(operation) => {
                require_replace_block_preconditions(&base, operation)?;
                SemanticChange::new(
                    base.project_revision(),
                    &json!({
                        "kind": "replace_function_body",
                        "target": operation.target,
                        "body": operation.replacement,
                    }),
                )?
            }
            SemanticTransactionOperation::AddContract(operation) => {
                require_add_contract_preconditions(&base, operation)?;
                SemanticChange::new(
                    base.project_revision(),
                    &json!({
                        "kind": "add_contract",
                        "target": operation.target,
                        "phase": operation.phase,
                        "predicate": operation.predicate,
                    }),
                )?
            }
            SemanticTransactionOperation::AddDeclaration(operation) => {
                require_add_declaration_preconditions(&base, operation)?;
                SemanticChange::new(
                    base.project_revision(),
                    &json!({
                        "declaration": operation.declaration(),
                        "kind": "add_declaration",
                        "target": operation.target(),
                    }),
                )?
            }
        };
        let candidate = initial.apply(initial.candidate_digest(), &change)?;
        require_canonical_comment_free_sources(candidate.revision())?;
        let candidate_workspace = candidate.revision().canonical_workspace_revision()?;
        let candidate_program_root = candidate_workspace.program_root()?;
        let source_review_text = candidate.source_review(candidate.candidate_digest())?;
        let source_review: Value = serde_json::from_str(&source_review_text)
            .map_err(|_| invalid("candidate source review is not valid JSON"))?;
        let (impact_key, impact_detail, review_detail, operation_result) = match &self.operation {
            SemanticTransactionOperation::RenameDisplayName(operation) => {
                let before = base
                    .semantic
                    .image_symbol(&operation.target)
                    .ok_or_else(|| {
                        stale("transaction target disappeared from the base revision")
                    })?;
                let after = candidate
                    .revision()
                    .semantic
                    .image_symbol(&operation.target)
                    .ok_or_else(|| {
                        stale("transaction target disappeared from the candidate revision")
                    })?;
                if before.get("name").and_then(Value::as_str)
                    != Some(operation.expected_old_value.as_str())
                    || after.get("name").and_then(Value::as_str)
                        != Some(operation.new_value.as_str())
                {
                    return Err(stale(
                        "RenameDisplayName result does not match its exact name preconditions",
                    ));
                }
                (
                    "display_name",
                    json!({"before": before["name"], "after": after["name"]}),
                    json!({
                        "candidate_rebuilt_from_canonical_source": true,
                        "exact_old_value_precondition": true,
                        "stable_identity_preserved": true,
                        "trivia_preservation": "comment_free_canonical_base_and_candidate",
                    }),
                    json!({
                        "kind": "rename_display_name", "outcome": "validated",
                        "target": operation.target, "old_value": operation.expected_old_value,
                        "new_value": operation.new_value,
                    }),
                )
            }
            SemanticTransactionOperation::ReplaceBlock(operation) => {
                let replacement = require_source_preserving_block_replacement(
                    &base,
                    candidate.revision(),
                    operation,
                )?;
                (
                    "block",
                    json!({
                        "before": operation.expected_old_block,
                        "after": replacement.new_block,
                        "source_path": replacement.path,
                        "source_outside_block_preserved": true,
                    }),
                    json!({
                        "candidate_rebuilt_from_canonical_source": true,
                        "exact_old_block_precondition": true,
                        "source_outside_selected_block_preserved": true,
                        "stable_identity_preserved": true,
                        "trivia_preservation": "exact_outside_authenticated_block_span",
                    }),
                    json!({
                        "kind": "replace_block", "outcome": "validated",
                        "target": operation.target,
                        "old_block": operation.expected_old_block,
                        "new_block": replacement.new_block,
                    }),
                )
            }
            SemanticTransactionOperation::AddContract(operation) => {
                let addition = require_source_preserving_contract_addition(
                    &base,
                    candidate.revision(),
                    operation,
                )?;
                (
                    "contract",
                    json!({
                        "after": addition.new_contract,
                        "before": operation.expected_old_contract,
                        "phase": operation.phase,
                        "source_path": addition.path,
                        "source_outside_added_clause_preserved": true,
                    }),
                    json!({
                        "candidate_rebuilt_from_canonical_source": true,
                        "exact_old_contract_precondition": true,
                        "prior_predicate_order_preserved": true,
                        "source_outside_added_clause_preserved": true,
                        "stable_identity_preserved": true,
                        "trivia_preservation": "exact_outside_added_contract_clause",
                    }),
                    json!({
                        "kind": "add_contract", "outcome": "validated",
                        "target": operation.target,
                        "phase": operation.phase,
                        "old_contract": operation.expected_old_contract,
                        "new_contract": addition.new_contract,
                    }),
                )
            }
            SemanticTransactionOperation::AddDeclaration(operation) => {
                let addition = require_source_preserving_declaration_addition(
                    &base,
                    candidate.revision(),
                    operation,
                )?;
                (
                    "declaration",
                    json!({
                        "added_identity_inventory": addition.added_identity_inventory.clone(),
                        "after_module": addition.new_module.clone(),
                        "before_module": operation.expected_old_module(),
                        "inserted_source": addition.inserted_source,
                        "source_outside_added_declaration_preserved": true,
                    }),
                    json!({
                        "candidate_rebuilt_from_canonical_source": true,
                        "exact_old_module_precondition": true,
                        "global_identity_freshness_checked": true,
                        "prior_declaration_order_preserved": true,
                        "source_outside_added_declaration_preserved": true,
                        "trivia_preservation": "exact_outside_added_declaration_span",
                    }),
                    json!({
                        "added_identity_inventory": addition.added_identity_inventory,
                        "kind": "add_declaration",
                        "new_module": addition.new_module,
                        "old_module": operation.expected_old_module(),
                        "outcome": "validated",
                        "target": operation.target(),
                    }),
                )
            }
        };
        if base_workspace.manifest_digest() != candidate_workspace.manifest_digest()
            || base_workspace.dependency_lock_digest()
                != candidate_workspace.dependency_lock_digest()
            || base_workspace.authority_policies() != candidate_workspace.authority_policies()
            || base_workspace.target_profiles() != candidate_workspace.target_profiles()
        {
            return Err(stale(
                "semantic transaction changed manifest, dependency, authority, or target facts",
            ));
        }

        let mut impact_value = json!({
            "base_workspace_revision": base_program_root.workspace_revision(),
            "candidate_workspace_revision": candidate_program_root.workspace_revision(),
            "classification": "descriptive_compiler_projection",
            "identity": {"preserved": true, "target": self.operation.target()},
            "nonclaims": ["not_behavioral_equivalence", "not_runtime_execution", "no_authority"],
            "schema": SEMANTIC_TRANSACTION_IMPACT_SCHEMA,
            "transaction_digest": self.digest,
        });
        impact_value
            .as_object_mut()
            .expect("impact projection is an object")
            .insert(impact_key.to_owned(), impact_detail);
        let impact = render(impact_value, MAX_SEMANTIC_TRANSACTION_ARTIFACT_BYTES)?;
        let impact_digest = digest(IMPACT_DOMAIN, impact.as_bytes());
        let review = render(
            json!({
                "authority": {"granted": false, "requested": "none"},
                "impact_digest": impact_digest,
                "review": review_detail,
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
                    "workspace_revision": base_program_root.workspace_revision(),
                },
                "candidate": {
                    "evidence": candidate_value,
                    "project_revision": candidate.revision().project_revision(),
                    "revision": candidate.candidate_digest(),
                    "workspace_revision": candidate_program_root.workspace_revision(),
                },
                "operation_results": [operation_result],
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
            base_program_root,
            candidate_program_root,
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
    base_program_root: ProgramRoot,
    candidate_program_root: ProgramRoot,
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
    pub fn base_program_root(&self) -> &ProgramRoot {
        &self.base_program_root
    }
    pub fn candidate_program_root(&self) -> &ProgramRoot {
        &self.candidate_program_root
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

fn require_rename_preconditions(
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

struct BlockSelection {
    path: String,
    start: usize,
    end: usize,
    block: String,
}

struct BlockReplacement {
    path: String,
    new_block: String,
}

struct ContractSelection {
    path: String,
    snapshot: Value,
    requires: usize,
    ensures: usize,
}

struct ContractAddition {
    path: String,
    new_contract: Value,
}

fn contract_snapshot(
    source: &str,
    function: &crate::ast::Function,
) -> Result<Value, Vec<Diagnostic>> {
    let expressions = |items: &[crate::ast::Expr]| -> Result<Vec<String>, Vec<Diagnostic>> {
        items
            .iter()
            .map(|expression| {
                source
                    .get(expression.span.start..expression.span.end)
                    .map(str::to_owned)
                    .ok_or_else(|| stale("AddContract predicate span is unavailable"))
            })
            .collect()
    };
    Ok(json!({
        "ensures": expressions(&function.ensures)?,
        "requires": expressions(&function.requires)?,
    }))
}

fn select_contract(
    revision: &ProjectRevision,
    target: &str,
) -> Result<ContractSelection, Vec<Diagnostic>> {
    let mut selected = None;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id != target {
                continue;
            }
            if selected.is_some() {
                return Err(invalid(
                    "AddContract v1 requires one unambiguous function target",
                ));
            }
            selected = Some(ContractSelection {
                path: source.path().to_owned(),
                snapshot: contract_snapshot(source.source(), function)?,
                requires: function.requires.len(),
                ensures: function.ensures.len(),
            });
        }
    }
    selected.ok_or_else(|| invalid("AddContract v1 requires an explicit function target"))
}

fn require_contract_snapshot_shape(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("AddContract expected old contract is not an object"))?;
    if object.len() != 2 || !object.contains_key("requires") || !object.contains_key("ensures") {
        return Err(invalid(
            "AddContract expected old contract field set is invalid",
        ));
    }
    for phase in ["requires", "ensures"] {
        let items = object[phase]
            .as_array()
            .ok_or_else(|| invalid("AddContract expected old contract inventory is invalid"))?;
        if items.len() > 1024
            || items.iter().any(|item| {
                item.as_str()
                    .is_none_or(|text| text.is_empty() || text.contains('\0'))
            })
        {
            return Err(invalid(
                "AddContract expected old contract inventory is invalid",
            ));
        }
    }
    Ok(())
}

fn require_add_contract_preconditions(
    revision: &ProjectRevision,
    operation: &SemanticTransactionAddContract,
) -> Result<(), Vec<Diagnostic>> {
    let eligibility = add_contract_eligibility(revision, &operation.target)?;
    if !eligibility.unique_function
        || !eligibility.explicit_identity
        || !eligibility.monomorphic
        || !eligibility.non_main
    {
        return Err(invalid(
            "AddContract v1 requires an explicit monomorphic non-main function",
        ));
    }
    if !eligibility.inventory_below_capacity {
        return Err(capacity("AddContract contract inventory exceeds its limit"));
    }
    if eligibility.expected_old_contract.as_ref() != Some(&operation.expected_old_contract) {
        return Err(stale(
            "AddContract expected old contract does not match the exact base",
        ));
    }
    Ok(())
}

fn require_source_preserving_contract_addition(
    base: &ProjectRevision,
    candidate: &ProjectRevision,
    operation: &SemanticTransactionAddContract,
) -> Result<ContractAddition, Vec<Diagnostic>> {
    let before = select_contract(base, &operation.target)?;
    let after = select_contract(candidate, &operation.target)?;
    if before.path != after.path || base.sources().len() != candidate.sources().len() {
        return Err(stale("AddContract changed the source owner or inventory"));
    }
    if before.snapshot != operation.expected_old_contract {
        return Err(stale(
            "AddContract old contract changed before source review",
        ));
    }
    let old_items = before.snapshot[operation.phase.as_str()]
        .as_array()
        .ok_or_else(|| stale("AddContract old contract inventory is unavailable"))?;
    let new_items = after.snapshot[operation.phase.as_str()]
        .as_array()
        .ok_or_else(|| stale("AddContract new contract inventory is unavailable"))?;
    if new_items.len() != old_items.len() + 1
        || new_items[..old_items.len()] != old_items[..]
        || (operation.phase == "requires" && before.ensures != after.ensures)
        || (operation.phase == "ensures" && before.requires != after.requires)
    {
        return Err(stale(
            "AddContract did not append exactly one predicate after the old contract",
        ));
    }
    new_items
        .last()
        .ok_or_else(|| stale("AddContract result is absent"))?;
    for old_source in base.sources() {
        let new_source = candidate
            .sources()
            .iter()
            .find(|source| source.path() == old_source.path())
            .ok_or_else(|| stale("AddContract changed the source inventory"))?;
        if old_source.path() != before.path {
            if old_source.source() != new_source.source() {
                return Err(stale("AddContract changed an unrelated source"));
            }
            continue;
        }
        let program = crate::parse(new_source.source(), Path::new(new_source.path()))
            .map_err(|error| vec![error])?;
        let function = program
            .functions
            .iter()
            .find(|function| function.stable_id == operation.target)
            .ok_or_else(|| stale("AddContract candidate target disappeared"))?;
        let expression = if operation.phase == "requires" {
            function.requires.last()
        } else {
            function.ensures.last()
        }
        .ok_or_else(|| stale("AddContract candidate predicate disappeared"))?;
        let line_start = new_source.source()[..expression.span.start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = new_source.source()[expression.span.end..]
            .find('\n')
            .map(|index| expression.span.end + index + 1)
            .ok_or_else(|| stale("AddContract clause terminator is unavailable"))?;
        let expected_prefix = format!("    {} ", operation.phase);
        if !new_source.source()[line_start..expression.span.start].ends_with(&expected_prefix) {
            return Err(stale("AddContract clause source shape is invalid"));
        }
        let mut restored = new_source.source().to_owned();
        restored.replace_range(line_start..line_end, "");
        if restored != old_source.source() {
            return Err(stale(
                "AddContract changed source outside the appended contract clause",
            ));
        }
    }
    Ok(ContractAddition {
        path: before.path,
        new_contract: after.snapshot,
    })
}

fn select_block(
    revision: &ProjectRevision,
    target: &str,
) -> Result<BlockSelection, Vec<Diagnostic>> {
    let mut selected = None;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id != target {
                continue;
            }
            if selected.is_some() {
                return Err(invalid(
                    "ReplaceBlock v1 requires one unambiguous function target",
                ));
            }
            let start = function.body.span.start;
            let end = function.body.span.end;
            let block = source
                .source()
                .get(start..end)
                .ok_or_else(|| stale("ReplaceBlock function block span is unavailable"))?;
            selected = Some(BlockSelection {
                path: source.path().to_owned(),
                start,
                end,
                block: block.to_owned(),
            });
        }
    }
    selected.ok_or_else(|| invalid("ReplaceBlock v1 requires an explicit function target"))
}

fn require_replace_block_preconditions(
    revision: &ProjectRevision,
    operation: &SemanticTransactionReplaceBlock,
) -> Result<(), Vec<Diagnostic>> {
    let eligibility = replace_block_eligibility(revision, &operation.target)?;
    if !eligibility.unique_function
        || !eligibility.explicit_identity
        || !eligibility.monomorphic
        || !eligibility.non_main
    {
        return Err(invalid(
            "ReplaceBlock v1 requires an explicit monomorphic non-main function",
        ));
    }
    if eligibility.expected_old_block.as_deref() != Some(operation.expected_old_block.as_str()) {
        return Err(stale(
            "ReplaceBlock expected old block does not match the exact base source",
        ));
    }
    Ok(())
}

fn require_source_preserving_block_replacement(
    base: &ProjectRevision,
    candidate: &ProjectRevision,
    operation: &SemanticTransactionReplaceBlock,
) -> Result<BlockReplacement, Vec<Diagnostic>> {
    let before = select_block(base, &operation.target)?;
    let after = select_block(candidate, &operation.target)?;
    if before.path != after.path {
        return Err(stale("ReplaceBlock changed the source owner of its target"));
    }
    if before.block != operation.expected_old_block || before.block == after.block {
        return Err(stale(
            "ReplaceBlock result does not match its exact block preconditions",
        ));
    }
    if base.sources().len() != candidate.sources().len() {
        return Err(stale("ReplaceBlock changed the source inventory"));
    }
    for (base_source, candidate_source) in base.sources().iter().zip(candidate.sources()) {
        if base_source.path() != candidate_source.path() {
            return Err(stale("ReplaceBlock changed the source inventory"));
        }
        if base_source.path() != before.path {
            if base_source.source() != candidate_source.source() {
                return Err(stale("ReplaceBlock changed an unrelated source"));
            }
            continue;
        }
        let base_prefix = base_source
            .source()
            .get(..before.start)
            .ok_or_else(|| stale("ReplaceBlock base prefix is unavailable"))?;
        let base_suffix = base_source
            .source()
            .get(before.end..)
            .ok_or_else(|| stale("ReplaceBlock base suffix is unavailable"))?;
        let candidate_prefix = candidate_source
            .source()
            .get(..after.start)
            .ok_or_else(|| stale("ReplaceBlock candidate prefix is unavailable"))?;
        let candidate_suffix = candidate_source
            .source()
            .get(after.end..)
            .ok_or_else(|| stale("ReplaceBlock candidate suffix is unavailable"))?;
        if base_prefix != candidate_prefix || base_suffix != candidate_suffix {
            return Err(stale(
                "ReplaceBlock changed source outside the authenticated block span",
            ));
        }
    }
    Ok(BlockReplacement {
        path: before.path,
        new_block: after.block,
    })
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

pub(super) struct ReplaceBlockEligibility {
    pub(super) expected_old_block: Option<String>,
    pub(super) comment_free_canonical_workspace: bool,
    pub(super) explicit_identity: bool,
    pub(super) monomorphic: bool,
    pub(super) non_main: bool,
    unique_function: bool,
}

impl ReplaceBlockEligibility {
    pub(super) fn available(&self) -> bool {
        self.unique_function
            && self.comment_free_canonical_workspace
            && self.explicit_identity
            && self.monomorphic
            && self.non_main
    }
}

pub(super) struct AddContractEligibility {
    pub(super) expected_old_contract: Option<Value>,
    pub(super) comment_free_canonical_workspace: bool,
    pub(super) explicit_identity: bool,
    pub(super) monomorphic: bool,
    pub(super) non_main: bool,
    pub(super) inventory_below_capacity: bool,
    unique_function: bool,
}

impl AddContractEligibility {
    pub(super) fn available(&self) -> bool {
        self.unique_function
            && self.comment_free_canonical_workspace
            && self.explicit_identity
            && self.monomorphic
            && self.non_main
            && self.inventory_below_capacity
    }
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

pub(super) fn replace_block_eligibility(
    revision: &ProjectRevision,
    target: &str,
) -> Result<ReplaceBlockEligibility, Vec<Diagnostic>> {
    let mut expected_old_block = None;
    let mut matches = 0usize;
    let mut explicit_identity = false;
    let mut monomorphic = false;
    let mut non_main = false;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id != target {
                continue;
            }
            matches += 1;
            expected_old_block = source
                .source()
                .get(function.body.span.start..function.body.span.end)
                .map(str::to_owned);
            explicit_identity = function.explicit_id;
            monomorphic = function.type_parameters.is_empty();
            non_main = function.name != "main";
        }
    }
    Ok(ReplaceBlockEligibility {
        expected_old_block,
        comment_free_canonical_workspace: comment_free_canonical_workspace(revision),
        explicit_identity,
        monomorphic,
        non_main,
        unique_function: matches == 1,
    })
}

pub(super) fn add_contract_eligibility(
    revision: &ProjectRevision,
    target: &str,
) -> Result<AddContractEligibility, Vec<Diagnostic>> {
    let mut expected_old_contract = None;
    let mut matches = 0usize;
    let mut explicit_identity = false;
    let mut monomorphic = false;
    let mut non_main = false;
    let mut inventory_below_capacity = false;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id != target {
                continue;
            }
            matches += 1;
            expected_old_contract = Some(contract_snapshot(source.source(), function)?);
            explicit_identity = function.explicit_id;
            monomorphic = function.type_parameters.is_empty();
            non_main = function.name != "main";
            inventory_below_capacity = function
                .requires
                .len()
                .saturating_add(function.ensures.len())
                < 1024;
        }
    }
    Ok(AddContractEligibility {
        expected_old_contract,
        comment_free_canonical_workspace: comment_free_canonical_workspace(revision),
        explicit_identity,
        monomorphic,
        non_main,
        inventory_below_capacity,
        unique_function: matches == 1,
    })
}

fn comment_free_canonical_workspace(revision: &ProjectRevision) -> bool {
    revision.sources().iter().all(|source| {
        crate::parse_with_comments(source.source(), Path::new(source.path())).is_ok_and(
            |(program, comments)| {
                comments.items.is_empty() && crate::format::canonical(&program) == source.source()
            },
        )
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
