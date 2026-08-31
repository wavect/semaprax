//! Bounded, invocation-local Project v1 input authority.
//!
//! A project names every source explicitly. Loading authenticates those exact
//! files, runs the existing Semantic Workspace Phase-A build once in memory,
//! and retains held identities for a final caller-boundary recheck. It creates
//! no managed workspace and grants no publication authority.

mod admission;
mod authority;
mod build;
mod candidate;
mod execution;
mod flat_owned_record;
mod image;
mod image_coverage;
mod image_dependencies;
mod image_facets;
mod image_protocols;
mod image_store;
mod image_targets;
pub(crate) mod incremental;
mod manifest;
mod native_sdk;
mod npm;
mod prepared_interpreter;
mod profile;
mod public_api;
mod public_utf8_api;
#[cfg(test)]
mod publication_tests;
mod rename;
mod revision;
mod semantic;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;

pub use crate::wasm::{ProjectWebBuild, MAX_PROJECT_WEB_BUILD_BYTES, PROJECT_WEB_BUILD_SCHEMA};
use authority::{authentication, DeclaredPathSelection, HeldDirectory, HeldFile};
#[cfg(all(test, windows))]
use authority::{declared_absolute_path, has_declared_alias_component};
pub use candidate::{
    apply_candidate_git_publication, apply_candidate_publication, prepare_candidate_publication,
    CandidateGitAuthority, CandidateGitCommitMetadata, CandidateGitObject, CandidateGitObjectKind,
    CandidateGitProcessAuthority, CandidateGitRefUpdate, CandidateGitRepository,
    CandidateGitTarget, CandidateTestPolicy, CandidateTestReport, GitObjectFormat,
    ProjectCandidate, ProjectCandidateAttempt, ProjectCandidateAttemptOutcome,
    ProjectCandidateDraft, ProjectCandidatePublication, ProjectCandidateRebase, SemanticChange,
    MAX_CANDIDATE_TEST_STEPS, MAX_PROJECT_CANDIDATE_BYTES, MAX_PROJECT_CANDIDATE_HOLES,
    MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES, MAX_PROJECT_CANDIDATE_RECOVERY_BYTES,
    MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES, MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_BYTES,
    MAX_PROJECT_CANDIDATE_TEST_PLAN_BYTES, MAX_PROJECT_CANDIDATE_TEST_REPORT_BYTES,
    MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES, MAX_SEMANTIC_CHANGE_BYTES,
    PROJECT_CANDIDATE_ATTEMPT_SCHEMA, PROJECT_CANDIDATE_DRAFT_SCHEMA,
    PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA, PROJECT_CANDIDATE_HOLE_CONTEXT_SCHEMA,
    PROJECT_CANDIDATE_PUBLICATION_SCHEMA, PROJECT_CANDIDATE_REBASE_SCHEMA,
    PROJECT_CANDIDATE_RECOVERY_COMPATIBILITY, PROJECT_CANDIDATE_RECOVERY_SCHEMA,
    PROJECT_CANDIDATE_REPAIR_CATALOG_SCHEMA, PROJECT_CANDIDATE_SCHEMA,
    PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_SCHEMA, PROJECT_CANDIDATE_SEMANTIC_DELTA_SCHEMA,
    PROJECT_CANDIDATE_TEST_PLAN_SCHEMA, PROJECT_CANDIDATE_TEST_REPORT_SCHEMA,
    PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA, SEMANTIC_CHANGE_REQUIREMENTS, SEMANTIC_CHANGE_SCHEMA,
};
pub use candidate::{
    ProjectCandidateArchive, MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES, PROJECT_CANDIDATE_ARCHIVE_SCHEMA,
};
pub use candidate::{
    ProjectCandidateDraftArchive, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES,
    PROJECT_CANDIDATE_DRAFT_ARCHIVE_COMPATIBILITY, PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,
};
pub use candidate::{
    ProjectCandidateDraftMerge, MAX_PROJECT_CANDIDATE_DRAFT_MERGE_BYTES,
    PROJECT_CANDIDATE_DRAFT_MERGE_SCHEMA,
};
pub use candidate::{
    ProjectCandidateDraftRebase, MAX_PROJECT_CANDIDATE_DRAFT_REBASE_BYTES,
    PROJECT_CANDIDATE_DRAFT_REBASE_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_ARTIFACT_DELTA_BYTES, PROJECT_CANDIDATE_ARTIFACT_DELTA_SCHEMA,
    PROJECT_CANDIDATE_ARTIFACT_DELTA_VERIFICATION_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES,
    PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA,
    PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES, PROJECT_CANDIDATE_CONTRACT_DELTA_SCHEMA,
    PROJECT_CANDIDATE_CONTRACT_DELTA_VERIFICATION_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_DEPENDENCY_PAGE_BYTES, MAX_PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_BYTES,
    PROJECT_CANDIDATE_DEPENDENCY_PAGE_SCHEMA, PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES, PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY,
    PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES, PROJECT_CANDIDATE_INTERFACE_DELTA_SCHEMA,
    PROJECT_CANDIDATE_INTERFACE_DELTA_VERIFICATION_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES, PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA,
    PROJECT_CANDIDATE_MERGE_PREVIEW_VERIFICATION_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES, PROJECT_CANDIDATE_OWNERSHIP_DELTA_SCHEMA,
    PROJECT_CANDIDATE_OWNERSHIP_DELTA_VERIFICATION_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES, PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_HOLE_FILL_SUGGESTIONS_BYTES, PROJECT_HOLE_FILL_SUGGESTIONS_SCHEMA,
};
pub use candidate::{
    MAX_PROJECT_HOLE_NAVIGATION_BYTES, MAX_PROJECT_HOLE_NAVIGATION_ITEMS, PROJECT_HOLE_PAGE_SCHEMA,
    PROJECT_HOLE_SUMMARY_SCHEMA,
};
pub use execution::{
    verify_execution_envelope, ProjectExecution, ProjectExecutionOptions, ProjectExecutionOutcome,
    ProjectExecutionRole, PROJECT_EXECUTION_SCHEMA,
};
pub use flat_owned_record::{
    derive_flat_owned_record_api_descriptor, render_flat_owned_record_metadata,
    render_flat_owned_record_rust, render_flat_owned_record_typescript,
    replay_flat_owned_record_api_descriptor, replay_flat_owned_record_metadata,
    FlatOwnedRecordApiDescriptor, FlatOwnedRecordCarrierPlan, FlatOwnedRecordExport,
    FlatOwnedRecordField, FlatOwnedRecordFieldType, FlatOwnedRecordSettlement,
    FLAT_OWNED_RECORD_API_SCHEMA, FLAT_OWNED_RECORD_METADATA_SCHEMA,
    FLAT_OWNED_RECORD_NPM_BUILD_SCHEMA, FLAT_OWNED_RECORD_PROJECT_SCHEMA,
};
pub use image::{
    ProjectSemanticImage, MAX_SEMANTIC_IMAGE_BYTES, PROJECT_SEMANTIC_IMAGE_COMPATIBILITY,
    PROJECT_SEMANTIC_IMAGE_SCHEMA, PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA,
};
pub use image_coverage::{IMAGE_ANALYSIS_COVERAGE_SCHEMA, MAX_IMAGE_ANALYSIS_COVERAGE_BYTES};
pub use image_dependencies::{
    ImageDependencyPageOptions, ImageDependencyView, IMAGE_CLEANUP_DEPENDENCIES_SCHEMA,
    IMAGE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA, IMAGE_DECLARATION_DEPENDENCIES_SCHEMA,
    IMAGE_DEPENDENCY_PAGE_SCHEMA, IMAGE_DEPENDENCY_SUMMARY_SCHEMA,
    MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES, MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES,
};
pub use image_facets::{
    ImageFacet, ImageFacetOptions, IMAGE_FACET_SCHEMA, IMAGE_FUNCTION_INSTANCES_SCHEMA,
    IMAGE_FUNCTION_SUMMARY_SCHEMA, IMAGE_INSTANCE_FACET_SCHEMA,
};
pub use image_protocols::{
    IMAGE_PROTOCOL_CONFORMANCE_SCHEMA, MAX_IMAGE_PROTOCOL_CONFORMANCE_BYTES,
};
pub use image_store::{
    load_semantic_image, persist_semantic_image, ImageRefreshReport, ImageStoreReceipt,
    ImageWorkspace, MAX_IMAGE_REFRESH_REPORT_BYTES, MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES,
    SEMANTIC_IMAGE_REFRESH_SCHEMA, SEMANTIC_IMAGE_STORE_SCHEMA,
};
pub use image_targets::{
    ImageArtifactKind, IMAGE_ARTIFACT_PROJECTION_SCHEMA, IMAGE_TARGET_ADMISSION_SCHEMA,
    MAX_IMAGE_ARTIFACT_BUILD_BYTES, MAX_IMAGE_ARTIFACT_REPORT_BYTES,
};
pub use incremental::{
    ProjectFrontendBuild, ProjectFrontendCache, ProjectFrontendSource,
    MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND, MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET,
    MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES, MAX_PROJECT_FRONTEND_REPORT_BYTES,
    PROJECT_FRONTEND_CACHE_COMPATIBILITY, PROJECT_FRONTEND_CACHE_SCHEMA,
    PROJECT_SEMANTIC_CACHE_COMPATIBILITY, PROJECT_SEMANTIC_CACHE_SCHEMA,
};
use manifest::{capacity, grammar};
pub use manifest::{
    ProjectManifest, MAX_MANIFEST_BYTES, MAX_MODULE_BYTES, MAX_NAME_BYTES, MAX_PATH_BYTES,
    MAX_SOURCES, MAX_STABLE_ID_BYTES, MAX_TOTAL_SOURCE_BYTES, MAX_VERSION_BYTES, MAX_WEB_EXPORTS,
    PROJECT_SCHEMA, PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3, PROJECT_SCHEMA_V4,
    PROJECT_SCHEMA_V5, PROJECT_SCHEMA_V6, PROJECT_SCHEMA_V7, PROJECT_SCHEMA_V8, PROJECT_SCHEMA_V9,
};
pub use native_sdk::{
    with_native_owned_data_sdk_subject, ProjectNativeRustPackage, ProjectNativeRustPackageMode,
    ProjectNativeSdkExport, ProjectNativeSdkSubject, ProjectOwnedDataNativeSdkSubject,
};
pub use npm::{
    ProjectNpmBuild, ProjectNpmPublication, MAX_PROJECT_NPM_BUILD_BYTES, PROJECT_NPM_BUILD_SCHEMA,
    PROJECT_NPM_BUILD_SCHEMA_V2, PROJECT_NPM_BUILD_SCHEMA_V3, PROJECT_NPM_BUILD_SCHEMA_V4,
    PROJECT_NPM_BUILD_SCHEMA_V5, PROJECT_NPM_BUILD_SCHEMA_V6, PROJECT_NPM_BUILD_SCHEMA_V7,
    PROJECT_NPM_BUILD_SCHEMA_V8, PROJECT_NPM_BUILD_SCHEMA_V9,
};
pub use prepared_interpreter::{
    prepare_project_interpreter, verify_project_source_trace,
    verify_project_source_trace_against_revision, PreparedProjectExecution,
    PreparedProjectExecutionOptions, PreparedProjectInterpreter, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, ProjectPreparedExecutionOutcome, ProjectSourceTrace,
    ProjectSourceTraceEvent, DEFAULT_PROJECT_SOURCE_TRACE_BYTES,
    DEFAULT_PROJECT_SOURCE_TRACE_EVENTS, MAX_PROJECT_SOURCE_TRACE_BYTES,
    MAX_PROJECT_SOURCE_TRACE_EVENTS, MIN_PROJECT_SOURCE_TRACE_BYTES, PROJECT_SOURCE_TRACE_SCHEMA,
};

/// Prepare the additive WP-10/WP-11 owned-data package from held HIR and the
/// canonical descriptor. This does not activate a Project manifest profile.
pub fn prepare_owned_data_npm_build(
    program: &crate::hir::ResolvedProgram,
    descriptor: &PublicApiDescriptor,
    package: &str,
    version: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    npm::prepare_owned_data(program, descriptor, package, version, max_bytes)
}
pub use profile::{
    ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_COMMAND_ARGS_READ_CAPABILITY,
    PROJECT_COMMAND_INPUT_V1, PROJECT_COMMAND_STDERR_WRITE_CAPABILITY,
    PROJECT_COMMAND_STDIN_READ_CAPABILITY, PROJECT_COMMAND_STDOUT_CAPABILITY,
    PROJECT_LANGUAGE_COMMAND_INPUT_V1, PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1,
    PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1, PROJECT_PROFILE_LINE_COMMAND_IO_V1,
    PROJECT_PROFILE_OWNED_DATA_API_V1, PROJECT_PROFILE_OWNED_UTF8_API_V1,
    PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1, PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2,
    PROJECT_PROFILE_USEFUL_DATA_V1, PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1,
};
pub use public_api::{
    derive_public_api_descriptor, replay_public_api_descriptor, PublicApiDescriptor,
    PublicApiExport, PublicApiLimits, PublicApiParameter, PublicApiParameterType,
    PublicApiResultType, PublicApiSubject, MAX_PUBLIC_API_BORROWED_INPUT_BYTES,
    MAX_PUBLIC_API_CLOSURE_FUNCTIONS, MAX_PUBLIC_API_DESCRIPTOR_BYTES, MAX_PUBLIC_API_EXPORTS,
    MAX_PUBLIC_API_OWNED_OUTPUT_BYTES, MAX_PUBLIC_API_PARAMETERS, PUBLIC_OPTION_NONE_TAG,
    PUBLIC_OPTION_SOME_TAG, PUBLIC_OWNED_DATA_API_SCHEMA, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
    PUBLIC_RESULT_ERR_TAG, PUBLIC_RESULT_OK_TAG,
};
pub use public_utf8_api::{PUBLIC_OWNED_UTF8_API_SCHEMA, PUBLIC_OWNED_UTF8_PROJECT_SCHEMA};
pub(crate) use rename::{PreparedProjectRename, ProjectRenameDerivation};
pub use revision::ProjectRevision;
pub use semantic::{
    PROJECT_SEMANTIC_CONTEXT_SCHEMA, PROJECT_SEMANTIC_GRAPH_SCHEMA, PROJECT_SEMANTIC_IMPACT_SCHEMA,
};

#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
pub(crate) fn rebuild_owned_revision(
    manifest: ProjectManifest,
    sources: Vec<SemanticWorkspaceSource>,
) -> Result<ProjectRevision, Vec<Diagnostic>> {
    let built = build::build_owned(&manifest, sources)?;
    Ok(ProjectRevision::from_built(manifest, built))
}

const MANIFEST_FILE: &str = "semaprax.toml";
const MAX_HELD_DIRECTORIES: usize = 128;

/// One authenticated canonical source fact returned by the shared Workspace
/// Phase-A preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSource {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    source: String,
}

impl ProjectSource {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn source_graph_schema(&self) -> &str {
        &self.source_graph_schema
    }

    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

/// An invocation-local authenticated project snapshot.
pub struct ProjectSnapshot {
    root: PathBuf,
    revision: Arc<ProjectRevision>,
    declared_inputs: Vec<DeclaredPathSelection>,
    held_manifest: HeldFile,
    held_sources: Vec<HeldFile>,
    held_directories: Vec<HeldDirectory>,
    published_subject: Option<&'static str>,
    request_invalidation: Option<Vec<Diagnostic>>,
}

impl Deref for ProjectSnapshot {
    type Target = ProjectRevision;

    fn deref(&self) -> &Self::Target {
        &self.revision
    }
}

impl ProjectSnapshot {
    /// Consume one retained session snapshot after a final complete held-input
    /// recheck. Dropping the returned value releases every retained handle.
    pub(crate) fn finish_session(mut self) -> Result<(), Vec<Diagnostic>> {
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    // Keep the established ProjectSnapshot API as inherent methods. Deref
    // makes ordinary calls ergonomic, but it does not preserve UFCS or
    // function-item uses such as `ProjectSnapshot::check`.
    pub fn manifest(&self) -> &ProjectManifest {
        self.revision.manifest()
    }

    pub fn sources(&self) -> &[ProjectSource] {
        self.revision.sources()
    }

    pub fn workspace_manifest(&self) -> &str {
        self.revision.workspace_manifest()
    }

    pub fn workspace_revision(&self) -> &str {
        self.revision.workspace_revision()
    }

    pub fn project_revision(&self) -> &str {
        self.revision.project_revision()
    }

    pub fn entry_program(&self) -> &crate::hir::ResolvedProgram {
        self.revision.entry_program()
    }

    pub fn test_program(&self) -> &crate::hir::ResolvedProgram {
        self.revision.test_program()
    }

    pub fn semantic_graph(&self) -> &str {
        self.revision.semantic_graph()
    }

    pub fn semantic_context(
        &self,
        target_kind: crate::workspace_analysis::WorkspaceAnalysisTargetKind,
        target: &str,
        options: crate::workspace_analysis::WorkspaceContextOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.revision.semantic_context(target_kind, target, options)
    }

    pub fn semantic_impact(
        &self,
        target_kind: crate::workspace_analysis::WorkspaceAnalysisTargetKind,
        target: &str,
        options: crate::workspace_analysis::WorkspaceImpactOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.revision.semantic_impact(target_kind, target, options)
    }

    pub fn check(&self) -> Result<(), Vec<Diagnostic>> {
        self.revision.check()
    }

    pub fn execute_entry(
        &self,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        self.revision.execute_entry(options)
    }

    pub fn execute_test(
        &self,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        self.revision.execute_test(options)
    }

    pub fn execute(
        &self,
        role: ProjectExecutionRole,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        self.revision.execute(role, options)
    }

    pub fn build_web_inline(&self, max_bytes: usize) -> Result<ProjectWebBuild, Vec<Diagnostic>> {
        self.revision.build_web_inline(max_bytes)
    }

    pub fn build_npm_inline(&self, max_bytes: usize) -> Result<ProjectNpmBuild, Vec<Diagnostic>> {
        self.revision.build_npm_inline(max_bytes)
    }

    pub fn public_api_descriptor(&self) -> Result<PublicApiDescriptor, Vec<Diagnostic>> {
        self.revision.public_api_descriptor()
    }

    /// Derive and replay the exact Project v9 flat owned-record descriptor
    /// without granting target or publication authority.
    pub fn flat_owned_record_api_descriptor(
        &self,
    ) -> Result<FlatOwnedRecordApiDescriptor, Vec<Diagnostic>> {
        self.revision.flat_owned_record_api_descriptor()
    }

    /// Derive and replay the exact Project v10 owned UTF-8 descriptor without
    /// granting target or publication authority.
    pub fn owned_utf8_api_descriptor(&self) -> Result<PublicApiDescriptor, Vec<Diagnostic>> {
        self.revision.owned_utf8_api_descriptor()
    }

    pub fn test_wasm_module(&self) -> Result<Vec<u8>, Vec<Diagnostic>> {
        self.revision.test_wasm_module()
    }

    /// Retain the immutable, authority-neutral Project revision independently
    /// of this live authenticated snapshot and its held filesystem handles.
    pub fn retain_revision(&self) -> Arc<ProjectRevision> {
        Arc::clone(&self.revision)
    }

    /// Prepare one read-only stable-ID display rename over the complete
    /// authenticated Project without granting commit authority.
    pub(crate) fn prepare_rename(
        &self,
        target_id: &str,
        from: &str,
        to: &str,
    ) -> Result<PreparedProjectRename, Vec<Diagnostic>> {
        rename::prepare(self, target_id, from, to)
    }

    /// Reauthenticate immediately before and after one complete read-only
    /// request. Any observed drift permanently invalidates this snapshot so a
    /// later request cannot act on retained state.
    pub fn with_authenticated_request<T>(
        &mut self,
        operation: impl FnOnce(&ProjectSnapshot) -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        if let Some(invalidation) = &self.request_invalidation {
            return Err(invalidation.clone());
        }
        if let Err(drift) = self.recheck() {
            let invalidation = self.publication_uncertainty(drift);
            self.request_invalidation = Some(invalidation.clone());
            return Err(invalidation);
        }
        let result = operation(self);
        match self.recheck() {
            Ok(()) => result,
            Err(drift) => {
                let mut invalidation = self.publication_uncertainty(drift);
                self.request_invalidation = Some(invalidation.clone());
                match result {
                    Ok(_) => Err(invalidation),
                    Err(mut primary) => {
                        primary.append(&mut invalidation);
                        Err(primary)
                    }
                }
            }
        }
    }

    /// Build the authenticated project entry closure as its profile-selected
    /// Web product.
    pub fn build_web(&mut self, output: &Path) -> Result<(), Vec<Diagnostic>> {
        // Project v2-v7 each have one public JavaScript product: their exact
        // schema-selected npm/Web package. Keeping `web` and the default route as
        // aliases avoids a scalar-v1 fallback while `npm` remains the explicit
        // package-manager spelling. Frozen Project v1 bytes and publication
        // behavior stay on the scalar route below.
        if self.manifest.project_profile() != ProjectProfile::ScalarV1 {
            return self.build_npm(output);
        }
        let prepared = crate::wasm::prepare_project_web_with_scalar_exports(
            &self.entry_program,
            self.manifest.name(),
            &self.project_revision,
            &self.workspace_revision,
            self.semantic.graph_digest(),
            self.manifest.entry(),
            self.manifest.web_exports(),
        )
        .map_err(|error| vec![error])?;
        self.recheck()?;
        prepared.publish(output).map_err(|error| vec![error])?;
        self.published_subject = Some(WEB_PUBLICATION_SUBJECT);
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    /// Build and publish the exact installable schema-selected npm package.
    pub fn build_npm(&mut self, output: &Path) -> Result<(), Vec<Diagnostic>> {
        let prepared = npm::prepare(
            &self.manifest,
            &self.entry_program,
            &self.project_revision,
            &self.workspace_revision,
            self.semantic.graph_digest(),
            MAX_PROJECT_NPM_BUILD_BYTES,
        )
        .map_err(|error| vec![error])?;
        self.recheck()?;
        prepared.publish(output).map_err(|error| vec![error])?;
        self.published_subject = Some(NPM_PUBLICATION_SUBJECT);
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    /// Prepare an exact owned npm package for an explicitly supplied trusted
    /// publication host. The callback owns filesystem effects, not source or
    /// capsule authority. Errors do not authorize cleanup or promise rollback.
    pub fn build_owned_npm_with(
        &mut self,
        output: &Path,
        publish: impl FnOnce(&ProjectNpmPublication, &Path) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !self.manifest.is_v8() && !self.manifest.is_v9() && !self.manifest.is_v10() {
            return Err(vec![Diagnostic::io(
                "SPX-J114",
                "owned npm publication requires the exact Project v8, v9, or v10 profile",
            )]);
        }
        let prepared = npm::prepare(
            &self.manifest,
            &self.entry_program,
            &self.project_revision,
            &self.workspace_revision,
            self.semantic.graph_digest(),
            MAX_PROJECT_NPM_BUILD_BYTES,
        )
        .map_err(|error| vec![error])?;
        let plan = ProjectNpmPublication::prepare(&prepared, self.manifest.schema())
            .map_err(|error| vec![error])?;
        self.recheck()?;
        publish(&plan, output)?;
        self.published_subject = Some(NPM_PUBLICATION_SUBJECT);
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    /// Build the authenticated project entry closure as one native executable.
    ///
    /// The executable is compiled from exactly the linked entry HIR that Web
    /// publication and internal lowering-equivalence evidence consume. The
    /// destination must not exist, so publication never clobbers a file the
    /// caller did not create for this exact operation.
    pub fn build_native(&mut self, output: &Path) -> Result<(), Vec<Diagnostic>> {
        if self.manifest.project_profile() == ProjectProfile::OwnedDataApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-I308",
                "Project v8 owned-data-api.v1 does not admit native executable publication",
            )]);
        }
        if self.manifest.project_profile() == ProjectProfile::FlatOwnedRecordApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-B104",
                "Project v9 does not expose a native executable aggregate ABI",
            )]);
        }
        if self.manifest.project_profile() == ProjectProfile::OwnedUtf8ApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-I308",
                "Project v10 owned-utf8-api.v1 does not admit native executable publication",
            )]);
        }
        match std::fs::symlink_metadata(output) {
            Ok(_) => {
                return Err(vec![Diagnostic::io(
                    "SPX-I307",
                    format!(
                        "Project v1 native executable destination already exists: {}",
                        output.display()
                    ),
                )]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(vec![Diagnostic::io(
                    "SPX-I301",
                    format!(
                        "cannot inspect Project v1 native destination {}: {error}",
                        output.display()
                    ),
                )]);
            }
        }
        let profile = self.manifest.project_profile();
        let prepared = match profile {
            ProjectProfile::UsefulDataCommandV2 => crate::codegen::emit_hir_c_with_native_command(
                &self.entry_program,
                self.manifest.command().unwrap_or(""),
            ),
            ProjectProfile::LanguageCommandIoV1 => {
                crate::codegen::emit_hir_c_with_language_command_io(
                    &self.entry_program,
                    self.manifest.command().unwrap_or(""),
                )
            }
            ProjectProfile::LineCommandIoV1 => crate::codegen::emit_hir_c_with_line_command_io(
                &self.entry_program,
                self.manifest.command().unwrap_or(""),
            ),
            _ => crate::codegen::emit_hir_c(&self.entry_program),
        }
        .map_err(|error| vec![error])?;
        self.recheck()?;
        if matches!(
            profile,
            ProjectProfile::UsefulDataCommandV2
                | ProjectProfile::LanguageCommandIoV1
                | ProjectProfile::LineCommandIoV1
        ) {
            crate::codegen::compile_native_command_executable(&prepared, output)
        } else {
            crate::codegen::compile_native_executable(&prepared, output)
        }
        .map_err(|error| vec![error])?;
        self.published_subject = Some(NATIVE_PUBLICATION_SUBJECT);
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    fn publication_uncertainty(&self, mut drift: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let Some(subject) = self.published_subject else {
            return drift;
        };
        let mut diagnostics = vec![Diagnostic::io(
            "SPX-J103",
            format!("Project v1 inputs drifted after one complete {subject} was published"),
        )];
        diagnostics.append(&mut drift);
        diagnostics
    }

    fn recheck(&mut self) -> Result<(), Vec<Diagnostic>> {
        for declared in &self.declared_inputs {
            declared.recheck()?;
        }
        for directory in &self.held_directories {
            directory.recheck()?;
        }
        self.held_manifest.recheck()?;
        for source in &mut self.held_sources {
            source.recheck()?;
        }
        Ok(())
    }
}

/// Authenticate, resolve, and retain one project for exactly one caller
/// operation. A final held-object recheck runs regardless of operation result.
pub fn with_authenticated_project<T>(
    manifest_path: &Path,
    operation: impl FnOnce(&mut ProjectSnapshot) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let mut snapshot = load_snapshot(manifest_path)?;
    let result = operation(&mut snapshot);
    let recheck = snapshot
        .recheck()
        .map_err(|drift| snapshot.publication_uncertainty(drift));
    match (result, recheck) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(drift)) => Err(drift),
        (Err(primary), Ok(())) => Err(primary),
        (Err(mut primary), Err(mut drift)) => {
            if !primary
                .iter()
                .any(|diagnostic| matches!(diagnostic.code, "SPX-J102" | "SPX-J103"))
            {
                primary.append(&mut drift);
            }
            Err(primary)
        }
    }
}

/// Build, check, and execute the declared test closure from exact caller-owned
/// bytes. This path has no filesystem, handle, process, or publication
/// authority; it is intended for immutable rendered subjects that must be
/// admitted before any publication authority is acquired.
#[doc(hidden)]
pub fn validate_owned_project_test(
    manifest_source: &str,
    sources: &[(&str, &str)],
    options: &ProjectExecutionOptions,
) -> Result<ProjectExecution, Vec<Diagnostic>> {
    let manifest = ProjectManifest::parse(manifest_source)?;
    if sources.len() != manifest.sources().len()
        || sources
            .iter()
            .zip(manifest.sources())
            .any(|((actual, _), expected)| *actual != expected.as_str())
    {
        return Err(grammar(
            "owned Project source bytes must exactly match the ordered manifest inventory",
        ));
    }
    let mut total_source_bytes = 0usize;
    let mut workspace_sources = Vec::with_capacity(sources.len());
    for (path, source) in sources {
        total_source_bytes = total_source_bytes
            .checked_add(source.len())
            .ok_or_else(|| capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
        workspace_sources.push(SemanticWorkspaceSource {
            path: (*path).to_owned(),
            source: (*source).to_owned(),
        });
    }
    let built = build::build_owned(&manifest, workspace_sources)?;
    let revision = ProjectRevision::from_built(manifest, built);
    revision.check()?;
    revision.execute_test(options)
}

pub(crate) fn load_snapshot(manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
    load_snapshot_building(manifest_path, |manifest, sources| {
        let built = build::build_owned(&manifest, sources)?;
        Ok((Arc::new(ProjectRevision::from_built(manifest, built)), ()))
    })
    .map(|(snapshot, ())| snapshot)
}

/// The cache is staged by value: failed authentication or admission cannot alter
/// a caller's retained cache. No filesystem check is bypassed on a cache hit.
pub(crate) fn load_snapshot_with_frontend(
    manifest_path: &Path,
    mut cache: ProjectFrontendCache,
) -> Result<(ProjectSnapshot, ProjectFrontendCache, serde_json::Value), Vec<Diagnostic>> {
    let (snapshot, work) = load_snapshot_building(manifest_path, |manifest, sources| {
        let build = cache.build_authenticated_sources(&manifest, sources)?;
        let work = incremental::work_value(&build)?;
        Ok((build.into_revision(), work))
    })?;
    Ok((snapshot, cache, work))
}

fn load_snapshot_building<T>(
    manifest_path: &Path,
    build: impl FnOnce(
        ProjectManifest,
        Vec<SemanticWorkspaceSource>,
    ) -> Result<(Arc<ProjectRevision>, T), Vec<Diagnostic>>,
) -> Result<(ProjectSnapshot, T), Vec<Diagnostic>> {
    let manifest_selection = DeclaredPathSelection::open(manifest_path, "manifest")?;
    let manifest_path = manifest_selection.canonical_path.clone();
    if manifest_path.file_name().and_then(|name| name.to_str()) != Some(MANIFEST_FILE) {
        return Err(grammar("Project v1 manifest path must name semaprax.toml"));
    }
    let root = manifest_path
        .parent()
        .ok_or_else(|| grammar("Project v1 manifest must have an explicit project root"))?
        .to_path_buf();
    let mut root_ancestors = root.ancestors().map(Path::to_path_buf).collect::<Vec<_>>();
    if root_ancestors.len() > MAX_HELD_DIRECTORIES {
        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES));
    }
    root_ancestors.reverse();
    let mut held_directories = root_ancestors
        .iter()
        .cloned()
        .map(HeldDirectory::open)
        .collect::<Result<Vec<_>, _>>()?;
    let mut held_manifest = HeldFile::open(manifest_path.clone(), MAX_MANIFEST_BYTES)?;
    if held_manifest.identity != manifest_selection.identity {
        return Err(authentication(
            "Project v1 manifest selection changed while opening",
        ));
    }
    let manifest_text = held_manifest.utf8()?;
    let manifest = ProjectManifest::parse(&manifest_text)?;

    let mut held_sources = Vec::with_capacity(manifest.sources().len());
    let mut declared_inputs = vec![manifest_selection];
    let mut workspace_sources = Vec::with_capacity(manifest.sources().len());
    let mut seen_directories = root_ancestors.into_iter().collect::<BTreeSet<_>>();
    let mut total_source_bytes = 0usize;
    for relative in manifest.sources() {
        let relative_path = Path::new(relative);
        let mut ancestor = root.clone();
        if let Some(parent) = relative_path.parent() {
            for component in parent.components() {
                ancestor.push(component.as_os_str());
                if seen_directories.insert(ancestor.clone()) {
                    if seen_directories.len() > MAX_HELD_DIRECTORIES {
                        return Err(capacity("ancestor_directories", MAX_HELD_DIRECTORIES));
                    }
                    held_directories.push(HeldDirectory::open(ancestor.clone())?);
                }
            }
        }
        let selection = DeclaredPathSelection::open(&root.join(relative_path), "source")?;
        let path = selection.canonical_path.clone();
        // Each source is bounded by the *remaining* shared budget, not the
        // whole aggregate constant, so one large source cannot consume the
        // entire multi-file allowance before the total check fires.
        let remaining_source_bytes = MAX_TOTAL_SOURCE_BYTES - total_source_bytes;
        let mut held = HeldFile::open(path, remaining_source_bytes)?;
        if held.identity != selection.identity {
            return Err(authentication(format!(
                "Project v1 source {relative} selection changed while opening"
            )));
        }
        if held.identity == held_manifest.identity
            || held_sources
                .iter()
                .any(|existing: &HeldFile| existing.identity == held.identity)
        {
            return Err(authentication(
                "Project v1 source paths resolve to one physical file",
            ));
        }
        let source = held.utf8()?;
        total_source_bytes = total_source_bytes
            .checked_add(source.len())
            .ok_or_else(|| capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
        workspace_sources.push(SemanticWorkspaceSource {
            path: relative.clone(),
            source,
        });
        held_sources.push(held);
        declared_inputs.push(selection);
    }

    let (revision, result) = build(manifest, workspace_sources)?;
    let mut snapshot = ProjectSnapshot {
        root,
        revision,
        declared_inputs,
        held_manifest,
        held_sources,
        held_directories,
        published_subject: None,
        request_invalidation: None,
    };
    snapshot.recheck()?;
    Ok((snapshot, result))
}

const WEB_PUBLICATION_SUBJECT: &str = "digest-bound Web package";
const NPM_PUBLICATION_SUBJECT: &str = "installable npm package";
const NATIVE_PUBLICATION_SUBJECT: &str = "native executable";
const AUTHENTICATED_PROJECT_SUBJECT_OPERATION: &str = "authenticated Project subject operation";
