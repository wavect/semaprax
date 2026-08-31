//! Candidate-aware reverse cleanup facts through the shared checked-plan index.
//! No independent HIR walker, plan normalization or source authority.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{wire, ProjectCandidate};
use crate::diagnostic::Diagnostic;
use crate::project::{ProjectRevision, ProjectSemanticImage};

pub const PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA: &str =
    "semaprax.project-candidate-cleanup-dependencies.v1";
pub const PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-cleanup-dependencies-verification.v1";
pub const MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES: usize = 8 * 1024 * 1024;
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-cleanup-dependencies.report.v1\0";
const SIDE_DOMAIN: &[u8] = b"semaprax.candidate-cleanup-dependencies.side.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectCandidate {
    /// Source-bound before/after inventories for one persistent declaration.
    /// An absent declaration differs from a present one with no selected facts.
    pub fn cleanup_dependencies(&self, expected_candidate: &str, target: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if target.is_empty() || target.len() > 4096 || target.contains('\0') {
            return Err(invalid());
        }
        let present_before = self.base.semantic.image_symbol(target).is_some();
        let present_after = self.revision.semantic.image_symbol(target).is_some();
        if !present_before && !present_after {
            return Err(invalid());
        }
        let mut remaining = MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES;
        let before = side(&self.base, target, present_before, &mut remaining)?;
        let after = side(&self.revision, target, present_after, &mut remaining)?;
        let comparison = match (&before, &after) {
            (Some(before), Some(after)) => json!({
                "obligations_exact_equal":before["report"]["obligations"] == after["report"]["obligations"],
                "unavailable_templates_exact_equal":before["report"]["unavailable_templates"] == after["report"]["unavailable_templates"],
                "declaration_exact_equal":before["report"]["typed_declaration"] == after["report"]["typed_declaration"],
            }),
            _ => json!({"obligations_exact_equal":null,
                "unavailable_templates_exact_equal":null,"declaration_exact_equal":null}),
        };
        render(json!({
            "schema":PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA,
            "candidate_digest":expected_candidate,"target":target,
            "base_project_revision":self.base.project_revision(),
            "project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),
            "workspace_revision":self.revision.workspace_revision(),
            "presence":match (present_before,present_after) {
                (true,true)=>"both",(false,true)=>"added",(true,false)=>"removed",_=>unreachable!(),
            },
            "base":before,"candidate":after,"comparison":comparison,
            "comparison_basis":"exact_selected_checked_plan_facts_including_source_provenance_and_revision_local_ids",
            "evidence_owner":"shared_image_cleanup_dependency_index",
            "execution":false,"source_authority":false,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES,
                "max_combined_image_report_bytes":MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES},
            "nonclaims":["not_behavioral_equivalence","not_runtime_liveness_or_finalization",
                "not_source_span_or_plan_id_normalization","not_test_or_target_execution",
                "no_new_plan_or_analysis_authority","no_source_or_publication_authority"]
        }))
    }

    /// Replay the complete source intention history before exact recomputation.
    /// Submitted report bytes cannot populate an index or alter a plan.
    pub fn verify_cleanup_dependencies(
        &self,
        expected_candidate: &str,
        target: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES {
            return Err(capacity());
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        if replay
            .cleanup_dependencies(expected_candidate, target)?
            .as_bytes()
            != bytes
        {
            return Err(vec![Diagnostic::io(
                "SPX-G339",
                "candidate cleanup dependencies failed exact source-history replay",
            )]);
        }
        render(json!({
            "schema":PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA,
            "result":"exact_source_history_recomputation",
            "candidate_digest":expected_candidate,"target":target,
            "base_project_revision":self.base.project_revision(),
            "project_revision":self.revision.project_revision(),
            "report_digest":wire::digest(REPORT_DOMAIN,bytes),
            "execution":false,"source_authority":false,
        }))
    }
}

fn side(
    revision: &Arc<ProjectRevision>,
    target: &str,
    present: bool,
    remaining: &mut usize,
) -> Result<Option<Value>> {
    if !present {
        return Ok(None);
    }
    let image = ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision())?;
    let report = image.cleanup_dependencies(image.image_digest(), target)?;
    *remaining = remaining.checked_sub(report.len()).ok_or_else(capacity)?;
    // Only bounded compiler-created JSON reaches this parser. Untrusted report
    // bytes are compared directly after history replay in the verification API.
    let facts: Value = serde_json::from_str(&report).map_err(|_| invalid())?;
    Ok(Some(json!({"image_digest":image.image_digest(),
        "report_digest":wire::digest(SIDE_DOMAIN,report.as_bytes()),"report":facts})))
}

fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES).map_err(|_| capacity())
}
fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G337",
        "candidate cleanup dependency target or compiler report is invalid",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G338",
        "candidate cleanup dependency report exceeds its byte bound",
    )]
}
