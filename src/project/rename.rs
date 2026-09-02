//! Read-only Project Rename Planning v1.
//!
//! Planning selects one explicit monomorphic Web export from retained typed
//! Project state, derives one canonical Patch-v1 buffer, and validates one
//! complete candidate Project overlay. It owns no path or commit authority.

use std::path::PathBuf;

use sha2::{Digest as _, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::{build, ProjectSnapshot, ProjectSource, MAX_STABLE_ID_BYTES};

pub(crate) const PROJECT_RENAME_PREVIEW_SCHEMA: &str = "semaprax.project-rename-preview.v1";
pub(crate) const PROJECT_RENAME_DERIVATION_SCHEMA: &str = "semaprax.project-rename-derivation.v1";
pub(crate) const PROJECT_CHANGE_IMPACT_SCHEMA: &str = "semaprax.project-change-impact.v1";
pub(crate) const PROJECT_CHANGE_REVIEW_SCHEMA: &str = "semaprax.project-change-review.v1";
pub(crate) const PROJECT_CHANGE_PREVIEW_SCHEMA: &str = "semaprax.project-change-preview.v1";
const MAX_RENAME_NAME_BYTES: usize = 128;
const MAX_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_DERIVATION_BYTES: usize = 64 * 1024;
const MAX_CHANGE_IMPACT_BYTES: usize = 256 * 1024;
const MAX_CHANGE_REVIEW_BYTES: usize = 512 * 1024;
const MAX_CHANGE_PREVIEW_BYTES: usize = 1024 * 1024;
const PREVIEW_DIGEST_DOMAIN: &[u8] = b"semaprax.project-rename-preview.payload.v1\0";
const PATCH_DIGEST_DOMAIN: &[u8] = b"semaprax.project-rename-preview.patch.v1\0";
const DERIVATION_DIGEST_DOMAIN: &[u8] = b"semaprax.project-rename-derivation.payload.v1\0";
const CHANGE_IMPACT_DIGEST_DOMAIN: &[u8] = b"semaprax.project-change-impact.payload.v1\0";
const CHANGE_REVIEW_DIGEST_DOMAIN: &[u8] = b"semaprax.project-change-review.payload.v1\0";
const CHANGE_PREVIEW_DIGEST_DOMAIN: &[u8] = b"semaprax.project-change-preview.payload.v1\0";

pub(crate) struct PreparedProjectRename {
    target_path: PathBuf,
    patch_bytes: String,
    preview: String,
    preview_digest: String,
    derivation: String,
    derivation_digest: String,
    impact: String,
    impact_digest: String,
    review: String,
    review_digest: String,
    change_preview: String,
    change_preview_digest: String,
    base_workspace_revision: String,
    candidate_workspace_revision: String,
    base_project_revision: String,
    candidate_project_revision: String,
    base_source: ProjectSource,
    candidate_source: ProjectSource,
    candidate_project_graph: String,
    candidate_project_graph_digest: String,
}

/// Opaque capability for the planner's syntax-only Project-module patch pass.
/// Only this module can construct it, before the complete candidate Project is
/// admitted and promoted to [`PreparedProjectRename`].
pub(crate) struct ProjectRenameDerivation {
    source: String,
    patch_bytes: String,
    diagnostic_path: PathBuf,
}

impl ProjectRenameDerivation {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn patch_bytes(&self) -> &str {
        &self.patch_bytes
    }

    pub(crate) fn diagnostic_path(&self) -> &std::path::Path {
        &self.diagnostic_path
    }
}

impl PreparedProjectRename {
    pub(crate) fn patch_bytes(&self) -> &str {
        &self.patch_bytes
    }

    pub(crate) fn preview(&self) -> &str {
        &self.preview
    }

    pub(crate) fn preview_digest(&self) -> &str {
        &self.preview_digest
    }

    pub(crate) fn derivation(&self) -> &str {
        &self.derivation
    }

    pub(crate) fn derivation_digest(&self) -> &str {
        &self.derivation_digest
    }

    pub(crate) fn impact(&self) -> &str {
        &self.impact
    }

    pub(crate) fn impact_digest(&self) -> &str {
        &self.impact_digest
    }

    pub(crate) fn review(&self) -> &str {
        &self.review
    }

    pub(crate) fn review_digest(&self) -> &str {
        &self.review_digest
    }

    pub(crate) fn change_preview(&self) -> &str {
        &self.change_preview
    }

    pub(crate) fn change_preview_digest(&self) -> &str {
        &self.change_preview_digest
    }

    pub(crate) fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }

    pub(crate) fn candidate_workspace_revision(&self) -> &str {
        &self.candidate_workspace_revision
    }

    pub(crate) fn base_project_revision(&self) -> &str {
        &self.base_project_revision
    }

    pub(crate) fn candidate_project_revision(&self) -> &str {
        &self.candidate_project_revision
    }

    pub(crate) fn base_source(&self) -> &ProjectSource {
        &self.base_source
    }

    pub(crate) fn candidate_source(&self) -> &ProjectSource {
        &self.candidate_source
    }

    pub(crate) fn candidate_project_graph(&self) -> &str {
        &self.candidate_project_graph
    }

    pub(crate) fn candidate_project_graph_digest(&self) -> &str {
        &self.candidate_project_graph_digest
    }

    /// Acquire the ordinary A0 lock and exact authenticated source handoff for
    /// this completely validated plan. No raw Project-module deferred-profile
    /// constructor is exposed to the transport or other crate callers.
    pub(crate) fn acquire_a0(
        &self,
    ) -> Result<crate::patch::A0OwnedPreparedCommit, Vec<Diagnostic>> {
        crate::patch::acquire_prepared_project_rename(self)
    }

    pub(crate) fn target_path(&self) -> &std::path::Path {
        &self.target_path
    }
}

pub(super) fn prepare(
    snapshot: &ProjectSnapshot,
    target_id: &str,
    from: &str,
    to: &str,
) -> Result<PreparedProjectRename, Vec<Diagnostic>> {
    // The v1 change envelopes below freeze `semaprax.project.v1` and scalar
    // ownership conclusions. Refuse newer Project schemas before planning so
    // their richer profiles are never mislabeled as v1 evidence.
    if snapshot.manifest.schema() != super::PROJECT_SCHEMA {
        return Err(rename_error(
            "Project display-rename evidence currently admits only semaprax.project.v1",
        ));
    }
    validate_request_text("target_id", target_id, MAX_STABLE_ID_BYTES)?;
    validate_request_text("from", from, MAX_RENAME_NAME_BYTES)?;
    validate_request_text("to", to, MAX_RENAME_NAME_BYTES)?;
    if from == to {
        return Err(rename_error("Project rename must change the display name"));
    }

    let selected = snapshot
        .semantic
        .rename_function(target_id)
        .ok_or_else(|| rename_error("Project rename target is not a retained Project function"))?;
    if selected.origin != crate::hir::IdentityOrigin::Explicit {
        return Err(rename_error(
            "Project rename target must be an explicitly identified monomorphic function",
        ));
    }
    if !snapshot
        .manifest
        .web_exports()
        .iter()
        .any(|export| export == target_id)
    {
        return Err(rename_error(
            "Project rename target must be selected by manifest web_exports",
        ));
    }
    if selected.name != from {
        return Err(rename_error(
            "Project rename `from` does not match the authenticated function display name",
        ));
    }
    let base_source = snapshot
        .sources
        .iter()
        .find(|source| source.path == selected.path)
        .cloned()
        .ok_or_else(|| rename_error("Project rename source path is absent from the snapshot"))?;

    let patch_bytes = format!(
        "base {}\nrename {target_id} to {to}\n",
        base_source.source_revision
    );
    let target_path = diagnostic_path(snapshot, &selected.path);
    let derivation = ProjectRenameDerivation {
        source: base_source.source.clone(),
        patch_bytes: patch_bytes.clone(),
        diagnostic_path: target_path.clone(),
    };
    let preflight = crate::patch::preflight_project_rename_owned(&derivation)?;
    let candidate_text = preflight.canonical_candidate().to_owned();
    let candidate_total = snapshot
        .sources
        .iter()
        .try_fold(0usize, |total, source| {
            let bytes = if source.path == selected.path {
                candidate_text.len()
            } else {
                source.source.len()
            };
            total.checked_add(bytes)
        })
        .ok_or_else(|| rename_error("Project rename candidate source size overflow"))?;
    if candidate_total > super::MAX_TOTAL_SOURCE_BYTES {
        return Err(rename_error(
            "Project rename candidate exceeds the complete Project source bound",
        ));
    }
    let overlay = snapshot
        .sources
        .iter()
        .map(|source| SemanticWorkspaceSource {
            path: source.path.clone(),
            source: if source.path == selected.path {
                candidate_text.clone()
            } else {
                source.source.clone()
            },
        })
        .collect();
    // Exactly one complete candidate Phase-A/closure/graph/Web-admission build.
    let candidate = build::build_owned(&snapshot.manifest, overlay)?;
    let candidate_source = candidate
        .sources
        .iter()
        .find(|source| source.path == selected.path)
        .cloned()
        .ok_or_else(|| rename_error("Project rename candidate source fact is absent"))?;
    if candidate_source.source != candidate_text
        || candidate_source.source_revision != preflight.candidate_revision()
    {
        return Err(rename_error(
            "Project rename pure patch and complete candidate build disagree",
        ));
    }
    let patch_digest = domain_digest(PATCH_DIGEST_DOMAIN, patch_bytes.as_bytes());
    let candidate_graph_digest = candidate.semantic.graph_digest().to_owned();
    if !snapshot
        .semantic
        .display_rename_equivalent(&candidate.semantic)
    {
        return Err(rename_error(
            "Project rename changed typed call, import, type, effect, capability, or authority facts",
        ));
    }
    let payload = format!(
        "{{\"schema\":{},\"project_schema\":{},\"base_project_revision\":{},\"candidate_project_revision\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"target\":{{\"stable_id\":{},\"from\":{},\"to\":{},\"path\":{}}},\"patch\":{{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":{},\"bytes\":{}}},\"base_source\":{},\"candidate_source\":{},\"candidate_project_graph\":{{\"schema\":{},\"digest\":{}}},\"limits\":{{\"max_preview_bytes\":{},\"max_target_id_bytes\":{},\"max_name_bytes\":{}}},\"nonclaims\":[\"read_only_plan_no_commit_authority\",\"no_request_selected_path_or_source_bytes\",\"no_multi_file_or_import_alias_rename\",\"no_build_target_or_test_execution\",\"no_provenance_approval_or_exactly_once_effect\"]}}",
        quote_json(PROJECT_RENAME_PREVIEW_SCHEMA),
        quote_json(super::PROJECT_SCHEMA),
        quote_json(snapshot.project_revision()),
        quote_json(&candidate.project_revision),
        quote_json(snapshot.workspace_revision()),
        quote_json(&candidate.workspace_revision),
        quote_json(target_id),
        quote_json(from),
        quote_json(to),
        quote_json(&selected.path),
        quote_json(&patch_digest),
        patch_bytes.len(),
        source_fact_json(&base_source),
        source_fact_json(&candidate_source),
        quote_json(super::PROJECT_SEMANTIC_GRAPH_SCHEMA),
        quote_json(&candidate_graph_digest),
        MAX_PREVIEW_BYTES,
        MAX_STABLE_ID_BYTES,
        MAX_RENAME_NAME_BYTES,
    );
    let preview_digest = domain_digest(PREVIEW_DIGEST_DOMAIN, payload.as_bytes());
    let preview = format!(
        "{},\"preview_digest\":{}}}",
        payload
            .strip_suffix('}')
            .expect("Project rename payload is an object"),
        quote_json(&preview_digest),
    );
    if preview.len() > MAX_PREVIEW_BYTES {
        return Err(rename_error(
            "Project rename preview exceeds its exact byte bound",
        ));
    }
    let derivation_payload = format!(
        "{{\"schema\":{},\"project_schema\":{},\"base_project_revision\":{},\"base_workspace_revision\":{},\"target\":{{\"stable_id\":{},\"from\":{},\"to\":{},\"path\":{}}},\"patch\":{{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":{},\"bytes\":{}}},\"validated_candidate\":{{\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"preview_digest\":{}}},\"nonclaims\":[\"read_only_server_derived_intent_no_commit_authority\",\"no_request_selected_path_source_or_patch_bytes\",\"single_exported_function_display_rename_only\",\"complete_candidate_validation_occurs_before_derivation_is_retained\"]}}",
        quote_json(PROJECT_RENAME_DERIVATION_SCHEMA),
        quote_json(super::PROJECT_SCHEMA),
        quote_json(snapshot.project_revision()),
        quote_json(snapshot.workspace_revision()),
        quote_json(target_id),
        quote_json(from),
        quote_json(to),
        quote_json(&selected.path),
        quote_json(&patch_digest),
        patch_bytes.len(),
        quote_json(&candidate.project_revision),
        quote_json(&candidate.workspace_revision),
        quote_json(&candidate_graph_digest),
        quote_json(&preview_digest),
    );
    let (derivation, derivation_digest) = bind_object(
        DERIVATION_DIGEST_DOMAIN,
        derivation_payload,
        MAX_DERIVATION_BYTES,
        "Project rename derivation",
    )?;

    let impact_options =
        crate::workspace_analysis::WorkspaceImpactOptions::new(16, 64 * 1024, 1024)
            .map_err(|diagnostic| vec![diagnostic])?;
    let base_dependency_impact = snapshot.semantic.impact(
        snapshot.manifest.name(),
        snapshot.project_revision(),
        snapshot.manifest.test_module(),
        crate::workspace_analysis::WorkspaceAnalysisTargetKind::Declaration,
        target_id,
        impact_options,
    )?;
    let candidate_dependency_impact = candidate.semantic.impact(
        snapshot.manifest.name(),
        &candidate.project_revision,
        snapshot.manifest.test_module(),
        crate::workspace_analysis::WorkspaceAnalysisTargetKind::Declaration,
        target_id,
        impact_options,
    )?;
    let impact_payload = format!(
        "{{\"schema\":{},\"project_schema\":{},\"operation\":{{\"kind\":\"display_rename\",\"stable_id\":{},\"from\":{},\"to\":{},\"path\":{}}},\"base_project_revision\":{},\"candidate_project_revision\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"base_project_graph_digest\":{},\"candidate_project_graph_digest\":{},\"derivation_digest\":{},\"preview_digest\":{},\"base_dependency_impact\":{},\"candidate_dependency_impact\":{},\"conclusions\":{{\"stable_identity_preserved\":true,\"selected_external_export_preserved\":true,\"behavioral_call_edge_delta\":false,\"source_projection_changed\":true,\"rebuild_required\":true}},\"nonclaims\":[\"bounded_display_rename_delta_not_general_project_impact\",\"structural_reverse_closure_over_six_edge_families_only\",\"no_target_execution_external_consumer_or_compatibility_proof\",\"no_commit_authority\"]}}",
        quote_json(PROJECT_CHANGE_IMPACT_SCHEMA),
        quote_json(super::PROJECT_SCHEMA),
        quote_json(target_id),
        quote_json(from),
        quote_json(to),
        quote_json(&selected.path),
        quote_json(snapshot.project_revision()),
        quote_json(&candidate.project_revision),
        quote_json(snapshot.workspace_revision()),
        quote_json(&candidate.workspace_revision),
        quote_json(snapshot.semantic.graph_digest()),
        quote_json(&candidate_graph_digest),
        quote_json(&derivation_digest),
        quote_json(&preview_digest),
        base_dependency_impact,
        candidate_dependency_impact,
    );
    let (impact, impact_digest) = bind_object(
        CHANGE_IMPACT_DIGEST_DOMAIN,
        impact_payload,
        MAX_CHANGE_IMPACT_BYTES,
        "Project change impact",
    )?;
    let review_payload = format!(
        "{{\"schema\":{},\"project_schema\":{},\"base_project_revision\":{},\"candidate_project_revision\":{},\"preview_digest\":{},\"impact_digest\":{},\"impact\":{},\"sections\":{{\"behavior\":[{{\"code\":\"project_display_rename_behavior_preserved\",\"assessment\":\"unchanged\",\"evidence\":\"impact.conclusions.behavioral_call_edge_delta\"}}],\"api_identity\":[{{\"code\":\"project_stable_export_identity_preserved\",\"assessment\":\"source_display_changed_external_identity_unchanged\",\"evidence\":\"impact.conclusions.stable_identity_preserved\"}}],\"security_authority\":[{{\"code\":\"project_authority_unchanged\",\"assessment\":\"unchanged\",\"evidence\":\"impact.base_dependency_impact\"}}],\"memory_ownership\":[{{\"code\":\"project_scalar_ownership_unchanged\",\"assessment\":\"unchanged\",\"evidence\":\"impact.conclusions.behavioral_call_edge_delta\"}}],\"target_artifact\":[{{\"code\":\"project_rebuild_required\",\"assessment\":\"changed_revision_requires_rebuild\",\"evidence\":\"impact.conclusions.rebuild_required\"}}],\"migration\":[{{\"code\":\"project_source_display_migration\",\"assessment\":\"source_projection_changed_stable_consumers_unchanged\",\"evidence\":\"impact.conclusions.source_projection_changed\"}}],\"unsafe\":[{{\"code\":\"project_unsafe_surface_absent\",\"assessment\":\"not_present_in_admitted_profile\",\"evidence\":\"impact.operation.kind\"}}]}},\"verdict\":\"review_required_rebuild_safe_for_stable_id_consumers_within_bounded_profile\",\"nonclaims\":[\"fixed_bounded_display_rename_review_not_general_security_or_compatibility_audit\",\"no_human_approval_policy_provenance_or_commit_authority\",\"no_target_execution\"]}}",
        quote_json(PROJECT_CHANGE_REVIEW_SCHEMA),
        quote_json(super::PROJECT_SCHEMA),
        quote_json(snapshot.project_revision()),
        quote_json(&candidate.project_revision),
        quote_json(&preview_digest),
        quote_json(&impact_digest),
        impact,
    );
    let (review, review_digest) = bind_object(
        CHANGE_REVIEW_DIGEST_DOMAIN,
        review_payload,
        MAX_CHANGE_REVIEW_BYTES,
        "Project change review",
    )?;
    let change_preview_payload = format!(
        "{{\"schema\":{},\"project_schema\":{},\"base_project_revision\":{},\"candidate_project_revision\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"derivation_digest\":{},\"rename_preview_digest\":{},\"impact_digest\":{},\"review_digest\":{},\"rename_preview\":{},\"impact\":{},\"review\":{},\"nonclaims\":[\"bounded_display_rename_change_only\",\"read_only_preview_no_commit_authority\",\"no_general_patch_multi_file_or_target_execution\"]}}",
        quote_json(PROJECT_CHANGE_PREVIEW_SCHEMA),
        quote_json(super::PROJECT_SCHEMA),
        quote_json(snapshot.project_revision()),
        quote_json(&candidate.project_revision),
        quote_json(snapshot.workspace_revision()),
        quote_json(&candidate.workspace_revision),
        quote_json(&derivation_digest),
        quote_json(&preview_digest),
        quote_json(&impact_digest),
        quote_json(&review_digest),
        preview,
        impact,
        review,
    );
    let (change_preview, change_preview_digest) = bind_object(
        CHANGE_PREVIEW_DIGEST_DOMAIN,
        change_preview_payload,
        MAX_CHANGE_PREVIEW_BYTES,
        "Project change preview",
    )?;
    Ok(PreparedProjectRename {
        target_path,
        patch_bytes,
        preview,
        preview_digest,
        derivation,
        derivation_digest,
        impact,
        impact_digest,
        review,
        review_digest,
        change_preview,
        change_preview_digest,
        base_workspace_revision: snapshot.workspace_revision().to_owned(),
        candidate_workspace_revision: candidate.workspace_revision,
        base_project_revision: snapshot.project_revision().to_owned(),
        candidate_project_revision: candidate.project_revision,
        base_source,
        candidate_source,
        candidate_project_graph: candidate.semantic.graph().to_owned(),
        candidate_project_graph_digest: candidate_graph_digest,
    })
}

fn bind_object(
    domain: &[u8],
    payload: String,
    max_bytes: usize,
    name: &str,
) -> Result<(String, String), Vec<Diagnostic>> {
    if payload.len() > max_bytes {
        return Err(rename_error(format!("{name} exceeds its exact byte bound")));
    }
    let digest = domain_digest(domain, payload.as_bytes());
    let bound = format!(
        "{},\"artifact_digest\":{}}}",
        payload
            .strip_suffix('}')
            .expect("canonical Project artifact payload is an object"),
        quote_json(&digest),
    );
    if bound.len() > max_bytes {
        return Err(rename_error(format!("{name} exceeds its exact byte bound")));
    }
    Ok((bound, digest))
}

fn validate_request_text(name: &str, value: &str, max_bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(rename_error(format!(
            "Project rename {name} must be nonempty, at most {max_bytes} bytes, and contain no control characters"
        )));
    }
    Ok(())
}

fn source_fact_json(source: &ProjectSource) -> String {
    format!(
        "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
        quote_json(&source.path),
        quote_json(&source.source_graph_schema),
        quote_json(&source.source_revision),
        quote_json(&source.source_digest),
        source.source.len(),
    )
}

fn diagnostic_path(snapshot: &ProjectSnapshot, relative: &str) -> PathBuf {
    snapshot.root.join(relative)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn rename_error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J109", message)]
}

#[cfg(test)]
#[path = "rename/tests.rs"]
mod tests;
