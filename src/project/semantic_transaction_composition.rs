//! Authority-free structural diff, rebase, and ordered merge for validated
//! Universal Semantic Transaction v1 values.
//!
//! Composition always returns immutable evidence and checked candidates. It
//! never publishes source, and an ordered merge is deliberately not represented
//! as the closed one-operation Semantic Transaction v1 envelope.

use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::semantic_transaction::rename_display_name_eligibility;
use super::{
    ProjectCandidate, ProjectRevision, SemanticTransaction, SemanticTransactionArtifacts,
    SemanticTransactionRenameDisplayName, SemanticWorkspaceRevision,
};

pub const SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_SCHEMA: &str =
    "semaprax.semantic-workspace-structural-diff.v1";
pub const SEMANTIC_TRANSACTION_REBASE_SCHEMA: &str = "semaprax.semantic-transaction-rebase.v1";
pub const SEMANTIC_TRANSACTION_MERGE_SCHEMA: &str = "semaprax.semantic-transaction-merge.v1";
pub const MAX_SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SEMANTIC_TRANSACTION_COMPOSITION_BYTES: usize = 64 * 1024 * 1024;

const STRUCTURAL_DIFF_DOMAIN: &[u8] = b"semaprax.semantic-workspace-structural-diff.digest.v1\0";
const REBASE_DOMAIN: &[u8] = b"semaprax.semantic-transaction-rebase.digest.v1\0";
const MERGE_DOMAIN: &[u8] = b"semaprax.semantic-transaction-merge.digest.v1\0";
const CATALOG_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-structural-diff.root-catalog.digest.v1\0";
const SOURCE_REVIEW_DOMAIN: &[u8] =
    b"semaprax.semantic-transaction-composition.source-review.digest.v1\0";
const RECONCILIATION_DOMAIN: &[u8] =
    b"semaprax.semantic-transaction-composition.reconciliation.digest.v1\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// A bounded comparison of canonical workspace components and authored roots.
/// Equality is structural projection equality, never behavioral equivalence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceStructuralDiff {
    json: String,
    digest: String,
    source_review: String,
    source_review_digest: String,
}

impl SemanticWorkspaceStructuralDiff {
    pub fn derive(candidate: &ProjectCandidate, expected_candidate: &str) -> Result<Self> {
        require_digest(expected_candidate)?;
        if candidate.candidate_digest() != expected_candidate {
            return Err(stale("structural diff candidate selector is stale"));
        }
        let base = candidate.base_revision().canonical_workspace_revision()?;
        let after = candidate.revision().canonical_workspace_revision()?;
        let catalog = candidate.semantic_delta_catalog(expected_candidate)?;
        let source_review = candidate.source_review(expected_candidate)?;
        let catalog_value = parse_embedded(&catalog, "semantic delta catalog is invalid JSON")?;
        let source_review_value =
            parse_embedded(&source_review, "candidate source review is invalid JSON")?;
        let source_review_digest = digest(SOURCE_REVIEW_DOMAIN, source_review.as_bytes());
        let changed_components = changed_components(&base, &after);
        let changed_nodes = changed_nodes(&base, &after);
        let json = render(
            json!({
                "authority": false,
                "base": revision_binding(candidate.base_revision(), &base),
                "candidate": revision_binding(candidate.revision(), &after),
                "candidate_digest": candidate.candidate_digest(),
                "changed_components": changed_components,
                "changed_nodes": changed_nodes,
                "classification": "canonical_revision_digest_and_authored_structure_projection",
                "limits": {
                    "max_report_bytes": MAX_SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_BYTES,
                },
                "nonclaims": [
                    "not_behavioral_equivalence",
                    "not_complete_dynamic_impact",
                    "not_a_source_patch_or_trivia_preservation",
                    "no_runtime_or_test_execution",
                    "no_source_commit_or_publication_authority",
                ],
                "root_catalog": {
                    "digest": digest(CATALOG_DOMAIN, catalog.as_bytes()),
                    "value": catalog_value,
                },
                "schema": SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_SCHEMA,
                "source_review": {
                    "digest": source_review_digest,
                    "value": source_review_value,
                },
            }),
            MAX_SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_BYTES,
        )?;
        let digest = digest(STRUCTURAL_DIFF_DOMAIN, json.as_bytes());
        Ok(Self {
            json,
            digest,
            source_review,
            source_review_digest,
        })
    }

    /// Recompute from the retained candidate, including its independent source
    /// history replay, and require exact submitted bytes and digest.
    pub fn replay(
        candidate: &ProjectCandidate,
        expected_candidate: &str,
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        validate_submitted(
            bytes,
            MAX_SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_BYTES,
            SEMANTIC_WORKSPACE_STRUCTURAL_DIFF_SCHEMA,
        )?;
        require_digest(expected_digest)?;
        if digest(STRUCTURAL_DIFF_DOMAIN, bytes) != expected_digest {
            return Err(stale("structural diff digest is stale"));
        }
        let derived = Self::derive(candidate, expected_candidate)?;
        if derived.to_json().as_bytes() != bytes || derived.digest() != expected_digest {
            return Err(stale("structural diff failed exact replay"));
        }
        Ok(derived)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn source_review(&self) -> &str {
        &self.source_review
    }
    pub fn source_review_digest(&self) -> &str {
        &self.source_review_digest
    }
}

/// A freshly validated one-operation transaction reminted on an exact new base.
pub struct SemanticTransactionRebase {
    transaction: SemanticTransaction,
    artifacts: SemanticTransactionArtifacts,
    structural_diff: SemanticWorkspaceStructuralDiff,
    reconciliation: String,
    reconciliation_digest: String,
    json: String,
    digest: String,
}

impl SemanticTransactionRebase {
    pub fn derive(
        transaction: &SemanticTransaction,
        original_base: Arc<ProjectRevision>,
        onto: Arc<ProjectRevision>,
        expected_onto_workspace_revision: &str,
    ) -> Result<Self> {
        require_digest(expected_onto_workspace_revision)?;
        let original_workspace = original_base.canonical_workspace_revision()?;
        let onto_workspace = onto.canonical_workspace_revision()?;
        if onto_workspace.workspace_revision() != expected_onto_workspace_revision {
            return Err(stale("semantic transaction rebase destination is stale"));
        }
        let original_artifacts = transaction.validate(Arc::clone(&original_base))?;
        let eligibility = rename_display_name_eligibility(&onto, transaction.operation().target())?;
        if !eligibility.available() {
            return Err(conflict(
                "semantic transaction rebase target is outside RenameDisplayName v1",
            ));
        }
        let current_name = eligibility.expected_old_value.ok_or_else(|| {
            conflict("semantic transaction rebase target is absent from the destination")
        })?;
        if current_name == transaction.operation().new_value() {
            return Err(conflict(
                "semantic transaction rebase is already satisfied on the destination",
            ));
        }
        let reconciled = original_artifacts
            .candidate()
            .rebase(
                original_artifacts.candidate().candidate_digest(),
                Arc::clone(&onto),
                onto.project_revision(),
            )
            .map_err(map_candidate_composition)?;
        let reconciliation = reconciled.to_json().to_owned();
        let reconciliation_digest = digest(RECONCILIATION_DOMAIN, reconciliation.as_bytes());
        let rebased = SemanticTransaction::rename_display_name(
            onto_workspace.workspace_revision(),
            SemanticTransactionRenameDisplayName::new(
                transaction.operation().target(),
                &current_name,
                transaction.operation().new_value(),
            ),
        )?;
        let artifacts = rebased.validate(Arc::clone(&onto))?;
        require_candidate_parity(reconciled.candidate(), artifacts.candidate())?;
        let structural_diff = SemanticWorkspaceStructuralDiff::derive(
            artifacts.candidate(),
            artifacts.candidate().candidate_digest(),
        )?;
        let source_review = structural_diff.source_review();
        let json = render(
            json!({
                "authority": false,
                "base": {
                    "onto_project_revision": onto.project_revision(),
                    "onto_workspace_revision": onto_workspace.workspace_revision(),
                    "original_project_revision": original_base.project_revision(),
                    "original_workspace_revision": original_workspace.workspace_revision(),
                },
                "nonclaims": [
                    "not_behavioral_equivalence",
                    "not_external_consumer_compatibility",
                    "no_runtime_or_project_test_execution",
                    "no_source_commit_or_publication_authority",
                    "comment_free_canonical_transaction_domain_only",
                ],
                "original_transaction": {
                    "digest": transaction.digest(),
                    "value": parse_embedded(transaction.to_json(), "original transaction is invalid JSON")?,
                },
                "rebased_transaction": {
                    "digest": rebased.digest(),
                    "value": parse_embedded(rebased.to_json(), "rebased transaction is invalid JSON")?,
                },
                "reconciliation": {
                    "candidate_parity": "exact_candidate_evidence_and_source",
                    "digest": reconciliation_digest,
                    "value": parse_embedded(&reconciliation, "candidate rebase report is invalid JSON")?,
                },
                "result": {
                    "candidate_digest": artifacts.candidate().candidate_digest(),
                    "project_revision": artifacts.candidate().revision().project_revision(),
                    "workspace_revision": artifacts.candidate().revision().canonical_workspace_revision()?.workspace_revision(),
                },
                "schema": SEMANTIC_TRANSACTION_REBASE_SCHEMA,
                "source_review": {
                    "digest": structural_diff.source_review_digest(),
                    "value": parse_embedded(source_review, "source review is invalid JSON")?,
                },
                "structural_diff": {
                    "digest": structural_diff.digest(),
                    "value": parse_embedded(structural_diff.to_json(), "structural diff is invalid JSON")?,
                },
                "validation": {
                    "candidate_rebase": true,
                    "canonical_workspace_revision": true,
                    "complete_project_admission": true,
                    "fresh_transaction_validation": true,
                },
            }),
            MAX_SEMANTIC_TRANSACTION_COMPOSITION_BYTES,
        )?;
        let digest = digest(REBASE_DOMAIN, json.as_bytes());
        Ok(Self {
            transaction: rebased,
            artifacts,
            structural_diff,
            reconciliation,
            reconciliation_digest,
            json,
            digest,
        })
    }

    pub fn replay(
        original_base: Arc<ProjectRevision>,
        onto: Arc<ProjectRevision>,
        transaction_bytes: &[u8],
        expected_onto_workspace_revision: &str,
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        let transaction = SemanticTransaction::from_json(transaction_bytes)?;
        validate_submitted(
            bytes,
            MAX_SEMANTIC_TRANSACTION_COMPOSITION_BYTES,
            SEMANTIC_TRANSACTION_REBASE_SCHEMA,
        )?;
        require_digest(expected_digest)?;
        if digest(REBASE_DOMAIN, bytes) != expected_digest {
            return Err(stale("semantic transaction rebase digest is stale"));
        }
        let derived = Self::derive(
            &transaction,
            original_base,
            onto,
            expected_onto_workspace_revision,
        )?;
        if derived.to_json().as_bytes() != bytes || derived.digest() != expected_digest {
            return Err(stale("semantic transaction rebase failed exact replay"));
        }
        Ok(derived)
    }

    pub fn transaction(&self) -> &SemanticTransaction {
        &self.transaction
    }
    pub fn artifacts(&self) -> &SemanticTransactionArtifacts {
        &self.artifacts
    }
    pub fn structural_diff(&self) -> &SemanticWorkspaceStructuralDiff {
        &self.structural_diff
    }
    pub fn reconciliation(&self) -> &str {
        &self.reconciliation
    }
    pub fn reconciliation_digest(&self) -> &str {
        &self.reconciliation_digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// The requested, observable order for a two-parent candidate merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticTransactionMergeOrder {
    LeftThenRight,
    RightThenLeft,
}

impl SemanticTransactionMergeOrder {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LeftThenRight => "left_then_right",
            Self::RightThenLeft => "right_then_left",
        }
    }
}

/// A fully admitted ordered two-transaction candidate. This is not a v1
/// transaction because that existing schema admits exactly one operation.
pub struct SemanticTransactionMerge {
    candidate: ProjectCandidate,
    structural_diff: SemanticWorkspaceStructuralDiff,
    reconciliation: String,
    reconciliation_digest: String,
    json: String,
    digest: String,
}

impl SemanticTransactionMerge {
    pub fn derive(
        left: &SemanticTransaction,
        right: &SemanticTransaction,
        shared_base: Arc<ProjectRevision>,
        order: SemanticTransactionMergeOrder,
    ) -> Result<Self> {
        if left.expected_workspace_revision() != right.expected_workspace_revision() {
            return Err(stale(
                "semantic transaction merge parents do not share a workspace revision",
            ));
        }
        if left.operation().target() == right.operation().target() {
            return Err(conflict(
                "semantic transaction merge requires distinct stable-ID targets",
            ));
        }
        if left.digest() == right.digest() {
            return Err(conflict(
                "semantic transaction merge requires distinct parent transactions",
            ));
        }
        let workspace = shared_base.canonical_workspace_revision()?;
        if workspace.workspace_revision() != left.expected_workspace_revision() {
            return Err(stale("semantic transaction merge base is stale"));
        }
        let left_artifacts = left.validate(Arc::clone(&shared_base))?;
        let right_artifacts = right.validate(Arc::clone(&shared_base))?;
        // ProjectCandidate::merge applies its argument first. Select receiver
        // and argument so the public order name describes actual replay order.
        let merged = match order {
            SemanticTransactionMergeOrder::LeftThenRight => right_artifacts.candidate().merge(
                right_artifacts.candidate().candidate_digest(),
                left_artifacts.candidate(),
                left_artifacts.candidate().candidate_digest(),
            ),
            SemanticTransactionMergeOrder::RightThenLeft => left_artifacts.candidate().merge(
                left_artifacts.candidate().candidate_digest(),
                right_artifacts.candidate(),
                right_artifacts.candidate().candidate_digest(),
            ),
        }
        .map_err(map_candidate_composition)?;
        let reconciliation = merged.to_json().to_owned();
        let reconciliation_digest = digest(RECONCILIATION_DOMAIN, reconciliation.as_bytes());
        let candidate = merged.into_candidate();
        let structural_diff =
            SemanticWorkspaceStructuralDiff::derive(&candidate, candidate.candidate_digest())?;
        let candidate_workspace = candidate.revision().canonical_workspace_revision()?;
        let json = render(
            json!({
                "authority": false,
                "base": {
                    "project_revision": shared_base.project_revision(),
                    "workspace_revision": workspace.workspace_revision(),
                },
                "nonclaims": [
                    "merge_result_is_a_validated_project_candidate_not_semantic_transaction_v1",
                    "no_general_multi_operation_algebra",
                    "ordered_result_not_a_commutative_merge_claim",
                    "conservative_rejection_not_incompatibility",
                    "not_behavioral_equivalence",
                    "no_runtime_or_project_test_execution",
                    "no_source_commit_or_publication_authority",
                ],
                "order": order.as_str(),
                "parents": [
                    transaction_binding("left", left, left_artifacts.candidate()),
                    transaction_binding("right", right, right_artifacts.candidate()),
                ],
                "reconciliation": {
                    "digest": reconciliation_digest,
                    "value": parse_embedded(&reconciliation, "candidate merge report is invalid JSON")?,
                },
                "result": {
                    "candidate_digest": candidate.candidate_digest(),
                    "project_revision": candidate.revision().project_revision(),
                    "workspace_revision": candidate_workspace.workspace_revision(),
                },
                "schema": SEMANTIC_TRANSACTION_MERGE_SCHEMA,
                "source_review": {
                    "digest": structural_diff.source_review_digest(),
                    "value": parse_embedded(structural_diff.source_review(), "source review is invalid JSON")?,
                },
                "structural_diff": {
                    "digest": structural_diff.digest(),
                    "value": parse_embedded(structural_diff.to_json(), "structural diff is invalid JSON")?,
                },
                "validation": {
                    "canonical_workspace_revision": true,
                    "complete_project_admission": true,
                    "fresh_parent_transaction_validation": true,
                    "ordered_candidate_merge": true,
                },
            }),
            MAX_SEMANTIC_TRANSACTION_COMPOSITION_BYTES,
        )?;
        let digest = digest(MERGE_DOMAIN, json.as_bytes());
        Ok(Self {
            candidate,
            structural_diff,
            reconciliation,
            reconciliation_digest,
            json,
            digest,
        })
    }

    pub fn replay(
        shared_base: Arc<ProjectRevision>,
        left_transaction_bytes: &[u8],
        right_transaction_bytes: &[u8],
        order: SemanticTransactionMergeOrder,
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        let left = SemanticTransaction::from_json(left_transaction_bytes)?;
        let right = SemanticTransaction::from_json(right_transaction_bytes)?;
        validate_submitted(
            bytes,
            MAX_SEMANTIC_TRANSACTION_COMPOSITION_BYTES,
            SEMANTIC_TRANSACTION_MERGE_SCHEMA,
        )?;
        require_digest(expected_digest)?;
        if digest(MERGE_DOMAIN, bytes) != expected_digest {
            return Err(stale("semantic transaction merge digest is stale"));
        }
        let derived = Self::derive(&left, &right, shared_base, order)?;
        if derived.to_json().as_bytes() != bytes || derived.digest() != expected_digest {
            return Err(stale("semantic transaction merge failed exact replay"));
        }
        Ok(derived)
    }

    pub fn candidate(&self) -> &ProjectCandidate {
        &self.candidate
    }
    pub fn structural_diff(&self) -> &SemanticWorkspaceStructuralDiff {
        &self.structural_diff
    }
    pub fn reconciliation(&self) -> &str {
        &self.reconciliation
    }
    pub fn reconciliation_digest(&self) -> &str {
        &self.reconciliation_digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl SemanticTransaction {
    pub fn rebase(
        &self,
        original_base: Arc<ProjectRevision>,
        onto: Arc<ProjectRevision>,
        expected_onto_workspace_revision: &str,
    ) -> Result<SemanticTransactionRebase> {
        SemanticTransactionRebase::derive(
            self,
            original_base,
            onto,
            expected_onto_workspace_revision,
        )
    }

    pub fn merge(
        &self,
        other: &SemanticTransaction,
        shared_base: Arc<ProjectRevision>,
        order: SemanticTransactionMergeOrder,
    ) -> Result<SemanticTransactionMerge> {
        SemanticTransactionMerge::derive(self, other, shared_base, order)
    }
}

fn revision_binding(revision: &ProjectRevision, workspace: &SemanticWorkspaceRevision) -> Value {
    json!({
        "components": {
            "dependency_lock": workspace.dependency_lock_digest(),
            "manifest": workspace.manifest_digest(),
            "semantic": workspace.semantic_digest(),
            "source_projection": workspace.source_projection_digest(),
        },
        "nodes": node_digests(workspace),
        "project_revision": revision.project_revision(),
        "workspace_revision": workspace.workspace_revision(),
    })
}

fn node_digests(workspace: &SemanticWorkspaceRevision) -> Value {
    json!({
        "agent_definitions": workspace.agent_definitions().digest(),
        "authority_policies": workspace.authority_policies().digest(),
        "contracts_and_tests": workspace.contracts_and_tests().digest(),
        "dependency_closure": workspace.dependency_closure().digest(),
        "projection_metadata": workspace.projection_metadata().digest(),
        "semantic_program": workspace.semantic_program().digest(),
        "source_projection": workspace.source_projection().digest(),
        "stable_identity_index": workspace.stable_identity_index().digest(),
        "target_profiles": workspace.target_profiles().digest(),
    })
}

fn changed_components(
    before: &SemanticWorkspaceRevision,
    after: &SemanticWorkspaceRevision,
) -> Vec<&'static str> {
    [
        (
            "dependency_lock",
            before.dependency_lock_digest(),
            after.dependency_lock_digest(),
        ),
        (
            "manifest",
            before.manifest_digest(),
            after.manifest_digest(),
        ),
        (
            "semantic",
            before.semantic_digest(),
            after.semantic_digest(),
        ),
        (
            "source_projection",
            before.source_projection_digest(),
            after.source_projection_digest(),
        ),
    ]
    .into_iter()
    .filter_map(|(name, left, right)| (left != right).then_some(name))
    .collect()
}

fn changed_nodes(
    before: &SemanticWorkspaceRevision,
    after: &SemanticWorkspaceRevision,
) -> Vec<&'static str> {
    [
        (
            "agent_definitions",
            before.agent_definitions().digest(),
            after.agent_definitions().digest(),
        ),
        (
            "authority_policies",
            before.authority_policies().digest(),
            after.authority_policies().digest(),
        ),
        (
            "contracts_and_tests",
            before.contracts_and_tests().digest(),
            after.contracts_and_tests().digest(),
        ),
        (
            "dependency_closure",
            before.dependency_closure().digest(),
            after.dependency_closure().digest(),
        ),
        (
            "projection_metadata",
            before.projection_metadata().digest(),
            after.projection_metadata().digest(),
        ),
        (
            "semantic_program",
            before.semantic_program().digest(),
            after.semantic_program().digest(),
        ),
        (
            "source_projection",
            before.source_projection().digest(),
            after.source_projection().digest(),
        ),
        (
            "stable_identity_index",
            before.stable_identity_index().digest(),
            after.stable_identity_index().digest(),
        ),
        (
            "target_profiles",
            before.target_profiles().digest(),
            after.target_profiles().digest(),
        ),
    ]
    .into_iter()
    .filter_map(|(name, left, right)| (left != right).then_some(name))
    .collect()
}

fn transaction_binding(
    side: &'static str,
    transaction: &SemanticTransaction,
    candidate: &ProjectCandidate,
) -> Value {
    json!({
        "candidate_digest": candidate.candidate_digest(),
        "side": side,
        "transaction_digest": transaction.digest(),
        "value": serde_json::from_str::<Value>(transaction.to_json())
            .expect("validated transaction retains canonical JSON"),
    })
}

fn require_candidate_parity(left: &ProjectCandidate, right: &ProjectCandidate) -> Result<()> {
    let exact_sources = left.revision().manifest().to_canonical_toml()
        == right.revision().manifest().to_canonical_toml()
        && left.revision().sources().len() == right.revision().sources().len()
        && left
            .revision()
            .sources()
            .iter()
            .zip(right.revision().sources())
            .all(|(left, right)| left.path() == right.path() && left.source() == right.source());
    if !exact_sources
        || left.revision().project_revision() != right.revision().project_revision()
        || left.candidate_digest() != right.candidate_digest()
        || left.to_json() != right.to_json()
    {
        return Err(conflict(
            "candidate rebase and reminted transaction do not have exact parity",
        ));
    }
    Ok(())
}

fn parse_embedded(source: &str, message: &'static str) -> Result<Value> {
    serde_json::from_str(source).map_err(|_| invalid(message))
}

fn validate_submitted(bytes: &[u8], limit: usize, schema: &'static str) -> Result<Value> {
    if bytes.len() > limit {
        return Err(capacity(
            "semantic transaction composition exceeds its byte limit",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("semantic transaction composition is not valid JSON"))?;
    if value.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(
            "semantic transaction composition has an invalid schema",
        ));
    }
    if render(value.clone(), limit)?.as_bytes() != bytes {
        return Err(invalid(
            "semantic transaction composition is not canonical JSON",
        ));
    }
    Ok(value)
}

fn require_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "semantic transaction composition selector is not a canonical SHA-256 digest",
        ));
    }
    Ok(())
}

fn render(mut value: Value, limit: usize) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("semantic transaction composition could not be rendered"))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(capacity(
            "semantic transaction composition exceeds its byte limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid("semantic transaction composition is not UTF-8"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn map_candidate_composition(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| match diagnostic.code {
            "SPX-G233" => Diagnostic::io("SPX-G536", diagnostic.message),
            "SPX-G234" => Diagnostic::io("SPX-G537", diagnostic.message),
            "SPX-G235" => Diagnostic::io("SPX-G539", diagnostic.message),
            _ => diagnostic,
        })
        .collect()
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G536", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G537", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G538", message)]
}
fn conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G539", message)]
}
