//! Immutable, source-derived semantic candidates. No path or commit authority.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;
use crate::workspace_analysis::{WorkspaceAnalysisTargetKind, WorkspaceImpactOptions};

use super::{build, ProjectRevision, MAX_TOTAL_SOURCE_BYTES};

mod abi_delta;
mod analysis_artifact_evidence;
mod analysis_boundary_bundle;
mod analysis_coverage;
mod analysis_coverage_change;
mod analysis_evidence;
mod analysis_runtime_evidence;
mod archive;
mod artifact_delta;
mod catalog;
mod cleanup_dependencies;
mod contract_delta;
mod declaration;
mod delta;
mod dependency_navigation;
mod deployment_contract_evidence;
mod diagnostic_intent;
mod diagnostics;
mod draft;
mod draft_navigation;
mod draft_suggestions;
mod environment_consumer_review;
mod environment_review;
mod expression;
mod external_api_contract_delta;
mod external_api_contract_evidence;
mod extraction;
mod facet_navigation;
mod function_reference_rebind;
mod generated_file_provenance;
mod generic_rename;
mod git_publication;
mod impact_navigation;
mod intent;
mod interface;
mod interface_delta;
mod merge_preview;
mod movement;
mod ownership_delta;
mod package_consumer_replay;
mod publication;
mod rebase;
mod record_field;
mod recovery;
mod schemas;
mod source_review;
mod testing;
mod type_declaration;
mod type_rename;
mod wire;

pub use abi_delta::{
    MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES, PROJECT_CANDIDATE_ABI_DELTA_SCHEMA,
    PROJECT_CANDIDATE_ABI_DELTA_VERIFICATION_SCHEMA,
};
pub use analysis_artifact_evidence::{
    MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA,
};
pub use analysis_boundary_bundle::{
    MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_BYTES,
    MAX_PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_SCHEMA,
    PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_SCHEMA,
};
pub use analysis_coverage::{
    MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES, PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
};
pub use analysis_coverage_change::{
    CandidateAnalysisCoverageBoundaryInput, CandidateAnalysisCoverageChangeInput,
    MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_CHANGE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_COVERAGE_CHANGE_SCHEMA,
};
pub use analysis_evidence::{
    MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES, PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA,
};
pub use analysis_runtime_evidence::{
    MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA,
};
pub use archive::{
    ProjectCandidateArchive, MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES, PROJECT_CANDIDATE_ARCHIVE_SCHEMA,
};
pub use artifact_delta::{
    MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES, PROJECT_CANDIDATE_ARTIFACT_DELTA_SCHEMA,
    PROJECT_CANDIDATE_ARTIFACT_DELTA_VERIFICATION_SCHEMA,
};
pub use cleanup_dependencies::{
    MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES,
    PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA,
    PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA,
};
pub use contract_delta::{
    MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES, PROJECT_CANDIDATE_CONTRACT_DELTA_SCHEMA,
    PROJECT_CANDIDATE_CONTRACT_DELTA_VERIFICATION_SCHEMA,
};
pub use dependency_navigation::{
    MAX_PROJECT_CANDIDATE_DEPENDENCY_PAGE_BYTES, MAX_PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_BYTES,
    PROJECT_CANDIDATE_DEPENDENCY_PAGE_SCHEMA, PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_SCHEMA,
};
pub use deployment_contract_evidence::{
    MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEYS,
    MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEY_BYTES,
    MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_BYTES,
    MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_SCHEMA,
    PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA,
};
pub use environment_consumer_review::{
    MAX_PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_BYTES,
    PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_COVERAGE_SCHEMA,
    PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_SCHEMA,
};
pub use environment_review::{
    MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES, PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA,
};
pub use external_api_contract_delta::{
    MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_BYTES,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_BASE_DECLARATION_SCHEMA,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_SCHEMA,
};
pub use external_api_contract_evidence::{
    MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_BYTES,
    MAX_PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_BYTES,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DECLARATION_SCHEMA,
    PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA,
};
pub use facet_navigation::{
    MAX_PROJECT_CANDIDATE_FUNCTION_FACET_BYTES, MAX_PROJECT_CANDIDATE_FUNCTION_SUMMARY_BYTES,
    PROJECT_CANDIDATE_FUNCTION_FACET_ITEM_SCHEMA, PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA,
    PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA,
};
pub use function_reference_rebind::{
    MAX_PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_BYTES,
    PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_SCHEMA,
};
pub use generated_file_provenance::{
    MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_BYTES,
    MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_BYTES,
    MAX_PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_FILES,
    PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_DECLARATION_SCHEMA,
    PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA,
};
pub use ownership_delta::{
    MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES, PROJECT_CANDIDATE_OWNERSHIP_DELTA_SCHEMA,
    PROJECT_CANDIDATE_OWNERSHIP_DELTA_VERIFICATION_SCHEMA,
};
pub use package_consumer_replay::{
    CandidatePackageConsumerMigrationInput, CandidatePackageConsumerReplayInput,
    CandidatePackageSignatureConflictInput, MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_MIGRATION_BYTES,
    MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_BYTES,
    MAX_PROJECT_CANDIDATE_PACKAGE_SIGNATURE_CONFLICTS_BYTES,
    PROJECT_CANDIDATE_PACKAGE_CONSUMER_MIGRATION_SCHEMA,
    PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA,
    PROJECT_CANDIDATE_PACKAGE_SIGNATURE_CONFLICTS_SCHEMA,
};

pub use testing::{
    CandidateTestPolicy, CandidateTestReport, ProjectCandidateTestTaskOutcome,
    MAX_CANDIDATE_TEST_STEPS, MAX_PROJECT_CANDIDATE_TEST_PLAN_BYTES,
    MAX_PROJECT_CANDIDATE_TEST_REPORT_BYTES, PROJECT_CANDIDATE_TEST_PLAN_SCHEMA,
    PROJECT_CANDIDATE_TEST_REPORT_SCHEMA,
};

pub use diagnostics::{
    ProjectCandidateAttempt, ProjectCandidateAttemptOutcome, PROJECT_CANDIDATE_ATTEMPT_SCHEMA,
    PROJECT_CANDIDATE_REPAIR_CATALOG_SCHEMA,
};

pub use delta::{
    MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES, MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_BYTES,
    PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_SCHEMA, PROJECT_CANDIDATE_SEMANTIC_DELTA_SCHEMA,
};

pub use impact_navigation::{
    CandidateImpactPageOptions, CandidateImpactView, MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES,
    MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES, PROJECT_CANDIDATE_IMPACT_ITEM_SCHEMA,
    PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA, PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA,
};
pub use interface_delta::{
    MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES, PROJECT_CANDIDATE_INTERFACE_DELTA_SCHEMA,
    PROJECT_CANDIDATE_INTERFACE_DELTA_VERIFICATION_SCHEMA,
};

pub use draft::{
    ProjectCandidateDraft, ProjectCandidateDraftArchive, ProjectCandidateDraftMerge,
    ProjectCandidateDraftRebase, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES,
    MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE, MAX_PROJECT_CANDIDATE_DRAFT_MERGE_BYTES,
    MAX_PROJECT_CANDIDATE_DRAFT_REBASE_BYTES, MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES,
    MAX_PROJECT_CANDIDATE_HOLES, MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES,
    PROJECT_CANDIDATE_DRAFT_ARCHIVE_COMPATIBILITY, PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,
    PROJECT_CANDIDATE_DRAFT_LINEAGE_MERGE_SCHEMA, PROJECT_CANDIDATE_DRAFT_LINEAGE_REBASE_SCHEMA,
    PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_COMPATIBILITY,
    PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA, PROJECT_CANDIDATE_DRAFT_LINEAGE_SCHEMA,
    PROJECT_CANDIDATE_DRAFT_MERGE_SCHEMA, PROJECT_CANDIDATE_DRAFT_REBASE_SCHEMA,
    PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY, PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
    PROJECT_CANDIDATE_DRAFT_SCHEMA, PROJECT_CANDIDATE_HOLE_CONTEXT_SCHEMA,
    PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
};
pub use git_publication::{
    apply_candidate_git_publication, CandidateGitAuthority, CandidateGitCommitMetadata,
    CandidateGitObject, CandidateGitObjectKind, CandidateGitProcessAuthority,
    CandidateGitRefUpdate, CandidateGitRepository, CandidateGitTarget, GitObjectFormat,
    PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA,
};
pub use publication::{
    apply_candidate_publication, prepare_candidate_publication, ProjectCandidatePublication,
    MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES, PROJECT_CANDIDATE_PUBLICATION_SCHEMA,
};

pub use draft_navigation::{
    MAX_PROJECT_HOLE_NAVIGATION_BYTES, MAX_PROJECT_HOLE_NAVIGATION_ITEMS, PROJECT_HOLE_PAGE_SCHEMA,
    PROJECT_HOLE_SUMMARY_SCHEMA,
};
pub use draft_suggestions::{
    MAX_PROJECT_HOLE_FILL_SUGGESTIONS_BYTES, PROJECT_HOLE_FILL_SUGGESTIONS_SCHEMA,
};
pub use merge_preview::{
    MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES, PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA,
    PROJECT_CANDIDATE_MERGE_PREVIEW_VERIFICATION_SCHEMA,
};
pub use rebase::{ProjectCandidateRebase, PROJECT_CANDIDATE_REBASE_SCHEMA};
pub use recovery::{
    MAX_PROJECT_CANDIDATE_RECOVERY_BYTES, PROJECT_CANDIDATE_RECOVERY_COMPATIBILITY,
    PROJECT_CANDIDATE_RECOVERY_SCHEMA,
};
pub use source_review::{
    MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES, PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
};

pub const SEMANTIC_CHANGE_SCHEMA: &str = "semaprax.semantic-change.v1";
pub const PROJECT_CANDIDATE_SCHEMA: &str = "semaprax.project-candidate.v1";
pub const MAX_SEMANTIC_CHANGE_BYTES: usize = 1024 * 1024;
pub const MAX_PROJECT_CANDIDATE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CHANGES: usize = 32;

/// These constraints are mandatory, not caller assertions that bypass checks.
pub const SEMANTIC_CHANGE_REQUIREMENTS: &[&str] = &[
    "preserve_stable_identity",
    "preserve_public_exports",
    "update_all_callers",
    "no_new_effects",
    "no_new_capabilities",
    "preserve_contracts",
    "revalidate_ownership_and_cleanup",
    "preserve_project_profile_admission",
    "preserve_admitted_core_targets",
];

/// A closed, canonical typed intention bound to one current Project revision.
#[derive(Clone)]
pub struct SemanticChange {
    base_revision: String,
    intent: Value,
    json: String,
}

impl SemanticChange {
    /// Construct canonical change bytes; semantic admission happens in apply.
    pub fn new(base_revision: &str, intent: &Value) -> Result<Self, Vec<Diagnostic>> {
        wire::validate_digest(base_revision)?;
        wire::validate_value(intent)?;
        let value = json!({
            "schema": SEMANTIC_CHANGE_SCHEMA,
            "base_revision": base_revision,
            "intent": intent,
            "requirements": SEMANTIC_CHANGE_REQUIREMENTS,
        });
        let json = wire::render(value, MAX_SEMANTIC_CHANGE_BYTES)?;
        Ok(Self {
            base_revision: base_revision.to_owned(),
            intent: intent.clone(),
            json,
        })
    }

    /// Exact canonical admission rejects duplicate/unknown keys and alternate
    /// JSON encodings before the intention can enter a candidate.
    pub fn from_json(bytes: &[u8]) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_SEMANTIC_CHANGE_BYTES {
            return Err(capacity("semantic change exceeds its input bound"));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("semantic change is not bounded valid JSON"))?;
        wire::validate_value(&value)?;
        let base = value
            .get("base_revision")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("semantic change requires a base revision"))?;
        let intent = value
            .get("intent")
            .ok_or_else(|| invalid("semantic change requires an intent"))?;
        let change = Self::new(base, intent)?;
        if change.json.as_bytes() != bytes {
            return Err(invalid(
                "semantic change must have exact schema, requirements, and canonical bytes",
            ));
        }
        Ok(change)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }
}

/// A complete validated overlay. Applying another intent returns a new value;
/// neither its base nor any sibling candidate is modified. Dropping discards it.
pub struct ProjectCandidate {
    base: Arc<ProjectRevision>,
    revision: Arc<ProjectRevision>,
    changes: Vec<SemanticChange>,
    summaries: Vec<Value>,
    base_targets: Arc<Value>,
    targets: Value,
    json: String,
    digest: String,
    // Derived only from this immutable candidate; never serialized or authority.
    source_review_cache: OnceLock<Result<Arc<str>, Vec<Diagnostic>>>,
}

pub(super) fn target_projection_facts(
    revision: &ProjectRevision,
) -> Result<Value, Vec<Diagnostic>> {
    wire::target_facts(revision)
}

impl ProjectCandidate {
    pub fn open(
        base: Arc<ProjectRevision>,
        expected_revision: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        require_revision(&base, expected_revision)?;
        let targets = Arc::new(wire::target_facts(&base)?);
        Self::finish(
            Arc::clone(&base),
            base,
            vec![],
            vec![],
            Arc::clone(&targets),
            &targets,
        )
    }

    pub fn apply(
        &self,
        expected_candidate: &str,
        change: &SemanticChange,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_candidate(expected_candidate)?;
        require_revision(&self.revision, change.base_revision())?;
        if self.changes.len() >= MAX_CHANGES {
            return Err(capacity("candidate intention count exceeds 32"));
        }
        let mut programs = parse_revision(&self.revision)?;
        let mut before_implementations = interface::inventory(&programs)?;
        let mut before = invariant_facts(&programs);
        let mut field_addition = None;
        let mut movement = None;
        let mut implementation_addition = None;
        let mut type_addition = None;
        let mut nominal_rename = None;
        let generic_rename = generic_rename::plan(&self.revision, &programs, &change.intent)?;
        let (summary, addition) = match change.intent.get("kind").and_then(Value::as_str) {
            Some("rename_declaration")
                if type_rename::eligible(
                    &self.revision,
                    change.intent["target"].as_str().unwrap_or(""),
                )? =>
            {
                let (summary, rename) =
                    type_rename::apply(&self.revision, &mut programs, &change.intent)?;
                nominal_rename = Some(rename);
                (summary, None)
            }
            Some("implement_interface") => {
                let (summary, added) =
                    interface::apply(&self.revision, &mut programs, &change.intent)?;
                if before_implementations
                    .insert(added.id.clone(), added.fact.clone())
                    .is_some()
                {
                    return Err(interface::mismatch());
                }
                implementation_addition = Some(added);
                (summary, None)
            }
            Some("repair_diagnostic") => (
                diagnostic_intent::apply(self, &mut programs, &change.intent)?,
                None,
            ),
            Some("add_record_field") => {
                let (summary, field) =
                    record_field::apply(&self.revision, &mut programs, &change.intent)?;
                field_addition = Some(field);
                (summary, None)
            }
            Some("move_declaration") => {
                let (summary, moved) =
                    movement::apply(&self.revision, &mut programs, &change.intent)?;
                movement = Some(moved);
                (summary, None)
            }
            Some("add_declaration") => {
                if change.intent["declaration"].get("kind").is_some() {
                    let (summary, added) =
                        type_declaration::apply(&self.revision, &mut programs, &change.intent)?;
                    type_addition = Some(added);
                    (summary, None)
                } else {
                    let (summary, addition) =
                        declaration::apply(&self.revision, &mut programs, &change.intent)?;
                    (summary, Some(addition))
                }
            }
            Some("extract_function") => {
                let (summary, addition) =
                    extraction::apply(&self.revision, &mut programs, &change.intent)?;
                (summary, Some(addition))
            }
            Some("replace_expression") => (
                expression::apply(&self.revision, &mut programs, &change.intent)?,
                None,
            ),
            Some("replace_contract_expression") => (
                expression::apply_contract(&self.revision, &mut programs, &change.intent)?,
                None,
            ),
            _ => (
                intent::apply_with_revision(&self.revision, &mut programs, &change.intent)?,
                None,
            ),
        };
        intent::validate_builtin_namespace(
            &programs,
            self.changes
                .iter()
                .map(|change| &change.intent)
                .chain(std::iter::once(&change.intent)),
        )?;
        if let Some(moved) = &movement {
            let fact = before[&moved.source_path]["functions"]
                .as_object_mut()
                .and_then(|functions| functions.remove(&moved.id))
                .ok_or_else(|| invalid("moved function is absent from its source inventory"))?;
            let destination = before[&moved.destination_path]["functions"]
                .as_object_mut()
                .ok_or_else(|| invalid("move destination is absent from the source inventory"))?;
            if destination.insert(moved.id.clone(), fact).is_some() {
                return Err(invalid(
                    "move destination already contains the function identity",
                ));
            }
        }
        if let Some(addition) = &addition {
            let functions = before[&addition.path]["functions"]
                .as_object_mut()
                .ok_or_else(|| {
                    invalid("declaration owner is absent from the original source inventory")
                })?;
            if functions.insert(addition.id.clone(), json!({
                "effects":addition.effects, "requires":addition.requires_count, "ensures":addition.ensures_count,
            })).is_some() {
                return Err(invalid("declaration addition replaced an existing identity"));
            }
        }
        if summary.kind == "add_contract" {
            // The exact validated intention permits one additive predicate,
            // while all prior predicates, effects and permits stay intact.
            let phase = change.intent["phase"]
                .as_str()
                .ok_or_else(|| invalid("contract phase is missing"))?;
            let owner = programs
                .iter()
                .find(|program| {
                    program
                        .functions
                        .iter()
                        .any(|function| function.stable_id == summary.target_id)
                })
                .ok_or_else(|| invalid("contract owner is missing"))?;
            let count = before[&owner.path]["functions"][&summary.target_id][phase]
                .as_u64()
                .ok_or_else(|| invalid("contract baseline inventory is missing"))?;
            before[&owner.path]["functions"][&summary.target_id][phase] = json!(count + 1);
        }
        let after = invariant_facts(&programs);
        interface::identities(&programs)?;
        if interface::inventory(&programs)? != before_implementations {
            return Err(interface::mismatch());
        }
        if before != after {
            return Err(invalid(
                "intent changed permits, effects, or contract inventory",
            ));
        }
        let sources = materialize(&programs)?;
        // Candidate meaning must re-enter through canonical human source, never
        // through mutated HIR/graph fields. build_owned performs real Phase A,
        // ownership/cleanup replay, linkage and manifest-profile admission.
        let replay_sources = sources
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source.path.clone(),
                source: source.source.clone(),
            })
            .collect();
        let built = build::build_owned(self.revision.manifest(), sources)?;
        let candidate = Arc::new(ProjectRevision::from_built(
            self.revision.manifest().clone(),
            built,
        ));
        let replay = build::build_owned(candidate.manifest(), replay_sources)?;
        if replay.project_revision != candidate.project_revision()
            || replay.semantic.graph() != candidate.semantic_graph()
            || replay.sources.len() != candidate.sources().len()
            || replay
                .sources
                .iter()
                .zip(candidate.sources())
                .any(|(a, b)| a != b)
        {
            return Err(stale(
                "candidate source replay disagrees with intended projection",
            ));
        }
        preserve_explicit_identities(
            &self.revision,
            &candidate,
            addition.as_ref(),
            field_addition.as_ref(),
            movement.as_ref(),
            type_addition.as_ref(),
        )?;
        if let Some(addition) = addition.as_ref() {
            declaration::validate_added_signature(&candidate, addition)?;
        }
        if summary.kind == "change_function_signature" {
            intent::validate_computed_signature(&candidate, &change.intent)?;
        }
        if type_addition.is_some() {
            type_declaration::validate(&self.revision, &candidate, &change.intent)?;
        }
        if let Some(rename) = &nominal_rename {
            type_rename::validate(&candidate, rename)?;
        }
        if let Some(rename) = &generic_rename {
            generic_rename::validate(&candidate, rename)?;
        }
        let rebuilt_programs = parse_revision(&candidate)?;
        interface::identities(&rebuilt_programs)?;
        if interface::inventory(&rebuilt_programs)? != before_implementations {
            return Err(interface::mismatch());
        }
        if summary.kind == "replace_expression" {
            expression::validate_replacement(&self.revision, &candidate, &change.intent)?;
        }
        if summary.kind == "replace_contract_expression" {
            expression::validate_contract_replacement(&self.revision, &candidate, &change.intent)?;
        }
        if summary.kind == "extract_function" {
            extraction::validate(&self.revision, &candidate, &change.intent)?;
        }
        if summary.kind == "add_record_field" {
            record_field::validate(&self.revision, &candidate, &change.intent)?;
        }
        if summary.kind == "move_declaration" {
            movement::validate(&self.revision, &candidate, &change.intent)?;
        }
        if self.revision.manifest().to_canonical_toml() != candidate.manifest().to_canonical_toml()
        {
            return Err(invalid(
                "candidate changed the manifest or exported identity set",
            ));
        }
        let targets = wire::target_facts(&candidate)?;
        wire::preserve_targets(&self.targets, &targets)?;
        let mut changes = self.changes.clone();
        changes.push(change.clone());
        let mut summaries = self.summaries.clone();
        let mut operation = json!({
            "kind": summary.kind,
            "target": summary.target_id,
            "migrated_calls": summary.migrated_calls,
        });
        if let Some(addition) = addition {
            operation["new_declaration"] = json!({"id":addition.id,"name":addition.name,"path":addition.path,"module":addition.module});
        }
        if let Some(addition) = type_addition {
            operation["new_declaration"] = json!({"id":addition.id,"name":addition.name,"kind":addition.kind,
                "path":addition.path,"module":addition.module,"identities":addition.expected_identities});
        }
        if let Some(field) = field_addition {
            operation["new_declaration"] = json!({"id":field.id,"name":field.name,"owner":field.owner,"kind":"field","path":field.path,"module":field.module});
        }
        if let Some(moved) = movement {
            operation["relocation"] = json!({"id":moved.id,"source_path":moved.source_path,"source_module":moved.source_module,"destination_path":moved.destination_path,"destination_module":moved.destination_module});
        }
        if let Some(added) = implementation_addition {
            operation["new_declaration"] = json!({"id":added.id,"owner":added.owner,"kind":"protocol_implementation","path":added.path,"module":added.module,"runtime_graph_declaration":false});
            operation["source_conformance"] = added.fact;
        }
        if generic_rename.is_some() {
            operation["generic_instances"] = json!({
                "preserved": true,
                "evidence": "exact_retained_checked_template_and_concrete_instance_hir",
                "target_execution": false,
            });
        }
        summaries.push(operation);
        Self::finish(
            Arc::clone(&self.base),
            candidate,
            changes,
            summaries,
            Arc::clone(&self.base_targets),
            &targets,
        )
    }

    /// Reconstruct all intentions from the base and compare the complete source
    /// diff/evidence bytes. A self-consistently rehashed capsule is insufficient.
    pub fn replay(
        base: Arc<ProjectRevision>,
        expected_base: &str,
        changes: &[SemanticChange],
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_PROJECT_CANDIDATE_BYTES || changes.len() > MAX_CHANGES {
            return Err(capacity("candidate replay exceeds its bound"));
        }
        let mut candidate = Self::open(base, expected_base)?;
        for change in changes {
            candidate = candidate.apply(candidate.candidate_digest(), change)?;
        }
        if candidate.to_json().as_bytes() != bytes {
            return Err(stale("candidate evidence failed exact source replay"));
        }
        Ok(candidate)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn candidate_digest(&self) -> &str {
        &self.digest
    }
    pub fn revision(&self) -> &Arc<ProjectRevision> {
        &self.revision
    }
    pub fn base_revision(&self) -> &Arc<ProjectRevision> {
        &self.base
    }

    /// Comparison is descriptive, not a semantic-merge or compatibility proof.
    pub fn compare(&self, other: &Self) -> Result<String, Vec<Diagnostic>> {
        if self.base.project_revision() != other.base.project_revision() {
            return Err(stale(
                "candidate comparison requires the same base revision",
            ));
        }
        let targets = |candidate: &Self| {
            candidate
                .summaries
                .iter()
                .filter_map(|summary| summary["target"].as_str())
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        };
        let left = targets(self);
        let right = targets(other);
        wire::render(
            json!({
                "schema": "semaprax.project-candidate-comparison.v1",
                "base_revision": self.base.project_revision(),
                "left": self.candidate_digest(), "right": other.candidate_digest(),
                "same_source_revision": self.revision.project_revision() == other.revision.project_revision(),
                "overlapping_targets": left.intersection(&right).collect::<Vec<_>>(),
                "classification": "descriptive_requires_revalidation_before_merge",
                "commit_authority": false,
            }),
            MAX_SEMANTIC_CHANGE_BYTES,
        )
    }

    fn require_candidate(&self, expected: &str) -> Result<(), Vec<Diagnostic>> {
        wire::validate_digest(expected)?;
        if expected != self.digest {
            return Err(stale("candidate digest is stale"));
        }
        Ok(())
    }

    fn finish(
        base: Arc<ProjectRevision>,
        revision: Arc<ProjectRevision>,
        changes: Vec<SemanticChange>,
        summaries: Vec<Value>,
        base_targets: Arc<Value>,
        targets: &Value,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut source_changes = Vec::new();
        if base.sources().len() != revision.sources().len() {
            return Err(invalid("candidate changed source inventory cardinality"));
        }
        for (before, after) in base.sources().iter().zip(revision.sources()) {
            if before.path() != after.path() {
                return Err(invalid("candidate changed declared source paths"));
            }
            if before.source() == after.source() {
                continue;
            }
            let diff = wire::source_diff(before.path(), before.source(), after.source())?;
            source_changes.push(json!({
                "path": before.path(), "base_digest": before.source_digest(),
                "candidate_digest": after.source_digest(), "replacement_source": after.source(),
                "source_diff_digest": wire::digest(b"semaprax.candidate.source-diff.v1\0", diff.as_bytes()),
                "source_diff": diff,
            }));
        }
        let introduced = summaries
            .iter()
            .filter_map(|s| s["new_declaration"]["id"].as_str())
            .chain(
                summaries
                    .iter()
                    .flat_map(|summary| {
                        summary["new_declaration"]["identities"]
                            .as_array()
                            .into_iter()
                            .flatten()
                    })
                    .filter_map(|identity| identity["id"].as_str()),
            )
            .collect::<BTreeSet<_>>();
        let selected = summaries
            .iter()
            .filter_map(|s| s["target"].as_str())
            .chain(introduced.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut impacts = Vec::new();
        let bound_member_impacts = summaries
            .iter()
            .any(|summary| summary["new_declaration"]["identities"].is_array());
        let mut impact_bytes = 2usize;
        for id in selected {
            let options = WorkspaceImpactOptions::default();
            let before = if introduced.contains(id) && base.semantic.image_symbol(id).is_none() {
                Value::Null
            } else {
                serde_json::from_str::<Value>(&base.semantic_impact(
                    WorkspaceAnalysisTargetKind::Declaration,
                    id,
                    options,
                )?)
                .map_err(|_| invalid("invalid base impact"))?
            };
            let after = if revision.semantic.image_symbol(id).is_some() {
                serde_json::from_str::<Value>(&revision.semantic_impact(
                    WorkspaceAnalysisTargetKind::Declaration,
                    id,
                    options,
                )?)
                .map_err(|_| invalid("invalid candidate impact"))?
            } else if let Some(binding) = interface::binding(&revision, id)? {
                json!({"availability":"source_static_conformance_only","binding":binding,"cross_file_impact_available":false})
            } else {
                return Err(invalid(
                    "candidate impact target is absent from runtime and source inventories",
                ));
            };
            let impact = json!({"target": id, "base": before, "candidate": after});
            if bound_member_impacts {
                impact_bytes = impact_bytes.saturating_add(
                    wire::render(impact.clone(), MAX_PROJECT_CANDIDATE_BYTES)?.len(),
                );
                if impact_bytes > MAX_PROJECT_CANDIDATE_BYTES {
                    return Err(capacity(
                        "type member impact inventory exceeds the candidate report bound",
                    ));
                }
            }
            impacts.push(impact);
        }
        let change_values = changes
            .iter()
            .map(|change| {
                serde_json::from_str::<Value>(change.to_json())
                    .map_err(|_| invalid("invalid retained change"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let semantic_delta = wire::render(
            json!({"operations": summaries, "base_graph": base.semantic_graph_digest(), "candidate_graph": revision.semantic_graph_digest()}),
            MAX_SEMANTIC_CHANGE_BYTES,
        )?;
        let json = wire::render(
            json!({
                "schema": PROJECT_CANDIDATE_SCHEMA,
                "base_revision": base.project_revision(), "candidate_revision": revision.project_revision(),
                "base_graph_digest": base.semantic_graph_digest(), "candidate_graph_digest": revision.semantic_graph_digest(),
                "changes": change_values, "operations": summaries,
                "semantic_delta_digest": wire::digest(b"semaprax.candidate.semantic-delta.v1\0", semantic_delta.as_bytes()),
                "source_changes": source_changes, "impact": impacts,
                "validation": {"source_reparsed": true, "project_profile_admitted": true, "unresolved_holes": 0, "tests": "not_run"},
                "core_targets": {"base": base_targets.as_ref(), "candidate": targets},
                "requirements": SEMANTIC_CHANGE_REQUIREMENTS,
                "required_gates": ["affected_project_tests", "native_and_wasm_runtime_conformance", "full_quality_profile"],
                "nonclaims": ["no_commit_or_filesystem_authority", "not_external_consumer_compatibility", "not_behavioral_equivalence", "impact_uses_six_cross_file_edge_families", "no_target_or_test_execution", "no_typed_holes_or_semantic_merge_yet"],
            }),
            MAX_PROJECT_CANDIDATE_BYTES,
        )?;
        let digest = wire::digest(b"semaprax.project-candidate.v1\0", json.as_bytes());
        Ok(Self {
            base,
            revision,
            changes,
            summaries,
            base_targets,
            targets: targets.clone(),
            json,
            digest,
            source_review_cache: OnceLock::new(),
        })
    }
}

fn require_revision(revision: &ProjectRevision, expected: &str) -> Result<(), Vec<Diagnostic>> {
    wire::validate_digest(expected)?;
    if revision.project_revision() != expected {
        return Err(stale("semantic change base revision is stale"));
    }
    Ok(())
}

fn parse_revision(revision: &ProjectRevision) -> Result<Vec<Program>, Vec<Diagnostic>> {
    revision
        .sources()
        .iter()
        .map(|source| crate::parse(source.source(), source.path()).map_err(|d| vec![d]))
        .collect()
}

fn materialize(programs: &[Program]) -> Result<Vec<SemanticWorkspaceSource>, Vec<Diagnostic>> {
    let mut total = 0usize;
    let mut sources = Vec::new();
    for program in programs {
        let (source, overflow) = crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
            crate::format::canonical(program)
        });
        if overflow {
            return Err(capacity("candidate canonical source exceeds its bound"));
        }
        total = total
            .checked_add(source.len())
            .ok_or_else(|| capacity("candidate source size overflow"))?;
        if total > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity("candidate sources exceed the Project bound"));
        }
        let reparsed = crate::parse(&source, &program.path).map_err(|d| vec![d])?;
        let (roundtrip, overflow) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(&reparsed)
            });
        if overflow || roundtrip != source {
            return Err(stale(
                "candidate source is not an exact canonical round trip",
            ));
        }
        sources.push(SemanticWorkspaceSource {
            path: program.path.clone(),
            source,
        });
    }
    Ok(sources)
}

fn invariant_facts(programs: &[Program]) -> Value {
    let facts = programs.iter().map(|program| {
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        for ty in &program.types {
            if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &ty.kind { functions.extend(methods); }
        }
        let functions = functions.into_iter().map(|function| (function.stable_id.clone(), json!({"effects": function.effects, "requires": function.requires.len(), "ensures": function.ensures.len()}))).collect::<BTreeMap<_,_>>();
        (program.path.clone(), json!({"permits": program.permits, "functions": functions}))
    }).collect::<BTreeMap<_,_>>();
    json!(facts)
}

fn preserve_explicit_identities(
    base: &ProjectRevision,
    candidate: &ProjectRevision,
    addition: Option<&declaration::DeclarationAddition>,
    field: Option<&record_field::FieldAddition>,
    movement: Option<&movement::DeclarationMove>,
    type_addition: Option<&type_declaration::TypeAddition>,
) -> Result<(), Vec<Diagnostic>> {
    fn identities(revision: &ProjectRevision) -> Result<BTreeMap<String, Value>, Vec<Diagnostic>> {
        let graph: Value = serde_json::from_str(revision.semantic_graph())
            .map_err(|_| invalid("invalid retained graph"))?;
        Ok(graph["declarations"]
            .as_array()
            .ok_or_else(|| invalid("retained graph lacks declarations"))?
            .iter()
            .filter(|d| d["identity_origin"] == "explicit")
            .map(|d| (d["id"].as_str().unwrap_or_default().to_owned(), d.clone()))
            .collect())
    }
    let mut before = identities(base)?;
    let mut after = identities(candidate)?;
    if let Some(addition) = type_addition {
        for expected in &addition.expected_identities {
            let id = expected["id"]
                .as_str()
                .ok_or_else(|| invalid("planned type identity is invalid"))?;
            if before.contains_key(id) || after.remove(id).as_ref() != Some(expected) {
                return Err(invalid(
                    "candidate does not contain exactly the planned type and member identities",
                ));
            }
        }
    }
    if let Some(addition) = addition {
        let expected = json!({"id":addition.id,"kind":"function","identity_origin":"explicit","owner":null,"path":addition.path,"module":addition.module});
        if before.contains_key(&addition.id)
            || after.remove(&addition.id).as_ref() != Some(&expected)
        {
            return Err(invalid(
                "candidate does not contain exactly the planned added function identity",
            ));
        }
    }
    if let Some(field) = field {
        let expected = json!({"id":field.id,"kind":"field","identity_origin":"explicit","owner":field.owner,"path":field.path,"module":field.module});
        if before.contains_key(&field.id) || after.remove(&field.id).as_ref() != Some(&expected) {
            return Err(invalid(
                "candidate does not contain exactly the planned added field identity",
            ));
        }
    }
    if let Some(moved) = movement {
        let fact = before
            .get_mut(&moved.id)
            .ok_or_else(|| invalid("moved identity is absent from the original graph"))?;
        if fact["kind"] != "function"
            || !fact["owner"].is_null()
            || fact["path"] != moved.source_path
            || fact["module"] != moved.source_module
        {
            return Err(invalid(
                "moved identity does not match its exact original owner",
            ));
        }
        fact["path"] = json!(moved.destination_path);
        fact["module"] = json!(moved.destination_module);
    }
    if before != after {
        return Err(invalid(
            "candidate changed explicit declaration identities or ownership",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G222", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G223", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G224", message)]
}
