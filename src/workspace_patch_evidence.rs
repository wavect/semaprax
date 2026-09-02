//! Deterministic Semantic Workspace Patch Evidence v1 generation and replay.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::{patch_evidence, review, workspace};

const EVIDENCE_SCHEMA: &str = "semaprax.semantic-workspace-patch-evidence.v1";
const VERIFICATION_SCHEMA: &str = "semaprax.semantic-workspace-patch-evidence-verification.v1";
const REVIEW_SCHEMA: &str = "semaprax.semantic-review.v1";
const PREVIEW_DIGEST_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-patch-evidence.preview-digest.v1\0";
const ARTIFACT_DIGEST_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-patch-evidence.artifact-digest.v1\0";
const FORMAT_LEAD: &str =
    "Semantic Workspace Patch Evidence must be one canonical JSON line with one terminal LF";

const MAX_CHANGED_FILES: usize = 16;
const MAX_CHILD_IMPACT_DEPTH: usize = 1024;
const MAX_CHILD_IMPACT_NODES: usize = 1024;
const MAX_TOTAL_IMPACT_NODES: usize = 16_384;
const MAX_TOTAL_IMPACT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_REVIEW_BYTES: usize = 32 * 1024 * 1024;
const MAX_CHILD_PATCH_EVIDENCE_BYTES: usize = 65_536;
const MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES: usize = 1024 * 1024;
const MAX_EVIDENCE_BYTES: usize = 65_536;
const MAX_RECEIPT_BYTES: usize = 65_536;
const MAX_JSON_NESTING_DEPTH: usize = 8;

const ASSESSMENT_KEYS: [&str; 7] = [
    "behavior",
    "api_identity",
    "security_authority",
    "memory_ownership",
    "target_artifact",
    "migration",
    "unsafe",
];
const ASSESSMENT_VALUES: [&str; 5] = [
    "change_proven",
    "unchanged_within_admitted_domain",
    "mixed",
    "unknown",
    "not_applicable",
];
const NONCLAIMS: [&str; 21] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_commit_authority",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_evidence_v2_aggregation",
    "no_agent_context_or_repository_analysis",
    "no_cross_file_module_type_call_capability_or_identity_resolution",
    "no_cross_file_impact_or_review_reasoning",
    "no_general_multi_file_repair",
    "no_create_delete_move_or_raw_tree_materialization",
    "no_atomic_visibility_for_raw_files_git_or_editors",
    "no_automatic_rollback_cleanup_or_gc",
    "no_network_distributed_nfs_or_overlay_guarantee",
    "no_power_loss_durability_guarantee",
    "no_acl_xattr_ads_preservation",
    "no_general_proof_system",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_patch_repair_graph_cleanup_backend_or_runtime_semantics",
];

#[derive(Clone, Copy)]
struct AggregateUsage {
    managed_files: usize,
    changed_files: usize,
    base_source_bytes: usize,
    candidate_source_bytes: usize,
    workspace_patch_bytes: usize,
    operations: usize,
    declarations: usize,
    callables: usize,
    call_sites: usize,
    manifest_bytes: usize,
    preview_bytes: usize,
    max_child_impact_depth: usize,
    max_child_impact_nodes: usize,
    total_impact_nodes: usize,
    total_impact_bytes: usize,
    total_review_bytes: usize,
    total_child_evidence_bytes: usize,
    retained_generations: usize,
    staging_attempts: usize,
}

struct WorkspaceChildEvidence {
    path: String,
    base_source_graph_schema: String,
    candidate_source_graph_schema: String,
    base_revision: String,
    candidate_revision: String,
    base_source_digest: String,
    candidate_source_digest: String,
    patch_schema: String,
    patch_digest: String,
    review_digest: String,
    assessments: [String; 7],
    supporting_kind: String,
    supporting_schema: String,
    supporting_digest: String,
    patch_evidence_digest: String,
}

struct WorkspaceEvidenceFacts {
    base_workspace_revision: String,
    candidate_workspace_revision: String,
    workspace_patch_digest: String,
    workspace_preview_digest: String,
    files: Vec<WorkspaceChildEvidence>,
    usage: AggregateUsage,
}

struct WorkspaceEvidenceBuild {
    read: workspace::WorkspaceReadBuild,
    facts: WorkspaceEvidenceFacts,
}

impl WorkspaceEvidenceBuild {
    fn recheck(self) -> Result<(), Vec<Diagnostic>> {
        self.read.recheck()
    }

    fn into_commit_authority(self) -> Result<workspace::WorkspaceCommitAuthority, Vec<Diagnostic>> {
        self.read.into_commit_authority()
    }

    fn release_with_error(self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        self.read.release_with_error(diagnostics)
    }
}

/// Generates one canonical read-only workspace evidence capsule.
pub fn generate(root: &Path, workspace_patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate_with_hook(root, workspace_patch_path, |_| {})
}

fn generate_with_hook(
    root: &Path,
    workspace_patch_path: &Path,
    mut hook: impl FnMut(EvidencePoint),
) -> Result<String, Vec<Diagnostic>> {
    let build = build_owned(root, workspace_patch_path)?;
    for child in &build.facts.files {
        crate::graph::reject_while_loop_evidence_schema(&child.base_source_graph_schema)
            .map_err(|error| vec![error])?;
        crate::graph::reject_while_loop_evidence_schema(&child.candidate_source_graph_schema)
            .map_err(|error| vec![error])?;
    }
    let capsule = match render_capsule_bounded(&build.facts) {
        Ok(capsule) => capsule,
        Err(diagnostics) => return Err(build.release_with_error(diagnostics)),
    };
    hook(EvidencePoint::BeforeFinalCheck);
    build.recheck()?;
    Ok(capsule)
}

/// Independently rebuilds and exactly replays one submitted workspace capsule.
pub fn verify(
    root: &Path,
    workspace_patch_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    verify_with_hook(root, workspace_patch_path, evidence_path, |_| {})
}

/// Applies one workspace transaction only after exact evidence replay.
pub fn apply(
    root: &Path,
    workspace_patch_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    apply_with_hook(root, workspace_patch_path, evidence_path, |_, _, _, _| {
        Ok(())
    })
}

fn apply_with_hook(
    root: &Path,
    workspace_patch_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(EvidenceApplyPoint, &Path, Option<&Path>, Option<&Path>) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let guard = workspace::acquire_evidence_apply_guard(root, workspace_patch_path)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    if let Err(error) = hook(EvidenceApplyPoint::AfterPatchRead, &active_path, None, None) {
        return Err(workspace::reject_evidence_apply_guard(
            guard,
            vec![Diagnostic::io(
                "SPX-I209",
                format!("workspace evidence patch post-read hook failed: {error}"),
            )],
        ));
    }
    let submitted = match read_evidence_bounded(evidence_path) {
        Ok(submitted) => submitted,
        Err(diagnostics) => {
            return Err(workspace::reject_evidence_apply_guard(guard, diagnostics));
        }
    };
    if let Err(error) = hook(
        EvidenceApplyPoint::AfterEvidenceRead,
        &active_path,
        None,
        None,
    ) {
        return Err(workspace::reject_evidence_apply_guard(
            guard,
            vec![Diagnostic::io(
                "SPX-I213",
                format!("workspace evidence post-read hook failed: {error}"),
            )],
        ));
    }
    let submitted_facts = match parse_canonical_capsule(&submitted) {
        Ok(facts) => facts,
        Err(diagnostics) => {
            return Err(workspace::reject_evidence_apply_guard(guard, diagnostics));
        }
    };
    let read = workspace::finish_evidence_apply_guard(guard)?;
    let build = build_from_read(read)?;
    let expected = match render_capsule_bounded(&build.facts) {
        Ok(expected) => expected,
        Err(diagnostics) => return Err(build.release_with_error(diagnostics)),
    };
    if submitted != expected || !same_bindings(&submitted_facts, &build.facts) {
        return Err(build.release_with_error(vec![mismatch()]));
    }
    if hook(EvidenceApplyPoint::AfterReplay, &active_path, None, None).is_err() {
        return Err(build.release_with_error(vec![invariant()]));
    }
    let authority = build.into_commit_authority()?;
    workspace::commit_workspace_authority_with_hook(
        authority,
        |point, active, staged, candidate| {
            hook(
                EvidenceApplyPoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceApplyPoint {
    AfterPatchRead,
    AfterEvidenceRead,
    AfterReplay,
    Workspace(workspace::ApplyPoint),
}

fn verify_with_hook(
    root: &Path,
    workspace_patch_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(EvidencePoint),
) -> Result<String, Vec<Diagnostic>> {
    let guard = workspace::acquire_evidence_guard(root)?;
    let submitted = match read_evidence_bounded(evidence_path) {
        Ok(submitted) => submitted,
        Err(diagnostics) => {
            return Err(workspace::reject_evidence_guard(guard, diagnostics));
        }
    };
    hook(EvidencePoint::AfterEvidenceRead);
    let submitted_facts = match parse_canonical_capsule(&submitted) {
        Ok(facts) => facts,
        Err(diagnostics) => {
            return Err(workspace::reject_evidence_guard(guard, diagnostics));
        }
    };
    let read = workspace::build_read_owned_from_guard(guard, workspace_patch_path)?;
    let build = build_from_read(read)?;
    let expected = match render_capsule_bounded(&build.facts) {
        Ok(expected) => expected,
        Err(diagnostics) => return Err(build.release_with_error(diagnostics)),
    };
    if submitted != expected || !same_bindings(&submitted_facts, &build.facts) {
        return Err(build.release_with_error(vec![mismatch()]));
    }
    let artifact_digest = domain_digest(ARTIFACT_DIGEST_DOMAIN, submitted.as_bytes());
    let receipt = match render_receipt_bounded(&build.facts, &artifact_digest, submitted.len()) {
        Ok(receipt) => receipt,
        Err(diagnostics) => return Err(build.release_with_error(diagnostics)),
    };
    hook(EvidencePoint::BeforeFinalCheck);
    build.recheck()?;
    Ok(receipt)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EvidencePoint {
    AfterEvidenceRead,
    BeforeFinalCheck,
}

fn build_owned(
    root: &Path,
    workspace_patch_path: &Path,
) -> Result<WorkspaceEvidenceBuild, Vec<Diagnostic>> {
    let guard = workspace::acquire_evidence_guard(root)?;
    let read = workspace::build_read_owned_from_guard(guard, workspace_patch_path)?;
    build_from_read(read)
}

fn build_from_read(
    mut read: workspace::WorkspaceReadBuild,
) -> Result<WorkspaceEvidenceBuild, Vec<Diagnostic>> {
    let facts = (|| -> Result<WorkspaceEvidenceFacts, Vec<Diagnostic>> {
        if read.plan().changed_files() < 2 {
            return Err(vec![format_error(
                "workspace evidence file cardinality is outside the closed schema",
            )]);
        }
        let preview = read.preview_json()?;
        let preview_digest = domain_digest(PREVIEW_DIGEST_DOMAIN, preview.as_bytes());
        let preflights = read.take_preflights()?;
        let mut files = Vec::with_capacity(preflights.len());
        let mut remaining_impact_nodes = MAX_TOTAL_IMPACT_NODES;
        let mut remaining_impact_bytes = MAX_TOTAL_IMPACT_BYTES;
        let mut remaining_review_bytes = MAX_TOTAL_REVIEW_BYTES;
        let mut remaining_child_evidence_bytes = MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES;
        let mut used_max_impact_depth = 0usize;
        let mut used_max_impact_nodes = 0usize;

        for child in preflights {
            let (path, preflight) = child.into_parts();
            let binding = read.evidence_binding(&path)?;
            if binding.path() != path {
                return Err(vec![invariant()]);
            }
            let child_impact_node_limit = MAX_CHILD_IMPACT_NODES.min(remaining_impact_nodes);
            let total_node_budget_is_tighter = remaining_impact_nodes < MAX_CHILD_IMPACT_NODES;
            let review = review::build_from_preflight_for_workspace(
                preflight,
                review::WorkspaceEvidenceLimits {
                    max_impact_nodes: child_impact_node_limit,
                    max_impact_bytes: MAX_TOTAL_IMPACT_BYTES.min(remaining_impact_bytes),
                    max_review_bytes: MAX_TOTAL_REVIEW_BYTES.min(remaining_review_bytes),
                },
            )
            .map_err(|diagnostics| {
                map_review_child_diagnostics(diagnostics, total_node_budget_is_tighter)
            })?;
            let candidate_source_digest =
                review::source_digest(review.preflight().canonical_candidate().as_bytes());
            let facts =
                patch_evidence::facts_from_review(&review).map_err(map_facts_diagnostics)?;
            validate_child_binding(&path, &binding, &facts, &candidate_source_digest)?;
            let usage = facts.usage();
            let review_bytes = usage.review_bytes();
            let impact_nodes = usage.impact_nodes();
            let impact_bytes = usage.impact_bytes();
            let child_limit = MAX_CHILD_PATCH_EVIDENCE_BYTES.min(remaining_child_evidence_bytes);
            let rendered = patch_evidence::render_from_facts_with_limit(&facts, child_limit)
                .map_err(|diagnostics| {
                    map_child_render_diagnostics(
                        diagnostics,
                        remaining_child_evidence_bytes < MAX_CHILD_PATCH_EVIDENCE_BYTES,
                    )
                })?;
            let (artifact, artifact_digest) = rendered.into_parts();

            remaining_impact_nodes = remaining_impact_nodes
                .checked_sub(impact_nodes)
                .ok_or_else(|| {
                    vec![limit_field(
                        "max_total_impact_nodes",
                        MAX_TOTAL_IMPACT_NODES,
                    )]
                })?;
            remaining_impact_bytes = remaining_impact_bytes
                .checked_sub(impact_bytes)
                .ok_or_else(|| {
                    vec![limit_field(
                        "max_total_impact_bytes",
                        MAX_TOTAL_IMPACT_BYTES,
                    )]
                })?;
            remaining_review_bytes = remaining_review_bytes
                .checked_sub(review_bytes)
                .ok_or_else(|| {
                    vec![limit_field(
                        "max_total_review_bytes",
                        MAX_TOTAL_REVIEW_BYTES,
                    )]
                })?;
            remaining_child_evidence_bytes = remaining_child_evidence_bytes
                .checked_sub(artifact.len())
                .ok_or_else(|| {
                    vec![limit_field(
                        "max_total_child_patch_evidence_bytes",
                        MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES,
                    )]
                })?;
            used_max_impact_depth = used_max_impact_depth.max(usage.impact_depth());
            used_max_impact_nodes = used_max_impact_nodes.max(impact_nodes);
            files.push(WorkspaceChildEvidence {
                path,
                base_source_graph_schema: binding.base_source_graph_schema().to_owned(),
                candidate_source_graph_schema: binding.candidate_source_graph_schema().to_owned(),
                base_revision: facts.base_revision().to_owned(),
                candidate_revision: facts.candidate_revision().to_owned(),
                base_source_digest: facts.source_digest().to_owned(),
                candidate_source_digest,
                patch_schema: facts.patch_schema().to_owned(),
                patch_digest: facts.patch_digest().to_owned(),
                review_digest: facts.review_digest().to_owned(),
                assessments: facts.assessments().clone(),
                supporting_kind: facts.supporting_kind().to_owned(),
                supporting_schema: facts.supporting_schema().to_owned(),
                supporting_digest: facts.supporting_digest().to_owned(),
                patch_evidence_digest: artifact_digest,
            });
            drop(artifact);
            drop(review);
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        if files.len() != read.plan().changed_files()
            || files.windows(2).any(|pair| pair[0].path >= pair[1].path)
        {
            return Err(vec![invariant()]);
        }
        let (declarations, callables, call_sites) = read.plan().semantic_usage();
        let facts = WorkspaceEvidenceFacts {
            base_workspace_revision: read.plan().base_workspace_revision().to_owned(),
            candidate_workspace_revision: read.plan().candidate_workspace_revision().to_owned(),
            workspace_patch_digest: read.plan().workspace_patch_digest().to_owned(),
            workspace_preview_digest: preview_digest,
            files,
            usage: AggregateUsage {
                managed_files: read.plan().managed_files(),
                changed_files: read.plan().changed_files(),
                base_source_bytes: read.base_source_bytes(),
                candidate_source_bytes: read.plan().candidate_source_bytes(),
                workspace_patch_bytes: read.plan().workspace_patch_bytes(),
                operations: read.plan().operations(),
                declarations,
                callables,
                call_sites,
                manifest_bytes: read.plan().candidate_manifest_bytes(),
                preview_bytes: preview.len(),
                max_child_impact_depth: used_max_impact_depth,
                max_child_impact_nodes: used_max_impact_nodes,
                total_impact_nodes: MAX_TOTAL_IMPACT_NODES - remaining_impact_nodes,
                total_impact_bytes: MAX_TOTAL_IMPACT_BYTES - remaining_impact_bytes,
                total_review_bytes: MAX_TOTAL_REVIEW_BYTES - remaining_review_bytes,
                total_child_evidence_bytes: MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES
                    - remaining_child_evidence_bytes,
                retained_generations: read.retained_generations(),
                staging_attempts: read.staging_attempts(),
            },
        };
        validate_usage(&facts.usage)?;
        Ok(facts)
    })();
    match facts {
        Ok(facts) => Ok(WorkspaceEvidenceBuild { read, facts }),
        Err(diagnostics) => Err(read.release_with_error(diagnostics)),
    }
}

fn validate_child_binding(
    path: &str,
    binding: &workspace::WorkspaceEvidenceBinding,
    facts: &patch_evidence::PatchEvidenceFacts,
    candidate_source_digest: &str,
) -> Result<(), Vec<Diagnostic>> {
    if binding.path() != path
        || binding.base_source_graph_schema() != facts.source_graph_schema()
        || binding.candidate_source_graph_schema() != facts.source_graph_schema()
        || binding.base_revision() != facts.base_revision()
        || binding.candidate_revision() != facts.candidate_revision()
        || binding.base_source_digest() != facts.source_digest()
        || binding.candidate_source_digest() != candidate_source_digest
        || binding.patch_schema() != facts.patch_schema()
        || binding.patch_digest() != facts.patch_digest()
    {
        let _ = path;
        return Err(vec![invariant()]);
    }
    Ok(())
}

fn render_capsule_bounded(facts: &WorkspaceEvidenceFacts) -> Result<String, Vec<Diagnostic>> {
    render_capsule_with_limit(facts, MAX_EVIDENCE_BYTES)
}

fn render_capsule_with_limit(
    facts: &WorkspaceEvidenceFacts,
    limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    if limit == 0 {
        return Err(vec![limit_field(
            "max_workspace_evidence_bytes",
            MAX_EVIDENCE_BYTES,
        )]);
    }
    let mut used = 0usize;
    for _ in 0..4 {
        let (mut output, overflowed) = crate::bounded_output::with_limit(limit - 1, || {
            render_document(EVIDENCE_SCHEMA, None, facts, used, 0, 0)
        });
        output.push('\n');
        if overflowed || output.len() > limit {
            return Err(vec![limit_field(
                "max_workspace_evidence_bytes",
                MAX_EVIDENCE_BYTES,
            )]);
        }
        if output.len() == used {
            return Ok(output);
        }
        used = output.len();
    }
    Err(vec![invariant()])
}

fn render_receipt_bounded(
    facts: &WorkspaceEvidenceFacts,
    artifact_digest: &str,
    used_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    render_receipt_with_limit(
        facts,
        artifact_digest,
        used_evidence_bytes,
        MAX_RECEIPT_BYTES,
    )
}

fn render_receipt_with_limit(
    facts: &WorkspaceEvidenceFacts,
    artifact_digest: &str,
    used_evidence_bytes: usize,
    limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    if limit == 0 {
        return Err(vec![limit_field(
            "max_workspace_receipt_bytes",
            MAX_RECEIPT_BYTES,
        )]);
    }
    let mut used = 0usize;
    for _ in 0..4 {
        let (mut output, overflowed) = crate::bounded_output::with_limit(limit - 1, || {
            render_document(
                VERIFICATION_SCHEMA,
                Some(artifact_digest),
                facts,
                used_evidence_bytes,
                used,
                used_evidence_bytes,
            )
        });
        output.push('\n');
        if overflowed || output.len() > limit {
            return Err(vec![limit_field(
                "max_workspace_receipt_bytes",
                MAX_RECEIPT_BYTES,
            )]);
        }
        if output.len() == used {
            return Ok(output);
        }
        used = output.len();
    }
    Err(vec![invariant()])
}

fn render_document(
    schema: &str,
    artifact_digest: Option<&str>,
    facts: &WorkspaceEvidenceFacts,
    used_workspace_evidence_bytes: usize,
    used_receipt_bytes: usize,
    submitted_evidence_bytes: usize,
) -> String {
    let receipt = artifact_digest.is_some();
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json(&mut output, schema);
    if receipt {
        output.push_str(",\"result\":\"exact_replay\"");
    }
    output.push_str(",\"workspace_manifest_schema\":");
    push_json(&mut output, workspace::MANIFEST_SCHEMA);
    output.push_str(",\"base_workspace_revision\":");
    push_json(&mut output, &facts.base_workspace_revision);
    output.push_str(",\"candidate_workspace_revision\":");
    push_json(&mut output, &facts.candidate_workspace_revision);
    output.push_str(",\"workspace_patch\":{\"schema\":");
    push_json(&mut output, workspace::PATCH_SCHEMA);
    output.push_str(",\"digest\":");
    push_json(&mut output, &facts.workspace_patch_digest);
    output.push_str("},\"workspace_preview\":{\"schema\":");
    push_json(&mut output, workspace::PREVIEW_SCHEMA);
    output.push_str(",\"digest\":");
    push_json(&mut output, &facts.workspace_preview_digest);
    output.push_str("}");
    if let Some(digest) = artifact_digest {
        output.push_str(",\"workspace_patch_evidence\":{\"schema\":");
        push_json(&mut output, EVIDENCE_SCHEMA);
        output.push_str(",\"digest\":");
        push_json(&mut output, digest);
        output.push('}');
    }
    output.push_str(",\"files\":[");
    let mut files = facts.files.iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    for (index, file) in files.into_iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        render_file(&mut output, file);
    }
    output.push_str("],\"limits\":");
    render_limits(&mut output);
    output.push_str(",\"budget\":");
    render_budget(
        &mut output,
        facts.usage,
        receipt,
        if receipt {
            submitted_evidence_bytes
        } else {
            used_workspace_evidence_bytes
        },
        used_receipt_bytes,
    );
    output.push_str(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json(&mut output, nonclaim);
    }
    output.push_str("]}");
    output.into_string()
}

fn render_file(output: &mut crate::bounded_output::CappedString, file: &WorkspaceChildEvidence) {
    output.push_str("{\"path\":");
    push_json(output, &file.path);
    output.push_str(",\"base_source_graph_schema\":");
    push_json(output, &file.base_source_graph_schema);
    output.push_str(",\"candidate_source_graph_schema\":");
    push_json(output, &file.candidate_source_graph_schema);
    output.push_str(",\"base_revision\":");
    push_json(output, &file.base_revision);
    output.push_str(",\"candidate_revision\":");
    push_json(output, &file.candidate_revision);
    output.push_str(",\"base_source\":{\"digest\":");
    push_json(output, &file.base_source_digest);
    output.push_str("},\"candidate_source\":{\"digest\":");
    push_json(output, &file.candidate_source_digest);
    output.push_str("},\"patch\":{\"schema\":");
    push_json(output, &file.patch_schema);
    output.push_str(",\"digest\":");
    push_json(output, &file.patch_digest);
    output.push_str("},\"review\":{\"schema\":");
    push_json(output, REVIEW_SCHEMA);
    output.push_str(",\"digest\":");
    push_json(output, &file.review_digest);
    output.push_str("},\"assessments\":{");
    for (index, (key, value)) in ASSESSMENT_KEYS.iter().zip(&file.assessments).enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json(output, key);
        output.push(':');
        push_json(output, value);
    }
    output.push_str("},\"supporting_evidence\":{\"id\":\"evidence:0\",\"kind\":");
    push_json(output, &file.supporting_kind);
    output.push_str(",\"schema\":");
    push_json(output, &file.supporting_schema);
    output.push_str(",\"digest\":");
    push_json(output, &file.supporting_digest);
    output.push_str("},\"patch_evidence\":{\"schema\":");
    push_json(output, patch_evidence::EVIDENCE_SCHEMA);
    output.push_str(",\"digest\":");
    push_json(output, &file.patch_evidence_digest);
    output.push_str("}}");
}

fn render_limits(output: &mut crate::bounded_output::CappedString) {
    let _ = write!(
        output,
        "{{\"max_managed_files\":{},\"max_changed_files\":{MAX_CHANGED_FILES},\"max_total_base_source_bytes\":{},\"max_total_candidate_source_bytes\":{},\"max_workspace_patch_bytes\":{},\"max_operations\":{},\"max_declarations\":{},\"max_callables\":{},\"max_call_sites\":{},\"max_manifest_bytes\":{},\"max_workspace_preview_bytes\":{},\"max_child_impact_depth\":{MAX_CHILD_IMPACT_DEPTH},\"max_child_impact_nodes\":{MAX_CHILD_IMPACT_NODES},\"max_total_impact_nodes\":{MAX_TOTAL_IMPACT_NODES},\"max_total_impact_bytes\":{MAX_TOTAL_IMPACT_BYTES},\"max_total_review_bytes\":{MAX_TOTAL_REVIEW_BYTES},\"max_child_patch_evidence_bytes\":{MAX_CHILD_PATCH_EVIDENCE_BYTES},\"max_total_child_patch_evidence_bytes\":{MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES},\"max_workspace_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_workspace_receipt_bytes\":{MAX_RECEIPT_BYTES},\"max_json_depth\":{MAX_JSON_NESTING_DEPTH},\"max_retained_generations\":{},\"max_staging_attempts\":{},\"max_unexpected_inventory_entries\":0}}",
        workspace::MAX_MANAGED_FILES,
        workspace::MAX_TOTAL_SOURCE_BYTES,
        workspace::MAX_TOTAL_SOURCE_BYTES,
        workspace::MAX_WORKSPACE_PATCH_BYTES,
        workspace::MAX_OPERATIONS,
        workspace::MAX_DECLARATIONS,
        workspace::MAX_CALLABLES,
        workspace::MAX_CALL_SITES,
        workspace::MAX_MANIFEST_BYTES,
        workspace::MAX_PREVIEW_BYTES,
        workspace::MAX_RETAINED_GENERATIONS,
        workspace::MAX_STAGING_ATTEMPTS,
    );
}

fn render_budget(
    output: &mut crate::bounded_output::CappedString,
    usage: AggregateUsage,
    receipt: bool,
    used_evidence_bytes: usize,
    used_receipt_bytes: usize,
) {
    let _ = write!(
        output,
        "{{\"used_managed_files\":{},\"used_changed_files\":{},\"used_total_base_source_bytes\":{},\"used_total_candidate_source_bytes\":{},\"used_workspace_patch_bytes\":{}",
        usage.managed_files,
        usage.changed_files,
        usage.base_source_bytes,
        usage.candidate_source_bytes,
        usage.workspace_patch_bytes,
    );
    if receipt {
        let _ = write!(
            output,
            ",\"used_workspace_evidence_bytes\":{used_evidence_bytes}"
        );
    }
    let _ = write!(
        output,
        ",\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_manifest_bytes\":{},\"used_workspace_preview_bytes\":{},\"used_max_child_impact_depth\":{},\"used_max_child_impact_nodes\":{},\"used_total_impact_nodes\":{},\"used_total_impact_bytes\":{},\"used_total_review_bytes\":{},\"used_total_child_patch_evidence_bytes\":{}",
        usage.operations,
        usage.declarations,
        usage.callables,
        usage.call_sites,
        usage.manifest_bytes,
        usage.preview_bytes,
        usage.max_child_impact_depth,
        usage.max_child_impact_nodes,
        usage.total_impact_nodes,
        usage.total_impact_bytes,
        usage.total_review_bytes,
        usage.total_child_evidence_bytes,
    );
    if receipt {
        let _ = write!(
            output,
            ",\"used_workspace_receipt_bytes\":{used_receipt_bytes}"
        );
    } else {
        let _ = write!(
            output,
            ",\"used_workspace_evidence_bytes\":{used_evidence_bytes}"
        );
    }
    let _ = write!(
        output,
        ",\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":0}}",
        usage.retained_generations, usage.staging_attempts,
    );
}

fn parse_canonical_capsule(source: &str) -> Result<WorkspaceEvidenceFacts, Vec<Diagnostic>> {
    if source.as_bytes().first() == Some(&0xef)
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len() - 1].contains('\n')
    {
        return Err(vec![format_error(FORMAT_LEAD)]);
    }
    let body = &source[..source.len() - 1];
    validate_json_structure(body)?;
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        vec![format_error(
            "workspace patch evidence is not canonical UTF-8 JSON",
        )]
    })?;
    let top = exact_object(
        &value,
        &[
            "schema",
            "workspace_manifest_schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "workspace_patch",
            "workspace_preview",
            "files",
            "limits",
            "budget",
            "nonclaims",
        ],
        "capsule",
    )?;
    require_text(top, "schema", EVIDENCE_SCHEMA)?;
    require_text(top, "workspace_manifest_schema", workspace::MANIFEST_SCHEMA)?;
    let base_workspace_revision = digest_text(top, "base_workspace_revision")?;
    let candidate_workspace_revision = digest_text(top, "candidate_workspace_revision")?;
    let workspace_patch = exact_object(
        &top["workspace_patch"],
        &["schema", "digest"],
        "workspace_patch",
    )?;
    require_text(workspace_patch, "schema", workspace::PATCH_SCHEMA)?;
    let workspace_patch_digest = digest_text(workspace_patch, "digest")?;
    let workspace_preview = exact_object(
        &top["workspace_preview"],
        &["schema", "digest"],
        "workspace_preview",
    )?;
    require_text(workspace_preview, "schema", workspace::PREVIEW_SCHEMA)?;
    let workspace_preview_digest = digest_text(workspace_preview, "digest")?;
    let file_values = top["files"]
        .as_array()
        .ok_or_else(|| vec![format_error("workspace evidence files must be an array")])?;
    if file_values.len() < 2 {
        return Err(vec![format_error(
            "workspace evidence file cardinality is outside the closed schema",
        )]);
    }
    if file_values.len() > MAX_CHANGED_FILES {
        return Err(vec![limit_field("max_changed_files", MAX_CHANGED_FILES)]);
    }
    let mut files = Vec::with_capacity(file_values.len());
    let mut prior = None::<String>;
    for value in file_values {
        let file = parse_file(value)?;
        if prior.as_ref().is_some_and(|path| path >= &file.path) {
            return Err(vec![format_error(
                "workspace evidence file paths are not canonical and unique",
            )]);
        }
        prior = Some(file.path.clone());
        files.push(file);
    }
    validate_limits(&top["limits"])?;
    let budget = exact_object(
        &top["budget"],
        &[
            "used_managed_files",
            "used_changed_files",
            "used_total_base_source_bytes",
            "used_total_candidate_source_bytes",
            "used_workspace_patch_bytes",
            "used_operations",
            "used_declarations",
            "used_callables",
            "used_call_sites",
            "used_manifest_bytes",
            "used_workspace_preview_bytes",
            "used_max_child_impact_depth",
            "used_max_child_impact_nodes",
            "used_total_impact_nodes",
            "used_total_impact_bytes",
            "used_total_review_bytes",
            "used_total_child_patch_evidence_bytes",
            "used_workspace_evidence_bytes",
            "used_retained_generations",
            "used_staging_attempts",
            "used_unexpected_inventory_entries",
        ],
        "budget",
    )?;
    if number(budget, "used_workspace_evidence_bytes")? != source.len()
        || number(budget, "used_unexpected_inventory_entries")? != 0
    {
        return Err(vec![format_error(
            "workspace evidence byte or inventory accounting is noncanonical",
        )]);
    }
    let usage = parse_usage(budget)?;
    if usage.changed_files != files.len() {
        return Err(vec![format_error(
            "used_changed_files differs from the canonical file array",
        )]);
    }
    validate_usage(&usage)?;
    validate_nonclaims(&top["nonclaims"])?;
    let facts = WorkspaceEvidenceFacts {
        base_workspace_revision,
        candidate_workspace_revision,
        workspace_patch_digest,
        workspace_preview_digest,
        files,
        usage,
    };
    if render_document(EVIDENCE_SCHEMA, None, &facts, source.len(), 0, 0) != body {
        return Err(vec![format_error(
            "workspace evidence key order or JSON spelling is noncanonical",
        )]);
    }
    Ok(facts)
}

fn parse_file(value: &serde_json::Value) -> Result<WorkspaceChildEvidence, Vec<Diagnostic>> {
    let object = exact_object(
        value,
        &[
            "path",
            "base_source_graph_schema",
            "candidate_source_graph_schema",
            "base_revision",
            "candidate_revision",
            "base_source",
            "candidate_source",
            "patch",
            "review",
            "assessments",
            "supporting_evidence",
            "patch_evidence",
        ],
        "file",
    )?;
    let path = text(object, "path")?;
    if !workspace::evidence_path_is_valid(&path) {
        return Err(vec![format_error(
            "workspace evidence contains a noncanonical logical path",
        )]);
    }
    let base_source_graph_schema = graph_schema_text(object, "base_source_graph_schema")?;
    let candidate_source_graph_schema = graph_schema_text(object, "candidate_source_graph_schema")?;
    let base_revision = digest_text(object, "base_revision")?;
    let candidate_revision = digest_text(object, "candidate_revision")?;
    let base_source = exact_object(&object["base_source"], &["digest"], "base_source")?;
    let candidate_source =
        exact_object(&object["candidate_source"], &["digest"], "candidate_source")?;
    let patch = exact_object(&object["patch"], &["schema", "digest"], "patch")?;
    let patch_schema = text(patch, "schema")?;
    if !matches!(
        patch_schema.as_str(),
        "semaprax.semantic-patch.v1" | "semaprax.semantic-patch.v2" | "semaprax.semantic-patch.v3"
    ) {
        return Err(vec![format_error(
            "workspace evidence carries an unsupported child Patch schema",
        )]);
    }
    let review = exact_object(&object["review"], &["schema", "digest"], "review")?;
    require_text(review, "schema", REVIEW_SCHEMA)?;
    let assessments_object = exact_object(&object["assessments"], &ASSESSMENT_KEYS, "assessments")?;
    let assessments = ASSESSMENT_KEYS
        .iter()
        .map(|key| {
            let value = text(assessments_object, key)?;
            if !ASSESSMENT_VALUES.contains(&value.as_str()) {
                return Err(vec![format_error(
                    "workspace evidence carries an unknown assessment",
                )]);
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?
        .try_into()
        .map_err(|_| vec![invariant()])?;
    let supporting = exact_object(
        &object["supporting_evidence"],
        &["id", "kind", "schema", "digest"],
        "supporting_evidence",
    )?;
    require_text(supporting, "id", "evidence:0")?;
    let supporting_kind = text(supporting, "kind")?;
    let supporting_schema = text(supporting, "schema")?;
    if !matches!(
        (supporting_kind.as_str(), supporting_schema.as_str()),
        ("semantic_impact_v1", "semaprax.semantic-impact.v1")
            | ("identity_rebase_v1", "semaprax.identity-rebase.v1")
    ) {
        return Err(vec![format_error(
            "workspace evidence supporting kind and schema disagree",
        )]);
    }
    let supporting_matches_patch = match patch_schema.as_str() {
        "semaprax.semantic-patch.v1" | "semaprax.semantic-patch.v2" => {
            supporting_kind == "semantic_impact_v1"
                && supporting_schema == "semaprax.semantic-impact.v1"
        }
        "semaprax.semantic-patch.v3" => {
            supporting_kind == "identity_rebase_v1"
                && supporting_schema == "semaprax.identity-rebase.v1"
        }
        _ => false,
    };
    if !supporting_matches_patch {
        return Err(vec![format_error(
            "child Patch schema and supporting evidence are not correlated",
        )]);
    }
    let child = exact_object(
        &object["patch_evidence"],
        &["schema", "digest"],
        "patch_evidence",
    )?;
    require_text(child, "schema", patch_evidence::EVIDENCE_SCHEMA)?;
    Ok(WorkspaceChildEvidence {
        path,
        base_source_graph_schema,
        candidate_source_graph_schema,
        base_revision,
        candidate_revision,
        base_source_digest: digest_text(base_source, "digest")?,
        candidate_source_digest: digest_text(candidate_source, "digest")?,
        patch_schema,
        patch_digest: digest_text(patch, "digest")?,
        review_digest: digest_text(review, "digest")?,
        assessments,
        supporting_kind,
        supporting_schema,
        supporting_digest: digest_text(supporting, "digest")?,
        patch_evidence_digest: digest_text(child, "digest")?,
    })
}

fn parse_usage(
    budget: &serde_json::Map<String, serde_json::Value>,
) -> Result<AggregateUsage, Vec<Diagnostic>> {
    Ok(AggregateUsage {
        managed_files: number(budget, "used_managed_files")?,
        changed_files: number(budget, "used_changed_files")?,
        base_source_bytes: number(budget, "used_total_base_source_bytes")?,
        candidate_source_bytes: number(budget, "used_total_candidate_source_bytes")?,
        workspace_patch_bytes: number(budget, "used_workspace_patch_bytes")?,
        operations: number(budget, "used_operations")?,
        declarations: number(budget, "used_declarations")?,
        callables: number(budget, "used_callables")?,
        call_sites: number(budget, "used_call_sites")?,
        manifest_bytes: number(budget, "used_manifest_bytes")?,
        preview_bytes: number(budget, "used_workspace_preview_bytes")?,
        max_child_impact_depth: number(budget, "used_max_child_impact_depth")?,
        max_child_impact_nodes: number(budget, "used_max_child_impact_nodes")?,
        total_impact_nodes: number(budget, "used_total_impact_nodes")?,
        total_impact_bytes: number(budget, "used_total_impact_bytes")?,
        total_review_bytes: number(budget, "used_total_review_bytes")?,
        total_child_evidence_bytes: number(budget, "used_total_child_patch_evidence_bytes")?,
        retained_generations: number(budget, "used_retained_generations")?,
        staging_attempts: number(budget, "used_staging_attempts")?,
    })
}

fn validate_usage(usage: &AggregateUsage) -> Result<(), Vec<Diagnostic>> {
    if usage.changed_files < 2 {
        return Err(vec![format_error(
            "workspace evidence file cardinality is outside the closed schema",
        )]);
    }
    if usage.managed_files > workspace::MAX_MANAGED_FILES
        || usage.changed_files > MAX_CHANGED_FILES
        || usage.base_source_bytes > workspace::MAX_TOTAL_SOURCE_BYTES
        || usage.candidate_source_bytes > workspace::MAX_TOTAL_SOURCE_BYTES
        || usage.workspace_patch_bytes > workspace::MAX_WORKSPACE_PATCH_BYTES
        || usage.operations > workspace::MAX_OPERATIONS
        || usage.declarations > workspace::MAX_DECLARATIONS
        || usage.callables > workspace::MAX_CALLABLES
        || usage.call_sites > workspace::MAX_CALL_SITES
        || usage.manifest_bytes > workspace::MAX_MANIFEST_BYTES
        || usage.preview_bytes > workspace::MAX_PREVIEW_BYTES
        || usage.max_child_impact_depth > MAX_CHILD_IMPACT_DEPTH
        || usage.max_child_impact_nodes > MAX_CHILD_IMPACT_NODES
        || usage.total_impact_nodes > MAX_TOTAL_IMPACT_NODES
        || usage.total_impact_bytes > MAX_TOTAL_IMPACT_BYTES
        || usage.total_review_bytes > MAX_TOTAL_REVIEW_BYTES
        || usage.total_child_evidence_bytes > MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES
        || usage.retained_generations > workspace::MAX_RETAINED_GENERATIONS
        || usage.staging_attempts > workspace::MAX_STAGING_ATTEMPTS
    {
        return Err(vec![usage_limit_error(usage)]);
    }
    Ok(())
}

fn usage_limit_error(usage: &AggregateUsage) -> Diagnostic {
    let (field, maximum) = if usage.managed_files > workspace::MAX_MANAGED_FILES {
        ("max_managed_files", workspace::MAX_MANAGED_FILES)
    } else if usage.changed_files > MAX_CHANGED_FILES {
        ("max_changed_files", MAX_CHANGED_FILES)
    } else if usage.base_source_bytes > workspace::MAX_TOTAL_SOURCE_BYTES {
        (
            "max_total_base_source_bytes",
            workspace::MAX_TOTAL_SOURCE_BYTES,
        )
    } else if usage.candidate_source_bytes > workspace::MAX_TOTAL_SOURCE_BYTES {
        (
            "max_total_candidate_source_bytes",
            workspace::MAX_TOTAL_SOURCE_BYTES,
        )
    } else if usage.workspace_patch_bytes > workspace::MAX_WORKSPACE_PATCH_BYTES {
        (
            "max_workspace_patch_bytes",
            workspace::MAX_WORKSPACE_PATCH_BYTES,
        )
    } else if usage.operations > workspace::MAX_OPERATIONS {
        ("max_operations", workspace::MAX_OPERATIONS)
    } else if usage.declarations > workspace::MAX_DECLARATIONS {
        ("max_declarations", workspace::MAX_DECLARATIONS)
    } else if usage.callables > workspace::MAX_CALLABLES {
        ("max_callables", workspace::MAX_CALLABLES)
    } else if usage.call_sites > workspace::MAX_CALL_SITES {
        ("max_call_sites", workspace::MAX_CALL_SITES)
    } else if usage.manifest_bytes > workspace::MAX_MANIFEST_BYTES {
        ("max_manifest_bytes", workspace::MAX_MANIFEST_BYTES)
    } else if usage.preview_bytes > workspace::MAX_PREVIEW_BYTES {
        ("max_workspace_preview_bytes", workspace::MAX_PREVIEW_BYTES)
    } else if usage.max_child_impact_depth > MAX_CHILD_IMPACT_DEPTH {
        ("max_child_impact_depth", MAX_CHILD_IMPACT_DEPTH)
    } else if usage.max_child_impact_nodes > MAX_CHILD_IMPACT_NODES {
        ("max_child_impact_nodes", MAX_CHILD_IMPACT_NODES)
    } else if usage.total_impact_nodes > MAX_TOTAL_IMPACT_NODES {
        ("max_total_impact_nodes", MAX_TOTAL_IMPACT_NODES)
    } else if usage.total_impact_bytes > MAX_TOTAL_IMPACT_BYTES {
        ("max_total_impact_bytes", MAX_TOTAL_IMPACT_BYTES)
    } else if usage.total_review_bytes > MAX_TOTAL_REVIEW_BYTES {
        ("max_total_review_bytes", MAX_TOTAL_REVIEW_BYTES)
    } else if usage.total_child_evidence_bytes > MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES {
        (
            "max_total_child_patch_evidence_bytes",
            MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES,
        )
    } else if usage.retained_generations > workspace::MAX_RETAINED_GENERATIONS {
        (
            "max_retained_generations",
            workspace::MAX_RETAINED_GENERATIONS,
        )
    } else {
        ("max_staging_attempts", workspace::MAX_STAGING_ATTEMPTS)
    };
    limit_field(field, maximum)
}

fn validate_limits(value: &serde_json::Value) -> Result<(), Vec<Diagnostic>> {
    let object = exact_object(
        value,
        &[
            "max_managed_files",
            "max_changed_files",
            "max_total_base_source_bytes",
            "max_total_candidate_source_bytes",
            "max_workspace_patch_bytes",
            "max_operations",
            "max_declarations",
            "max_callables",
            "max_call_sites",
            "max_manifest_bytes",
            "max_workspace_preview_bytes",
            "max_child_impact_depth",
            "max_child_impact_nodes",
            "max_total_impact_nodes",
            "max_total_impact_bytes",
            "max_total_review_bytes",
            "max_child_patch_evidence_bytes",
            "max_total_child_patch_evidence_bytes",
            "max_workspace_evidence_bytes",
            "max_workspace_receipt_bytes",
            "max_json_depth",
            "max_retained_generations",
            "max_staging_attempts",
            "max_unexpected_inventory_entries",
        ],
        "limits",
    )?;
    let expected = [
        ("max_managed_files", workspace::MAX_MANAGED_FILES),
        ("max_changed_files", MAX_CHANGED_FILES),
        (
            "max_total_base_source_bytes",
            workspace::MAX_TOTAL_SOURCE_BYTES,
        ),
        (
            "max_total_candidate_source_bytes",
            workspace::MAX_TOTAL_SOURCE_BYTES,
        ),
        (
            "max_workspace_patch_bytes",
            workspace::MAX_WORKSPACE_PATCH_BYTES,
        ),
        ("max_operations", workspace::MAX_OPERATIONS),
        ("max_declarations", workspace::MAX_DECLARATIONS),
        ("max_callables", workspace::MAX_CALLABLES),
        ("max_call_sites", workspace::MAX_CALL_SITES),
        ("max_manifest_bytes", workspace::MAX_MANIFEST_BYTES),
        ("max_workspace_preview_bytes", workspace::MAX_PREVIEW_BYTES),
        ("max_child_impact_depth", MAX_CHILD_IMPACT_DEPTH),
        ("max_child_impact_nodes", MAX_CHILD_IMPACT_NODES),
        ("max_total_impact_nodes", MAX_TOTAL_IMPACT_NODES),
        ("max_total_impact_bytes", MAX_TOTAL_IMPACT_BYTES),
        ("max_total_review_bytes", MAX_TOTAL_REVIEW_BYTES),
        (
            "max_child_patch_evidence_bytes",
            MAX_CHILD_PATCH_EVIDENCE_BYTES,
        ),
        (
            "max_total_child_patch_evidence_bytes",
            MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES,
        ),
        ("max_workspace_evidence_bytes", MAX_EVIDENCE_BYTES),
        ("max_workspace_receipt_bytes", MAX_RECEIPT_BYTES),
        ("max_json_depth", MAX_JSON_NESTING_DEPTH),
        (
            "max_retained_generations",
            workspace::MAX_RETAINED_GENERATIONS,
        ),
        ("max_staging_attempts", workspace::MAX_STAGING_ATTEMPTS),
        ("max_unexpected_inventory_entries", 0),
    ];
    if expected
        .into_iter()
        .any(|(key, expected)| number(object, key).ok() != Some(expected))
    {
        return Err(vec![format_error(
            "workspace evidence carries noncanonical limits",
        )]);
    }
    Ok(())
}

fn validate_nonclaims(value: &serde_json::Value) -> Result<(), Vec<Diagnostic>> {
    let array = value.as_array().ok_or_else(|| {
        vec![format_error(
            "workspace evidence nonclaims must be an array",
        )]
    })?;
    if array.len() != NONCLAIMS.len()
        || array
            .iter()
            .zip(NONCLAIMS)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(vec![format_error(
            "workspace evidence nonclaims are noncanonical",
        )]);
    }
    Ok(())
}

fn same_bindings(left: &WorkspaceEvidenceFacts, right: &WorkspaceEvidenceFacts) -> bool {
    render_document(EVIDENCE_SCHEMA, None, left, 0, 0, 0)
        == render_document(EVIDENCE_SCHEMA, None, right, 0, 0, 0)
}

fn read_evidence_bounded(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let file = File::open(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I213",
            format!(
                "cannot read Semantic Workspace Patch Evidence {}: {error}",
                path.display()
            ),
        )]
    })?;
    let metadata = file.metadata().map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I213",
            format!(
                "cannot read Semantic Workspace Patch Evidence metadata {}: {error}",
                path.display()
            ),
        )]
    })?;
    if !metadata.is_file() {
        return Err(vec![Diagnostic::io(
            "SPX-I213",
            "cannot read Semantic Workspace Patch Evidence: input is not a regular file",
        )]);
    }
    if metadata.len() > MAX_EVIDENCE_BYTES as u64 {
        return Err(vec![limit_field(
            "max_workspace_evidence_bytes",
            MAX_EVIDENCE_BYTES,
        )]);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_EVIDENCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I213",
                format!(
                    "cannot read Semantic Workspace Patch Evidence {}: {error}",
                    path.display()
                ),
            )]
        })?;
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(vec![limit_field(
            "max_workspace_evidence_bytes",
            MAX_EVIDENCE_BYTES,
        )]);
    }
    String::from_utf8(bytes).map_err(|_| {
        vec![Diagnostic::io(
            "SPX-I213",
            "cannot read Semantic Workspace Patch Evidence: input is not UTF-8",
        )]
    })
}

fn validate_json_structure(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut stack = Vec::with_capacity(MAX_JSON_NESTING_DEPTH);
    let mut in_string = false;
    let mut escaped = false;
    for byte in source.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                if stack.len() == MAX_JSON_NESTING_DEPTH {
                    return Err(vec![limit_field("max_json_depth", MAX_JSON_NESTING_DEPTH)]);
                }
                stack.push(byte);
            }
            b'}' if stack.pop() != Some(b'{') => {
                return Err(vec![format_error(
                    "workspace evidence JSON structure is unbalanced",
                )]);
            }
            b']' if stack.pop() != Some(b'[') => {
                return Err(vec![format_error(
                    "workspace evidence JSON structure is unbalanced",
                )]);
            }
            b'}' | b']' => {}
            _ => {}
        }
    }
    if in_string || escaped || !stack.is_empty() {
        return Err(vec![format_error(
            "workspace evidence JSON structure is unbalanced",
        )]);
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a serde_json::Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Vec<Diagnostic>> {
    let object = value.as_object().ok_or_else(|| {
        vec![format_error(format!(
            "workspace evidence {label} must be an object"
        ))]
    })?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(vec![format_error(format!(
            "workspace evidence {label} has missing or extra fields"
        ))]);
    }
    Ok(object)
}

fn text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, Vec<Diagnostic>> {
    object[key].as_str().map(str::to_owned).ok_or_else(|| {
        vec![format_error(format!(
            "workspace evidence field `{key}` must be text"
        ))]
    })
}

fn require_text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) -> Result<(), Vec<Diagnostic>> {
    if object[key].as_str() != Some(expected) {
        return Err(vec![format_error(format!(
            "workspace evidence field `{key}` has the wrong value"
        ))]);
    }
    Ok(())
}

fn digest_text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, Vec<Diagnostic>> {
    let value = text(object, key)?;
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(vec![format_error(format!(
            "workspace evidence field `{key}` is not a canonical SHA-256 digest"
        ))]);
    }
    Ok(value)
}

fn graph_schema_text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, Vec<Diagnostic>> {
    let value = text(object, key)?;
    if !matches!(
        value.as_str(),
        "semaprax.graph.v10"
            | "semaprax.graph.v11"
            | "semaprax.graph.v12"
            | "semaprax.graph.v13"
            | "semaprax.graph.v14"
    ) {
        return Err(vec![format_error(
            "workspace evidence carries an unsupported Graph schema",
        )]);
    }
    Ok(value)
}

fn number(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<usize, Vec<Diagnostic>> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            vec![format_error(format!(
                "workspace evidence field `{key}` must be a bounded integer"
            ))]
        })
}

fn push_json(output: &mut crate::bounded_output::CappedString, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value.is_control() => {
                let _ = write!(output, "\\u{:04x}", value as u32);
            }
            value => output.push(value),
        }
    }
    output.push('"');
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn map_review_child_diagnostics(
    diagnostics: Vec<Diagnostic>,
    total_node_budget_is_tighter: bool,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| match diagnostic.code {
            "SPX-G109" | "SPX-G120"
                if diagnostic.message.contains("node") && total_node_budget_is_tighter =>
            {
                limit_field("max_total_impact_nodes", MAX_TOTAL_IMPACT_NODES)
            }
            "SPX-G109" | "SPX-G120" if diagnostic.message.contains("node") => {
                limit_field("max_child_impact_nodes", MAX_CHILD_IMPACT_NODES)
            }
            // A completeness violation is a review policy failure, not a
            // byte- or node-budget breach; surface it unchanged instead of
            // naming an unviolated limit.
            "SPX-G109" | "SPX-G120" if diagnostic.message.contains("complete, nontruncated") => {
                diagnostic
            }
            "SPX-G109" | "SPX-G120" if diagnostic.message.contains("Impact") => {
                limit_field("max_total_impact_bytes", MAX_TOTAL_IMPACT_BYTES)
            }
            "SPX-G109" | "SPX-G120" => {
                limit_field("max_total_review_bytes", MAX_TOTAL_REVIEW_BYTES)
            }
            "SPX-G110" | "SPX-G121" | "SPX-G130" | "SPX-G132" | "SPX-G133" => invariant(),
            _ => diagnostic,
        })
        .collect()
}

fn map_facts_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| match diagnostic.code {
            "SPX-G131" => limit_field("max_total_review_bytes", MAX_TOTAL_REVIEW_BYTES),
            "SPX-G130" | "SPX-G132" | "SPX-G133" => invariant(),
            _ => diagnostic,
        })
        .collect()
}

fn map_child_render_diagnostics(
    diagnostics: Vec<Diagnostic>,
    total_budget_is_tighter: bool,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| match diagnostic.code {
            "SPX-G131" if total_budget_is_tighter => limit_field(
                "max_total_child_patch_evidence_bytes",
                MAX_TOTAL_CHILD_PATCH_EVIDENCE_BYTES,
            ),
            "SPX-G131" => limit_field(
                "max_child_patch_evidence_bytes",
                MAX_CHILD_PATCH_EVIDENCE_BYTES,
            ),
            "SPX-G130" | "SPX-G132" | "SPX-G133" => invariant(),
            _ => diagnostic,
        })
        .collect()
}

fn format_error(message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::io(
        "SPX-G160",
        if message == FORMAT_LEAD {
            message
        } else {
            format!("{FORMAT_LEAD}: {message}")
        },
    )
}

fn limit_field(field: &str, maximum: usize) -> Diagnostic {
    Diagnostic::io(
        "SPX-G161",
        format!("Semantic Workspace Patch Evidence `{field}` exceeds {maximum}"),
    )
}

fn mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G162",
        "submitted Semantic Workspace Patch Evidence differs from independent canonical replay",
    )
}

fn invariant() -> Diagnostic {
    Diagnostic::io(
        "SPX-G163",
        "typed Semantic Workspace Patch Evidence bindings disagree with the sealed workspace build",
    )
}

#[cfg(test)]
#[path = "workspace_patch_evidence/tests.rs"]
mod tests;
