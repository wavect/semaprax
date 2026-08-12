//! Canonical bounded artifacts derived only from typed Structural Change facts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::{
    limit, replay, SemanticWorkspacePreparedStructuralChange, SemanticWorkspaceStructuralOperation,
    MAX_ANALYSIS_BUILDER_BYTES, MAX_CANDIDATE_MANIFEST_BYTES, MAX_ENTRY_MODULE_BYTES,
    MAX_MANAGED_FILES, MAX_OPERATIONS, MAX_PATH_BYTES, MAX_PROPOSAL_BYTES,
    MAX_SOURCE_BYTES_PER_OPERATION, MAX_TOTAL_SOURCE_BYTES, MAX_TOTAL_SUPPLIED_SOURCE_BYTES,
    SCHEMA,
};
use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace;
use crate::{graph, review, semantic_workspace_change, workspace, workspace_graph};

const PREVIEW_SCHEMA: &str = "semaprax.workspace-semantic-structural-change-preview.v1";
const CONTEXT_SCHEMA: &str = "semaprax.workspace-semantic-structural-change-context.v1";
const IMPACT_SCHEMA: &str = "semaprax.workspace-semantic-structural-change-impact.v1";
const REVIEW_SCHEMA: &str = "semaprax.workspace-semantic-structural-change-review.v1";
const EVIDENCE_SCHEMA: &str = "semaprax.workspace-semantic-structural-change-evidence.v1";
const VERIFICATION_RECEIPT_SCHEMA: &str =
    "semaprax.workspace-semantic-structural-change-evidence-verification.v1";
const GRAPH_SCHEMA: &str = "semaprax.workspace-semantic-graph.v1";
const MANIFEST_SCHEMA: &str = "semaprax.workspace-semantic-manifest.v1";

const PROPOSAL_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change.proposal-digest.v1\0";
const CANDIDATE_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change.candidate-manifest-digest.v1\0";
const PREVIEW_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change-preview.artifact-digest.v1\0";
const CONTEXT_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change-context.artifact-digest.v1\0";
const IMPACT_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change-impact.artifact-digest.v1\0";
const REVIEW_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change-review.artifact-digest.v1\0";
const EVIDENCE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.workspace-semantic-structural-change-evidence.artifact-digest.v1\0";

const MAX_AFFECTED_PATHS: usize = 32;
const MAX_DELTA_ROOTS: usize = 8192;
const MAX_DELTA_EDGES: usize = 131_072;
const MAX_CONTEXT_NODES: usize = 16_384;
const MAX_IMPACT_NODES: usize = 16_384;
const MAX_IMPACT_PROVENANCE: usize = 65_536;
const MAX_IMPACT_DEPTH: usize = 1024;
const MAX_PREVIEW_BYTES: usize = 32 * 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMPACT_BYTES: usize = 32 * 1024 * 1024;
const MAX_REVIEW_BYTES: usize = 16 * 1024 * 1024;
const MAX_EVIDENCE_BYTES: usize = 1024 * 1024;
const MAX_RECEIPT_BYTES: usize = 65_536;
const MAX_TOTAL_ARTIFACT_BYTES: usize = 96 * 1024 * 1024;

const NONCLAIMS: [&str; 23] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_machine_code_claim",
    "no_current_state_context_impact_or_review_reuse",
    "no_raw_path_create_delete_move_or_write",
    "no_existing_generation_mutation_deletion_or_cleanup",
    "no_automatic_identity_preservation_across_move",
    "no_move_swap_chain_cycle_or_destination_vacating",
    "no_typed_stable_id_operation_language",
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

/// Opaque public bundle of canonical read-only structural-change artifacts.
pub struct SemanticWorkspaceStructuralChangeArtifacts {
    proposal_digest: String,
    candidate_manifest_digest: String,
    preview: Artifact,
    context: Artifact,
    impact: Artifact,
    review: Artifact,
    evidence: Artifact,
    usage: Usage,
}

impl SemanticWorkspaceStructuralChangeArtifacts {
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ArtifactSizes {
    preview: usize,
    context: usize,
    impact: usize,
    review: usize,
    evidence: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Usage {
    base_managed_files: usize,
    candidate_managed_files: usize,
    operations: usize,
    affected_paths: usize,
    created_files: usize,
    deleted_files: usize,
    moved_files: usize,
    replaced_files: usize,
    total_base_source_bytes: usize,
    total_candidate_source_bytes: usize,
    total_supplied_source_bytes: usize,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathFact {
    path: String,
    change: &'static str,
    peer_path: Option<String>,
    base_source_graph_schema: Option<String>,
    candidate_source_graph_schema: Option<String>,
    base_source_revision: Option<String>,
    candidate_source_revision: Option<String>,
    base_source_digest: Option<String>,
    candidate_source_digest: Option<String>,
    base_bytes: Option<usize>,
    candidate_bytes: Option<usize>,
}

pub(crate) fn render_artifacts(
    prepared: &SemanticWorkspacePreparedStructuralChange,
) -> Result<SemanticWorkspaceStructuralChangeArtifacts, Vec<Diagnostic>> {
    render_artifacts_with_total_limit_inner(prepared, MAX_TOTAL_ARTIFACT_BYTES)
}

#[cfg(test)]
pub(crate) fn render_artifacts_with_total_limit(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    total_limit: usize,
) -> Result<SemanticWorkspaceStructuralChangeArtifacts, Vec<Diagnostic>> {
    assert!(total_limit <= MAX_TOTAL_ARTIFACT_BYTES);
    render_artifacts_with_total_limit_inner(prepared, total_limit)
}

fn render_artifacts_with_total_limit_inner(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    total_limit: usize,
) -> Result<SemanticWorkspaceStructuralChangeArtifacts, Vec<Diagnostic>> {
    let paths = replay_paths(prepared)?;
    replay_prepared(prepared, &paths)?;
    let _ = usage(prepared, &paths, ArtifactSizes::default(), 0, 0)?;
    let replay_builder_bytes = replay_bindings(prepared)?;
    let proposal_digest = digest(
        PROPOSAL_DIGEST_DOMAIN,
        prepared.proposal_source().as_bytes(),
    );
    let candidate_manifest_digest = digest(
        CANDIDATE_MANIFEST_DIGEST_DOMAIN,
        prepared.candidate_manifest().as_bytes(),
    );
    let mut sizes = ArtifactSizes::default();
    for _ in 0..24 {
        let usage = usage(prepared, &paths, sizes, replay_builder_bytes, 0)?;
        let mut remaining = total_limit
            .checked_sub(prepared.proposal_source().len())
            .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
        let preview = artifact_bounded(
            PREVIEW_SCHEMA,
            PREVIEW_DIGEST_DOMAIN,
            MAX_PREVIEW_BYTES,
            "structural_change_preview_bytes",
            remaining,
            |output| {
                render_preview(
                    output,
                    prepared,
                    &paths,
                    &proposal_digest,
                    &candidate_manifest_digest,
                    usage,
                )
            },
        )?;
        remaining = subtract_artifact(remaining, preview.bytes.len())?;
        let context = artifact_bounded(
            CONTEXT_SCHEMA,
            CONTEXT_DIGEST_DOMAIN,
            MAX_CONTEXT_BYTES,
            "context_bytes",
            remaining,
            |output| render_context(output, prepared, &proposal_digest, &preview, usage),
        )?;
        remaining = subtract_artifact(remaining, context.bytes.len())?;
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
        remaining = subtract_artifact(remaining, impact.bytes.len())?;
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
        remaining = subtract_artifact(remaining, review.bytes.len())?;
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
            |output| render_evidence(output, prepared, &paths, children, usage),
        )?;
        let artifacts = SemanticWorkspaceStructuralChangeArtifacts {
            proposal_digest: proposal_digest.clone(),
            candidate_manifest_digest: candidate_manifest_digest.clone(),
            preview,
            context,
            impact,
            review,
            evidence,
            usage,
        };
        let next = ArtifactSizes {
            preview: artifacts.preview.bytes.len(),
            context: artifacts.context.bytes.len(),
            impact: artifacts.impact.bytes.len(),
            review: artifacts.review.bytes.len(),
            evidence: artifacts.evidence.bytes.len(),
        };
        if next == sizes {
            verify_bindings(&artifacts)?;
            return Ok(artifacts);
        }
        sizes = next;
    }
    Err(replay())
}

pub(crate) fn render_verification_receipt(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    artifacts: &SemanticWorkspaceStructuralChangeArtifacts,
    submitted_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    render_verification_receipt_with_limits_inner(
        prepared,
        artifacts,
        submitted_evidence_bytes,
        MAX_RECEIPT_BYTES,
        MAX_TOTAL_ARTIFACT_BYTES,
    )
}

#[cfg(test)]
pub(crate) fn render_verification_receipt_with_limits(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    artifacts: &SemanticWorkspaceStructuralChangeArtifacts,
    submitted_evidence_bytes: usize,
    receipt_limit: usize,
    total_limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    assert!(receipt_limit <= MAX_RECEIPT_BYTES);
    assert!(total_limit <= MAX_TOTAL_ARTIFACT_BYTES);
    render_verification_receipt_with_limits_inner(
        prepared,
        artifacts,
        submitted_evidence_bytes,
        receipt_limit,
        total_limit,
    )
}

fn render_verification_receipt_with_limits_inner(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    artifacts: &SemanticWorkspaceStructuralChangeArtifacts,
    submitted_evidence_bytes: usize,
    receipt_limit: usize,
    total_limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    if submitted_evidence_bytes != artifacts.evidence.bytes.len() {
        return Err(evidence_replay());
    }
    let paths = replay_paths(prepared)?;
    let aggregate_remaining = total_limit
        .checked_sub(artifacts.usage.total_artifact_bytes)
        .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))?;
    let effective_limit = receipt_limit.min(aggregate_remaining);
    let mut receipt_bytes = 0usize;
    for _ in 0..24 {
        let usage = usage(
            prepared,
            &paths,
            artifacts.usage.sizes,
            artifacts.usage.analysis_builder_bytes,
            receipt_bytes,
        )?;
        let (receipt, overflowed) = crate::bounded_output::with_limit(effective_limit, || {
            let mut output = CappedString::new();
            render_receipt(&mut output, prepared, artifacts, &paths, usage);
            output.into_string()
        });
        if overflowed || receipt.len() > effective_limit {
            return Err(if aggregate_remaining < receipt_limit {
                limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES)
            } else {
                limit("receipt_bytes", MAX_RECEIPT_BYTES)
            });
        }
        if receipt.len() == receipt_bytes {
            return Ok(receipt);
        }
        receipt_bytes = receipt.len();
    }
    Err(replay())
}

fn render_receipt(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
    artifacts: &SemanticWorkspaceStructuralChangeArtifacts,
    paths: &[PathFact],
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, VERIFICATION_RECEIPT_SCHEMA);
    output.push_str(",\"result\":\"exact_replay\",\"workspace_manifest_schema\":");
    push_json(output, MANIFEST_SCHEMA);
    push_common(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        SCHEMA,
        artifacts.proposal_digest(),
        prepared.proposal_source().len(),
    );
    output.push_str(",\"base_workspace_graph\":");
    push_graph_ref(output, prepared.base_workspace_graph_digest());
    output.push_str(",\"candidate_workspace_graph\":");
    push_graph_ref(output, prepared.candidate_workspace_graph_digest());
    output.push_str(",\"candidate_manifest\":");
    push_ref(
        output,
        MANIFEST_SCHEMA,
        artifacts.candidate_manifest_digest(),
        prepared.candidate_manifest().len(),
    );
    output.push_str(",\"structural_change_preview\":");
    push_artifact_ref(output, &artifacts.preview);
    output.push_str(",\"context\":");
    push_artifact_ref(output, &artifacts.context);
    output.push_str(",\"impact\":");
    push_artifact_ref(output, &artifacts.impact);
    output.push_str(",\"review\":");
    push_artifact_ref(output, &artifacts.review);
    output.push_str(",\"workspace_structural_change_evidence\":");
    push_artifact_ref(output, &artifacts.evidence);
    output.push_str(",\"paths\":");
    push_paths(output, paths);
    push_tail(output, usage);
}

fn evidence_replay() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G195",
        "Semantic Workspace Structural Change Evidence does not exactly replay the authenticated proposal and candidate",
    )]
}

fn replay_bindings(
    prepared: &SemanticWorkspacePreparedStructuralChange,
) -> Result<usize, Vec<Diagnostic>> {
    let proposal = super::render_proposal_facts(
        prepared.base_workspace_revision(),
        prepared.entry_module(),
        prepared.operations(),
    )
    .map_err(|_| artifact_canonical())?;
    if proposal != prepared.proposal_source() {
        return Err(artifact_canonical());
    }

    let candidate_manifest = semantic_workspace::render_manifest(prepared.candidate_files())
        .map_err(|_| artifact_canonical())?;
    if candidate_manifest != prepared.candidate_manifest()
        || semantic_workspace::semantic_workspace_revision(&candidate_manifest)
            != prepared.candidate_workspace_revision()
    {
        return Err(replay());
    }

    let base_manifest_facts = prepared
        .base_files()
        .iter()
        .map(|file| {
            (
                file.path(),
                file.source_graph_schema(),
                file.source_revision(),
                file.source_digest(),
                file.bytes(),
            )
        })
        .collect::<Vec<_>>();
    let base_manifest =
        semantic_workspace::render_manifest_facts(&base_manifest_facts).map_err(|_| replay())?;
    if base_manifest.len() != prepared.base_manifest_bytes()
        || semantic_workspace::semantic_workspace_revision(&base_manifest)
            != prepared.base_workspace_revision()
    {
        return Err(replay());
    }

    let base_digest = replay_graph_digest(
        prepared.base_graph(),
        prepared.base_files().iter().map(|file| {
            (
                file.path(),
                file.source_graph_schema(),
                file.source_revision(),
                file.source_digest(),
            )
        }),
        prepared.base_workspace_revision(),
        prepared.base_manifest_bytes(),
        prepared,
    )?;
    let candidate_digest = replay_graph_digest(
        prepared.candidate_graph(),
        prepared.candidate_files().iter().map(|file| {
            (
                file.path(),
                file.source_graph_schema(),
                file.source_revision(),
                file.source_digest(),
            )
        }),
        prepared.candidate_workspace_revision(),
        prepared.candidate_manifest().len(),
        prepared,
    )?;
    if base_digest != prepared.base_workspace_graph_digest()
        || candidate_digest != prepared.candidate_workspace_graph_digest()
    {
        return Err(replay());
    }
    replay_analysis(prepared)
}

fn replay_graph_digest<'a>(
    graph: &workspace_graph::WorkspaceGraphChangeView,
    facts: impl Iterator<Item = (&'a str, &'a str, &'a str, &'a str)>,
    revision: &str,
    manifest_bytes: usize,
    prepared: &SemanticWorkspacePreparedStructuralChange,
) -> Result<String, Vec<Diagnostic>> {
    const GRAPH_REPLAY_CAP: usize = 16 * 1024 * 1024;
    const SOURCE_FACT_SETUP_CAP: usize = 1024 * 1024;
    let (sources, setup_overflowed, _) =
        crate::bounded_output::with_limit_usage(SOURCE_FACT_SETUP_CAP, || {
            facts
                .map(|(path, schema, source_revision, source_digest)| {
                    workspace_graph::WorkspaceGraphChangeSourceFact {
                        path: crate::bounded_output::budgeted_clone(path),
                        source_graph_schema: crate::bounded_output::budgeted_clone(schema),
                        source_revision: crate::bounded_output::budgeted_clone(source_revision),
                        source_digest: crate::bounded_output::budgeted_clone(source_digest),
                    }
                })
                .collect::<Vec<_>>()
        });
    if setup_overflowed {
        return Err(replay());
    }
    let (result, overflowed, _) = crate::bounded_output::with_limit_usage(GRAPH_REPLAY_CAP, || {
        graph.projection_digest(
            revision,
            &sources,
            manifest_bytes,
            prepared.retained_generations(),
            prepared.staging_attempts(),
            prepared.entry_module(),
        )
    });
    if overflowed {
        Err(replay())
    } else {
        result
    }
}

fn replay_analysis(
    prepared: &SemanticWorkspacePreparedStructuralChange,
) -> Result<usize, Vec<Diagnostic>> {
    let (result, overflowed, used) =
        crate::bounded_output::with_limit_usage(MAX_ANALYSIS_BUILDER_BYTES, || {
            replay_analysis_inner(prepared)
        });
    if overflowed {
        return Err(limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES));
    }
    result?;
    Ok(used)
}

fn replay_analysis_inner(
    prepared: &SemanticWorkspacePreparedStructuralChange,
) -> Result<(), Vec<Diagnostic>> {
    let replay_prebound = semantic_workspace_change::delta_builder_prebound(
        prepared.base_graph(),
        prepared.candidate_graph(),
    )
    .map_err(|_| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;
    if !crate::bounded_output::reserve_active(replay_prebound) {
        return Err(limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES));
    }
    let changed_paths = prepared
        .operations()
        .iter()
        .flat_map(|operation| match operation {
            SemanticWorkspaceStructuralOperation::Create { path, .. }
            | SemanticWorkspaceStructuralOperation::Delete { path, .. }
            | SemanticWorkspaceStructuralOperation::Replace { path, .. } => {
                [Some(path.as_str()), None]
            }
            SemanticWorkspaceStructuralOperation::Move {
                from_path, to_path, ..
            } => [Some(from_path.as_str()), Some(to_path.as_str())],
        })
        .flatten()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let (expected_roots, expected_delta_edges) = semantic_workspace_change::build_structural_delta(
        prepared.base_graph(),
        prepared.candidate_graph(),
        &changed_paths,
    )
    .map_err(|_| replay())?;
    if expected_roots != prepared.roots() || expected_delta_edges != prepared.delta_edges() {
        return Err(replay());
    }
    drop(expected_roots);
    drop(expected_delta_edges);

    let expected_context = semantic_workspace_change::build_context_nodes(
        prepared.base_graph(),
        prepared.candidate_graph(),
        prepared.roots(),
        prepared.delta_edges(),
    )
    .map_err(|_| {
        incomplete("Semantic Workspace Structural Change Context node closure is incomplete")
    })?;
    if expected_context != prepared.context_nodes() {
        return Err(incomplete(
            "Semantic Workspace Structural Change Context node closure is incomplete",
        ));
    }
    drop(expected_context);
    let (expected_impact, expected_edges) = semantic_workspace_change::build_impact(
        prepared.base_graph(),
        prepared.candidate_graph(),
        prepared.roots(),
    )
    .map_err(|_| incomplete("Semantic Workspace Structural Change impact closure is incomplete"))?;
    if expected_impact.len() == prepared.impact().len() {
        for (expected, actual) in expected_impact.iter().zip(prepared.impact()) {
            if expected.root_provenance() != actual.root_provenance() {
                return Err(incomplete(
                    "Semantic Workspace Structural Change provenance is incomplete",
                ));
            }
            if expected.minimum_depth() != actual.minimum_depth() {
                return Err(incomplete(
                    "Semantic Workspace Structural Change impact depth is incomplete",
                ));
            }
        }
    }
    if expected_impact != prepared.impact() || expected_edges != prepared.impact_edges() {
        return Err(incomplete(
            "Semantic Workspace Structural Change impact closure is incomplete",
        ));
    }
    Ok(())
}

fn artifact_canonical() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G193",
        "Semantic Workspace Structural Change artifact is not canonical",
    )]
}

fn subtract_artifact(remaining: usize, bytes: usize) -> Result<usize, Vec<Diagnostic>> {
    remaining
        .checked_sub(bytes)
        .ok_or_else(|| limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES))
}

fn artifact_bounded(
    schema: &'static str,
    domain: &[u8],
    maximum: usize,
    field: &'static str,
    aggregate_remaining: usize,
    render: impl FnOnce(&mut CappedString),
) -> Result<Artifact, Vec<Diagnostic>> {
    let effective = maximum.min(aggregate_remaining);
    let (bytes, overflowed) = crate::bounded_output::with_limit(effective, || {
        let mut output = CappedString::new();
        render(&mut output);
        output.into_string()
    });
    if overflowed || bytes.len() > effective {
        return Err(if aggregate_remaining < maximum {
            limit("total_artifact_bytes", MAX_TOTAL_ARTIFACT_BYTES)
        } else {
            limit(field, maximum)
        });
    }
    if !bytes.ends_with('\n') || bytes[..bytes.len().saturating_sub(1)].contains('\n') {
        return Err(replay());
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
    format!("sha256:{:x}", hasher.finalize())
}

fn replay_paths(
    prepared: &SemanticWorkspacePreparedStructuralChange,
) -> Result<Vec<PathFact>, Vec<Diagnostic>> {
    let base = prepared
        .base_files()
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    let candidate = prepared
        .candidate_files()
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    if base.len() != prepared.base_files().len()
        || candidate.len() != prepared.candidate_files().len()
    {
        return Err(replay());
    }
    let mut facts = Vec::new();
    for operation in prepared.operations() {
        match operation {
            SemanticWorkspaceStructuralOperation::Create { path, source } => {
                let after = candidate.get(path.as_str()).ok_or_else(replay)?;
                if !candidate_source_matches(source, after) {
                    return Err(replay());
                }
                facts.push(path_fact(path, "created", None, None, Some(*after)));
            }
            SemanticWorkspaceStructuralOperation::Delete {
                path,
                base: binding,
            } => {
                let before = base.get(path.as_str()).ok_or_else(replay)?;
                if !base_binding_matches(binding, before) {
                    return Err(replay());
                }
                facts.push(path_fact(path, "deleted", None, Some(*before), None));
            }
            SemanticWorkspaceStructuralOperation::Move {
                from_path,
                to_path,
                base: binding,
            } => {
                let before = base.get(from_path.as_str()).ok_or_else(replay)?;
                let after = candidate.get(to_path.as_str()).ok_or_else(replay)?;
                if !base_binding_matches(binding, before)
                    || before.source_graph_schema() != after.source_graph_schema()
                    || before.source_revision() != after.source_revision()
                    || before.source_digest() != after.source_digest()
                    || before.bytes() != after.bytes()
                {
                    return Err(replay());
                }
                facts.push(path_fact(
                    from_path,
                    "moved_from",
                    Some(to_path),
                    Some(*before),
                    None,
                ));
                facts.push(path_fact(
                    to_path,
                    "moved_to",
                    Some(from_path),
                    None,
                    Some(*after),
                ));
            }
            SemanticWorkspaceStructuralOperation::Replace {
                path,
                base: binding,
                replacement_source,
            } => {
                let before = base.get(path.as_str()).ok_or_else(replay)?;
                let after = candidate.get(path.as_str()).ok_or_else(replay)?;
                if !base_binding_matches(binding, before)
                    || !candidate_source_matches(replacement_source, after)
                {
                    return Err(replay());
                }
                facts.push(path_fact(
                    path,
                    "replaced",
                    None,
                    Some(*before),
                    Some(*after),
                ));
            }
        }
    }
    facts.sort_by(|left, right| left.path.cmp(&right.path));
    if facts.len() > MAX_AFFECTED_PATHS || facts.windows(2).any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(replay());
    }
    Ok(facts)
}

fn base_binding_matches(
    binding: &super::BaseSourceBinding,
    fact: &super::StructuralBaseFileFact,
) -> bool {
    binding.source_graph_schema == fact.source_graph_schema()
        && binding.source_revision == fact.source_revision()
        && binding.source_digest == fact.source_digest()
}

fn candidate_source_matches(
    source: &str,
    fact: &crate::semantic_workspace::SemanticWorkspaceFileFact,
) -> bool {
    source == fact.source()
        && source.len() == fact.bytes()
        && graph::revision_from_canonical_source(source) == fact.source_revision()
        && review::source_digest(source.as_bytes()) == fact.source_digest()
}

fn path_fact(
    path: &str,
    change: &'static str,
    peer_path: Option<&String>,
    base: Option<&super::StructuralBaseFileFact>,
    candidate: Option<&crate::semantic_workspace::SemanticWorkspaceFileFact>,
) -> PathFact {
    PathFact {
        path: path.to_owned(),
        change,
        peer_path: peer_path.cloned(),
        base_source_graph_schema: base.map(|fact| fact.source_graph_schema().to_owned()),
        candidate_source_graph_schema: candidate.map(|fact| fact.source_graph_schema().to_owned()),
        base_source_revision: base.map(|fact| fact.source_revision().to_owned()),
        candidate_source_revision: candidate.map(|fact| fact.source_revision().to_owned()),
        base_source_digest: base.map(|fact| fact.source_digest().to_owned()),
        candidate_source_digest: candidate.map(|fact| fact.source_digest().to_owned()),
        base_bytes: base.map(super::StructuralBaseFileFact::bytes),
        candidate_bytes: candidate.map(crate::semantic_workspace::SemanticWorkspaceFileFact::bytes),
    }
}

fn replay_prepared(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    paths: &[PathFact],
) -> Result<(), Vec<Diagnostic>> {
    if prepared.operations().is_empty()
        || prepared.operations().len() > MAX_OPERATIONS
        || paths.is_empty()
        || paths.len() > MAX_AFFECTED_PATHS
        || prepared.roots().is_empty()
        || prepared.roots().len() > MAX_DELTA_ROOTS
        || prepared.delta_edges().len() > MAX_DELTA_EDGES
    {
        return Err(replay());
    }
    if prepared.context_nodes().len() > MAX_CONTEXT_NODES {
        return Err(incomplete(
            "Semantic Workspace Structural Change Context node closure is incomplete",
        ));
    }
    if prepared.impact().len() > MAX_IMPACT_NODES || prepared.impact_edges().len() > MAX_DELTA_EDGES
    {
        return Err(incomplete(
            "Semantic Workspace Structural Change impact closure is incomplete",
        ));
    }
    let context = prepared
        .context_nodes()
        .iter()
        .map(|node| (node.state(), node.kind(), node.id()))
        .collect::<BTreeSet<_>>();
    if prepared
        .roots()
        .iter()
        .any(|root| !context.contains(&(root.state(), root.kind(), root.id())))
    {
        return Err(incomplete(
            "Semantic Workspace Structural Change Context node closure is incomplete",
        ));
    }
    if prepared.impact().iter().any(|fact| {
        fact.root_provenance()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || fact
                .root_provenance()
                .iter()
                .any(|index| *index >= prepared.roots().len())
    }) {
        return Err(incomplete(
            "Semantic Workspace Structural Change provenance is incomplete",
        ));
    }
    Ok(())
}

fn usage(
    prepared: &SemanticWorkspacePreparedStructuralChange,
    paths: &[PathFact],
    sizes: ArtifactSizes,
    replay_builder_bytes: usize,
    receipt_bytes: usize,
) -> Result<Usage, Vec<Diagnostic>> {
    let total_base_source_bytes = checked_sum(
        prepared.base_files().iter().map(|file| file.bytes()),
        "total_base_source_bytes",
        MAX_TOTAL_SOURCE_BYTES,
    )?;
    let total_candidate_source_bytes = checked_sum(
        prepared.candidate_files().iter().map(|file| file.bytes()),
        "total_candidate_source_bytes",
        MAX_TOTAL_SOURCE_BYTES,
    )?;
    let impact_provenance = prepared
        .impact()
        .iter()
        .try_fold(0usize, |total, fact| {
            total.checked_add(fact.root_provenance().len())
        })
        .ok_or_else(|| {
            incomplete("Semantic Workspace Structural Change provenance is incomplete")
        })?;
    if impact_provenance > MAX_IMPACT_PROVENANCE {
        return Err(incomplete(
            "Semantic Workspace Structural Change provenance is incomplete",
        ));
    }
    let impact_depth = prepared
        .impact()
        .iter()
        .map(|fact| fact.minimum_depth())
        .max()
        .unwrap_or(0);
    if impact_depth > MAX_IMPACT_DEPTH {
        return Err(incomplete(
            "Semantic Workspace Structural Change impact depth is incomplete",
        ));
    }
    let total_artifact_bytes = [
        prepared.proposal_source().len(),
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
    let (mut created, mut deleted, mut moved, mut replaced) = (0, 0, 0, 0);
    let mut supplied = 0usize;
    for operation in prepared.operations() {
        match operation {
            SemanticWorkspaceStructuralOperation::Create { source, .. } => {
                created += 1;
                supplied = supplied.checked_add(source.len()).ok_or_else(replay)?;
            }
            SemanticWorkspaceStructuralOperation::Delete { .. } => deleted += 1,
            SemanticWorkspaceStructuralOperation::Move { .. } => moved += 1,
            SemanticWorkspaceStructuralOperation::Replace {
                replacement_source, ..
            } => {
                replaced += 1;
                supplied = supplied
                    .checked_add(replacement_source.len())
                    .ok_or_else(replay)?;
            }
        }
    }
    if supplied != prepared.used_total_supplied_source_bytes() {
        return Err(replay());
    }
    for (used, field, maximum) in [
        (
            prepared.base_files().len(),
            "base_managed_files",
            MAX_MANAGED_FILES,
        ),
        (
            prepared.candidate_files().len(),
            "candidate_managed_files",
            MAX_MANAGED_FILES,
        ),
        (prepared.operations().len(), "operations", MAX_OPERATIONS),
        (paths.len(), "affected_paths", MAX_AFFECTED_PATHS),
        (
            prepared.used_total_supplied_source_bytes(),
            "total_supplied_source_bytes",
            MAX_TOTAL_SUPPLIED_SOURCE_BYTES,
        ),
        (
            prepared.entry_module().len(),
            "entry_module_bytes",
            MAX_ENTRY_MODULE_BYTES,
        ),
        (
            prepared.proposal_source().len(),
            "proposal_bytes",
            MAX_PROPOSAL_BYTES,
        ),
        (
            prepared.candidate_manifest().len(),
            "candidate_manifest_bytes",
            MAX_CANDIDATE_MANIFEST_BYTES,
        ),
        (prepared.roots().len(), "delta_roots", MAX_DELTA_ROOTS),
        (prepared.delta_edges().len(), "delta_edges", MAX_DELTA_EDGES),
        (
            prepared.used_analysis_builder_bytes(),
            "analysis_builder_bytes",
            MAX_ANALYSIS_BUILDER_BYTES,
        ),
        (
            prepared.retained_generations(),
            "retained_generations",
            workspace::MAX_RETAINED_GENERATIONS,
        ),
        (
            prepared.staging_attempts(),
            "staging_attempts",
            workspace::MAX_STAGING_ATTEMPTS,
        ),
    ] {
        if used > maximum {
            return Err(limit(field, maximum));
        }
    }
    Ok(Usage {
        base_managed_files: prepared.base_files().len(),
        candidate_managed_files: prepared.candidate_files().len(),
        operations: prepared.operations().len(),
        affected_paths: paths.len(),
        created_files: created,
        deleted_files: deleted,
        moved_files: moved,
        replaced_files: replaced,
        total_base_source_bytes,
        total_candidate_source_bytes,
        total_supplied_source_bytes: prepared.used_total_supplied_source_bytes(),
        entry_module_bytes: prepared.entry_module().len(),
        proposal_bytes: prepared.proposal_source().len(),
        candidate_manifest_bytes: prepared.candidate_manifest().len(),
        delta_roots: prepared.roots().len(),
        delta_edges: prepared.delta_edges().len(),
        context_nodes: prepared.context_nodes().len(),
        impact_nodes: prepared.impact().len(),
        impact_provenance,
        impact_depth,
        analysis_builder_bytes: prepared
            .used_analysis_builder_bytes()
            .max(replay_builder_bytes),
        sizes,
        receipt_bytes,
        total_artifact_bytes,
        retained_generations: prepared.retained_generations(),
        staging_attempts: prepared.staging_attempts(),
    })
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

fn incomplete(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G194", message)]
}

fn render_preview(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
    paths: &[PathFact],
    proposal_digest: &str,
    manifest_digest: &str,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, PREVIEW_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json(output, MANIFEST_SCHEMA);
    push_common(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        SCHEMA,
        proposal_digest,
        prepared.proposal_source().len(),
    );
    output.push_str(",\"base_workspace_graph\":");
    push_graph_ref(output, prepared.base_workspace_graph_digest());
    output.push_str(",\"candidate_workspace_graph\":");
    push_graph_ref(output, prepared.candidate_workspace_graph_digest());
    output.push_str(",\"candidate_manifest\":");
    push_ref(
        output,
        MANIFEST_SCHEMA,
        manifest_digest,
        prepared.candidate_manifest().len(),
    );
    output.push_str(",\"paths\":");
    push_paths(output, paths);
    output.push_str(",\"delta\":{\"roots\":[");
    for (index, root) in prepared.roots().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_root(output, root);
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in prepared.delta_edges().iter().enumerate() {
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
    prepared: &SemanticWorkspacePreparedStructuralChange,
    proposal_digest: &str,
    preview: &Artifact,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, CONTEXT_SCHEMA);
    push_common(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        SCHEMA,
        proposal_digest,
        prepared.proposal_source().len(),
    );
    output.push_str(",\"structural_change_preview\":");
    push_artifact_ref(output, preview);
    output.push_str(",\"nodes\":[");
    for (index, node) in prepared.context_nodes().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"state\":");
        push_json(output, node.state());
        output.push_str(",\"kind\":");
        push_json(output, node.kind());
        output.push_str(",\"declaration_kind\":");
        push_optional(output, node.declaration_kind());
        output.push_str(",\"identity_origin\":");
        push_optional(output, node.identity_origin());
        output.push_str(",\"id\":");
        push_json(output, node.id());
        output.push_str(",\"path\":");
        push_optional(output, node.path());
        output.push_str(",\"module\":");
        push_optional(output, node.module());
        output.push('}');
    }
    output.push(']');
    push_tail(output, usage);
}

fn render_impact(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
    proposal_digest: &str,
    preview: &Artifact,
    context: &Artifact,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, IMPACT_SCHEMA);
    push_common(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        SCHEMA,
        proposal_digest,
        prepared.proposal_source().len(),
    );
    output.push_str(",\"structural_change_preview\":");
    push_artifact_ref(output, preview);
    output.push_str(",\"context\":");
    push_artifact_ref(output, context);
    output.push_str(",\"affected\":[");
    for (index, fact) in prepared.impact().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_impact_fact(output, fact);
    }
    output.push_str("],\"dependency_edges\":[");
    for (index, edge) in prepared.impact_edges().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_impact_edge(output, edge);
    }
    output.push(']');
    push_tail(output, usage);
}

fn render_review(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
    children: ChildRefs<'_>,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, REVIEW_SCHEMA);
    push_common(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        SCHEMA,
        children.proposal_digest,
        prepared.proposal_source().len(),
    );
    output.push_str(",\"structural_change_preview\":");
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
    push_tail(output, usage);
}

fn render_evidence(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
    paths: &[PathFact],
    children: ChildRefs<'_>,
    usage: Usage,
) {
    output.push_str("{\"schema\":");
    push_json(output, EVIDENCE_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json(output, MANIFEST_SCHEMA);
    push_common(output, prepared);
    output.push_str(",\"proposal\":");
    push_ref(
        output,
        SCHEMA,
        children.proposal_digest,
        prepared.proposal_source().len(),
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
        prepared.candidate_manifest().len(),
    );
    output.push_str(",\"structural_change_preview\":");
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
            .expect("typed rendering always retains Review"),
    );
    output.push_str(",\"paths\":");
    push_paths(output, paths);
    output.push_str(",\"limits\":");
    push_limits(output);
    output.push_str(",\"budget\":");
    push_budget(output, usage);
    output.push_str(",\"nonclaims\":");
    push_nonclaims(output);
    output.push_str("}\n");
}

fn push_common(output: &mut CappedString, prepared: &SemanticWorkspacePreparedStructuralChange) {
    output.push_str(",\"base_workspace_revision\":");
    push_json(output, prepared.base_workspace_revision());
    output.push_str(",\"candidate_workspace_revision\":");
    push_json(output, prepared.candidate_workspace_revision());
    output.push_str(",\"entry_module\":");
    push_json(output, prepared.entry_module());
}

fn push_paths(output: &mut CappedString, paths: &[PathFact]) {
    output.push('[');
    for (index, fact) in paths.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        push_json(output, &fact.path);
        output.push_str(",\"change\":");
        push_json(output, fact.change);
        output.push_str(",\"peer_path\":");
        push_optional(output, fact.peer_path.as_deref());
        output.push_str(",\"base_source_graph_schema\":");
        push_optional(output, fact.base_source_graph_schema.as_deref());
        output.push_str(",\"candidate_source_graph_schema\":");
        push_optional(output, fact.candidate_source_graph_schema.as_deref());
        output.push_str(",\"base_source_revision\":");
        push_optional(output, fact.base_source_revision.as_deref());
        output.push_str(",\"candidate_source_revision\":");
        push_optional(output, fact.candidate_source_revision.as_deref());
        output.push_str(",\"base_source_digest\":");
        push_optional(output, fact.base_source_digest.as_deref());
        output.push_str(",\"candidate_source_digest\":");
        push_optional(output, fact.candidate_source_digest.as_deref());
        output.push_str(",\"base_bytes\":");
        push_optional_usize(output, fact.base_bytes);
        output.push_str(",\"candidate_bytes\":");
        push_optional_usize(output, fact.candidate_bytes);
        output.push('}');
    }
    output.push(']');
}

fn push_root(
    output: &mut CappedString,
    root: &semantic_workspace_change::SemanticWorkspaceChangeRoot,
) {
    output.push_str("{\"state\":");
    push_json(output, root.state());
    output.push_str(",\"kind\":");
    push_json(output, root.kind());
    output.push_str(",\"id\":");
    push_json(output, root.id());
    output.push_str(",\"path\":");
    push_optional(output, root.path());
    output.push_str(",\"module\":");
    push_optional(output, root.module());
    output.push_str(",\"change\":");
    push_json(output, root.change());
    output.push_str(",\"identity_origin\":");
    push_optional(output, root.identity_origin());
    output.push('}');
}

fn push_delta_edge(
    output: &mut CappedString,
    fact: &semantic_workspace_change::SemanticWorkspaceChangeEdge,
) {
    output.push_str("{\"state\":");
    push_json(output, fact.state());
    output.push_str(",\"change\":");
    push_json(output, fact.change());
    output.push_str(",\"edge\":");
    push_edge(output, fact.edge());
    output.push('}');
}

fn push_impact_edge(
    output: &mut CappedString,
    fact: &semantic_workspace_change::SemanticWorkspaceChangeImpactEdge,
) {
    output.push_str("{\"state\":");
    push_json(output, fact.state());
    output.push_str(",\"edge\":");
    push_edge(output, fact.edge());
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
    write!(output, ",\"ordinal\":{}}}", edge.ordinal()).expect("string write");
}

fn push_impact_fact(
    output: &mut CappedString,
    fact: &semantic_workspace_change::SemanticWorkspaceChangeImpactFact,
) {
    output.push_str("{\"state\":");
    push_json(output, fact.state());
    output.push_str(",\"kind\":");
    push_json(output, fact.kind());
    output.push_str(",\"declaration_kind\":");
    push_optional(output, fact.declaration_kind());
    output.push_str(",\"identity_origin\":");
    push_optional(output, fact.identity_origin());
    output.push_str(",\"id\":");
    push_json(output, fact.id());
    output.push_str(",\"path\":");
    push_optional(output, fact.path());
    output.push_str(",\"module\":");
    push_optional(output, fact.module());
    write!(output, ",\"minimum_depth\":{}", fact.minimum_depth()).expect("string write");
    output.push_str(",\"role\":");
    push_json(output, fact.impact_role());
    output.push_str(",\"reasons\":[");
    for (index, reason) in fact.reasons().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json(output, reason);
    }
    output.push_str("],\"root_provenance\":[");
    for (index, root) in fact.root_provenance().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{root}").expect("string write");
    }
    output.push_str("]}");
}

#[derive(Clone, Copy)]
struct EvidenceOffsets {
    delta_edges: usize,
    affected: usize,
    dependency_edges: usize,
    end: usize,
}
impl EvidenceOffsets {
    fn new(prepared: &SemanticWorkspacePreparedStructuralChange) -> Self {
        let delta_edges = prepared.roots().len();
        let context_nodes = delta_edges + prepared.delta_edges().len();
        let affected = context_nodes + prepared.context_nodes().len();
        let dependency_edges = affected + prepared.impact().len();
        Self {
            delta_edges,
            affected,
            dependency_edges,
            end: dependency_edges + prepared.impact_edges().len(),
        }
    }
}

fn push_review_evidence(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
) {
    let groups = [
        (
            "structural_change_preview",
            "delta_root",
            prepared.roots().len(),
        ),
        (
            "structural_change_preview",
            "delta_edge",
            prepared.delta_edges().len(),
        ),
        ("context", "context_node", prepared.context_nodes().len()),
        ("impact", "affected", prepared.impact().len()),
        ("impact", "dependency_edge", prepared.impact_edges().len()),
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
            write!(output, ",\"index\":{index}").expect("string write");
            output.push_str(",\"relation\":");
            push_json(output, relation);
            output.push('}');
        }
    }
}

fn push_review_sections(
    output: &mut CappedString,
    prepared: &SemanticWorkspacePreparedStructuralChange,
) {
    let declaration = prepared
        .roots()
        .iter()
        .any(|root| root.kind() == "declaration");
    let security = prepared
        .roots()
        .iter()
        .any(|root| root.kind() == "capability")
        || prepared.delta_edges().iter().any(|fact| {
            matches!(
                fact.edge().kind(),
                "effect_requirement" | "capability_authority"
            )
        });
    let offsets = EvidenceOffsets::new(prepared);
    push_section(
        output,
        "behavior",
        "change_proven",
        "SWSC-BEHAVIOR-DELTA",
        "Authenticated behavior delta and reverse impact are represented by the indexed evidence.",
        "review_required",
        offsets.delta_edges..offsets.end,
    );
    output.push(',');
    push_section(
        output,
        "api_identity",
        if declaration {
            "change_proven"
        } else {
            "unchanged_within_admitted_domain"
        },
        "SWSC-API-IDENTITY-DELTA",
        "Authenticated declaration identity changes are represented by the indexed preview roots.",
        "review_required",
        prepared
            .roots()
            .iter()
            .enumerate()
            .filter(|(_, root)| root.kind() == "declaration")
            .map(|(index, _)| index),
    );
    output.push(',');
    push_section(output, "security_authority", if security { "change_proven" } else { "unchanged_within_admitted_domain" },
        "SWSC-SECURITY-AUTHORITY-DELTA", "Authenticated capability and effect-authority changes are represented by the indexed evidence.", "review_required",
        prepared.roots().iter().enumerate().filter(|(_, root)| root.kind() == "capability").map(|(index, _)| index)
            .chain(prepared.delta_edges().iter().enumerate().filter(|(_, fact)| matches!(fact.edge().kind(), "effect_requirement" | "capability_authority")).map(|(index, _)| offsets.delta_edges + index)));
    output.push(',');
    push_section(
        output,
        "memory_ownership",
        "unknown",
        "SWSC-MEMORY-OWNERSHIP-UNASSESSED",
        "No general cross-file memory-ownership compatibility claim is established.",
        "no_claim",
        std::iter::empty(),
    );
    output.push(',');
    push_section(
        output,
        "target_artifact",
        "unknown",
        "SWSC-TARGET-ARTIFACT-UNASSESSED",
        "No target artifact is emitted, executed, or verified.",
        "no_claim",
        std::iter::empty(),
    );
    output.push(',');
    push_section(output, "migration", "change_proven", "SWSC-MIGRATION-STRUCTURAL",
        "The proposal is a managed semantic-workspace structural migration with explicit create, delete, move, or replacement operations.", "review_required",
        (0..prepared.roots().len()).chain(offsets.affected..offsets.dependency_edges));
    output.push(',');
    push_section(
        output,
        "unsafe",
        "unknown",
        "SWSC-UNSAFE-UNASSESSED",
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
        write!(output, "{reference}").expect("string write");
    }
    output.push_str("]}]}");
}

fn push_limits(output: &mut CappedString) {
    write!(output,
        "{{\"max_managed_files\":16,\"max_operations\":16,\"max_affected_paths\":32,\"max_path_bytes\":{MAX_PATH_BYTES},\"max_source_bytes_per_operation\":{MAX_SOURCE_BYTES_PER_OPERATION},\"max_total_base_source_bytes\":{MAX_TOTAL_SOURCE_BYTES},\"max_total_candidate_source_bytes\":{MAX_TOTAL_SOURCE_BYTES},\"max_total_supplied_source_bytes\":{MAX_TOTAL_SUPPLIED_SOURCE_BYTES},\"max_entry_module_bytes\":{MAX_ENTRY_MODULE_BYTES},\"max_proposal_bytes\":{MAX_PROPOSAL_BYTES},\"max_candidate_manifest_bytes\":{MAX_CANDIDATE_MANIFEST_BYTES},\"max_delta_roots\":{MAX_DELTA_ROOTS},\"max_delta_edges\":{MAX_DELTA_EDGES},\"max_context_nodes\":{MAX_CONTEXT_NODES},\"max_impact_nodes\":{MAX_IMPACT_NODES},\"max_impact_provenance\":{MAX_IMPACT_PROVENANCE},\"max_impact_depth\":{MAX_IMPACT_DEPTH},\"max_analysis_builder_bytes\":{MAX_ANALYSIS_BUILDER_BYTES},\"max_structural_change_preview_bytes\":{MAX_PREVIEW_BYTES},\"max_context_bytes\":{MAX_CONTEXT_BYTES},\"max_impact_bytes\":{MAX_IMPACT_BYTES},\"max_review_bytes\":{MAX_REVIEW_BYTES},\"max_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_receipt_bytes\":{MAX_RECEIPT_BYTES},\"max_total_artifact_bytes\":{MAX_TOTAL_ARTIFACT_BYTES},\"max_json_depth\":8,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}}"
    ).expect("string write");
}

fn push_budget(output: &mut CappedString, usage: Usage) {
    write!(output,
        "{{\"used_base_managed_files\":{},\"used_candidate_managed_files\":{},\"used_operations\":{},\"used_affected_paths\":{},\"used_created_files\":{},\"used_deleted_files\":{},\"used_moved_files\":{},\"used_replaced_files\":{},\"used_total_base_source_bytes\":{},\"used_total_candidate_source_bytes\":{},\"used_total_supplied_source_bytes\":{},\"used_entry_module_bytes\":{},\"used_proposal_bytes\":{},\"used_candidate_manifest_bytes\":{},\"used_delta_roots\":{},\"used_delta_edges\":{},\"used_context_nodes\":{},\"used_impact_nodes\":{},\"used_impact_provenance\":{},\"used_impact_depth\":{},\"used_analysis_builder_bytes\":{},\"used_structural_change_preview_bytes\":{},\"used_context_bytes\":{},\"used_impact_bytes\":{},\"used_review_bytes\":{},\"used_evidence_bytes\":{},\"used_receipt_bytes\":{},\"used_total_artifact_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":0}}",
        usage.base_managed_files, usage.candidate_managed_files, usage.operations, usage.affected_paths,
        usage.created_files, usage.deleted_files, usage.moved_files, usage.replaced_files,
        usage.total_base_source_bytes, usage.total_candidate_source_bytes, usage.total_supplied_source_bytes,
        usage.entry_module_bytes, usage.proposal_bytes, usage.candidate_manifest_bytes, usage.delta_roots, usage.delta_edges,
        usage.context_nodes, usage.impact_nodes, usage.impact_provenance, usage.impact_depth, usage.analysis_builder_bytes,
        usage.sizes.preview, usage.sizes.context, usage.sizes.impact, usage.sizes.review, usage.sizes.evidence, usage.receipt_bytes,
        usage.total_artifact_bytes, usage.retained_generations, usage.staging_attempts).expect("string write");
}

fn push_tail(output: &mut CappedString, usage: Usage) {
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
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json(output, value);
    }
    output.push(']');
}
fn push_ref(output: &mut CappedString, schema: &str, digest: &str, bytes: usize) {
    output.push_str("{\"schema\":");
    push_json(output, schema);
    output.push_str(",\"digest\":");
    push_json(output, digest);
    write!(output, ",\"bytes\":{bytes}}}").expect("string write");
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
fn push_optional(output: &mut CappedString, value: Option<&str>) {
    if let Some(value) = value {
        push_json(output, value);
    } else {
        output.push_str("null");
    }
}
fn push_optional_usize(output: &mut CappedString, value: Option<usize>) {
    if let Some(value) = value {
        write!(output, "{value}").expect("string write");
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
            value if value.is_control() => {
                write!(output, "\\u{:04x}", value as u32).expect("string write");
            }
            value => output.push(value),
        }
    }
    output.push('"');
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "private renderer tests remain adjacent to JSON helpers while production replay helpers follow"
)]
mod tests {
    use super::*;
    use crate::semantic_workspace_structural_change::tests::{base_fixture, mixed_proposal};
    use serde_json::Value;

    fn prepared() -> SemanticWorkspacePreparedStructuralChange {
        let base = base_fixture();
        let proposal = super::super::parse_proposal(&mixed_proposal(&base)).unwrap();
        super::super::prepare_owned(
            base.revision,
            base.sources,
            base.graph,
            (base.manifest_bytes, 1, 0),
            proposal,
        )
        .unwrap()
    }

    fn raw_sha(source: &str) -> String {
        format!("sha256:{:x}", Sha256::digest(source.as_bytes()))
    }

    fn artifact_error(
        result: Result<SemanticWorkspaceStructuralChangeArtifacts, Vec<Diagnostic>>,
    ) -> Vec<Diagnostic> {
        match result {
            Ok(_) => panic!("expected structural artifact rendering to fail"),
            Err(diagnostics) => diagnostics,
        }
    }

    fn assert_artifacts_equal(
        actual: &SemanticWorkspaceStructuralChangeArtifacts,
        expected: &SemanticWorkspaceStructuralChangeArtifacts,
    ) {
        assert_eq!(
            [
                actual.proposal_digest(),
                actual.candidate_manifest_digest(),
                actual.preview(),
                actual.preview_digest(),
                actual.context(),
                actual.context_digest(),
                actual.impact(),
                actual.impact_digest(),
                actual.review(),
                actual.review_digest(),
                actual.evidence(),
                actual.evidence_digest(),
            ],
            [
                expected.proposal_digest(),
                expected.candidate_manifest_digest(),
                expected.preview(),
                expected.preview_digest(),
                expected.context(),
                expected.context_digest(),
                expected.impact(),
                expected.impact_digest(),
                expected.review(),
                expected.review_digest(),
                expected.evidence(),
                expected.evidence_digest(),
            ]
        );
    }

    fn top_keys(source: &str) -> Vec<String> {
        let bytes = source.as_bytes();
        let mut keys = Vec::new();
        let mut index = 0usize;
        let mut depth = 0usize;
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
        assert_eq!(value[key]["schema"], artifact.schema);
        assert_eq!(value[key]["digest"], artifact.digest);
        assert_eq!(value[key]["bytes"], artifact.bytes.len());
    }

    fn assert_canonical_path_row(source: &str, row: &Value) {
        let keys = [
            "path",
            "change",
            "peer_path",
            "base_source_graph_schema",
            "candidate_source_graph_schema",
            "base_source_revision",
            "candidate_source_revision",
            "base_source_digest",
            "candidate_source_digest",
            "base_bytes",
            "candidate_bytes",
        ];
        let fields = keys
            .iter()
            .map(|key| format!("\"{key}\":{}", row[*key]))
            .collect::<Vec<_>>()
            .join(",");
        assert!(source.contains(&format!("{{{fields}}}")));
    }

    fn object_after<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source.find(marker).unwrap() + marker.len();
        assert_eq!(source.as_bytes()[start], b'{');
        let mut depth = 0usize;
        let mut string = false;
        let mut escape = false;
        for (offset, byte) in source.as_bytes()[start..].iter().copied().enumerate() {
            if string {
                if escape {
                    escape = false;
                } else if byte == b'\\' {
                    escape = true;
                } else if byte == b'"' {
                    string = false;
                }
                continue;
            }
            match byte {
                b'"' => string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[start..=start + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("object after marker is unterminated")
    }

    fn rerender_proposal(prepared: &mut SemanticWorkspacePreparedStructuralChange) {
        prepared.proposal_source = super::super::render_proposal_facts(
            prepared.base_workspace_revision(),
            prepared.entry_module(),
            prepared.operations(),
        )
        .unwrap();
    }

    #[test]
    fn whole_document_kats_keys_domains_refs_paths_and_fixed_point_are_exact() {
        let prepared = prepared();
        let artifacts = render_artifacts(&prepared).unwrap();
        assert_eq!(
            raw_sha(prepared.proposal_source()),
            "sha256:b13dcbf801bdb0fe1cd05a5cff26b58085bc32a576d9a5b8fc7264755c5548f8"
        );
        assert_eq!(
            [
                raw_sha(artifacts.preview()),
                raw_sha(artifacts.context()),
                raw_sha(artifacts.impact()),
                raw_sha(artifacts.review()),
                raw_sha(artifacts.evidence()),
            ],
            [
                "sha256:1bf3eadefd58b3fa92c06e830979ba782b5ede6563f70a0ae7eeff5ca41e76d0",
                "sha256:fd06527f1ae53b3e38218f419cef8e48a723319e05962106f38ecaa7b4561a3d",
                "sha256:8adf3902746a7d8d316e67f015ddd4f103b04fed8f5b8d42bd726ddcac46c57f",
                "sha256:252c99954b7e0e82b288df2d536c9d413e875a3d1373fcb492b9414ea5a43809",
                "sha256:c163c425df9f6fefb354989453d7770174637c0aca5d1cad7f0f0cc7e56d2dac",
            ]
        );

        assert_eq!(
            artifacts.proposal_digest(),
            digest(
                PROPOSAL_DIGEST_DOMAIN,
                prepared.proposal_source().as_bytes()
            )
        );
        assert_eq!(
            artifacts.candidate_manifest_digest(),
            digest(
                CANDIDATE_MANIFEST_DIGEST_DOMAIN,
                prepared.candidate_manifest().as_bytes(),
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
            let mut mutated = artifact.bytes.as_bytes().to_vec();
            let middle = mutated.len() / 2;
            mutated[middle] ^= 1;
            assert_ne!(artifact.digest, digest(domain, &mutated));
        }

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
                "paths",
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
                "structural_change_preview",
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
                "structural_change_preview",
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
                "structural_change_preview",
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
                "structural_change_preview",
                "context",
                "impact",
                "review",
                "paths",
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
            assert_eq!(value["proposal"]["bytes"], prepared.proposal_source().len());
        }
        assert_reference(&context, "structural_change_preview", &artifacts.preview);
        assert_reference(&impact, "structural_change_preview", &artifacts.preview);
        assert_reference(&impact, "context", &artifacts.context);
        assert_reference(&review, "structural_change_preview", &artifacts.preview);
        assert_reference(&review, "context", &artifacts.context);
        assert_reference(&review, "impact", &artifacts.impact);
        assert_reference(&evidence, "structural_change_preview", &artifacts.preview);
        assert_reference(&evidence, "context", &artifacts.context);
        assert_reference(&evidence, "impact", &artifacts.impact);
        assert_reference(&evidence, "review", &artifacts.review);

        let paths = preview["paths"].as_array().unwrap();
        assert_eq!(
            paths
                .iter()
                .map(|row| (
                    row["path"].as_str().unwrap(),
                    row["change"].as_str().unwrap()
                ))
                .collect::<Vec<_>>(),
            [
                ("a/provider.spx", "moved_from"),
                ("b/created.spx", "created"),
                ("c/provider.spx", "moved_to"),
                ("n/island.spx", "deleted"),
                ("z/entry.spx", "replaced"),
            ]
        );
        assert_eq!(paths[0]["peer_path"], "c/provider.spx");
        assert_eq!(paths[2]["peer_path"], "a/provider.spx");
        for row in paths {
            assert_canonical_path_row(artifacts.preview(), row);
            assert_canonical_path_row(artifacts.evidence(), row);
        }

        let budget = &preview["budget"];
        for value in [&context, &impact, &review, &evidence] {
            assert_eq!(&value["budget"], budget);
        }
        assert_eq!(budget["used_operations"], 4);
        assert_eq!(budget["used_affected_paths"], 5);
        assert_eq!(budget["used_created_files"], 1);
        assert_eq!(budget["used_deleted_files"], 1);
        assert_eq!(budget["used_moved_files"], 1);
        assert_eq!(budget["used_replaced_files"], 1);
        assert_eq!(
            budget["used_structural_change_preview_bytes"],
            artifacts.preview().len()
        );
        assert_eq!(budget["used_context_bytes"], artifacts.context().len());
        assert_eq!(budget["used_impact_bytes"], artifacts.impact().len());
        assert_eq!(budget["used_review_bytes"], artifacts.review().len());
        assert_eq!(budget["used_evidence_bytes"], artifacts.evidence().len());
        assert_eq!(
            budget["used_total_artifact_bytes"],
            prepared.proposal_source().len()
                + artifacts.preview().len()
                + artifacts.context().len()
                + artifacts.impact().len()
                + artifacts.review().len()
                + artifacts.evidence().len()
        );

        let expected_nonclaims = NONCLAIMS.into_iter().map(Value::from).collect::<Vec<_>>();
        for value in [&preview, &context, &impact, &review, &evidence] {
            assert_eq!(
                value["nonclaims"].as_array().unwrap().as_slice(),
                expected_nonclaims.as_slice()
            );
        }
        assert_eq!(
            top_keys(object_after(artifacts.review(), "\"sections\":")),
            [
                "behavior",
                "api_identity",
                "security_authority",
                "memory_ownership",
                "target_artifact",
                "migration",
                "unsafe",
            ]
        );
        let sections = &review["sections"];
        let literals = [
            (
                "behavior",
                "change_proven",
                "SWSC-BEHAVIOR-DELTA",
                "Authenticated behavior delta and reverse impact are represented by the indexed evidence.",
                "review_required",
                true,
            ),
            (
                "api_identity",
                "change_proven",
                "SWSC-API-IDENTITY-DELTA",
                "Authenticated declaration identity changes are represented by the indexed preview roots.",
                "review_required",
                true,
            ),
            (
                "security_authority",
                "change_proven",
                "SWSC-SECURITY-AUTHORITY-DELTA",
                "Authenticated capability and effect-authority changes are represented by the indexed evidence.",
                "review_required",
                true,
            ),
            (
                "memory_ownership",
                "unknown",
                "SWSC-MEMORY-OWNERSHIP-UNASSESSED",
                "No general cross-file memory-ownership compatibility claim is established.",
                "no_claim",
                false,
            ),
            (
                "target_artifact",
                "unknown",
                "SWSC-TARGET-ARTIFACT-UNASSESSED",
                "No target artifact is emitted, executed, or verified.",
                "no_claim",
                false,
            ),
            (
                "migration",
                "change_proven",
                "SWSC-MIGRATION-STRUCTURAL",
                "The proposal is a managed semantic-workspace structural migration with explicit create, delete, move, or replacement operations.",
                "review_required",
                true,
            ),
            (
                "unsafe",
                "unknown",
                "SWSC-UNSAFE-UNASSESSED",
                "No general unsafe, ABI, or foreign-code analysis is established.",
                "no_claim",
                false,
            ),
        ];
        for (name, assessment, code, statement, disposition, has_evidence) in literals {
            let section = &sections[name];
            assert_eq!(section["assessment"], assessment);
            let finding = &section["findings"][0];
            assert_eq!(finding["code"], code);
            assert_eq!(finding["statement"], statement);
            assert_eq!(finding["disposition"], disposition);
            assert_eq!(
                !finding["evidence"].as_array().unwrap().is_empty(),
                has_evidence
            );
        }
        let offsets = EvidenceOffsets::new(&prepared);
        let section_evidence = |name: &str| {
            sections[name]["findings"][0]["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            section_evidence("behavior"),
            (offsets.delta_edges..offsets.end).collect::<Vec<_>>()
        );
        assert_eq!(
            section_evidence("api_identity"),
            prepared
                .roots()
                .iter()
                .enumerate()
                .filter(|(_, root)| root.kind() == "declaration")
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            section_evidence("security_authority"),
            prepared
                .roots()
                .iter()
                .enumerate()
                .filter(|(_, root)| root.kind() == "capability")
                .map(|(index, _)| index)
                .chain(
                    prepared
                        .delta_edges()
                        .iter()
                        .enumerate()
                        .filter(|(_, fact)| {
                            matches!(
                                fact.edge().kind(),
                                "effect_requirement" | "capability_authority"
                            )
                        })
                        .map(|(index, _)| offsets.delta_edges + index),
                )
                .collect::<Vec<_>>()
        );
        assert_eq!(
            section_evidence("migration"),
            (0..prepared.roots().len())
                .chain(offsets.affected..offsets.dependency_edges)
                .collect::<Vec<_>>()
        );
        for name in ["memory_ownership", "target_artifact", "unsafe"] {
            assert!(section_evidence(name).is_empty());
        }
    }

    #[test]
    fn review_evidence_is_complete_ordered_and_exactly_references_children() {
        let prepared = prepared();
        let artifacts = render_artifacts(&prepared).unwrap();
        let review: Value = serde_json::from_str(artifacts.review()).unwrap();
        let actual = review["evidence"].as_array().unwrap();
        let groups = [
            (
                "structural_change_preview",
                "delta_root",
                prepared.roots().len(),
            ),
            (
                "structural_change_preview",
                "delta_edge",
                prepared.delta_edges().len(),
            ),
            ("context", "context_node", prepared.context_nodes().len()),
            ("impact", "affected", prepared.impact().len()),
            ("impact", "dependency_edge", prepared.impact_edges().len()),
        ];
        let expected = groups
            .into_iter()
            .flat_map(|(artifact, relation, count)| {
                (0..count).map(move |index| (artifact, relation, index))
            })
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len());
        for (fact, (artifact, relation, index)) in actual.iter().zip(expected) {
            assert_eq!(fact["artifact"], artifact);
            assert_eq!(fact["relation"], relation);
            assert_eq!(fact["index"], index);
        }
    }

    #[test]
    fn output_caps_replay_mutations_and_incomplete_facts_fail_closed() {
        let rendered_prepared = prepared();
        let artifacts = render_artifacts(&rendered_prepared).unwrap();
        for (source, schema, domain, field) in [
            (
                &artifacts.preview,
                PREVIEW_SCHEMA,
                PREVIEW_DIGEST_DOMAIN,
                "structural_change_preview_bytes",
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
            let exact = artifact_bounded(
                schema,
                domain,
                source.bytes.len(),
                field,
                usize::MAX,
                |output| output.push_str(&source.bytes),
            )
            .unwrap();
            assert_eq!(exact, *source);
            let diagnostics = artifact_bounded(
                schema,
                domain,
                source.bytes.len() - 1,
                field,
                usize::MAX,
                |output| output.push_str(&source.bytes),
            )
            .unwrap_err();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, "SPX-G191");
        }
        let aggregate_exact = artifact_bounded(
            EVIDENCE_SCHEMA,
            EVIDENCE_DIGEST_DOMAIN,
            MAX_EVIDENCE_BYTES,
            "evidence_bytes",
            artifacts.evidence.bytes.len(),
            |output| output.push_str(&artifacts.evidence.bytes),
        )
        .unwrap();
        assert_eq!(aggregate_exact, artifacts.evidence);
        let diagnostics = artifact_bounded(
            EVIDENCE_SCHEMA,
            EVIDENCE_DIGEST_DOMAIN,
            MAX_EVIDENCE_BYTES,
            "evidence_bytes",
            artifacts.evidence.bytes.len() - 1,
            |output| output.push_str(&artifacts.evidence.bytes),
        )
        .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G191");
        assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: total_artifact_bytes maximum {MAX_TOTAL_ARTIFACT_BYTES}"
            )
        );

        let mut proposal_mutation = prepared();
        proposal_mutation.entry_module.push_str(".mutated");
        assert_eq!(
            artifact_error(render_artifacts(&proposal_mutation))[0].code,
            "SPX-G193"
        );
        let mut manifest_mutation = prepared();
        let digit = manifest_mutation
            .candidate_manifest
            .find("sha256:")
            .unwrap()
            + "sha256:".len();
        manifest_mutation
            .candidate_manifest
            .replace_range(digit..digit + 1, "0");
        assert_eq!(
            artifact_error(render_artifacts(&manifest_mutation))[0].code,
            "SPX-G192"
        );

        let mut incomplete_context = prepared();
        incomplete_context.context_nodes.remove(0);
        assert_eq!(
            artifact_error(render_artifacts(&incomplete_context))[0].code,
            "SPX-G194"
        );
        let mut incomplete_impact = prepared();
        incomplete_impact.impact.remove(0);
        assert_eq!(
            artifact_error(render_artifacts(&incomplete_impact))[0].code,
            "SPX-G194"
        );
    }

    #[test]
    fn operation_base_graph_revision_and_supplied_budget_bridges_fail_closed() {
        for case in [
            "create_source",
            "delete_base",
            "move_base",
            "replace_base",
            "replace_source",
        ] {
            let mut mutation = prepared();
            let operation = mutation
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(
                        (case, operation),
                        (
                            "create_source",
                            SemanticWorkspaceStructuralOperation::Create { .. }
                        ) | (
                            "delete_base",
                            SemanticWorkspaceStructuralOperation::Delete { .. }
                        ) | (
                            "move_base",
                            SemanticWorkspaceStructuralOperation::Move { .. }
                        ) | (
                            "replace_base" | "replace_source",
                            SemanticWorkspaceStructuralOperation::Replace { .. }
                        )
                    )
                })
                .unwrap();
            match (case, operation) {
                ("create_source", SemanticWorkspaceStructuralOperation::Create { source, .. })
                | (
                    "replace_source",
                    SemanticWorkspaceStructuralOperation::Replace {
                        replacement_source: source,
                        ..
                    },
                ) => source.push('x'),
                ("delete_base", SemanticWorkspaceStructuralOperation::Delete { base, .. })
                | ("move_base", SemanticWorkspaceStructuralOperation::Move { base, .. })
                | ("replace_base", SemanticWorkspaceStructuralOperation::Replace { base, .. }) => {
                    base.source_digest = format!("sha256:{}", "f".repeat(64));
                }
                _ => unreachable!(),
            }
            rerender_proposal(&mut mutation);
            assert_eq!(
                artifact_error(render_artifacts(&mutation))[0].code,
                "SPX-G192",
                "{case}"
            );
        }

        let mut base_manifest_bytes = prepared();
        base_manifest_bytes.base_manifest_bytes += 1;
        assert_eq!(
            artifact_error(render_artifacts(&base_manifest_bytes))[0].code,
            "SPX-G192"
        );
        let mut base_revision = prepared();
        base_revision.base_workspace_revision = format!("sha256:{}", "e".repeat(64));
        rerender_proposal(&mut base_revision);
        assert_eq!(
            artifact_error(render_artifacts(&base_revision))[0].code,
            "SPX-G192"
        );
        let mut candidate_revision = prepared();
        candidate_revision.candidate_workspace_revision = format!("sha256:{}", "c".repeat(64));
        assert_eq!(
            artifact_error(render_artifacts(&candidate_revision))[0].code,
            "SPX-G192"
        );
        let mut base_fact = prepared();
        base_fact.base_files[0].source_digest = format!("sha256:{}", "b".repeat(64));
        assert_eq!(
            artifact_error(render_artifacts(&base_fact))[0].code,
            "SPX-G192"
        );
        for candidate in [false, true] {
            let mut graph = prepared();
            if candidate {
                graph.candidate_workspace_graph_digest = format!("sha256:{}", "d".repeat(64));
            } else {
                graph.base_workspace_graph_digest = format!("sha256:{}", "d".repeat(64));
            }
            assert_eq!(artifact_error(render_artifacts(&graph))[0].code, "SPX-G192");
        }
        let mut supplied = prepared();
        supplied.used_total_supplied_source_bytes += 1;
        assert_eq!(
            artifact_error(render_artifacts(&supplied))[0].code,
            "SPX-G192"
        );
    }

    fn analysis_replay_with_limit(cap: usize) -> Result<usize, Vec<Diagnostic>> {
        let prepared = prepared();
        let (result, overflowed, used) =
            crate::bounded_output::with_limit_usage(cap, || replay_analysis_inner(&prepared));
        if overflowed {
            Err(limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))
        } else {
            result.map(|()| used)
        }
    }

    #[test]
    fn analysis_replay_has_an_exact_minimum_successful_boundary() {
        let mut low = 0usize;
        let mut high = MAX_ANALYSIS_BUILDER_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if analysis_replay_with_limit(middle).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(low > 0);
        assert_eq!(analysis_replay_with_limit(low).unwrap(), low);
        let diagnostics = analysis_replay_with_limit(low - 1).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G191");
        assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: analysis_builder_bytes maximum {MAX_ANALYSIS_BUILDER_BYTES}"
            )
        );
    }

    #[test]
    fn aggregate_artifact_render_has_an_exact_minimum_successful_boundary() {
        let prepared = prepared();
        let expected = render_artifacts(&prepared).unwrap();
        let expected_total = prepared.proposal_source().len()
            + expected.preview.bytes.len()
            + expected.context.bytes.len()
            + expected.impact.bytes.len()
            + expected.review.bytes.len()
            + expected.evidence.bytes.len();

        let mut low = 0usize;
        let mut high = MAX_TOTAL_ARTIFACT_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if render_artifacts_with_total_limit(&prepared, middle).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }

        assert_eq!(low, expected_total);
        let actual = render_artifacts_with_total_limit(&prepared, low).unwrap();
        assert_artifacts_equal(&actual, &expected);
        let diagnostics = artifact_error(render_artifacts_with_total_limit(&prepared, low - 1));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G191");
        assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: total_artifact_bytes maximum {MAX_TOTAL_ARTIFACT_BYTES}"
            )
        );
    }

    #[test]
    #[should_panic]
    fn aggregate_artifact_test_limit_cannot_exceed_production_limit() {
        let prepared = prepared();
        let _ = render_artifacts_with_total_limit(&prepared, MAX_TOTAL_ARTIFACT_BYTES + 1);
    }

    #[test]
    fn verification_receipt_individual_and_aggregate_limits_are_exact() {
        let prepared = prepared();
        let artifacts = render_artifacts(&prepared).unwrap();
        let expected =
            render_verification_receipt(&prepared, &artifacts, artifacts.evidence.bytes.len())
                .unwrap();

        let mut low = 0usize;
        let mut high = MAX_RECEIPT_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if render_verification_receipt_with_limits(
                &prepared,
                &artifacts,
                artifacts.evidence.bytes.len(),
                middle,
                MAX_TOTAL_ARTIFACT_BYTES,
            )
            .is_ok()
            {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert_eq!(low, expected.len());
        assert_eq!(
            render_verification_receipt_with_limits(
                &prepared,
                &artifacts,
                artifacts.evidence.bytes.len(),
                low,
                MAX_TOTAL_ARTIFACT_BYTES,
            )
            .unwrap(),
            expected
        );
        let diagnostics = render_verification_receipt_with_limits(
            &prepared,
            &artifacts,
            artifacts.evidence.bytes.len(),
            low - 1,
            MAX_TOTAL_ARTIFACT_BYTES,
        )
        .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G191");
        assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: receipt_bytes maximum {MAX_RECEIPT_BYTES}"
            )
        );

        let expected_total = artifacts.usage.total_artifact_bytes + expected.len();
        low = 0;
        high = MAX_TOTAL_ARTIFACT_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if render_verification_receipt_with_limits(
                &prepared,
                &artifacts,
                artifacts.evidence.bytes.len(),
                MAX_RECEIPT_BYTES,
                middle,
            )
            .is_ok()
            {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert_eq!(low, expected_total);
        assert_eq!(
            render_verification_receipt_with_limits(
                &prepared,
                &artifacts,
                artifacts.evidence.bytes.len(),
                MAX_RECEIPT_BYTES,
                low,
            )
            .unwrap(),
            expected
        );
        let diagnostics = render_verification_receipt_with_limits(
            &prepared,
            &artifacts,
            artifacts.evidence.bytes.len(),
            MAX_RECEIPT_BYTES,
            low - 1,
        )
        .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G191");
        assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: total_artifact_bytes maximum {MAX_TOTAL_ARTIFACT_BYTES}"
            )
        );
    }

    #[test]
    fn verification_receipt_test_limits_cannot_exceed_production_limits() {
        let prepared = prepared();
        let artifacts = render_artifacts(&prepared).unwrap();
        assert!(std::panic::catch_unwind(|| {
            let _ = render_verification_receipt_with_limits(
                &prepared,
                &artifacts,
                artifacts.evidence.bytes.len(),
                MAX_RECEIPT_BYTES + 1,
                MAX_TOTAL_ARTIFACT_BYTES,
            );
        })
        .is_err());
        assert!(std::panic::catch_unwind(|| {
            let _ = render_verification_receipt_with_limits(
                &prepared,
                &artifacts,
                artifacts.evidence.bytes.len(),
                MAX_RECEIPT_BYTES,
                MAX_TOTAL_ARTIFACT_BYTES + 1,
            );
        })
        .is_err());
    }
}

fn verify_bindings(
    artifacts: &SemanticWorkspaceStructuralChangeArtifacts,
) -> Result<(), Vec<Diagnostic>> {
    for (artifact, domain) in [
        (&artifacts.preview, PREVIEW_DIGEST_DOMAIN),
        (&artifacts.context, CONTEXT_DIGEST_DOMAIN),
        (&artifacts.impact, IMPACT_DIGEST_DOMAIN),
        (&artifacts.review, REVIEW_DIGEST_DOMAIN),
        (&artifacts.evidence, EVIDENCE_DIGEST_DOMAIN),
    ] {
        if artifact.digest != digest(domain, artifact.bytes.as_bytes()) {
            return Err(replay());
        }
    }
    Ok(())
}
