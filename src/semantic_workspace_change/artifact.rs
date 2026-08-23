//! Canonical, bounded, private artifacts derived from one prepared change.

use std::collections::BTreeSet;
use std::fmt::Write as _;
#[cfg(test)]
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    incomplete, limit, replay, SemanticWorkspaceChangeEdge, SemanticWorkspaceChangeImpactEdge,
    SemanticWorkspaceChangeImpactFact, SemanticWorkspaceChangeRoot,
    SemanticWorkspaceChangedFileFact, SemanticWorkspacePreparedChange, MAX_CHANGED_FILES,
    MAX_DELTA_EDGES, MAX_DELTA_ROOTS, MAX_ENTRY_MODULE_BYTES, MAX_IMPACT_DEPTH, MAX_IMPACT_NODES,
    MAX_IMPACT_PROVENANCE, MAX_PROPOSAL_BYTES, MAX_SOURCE_BYTES_PER_CHANGE,
    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
};
#[cfg(test)]
use super::{with_authenticated_change, SemanticWorkspaceChangeSet};
use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::{semantic_workspace, workspace_graph};

const PREVIEW_SCHEMA: &str = "semaprax.workspace-semantic-change-preview.v1";
const CONTEXT_SCHEMA: &str = "semaprax.workspace-semantic-change-context.v1";
const IMPACT_SCHEMA: &str = "semaprax.workspace-semantic-change-impact.v1";
const REVIEW_SCHEMA: &str = "semaprax.workspace-semantic-change-review.v1";
pub(super) const EVIDENCE_SCHEMA: &str = "semaprax.workspace-semantic-change-evidence.v1";
pub(super) const RECEIPT_SCHEMA: &str =
    "semaprax.workspace-semantic-change-evidence-verification.v1";
const APPLICATION_RECEIPT_SCHEMA: &str =
    "semaprax.workspace-semantic-change-evidence-application.v1";
const GRAPH_SCHEMA: &str = "semaprax.workspace-semantic-graph.v1";
const MANIFEST_SCHEMA: &str = "semaprax.workspace-semantic-manifest.v1";

const PROPOSAL_DIGEST_DOMAIN: &[u8] = b"semaprax.workspace-semantic-change.proposal-digest.v1\0";
const CANDIDATE_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-change.candidate-manifest-digest.v1\0";
const PREVIEW_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-change-preview.artifact-digest.v1\0";
const CONTEXT_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-change-context.artifact-digest.v1\0";
const IMPACT_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-change-impact.artifact-digest.v1\0";
const REVIEW_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-change-review.artifact-digest.v1\0";
pub(super) const EVIDENCE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-change-evidence.artifact-digest.v1\0";

pub(crate) fn digest_evidence(source: &str) -> String {
    digest(EVIDENCE_DIGEST_DOMAIN, source.as_bytes())
}

const MAX_TOTAL_BASE_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_CANDIDATE_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CANDIDATE_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_CONTEXT_NODES: usize = 16_384;
const MAX_ANALYSIS_BUILDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 32 * 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMPACT_BYTES: usize = 32 * 1024 * 1024;
const MAX_REVIEW_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_EVIDENCE_BYTES: usize = 1024 * 1024;
pub(super) const MAX_RECEIPT_BYTES: usize = 65_536;
const MAX_TOTAL_ARTIFACT_BYTES: usize = 96 * 1024 * 1024;

const NONCLAIMS: [&str; 19] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_machine_code_claim",
    "no_current_state_context_impact_or_review_reuse",
    "no_create_delete_move_or_path_set_change",
    "no_unmanaged_path_or_raw_tree_authority",
    "no_raw_tree_git_or_editor_atomic_visibility",
    "no_commit_authority_in_preview_context_impact_review_or_evidence",
    "no_automatic_rollback_cleanup_or_gc",
    "no_power_loss_durability_guarantee",
    "no_network_distributed_nfs_or_overlay_guarantee",
    "no_acl_xattr_ads_preservation",
    "no_general_proof_system",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_language_graph_cleanup_backend_or_runtime_semantics",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct Artifact {
    schema: &'static str,
    digest: String,
    bytes: String,
}

#[derive(Clone, Copy)]
struct ChildRefs<'a> {
    proposal_digest: &'a str,
    candidate_manifest_digest: &'a str,
    preview: &'a Artifact,
    context: &'a Artifact,
    impact: &'a Artifact,
    review: Option<&'a Artifact>,
}

#[derive(Clone, Copy)]
struct EvidenceOffsets {
    delta_edges: usize,
    affected: usize,
    dependency_edges: usize,
    end: usize,
}

impl EvidenceOffsets {
    fn new(prepared: &SemanticWorkspacePreparedChange) -> Self {
        let delta_edges = prepared.roots.len();
        let context_nodes = delta_edges + prepared.delta_edges.len();
        let affected = context_nodes + prepared.context_nodes.len();
        let dependency_edges = affected + prepared.impact.len();
        let end = dependency_edges + prepared.impact_edges.len();
        Self {
            delta_edges,
            affected,
            dependency_edges,
            end,
        }
    }
}

fn push_review_evidence(output: &mut CappedString, prepared: &SemanticWorkspacePreparedChange) {
    let groups = [
        ("change_preview", "delta_root", prepared.roots.len()),
        ("change_preview", "delta_edge", prepared.delta_edges.len()),
        ("context", "context_node", prepared.context_nodes.len()),
        ("impact", "affected", prepared.impact.len()),
        ("impact", "dependency_edge", prepared.impact_edges.len()),
    ];
    let mut first = true;
    for (artifact, relation, count) in groups {
        for index in 0..count {
            if !first {
                output.push(',');
            }
            first = false;
            output.push_str("{\"artifact\":");
            push_json(output, artifact);
            write!(output, ",\"index\":{index}").expect("string writes cannot fail");
            output.push_str(",\"relation\":");
            push_json(output, relation);
            output.push('}');
        }
    }
}

/// Opaque public bundle of canonical read-only change artifacts and digests.
pub struct SemanticWorkspaceChangeArtifacts {
    proposal_digest: String,
    candidate_manifest_digest: String,
    preview: Artifact,
    context: Artifact,
    impact: Artifact,
    review: Artifact,
    evidence: Artifact,
}

impl SemanticWorkspaceChangeArtifacts {
    pub fn proposal_digest(&self) -> &str {
        &self.proposal_digest
    }

    pub fn candidate_manifest_digest(&self) -> &str {
        &self.candidate_manifest_digest
    }

    pub fn preview(&self) -> &str {
        &self.preview.bytes
    }

    pub fn preview_digest(&self) -> &str {
        &self.preview.digest
    }

    pub fn context(&self) -> &str {
        &self.context.bytes
    }

    pub fn context_digest(&self) -> &str {
        &self.context.digest
    }

    pub fn impact(&self) -> &str {
        &self.impact.bytes
    }

    pub fn impact_digest(&self) -> &str {
        &self.impact.digest
    }

    pub fn review(&self) -> &str {
        &self.review.bytes
    }

    pub fn review_digest(&self) -> &str {
        &self.review.digest
    }

    pub fn evidence(&self) -> &str {
        &self.evidence.bytes
    }

    pub fn evidence_digest(&self) -> &str {
        &self.evidence.digest
    }

    pub(super) fn into_preview(mut self) -> String {
        std::mem::take(&mut self.preview.bytes)
    }

    pub(super) fn into_evidence(mut self) -> String {
        std::mem::take(&mut self.evidence.bytes)
    }

    pub(crate) fn evidence_bytes(&self) -> &str {
        &self.evidence.bytes
    }

    pub(crate) fn evidence_artifact_digest(&self) -> &str {
        &self.evidence.digest
    }

    pub(crate) fn total_artifact_bytes(&self, proposal_bytes: usize) -> Option<usize> {
        [
            proposal_bytes,
            self.preview.bytes.len(),
            self.context.bytes.len(),
            self.impact.bytes.len(),
            self.review.bytes.len(),
            self.evidence.bytes.len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
    }
}

#[cfg(test)]
pub(crate) fn build_authenticated_artifacts(
    root: &Path,
    change_set: SemanticWorkspaceChangeSet,
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    with_authenticated_change(root, change_set, |prepared| render_artifacts(&prepared))
}

#[cfg(test)]
fn build_authenticated_artifacts_with_hook(
    root: &Path,
    change_set: SemanticWorkspaceChangeSet,
    after_render: impl FnOnce(&SemanticWorkspaceChangeArtifacts),
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    with_authenticated_change(root, change_set, |prepared| {
        let artifacts = render_artifacts(&prepared)?;
        after_render(&artifacts);
        Ok(artifacts)
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ArtifactSizes {
    preview: usize,
    context: usize,
    impact: usize,
    review: usize,
    evidence: usize,
}

#[derive(Clone, Copy)]
struct Usage {
    managed_files: usize,
    changed_files: usize,
    total_base_source_bytes: usize,
    total_candidate_source_bytes: usize,
    total_replacement_source_bytes: usize,
    entry_module_bytes: usize,
    proposal_bytes: usize,
    candidate_manifest_bytes: usize,
    delta_roots: usize,
    delta_edges: usize,
    context_nodes: usize,
    impact_nodes: usize,
    impact_provenance: usize,
    impact_depth: usize,
    analysis_builder_bytes: usize,
    sizes: ArtifactSizes,
    receipt_bytes: usize,
    total_artifact_bytes: usize,
    retained_generations: usize,
    staging_attempts: usize,
}

pub(crate) fn render_artifacts(
    prepared: &SemanticWorkspacePreparedChange,
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    replay_prepared(prepared)?;
    let proposal_digest = digest(PROPOSAL_DIGEST_DOMAIN, prepared.proposal_source.as_bytes());
    let candidate_manifest_digest = digest(
        CANDIDATE_MANIFEST_DIGEST_DOMAIN,
        prepared.candidate_manifest.as_bytes(),
    );
    let mut sizes = ArtifactSizes::default();
    for _ in 0..24 {
        let usage = usage(prepared, sizes, 0)?;
        let mut remaining = MAX_TOTAL_ARTIFACT_BYTES
            .checked_sub(prepared.proposal_source.len())
            .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
        let preview = artifact_bounded(
            PREVIEW_SCHEMA,
            PREVIEW_DIGEST_DOMAIN,
            MAX_PREVIEW_BYTES,
            "change_preview_bytes",
            remaining,
            |output| {
                render_preview(
                    output,
                    prepared,
                    &proposal_digest,
                    &candidate_manifest_digest,
                    usage,
                )
            },
        )?;
        remaining = remaining
            .checked_sub(preview.bytes.len())
            .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
        let context = artifact_bounded(
            CONTEXT_SCHEMA,
            CONTEXT_DIGEST_DOMAIN,
            MAX_CONTEXT_BYTES,
            "context_bytes",
            remaining,
            |output| render_context(output, prepared, &proposal_digest, &preview, usage),
        )?;
        remaining = remaining
            .checked_sub(context.bytes.len())
            .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
        let impact = artifact_bounded(
            IMPACT_SCHEMA,
            IMPACT_DIGEST_DOMAIN,
            MAX_IMPACT_BYTES,
            "impact_bytes",
            remaining,
            |output| {
                render_impact(
                    output,
                    prepared,
                    &proposal_digest,
                    &preview,
                    &context,
                    usage,
                )
            },
        )?;
        remaining = remaining
            .checked_sub(impact.bytes.len())
            .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
        let children = ChildRefs {
            proposal_digest: &proposal_digest,
            candidate_manifest_digest: &candidate_manifest_digest,
            preview: &preview,
            context: &context,
            impact: &impact,
            review: None,
        };
        let review = artifact_bounded(
            REVIEW_SCHEMA,
            REVIEW_DIGEST_DOMAIN,
            MAX_REVIEW_BYTES,
            "review_bytes",
            remaining,
            |output| render_review(output, prepared, children, usage),
        )?;
        remaining = remaining
            .checked_sub(review.bytes.len())
            .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
        let children = ChildRefs {
            review: Some(&review),
            ..children
        };
        let evidence = artifact_bounded(
            EVIDENCE_SCHEMA,
            EVIDENCE_DIGEST_DOMAIN,
            MAX_EVIDENCE_BYTES,
            "evidence_bytes",
            remaining,
            |output| render_evidence(output, prepared, children, usage),
        )?;
        let artifacts = SemanticWorkspaceChangeArtifacts {
            proposal_digest: proposal_digest.clone(),
            candidate_manifest_digest: candidate_manifest_digest.clone(),
            preview,
            context,
            impact,
            review,
            evidence,
        };
        let next_sizes = ArtifactSizes {
            preview: artifacts.preview.bytes.len(),
            context: artifacts.context.bytes.len(),
            impact: artifacts.impact.bytes.len(),
            review: artifacts.review.bytes.len(),
            evidence: artifacts.evidence.bytes.len(),
        };
        if next_sizes == sizes {
            verify_artifact_bindings(&artifacts)?;
            return Ok(artifacts);
        }
        sizes = next_sizes;
    }
    Err(replay(
        "Semantic Workspace Change artifact budget fixed point disagrees",
    ))
}

pub(super) fn render_verification_receipt(
    prepared: &SemanticWorkspacePreparedChange,
    artifacts: &SemanticWorkspaceChangeArtifacts,
    submitted_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    render_receipt_bounded(
        prepared,
        artifacts,
        submitted_evidence_bytes,
        RECEIPT_SCHEMA,
        "exact_replay",
    )
}

pub(super) fn render_application_receipt(
    prepared: &SemanticWorkspacePreparedChange,
    artifacts: &SemanticWorkspaceChangeArtifacts,
    submitted_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    render_receipt_bounded(
        prepared,
        artifacts,
        submitted_evidence_bytes,
        APPLICATION_RECEIPT_SCHEMA,
        "applied",
    )
}

fn render_receipt_bounded(
    prepared: &SemanticWorkspacePreparedChange,
    artifacts: &SemanticWorkspaceChangeArtifacts,
    submitted_evidence_bytes: usize,
    schema: &str,
    result: &str,
) -> Result<String, Vec<Diagnostic>> {
    if submitted_evidence_bytes != artifacts.evidence.bytes.len() {
        return Err(evidence_replay());
    }
    let sizes = ArtifactSizes {
        preview: artifacts.preview.bytes.len(),
        context: artifacts.context.bytes.len(),
        impact: artifacts.impact.bytes.len(),
        review: artifacts.review.bytes.len(),
        evidence: submitted_evidence_bytes,
    };
    let without_receipt = [
        prepared.proposal_source.len(),
        sizes.preview,
        sizes.context,
        sizes.impact,
        sizes.review,
        sizes.evidence,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
    let aggregate_remaining = MAX_TOTAL_ARTIFACT_BYTES
        .checked_sub(without_receipt)
        .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
    let receipt_limit = MAX_RECEIPT_BYTES.min(aggregate_remaining);
    let mut receipt_bytes = 0usize;
    for _ in 0..24 {
        let usage = usage(prepared, sizes, receipt_bytes)?;
        let (receipt, overflowed) = crate::bounded_output::with_limit(receipt_limit, || {
            let mut output = CappedString::new();
            render_receipt(&mut output, prepared, artifacts, usage, schema, result);
            output.into_string()
        });
        if overflowed {
            return if aggregate_remaining < MAX_RECEIPT_BYTES {
                Err(limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))
            } else {
                Err(limit("receipt_bytes", MAX_RECEIPT_BYTES))
            };
        }
        if receipt.len() == receipt_bytes {
            return Ok(receipt);
        }
        receipt_bytes = receipt.len();
    }
    Err(replay(
        "Semantic Workspace Change verification receipt budget fixed point disagrees",
    ))
}

fn evidence_replay() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G187",
        "Semantic Workspace Change Evidence does not exactly replay the authenticated proposal and candidate",
    )]
}

fn usage(
    prepared: &SemanticWorkspacePreparedChange,
    sizes: ArtifactSizes,
    receipt_bytes: usize,
) -> Result<Usage, Vec<Diagnostic>> {
    validate_frozen_usage(prepared)?;
    let total_base_source_bytes = checked_sum(
        prepared.base_files.iter().map(|file| file.bytes),
        "total_base_source_bytes",
        MAX_TOTAL_BASE_SOURCE_BYTES,
    )?;
    let total_candidate_source_bytes = checked_sum(
        prepared.candidate_files.iter().map(|file| file.bytes()),
        "total_candidate_source_bytes",
        MAX_TOTAL_CANDIDATE_SOURCE_BYTES,
    )?;
    let impact_provenance = prepared
        .impact
        .iter()
        .try_fold(0usize, |total, fact| {
            total.checked_add(fact.root_provenance.len())
        })
        .ok_or_else(|| incomplete("Semantic Workspace Change provenance is incomplete"))?;
    if impact_provenance > MAX_IMPACT_PROVENANCE {
        return Err(incomplete(
            "Semantic Workspace Change provenance is incomplete",
        ));
    }
    let impact_depth = prepared
        .impact
        .iter()
        .map(|fact| fact.minimum_depth)
        .max()
        .unwrap_or(0);
    if impact_depth > MAX_IMPACT_DEPTH {
        return Err(incomplete(
            "Semantic Workspace Change impact depth is incomplete",
        ));
    }
    let total_artifact_bytes = [
        prepared.proposal_source.len(),
        sizes.preview,
        sizes.context,
        sizes.impact,
        sizes.review,
        sizes.evidence,
        receipt_bytes,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
    if total_artifact_bytes > MAX_TOTAL_ARTIFACT_BYTES {
        return Err(limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES));
    }
    Ok(Usage {
        managed_files: prepared.base_files.len(),
        changed_files: prepared.changed_files.len(),
        total_base_source_bytes,
        total_candidate_source_bytes,
        total_replacement_source_bytes: prepared.used_total_replacement_source_bytes,
        entry_module_bytes: prepared.entry_module.len(),
        proposal_bytes: prepared.proposal_source.len(),
        candidate_manifest_bytes: prepared.candidate_manifest.len(),
        delta_roots: prepared.roots.len(),
        delta_edges: prepared.delta_edges.len(),
        context_nodes: prepared.context_nodes.len(),
        impact_nodes: prepared.impact.len(),
        impact_provenance,
        impact_depth,
        analysis_builder_bytes: prepared.used_builder_bytes,
        sizes,
        receipt_bytes,
        total_artifact_bytes,
        retained_generations: prepared.retained_generations,
        staging_attempts: prepared.staging_attempts,
    })
}

fn validate_frozen_usage(
    prepared: &SemanticWorkspacePreparedChange,
) -> Result<(), Vec<Diagnostic>> {
    for (used, field, maximum) in [
        (
            prepared.base_files.len(),
            "managed_files",
            semantic_workspace::MAX_MANAGED_FILES,
        ),
        (
            prepared.changed_files.len(),
            "changed_files",
            MAX_CHANGED_FILES,
        ),
        (
            prepared.used_total_replacement_source_bytes,
            "total_replacement_source_bytes",
            MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
        ),
        (
            prepared.entry_module.len(),
            "entry_module_bytes",
            MAX_ENTRY_MODULE_BYTES,
        ),
        (
            prepared.proposal_source.len(),
            "proposal_bytes",
            MAX_PROPOSAL_BYTES,
        ),
        (
            prepared.candidate_manifest.len(),
            "candidate_manifest_bytes",
            MAX_CANDIDATE_MANIFEST_BYTES,
        ),
        (prepared.roots.len(), "delta_roots", MAX_DELTA_ROOTS),
        (prepared.delta_edges.len(), "delta_edges", MAX_DELTA_EDGES),
        (
            prepared.used_builder_bytes,
            "analysis_builder_bytes",
            MAX_ANALYSIS_BUILDER_BYTES,
        ),
        (
            prepared.retained_generations,
            "retained_generations",
            crate::workspace::MAX_RETAINED_GENERATIONS,
        ),
        (
            prepared.staging_attempts,
            "staging_attempts",
            crate::workspace::MAX_STAGING_ATTEMPTS,
        ),
    ] {
        if used > maximum {
            return Err(limit(field, maximum));
        }
    }
    if prepared.base_files.len() < 2 || prepared.changed_files.len() < 2 {
        return Err(replay(
            "Semantic Workspace Change artifact source cardinality disagrees",
        ));
    }
    if prepared.context_nodes.len() > MAX_CONTEXT_NODES {
        return Err(incomplete(
            "Semantic Workspace Change Context node closure is incomplete",
        ));
    }
    if prepared.impact.len() > MAX_IMPACT_NODES || prepared.impact_edges.len() > MAX_DELTA_EDGES {
        return Err(incomplete(
            "Semantic Workspace Change impact closure is incomplete",
        ));
    }
    Ok(())
}

fn checked_sum(
    values: impl IntoIterator<Item = usize>,
    field: &'static str,
    maximum: usize,
) -> Result<usize, Vec<Diagnostic>> {
    let sum = values
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| limit(field, maximum))?;
    if sum > maximum {
        Err(limit(field, maximum))
    } else {
        Ok(sum)
    }
}

#[cfg(test)]
fn artifact(
    schema: &'static str,
    domain: &[u8],
    maximum: usize,
    field: &'static str,
    render: impl FnOnce(&mut CappedString),
) -> Result<Artifact, Vec<Diagnostic>> {
    artifact_bounded(schema, domain, maximum, field, maximum, render)
}

fn artifact_bounded(
    schema: &'static str,
    domain: &[u8],
    maximum: usize,
    field: &'static str,
    aggregate_remaining: usize,
    render: impl FnOnce(&mut CappedString),
) -> Result<Artifact, Vec<Diagnostic>> {
    let effective_limit = maximum.min(aggregate_remaining);
    let (bytes, overflowed) = crate::bounded_output::with_limit(effective_limit, || {
        let mut output = CappedString::new();
        render(&mut output);
        output.into_string()
    });
    if overflowed || bytes.len() > effective_limit {
        return Err(if aggregate_remaining < maximum {
            limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES)
        } else {
            limit(field, maximum)
        });
    }
    if !bytes.ends_with('\n') || bytes[..bytes.len().saturating_sub(1)].contains('\n') {
        return Err(replay(
            "Semantic Workspace Change artifact line binding disagrees",
        ));
    }
    Ok(Artifact {
        schema,
        digest: digest(domain, bytes.as_bytes()),
        bytes,
    })
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

fn render_preview(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
    proposal_digest: &str,
    candidate_manifest_digest: &str,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, PREVIEW_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json(output, MANIFEST_SCHEMA);
    push_common_change_members(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        super::SCHEMA,
        proposal_digest,
        prepared.proposal_source.len(),
    );
    output.push_str(",\"base_workspace_graph\":");
    push_graph_ref(output, prepared.base_workspace_graph_digest());
    output.push_str(",\"candidate_workspace_graph\":");
    push_graph_ref(output, prepared.candidate_workspace_graph_digest());
    output.push_str(",\"candidate_manifest\":");
    push_ref(
        output,
        MANIFEST_SCHEMA,
        candidate_manifest_digest,
        prepared.candidate_manifest.len(),
    );
    output.push_str(",\"files\":");
    push_files(output, &prepared.changed_files);
    output.push_str(",\"delta\":{\"roots\":[");
    for (index, root) in prepared.roots.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_root(output, root);
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in prepared.delta_edges.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_delta_edge(output, edge);
    }
    output.push_str("]},\"limits\":");
    push_limits(output);
    output.push_str(",\"budget\":");
    push_budget(output, usage);
    output.push_str(",\"nonclaims\":");
    push_nonclaims(output);
    output.push_str("}\n");
}

fn render_context(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
    proposal_digest: &str,
    preview: &Artifact,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, CONTEXT_SCHEMA);
    push_common_change_members(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        super::SCHEMA,
        proposal_digest,
        prepared.proposal_source.len(),
    );
    output.push_str(",\"change_preview\":");
    push_artifact_ref(output, preview);
    output.push_str(",\"nodes\":[");
    for (index, node) in prepared.context_nodes.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"state\":");
        push_json(output, node.state);
        output.push_str(",\"kind\":");
        push_json(output, node.kind);
        output.push_str(",\"declaration_kind\":");
        push_optional(output, node.declaration_kind);
        output.push_str(",\"identity_origin\":");
        push_optional(output, node.identity_origin);
        output.push_str(",\"id\":");
        push_json(output, &node.id);
        output.push_str(",\"path\":");
        push_optional(output, node.path.as_deref());
        output.push_str(",\"module\":");
        push_optional(output, node.module.as_deref());
        output.push('}');
    }
    output.push(']');
    push_limits_budget_nonclaims(output, usage);
}

fn render_impact(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
    proposal_digest: &str,
    preview: &Artifact,
    context: &Artifact,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, IMPACT_SCHEMA);
    push_common_change_members(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        super::SCHEMA,
        proposal_digest,
        prepared.proposal_source.len(),
    );
    output.push_str(",\"change_preview\":");
    push_artifact_ref(output, preview);
    output.push_str(",\"context\":");
    push_artifact_ref(output, context);
    output.push_str(",\"affected\":[");
    for (index, fact) in prepared.impact.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_impact_fact(output, fact);
    }
    output.push_str("],\"dependency_edges\":[");
    for (index, edge) in prepared.impact_edges.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_impact_edge(output, edge);
    }
    output.push(']');
    push_limits_budget_nonclaims(output, usage);
}

fn render_review(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
    children: ChildRefs<'_>,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, REVIEW_SCHEMA);
    push_common_change_members(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        super::SCHEMA,
        children.proposal_digest,
        prepared.proposal_source.len(),
    );
    output.push_str(",\"change_preview\":");
    push_artifact_ref(output, children.preview);
    output.push_str(",\"context\":");
    push_artifact_ref(output, children.context);
    output.push_str(",\"impact\":");
    push_artifact_ref(output, children.impact);
    output.push_str(",\"sections\":{");
    push_review_sections(output, prepared);
    output.push_str("},\"evidence\":[");
    push_review_evidence(output, prepared);
    output.push(']');
    push_limits_budget_nonclaims(output, usage);
}

fn push_review_sections(output: &mut CappedString, prepared: &SemanticWorkspacePreparedChange) {
    let declaration_roots = prepared.roots.iter().any(|root| root.kind == "declaration");
    let security_change = prepared.roots.iter().any(|root| root.kind == "capability")
        || prepared.delta_edges.iter().any(|edge| {
            matches!(
                edge.edge.kind(),
                "effect_requirement" | "capability_authority"
            )
        });
    let offsets = EvidenceOffsets::new(prepared);
    push_section(
        output,
        "behavior",
        "change_proven",
        "SWC-BEHAVIOR-DELTA",
        "Authenticated behavior delta and reverse impact are represented by the indexed evidence.",
        "review_required",
        offsets.delta_edges..offsets.end,
    );
    output.push(',');
    push_section(
        output,
        "api_identity",
        if declaration_roots {
            "change_proven"
        } else {
            "unchanged_within_admitted_domain"
        },
        "SWC-API-IDENTITY-DELTA",
        "Authenticated declaration identity changes are represented by the indexed preview roots.",
        "review_required",
        prepared
            .roots
            .iter()
            .enumerate()
            .filter(|(_, root)| root.kind == "declaration")
            .map(|(index, _)| index),
    );
    output.push(',');
    push_section(
        output,
        "security_authority",
        if security_change {
            "change_proven"
        } else {
            "unchanged_within_admitted_domain"
        },
        "SWC-SECURITY-AUTHORITY-DELTA",
        "Authenticated capability and effect-authority changes are represented by the indexed evidence.",
        "review_required",
        prepared
            .roots
            .iter()
            .enumerate()
            .filter(|(_, root)| root.kind == "capability")
            .map(|(index, _)| index)
            .chain(
                prepared
                    .delta_edges
                    .iter()
                    .enumerate()
                    .filter(|(_, edge)| {
                        matches!(
                            edge.edge.kind(),
                            "effect_requirement" | "capability_authority"
                        )
                    })
                    .map(|(index, _)| offsets.delta_edges + index),
            ),
    );
    output.push(',');
    push_section(
        output,
        "memory_ownership",
        "unknown",
        "SWC-MEMORY-OWNERSHIP-UNASSESSED",
        "No general cross-file memory-ownership compatibility claim is established.",
        "no_claim",
        std::iter::empty(),
    );
    output.push(',');
    push_section(
        output,
        "target_artifact",
        "unknown",
        "SWC-TARGET-ARTIFACT-UNASSESSED",
        "No target artifact is emitted, executed, or verified.",
        "no_claim",
        std::iter::empty(),
    );
    output.push(',');
    push_section(
        output,
        "migration",
        "change_proven",
        "SWC-MIGRATION-REPLACEMENTS",
        "The proposal is a replacements-only managed semantic-workspace migration.",
        "review_required",
        (0..prepared.roots.len()).chain(offsets.affected..offsets.dependency_edges),
    );
    output.push(',');
    push_section(
        output,
        "unsafe",
        "unknown",
        "SWC-UNSAFE-UNASSESSED",
        "No general unsafe, ABI, or foreign-code analysis is established.",
        "no_claim",
        std::iter::empty(),
    );
}

fn push_section(
    output: &mut CappedString,
    name: &str,
    assessment: &str,
    code: &str,
    statement: &str,
    disposition: &str,
    evidence: impl IntoIterator<Item = usize>,
) {
    push_json(output, name);
    output.push_str(":{\"assessment\":");
    push_json(output, assessment);
    output.push_str(",\"findings\":[{\"code\":");
    push_json(output, code);
    output.push_str(",\"statement\":");
    push_json(output, statement);
    output.push_str(",\"disposition\":");
    push_json(output, disposition);
    output.push_str(",\"evidence\":[");
    for (index, reference) in evidence.into_iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{reference}").expect("string writes cannot fail");
    }
    output.push_str("]}]}");
}

fn render_evidence(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
    children: ChildRefs<'_>,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, EVIDENCE_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json(output, MANIFEST_SCHEMA);
    push_common_change_members(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        super::SCHEMA,
        children.proposal_digest,
        prepared.proposal_source.len(),
    );
    output.push_str(",\"base_workspace_graph\":");
    push_graph_ref(output, prepared.base_workspace_graph_digest());
    output.push_str(",\"candidate_workspace_graph\":");
    push_graph_ref(output, prepared.candidate_workspace_graph_digest());
    output.push_str(",\"candidate_manifest\":");
    push_ref(
        output,
        MANIFEST_SCHEMA,
        children.candidate_manifest_digest,
        prepared.candidate_manifest.len(),
    );
    output.push_str(",\"change_preview\":");
    push_artifact_ref(output, children.preview);
    output.push_str(",\"context\":");
    push_artifact_ref(output, children.context);
    output.push_str(",\"impact\":");
    push_artifact_ref(output, children.impact);
    output.push_str(",\"review\":");
    push_artifact_ref(
        output,
        children
            .review
            .expect("Evidence rendering requires the retained Review artifact"),
    );
    output.push_str(",\"files\":");
    push_files(output, &prepared.changed_files);
    output.push_str(",\"limits\":");
    push_limits(output);
    output.push_str(",\"budget\":");
    push_budget(output, usage);
    output.push_str(",\"nonclaims\":");
    push_nonclaims(output);
    output.push_str("}\n");
}

fn render_receipt(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
    artifacts: &SemanticWorkspaceChangeArtifacts,
    usage: Usage,
    schema: &str,
    result: &str,
) {
    output.push_str("{\"schema\":");
    push_json(output, schema);
    output.push_str(",\"result\":");
    push_json(output, result);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json(output, MANIFEST_SCHEMA);
    push_common_change_members(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        super::SCHEMA,
        &artifacts.proposal_digest,
        prepared.proposal_source.len(),
    );
    output.push_str(",\"base_workspace_graph\":");
    push_graph_ref(output, prepared.base_workspace_graph_digest());
    output.push_str(",\"candidate_workspace_graph\":");
    push_graph_ref(output, prepared.candidate_workspace_graph_digest());
    output.push_str(",\"candidate_manifest\":");
    push_ref(
        output,
        MANIFEST_SCHEMA,
        &artifacts.candidate_manifest_digest,
        prepared.candidate_manifest.len(),
    );
    output.push_str(",\"change_preview\":");
    push_artifact_ref(output, &artifacts.preview);
    output.push_str(",\"context\":");
    push_artifact_ref(output, &artifacts.context);
    output.push_str(",\"impact\":");
    push_artifact_ref(output, &artifacts.impact);
    output.push_str(",\"review\":");
    push_artifact_ref(output, &artifacts.review);
    output.push_str(",\"workspace_change_evidence\":");
    push_artifact_ref(output, &artifacts.evidence);
    output.push_str(",\"files\":");
    push_files(output, &prepared.changed_files);
    output.push_str(",\"limits\":");
    push_limits(output);
    output.push_str(",\"budget\":");
    push_budget(output, usage);
    output.push_str(",\"nonclaims\":");
    push_nonclaims(output);
    output.push_str("}\n");
}

fn push_common_change_members(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedChange,
) {
    output.push_str(",\"base_workspace_revision\":");
    push_json(output, prepared.base_workspace_revision());
    output.push_str(",\"candidate_workspace_revision\":");
    push_json(output, prepared.candidate_workspace_revision());
    output.push_str(",\"entry_module\":");
    push_json(output, prepared.entry_module());
}

fn push_ref(output: &mut CappedString, schema: &str, digest: &str, bytes: usize) {
    output.push_str("{\"schema\":");
    push_json(output, schema);
    output.push_str(",\"digest\":");
    push_json(output, digest);
    write!(output, ",\"bytes\":{bytes}}}").expect("string writes cannot fail");
}

fn push_graph_ref(output: &mut CappedString, digest: &str) {
    output.push_str("{\"schema\":");
    push_json(output, GRAPH_SCHEMA);
    output.push_str(",\"digest\":");
    push_json(output, digest);
    output.push('}');
}

fn push_artifact_ref(output: &mut CappedString, artifact: &Artifact) {
    push_ref(
        output,
        artifact.schema,
        &artifact.digest,
        artifact.bytes.len(),
    );
}

fn push_files(output: &mut CappedString, files: &[SemanticWorkspaceChangedFileFact]) {
    output.push('[');
    for (index, file) in files.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        push_json(output, &file.path);
        output.push_str(",\"base_source_graph_schema\":");
        push_json(output, &file.base_source_graph_schema);
        output.push_str(",\"candidate_source_graph_schema\":");
        push_json(output, &file.candidate_source_graph_schema);
        output.push_str(",\"base_source_revision\":");
        push_json(output, &file.base_source_revision);
        output.push_str(",\"candidate_source_revision\":");
        push_json(output, &file.candidate_source_revision);
        output.push_str(",\"base_source_digest\":");
        push_json(output, &file.base_source_digest);
        output.push_str(",\"candidate_source_digest\":");
        push_json(output, &file.candidate_source_digest);
        write!(
            output,
            ",\"base_bytes\":{},\"candidate_bytes\":{}}}",
            file.base_bytes, file.candidate_bytes
        )
        .expect("string writes cannot fail");
    }
    output.push(']');
}

fn push_root(output: &mut CappedString, root: &SemanticWorkspaceChangeRoot) {
    output.push_str("{\"state\":");
    push_json(output, root.state);
    output.push_str(",\"kind\":");
    push_json(output, root.kind);
    output.push_str(",\"id\":");
    push_json(output, &root.id);
    output.push_str(",\"path\":");
    push_optional(output, root.path.as_deref());
    output.push_str(",\"module\":");
    push_optional(output, root.module.as_deref());
    output.push_str(",\"change\":");
    push_json(output, root.change);
    output.push_str(",\"identity_origin\":");
    push_optional(output, root.identity_origin);
    output.push('}');
}

fn push_delta_edge(output: &mut CappedString, edge: &SemanticWorkspaceChangeEdge) {
    output.push_str("{\"state\":");
    push_json(output, edge.state);
    output.push_str(",\"change\":");
    push_json(output, edge.change);
    output.push_str(",\"edge\":");
    push_edge(output, &edge.edge);
    output.push('}');
}

fn push_impact_edge(output: &mut CappedString, edge: &SemanticWorkspaceChangeImpactEdge) {
    output.push_str("{\"state\":");
    push_json(output, edge.state);
    output.push_str(",\"edge\":");
    push_edge(output, &edge.edge);
    output.push('}');
}

fn push_edge(output: &mut CappedString, edge: &workspace_graph::WorkspaceEdge) {
    output.push_str("{\"caller_path\":");
    push_json(output, edge.caller_path());
    output.push_str(",\"caller\":");
    push_json(output, edge.caller());
    output.push_str(",\"target_path\":");
    push_json(output, edge.target_path());
    output.push_str(",\"target\":");
    push_json(output, edge.target());
    output.push_str(",\"kind\":");
    push_json(output, edge.kind());
    output.push_str(",\"site\":");
    push_json(output, edge.site());
    output.push_str(",\"expression\":");
    push_json(output, edge.expression());
    output.push_str(",\"ast_path\":");
    push_json(output, edge.ast_path());
    output.push_str(",\"alias\":");
    push_json(output, edge.alias());
    write!(output, ",\"ordinal\":{}}}", edge.ordinal()).expect("string writes cannot fail");
}

fn push_impact_fact(output: &mut CappedString, fact: &SemanticWorkspaceChangeImpactFact) {
    output.push_str("{\"state\":");
    push_json(output, fact.state);
    output.push_str(",\"kind\":");
    push_json(output, fact.kind);
    output.push_str(",\"declaration_kind\":");
    push_optional(output, fact.declaration_kind);
    output.push_str(",\"identity_origin\":");
    push_optional(output, fact.identity_origin);
    output.push_str(",\"id\":");
    push_json(output, &fact.id);
    output.push_str(",\"path\":");
    push_optional(output, fact.path.as_deref());
    output.push_str(",\"module\":");
    push_optional(output, fact.module.as_deref());
    write!(output, ",\"minimum_depth\":{}", fact.minimum_depth).expect("string writes cannot fail");
    output.push_str(",\"role\":");
    push_json(output, fact.impact_role);
    output.push_str(",\"reasons\":[");
    for (index, reason) in fact.reasons.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json(output, reason);
    }
    output.push_str("],\"root_provenance\":[");
    for (index, root) in fact.root_provenance.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{root}").expect("string writes cannot fail");
    }
    output.push_str("]}");
}

fn push_limits(output: &mut CappedString) {
    write!(
        output,
        "{{\"max_managed_files\":16,\"max_changed_files\":16,\"max_source_bytes_per_change\":{MAX_SOURCE_BYTES_PER_CHANGE},\"max_total_base_source_bytes\":{MAX_TOTAL_BASE_SOURCE_BYTES},\"max_total_candidate_source_bytes\":{MAX_TOTAL_CANDIDATE_SOURCE_BYTES},\"max_total_replacement_source_bytes\":{MAX_TOTAL_REPLACEMENT_SOURCE_BYTES},\"max_entry_module_bytes\":{MAX_ENTRY_MODULE_BYTES},\"max_proposal_bytes\":{MAX_PROPOSAL_BYTES},\"max_candidate_manifest_bytes\":{MAX_CANDIDATE_MANIFEST_BYTES},\"max_delta_roots\":{MAX_DELTA_ROOTS},\"max_delta_edges\":{MAX_DELTA_EDGES},\"max_context_nodes\":{MAX_CONTEXT_NODES},\"max_impact_nodes\":{MAX_IMPACT_NODES},\"max_impact_provenance\":{MAX_IMPACT_PROVENANCE},\"max_impact_depth\":{MAX_IMPACT_DEPTH},\"max_analysis_builder_bytes\":{MAX_ANALYSIS_BUILDER_BYTES},\"max_change_preview_bytes\":{MAX_PREVIEW_BYTES},\"max_context_bytes\":{MAX_CONTEXT_BYTES},\"max_impact_bytes\":{MAX_IMPACT_BYTES},\"max_review_bytes\":{MAX_REVIEW_BYTES},\"max_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_receipt_bytes\":{MAX_RECEIPT_BYTES},\"max_total_artifact_bytes\":{MAX_TOTAL_ARTIFACT_BYTES},\"max_json_depth\":8,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}}"
    )
    .expect("string writes cannot fail");
}

fn push_budget(output: &mut CappedString, usage: Usage) {
    write!(
        output,
        "{{\"used_managed_files\":{},\"used_changed_files\":{},\"used_total_base_source_bytes\":{},\"used_total_candidate_source_bytes\":{},\"used_total_replacement_source_bytes\":{},\"used_entry_module_bytes\":{},\"used_proposal_bytes\":{},\"used_candidate_manifest_bytes\":{},\"used_delta_roots\":{},\"used_delta_edges\":{},\"used_context_nodes\":{},\"used_impact_nodes\":{},\"used_impact_provenance\":{},\"used_impact_depth\":{},\"used_analysis_builder_bytes\":{},\"used_change_preview_bytes\":{},\"used_context_bytes\":{},\"used_impact_bytes\":{},\"used_review_bytes\":{},\"used_evidence_bytes\":{},\"used_receipt_bytes\":{},\"used_total_artifact_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":0}}",
        usage.managed_files,
        usage.changed_files,
        usage.total_base_source_bytes,
        usage.total_candidate_source_bytes,
        usage.total_replacement_source_bytes,
        usage.entry_module_bytes,
        usage.proposal_bytes,
        usage.candidate_manifest_bytes,
        usage.delta_roots,
        usage.delta_edges,
        usage.context_nodes,
        usage.impact_nodes,
        usage.impact_provenance,
        usage.impact_depth,
        usage.analysis_builder_bytes,
        usage.sizes.preview,
        usage.sizes.context,
        usage.sizes.impact,
        usage.sizes.review,
        usage.sizes.evidence,
        usage.receipt_bytes,
        usage.total_artifact_bytes,
        usage.retained_generations,
        usage.staging_attempts,
    )
    .expect("string writes cannot fail");
}

fn push_limits_budget_nonclaims(output: &mut CappedString, usage: Usage) {
    output.push_str(",\"limits\":");
    push_limits(output);
    output.push_str(",\"budget\":");
    push_budget(output, usage);
    output.push_str(",\"nonclaims\":");
    push_nonclaims(output);
    output.push_str("}\n");
}

fn push_nonclaims(output: &mut CappedString) {
    output.push('[');
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json(output, nonclaim);
    }
    output.push(']');
}

fn push_optional(output: &mut CappedString, value: Option<&str>) {
    if let Some(value) = value {
        push_json(output, value);
    } else {
        output.push_str("null");
    }
}

fn push_json(output: &mut CappedString, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(output, "\\u{:04x}", character as u32).expect("string writes cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn replay_prepared(prepared: &SemanticWorkspacePreparedChange) -> Result<(), Vec<Diagnostic>> {
    if prepared.changed_files.len() < 2 || prepared.changed_files.len() > MAX_CHANGED_FILES {
        return Err(replay(
            "Semantic Workspace Change artifact changed-file replay disagrees",
        ));
    }
    if prepared
        .changed_files
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
        || prepared.roots.windows(2).any(|pair| {
            (
                pair[0].state,
                pair[0].kind,
                &pair[0].id,
                &pair[0].path,
                &pair[0].module,
                pair[0].change,
                pair[0].identity_origin,
            ) >= (
                pair[1].state,
                pair[1].kind,
                &pair[1].id,
                &pair[1].path,
                &pair[1].module,
                pair[1].change,
                pair[1].identity_origin,
            )
        })
        || prepared.context_nodes.windows(2).any(|pair| {
            (
                pair[0].state,
                pair[0].kind,
                &pair[0].id,
                &pair[0].path,
                &pair[0].module,
            ) >= (
                pair[1].state,
                pair[1].kind,
                &pair[1].id,
                &pair[1].path,
                &pair[1].module,
            )
        })
    {
        return Err(replay(
            "Semantic Workspace Change artifact canonical fact order disagrees",
        ));
    }
    if prepared.context_nodes.len() > MAX_CONTEXT_NODES {
        return Err(incomplete(
            "Semantic Workspace Change Context node closure is incomplete",
        ));
    }
    if prepared.impact.iter().any(|fact| {
        fact.root_provenance
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || fact
                .root_provenance
                .iter()
                .any(|index| *index >= prepared.roots.len())
    }) {
        return Err(incomplete(
            "Semantic Workspace Change provenance is incomplete",
        ));
    }
    let context_keys = prepared
        .context_nodes
        .iter()
        .map(|node| (node.state, node.kind, node.id.as_str()))
        .collect::<BTreeSet<_>>();
    for root in &prepared.roots {
        if !context_keys.contains(&(root.state, root.kind, root.id.as_str())) {
            return Err(incomplete(
                "Semantic Workspace Change Context node closure is incomplete",
            ));
        }
    }
    Ok(())
}

fn verify_artifact_bindings(
    artifacts: &SemanticWorkspaceChangeArtifacts,
) -> Result<(), Vec<Diagnostic>> {
    let bindings = [
        (&artifacts.preview, PREVIEW_DIGEST_DOMAIN),
        (&artifacts.context, CONTEXT_DIGEST_DOMAIN),
        (&artifacts.impact, IMPACT_DIGEST_DOMAIN),
        (&artifacts.review, REVIEW_DIGEST_DOMAIN),
        (&artifacts.evidence, EVIDENCE_DIGEST_DOMAIN),
    ];
    if bindings
        .into_iter()
        .any(|(artifact, domain)| artifact.digest != digest(domain, artifact.bytes.as_bytes()))
    {
        return Err(replay(
            "Semantic Workspace Change artifact digest binding disagrees",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write as _;

    use serde_json::Value;

    use super::*;
    use crate::semantic_workspace_change::tests::Fixture;

    fn raw_sha(bytes: &str) -> String {
        format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
        )
    }

    fn top_keys(source: &str) -> Vec<String> {
        let bytes = source.as_bytes();
        let mut keys = Vec::new();
        let mut depth = 0usize;
        let mut index = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'{' | b'[' => {
                    depth += 1;
                    index += 1;
                }
                b'}' | b']' => {
                    depth -= 1;
                    index += 1;
                }
                b'"' => {
                    let start = index + 1;
                    index += 1;
                    let mut escape = false;
                    while index < bytes.len() {
                        if escape {
                            escape = false;
                        } else if bytes[index] == b'\\' {
                            escape = true;
                        } else if bytes[index] == b'"' {
                            break;
                        }
                        index += 1;
                    }
                    let end = index;
                    index += 1;
                    if depth == 1 && bytes.get(index) == Some(&b':') {
                        keys.push(source[start..end].to_owned());
                    }
                }
                _ => index += 1,
            }
        }
        keys
    }

    fn assert_reference(value: &Value, key: &str, artifact: &Artifact) {
        let reference = &value[key];
        assert_eq!(reference["schema"], artifact.schema);
        assert_eq!(reference["digest"], artifact.digest);
        assert_eq!(reference["bytes"], artifact.bytes.len());
    }

    #[test]
    fn literal_kats_wire_order_domains_and_reference_parity() {
        let fixture = Fixture::new("artifact-kats");
        let proposal = fixture.proposal();
        let proposal_source = proposal.source().to_owned();
        let prepared = super::super::build_authenticated_change(
            &fixture.root,
            super::super::parse_proposal(&proposal_source).unwrap(),
        )
        .unwrap();
        let candidate_manifest = prepared.candidate_manifest().to_owned();
        let artifacts = build_authenticated_artifacts(&fixture.root, proposal).unwrap();

        assert_eq!(
            artifacts.proposal_digest(),
            digest(PROPOSAL_DIGEST_DOMAIN, proposal_source.as_bytes())
        );
        assert_eq!(
            artifacts.candidate_manifest_digest(),
            digest(
                CANDIDATE_MANIFEST_DIGEST_DOMAIN,
                candidate_manifest.as_bytes()
            )
        );
        for (artifact, domain) in [
            (&artifacts.preview, PREVIEW_DIGEST_DOMAIN),
            (&artifacts.context, CONTEXT_DIGEST_DOMAIN),
            (&artifacts.impact, IMPACT_DIGEST_DOMAIN),
            (&artifacts.review, REVIEW_DIGEST_DOMAIN),
            (&artifacts.evidence, EVIDENCE_DIGEST_DOMAIN),
        ] {
            assert_eq!(artifact.digest, digest(domain, artifact.bytes.as_bytes()));
            assert!(artifact.bytes.ends_with('\n'));
            assert!(!artifact.bytes[..artifact.bytes.len() - 1].contains('\n'));
            let mut mutated = artifact.bytes.clone().into_bytes();
            let middle = mutated.len() / 2;
            mutated[middle] ^= 1;
            assert_ne!(artifact.digest, digest(domain, &mutated));
        }

        assert_eq!(
            [
                raw_sha(artifacts.preview()),
                raw_sha(artifacts.context()),
                raw_sha(artifacts.impact()),
                raw_sha(artifacts.review()),
                raw_sha(artifacts.evidence()),
            ],
            [
                "sha256:7578569bd190bf11e20e0bc5f0259caeb6ae27a5e7b5bdfe7900e91340ad88f9",
                "sha256:6b8bcd49da631e33a579bcba66d17e35cd2cc7b3fb96a18f5d8970b2f8fa022f",
                "sha256:96dd548d54ea89cb40c323d147bd545b1f67627cec93cc4cc136d72d02878945",
                "sha256:e7243b1ccb7e732a2adb1f78bd6167b30ec4d5a36d5fb33225dc8323cf764e69",
                "sha256:d8f352b5f05914620cc2b29bd52888a070f2959764c9518c4a61b9342d5e92c7"
            ]
        );

        assert_eq!(
            top_keys(artifacts.preview()),
            [
                "schema",
                "workspace_manifest_schema",
                "base_workspace_revision",
                "candidate_workspace_revision",
                "entry_module",
                "proposal",
                "base_workspace_graph",
                "candidate_workspace_graph",
                "candidate_manifest",
                "files",
                "delta",
                "limits",
                "budget",
                "nonclaims",
            ]
        );
        assert_eq!(
            top_keys(artifacts.context()),
            [
                "schema",
                "base_workspace_revision",
                "candidate_workspace_revision",
                "entry_module",
                "proposal",
                "change_preview",
                "nodes",
                "limits",
                "budget",
                "nonclaims",
            ]
        );
        assert_eq!(
            top_keys(artifacts.impact()),
            [
                "schema",
                "base_workspace_revision",
                "candidate_workspace_revision",
                "entry_module",
                "proposal",
                "change_preview",
                "context",
                "affected",
                "dependency_edges",
                "limits",
                "budget",
                "nonclaims",
            ]
        );
        assert_eq!(
            top_keys(artifacts.review()),
            [
                "schema",
                "base_workspace_revision",
                "candidate_workspace_revision",
                "entry_module",
                "proposal",
                "change_preview",
                "context",
                "impact",
                "sections",
                "evidence",
                "limits",
                "budget",
                "nonclaims",
            ]
        );
        assert_eq!(
            top_keys(artifacts.evidence()),
            [
                "schema",
                "workspace_manifest_schema",
                "base_workspace_revision",
                "candidate_workspace_revision",
                "entry_module",
                "proposal",
                "base_workspace_graph",
                "candidate_workspace_graph",
                "candidate_manifest",
                "change_preview",
                "context",
                "impact",
                "review",
                "files",
                "limits",
                "budget",
                "nonclaims",
            ]
        );

        let preview: Value = serde_json::from_str(artifacts.preview()).unwrap();
        let context: Value = serde_json::from_str(artifacts.context()).unwrap();
        let impact: Value = serde_json::from_str(artifacts.impact()).unwrap();
        let review: Value = serde_json::from_str(artifacts.review()).unwrap();
        let evidence: Value = serde_json::from_str(artifacts.evidence()).unwrap();
        for value in [&preview, &context, &impact, &review, &evidence] {
            assert_eq!(value["proposal"]["digest"], artifacts.proposal_digest());
            assert_eq!(value["proposal"]["bytes"], proposal_source.len());
        }
        assert_reference(&context, "change_preview", &artifacts.preview);
        assert_reference(&impact, "change_preview", &artifacts.preview);
        assert_reference(&impact, "context", &artifacts.context);
        assert_reference(&review, "change_preview", &artifacts.preview);
        assert_reference(&review, "context", &artifacts.context);
        assert_reference(&review, "impact", &artifacts.impact);
        assert_reference(&evidence, "change_preview", &artifacts.preview);
        assert_reference(&evidence, "context", &artifacts.context);
        assert_reference(&evidence, "impact", &artifacts.impact);
        assert_reference(&evidence, "review", &artifacts.review);
        assert_eq!(
            preview["delta"]["roots"].as_array().unwrap().len(),
            prepared.roots().len()
        );
        assert_eq!(
            preview["delta"]["edges"].as_array().unwrap().len(),
            prepared.delta_edges().len()
        );
        assert_eq!(
            context["nodes"].as_array().unwrap().len(),
            prepared.context_nodes().len()
        );
        assert_eq!(
            impact["affected"].as_array().unwrap().len(),
            prepared.impact().len()
        );

        let mut escaped = CappedString::new();
        push_json(&mut escaped, "quote\" slash\\ lf\n cr\r tab\t \u{0001}");
        assert_eq!(
            escaped.into_string(),
            "\"quote\\\" slash\\\\ lf\\n cr\\r tab\\t \\u0001\""
        );
    }

    #[test]
    fn exact_output_and_builder_caps_and_complete_only_children() {
        let fixture = Fixture::new("artifact-limits");
        let artifacts = build_authenticated_artifacts(&fixture.root, fixture.proposal()).unwrap();
        for (source, schema, domain, field) in [
            (
                &artifacts.preview,
                PREVIEW_SCHEMA,
                PREVIEW_DIGEST_DOMAIN,
                "change_preview_bytes",
            ),
            (
                &artifacts.context,
                CONTEXT_SCHEMA,
                CONTEXT_DIGEST_DOMAIN,
                "context_bytes",
            ),
            (
                &artifacts.impact,
                IMPACT_SCHEMA,
                IMPACT_DIGEST_DOMAIN,
                "impact_bytes",
            ),
            (
                &artifacts.review,
                REVIEW_SCHEMA,
                REVIEW_DIGEST_DOMAIN,
                "review_bytes",
            ),
            (
                &artifacts.evidence,
                EVIDENCE_SCHEMA,
                EVIDENCE_DIGEST_DOMAIN,
                "evidence_bytes",
            ),
        ] {
            let exact = artifact(schema, domain, source.bytes.len(), field, |output| {
                output.push_str(&source.bytes)
            })
            .unwrap();
            assert_eq!(exact, *source);
            let error = artifact(schema, domain, source.bytes.len() - 1, field, |output| {
                output.push_str(&source.bytes)
            })
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-G183");
        }

        let mut exact_builder =
            super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
        exact_builder.used_builder_bytes = MAX_ANALYSIS_BUILDER_BYTES;
        let exact = render_artifacts(&exact_builder).unwrap();
        let evidence: Value = serde_json::from_str(exact.evidence()).unwrap();
        assert_eq!(
            evidence["budget"]["used_analysis_builder_bytes"],
            MAX_ANALYSIS_BUILDER_BYTES
        );
        let mut over_builder =
            super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
        over_builder.used_builder_bytes = MAX_ANALYSIS_BUILDER_BYTES + 1;
        let error = render_artifacts(&over_builder)
            .err()
            .expect("over-limit builder must fail");
        assert_eq!(error[0].code, "SPX-G183");
        assert_eq!(
            error[0].message,
            "Semantic Workspace Change `analysis_builder_bytes` exceeds 33554432"
        );

        let mut incomplete_context =
            super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
        let root = &incomplete_context.roots[0];
        let index = incomplete_context
            .context_nodes
            .iter()
            .position(|node| {
                node.state == root.state && node.kind == root.kind && node.id == root.id
            })
            .unwrap();
        incomplete_context.context_nodes.remove(index);
        let error = render_artifacts(&incomplete_context)
            .err()
            .expect("incomplete Context must fail");
        assert_eq!(error[0].code, "SPX-G186");
        let mut incomplete_provenance =
            super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
        incomplete_provenance.impact[0]
            .root_provenance
            .push(incomplete_provenance.roots.len());
        let error = render_artifacts(&incomplete_provenance)
            .err()
            .expect("incomplete provenance must fail");
        assert_eq!(error[0].code, "SPX-G186");
    }

    #[test]
    fn after_render_drift_discards_artifacts_and_releases_authority() {
        let fixture = Fixture::new("artifact-final-recheck");
        let called = std::cell::Cell::new(false);
        let result = build_authenticated_artifacts_with_hook(
            &fixture.root,
            fixture.proposal(),
            |artifacts| {
                called.set(true);
                assert!(!artifacts.evidence().is_empty());
                OpenOptions::new()
                    .append(true)
                    .open(fixture.root.join(".semaprax-workspace/ACTIVE"))
                    .unwrap()
                    .write_all(b"x")
                    .unwrap();
            },
        );
        assert!(called.get());
        let error = result.err().unwrap();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G153");
        assert_eq!(
            error[0].message,
            "workspace object changed during authentication"
        );
        fixture.assert_exclusive_reacquire();
    }
}
