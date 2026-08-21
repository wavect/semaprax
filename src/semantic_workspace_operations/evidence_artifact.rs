//! Canonical outer Operations-intent Evidence and replay receipts.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::{
    limit, replay, PreparedOperationsEvidenceInput, DERIVATION_SCHEMA, SCHEMA,
    WORKSPACE_MANIFEST_SCHEMA,
};
use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace_change;

pub(super) const EVIDENCE_SCHEMA: &str = "semaprax.semantic-workspace-operations-evidence.v1";
pub(super) const VERIFICATION_RECEIPT_SCHEMA: &str =
    "semaprax.semantic-workspace-operations-evidence-verification.v1";
pub(super) const APPLICATION_RECEIPT_SCHEMA: &str =
    "semaprax.semantic-workspace-operations-evidence-application.v1";
const EVIDENCE_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-operations-evidence.artifact-digest.v1\0";

pub(super) const MAX_CHANGE_EVIDENCE_BYTES: usize = 1_048_576;
pub(super) const MAX_OPERATIONS_EVIDENCE_BYTES: usize = 4_194_304;
pub(super) const MAX_RECEIPT_BYTES: usize = 65_536;
pub(super) const MAX_TOTAL_BYTES: usize = 150_994_944;

pub(super) const NONCLAIMS: [&str; 24] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_machine_code_claim",
    "no_operations_native_context_impact_or_review",
    "no_change_v1_evidence_binding_to_operations_intent_without_this_exact_wrapper",
    "no_receipt_as_authority",
    "no_raw_path_create_delete_move_or_write",
    "no_path_set_change",
    "no_automatic_or_compiler_identity_targeting",
    "no_unmanaged_path_or_raw_tree_authority",
    "no_raw_tree_git_or_editor_atomic_visibility",
    "no_existing_generation_mutation_deletion_or_cleanup",
    "no_automatic_rollback_cleanup_or_gc",
    "no_power_loss_durability_guarantee",
    "no_network_distributed_nfs_or_overlay_guarantee",
    "no_acl_xattr_ads_preservation",
    "no_general_proof_system",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_language_graph_cleanup_backend_or_runtime_semantics",
    "no_change_v1_schema_api_or_kat_modification",
];

/// Opaque bundle binding Operations intent to unchanged Change-v1 Evidence.
pub struct SemanticWorkspaceOperationsEvidenceArtifacts {
    operations_proposal_digest: String,
    derivation: String,
    derivation_digest: String,
    derived_change_proposal: String,
    derived_change_proposal_digest: String,
    workspace_change_evidence: String,
    workspace_change_evidence_digest: String,
    operations_evidence: String,
    operations_evidence_digest: String,
}

impl SemanticWorkspaceOperationsEvidenceArtifacts {
    /// Returns the digest of the exact Operations proposal.
    pub fn operations_proposal_digest(&self) -> &str {
        &self.operations_proposal_digest
    }
    /// Returns the canonical Operations derivation wrapper, including its LF.
    pub fn derivation(&self) -> &str {
        &self.derivation
    }
    /// Returns the digest of the Operations derivation wrapper.
    pub fn derivation_digest(&self) -> &str {
        &self.derivation_digest
    }
    /// Returns the unchanged canonical derived Change-v1 proposal.
    pub fn derived_change_proposal(&self) -> &str {
        &self.derived_change_proposal
    }
    /// Returns the unchanged Change-v1 proposal digest.
    pub fn derived_change_proposal_digest(&self) -> &str {
        &self.derived_change_proposal_digest
    }
    /// Returns the exact embedded Change-v1 Evidence document.
    pub fn workspace_change_evidence(&self) -> &str {
        &self.workspace_change_evidence
    }
    /// Returns the unchanged Change-v1 Evidence artifact digest.
    pub fn workspace_change_evidence_digest(&self) -> &str {
        &self.workspace_change_evidence_digest
    }
    /// Returns the canonical outer Operations Evidence, including its LF.
    pub fn operations_evidence(&self) -> &str {
        &self.operations_evidence
    }
    /// Returns the domain-separated Operations Evidence digest.
    pub fn operations_evidence_digest(&self) -> &str {
        &self.operations_evidence_digest
    }
    pub(super) fn into_operations_evidence(self) -> String {
        self.operations_evidence
    }
}

#[derive(Clone, Copy)]
struct Usage {
    operations_proposal: usize,
    derivation: usize,
    change_total: usize,
    change_evidence: usize,
    operations_evidence: usize,
    receipt: usize,
    total: usize,
    retained_generations: usize,
    staging_attempts: usize,
}

pub(super) fn render_evidence(
    prepared: &PreparedOperationsEvidenceInput,
    change_artifacts: &semantic_workspace_change::SemanticWorkspaceChangeArtifacts,
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, Vec<Diagnostic>> {
    render_evidence_with_limits(
        prepared,
        change_artifacts,
        MAX_OPERATIONS_EVIDENCE_BYTES,
        MAX_TOTAL_BYTES,
    )
}

fn render_evidence_with_limits(
    prepared: &PreparedOperationsEvidenceInput,
    change_artifacts: &semantic_workspace_change::SemanticWorkspaceChangeArtifacts,
    evidence_limit: usize,
    total_limit: usize,
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, Vec<Diagnostic>> {
    let workspace_change_evidence = change_artifacts.evidence_bytes();
    if workspace_change_evidence.len() > MAX_CHANGE_EVIDENCE_BYTES {
        return Err(limit(
            "workspace_change_evidence_bytes",
            MAX_CHANGE_EVIDENCE_BYTES,
        ));
    }
    let change_total = change_artifacts
        .total_artifact_bytes(prepared.change.proposal_source().len())
        .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
    let fixed = prepared
        .operations_proposal
        .len()
        .checked_add(prepared.derivation.derivation().len())
        .and_then(|value| value.checked_add(change_total))
        .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
    let aggregate_remaining = total_limit
        .checked_sub(fixed)
        .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
    let output_limit = aggregate_remaining.min(evidence_limit);
    let mut exact_bytes = 0usize;
    let mut operations_evidence = None;
    for _ in 0..24 {
        let total = fixed
            .checked_add(exact_bytes)
            .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
        let usage = Usage {
            operations_proposal: prepared.operations_proposal.len(),
            derivation: prepared.derivation.derivation().len(),
            change_total,
            change_evidence: workspace_change_evidence.len(),
            operations_evidence: exact_bytes,
            receipt: 0,
            total,
            retained_generations: prepared.change.retained_generations(),
            staging_attempts: prepared.change.staging_attempts(),
        };
        let (document, overflowed) = crate::bounded_output::with_limit(output_limit, || {
            render_evidence_document(prepared, change_artifacts, usage)
        });
        if overflowed {
            return Err(if aggregate_remaining < evidence_limit {
                limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES)
            } else {
                limit("operations_evidence_bytes", MAX_OPERATIONS_EVIDENCE_BYTES)
            });
        }
        if document.len() == exact_bytes {
            operations_evidence = Some(document);
            break;
        }
        exact_bytes = document.len();
    }
    let operations_evidence = operations_evidence.ok_or_else(replay)?;
    let operations_evidence_digest = digest(EVIDENCE_DOMAIN, operations_evidence.as_bytes());
    Ok(SemanticWorkspaceOperationsEvidenceArtifacts {
        operations_proposal_digest: prepared.operations_proposal_digest.clone(),
        derivation: prepared.derivation.derivation().to_owned(),
        derivation_digest: prepared.derivation.derivation_digest().to_owned(),
        derived_change_proposal: prepared.derivation.derived_change_proposal().to_owned(),
        derived_change_proposal_digest: prepared
            .derivation
            .derived_change_proposal_digest()
            .to_owned(),
        workspace_change_evidence: workspace_change_evidence.to_owned(),
        workspace_change_evidence_digest: change_artifacts.evidence_artifact_digest().to_owned(),
        operations_evidence,
        operations_evidence_digest,
    })
}

#[cfg(test)]
pub(super) fn render_evidence_with_test_limits(
    prepared: &PreparedOperationsEvidenceInput,
    change_artifacts: &semantic_workspace_change::SemanticWorkspaceChangeArtifacts,
    evidence_limit: usize,
    total_limit: usize,
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, Vec<Diagnostic>> {
    assert!(evidence_limit <= MAX_OPERATIONS_EVIDENCE_BYTES);
    assert!(total_limit <= MAX_TOTAL_BYTES);
    render_evidence_with_limits(prepared, change_artifacts, evidence_limit, total_limit)
}

fn render_evidence_document(
    prepared: &PreparedOperationsEvidenceInput,
    change_artifacts: &semantic_workspace_change::SemanticWorkspaceChangeArtifacts,
    usage: Usage,
) -> String {
    let mut out = CappedString::new();
    out.push_str("{\"schema\":");
    json(&mut out, EVIDENCE_SCHEMA);
    out.push_str(",\"workspace_manifest_schema\":");
    json(&mut out, WORKSPACE_MANIFEST_SCHEMA);
    out.push_str(",\"base_workspace_revision\":");
    json(&mut out, prepared.change.base_workspace_revision());
    out.push_str(",\"candidate_workspace_revision\":");
    json(&mut out, prepared.change.candidate_workspace_revision());
    out.push_str(",\"entry_module\":");
    json(&mut out, prepared.change.entry_module());
    out.push_str(",\"operations_proposal\":");
    reference(
        &mut out,
        SCHEMA,
        &prepared.operations_proposal_digest,
        prepared.operations_proposal.len(),
    );
    out.push_str(",\"operations_derivation\":");
    reference(
        &mut out,
        DERIVATION_SCHEMA,
        prepared.derivation.derivation_digest(),
        prepared.derivation.derivation().len(),
    );
    out.push_str(",\"derived_workspace_change_proposal\":");
    reference(
        &mut out,
        semantic_workspace_change::SCHEMA,
        prepared.derivation.derived_change_proposal_digest(),
        prepared.derivation.derived_change_proposal().len(),
    );
    out.push_str(",\"workspace_change_evidence\":{\"schema\":");
    json(&mut out, semantic_workspace_change::EVIDENCE_SCHEMA);
    out.push_str(",\"digest\":");
    json(&mut out, change_artifacts.evidence_artifact_digest());
    write!(
        out,
        ",\"bytes\":{}",
        change_artifacts.evidence_bytes().len()
    )
    .expect("string writes cannot fail");
    out.push_str(",\"document\":");
    json(&mut out, change_artifacts.evidence_bytes());
    out.push('}');
    push_limits(&mut out);
    push_budget(&mut out, usage);
    push_nonclaims(&mut out);
    out.push_str("}\n");
    out.into_string()
}

pub(super) fn render_receipt(
    prepared: &PreparedOperationsEvidenceInput,
    artifacts: &SemanticWorkspaceOperationsEvidenceArtifacts,
    replay_token: &ExactOperationsEvidenceReplay,
    application: bool,
) -> Result<String, Vec<Diagnostic>> {
    render_receipt_with_limits(
        prepared,
        artifacts,
        replay_token,
        application,
        MAX_RECEIPT_BYTES,
        MAX_TOTAL_BYTES,
    )
}

fn render_receipt_with_limits(
    prepared: &PreparedOperationsEvidenceInput,
    artifacts: &SemanticWorkspaceOperationsEvidenceArtifacts,
    replay_token: &ExactOperationsEvidenceReplay,
    application: bool,
    receipt_limit: usize,
    total_limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    let submitted_evidence_bytes = replay_token.bytes;
    if submitted_evidence_bytes != artifacts.operations_evidence.len() {
        return Err(replay());
    }
    let change_total = replay_token.change_total;
    let without_receipt = prepared
        .operations_proposal
        .len()
        .checked_add(prepared.derivation.derivation().len())
        .and_then(|value| value.checked_add(change_total))
        .and_then(|value| value.checked_add(submitted_evidence_bytes))
        .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
    let aggregate_remaining = total_limit
        .checked_sub(without_receipt)
        .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
    let output_limit = aggregate_remaining.min(receipt_limit);
    let mut exact_bytes = 0usize;
    for _ in 0..24 {
        let total = without_receipt
            .checked_add(exact_bytes)
            .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
        let usage = Usage {
            operations_proposal: prepared.operations_proposal.len(),
            derivation: prepared.derivation.derivation().len(),
            change_total,
            change_evidence: artifacts.workspace_change_evidence.len(),
            operations_evidence: submitted_evidence_bytes,
            receipt: exact_bytes,
            total,
            retained_generations: prepared.change.retained_generations(),
            staging_attempts: prepared.change.staging_attempts(),
        };
        let (document, overflowed) = crate::bounded_output::with_limit(output_limit, || {
            render_receipt_document(prepared, artifacts, usage, application)
        });
        if overflowed {
            return Err(if aggregate_remaining < receipt_limit {
                limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES)
            } else {
                limit("receipt_bytes", MAX_RECEIPT_BYTES)
            });
        }
        if document.len() == exact_bytes {
            return Ok(document);
        }
        exact_bytes = document.len();
    }
    Err(replay())
}

#[cfg(test)]
pub(super) fn render_receipt_with_test_limits(
    prepared: &PreparedOperationsEvidenceInput,
    artifacts: &SemanticWorkspaceOperationsEvidenceArtifacts,
    replay_token: &ExactOperationsEvidenceReplay,
    application: bool,
    receipt_limit: usize,
    total_limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    assert!(receipt_limit <= MAX_RECEIPT_BYTES);
    assert!(total_limit <= MAX_TOTAL_BYTES);
    render_receipt_with_limits(
        prepared,
        artifacts,
        replay_token,
        application,
        receipt_limit,
        total_limit,
    )
}

pub(super) struct ExactOperationsEvidenceReplay {
    bytes: usize,
    change_total: usize,
    pub(super) operations_proposal_digest: String,
    pub(super) derivation_digest: String,
    pub(super) derived_change_proposal_digest: String,
    pub(super) workspace_change_evidence_digest: String,
    pub(super) operations_evidence_digest: String,
}

pub(super) fn exact_replay_token(
    prepared: &PreparedOperationsEvidenceInput,
    change_artifacts: &semantic_workspace_change::SemanticWorkspaceChangeArtifacts,
    artifacts: &SemanticWorkspaceOperationsEvidenceArtifacts,
    submitted: &str,
) -> Result<ExactOperationsEvidenceReplay, Vec<Diagnostic>> {
    if submitted != artifacts.operations_evidence() {
        return Err(super::operations_evidence_replay());
    }
    let change_total = change_artifacts
        .total_artifact_bytes(prepared.change.proposal_source().len())
        .ok_or_else(|| limit("total_operations_artifact_bytes", MAX_TOTAL_BYTES))?;
    Ok(ExactOperationsEvidenceReplay {
        bytes: submitted.len(),
        change_total,
        operations_proposal_digest: artifacts.operations_proposal_digest().to_owned(),
        derivation_digest: artifacts.derivation_digest().to_owned(),
        derived_change_proposal_digest: artifacts.derived_change_proposal_digest().to_owned(),
        workspace_change_evidence_digest: artifacts.workspace_change_evidence_digest().to_owned(),
        operations_evidence_digest: artifacts.operations_evidence_digest().to_owned(),
    })
}

fn render_receipt_document(
    prepared: &PreparedOperationsEvidenceInput,
    artifacts: &SemanticWorkspaceOperationsEvidenceArtifacts,
    usage: Usage,
    application: bool,
) -> String {
    let mut out = CappedString::new();
    out.push_str("{\"schema\":");
    json(
        &mut out,
        if application {
            APPLICATION_RECEIPT_SCHEMA
        } else {
            VERIFICATION_RECEIPT_SCHEMA
        },
    );
    out.push_str(",\"result\":");
    json(
        &mut out,
        if application {
            "applied"
        } else {
            "exact_replay"
        },
    );
    out.push_str(",\"workspace_manifest_schema\":");
    json(&mut out, WORKSPACE_MANIFEST_SCHEMA);
    out.push_str(",\"base_workspace_revision\":");
    json(&mut out, prepared.change.base_workspace_revision());
    out.push_str(",\"candidate_workspace_revision\":");
    json(&mut out, prepared.change.candidate_workspace_revision());
    out.push_str(",\"entry_module\":");
    json(&mut out, prepared.change.entry_module());
    out.push_str(",\"operations_proposal\":");
    reference(
        &mut out,
        SCHEMA,
        artifacts.operations_proposal_digest(),
        usage.operations_proposal,
    );
    out.push_str(",\"operations_derivation\":");
    reference(
        &mut out,
        DERIVATION_SCHEMA,
        artifacts.derivation_digest(),
        usage.derivation,
    );
    out.push_str(",\"derived_workspace_change_proposal\":");
    reference(
        &mut out,
        semantic_workspace_change::SCHEMA,
        artifacts.derived_change_proposal_digest(),
        artifacts.derived_change_proposal().len(),
    );
    out.push_str(",\"workspace_change_evidence\":");
    reference(
        &mut out,
        semantic_workspace_change::EVIDENCE_SCHEMA,
        artifacts.workspace_change_evidence_digest(),
        usage.change_evidence,
    );
    out.push_str(",\"workspace_operations_evidence\":");
    reference(
        &mut out,
        EVIDENCE_SCHEMA,
        artifacts.operations_evidence_digest(),
        usage.operations_evidence,
    );
    push_limits(&mut out);
    push_budget(&mut out, usage);
    push_nonclaims(&mut out);
    out.push_str("}\n");
    out.into_string()
}

fn reference(out: &mut CappedString, schema: &str, digest: &str, bytes: usize) {
    out.push_str("{\"schema\":");
    json(out, schema);
    out.push_str(",\"digest\":");
    json(out, digest);
    write!(out, ",\"bytes\":{bytes}}}").expect("string writes cannot fail");
}

fn push_limits(out: &mut CappedString) {
    out.push_str(",\"limits\":{\"max_workspace_change_evidence_bytes\":1048576,\"max_operations_evidence_bytes\":4194304,\"max_receipt_bytes\":65536,\"max_total_operations_artifact_bytes\":150994944,\"max_json_depth\":8,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}");
}

fn push_budget(out: &mut CappedString, usage: Usage) {
    write!(
        out,
        ",\"budget\":{{\"used_operations_proposal_bytes\":{},\"used_derivation_bytes\":{},\"used_workspace_change_total_artifact_bytes\":{},\"used_workspace_change_evidence_bytes\":{},\"used_operations_evidence_bytes\":{},\"used_receipt_bytes\":{},\"used_total_operations_artifact_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":0}}",
        usage.operations_proposal,
        usage.derivation,
        usage.change_total,
        usage.change_evidence,
        usage.operations_evidence,
        usage.receipt,
        usage.total,
        usage.retained_generations,
        usage.staging_attempts,
    )
    .expect("string writes cannot fail");
}

fn push_nonclaims(out: &mut CappedString) {
    out.push_str(",\"nonclaims\":[");
    for (index, claim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        json(out, claim);
    }
    out.push(']');
}

fn json(out: &mut CappedString, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character.is_control() => {
                write!(out, "\\u{:04x}", character as u32).expect("string writes cannot fail");
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}
