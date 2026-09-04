//! Public read-only Workspace Semantic Graph over an authenticated managed
//! workspace snapshot. The route holds the shared workspace lock through one
//! bounded resolver pass, canonical bounded wire rendering, final
//! authentication, and checked unlock; the fixed `workspace-graph` CLI prints
//! that exact API body plus one terminal LF.
//!
//! This module exposes no parser, verifier, raw-source constructor, write,
//! staging, ACTIVE-pivot, backend, or runtime authority.

#![allow(
    dead_code,
    reason = "sealed validation and test-only replay seams remain non-public"
)]

mod expected_projection;
mod operation_sidecar;
mod owned_generics;
mod package;
mod retained_validation;

use operation_sidecar::build_operation_sidecar;
pub(crate) use operation_sidecar::project_operation_sidecar;
#[cfg(test)]
use retained_validation::validate_effect_and_capability_edges;
use retained_validation::validate_retained_facts;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ast::{
    Expr, ExprKind, Function, ModuleUse, ModuleUseKind, ParamMode, Program, Span, Type,
    TypeDeclaration, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::{format, graph, hir, parse, prelude, workspace};

#[cfg(test)]
use expected_projection::dependency_depths;
use expected_projection::{
    collect_expected_edges, synthetic_builder_bytes, synthetic_program, validate_dependency_dag,
    verify_resolved_call_edges,
};

const MAX_FILES: usize = 16;
const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECLARATIONS: usize = 4096;
const MAX_CALLABLES: usize = 1024;
const MAX_CALLS: usize = 65_536;
const MAX_USES: usize = 4096;
const MAX_CROSS_FILE_EDGES: usize = 65_536;
const MAX_DEPENDENCY_DEPTH: usize = 16;
const MAX_BUILDER_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHANGE_BUILDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENTRY_MODULE_BYTES: usize = 16 * 1024 * 1024;
const WORKSPACE_GRAPH_SCHEMA: &str = "semaprax.workspace-semantic-graph.v1";
const WORKSPACE_MANIFEST_SCHEMA: &str = "semaprax.workspace-semantic-manifest.v1";
const ARTIFACT_DIGEST_DOMAIN: &[u8] = b"semaprax.workspace-semantic-graph.artifact-digest.v1\0";
const PROJECT_GRAPH_SCHEMA: &str = "semaprax.project-semantic-graph.v1";
const PROJECT_GRAPH_DIGEST_DOMAIN: &[u8] = b"semaprax.project-semantic-graph.artifact-digest.v1\0";
const PROJECT_GRAPH_NONCLAIMS: [&str; 9] = [
    "authenticated_declared_project_inputs_not_managed_workspace_state",
    "no_exclusive_lock_stage_publish_apply_or_commit_authority",
    "not_patch_evidence_signature_provenance_approval_or_reusable_authorization",
    "no_target_codegen_artifact_project_test_or_external_execution",
    "no_generic_resource_interface_ownership_or_lifetime_composition",
    "no_dynamic_linking_package_registry_network_or_dependency_fetch",
    "no_incremental_cache_persistence_or_repository_index",
    "no_create_delete_move_or_source_rewrite",
    "no_external_consumer_compatibility",
];
const NONCLAIMS: [&str; 18] = [
    "no_exclusive_lock_stage_publish_apply_or_commit_authority",
    "not_patch_evidence_signature_provenance_approval_or_reusable_authorization",
    "not_general_formal_proof_or_behavioral_equivalence_certificate",
    "no_target_codegen_artifact_project_test_or_external_execution",
    "no_cross_file_impact_review_context_repair_or_patch_generation",
    "no_cross_file_agent_context_embedding_or_search",
    "no_generic_cross_file_composition",
    "no_cross_file_resource_interface_ownership_borrowing_or_lifetime_composition",
    "no_reexport_wildcard_implicit_or_ambiguous_imports",
    "no_dynamic_linking_package_registry_network_or_dependency_fetch",
    "no_raw_working_tree_git_editor_or_unmanaged_file_analysis",
    "no_create_delete_move_or_flat_materialization",
    "no_incremental_cache_persistence_or_repository_index",
    "no_automatic_recovery_rollback_cleanup_or_gc",
    "no_power_loss_network_nfs_or_overlay_guarantee",
    "no_acl_xattr_ads_preservation",
    "no_new_patch_source_graph_cleanup_backend_or_runtime_semantics",
    "no_external_consumer_compatibility",
];
thread_local! {
    static ACTIVE_BUILDER_LIMIT: Cell<usize> = const { Cell::new(MAX_BUILDER_BYTES) };
}
// The resolver retains at most 48 owned copies of any source string across its
// AST clone, declaration/name/type indexes, resolved HIR, cleanup structures,
// and validation indexes. Fixed nodes are bounded by the structural bundles
// asserted below. Use 64 source-tree footprints to leave sixteen footprints
// for map/tree node bookkeeping. Generic materializations are charged as
// additional complete function trees before applying the factor.
const HIR_FIXED_EXPANSION_FACTOR: usize = 64;
// A declaration identity at one resolved occurrence can be present in the
// retained HIR, declaration/type/call indexes, validation sets, cleanup
// inventory, cleanup-plan projections, and transient resolver maps. The
// enumerated resolver paths use fewer than 48 copies; 64 also covers map keys
// retained concurrently during exact replay.
const HIR_IDENTITY_COPY_FACTOR: usize = 64;
const HIR_EXPR_FIXED_BUNDLE: usize = std::mem::size_of::<hir::ResolvedExpr>()
    + std::mem::size_of::<hir::ResolvedBinding>()
    + std::mem::size_of::<hir::ResolvedStatement>()
    + std::mem::size_of::<hir::ResolvedFieldInitializer>()
    + std::mem::size_of::<hir::ResolvedMatchArm>()
    + std::mem::size_of::<hir::ResolvedMatchPatternField>()
    + std::mem::size_of::<hir::ResolvedRecordMatchPatternField>()
    + std::mem::size_of::<crate::cleanup::CleanupStorageSlot>()
    + std::mem::size_of::<crate::cleanup::CleanupEntryState>()
    + std::mem::size_of::<crate::cleanup::CleanupFlag>()
    + std::mem::size_of::<crate::cleanup::CleanupPlace>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupSlot>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupEntryState>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupTransition>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupBlock>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupEdge>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupRegion>();
const HIR_FUNCTION_FIXED_BUNDLE: usize = std::mem::size_of::<hir::ResolvedFunction>()
    + std::mem::size_of::<hir::ResolvedFunctionTemplate>()
    + std::mem::size_of::<hir::ResolvedFunctionInstance>()
    + std::mem::size_of::<crate::cleanup::CleanupInventory>()
    + std::mem::size_of::<crate::cleanup_plan::CleanupPlan>();
const HIR_DECLARATION_FIXED_BUNDLE: usize = std::mem::size_of::<hir::Declaration>() * 12
    + std::mem::size_of::<hir::ResolvedTypeDeclaration>()
    + std::mem::size_of::<hir::ResolvedVariantCaseDeclaration>()
    + std::mem::size_of::<hir::ResolvedFieldDeclaration>()
    + std::mem::size_of::<hir::ResolvedInterface>()
    + std::mem::size_of::<hir::ResolvedImport>();
const _: () = assert!(
    HIR_EXPR_FIXED_BUNDLE <= HIR_FIXED_EXPANSION_FACTOR * std::mem::size_of::<crate::ast::Expr>()
);
const _: () = assert!(
    HIR_FUNCTION_FIXED_BUNDLE
        <= HIR_FIXED_EXPANSION_FACTOR * std::mem::size_of::<crate::ast::Function>()
);
const _: () = assert!(
    HIR_DECLARATION_FIXED_BUNDLE
        <= HIR_FIXED_EXPANSION_FACTOR * std::mem::size_of::<crate::ast::TypeDeclaration>()
);

#[derive(Clone)]
pub(crate) struct WorkspaceSource {
    pub(crate) path: String,
    pub(crate) source: String,
}

pub(crate) struct WorkspaceGraphBuild {
    hir: ValidatedWorkspaceHir,
    edges: Vec<WorkspaceEdge>,
    usage: WorkspaceGraphWorkUsage,
    change_fingerprints: Option<BTreeMap<String, String>>,
    change_builder_bytes: usize,
    operation_sidecar: Option<WorkspaceOperationSidecar>,
    operation_builder_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectGraphSourceFact {
    pub(crate) path: String,
    pub(crate) source_graph_schema: String,
    pub(crate) source_revision: String,
    pub(crate) source_digest: String,
}

pub(crate) struct ProjectSemanticParts {
    pub(crate) entry_program: hir::ResolvedProgram,
    pub(crate) test_program: hir::ResolvedProgram,
    pub(crate) projection: WorkspaceGraphProjection,
}

pub(crate) struct ProjectWebRoots<'a> {
    pub(crate) stable_ids: &'a [String],
    pub(crate) profile: crate::project::ProjectProfile,
}

pub(crate) struct ProjectSemanticGraphArtifact {
    json: String,
    digest: String,
}

impl ProjectSemanticGraphArtifact {
    pub(crate) fn json(&self) -> &str {
        &self.json
    }

    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }
}

pub(crate) struct WorkspaceGraphOperationView {
    pub(crate) graph: WorkspaceGraphChangeView,
    pub(crate) sidecar: WorkspaceOperationSidecar,
    pub(crate) builder_bytes: usize,
    pub(crate) change_builder_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOperationOccurrence {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) owner: Option<String>,
    pub(crate) shorthand_binding: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOperationDeclaration {
    pub(crate) path: String,
    pub(crate) module: String,
    pub(crate) id: String,
    pub(crate) kind: &'static str,
    pub(crate) explicit: bool,
    pub(crate) name: String,
    pub(crate) namespace_owner: Option<String>,
    pub(crate) span: Span,
    pub(crate) normalized_fingerprint: String,
    pub(crate) occurrences: Vec<WorkspaceOperationOccurrence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOperationImport {
    pub(crate) path: String,
    pub(crate) kind: &'static str,
    pub(crate) target_id: String,
    pub(crate) target_module: String,
    pub(crate) alias: String,
    pub(crate) occurrences: Vec<WorkspaceOperationOccurrence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOperationSidecar {
    pub(crate) declarations: Vec<WorkspaceOperationDeclaration>,
    pub(crate) imports: Vec<WorkspaceOperationImport>,
}

pub(crate) struct WorkspaceGraphChangeView {
    modules: Vec<WorkspaceGraphChangeModule>,
    declarations: Vec<WorkspaceGraphChangeDeclaration>,
    edges: Vec<WorkspaceEdge>,
    dependency_depths: BTreeMap<String, usize>,
    shared_prelude_ids: Vec<&'static str>,
    usage: WorkspaceGraphWorkUsage,
}

pub(crate) struct WorkspaceGraphChangeSourceFact {
    pub(crate) path: String,
    pub(crate) source_graph_schema: String,
    pub(crate) source_revision: String,
    pub(crate) source_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceGraphChangeModule {
    path: String,
    module: String,
    source_graph_schema: &'static str,
    permits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceGraphChangeDeclaration {
    id: String,
    kind: hir::DeclarationKind,
    origin: hir::IdentityOrigin,
    owner: Option<String>,
    path: Option<String>,
    module: Option<String>,
    semantic_fingerprint: String,
}

pub(crate) struct AuthenticatedWorkspaceGraphBuild {
    workspace_revision: String,
    sources: BTreeMap<String, AuthenticatedSourceFact>,
    storage: AuthenticatedWorkspaceStorageUsage,
    graph: WorkspaceGraphBuild,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthenticatedSourceFact {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedWorkspaceStorageUsage {
    manifest_bytes: usize,
    retained_generations: usize,
    staging_attempts: usize,
    unexpected_inventory_entries: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkspaceGraphWorkUsage {
    managed_files: usize,
    total_source_bytes: usize,
    declarations: usize,
    callables: usize,
    call_sites: usize,
    uses: usize,
    resolved_cross_file_edges: usize,
    dependency_depth: usize,
    builder_bytes: usize,
}

pub(crate) struct WorkspaceGraphProjection {
    workspace_revision: String,
    entry_module: String,
    modules: Vec<WorkspaceGraphProjectionModule>,
    declarations: Vec<WorkspaceGraphProjectionDeclaration>,
    edges: Vec<WorkspaceEdge>,
    shared_prelude_ids: Vec<&'static str>,
    usage: WorkspaceGraphProjectionUsage,
}

pub struct WorkspaceSemanticGraph {
    workspace_revision: String,
    graph_digest: String,
    entry: WorkspaceSemanticGraphEntry,
    modules: Vec<WorkspaceSemanticGraphModule>,
    declarations: Vec<WorkspaceSemanticGraphDeclaration>,
    edges: Vec<WorkspaceSemanticGraphEdge>,
    limits: WorkspaceSemanticGraphLimits,
    budget: WorkspaceSemanticGraphBudget,
    json: String,
}

pub struct WorkspaceSemanticGraphEntry {
    module: String,
    path: String,
}

pub struct WorkspaceSemanticGraphModule {
    path: String,
    module: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    dependency_depth: usize,
    permits: Vec<String>,
}

pub struct WorkspaceSemanticGraphDeclaration {
    id: String,
    kind: &'static str,
    identity_origin: &'static str,
    owner: Option<String>,
    path: Option<String>,
    module: Option<String>,
}

pub struct WorkspaceSemanticGraphEdge {
    caller_path: String,
    caller: String,
    target_path: String,
    target: String,
    kind: &'static str,
    site: &'static str,
    expression: String,
    ast_path: String,
    alias: String,
    ordinal: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceSemanticGraphLimits {
    max_managed_files: usize,
    max_reachable_modules: usize,
    max_entry_module_bytes: usize,
    max_total_source_bytes: usize,
    max_declarations: usize,
    max_callables: usize,
    max_call_sites: usize,
    max_uses: usize,
    max_resolved_cross_file_edges: usize,
    max_dependency_depth: usize,
    max_builder_bytes: usize,
    max_manifest_bytes: usize,
    max_output_bytes: usize,
    max_retained_generations: usize,
    max_staging_attempts: usize,
    max_unexpected_inventory_entries: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceSemanticGraphBudget {
    used_managed_files: usize,
    used_reachable_modules: usize,
    used_entry_module_bytes: usize,
    used_total_source_bytes: usize,
    used_declarations: usize,
    used_callables: usize,
    used_call_sites: usize,
    used_uses: usize,
    used_resolved_cross_file_edges: usize,
    used_dependency_depth: usize,
    used_builder_bytes: usize,
    used_manifest_bytes: usize,
    used_output_bytes: usize,
    used_retained_generations: usize,
    used_staging_attempts: usize,
    used_unexpected_inventory_entries: usize,
}

pub(crate) struct WorkspaceGraphProjectionModule {
    path: String,
    module: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    dependency_depth: usize,
    permits: Vec<String>,
    types: Vec<hir::ResolvedTypeDeclaration>,
    interfaces: Vec<hir::ResolvedInterface>,
    functions: Vec<hir::ResolvedFunction>,
    function_templates: Vec<hir::ResolvedFunctionTemplate>,
    function_instances: Vec<hir::ResolvedFunctionInstance>,
    signature_types: BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>,
}

pub(crate) struct WorkspaceGraphProjectionDeclaration {
    id: String,
    kind: hir::DeclarationKind,
    origin: hir::IdentityOrigin,
    owner: Option<String>,
    path: Option<String>,
    module: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceGraphProjectionUsage {
    used_managed_files: usize,
    used_total_source_bytes: usize,
    used_entry_module_bytes: usize,
    used_declarations: usize,
    used_callables: usize,
    used_call_sites: usize,
    used_uses: usize,
    used_resolved_cross_file_edges: usize,
    used_dependency_depth: usize,
    used_builder_bytes: usize,
    used_manifest_bytes: usize,
    used_output_bytes: usize,
    used_retained_generations: usize,
    used_staging_attempts: usize,
    used_unexpected_inventory_entries: usize,
    used_reachable_modules: usize,
}

struct ValidatedWorkspaceHir {
    modules: Vec<WorkspaceResolvedModule>,
    module_paths: BTreeMap<String, String>,
    dependency_depths: BTreeMap<String, usize>,
    declarations: BTreeMap<String, WorkspaceDeclarationFact>,
    shared_prelude_ids: BTreeSet<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceDeclarationFact {
    kind: hir::DeclarationKind,
    origin: hir::IdentityOrigin,
    owner: Option<String>,
    path: Option<String>,
    module: Option<String>,
}

struct WorkspaceResolvedModule {
    path: String,
    module: String,
    permits: Vec<String>,
    types: Vec<hir::ResolvedTypeDeclaration>,
    interfaces: Vec<hir::ResolvedInterface>,
    functions: Vec<hir::ResolvedFunction>,
    function_templates: Vec<hir::ResolvedFunctionTemplate>,
    function_instances: Vec<hir::ResolvedFunctionInstance>,
    signature_types: BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct WorkspaceEdge {
    caller_path: String,
    caller: String,
    target_path: String,
    target: String,
    kind: &'static str,
    site: &'static str,
    expression: String,
    ast_path: String,
    alias: String,
    ordinal: usize,
}

impl WorkspaceGraphProjection {
    pub(crate) fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub(crate) fn entry_module(&self) -> &str {
        &self.entry_module
    }

    pub(crate) fn modules(&self) -> &[WorkspaceGraphProjectionModule] {
        &self.modules
    }

    pub(crate) fn declarations(&self) -> &[WorkspaceGraphProjectionDeclaration] {
        &self.declarations
    }

    pub(crate) fn edges(&self) -> &[WorkspaceEdge] {
        &self.edges
    }

    pub(crate) fn shared_prelude_ids(&self) -> &[&'static str] {
        &self.shared_prelude_ids
    }

    pub(crate) fn usage(&self) -> WorkspaceGraphProjectionUsage {
        self.usage
    }

    #[cfg(test)]
    pub(crate) fn push_analysis_test_edge(
        &mut self,
        caller_path: String,
        caller: String,
        target_path: String,
        target: String,
        kind: &'static str,
    ) {
        self.edges.push(WorkspaceEdge {
            caller_path,
            caller,
            target_path,
            target,
            kind,
            site: "test",
            expression: "test".to_owned(),
            ast_path: "test".to_owned(),
            alias: String::new(),
            ordinal: self.edges.len(),
        });
    }
}

impl WorkspaceGraphProjectionModule {
    /// Compiler-checked facts for exact nominal parameters and returns, including functions
    /// outside the entry/test closures. Never reconstructed from source labels.
    pub(crate) fn signature_type_facts(
        &self,
        ty: &hir::ResolvedType,
    ) -> Option<&(hir::DeclarationKind, hir::TypeFacts)> {
        self.signature_types.get(&ty.identity_key())
    }
    /// Exact checked nominal value facts, including body/contract expressions,
    /// local bindings and nested patterns. This is the same bounded inventory
    /// as signature facts; a missing entry never implies Copy eligibility.
    pub(crate) fn value_type_facts(
        &self,
        ty: &hir::ResolvedType,
    ) -> Option<&(hir::DeclarationKind, hir::TypeFacts)> {
        self.signature_type_facts(ty)
    }
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn module(&self) -> &str {
        &self.module
    }

    pub(crate) fn source_graph_schema(&self) -> &str {
        &self.source_graph_schema
    }

    pub(crate) fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub(crate) fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub(crate) fn dependency_depth(&self) -> usize {
        self.dependency_depth
    }

    pub(crate) fn permits(&self) -> &[String] {
        &self.permits
    }

    pub(crate) fn types(&self) -> &[hir::ResolvedTypeDeclaration] {
        &self.types
    }

    pub(crate) fn interfaces(&self) -> &[hir::ResolvedInterface] {
        &self.interfaces
    }

    pub(crate) fn functions(&self) -> &[hir::ResolvedFunction] {
        &self.functions
    }

    pub(crate) fn function_templates(&self) -> &[hir::ResolvedFunctionTemplate] {
        &self.function_templates
    }

    pub(crate) fn function_instances(&self) -> &[hir::ResolvedFunctionInstance] {
        &self.function_instances
    }
}

impl WorkspaceGraphProjectionDeclaration {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn kind(&self) -> hir::DeclarationKind {
        self.kind
    }

    pub(crate) fn origin(&self) -> hir::IdentityOrigin {
        self.origin
    }

    pub(crate) fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub(crate) fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }
}

impl WorkspaceGraphProjectionUsage {
    pub(crate) fn used_managed_files(self) -> usize {
        self.used_managed_files
    }

    pub(crate) fn used_total_source_bytes(self) -> usize {
        self.used_total_source_bytes
    }

    pub(crate) fn used_entry_module_bytes(self) -> usize {
        self.used_entry_module_bytes
    }

    pub(crate) fn used_declarations(self) -> usize {
        self.used_declarations
    }

    pub(crate) fn used_callables(self) -> usize {
        self.used_callables
    }

    pub(crate) fn used_call_sites(self) -> usize {
        self.used_call_sites
    }

    pub(crate) fn used_uses(self) -> usize {
        self.used_uses
    }

    pub(crate) fn used_resolved_cross_file_edges(self) -> usize {
        self.used_resolved_cross_file_edges
    }

    pub(crate) fn used_dependency_depth(self) -> usize {
        self.used_dependency_depth
    }

    pub(crate) fn used_builder_bytes(self) -> usize {
        self.used_builder_bytes
    }

    pub(crate) fn used_manifest_bytes(self) -> usize {
        self.used_manifest_bytes
    }

    pub(crate) fn used_output_bytes(self) -> usize {
        self.used_output_bytes
    }

    pub(crate) fn used_retained_generations(self) -> usize {
        self.used_retained_generations
    }

    pub(crate) fn used_staging_attempts(self) -> usize {
        self.used_staging_attempts
    }

    pub(crate) fn used_unexpected_inventory_entries(self) -> usize {
        self.used_unexpected_inventory_entries
    }

    pub(crate) fn used_reachable_modules(self) -> usize {
        self.used_reachable_modules
    }
}

impl WorkspaceSemanticGraph {
    pub fn schema(&self) -> &str {
        WORKSPACE_GRAPH_SCHEMA
    }

    pub fn workspace_manifest_schema(&self) -> &str {
        WORKSPACE_MANIFEST_SCHEMA
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn graph_digest(&self) -> &str {
        &self.graph_digest
    }

    pub fn entry(&self) -> &WorkspaceSemanticGraphEntry {
        &self.entry
    }

    pub fn modules(&self) -> &[WorkspaceSemanticGraphModule] {
        &self.modules
    }

    pub fn declarations(&self) -> &[WorkspaceSemanticGraphDeclaration] {
        &self.declarations
    }

    pub fn edges(&self) -> &[WorkspaceSemanticGraphEdge] {
        &self.edges
    }

    pub fn limits(&self) -> WorkspaceSemanticGraphLimits {
        self.limits
    }

    pub fn budget(&self) -> WorkspaceSemanticGraphBudget {
        self.budget
    }

    pub fn nonclaims(&self) -> &'static [&'static str] {
        &NONCLAIMS
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
}

impl WorkspaceSemanticGraphEntry {
    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl WorkspaceSemanticGraphModule {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn module(&self) -> &str {
        &self.module
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
    pub fn dependency_depth(&self) -> usize {
        self.dependency_depth
    }
    pub fn permits(&self) -> &[String] {
        &self.permits
    }
}

impl WorkspaceSemanticGraphDeclaration {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn kind(&self) -> &str {
        self.kind
    }
    pub fn identity_origin(&self) -> &str {
        self.identity_origin
    }
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
    pub fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }
}

impl WorkspaceSemanticGraphEdge {
    pub fn caller_path(&self) -> &str {
        &self.caller_path
    }
    pub fn caller(&self) -> &str {
        &self.caller
    }
    pub fn target_path(&self) -> &str {
        &self.target_path
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn kind(&self) -> &str {
        self.kind
    }
    pub fn site(&self) -> &str {
        self.site
    }
    pub fn expression(&self) -> &str {
        &self.expression
    }
    pub fn ast_path(&self) -> &str {
        &self.ast_path
    }
    pub fn alias(&self) -> &str {
        &self.alias
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
}

impl WorkspaceSemanticGraphLimits {
    pub fn max_managed_files(self) -> usize {
        self.max_managed_files
    }
    pub fn max_reachable_modules(self) -> usize {
        self.max_reachable_modules
    }
    pub fn max_entry_module_bytes(self) -> usize {
        self.max_entry_module_bytes
    }
    pub fn max_total_source_bytes(self) -> usize {
        self.max_total_source_bytes
    }
    pub fn max_declarations(self) -> usize {
        self.max_declarations
    }
    pub fn max_callables(self) -> usize {
        self.max_callables
    }
    pub fn max_call_sites(self) -> usize {
        self.max_call_sites
    }
    pub fn max_uses(self) -> usize {
        self.max_uses
    }
    pub fn max_resolved_cross_file_edges(self) -> usize {
        self.max_resolved_cross_file_edges
    }
    pub fn max_dependency_depth(self) -> usize {
        self.max_dependency_depth
    }
    pub fn max_builder_bytes(self) -> usize {
        self.max_builder_bytes
    }
    pub fn max_manifest_bytes(self) -> usize {
        self.max_manifest_bytes
    }
    pub fn max_output_bytes(self) -> usize {
        self.max_output_bytes
    }
    pub fn max_retained_generations(self) -> usize {
        self.max_retained_generations
    }
    pub fn max_staging_attempts(self) -> usize {
        self.max_staging_attempts
    }
    pub fn max_unexpected_inventory_entries(self) -> usize {
        self.max_unexpected_inventory_entries
    }
}

impl WorkspaceSemanticGraphBudget {
    pub fn used_managed_files(self) -> usize {
        self.used_managed_files
    }
    pub fn used_reachable_modules(self) -> usize {
        self.used_reachable_modules
    }
    pub fn used_entry_module_bytes(self) -> usize {
        self.used_entry_module_bytes
    }
    pub fn used_total_source_bytes(self) -> usize {
        self.used_total_source_bytes
    }
    pub fn used_declarations(self) -> usize {
        self.used_declarations
    }
    pub fn used_callables(self) -> usize {
        self.used_callables
    }
    pub fn used_call_sites(self) -> usize {
        self.used_call_sites
    }
    pub fn used_uses(self) -> usize {
        self.used_uses
    }
    pub fn used_resolved_cross_file_edges(self) -> usize {
        self.used_resolved_cross_file_edges
    }
    pub fn used_dependency_depth(self) -> usize {
        self.used_dependency_depth
    }
    pub fn used_builder_bytes(self) -> usize {
        self.used_builder_bytes
    }
    pub fn used_manifest_bytes(self) -> usize {
        self.used_manifest_bytes
    }
    pub fn used_output_bytes(self) -> usize {
        self.used_output_bytes
    }
    pub fn used_retained_generations(self) -> usize {
        self.used_retained_generations
    }
    pub fn used_staging_attempts(self) -> usize {
        self.used_staging_attempts
    }
    pub fn used_unexpected_inventory_entries(self) -> usize {
        self.used_unexpected_inventory_entries
    }
}

impl WorkspaceEdge {
    pub(crate) fn caller_path(&self) -> &str {
        &self.caller_path
    }

    pub(crate) fn caller(&self) -> &str {
        &self.caller
    }

    pub(crate) fn target_path(&self) -> &str {
        &self.target_path
    }

    pub(crate) fn target(&self) -> &str {
        &self.target
    }

    pub(crate) fn kind(&self) -> &str {
        self.kind
    }

    pub(crate) fn site(&self) -> &str {
        self.site
    }

    pub(crate) fn expression(&self) -> &str {
        &self.expression
    }

    pub(crate) fn ast_path(&self) -> &str {
        &self.ast_path
    }

    pub(crate) fn alias(&self) -> &str {
        &self.alias
    }

    pub(crate) fn ordinal(&self) -> usize {
        self.ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CallOccurrenceKey<'a> {
    caller_path: &'a str,
    caller: &'a str,
    target_path: &'a str,
    site: &'a str,
    expression: &'a str,
    ast_path: &'a str,
    alias: &'a str,
    ordinal: usize,
}

impl<'a> CallOccurrenceKey<'a> {
    fn from_edge(edge: &'a WorkspaceEdge) -> Self {
        Self {
            caller_path: &edge.caller_path,
            caller: &edge.caller,
            target_path: &edge.target_path,
            site: edge.site,
            expression: &edge.expression,
            ast_path: &edge.ast_path,
            alias: &edge.alias,
            ordinal: edge.ordinal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoredKind {
    Function,
    Type,
    Protocol,
    Other,
}

struct AuthoredDeclaration<'a> {
    path: &'a str,
    module: &'a str,
    explicit: bool,
    kind: AuthoredKind,
    function: Option<&'a Function>,
    ty: Option<&'a TypeDeclaration>,
}

pub(crate) fn build_owned(
    sources: Vec<WorkspaceSource>,
) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    build_owned_with_builder_limit(sources, MAX_BUILDER_BYTES)
}

pub(crate) use package::{
    build_package_scalar_sources, PackageWorkspaceImport, PackageWorkspaceModule,
};

impl WorkspaceGraphBuild {
    pub(crate) fn contains_module(&self, module: &str) -> bool {
        self.hir.module_paths.contains_key(module)
    }

    /// Every checked function of this validated workspace, in module order. A
    /// linked program keeps only its own call closure, so a reader of a
    /// declaration outside it reads this rather than source text.
    pub(crate) fn validated_functions(&self) -> impl Iterator<Item = &hir::ResolvedFunction> {
        let modules = self.hir.modules.iter();
        modules.flat_map(|module| module.functions.iter())
    }

    /// Consume one validated workspace build into the entry module's complete
    /// provider closure and link its real scalar function bodies. This is a
    /// private backend-preparation seam, not a new Workspace authority or a
    /// general cross-file composition surface.
    pub(crate) fn linked_scalar_program(
        &self,
        entry_module: &str,
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        self.linked_project_program(entry_module, crate::project::ProjectProfile::ScalarV1)
    }

    /// Link an authenticated package's exact public scalar roots without
    /// inheriting the Project profile's display-name requirement for `main`.
    /// The byte-lowest selected `fn() -> i64` root is only the internal HIR
    /// anchor; every selected root and its transitive callees is retained.
    pub(crate) fn linked_package_scalar_exports(
        &self,
        root_module: &str,
        export_ids: &[String],
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        validate_entry_module(root_module)?;
        if export_ids.is_empty() {
            return Err(vec![graph_error(
                "SPX-PS504",
                "package-source root export inventory is empty",
            )]);
        }
        if export_ids
            .windows(2)
            .any(|pair| pair[0].as_bytes() >= pair[1].as_bytes())
        {
            return Err(vec![graph_error(
                "SPX-PS503",
                "package-source root exports are not strictly byte-sorted and unique",
            )]);
        }

        let mut available = BTreeMap::<hir::DeclarationId, hir::LinkedScalarFunction>::new();
        for module in &self.hir.modules {
            for function in &module.functions {
                let Some(fact) = self.hir.declarations.get(function.id.as_str()) else {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "package-source function is absent from declaration facts",
                    )]);
                };
                if fact.kind != hir::DeclarationKind::Function
                    || fact.path.as_deref() != Some(module.path.as_str())
                    || fact.module.as_deref() != Some(module.module.as_str())
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "package-source function declaration facts disagree with retained body",
                    )]);
                }
                if available
                    .insert(
                        function.id.clone(),
                        hir::LinkedScalarFunction {
                            function: function.clone(),
                            origin: fact.origin,
                        },
                    )
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "package-source function identity is duplicated",
                    )]);
                }
            }
        }

        let mut anchor = None;
        let mut pending = BTreeSet::new();
        for export in export_ids {
            let id = hir::DeclarationId::new(export.clone());
            let Some(linked) = available.get(&id) else {
                return Err(vec![graph_error(
                    "SPX-PS503",
                    "package-source root export is absent from retained HIR",
                )]);
            };
            let fact = self
                .hir
                .declarations
                .get(linked.function.id.as_str())
                .expect("available functions have declaration facts");
            if fact.module.as_deref() != Some(root_module)
                || fact.origin != hir::IdentityOrigin::Explicit
            {
                return Err(vec![graph_error(
                    "SPX-PS503",
                    "package-source export is not an explicit root-owned function",
                )]);
            }
            if anchor.is_none()
                && linked.function.params.is_empty()
                && linked.function.return_type == hir::ResolvedType::I64
            {
                anchor = Some(id.clone());
            }
            pending.insert(id);
        }
        let anchor = anchor.ok_or_else(|| {
            vec![graph_error(
                "SPX-PS504",
                "package-source exports require a byte-lowest fn() -> i64 HIR anchor",
            )]
        })?;

        let mut retained = BTreeSet::new();
        while let Some(function_id) = pending.pop_first() {
            let linked = available.get(&function_id).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "package-source call closure names an unauthenticated function",
                )]
            })?;
            if !retained.insert(function_id) {
                continue;
            }
            for callee in resolved_function_callees(&linked.function) {
                if available.contains_key(&callee) && !retained.contains(&callee) {
                    pending.insert(callee);
                }
            }
        }
        let functions = retained
            .into_iter()
            .map(|id| {
                available
                    .get(&id)
                    .map(|linked| hir::LinkedScalarFunction {
                        function: linked.function.clone(),
                        origin: linked.origin,
                    })
                    .ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "package-source retained closure lost an authenticated function",
                        )]
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        hir::link_package_scalar_workspace(root_module.to_owned(), anchor, functions)
            .map_err(|error| vec![error])
    }

    fn linked_project_program(
        &self,
        entry_module: &str,
        profile: crate::project::ProjectProfile,
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        if profile.is_owned_api() {
            return self.linked_owned_data_api_program_with_roots(entry_module, &[]);
        }
        validate_entry_module(entry_module)?;
        let Some(entry_path) = self.hir.module_paths.get(entry_module).cloned() else {
            return Err(vec![graph_error(
                "SPX-G172",
                format!("Workspace Semantic Graph entry module `{entry_module}` is absent"),
            )]);
        };

        let authenticated_paths = self
            .hir
            .module_paths
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut direct_providers = BTreeMap::<String, BTreeSet<String>>::new();
        for edge in &self.edges {
            if !matches!(edge.kind, "function_import" | "type_import") {
                continue;
            }
            if !authenticated_paths.contains(edge.caller_path.as_str())
                || !authenticated_paths.contains(edge.target_path.as_str())
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace import edge paths disagree with authenticated modules",
                )]);
            }
            direct_providers
                .entry(edge.caller_path.clone())
                .or_default()
                .insert(edge.target_path.clone());
        }
        let mut reachable_paths = BTreeSet::from([entry_path.clone()]);
        let mut pending = BTreeSet::from([entry_path]);
        while let Some(path) = pending.pop_first() {
            if let Some(providers) = direct_providers.get(&path) {
                for provider in providers {
                    if reachable_paths.insert(provider.clone()) {
                        pending.insert(provider.clone());
                    }
                }
            }
        }

        if self.edges.iter().any(|edge| {
            reachable_paths.contains(edge.caller_path.as_str()) && edge.kind == "type_import"
        }) {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace scalar linker does not admit `use type` imports",
            )]);
        }

        // Native Rust callbacks are the only interface the scalar Project
        // profile retains, and their declared effects are the only authority a
        // retained module or function may carry.
        let natives = retained_validation::scalar_native_imports(
            profile,
            self.hir
                .modules
                .iter()
                .filter(|module| reachable_paths.contains(module.path.as_str()))
                .flat_map(|module| &module.interfaces),
        );
        let mut functions = Vec::new();
        let mut entrypoints = Vec::new();
        let mut retained_modules = 0usize;
        for module in &self.hir.modules {
            if !reachable_paths.contains(module.path.as_str()) {
                continue;
            }
            retained_modules += 1;
            let permits_admitted =
                retained_validation::permits_admitted(profile, module, entry_module, &natives);
            let project_shape_admitted = profile.is_owned_api()
                || matches!(profile, crate::project::ProjectProfile::ScalarV1)
                || (module.types.is_empty()
                    && module.interfaces.is_empty()
                    && module.function_templates.is_empty()
                    && module.function_instances.is_empty());
            if !permits_admitted || !project_shape_admitted {
                return Err(vec![graph_error(
                    "SPX-G172",
                    format!(
                        "workspace module `{}` is outside the pure scalar linker profile",
                        module.module
                    ),
                )]);
            }
            for function in &module.functions {
                if profile == crate::project::ProjectProfile::ScalarV1
                    && module.module != entry_module
                    && function.name == "main"
                {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "workspace scalar provider modules may not declare `main`",
                    )]);
                }
                let Some(fact) = self.hir.declarations.get(function.id.as_str()) else {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace scalar function is absent from declaration facts",
                    )]);
                };
                if fact.kind != hir::DeclarationKind::Function
                    || fact.path.as_deref() != Some(module.path.as_str())
                    || fact.module.as_deref() != Some(module.module.as_str())
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace scalar function declaration facts disagree with retained body",
                    )]);
                }
                if module.module == entry_module && function.name == "main" {
                    if !function.params.is_empty() || function.return_type != hir::ResolvedType::I64
                    {
                        return Err(vec![graph_error(
                            "SPX-G172",
                            "workspace scalar entry module `main` must have the exact signature fn main() -> i64",
                        )]);
                    }
                    entrypoints.push((function.id.clone(), fact.origin));
                }
                // Project v6 has two deliberately distinct roots: the ordinary
                // pure `main` closure and the manifest-selected effectful
                // command closure. Build the former without effectful authored
                // functions; `linked_scalar_program_with_roots` independently
                // adds the exact stable-ID command and its callees before using
                // the language-command linker.
                if !matches!(
                    profile,
                    crate::project::ProjectProfile::LanguageCommandIoV1
                        | crate::project::ProjectProfile::LineCommandIoV1
                ) || function.effects.is_empty()
                {
                    functions.push(hir::LinkedScalarFunction {
                        function: function.clone(),
                        origin: fact.origin,
                    });
                }
            }
        }
        if retained_modules != reachable_paths.len() {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace scalar provider closure disagrees with retained modules",
            )]);
        }
        if entrypoints.len() != 1 {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace scalar entry module must declare exactly one authored `main` function",
            )]);
        }
        let (entrypoint, origin) = entrypoints.pop().expect("length checked above");
        if origin != hir::IdentityOrigin::Explicit {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace scalar entry module `main` must have an explicit identity",
            )]);
        }
        match profile {
            crate::project::ProjectProfile::ScalarV1 => natives.link(
                entry_module.to_owned(),
                entrypoint,
                functions,
                &self.hir.declarations,
            ),
            crate::project::ProjectProfile::UsefulTextConsumerV1 => {
                hir::link_useful_text_workspace(entry_module.to_owned(), entrypoint, functions)
            }
            crate::project::ProjectProfile::UsefulDataV1 => {
                hir::link_useful_data_workspace(entry_module.to_owned(), entrypoint, functions)
            }
            crate::project::ProjectProfile::UsefulDataCommandV1 => {
                hir::link_useful_data_command_workspace(
                    entry_module.to_owned(),
                    entrypoint,
                    functions,
                )
            }
            crate::project::ProjectProfile::UsefulDataCommandV2 => {
                hir::link_useful_data_command_workspace(
                    entry_module.to_owned(),
                    entrypoint,
                    functions,
                )
            }
            crate::project::ProjectProfile::LanguageCommandIoV1 => {
                // The ordinary project/test entry remains a pure useful-data
                // closure. `linked_scalar_program_with_roots` below retains
                // the selected command as a distinct authenticated root.
                hir::link_useful_data_workspace(entry_module.to_owned(), entrypoint, functions)
            }
            crate::project::ProjectProfile::LineCommandIoV1 => {
                hir::link_useful_data_workspace(entry_module.to_owned(), entrypoint, functions)
            }
            crate::project::ProjectProfile::OwnedDataApiV1 => {
                unreachable!("Project v8 uses the exact function-reachable linker")
            }
            crate::project::ProjectProfile::FlatOwnedRecordApiV1 => {
                unreachable!("Project v9 uses the exact aggregate-aware linker")
            }
            crate::project::ProjectProfile::OwnedUtf8ApiV1 => {
                unreachable!("Project v10 uses the exact function-reachable linker")
            }
            crate::project::ProjectProfile::NestedOwnedRecordApiV1 => {
                unreachable!("Project v11 uses the exact aggregate-aware linker")
            }
        }
        .map_err(|error| vec![error])
    }

    /// Link the ordinary entry-provider closure plus exact persistent
    /// function identities selected as additional roots. This is the Project
    /// Web planning seam: a selected export need not be called by `main`, but
    /// unrelated unselected functions outside the entry closure remain out of
    /// the backend HIR.
    pub(crate) fn linked_scalar_program_with_roots(
        &self,
        entry_module: &str,
        additional_roots: &[String],
        profile: crate::project::ProjectProfile,
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        if profile.is_owned_api() {
            return self.linked_owned_data_api_program_with_roots(entry_module, additional_roots);
        }
        let base = self.linked_project_program(entry_module, profile)?;
        if additional_roots.is_empty() {
            return Ok(base);
        }
        let natives = retained_validation::scalar_native_imports(profile, base.interfaces.iter());

        let mut available = BTreeMap::<hir::DeclarationId, hir::LinkedScalarFunction>::new();
        for module in &self.hir.modules {
            for function in &module.functions {
                let Some(fact) = self.hir.declarations.get(function.id.as_str()) else {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace scalar function is absent from declaration facts",
                    )]);
                };
                if fact.kind != hir::DeclarationKind::Function
                    || fact.path.as_deref() != Some(module.path.as_str())
                    || fact.module.as_deref() != Some(module.module.as_str())
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace scalar function declaration facts disagree with retained body",
                    )]);
                }
                if available
                    .insert(
                        function.id.clone(),
                        hir::LinkedScalarFunction {
                            function: function.clone(),
                            origin: fact.origin,
                        },
                    )
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace scalar function identity is duplicated",
                    )]);
                }
            }
        }

        let mut retained = base
            .functions
            .iter()
            .map(|function| function.id.clone())
            .collect::<BTreeSet<_>>();
        let mut pending = additional_roots
            .iter()
            .map(|root| hir::DeclarationId::new(root.clone()))
            .collect::<BTreeSet<_>>();
        while let Some(function_id) = pending.pop_first() {
            let Some(linked) = available.get(&function_id) else {
                return Err(vec![Diagnostic::io(
                    "SPX-W115",
                    format!(
                        "selected Project Web export identity `{function_id}` does not name an authenticated function"
                    ),
                )]);
            };
            if !retained.insert(function_id.clone()) {
                continue;
            }
            for callee in resolved_function_callees(&linked.function) {
                if available.contains_key(&callee) && !retained.contains(&callee) {
                    pending.insert(callee);
                }
            }
        }

        let functions = retained
            .into_iter()
            .map(|id| {
                available
                    .get(&id)
                    .map(|linked| hir::LinkedScalarFunction {
                        function: linked.function.clone(),
                        origin: linked.origin,
                    })
                    .ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "entry-provider closure names an unauthenticated function",
                        )]
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        match profile {
            crate::project::ProjectProfile::ScalarV1 => natives.link(
                base.module,
                base.entrypoint,
                functions,
                &self.hir.declarations,
            ),
            crate::project::ProjectProfile::UsefulTextConsumerV1 => {
                hir::link_useful_text_workspace(base.module, base.entrypoint, functions)
            }
            crate::project::ProjectProfile::UsefulDataV1 => {
                hir::link_useful_data_workspace(base.module, base.entrypoint, functions)
            }
            crate::project::ProjectProfile::UsefulDataCommandV1 => {
                hir::link_useful_data_command_workspace(base.module, base.entrypoint, functions)
            }
            crate::project::ProjectProfile::UsefulDataCommandV2 => {
                hir::link_useful_data_command_workspace(base.module, base.entrypoint, functions)
            }
            crate::project::ProjectProfile::LanguageCommandIoV1 => {
                let [command_id] = additional_roots else {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "Language Command I/O v1 must select exactly one command identity",
                    )]);
                };
                hir::link_language_command_io_workspace(
                    base.module,
                    base.entrypoint,
                    hir::DeclarationId::new(command_id.clone()),
                    functions,
                )
            }
            crate::project::ProjectProfile::LineCommandIoV1 => {
                let [command_id] = additional_roots else {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "Line Command I/O v1 must select exactly one command identity",
                    )]);
                };
                hir::link_line_command_io_workspace(
                    base.module,
                    base.entrypoint,
                    hir::DeclarationId::new(command_id.clone()),
                    functions,
                )
            }
            crate::project::ProjectProfile::OwnedDataApiV1 => {
                unreachable!("Project v8 uses the exact function-reachable linker")
            }
            crate::project::ProjectProfile::FlatOwnedRecordApiV1 => {
                unreachable!("Project v9 uses the exact aggregate-aware linker")
            }
            crate::project::ProjectProfile::OwnedUtf8ApiV1 => {
                unreachable!("Project v10 uses the exact function-reachable linker")
            }
            crate::project::ProjectProfile::NestedOwnedRecordApiV1 => {
                unreachable!("Project v11 uses the exact aggregate-aware linker")
            }
        }
        .map_err(|error| vec![error])
    }

    /// Link exactly the union of the Project-v8 entry closure and selected
    /// public roots. Older Project profiles deliberately retain their frozen
    /// module-oriented behavior; v8's owned-data runtime must be absent for
    /// every unrelated function, including one in an otherwise reachable
    /// module.
    fn linked_owned_data_api_program_with_roots(
        &self,
        entry_module: &str,
        additional_roots: &[String],
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        validate_entry_module(entry_module)?;
        let mut available = BTreeMap::<hir::DeclarationId, hir::LinkedScalarFunction>::new();
        let mut entrypoints = Vec::new();
        for module in &self.hir.modules {
            for function in &module.functions {
                let Some(fact) = self.hir.declarations.get(function.id.as_str()) else {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data function is absent from declaration facts",
                    )]);
                };
                if fact.kind != hir::DeclarationKind::Function
                    || fact.path.as_deref() != Some(module.path.as_str())
                    || fact.module.as_deref() != Some(module.module.as_str())
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data function facts disagree with its retained body",
                    )]);
                }
                if module.module == entry_module && function.name == "main" {
                    if !function.params.is_empty() || function.return_type != hir::ResolvedType::I64
                    {
                        return Err(vec![graph_error(
                            "SPX-G172",
                            "workspace owned-data entry must have the exact signature fn main() -> i64",
                        )]);
                    }
                    entrypoints.push((function.id.clone(), fact.origin));
                }
                if available
                    .insert(
                        function.id.clone(),
                        hir::LinkedScalarFunction {
                            function: function.clone(),
                            origin: fact.origin,
                        },
                    )
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data function identity is duplicated",
                    )]);
                }
            }
        }
        let [(entrypoint, hir::IdentityOrigin::Explicit)] = entrypoints.as_slice() else {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace owned-data entry module must declare exactly one explicit authored `main` function",
            )]);
        };
        let entrypoint = entrypoint.clone();
        let mut roots = BTreeSet::from([entrypoint.clone()]);
        roots.extend(
            additional_roots
                .iter()
                .map(|root| hir::DeclarationId::new(root.clone())),
        );
        let generics = owned_generics::OwnedGenericInventory::collect(
            &self.hir.modules,
            &self.hir.declarations,
        )?;
        let closure = owned_generics::close_owned_data_closure(&available, &generics, roots)?;
        let functions = closure
            .functions
            .iter()
            .map(|id| {
                available
                    .get(id)
                    .map(|linked| hir::LinkedScalarFunction {
                        function: linked.function.clone(),
                        origin: linked.origin,
                    })
                    .ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "owned-data closure names an unauthenticated function",
                        )]
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let function_templates = generics.retained_templates(&closure.templates)?;
        let function_instances = generics.retained_instances(&functions, &closure)?;

        let referenced_imports = functions
            .iter()
            .flat_map(|linked| resolved_function_imports(&linked.function))
            .collect::<BTreeSet<_>>();
        let mut imports =
            BTreeMap::<hir::DeclarationId, (&hir::ResolvedInterface, &hir::ResolvedImport)>::new();
        for module in &self.hir.modules {
            for interface in &module.interfaces {
                for import in &interface.imports {
                    if import.interface != interface.id
                        || imports
                            .insert(import.id.clone(), (interface, import))
                            .is_some()
                    {
                        return Err(vec![graph_error(
                            "SPX-G173",
                            "workspace owned-data import inventory is ambiguous",
                        )]);
                    }
                }
            }
        }
        let mut selected_imports =
            BTreeMap::<hir::DeclarationId, BTreeSet<hir::DeclarationId>>::new();
        for import_id in referenced_imports {
            let Some((interface, _)) = imports.get(&import_id) else {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data closure references unknown import `{import_id}`"),
                )]);
            };
            selected_imports
                .entry(interface.id.clone())
                .or_default()
                .insert(import_id);
        }
        let interfaces = selected_imports
            .into_iter()
            .map(|(interface_id, selected)| {
                let interface = imports
                    .values()
                    .find_map(|(interface, _)| (interface.id == interface_id).then_some(*interface))
                    .ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "owned-data interface selection lost its authenticated owner",
                        )]
                    })?;
                let mut interface = interface.clone();
                interface
                    .imports
                    .retain(|import| selected.contains(&import.id));
                Ok(interface)
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;

        let mut available_types = BTreeMap::new();
        for module in &self.hir.modules {
            for declaration in &module.types {
                if available_types
                    .insert(declaration.id.clone(), declaration.clone())
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data type identity is duplicated",
                    )]);
                }
            }
        }
        let types = hir::reachable_authored_types(
            &functions,
            &function_instances,
            &interfaces,
            &available_types,
        )
        .map_err(|error| vec![error])?;

        fn retain_fact(
            authenticated: &BTreeMap<String, WorkspaceDeclarationFact>,
            selected: &mut BTreeMap<hir::DeclarationId, hir::LinkedDeclarationFact>,
            id: &hir::DeclarationId,
            kind: hir::DeclarationKind,
            owner: Option<&hir::DeclarationId>,
        ) -> Result<(), Vec<Diagnostic>> {
            let Some(fact) = authenticated.get(id.as_str()) else {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data declaration `{id}` has no Phase-A fact"),
                )]);
            };
            if fact.kind != kind || fact.owner.as_deref() != owner.map(hir::DeclarationId::as_str) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data declaration `{id}` disagrees with its Phase-A fact"),
                )]);
            }
            if selected
                .insert(
                    id.clone(),
                    hir::LinkedDeclarationFact {
                        kind: fact.kind,
                        origin: fact.origin,
                        owner: owner.cloned(),
                    },
                )
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data declaration `{id}` is selected more than once"),
                )]);
            }
            Ok(())
        }

        let mut declaration_facts = BTreeMap::new();
        for linked in &functions {
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &linked.function.id,
                hir::DeclarationKind::Function,
                None,
            )?;
        }
        for template in &function_templates {
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &template.id,
                hir::DeclarationKind::Function,
                None,
            )?;
        }
        for declaration in &types {
            let kind = match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
                hir::ResolvedTypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
                hir::ResolvedTypeDeclarationKind::Class { .. }
                | hir::ResolvedTypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "owned-data type projection escaped the record/variant profile",
                    )]);
                }
            };
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &declaration.id,
                kind,
                None,
            )?;
            match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        retain_fact(
                            &self.hir.declarations,
                            &mut declaration_facts,
                            &field.id,
                            hir::DeclarationKind::Field,
                            Some(&declaration.id),
                        )?;
                    }
                }
                hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        retain_fact(
                            &self.hir.declarations,
                            &mut declaration_facts,
                            &case.id,
                            hir::DeclarationKind::VariantCase,
                            Some(&declaration.id),
                        )?;
                        for field in &case.fields {
                            retain_fact(
                                &self.hir.declarations,
                                &mut declaration_facts,
                                &field.id,
                                hir::DeclarationKind::CaseField,
                                Some(&case.id),
                            )?;
                        }
                    }
                }
                hir::ResolvedTypeDeclarationKind::Class { .. }
                | hir::ResolvedTypeDeclarationKind::Resource { .. } => {
                    unreachable!("rejected above")
                }
            }
        }
        for interface in &interfaces {
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &interface.id,
                hir::DeclarationKind::Interface,
                None,
            )?;
            for import in &interface.imports {
                retain_fact(
                    &self.hir.declarations,
                    &mut declaration_facts,
                    &import.id,
                    hir::DeclarationKind::Import,
                    Some(&interface.id),
                )?;
            }
        }
        let permits = functions
            .iter()
            .flat_map(|linked| linked.function.effects.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        hir::link_owned_data_api_workspace(
            entry_module.to_owned(),
            entrypoint,
            functions,
            hir::LinkedOwnedDataParts {
                permits,
                types,
                interfaces,
                declaration_facts,
                function_templates,
                function_instances,
            },
        )
        .map_err(|error| vec![error])
    }

    /// Consume one Phase-A graph build only after deriving both requested
    /// closures. The returned programs retain independently validated HIR, but
    /// their common provider bodies originate from this one graph build.
    pub(crate) fn into_linked_scalar_programs(
        self,
        entry_module: &str,
        test_module: &str,
    ) -> Result<(hir::ResolvedProgram, hir::ResolvedProgram), Vec<Diagnostic>> {
        self.validate_entire_scalar_workspace(entry_module, test_module)?;
        let entry = self.linked_scalar_program(entry_module)?;
        let test = self.linked_scalar_program(test_module)?;
        Ok((entry, test))
    }

    /// Consume one Project Phase-A build into its two executable closures and
    /// one complete declared-project projection. The projection moves the
    /// already validated HIR and graph edges; it never reparses or resolves a
    /// source and it is not a managed Workspace authority.
    pub(crate) fn into_project_semantic_parts(
        self,
        workspace_revision: &str,
        source_facts: Vec<ProjectGraphSourceFact>,
        manifest_bytes: usize,
        entry_module: &str,
        test_module: &str,
        web_roots: ProjectWebRoots<'_>,
    ) -> Result<ProjectSemanticParts, Vec<Diagnostic>> {
        self.validate_entire_project_workspace(entry_module, test_module, web_roots.profile)?;
        if matches!(
            web_roots.profile,
            crate::project::ProjectProfile::LanguageCommandIoV1
                | crate::project::ProjectProfile::LineCommandIoV1
        ) {
            let [command_id] = web_roots.stable_ids else {
                return Err(vec![graph_error(
                    "SPX-G172",
                    "command I/O profile must select exactly one command identity",
                )]);
            };
            let command = self
                .hir
                .modules
                .iter()
                .flat_map(|module| module.functions.iter())
                .find(|function| function.id.as_str() == command_id)
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G172",
                        "command I/O profile command identity does not name a function",
                    )]
                })?;
            let explicit = self
                .hir
                .declarations
                .get(command.id.as_str())
                .is_some_and(|fact| fact.origin == hir::IdentityOrigin::Explicit);
            if !explicit
                || !command.params.is_empty()
                || command.return_type != hir::ResolvedType::Bool
            {
                return Err(vec![graph_error(
                    "SPX-G172",
                    "command I/O profile command must have an explicit identity and exact signature fn() -> bool",
                )]);
            }
        }
        let entry_program = self.linked_scalar_program_with_roots(
            entry_module,
            web_roots.stable_ids,
            web_roots.profile,
        )?;
        let test_program = self.linked_project_program(test_module, web_roots.profile)?;
        let projection = self.into_project_projection(
            workspace_revision,
            source_facts,
            manifest_bytes,
            entry_module,
        )?;
        Ok(ProjectSemanticParts {
            entry_program,
            test_program,
            projection,
        })
    }

    fn into_project_projection(
        self,
        workspace_revision: &str,
        source_facts: Vec<ProjectGraphSourceFact>,
        manifest_bytes: usize,
        entry_module: &str,
    ) -> Result<WorkspaceGraphProjection, Vec<Diagnostic>> {
        validate_entry_module(entry_module)?;
        let mut source_facts = source_facts
            .into_iter()
            .map(|fact| (fact.path.clone(), fact))
            .collect::<BTreeMap<_, _>>();
        if source_facts.len() != self.usage.managed_files {
            return Err(vec![graph_error(
                "SPX-G173",
                "project source facts disagree with the Phase-A file inventory",
            )]);
        }

        let mut dependency_depths = self.hir.dependency_depths;
        let mut modules = Vec::with_capacity(self.hir.modules.len());
        for module in self.hir.modules {
            let source = source_facts.remove(&module.path).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "project module source fact is absent from the authenticated snapshot",
                )]
            })?;
            let dependency_depth = dependency_depths.remove(&module.module).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "project module dependency depth is absent",
                )]
            })?;
            modules.push(WorkspaceGraphProjectionModule {
                path: module.path,
                module: module.module,
                source_graph_schema: source.source_graph_schema,
                source_revision: source.source_revision,
                source_digest: source.source_digest,
                dependency_depth,
                permits: module.permits,
                types: module.types,
                interfaces: module.interfaces,
                functions: module.functions,
                function_templates: module.function_templates,
                function_instances: module.function_instances,
                signature_types: module.signature_types,
            });
        }
        modules.sort_by(|left, right| left.path.cmp(&right.path));
        if !source_facts.is_empty() || !dependency_depths.is_empty() {
            return Err(vec![graph_error(
                "SPX-G173",
                "project Phase-A facts contain unconsumed source or module entries",
            )]);
        }

        let mut declarations = self
            .hir
            .declarations
            .into_iter()
            .map(|(id, fact)| WorkspaceGraphProjectionDeclaration {
                id,
                kind: fact.kind,
                origin: fact.origin,
                owner: fact.owner,
                path: fact.path,
                module: fact.module,
            })
            .collect::<Vec<_>>();
        declarations.sort_by(|left, right| left.id.cmp(&right.id));
        let mut edges = self.edges;
        edges.sort();
        let work = self.usage;
        Ok(WorkspaceGraphProjection {
            workspace_revision: workspace_revision.to_owned(),
            entry_module: entry_module.to_owned(),
            usage: WorkspaceGraphProjectionUsage {
                used_managed_files: work.managed_files,
                used_total_source_bytes: work.total_source_bytes,
                used_entry_module_bytes: entry_module.len(),
                used_declarations: work.declarations,
                used_callables: work.callables,
                used_call_sites: work.call_sites,
                used_uses: work.uses,
                used_resolved_cross_file_edges: work.resolved_cross_file_edges,
                used_dependency_depth: work.dependency_depth,
                used_builder_bytes: work.builder_bytes,
                used_manifest_bytes: manifest_bytes,
                used_output_bytes: 0,
                used_retained_generations: 0,
                used_staging_attempts: 0,
                used_unexpected_inventory_entries: 0,
                used_reachable_modules: modules.len(),
            },
            modules,
            declarations,
            edges,
            shared_prelude_ids: self.hir.shared_prelude_ids.into_iter().collect(),
        })
    }

    fn validate_entire_project_workspace(
        &self,
        entry_module: &str,
        test_module: &str,
        profile: crate::project::ProjectProfile,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_entry_module(entry_module)?;
        validate_entry_module(test_module)?;
        let roots = BTreeSet::from([entry_module, test_module]);
        let natives = retained_validation::scalar_native_imports(
            profile,
            self.hir
                .modules
                .iter()
                .flat_map(|module| &module.interfaces),
        );
        for module in &self.hir.modules {
            let permits_admitted =
                retained_validation::permits_admitted(profile, module, entry_module, &natives);
            let project_shape_admitted = profile.is_owned_api()
                || matches!(profile, crate::project::ProjectProfile::ScalarV1)
                || (module.types.is_empty()
                    && module.interfaces.is_empty()
                    && module.function_templates.is_empty()
                    && module.function_instances.is_empty());
            if !permits_admitted || !project_shape_admitted {
                return Err(vec![graph_error(
                    "SPX-G172",
                    format!(
                        "workspace module `{}` is outside the pure scalar linker profile",
                        module.module
                    ),
                )]);
            }
            if !roots.contains(module.module.as_str())
                && module
                    .functions
                    .iter()
                    .any(|function| function.name == "main")
            {
                return Err(vec![graph_error(
                    "SPX-G172",
                    "workspace scalar provider modules may not declare `main`",
                )]);
            }
            // Project v8 target admission is reachability-gated. Irrelevant
            // verified functions receive no runtime or target authority and
            // therefore cannot broaden or spuriously reject the exact union
            // linked below. The retained closure is independently checked by
            // the owned-data linker and canonical descriptor.
            if profile.is_owned_api() {
                continue;
            }
            for function in &module.functions {
                let admitted_parameter = |parameter: &hir::ResolvedParam| match profile {
                    crate::project::ProjectProfile::ScalarV1 => {
                        parameter.ownership == hir::OwnershipMode::Value
                            && hir::copy_scalar_type(&parameter.ty)
                    }
                    crate::project::ProjectProfile::UsefulTextConsumerV1 => matches!(
                        (&parameter.ty, parameter.ownership),
                        (
                            hir::ResolvedType::I64 | hir::ResolvedType::Bool,
                            hir::OwnershipMode::Value
                        ) | (hir::ResolvedType::Str, hir::OwnershipMode::Borrow)
                    ),
                    crate::project::ProjectProfile::UsefulDataV1
                    | crate::project::ProjectProfile::UsefulDataCommandV1
                    | crate::project::ProjectProfile::UsefulDataCommandV2
                    | crate::project::ProjectProfile::LanguageCommandIoV1
                    | crate::project::ProjectProfile::LineCommandIoV1
                    | crate::project::ProjectProfile::OwnedDataApiV1
                    | crate::project::ProjectProfile::FlatOwnedRecordApiV1
                    | crate::project::ProjectProfile::OwnedUtf8ApiV1
                    | crate::project::ProjectProfile::NestedOwnedRecordApiV1 => {
                        hir::useful_data_workspace_parameter_admitted(
                            &parameter.ty,
                            parameter.ownership,
                        )
                    }
                };
                let admitted_return = match profile {
                    crate::project::ProjectProfile::ScalarV1 => {
                        hir::copy_scalar_type(&function.return_type)
                    }
                    crate::project::ProjectProfile::UsefulTextConsumerV1 => matches!(
                        function.return_type,
                        hir::ResolvedType::I64 | hir::ResolvedType::Bool
                    ),
                    crate::project::ProjectProfile::UsefulDataV1
                    | crate::project::ProjectProfile::UsefulDataCommandV1
                    | crate::project::ProjectProfile::UsefulDataCommandV2
                    | crate::project::ProjectProfile::LanguageCommandIoV1
                    | crate::project::ProjectProfile::LineCommandIoV1 => {
                        hir::useful_data_workspace_return_admitted(&function.return_type)
                    }
                    crate::project::ProjectProfile::OwnedDataApiV1 => {
                        hir::owned_data_api_workspace_return_admitted(&function.return_type)
                    }
                    crate::project::ProjectProfile::FlatOwnedRecordApiV1 => true,
                    crate::project::ProjectProfile::OwnedUtf8ApiV1 => {
                        hir::owned_data_api_workspace_return_admitted(&function.return_type)
                            || function.return_type == hir::ResolvedType::String
                    }
                    crate::project::ProjectProfile::NestedOwnedRecordApiV1 => true,
                };
                let effects_admitted = function.effects.is_empty()
                    || natives.effects_admitted(&function.effects)
                    || (matches!(
                        profile,
                        crate::project::ProjectProfile::UsefulDataCommandV1
                            | crate::project::ProjectProfile::UsefulDataCommandV2
                    ) && function.effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT])
                    || (matches!(
                        profile,
                        crate::project::ProjectProfile::LanguageCommandIoV1
                            | crate::project::ProjectProfile::LineCommandIoV1
                    ) && function.effects.iter().all(|effect| {
                        matches!(
                            effect.as_str(),
                            crate::command_io_ops::ARGS_READ_EFFECT
                                | crate::command_io_ops::STDERR_WRITE_EFFECT
                                | crate::command_io_ops::STDIN_READ_EFFECT
                                | crate::host_io_ops::STDOUT_WRITE_EFFECT
                        )
                    }));
                if !effects_admitted
                    || !admitted_return
                    || function
                        .params
                        .iter()
                        .any(|parameter| !admitted_parameter(parameter))
                {
                    let profile = match profile {
                        crate::project::ProjectProfile::ScalarV1 => "pure scalar linker",
                        crate::project::ProjectProfile::UsefulTextConsumerV1 => {
                            "Useful Text Consumer linker"
                        }
                        crate::project::ProjectProfile::UsefulDataV1 => "Useful Data linker",
                        crate::project::ProjectProfile::UsefulDataCommandV1 => {
                            "Useful Data Command linker"
                        }
                        crate::project::ProjectProfile::UsefulDataCommandV2 => {
                            "Useful Data Command v2 linker"
                        }
                        crate::project::ProjectProfile::LanguageCommandIoV1 => {
                            "Language Command I/O v1 linker"
                        }
                        crate::project::ProjectProfile::LineCommandIoV1 => {
                            "Line Command I/O v1 linker"
                        }
                        crate::project::ProjectProfile::OwnedDataApiV1 => {
                            "Owned Data API v1 linker"
                        }
                        crate::project::ProjectProfile::FlatOwnedRecordApiV1 => {
                            "Flat Owned Record API v1 linker"
                        }
                        crate::project::ProjectProfile::OwnedUtf8ApiV1 => {
                            "Owned UTF-8 API v1 linker"
                        }
                        crate::project::ProjectProfile::NestedOwnedRecordApiV1 => {
                            "Nested Owned Record API v1 linker"
                        }
                    };
                    return Err(vec![graph_error(
                        "SPX-G172",
                        format!(
                            "workspace function `{}` is outside the {profile} profile",
                            function.id
                        ),
                    )]);
                }
            }
        }
        if !profile.is_owned_api() && self.edges.iter().any(|edge| edge.kind == "type_import") {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace scalar linker does not admit `use type` imports",
            )]);
        }
        Ok(())
    }

    fn validate_entire_scalar_workspace(
        &self,
        entry_module: &str,
        test_module: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        self.validate_entire_project_workspace(
            entry_module,
            test_module,
            crate::project::ProjectProfile::ScalarV1,
        )
    }

    pub(crate) fn change_builder_bytes(&self) -> Option<usize> {
        self.change_fingerprints
            .as_ref()
            .map(|_| self.change_builder_bytes)
    }

    pub(crate) fn into_change_view(self) -> Result<WorkspaceGraphChangeView, Vec<Diagnostic>> {
        let mut fingerprints = self.change_fingerprints.ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace change fingerprint sidecar is absent",
            )]
        })?;
        let modules = self
            .hir
            .modules
            .into_iter()
            .map(|module| {
                let source_graph_schema = semantic_workspace_source_schema(&module)?;
                Ok(WorkspaceGraphChangeModule {
                    path: module.path,
                    module: module.module,
                    source_graph_schema,
                    permits: module.permits,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let declarations = self
            .hir
            .declarations
            .into_iter()
            .map(|(id, declaration)| {
                let semantic_fingerprint =
                    if declaration.origin == hir::IdentityOrigin::CompilerOwned {
                        String::new()
                    } else {
                        fingerprints.remove(&id).ok_or_else(|| {
                            vec![graph_error(
                                "SPX-G173",
                                "workspace change declaration fingerprint is absent",
                            )]
                        })?
                    };
                Ok(WorkspaceGraphChangeDeclaration {
                    id,
                    kind: declaration.kind,
                    origin: declaration.origin,
                    owner: declaration.owner,
                    path: declaration.path,
                    module: declaration.module,
                    semantic_fingerprint,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        if !fingerprints.is_empty() {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace change fingerprints contain unknown declarations",
            )]);
        }
        Ok(WorkspaceGraphChangeView {
            modules,
            declarations,
            edges: self.edges,
            dependency_depths: self.hir.dependency_depths,
            shared_prelude_ids: self.hir.shared_prelude_ids.into_iter().collect(),
            usage: self.usage,
        })
    }

    pub(crate) fn into_operation_view(
        mut self,
    ) -> Result<WorkspaceGraphOperationView, Vec<Diagnostic>> {
        let sidecar = self.operation_sidecar.take().ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace operations AST sidecar is absent",
            )]
        })?;
        let builder_bytes = self.operation_builder_bytes;
        let change_builder_bytes = self.change_builder_bytes;
        let graph = self.into_change_view()?;
        Ok(WorkspaceGraphOperationView {
            graph,
            sidecar,
            builder_bytes,
            change_builder_bytes,
        })
    }

    pub(crate) fn source_graph_schemas(
        &self,
    ) -> Result<BTreeMap<String, &'static str>, Vec<Diagnostic>> {
        let mut schemas = BTreeMap::new();
        for module in &self.hir.modules {
            let schema = semantic_workspace_source_schema(module)?;
            if schemas.insert(module.path.clone(), schema).is_some() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace module paths disagree while deriving source Graph schemas",
                )]);
            }
        }
        if schemas.len() != self.usage.managed_files {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace source Graph schema facts disagree with managed files",
            )]);
        }
        Ok(schemas)
    }
}

fn semantic_workspace_source_schema(
    module: &WorkspaceResolvedModule,
) -> Result<&'static str, Vec<Diagnostic>> {
    graph::graph_schema_from_parts_and_instances(
        &module.interfaces,
        &module.types,
        &module.functions,
        &module.function_templates,
        &module.function_instances,
    )
    .map_err(|error| vec![error])
}

fn resolved_function_callees(function: &hir::ResolvedFunction) -> BTreeSet<hir::DeclarationId> {
    fn visit(expression: &hir::ResolvedExpr, callees: &mut BTreeSet<hir::DeclarationId>) {
        if let hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            callees.insert(callee.clone());
        }
        match &expression.kind {
            hir::ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                visit(source, callees);
                visit(start, callees);
                visit(end, callees);
            }
            hir::ResolvedExprKind::Call { args, .. } => {
                for argument in args {
                    visit(argument, callees);
                }
            }
            hir::ResolvedExprKind::NativeRustImportCall(call) => {
                for argument in &call.args {
                    visit(argument, callees);
                }
            }
            hir::ResolvedExprKind::HostCommandCall(call) => {
                for argument in &call.args {
                    visit(argument, callees);
                }
            }
            hir::ResolvedExprKind::Unary { value, .. }
            | hir::ResolvedExprKind::Try { operand: value, .. }
            | hir::ResolvedExprKind::TryOption { operand: value, .. }
            | hir::ResolvedExprKind::Project { base: value, .. }
            | hir::ResolvedExprKind::Upcast { source: value } => visit(value, callees),
            hir::ResolvedExprKind::Binary { left, right, .. } => {
                visit(left, callees);
                visit(right, callees);
            }
            hir::ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            visit(child, callees);
                        }
                    }
                }
                visit(tail, callees);
            }
            hir::ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                visit(condition, callees);
                visit(then_branch, callees);
                visit(else_branch, callees);
            }
            hir::ResolvedExprKind::ConstructRecord { fields, .. }
            | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    visit(&field.value, callees);
                }
            }
            hir::ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                visit(scrutinee, callees);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        visit(guard, callees);
                    }
                    visit(&arm.value, callees);
                }
            }
            hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                visit(base, callees);
                for field in fields {
                    visit(&field.value, callees);
                }
            }
            hir::ResolvedExprKind::Int(_)
            | hir::ResolvedExprKind::Int32(_)
            | hir::ResolvedExprKind::Char(_)
            | hir::ResolvedExprKind::Uint8(_)
            | hir::ResolvedExprKind::Usize(_)
            | hir::ResolvedExprKind::Float32(_)
            | hir::ResolvedExprKind::Float64(_)
            | hir::ResolvedExprKind::Bool(_)
            | hir::ResolvedExprKind::String(_)
            | hir::ResolvedExprKind::ArrayU8(_)
            | hir::ResolvedExprKind::RepeatArrayU8 { .. }
            | hir::ResolvedExprKind::BorrowPlace { .. }
            | hir::ResolvedExprKind::Place(_) => {}
        }
    }

    let mut callees = BTreeSet::new();
    for requirement in &function.requires {
        visit(requirement, &mut callees);
    }
    visit(&function.body, &mut callees);
    for postcondition in &function.ensures {
        visit(postcondition, &mut callees);
    }
    callees
}

fn resolved_function_imports(function: &hir::ResolvedFunction) -> BTreeSet<hir::DeclarationId> {
    fn visit(expression: &hir::ResolvedExpr, imports: &mut BTreeSet<hir::DeclarationId>) {
        match &expression.kind {
            hir::ResolvedExprKind::NativeRustImportCall(call) => {
                imports.insert(call.import.clone());
                for argument in &call.args {
                    visit(argument, imports);
                }
            }
            hir::ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                visit(source, imports);
                visit(start, imports);
                visit(end, imports);
            }
            hir::ResolvedExprKind::Call { args, .. }
            | hir::ResolvedExprKind::HostCommandCall(hir::ResolvedHostCommandCall {
                args, ..
            }) => {
                for argument in args {
                    visit(argument, imports);
                }
            }
            hir::ResolvedExprKind::Unary { value, .. }
            | hir::ResolvedExprKind::Try { operand: value, .. }
            | hir::ResolvedExprKind::TryOption { operand: value, .. }
            | hir::ResolvedExprKind::Project { base: value, .. }
            | hir::ResolvedExprKind::Upcast { source: value } => visit(value, imports),
            hir::ResolvedExprKind::Binary { left, right, .. } => {
                visit(left, imports);
                visit(right, imports);
            }
            hir::ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            visit(child, imports);
                        }
                    }
                }
                visit(tail, imports);
            }
            hir::ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                visit(condition, imports);
                visit(then_branch, imports);
                visit(else_branch, imports);
            }
            hir::ResolvedExprKind::ConstructRecord { fields, .. }
            | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    visit(&field.value, imports);
                }
            }
            hir::ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                visit(scrutinee, imports);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        visit(guard, imports);
                    }
                    visit(&arm.value, imports);
                }
            }
            hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                visit(base, imports);
                for field in fields {
                    visit(&field.value, imports);
                }
            }
            hir::ResolvedExprKind::Int(_)
            | hir::ResolvedExprKind::Int32(_)
            | hir::ResolvedExprKind::Char(_)
            | hir::ResolvedExprKind::Uint8(_)
            | hir::ResolvedExprKind::Usize(_)
            | hir::ResolvedExprKind::Float32(_)
            | hir::ResolvedExprKind::Float64(_)
            | hir::ResolvedExprKind::Bool(_)
            | hir::ResolvedExprKind::String(_)
            | hir::ResolvedExprKind::ArrayU8(_)
            | hir::ResolvedExprKind::RepeatArrayU8 { .. }
            | hir::ResolvedExprKind::BorrowPlace { .. }
            | hir::ResolvedExprKind::Place(_) => {}
        }
    }

    let mut imports = BTreeSet::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit(expression, &mut imports);
    }
    imports
}

impl WorkspaceGraphChangeView {
    pub(crate) const fn used_managed_files(&self) -> usize {
        self.usage.managed_files
    }

    pub(crate) const fn used_total_source_bytes(&self) -> usize {
        self.usage.total_source_bytes
    }

    pub(crate) const fn used_builder_bytes(&self) -> usize {
        self.usage.builder_bytes
    }

    pub(crate) fn modules(&self) -> &[WorkspaceGraphChangeModule] {
        &self.modules
    }

    pub(crate) fn declarations(&self) -> &[WorkspaceGraphChangeDeclaration] {
        &self.declarations
    }

    pub(crate) fn edges(&self) -> &[WorkspaceEdge] {
        &self.edges
    }

    #[cfg(test)]
    pub(crate) fn extend_change_test_reverse_call_chain(
        &mut self,
        prefix: &str,
        edge_count: usize,
    ) -> String {
        let path = self.modules[0].path.clone();
        let module = self.modules[0].module.clone();
        for index in 0..=edge_count {
            self.declarations.push(WorkspaceGraphChangeDeclaration {
                id: format!("{prefix}.{index}"),
                kind: hir::DeclarationKind::Function,
                origin: hir::IdentityOrigin::Automatic,
                owner: None,
                path: Some(path.clone()),
                module: Some(module.clone()),
                semantic_fingerprint: format!("test:{prefix}:{index}"),
            });
        }
        for index in 0..edge_count {
            self.edges.push(WorkspaceEdge {
                caller_path: path.clone(),
                caller: format!("{prefix}.{}", index + 1),
                target_path: path.clone(),
                target: format!("{prefix}.{index}"),
                kind: "call",
                site: "body",
                expression: format!("test-call-{index}"),
                ast_path: format!("test.chain.{index}"),
                alias: format!("test_{index}"),
                ordinal: index,
            });
        }
        format!("{prefix}.0")
    }

    pub(crate) fn projection_digest(
        &self,
        workspace_revision: &str,
        sources: &[WorkspaceGraphChangeSourceFact],
        manifest_bytes: usize,
        retained_generations: usize,
        staging_attempts: usize,
        entry_module: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        validate_entry_module(entry_module)?;
        let Some(entry) = self
            .modules
            .iter()
            .find(|module| module.module == entry_module)
        else {
            return Err(vec![graph_error(
                "SPX-G172",
                format!("Workspace Semantic Graph entry module `{entry_module}` is absent"),
            )]);
        };
        let authenticated_paths = self
            .modules
            .iter()
            .map(|module| module.path.as_str())
            .collect::<BTreeSet<_>>();
        let mut providers = BTreeMap::<&str, BTreeSet<&str>>::new();
        for edge in self
            .edges
            .iter()
            .filter(|edge| matches!(edge.kind, "function_import" | "type_import"))
        {
            if !authenticated_paths.contains(edge.caller_path.as_str())
                || !authenticated_paths.contains(edge.target_path.as_str())
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace change projection import paths disagree",
                )]);
            }
            providers
                .entry(edge.caller_path.as_str())
                .or_default()
                .insert(edge.target_path.as_str());
        }
        let mut reachable = BTreeSet::from([entry.path.as_str()]);
        let mut pending = BTreeSet::from([entry.path.as_str()]);
        while let Some(path) = pending.pop_first() {
            if let Some(direct) = providers.get(path) {
                for provider in direct {
                    if reachable.insert(provider) {
                        pending.insert(provider);
                    }
                }
            }
        }
        let source_facts = sources
            .iter()
            .map(|source| (source.path.as_str(), source))
            .collect::<BTreeMap<_, _>>();
        let mut modules = Vec::with_capacity(reachable.len());
        for module in self
            .modules
            .iter()
            .filter(|module| reachable.contains(module.path.as_str()))
        {
            let source = source_facts.get(module.path.as_str()).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace change projection source fact is absent",
                )]
            })?;
            let dependency_depth =
                *self.dependency_depths.get(&module.module).ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "workspace change projection dependency depth is absent",
                    )]
                })?;
            modules.push(WorkspaceGraphProjectionModule {
                path: crate::bounded_output::budgeted_clone(&module.path),
                module: crate::bounded_output::budgeted_clone(&module.module),
                source_graph_schema: crate::bounded_output::budgeted_clone(
                    &source.source_graph_schema,
                ),
                source_revision: crate::bounded_output::budgeted_clone(&source.source_revision),
                source_digest: crate::bounded_output::budgeted_clone(&source.source_digest),
                dependency_depth,
                permits: module
                    .permits
                    .iter()
                    .map(|permit| crate::bounded_output::budgeted_clone(permit))
                    .collect(),
                types: Vec::new(),
                interfaces: Vec::new(),
                functions: Vec::new(),
                function_templates: Vec::new(),
                function_instances: Vec::new(),
                signature_types: BTreeMap::new(),
            });
        }
        modules.sort_by(|left, right| left.path.cmp(&right.path));
        let mut declarations = self
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.origin == hir::IdentityOrigin::CompilerOwned
                    || declaration
                        .path
                        .as_deref()
                        .is_some_and(|path| reachable.contains(path))
            })
            .map(|declaration| WorkspaceGraphProjectionDeclaration {
                id: crate::bounded_output::budgeted_clone(&declaration.id),
                kind: declaration.kind,
                origin: declaration.origin,
                owner: declaration
                    .owner
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
                path: declaration
                    .path
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
                module: declaration
                    .module
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
            })
            .collect::<Vec<_>>();
        declarations.sort_by(|left, right| left.id.cmp(&right.id));
        let mut edges = self
            .edges
            .iter()
            .filter(|edge| reachable.contains(edge.caller_path.as_str()))
            .map(budgeted_edge_clone)
            .collect::<Vec<_>>();
        if edges
            .iter()
            .any(|edge| !reachable.contains(edge.target_path.as_str()))
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace change projected edge target escapes provider closure",
            )]);
        }
        edges.sort();
        let work = self.usage;
        let projection = WorkspaceGraphProjection {
            workspace_revision: crate::bounded_output::budgeted_clone(workspace_revision),
            entry_module: crate::bounded_output::budgeted_clone(entry_module),
            modules,
            declarations,
            edges,
            shared_prelude_ids: self.shared_prelude_ids.clone(),
            usage: WorkspaceGraphProjectionUsage {
                used_managed_files: work.managed_files,
                used_total_source_bytes: work.total_source_bytes,
                used_entry_module_bytes: entry_module.len(),
                used_declarations: work.declarations,
                used_callables: work.callables,
                used_call_sites: work.call_sites,
                used_uses: work.uses,
                used_resolved_cross_file_edges: work.resolved_cross_file_edges,
                used_dependency_depth: work.dependency_depth,
                used_builder_bytes: work.builder_bytes,
                used_manifest_bytes: manifest_bytes,
                used_output_bytes: 0,
                used_retained_generations: retained_generations,
                used_staging_attempts: staging_attempts,
                used_unexpected_inventory_entries: 0,
                used_reachable_modules: reachable.len(),
            },
        };
        semantic_graph_digest_and_output_bytes(&projection, MAX_OUTPUT_BYTES)
            .map(|(digest, _)| digest)
    }
}

impl WorkspaceGraphChangeModule {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn module(&self) -> &str {
        &self.module
    }

    pub(crate) const fn source_graph_schema(&self) -> &'static str {
        self.source_graph_schema
    }

    pub(crate) fn permits(&self) -> &[String] {
        &self.permits
    }
}

impl WorkspaceGraphChangeDeclaration {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn kind(&self) -> hir::DeclarationKind {
        self.kind
    }

    pub(crate) fn origin(&self) -> hir::IdentityOrigin {
        self.origin
    }

    pub(crate) fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub(crate) fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }

    pub(crate) fn semantic_fingerprint(&self) -> &str {
        &self.semantic_fingerprint
    }
}

fn build_from_authenticated_authority(
    authority: &mut workspace::WorkspaceSemanticReadAuthority,
) -> Result<AuthenticatedWorkspaceGraphBuild, Vec<Diagnostic>> {
    let workspace_revision = authority.workspace_revision().to_owned();
    let storage = AuthenticatedWorkspaceStorageUsage {
        manifest_bytes: authority.manifest_bytes(),
        retained_generations: authority.retained_generations(),
        staging_attempts: authority.staging_attempts(),
        unexpected_inventory_entries: 0,
    };
    let mut source_facts = BTreeMap::new();
    for source in authority.take_sources() {
        let path = source.path;
        source_facts.insert(
            path.clone(),
            AuthenticatedSourceFact {
                path: path.clone(),
                source_graph_schema: source.source_graph_schema,
                source_revision: source.source_revision,
                source_digest: source.source_digest,
            },
        );
    }
    let graph = authority.take_graph()?;
    Ok(AuthenticatedWorkspaceGraphBuild {
        workspace_revision,
        sources: source_facts,
        storage,
        graph,
    })
}

pub(crate) fn build_authenticated_projection(
    root: &Path,
    entry_module: &str,
) -> Result<WorkspaceGraphProjection, Vec<Diagnostic>> {
    with_authenticated_projection(root, entry_module, Ok)
}

pub fn snapshot(
    root: &Path,
    entry_module: &str,
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>> {
    build_authenticated_semantic_graph_inner(root, entry_module, |_| {})
}

fn build_authenticated_semantic_graph_inner(
    root: &Path,
    entry_module: &str,
    after_render: impl FnOnce(&WorkspaceSemanticGraph),
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>> {
    with_authenticated_projection(root, entry_module, |projection| {
        let graph = render_semantic_graph(projection)?;
        after_render(&graph);
        Ok(graph)
    })
}

#[cfg(test)]
fn build_authenticated_semantic_graph_with_hook(
    root: &Path,
    entry_module: &str,
    after_render: impl FnOnce(&WorkspaceSemanticGraph),
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>> {
    build_authenticated_semantic_graph_inner(root, entry_module, after_render)
}

fn with_authenticated_projection<T>(
    root: &Path,
    entry_module: &str,
    operation: impl FnOnce(WorkspaceGraphProjection) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let mut authority = workspace::acquire_semantic_read(root)?;
    let result = validate_entry_module(entry_module)
        .and_then(|()| build_from_authenticated_authority(&mut authority))
        .and_then(|build| build.project(entry_module))
        .and_then(operation);
    authority.finish(result)
}

fn with_authenticated_analysis<T>(
    root: &Path,
    entry_module: &str,
    operation: impl FnOnce(crate::workspace_analysis::WorkspaceAnalysis) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    with_authenticated_projection(root, entry_module, |projection| {
        crate::workspace_analysis::WorkspaceAnalysis::build(projection).and_then(operation)
    })
}

#[cfg(test)]
pub(crate) fn build_authenticated_analysis_for_test(
    root: &Path,
    entry_module: &str,
) -> Result<crate::workspace_analysis::WorkspaceAnalysis, Vec<Diagnostic>> {
    with_authenticated_analysis(root, entry_module, Ok)
}

pub(crate) fn with_authenticated_context<T>(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    direction: crate::workspace_analysis::WorkspaceAnalysisDirection,
    depth: usize,
    max_nodes: usize,
    operation: impl FnOnce(
        crate::workspace_analysis::WorkspaceContextFacts,
    ) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    with_authenticated_analysis(root, entry_module, |analysis| {
        analysis
            .context(target, direction, depth, max_nodes)
            .and_then(operation)
    })
}

pub(crate) fn with_authenticated_impact<T>(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    depth: usize,
    max_nodes: usize,
    operation: impl FnOnce(
        crate::workspace_analysis::WorkspaceImpactFacts,
    ) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    with_authenticated_analysis(root, entry_module, |analysis| {
        analysis
            .impact(target, depth, max_nodes)
            .and_then(operation)
    })
}

pub(crate) fn build_authenticated_context_artifact(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    direction: crate::workspace_analysis::WorkspaceAnalysisDirection,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
) -> Result<crate::workspace_analysis::WorkspaceContextArtifact, Vec<Diagnostic>> {
    build_authenticated_context_artifact_inner(
        root,
        entry_module,
        target,
        direction,
        depth,
        max_bytes,
        max_nodes,
        |_| {},
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "closed authenticated Context operation"
)]
fn build_authenticated_context_artifact_inner(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    direction: crate::workspace_analysis::WorkspaceAnalysisDirection,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    after_render: impl FnOnce(&crate::workspace_analysis::WorkspaceContextArtifact),
) -> Result<crate::workspace_analysis::WorkspaceContextArtifact, Vec<Diagnostic>> {
    with_authenticated_analysis(root, entry_module, |analysis| {
        let artifact = analysis.render_context(target, direction, depth, max_bytes, max_nodes)?;
        after_render(&artifact);
        Ok(artifact)
    })
    .map_err(|diagnostics| {
        crate::workspace_analysis::map_artifact_diagnostics(
            crate::workspace_analysis::WorkspaceAnalysisArtifactKind::Context,
            diagnostics,
        )
    })
}

#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "test-only Context final-boundary seam"
)]
pub(crate) fn build_authenticated_context_artifact_with_hook(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    direction: crate::workspace_analysis::WorkspaceAnalysisDirection,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    after_render: impl FnOnce(&crate::workspace_analysis::WorkspaceContextArtifact),
) -> Result<crate::workspace_analysis::WorkspaceContextArtifact, Vec<Diagnostic>> {
    build_authenticated_context_artifact_inner(
        root,
        entry_module,
        target,
        direction,
        depth,
        max_bytes,
        max_nodes,
        after_render,
    )
}

pub(crate) fn build_authenticated_impact_artifact(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
) -> Result<crate::workspace_analysis::WorkspaceImpactArtifact, Vec<Diagnostic>> {
    build_authenticated_impact_artifact_inner(
        root,
        entry_module,
        target,
        depth,
        max_bytes,
        max_nodes,
        |_| {},
    )
}

fn build_authenticated_impact_artifact_inner(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    after_render: impl FnOnce(&crate::workspace_analysis::WorkspaceImpactArtifact),
) -> Result<crate::workspace_analysis::WorkspaceImpactArtifact, Vec<Diagnostic>> {
    with_authenticated_analysis(root, entry_module, |analysis| {
        let artifact = analysis.render_impact(target, depth, max_bytes, max_nodes)?;
        after_render(&artifact);
        Ok(artifact)
    })
    .map_err(|diagnostics| {
        crate::workspace_analysis::map_artifact_diagnostics(
            crate::workspace_analysis::WorkspaceAnalysisArtifactKind::Impact,
            diagnostics,
        )
    })
}

#[cfg(test)]
pub(crate) fn build_authenticated_impact_artifact_with_hook(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    after_render: impl FnOnce(&crate::workspace_analysis::WorkspaceImpactArtifact),
) -> Result<crate::workspace_analysis::WorkspaceImpactArtifact, Vec<Diagnostic>> {
    build_authenticated_impact_artifact_inner(
        root,
        entry_module,
        target,
        depth,
        max_bytes,
        max_nodes,
        after_render,
    )
}

pub(crate) fn build_authenticated_review_artifact(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
) -> Result<crate::workspace_analysis::WorkspaceReviewArtifact, Vec<Diagnostic>> {
    build_authenticated_review_artifact_inner(root, entry_module, target, |_| {})
}

fn build_authenticated_review_artifact_inner(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    after_render: impl FnOnce(&crate::workspace_analysis::WorkspaceReviewArtifact),
) -> Result<crate::workspace_analysis::WorkspaceReviewArtifact, Vec<Diagnostic>> {
    with_authenticated_analysis(root, entry_module, |analysis| {
        let artifact = analysis.render_review(target)?;
        after_render(&artifact);
        Ok(artifact)
    })
    .map_err(|diagnostics| {
        crate::workspace_analysis::map_artifact_diagnostics(
            crate::workspace_analysis::WorkspaceAnalysisArtifactKind::Review,
            diagnostics,
        )
    })
}

#[cfg(test)]
pub(crate) fn build_authenticated_review_artifact_with_hook(
    root: &Path,
    entry_module: &str,
    target: crate::workspace_analysis::WorkspaceAnalysisTarget,
    after_render: impl FnOnce(&crate::workspace_analysis::WorkspaceReviewArtifact),
) -> Result<crate::workspace_analysis::WorkspaceReviewArtifact, Vec<Diagnostic>> {
    build_authenticated_review_artifact_inner(root, entry_module, target, after_render)
}

#[cfg(test)]
fn build_authenticated_projection_with_hook(
    root: &Path,
    entry_module: &str,
    hook: impl FnOnce(),
) -> Result<WorkspaceGraphProjection, Vec<Diagnostic>> {
    with_authenticated_projection(root, entry_module, |projection| {
        hook();
        Ok(projection)
    })
}

impl AuthenticatedWorkspaceGraphBuild {
    fn project(self, entry_module: &str) -> Result<WorkspaceGraphProjection, Vec<Diagnostic>> {
        validate_entry_module(entry_module)?;
        let Some(entry_path) = self.graph.hir.module_paths.get(entry_module).cloned() else {
            return Err(vec![graph_error(
                "SPX-G172",
                format!("Workspace Semantic Graph entry module `{entry_module}` is absent"),
            )]);
        };

        let authenticated_paths = self
            .graph
            .hir
            .module_paths
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut direct_providers = BTreeMap::<String, BTreeSet<String>>::new();
        for edge in &self.graph.edges {
            if !matches!(edge.kind, "function_import" | "type_import") {
                continue;
            }
            if !authenticated_paths.contains(edge.caller_path.as_str())
                || !authenticated_paths.contains(edge.target_path.as_str())
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace import edge paths disagree with authenticated modules",
                )]);
            }
            direct_providers
                .entry(edge.caller_path.clone())
                .or_default()
                .insert(edge.target_path.clone());
        }

        let mut reachable_paths = BTreeSet::from([entry_path.clone()]);
        let mut pending = BTreeSet::from([entry_path]);
        while let Some(path) = pending.pop_first() {
            if let Some(providers) = direct_providers.get(&path) {
                for provider in providers {
                    if reachable_paths.insert(provider.clone()) {
                        pending.insert(provider.clone());
                    }
                }
            }
        }

        let mut source_facts = self.sources;
        let mut dependency_depths = self.graph.hir.dependency_depths;
        let mut modules = Vec::with_capacity(reachable_paths.len());
        for module in self.graph.hir.modules {
            if !reachable_paths.contains(module.path.as_str()) {
                continue;
            }
            let source = source_facts.remove(&module.path).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace module source facts disagree with authenticated snapshot paths",
                )]
            })?;
            if source.path != module.path {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace module source path disagrees with authenticated snapshot facts",
                )]);
            }
            let dependency_depth = dependency_depths.remove(&module.module).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace module dependency depth is absent",
                )]
            })?;
            modules.push(WorkspaceGraphProjectionModule {
                path: module.path,
                module: module.module,
                source_graph_schema: source.source_graph_schema,
                source_revision: source.source_revision,
                source_digest: source.source_digest,
                dependency_depth,
                permits: module.permits,
                types: module.types,
                interfaces: module.interfaces,
                functions: module.functions,
                function_templates: module.function_templates,
                function_instances: module.function_instances,
                signature_types: module.signature_types,
            });
        }
        modules.sort_by(|left, right| left.path.cmp(&right.path));
        if modules.len() != reachable_paths.len() {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace reachable-module closure disagrees with retained modules",
            )]);
        }

        let mut declarations = self
            .graph
            .hir
            .declarations
            .into_iter()
            .filter_map(|(id, fact)| {
                let retained = fact.origin == hir::IdentityOrigin::CompilerOwned
                    || fact
                        .path
                        .as_deref()
                        .is_some_and(|path| reachable_paths.contains(path));
                retained.then_some(WorkspaceGraphProjectionDeclaration {
                    id,
                    kind: fact.kind,
                    origin: fact.origin,
                    owner: fact.owner,
                    path: fact.path,
                    module: fact.module,
                })
            })
            .collect::<Vec<_>>();
        declarations.sort_by(|left, right| left.id.cmp(&right.id));

        let mut edges = self
            .graph
            .edges
            .into_iter()
            .filter(|edge| reachable_paths.contains(edge.caller_path.as_str()))
            .collect::<Vec<_>>();
        if edges
            .iter()
            .any(|edge| !reachable_paths.contains(edge.target_path.as_str()))
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace projected edge target escapes the provider closure",
            )]);
        }
        edges.sort();

        let work = self.graph.usage;
        let usage = WorkspaceGraphProjectionUsage {
            used_managed_files: work.managed_files,
            used_total_source_bytes: work.total_source_bytes,
            used_entry_module_bytes: entry_module.len(),
            used_declarations: work.declarations,
            used_callables: work.callables,
            used_call_sites: work.call_sites,
            used_uses: work.uses,
            used_resolved_cross_file_edges: work.resolved_cross_file_edges,
            used_dependency_depth: work.dependency_depth,
            used_builder_bytes: work.builder_bytes,
            used_manifest_bytes: self.storage.manifest_bytes,
            used_output_bytes: 0,
            used_retained_generations: self.storage.retained_generations,
            used_staging_attempts: self.storage.staging_attempts,
            used_unexpected_inventory_entries: self.storage.unexpected_inventory_entries,
            used_reachable_modules: modules.len(),
        };
        Ok(WorkspaceGraphProjection {
            workspace_revision: self.workspace_revision,
            entry_module: entry_module.to_owned(),
            modules,
            declarations,
            edges,
            shared_prelude_ids: self.graph.hir.shared_prelude_ids.into_iter().collect(),
            usage,
        })
    }
}

pub(crate) fn validate_entry_module(entry_module: &str) -> Result<(), Vec<Diagnostic>> {
    if entry_module.len() > MAX_ENTRY_MODULE_BYTES {
        return Err(vec![limit_error(
            "entry_module_bytes",
            MAX_ENTRY_MODULE_BYTES,
        )]);
    }
    let canonical = !entry_module.is_empty()
        && entry_module.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        });
    if canonical {
        Ok(())
    } else {
        Err(vec![graph_error(
            "SPX-G170",
            format!("Workspace Semantic Graph entry module `{entry_module}` is not canonical"),
        )])
    }
}

fn render_semantic_graph(
    projection: WorkspaceGraphProjection,
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>> {
    render_semantic_graph_with_output_limit(projection, MAX_OUTPUT_BYTES)
}

pub(crate) fn render_project_semantic_graph(
    projection: &WorkspaceGraphProjection,
    project_schema: &str,
    project_name: &str,
    project_revision: &str,
    test_module: &str,
) -> Result<ProjectSemanticGraphArtifact, Vec<Diagnostic>> {
    validate_render_projection(projection)?;
    validate_entry_module(test_module)?;
    if !projection
        .modules
        .iter()
        .any(|module| module.module == test_module)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "project test module is absent from the complete semantic projection",
        )]);
    }
    let (payload, overflowed) = crate::bounded_output::with_limit(MAX_OUTPUT_BYTES, || {
        render_project_graph_json(
            projection,
            project_schema,
            project_name,
            project_revision,
            test_module,
            None,
        )
    });
    if overflowed {
        return Err(vec![limit_error("output_bytes", MAX_OUTPUT_BYTES)]);
    }
    let digest = project_artifact_digest(PROJECT_GRAPH_DIGEST_DOMAIN, payload.as_bytes());
    let (json, overflowed) = crate::bounded_output::with_limit(MAX_OUTPUT_BYTES, || {
        render_project_graph_json(
            projection,
            project_schema,
            project_name,
            project_revision,
            test_module,
            Some(&digest),
        )
    });
    if overflowed || json.len() > MAX_OUTPUT_BYTES {
        return Err(vec![limit_error("output_bytes", MAX_OUTPUT_BYTES)]);
    }
    Ok(ProjectSemanticGraphArtifact { json, digest })
}

fn render_project_graph_json(
    projection: &WorkspaceGraphProjection,
    project_schema: &str,
    project_name: &str,
    project_revision: &str,
    test_module: &str,
    digest: Option<&str>,
) -> String {
    use std::fmt::Write as _;

    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, PROJECT_GRAPH_SCHEMA);
    output.push_str(",\"project_schema\":");
    push_json_string(&mut output, project_schema);
    output.push_str(",\"project\":");
    push_json_string(&mut output, project_name);
    output.push_str(",\"project_revision\":");
    push_json_string(&mut output, project_revision);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, projection.workspace_revision());
    if let Some(digest) = digest {
        output.push_str(",\"graph_digest\":");
        push_json_string(&mut output, digest);
    }
    output.push_str(",\"entry_module\":");
    push_json_string(&mut output, projection.entry_module());
    output.push_str(",\"test_module\":");
    push_json_string(&mut output, test_module);
    output.push_str(",\"modules\":[");
    for (index, module) in projection.modules.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        push_json_string(&mut output, &module.path);
        output.push_str(",\"module\":");
        push_json_string(&mut output, &module.module);
        output.push_str(",\"source_graph_schema\":");
        push_json_string(&mut output, &module.source_graph_schema);
        output.push_str(",\"source_revision\":");
        push_json_string(&mut output, &module.source_revision);
        output.push_str(",\"source_digest\":");
        push_json_string(&mut output, &module.source_digest);
        write!(
            output,
            ",\"dependency_depth\":{}}}",
            module.dependency_depth
        )
        .expect("writing to a string cannot fail");
    }
    output.push_str("],\"declarations\":[");
    for (index, declaration) in projection.declarations.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        push_json_string(&mut output, &declaration.id);
        output.push_str(",\"kind\":");
        push_json_string(&mut output, declaration_kind_text(declaration.kind));
        output.push_str(",\"identity_origin\":");
        push_json_string(&mut output, identity_origin_text(declaration.origin));
        output.push_str(",\"owner\":");
        push_optional_json_string(&mut output, declaration.owner.as_deref());
        output.push_str(",\"path\":");
        push_optional_json_string(&mut output, declaration.path.as_deref());
        output.push_str(",\"module\":");
        push_optional_json_string(&mut output, declaration.module.as_deref());
        output.push('}');
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in projection.edges.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"caller_path\":");
        push_json_string(&mut output, &edge.caller_path);
        output.push_str(",\"caller\":");
        push_json_string(&mut output, &edge.caller);
        output.push_str(",\"target_path\":");
        push_json_string(&mut output, &edge.target_path);
        output.push_str(",\"target\":");
        push_json_string(&mut output, &edge.target);
        output.push_str(",\"kind\":");
        push_json_string(&mut output, edge.kind);
        output.push_str(",\"site\":");
        push_json_string(&mut output, edge.site);
        output.push_str(",\"expression\":");
        push_json_string(&mut output, &edge.expression);
        output.push_str(",\"ast_path\":");
        push_json_string(&mut output, &edge.ast_path);
        output.push_str(",\"alias\":");
        push_json_string(&mut output, &edge.alias);
        write!(output, ",\"ordinal\":{}}}", edge.ordinal).expect("writing to a string cannot fail");
    }
    let usage = projection.usage;
    output.push_str("],\"budget\":{");
    write!(
        output,
        "\"used_sources\":{},\"used_total_source_bytes\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_uses\":{},\"used_cross_file_edges\":{},\"used_dependency_depth\":{},\"used_builder_bytes\":{},\"used_manifest_bytes\":{}",
        usage.used_managed_files,
        usage.used_total_source_bytes,
        usage.used_declarations,
        usage.used_callables,
        usage.used_call_sites,
        usage.used_uses,
        usage.used_resolved_cross_file_edges,
        usage.used_dependency_depth,
        usage.used_builder_bytes,
        usage.used_manifest_bytes,
    )
    .expect("writing to a string cannot fail");
    output.push_str("},\"nonclaims\":[");
    for (index, nonclaim) in PROJECT_GRAPH_NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(&mut output, nonclaim);
    }
    output.push_str("]}");
    output.into_string()
}

fn project_artifact_digest(domain: &[u8], payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn render_semantic_graph_with_output_limit(
    mut projection: WorkspaceGraphProjection,
    output_limit: usize,
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>> {
    assert!(
        output_limit <= MAX_OUTPUT_BYTES,
        "private Workspace Semantic Graph output limit cannot exceed the production maximum"
    );
    validate_render_projection(&projection)?;
    let (graph_digest, used_output_bytes) =
        semantic_graph_digest_and_output_bytes(&projection, output_limit)?;
    let json = bounded_graph_json(
        &projection,
        Some(&graph_digest),
        used_output_bytes,
        output_limit,
    )?;
    if json.len() != used_output_bytes {
        return Err(render_binding_error());
    }
    projection.usage.used_output_bytes = used_output_bytes;
    Ok(public_semantic_graph(projection, graph_digest, json))
}

pub(crate) fn projection_graph_binding(
    projection: &WorkspaceGraphProjection,
) -> Result<(String, usize), Vec<Diagnostic>> {
    validate_render_projection(projection)?;
    semantic_graph_digest_and_output_bytes(projection, MAX_OUTPUT_BYTES)
}

pub(crate) fn validate_project_projection(
    projection: &WorkspaceGraphProjection,
) -> Result<(), Vec<Diagnostic>> {
    validate_render_projection(projection)
}

fn semantic_graph_digest_and_output_bytes(
    projection: &WorkspaceGraphProjection,
    output_limit: usize,
) -> Result<(String, usize), Vec<Diagnostic>> {
    const DIGEST_PLACEHOLDER: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let mut used_output_bytes = 0usize;
    for _ in 0..20 {
        let placeholder = bounded_graph_json(
            projection,
            Some(DIGEST_PLACEHOLDER),
            used_output_bytes,
            output_limit,
        )?;
        if placeholder.len() == used_output_bytes {
            let payload = bounded_graph_json(projection, None, used_output_bytes, output_limit)?;
            return Ok((artifact_digest(payload.as_bytes()), used_output_bytes));
        }
        used_output_bytes = placeholder.len();
    }
    Err(render_binding_error())
}

fn public_semantic_graph(
    projection: WorkspaceGraphProjection,
    graph_digest: String,
    json: String,
) -> WorkspaceSemanticGraph {
    let entry_path = projection
        .modules
        .iter()
        .find(|module| module.module == projection.entry_module)
        .expect("validated projection retains its entry module")
        .path
        .clone();
    let modules = projection
        .modules
        .into_iter()
        .map(|module| WorkspaceSemanticGraphModule {
            path: module.path,
            module: module.module,
            source_graph_schema: module.source_graph_schema,
            source_revision: module.source_revision,
            source_digest: module.source_digest,
            dependency_depth: module.dependency_depth,
            permits: module.permits,
        })
        .collect();
    let declarations = projection
        .declarations
        .into_iter()
        .map(|declaration| WorkspaceSemanticGraphDeclaration {
            id: declaration.id,
            kind: declaration_kind_text(declaration.kind),
            identity_origin: identity_origin_text(declaration.origin),
            owner: declaration.owner,
            path: declaration.path,
            module: declaration.module,
        })
        .collect();
    let edges = projection
        .edges
        .into_iter()
        .map(|edge| WorkspaceSemanticGraphEdge {
            caller_path: edge.caller_path,
            caller: edge.caller,
            target_path: edge.target_path,
            target: edge.target,
            kind: edge.kind,
            site: edge.site,
            expression: edge.expression,
            ast_path: edge.ast_path,
            alias: edge.alias,
            ordinal: edge.ordinal,
        })
        .collect();
    let usage = projection.usage;
    WorkspaceSemanticGraph {
        workspace_revision: projection.workspace_revision,
        graph_digest,
        entry: WorkspaceSemanticGraphEntry {
            module: projection.entry_module,
            path: entry_path,
        },
        modules,
        declarations,
        edges,
        limits: WorkspaceSemanticGraphLimits {
            max_managed_files: MAX_FILES,
            max_reachable_modules: MAX_FILES,
            max_entry_module_bytes: MAX_ENTRY_MODULE_BYTES,
            max_total_source_bytes: MAX_TOTAL_SOURCE_BYTES,
            max_declarations: MAX_DECLARATIONS,
            max_callables: MAX_CALLABLES,
            max_call_sites: MAX_CALLS,
            max_uses: MAX_USES,
            max_resolved_cross_file_edges: MAX_CROSS_FILE_EDGES,
            max_dependency_depth: MAX_DEPENDENCY_DEPTH,
            max_builder_bytes: MAX_BUILDER_BYTES,
            max_manifest_bytes: 1024 * 1024,
            max_output_bytes: MAX_OUTPUT_BYTES,
            max_retained_generations: 32,
            max_staging_attempts: 32,
            max_unexpected_inventory_entries: 0,
        },
        budget: WorkspaceSemanticGraphBudget {
            used_managed_files: usage.used_managed_files,
            used_reachable_modules: usage.used_reachable_modules,
            used_entry_module_bytes: usage.used_entry_module_bytes,
            used_total_source_bytes: usage.used_total_source_bytes,
            used_declarations: usage.used_declarations,
            used_callables: usage.used_callables,
            used_call_sites: usage.used_call_sites,
            used_uses: usage.used_uses,
            used_resolved_cross_file_edges: usage.used_resolved_cross_file_edges,
            used_dependency_depth: usage.used_dependency_depth,
            used_builder_bytes: usage.used_builder_bytes,
            used_manifest_bytes: usage.used_manifest_bytes,
            used_output_bytes: usage.used_output_bytes,
            used_retained_generations: usage.used_retained_generations,
            used_staging_attempts: usage.used_staging_attempts,
            used_unexpected_inventory_entries: usage.used_unexpected_inventory_entries,
        },
        json,
    }
}

fn bounded_graph_json(
    projection: &WorkspaceGraphProjection,
    graph_digest: Option<&str>,
    used_output_bytes: usize,
    output_limit: usize,
) -> Result<String, Vec<Diagnostic>> {
    let (json, overflowed) = crate::bounded_output::with_limit(output_limit, || {
        render_graph_json(projection, graph_digest, used_output_bytes)
    });
    if overflowed || json.len() > output_limit {
        Err(vec![limit_error("output_bytes", output_limit)])
    } else {
        Ok(json)
    }
}

fn render_binding_error() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "Workspace Semantic Graph rendering or digest binding disagrees",
    )]
}

fn render_graph_json(
    projection: &WorkspaceGraphProjection,
    graph_digest: Option<&str>,
    used_output_bytes: usize,
) -> String {
    use std::fmt::Write as _;

    let entry = projection
        .modules
        .iter()
        .find(|module| module.module == projection.entry_module)
        .expect("validated projection has exactly one entry module");
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, WORKSPACE_GRAPH_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json_string(&mut output, WORKSPACE_MANIFEST_SCHEMA);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, &projection.workspace_revision);
    if let Some(graph_digest) = graph_digest {
        output.push_str(",\"graph_digest\":");
        push_json_string(&mut output, graph_digest);
    }
    output.push_str(",\"entry\":{\"module\":");
    push_json_string(&mut output, &projection.entry_module);
    output.push_str(",\"path\":");
    push_json_string(&mut output, &entry.path);
    output.push_str("},\"modules\":[");
    for (index, module) in projection.modules.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        push_json_string(&mut output, &module.path);
        output.push_str(",\"module\":");
        push_json_string(&mut output, &module.module);
        output.push_str(",\"source_graph_schema\":");
        push_json_string(&mut output, &module.source_graph_schema);
        output.push_str(",\"source_revision\":");
        push_json_string(&mut output, &module.source_revision);
        output.push_str(",\"source_digest\":");
        push_json_string(&mut output, &module.source_digest);
        write!(
            output,
            ",\"dependency_depth\":{},\"permits\":[",
            module.dependency_depth
        )
        .expect("writing to a string cannot fail");
        for (permit_index, permit) in module.permits.iter().enumerate() {
            if permit_index > 0 {
                output.push(',');
            }
            push_json_string(&mut output, permit);
        }
        output.push_str("]}");
    }
    output.push_str("],\"declarations\":[");
    for (index, declaration) in projection.declarations.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        push_json_string(&mut output, &declaration.id);
        output.push_str(",\"kind\":");
        push_json_string(&mut output, declaration_kind_text(declaration.kind));
        output.push_str(",\"identity_origin\":");
        push_json_string(&mut output, identity_origin_text(declaration.origin));
        output.push_str(",\"owner\":");
        push_optional_json_string(&mut output, declaration.owner.as_deref());
        output.push_str(",\"path\":");
        push_optional_json_string(&mut output, declaration.path.as_deref());
        output.push_str(",\"module\":");
        push_optional_json_string(&mut output, declaration.module.as_deref());
        output.push('}');
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in projection.edges.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"caller_path\":");
        push_json_string(&mut output, &edge.caller_path);
        output.push_str(",\"caller\":");
        push_json_string(&mut output, &edge.caller);
        output.push_str(",\"target_path\":");
        push_json_string(&mut output, &edge.target_path);
        output.push_str(",\"target\":");
        push_json_string(&mut output, &edge.target);
        output.push_str(",\"kind\":");
        push_json_string(&mut output, edge.kind);
        output.push_str(",\"site\":");
        push_json_string(&mut output, edge.site);
        output.push_str(",\"expression\":");
        push_json_string(&mut output, &edge.expression);
        output.push_str(",\"ast_path\":");
        push_json_string(&mut output, &edge.ast_path);
        output.push_str(",\"alias\":");
        push_json_string(&mut output, &edge.alias);
        write!(output, ",\"ordinal\":{}}}", edge.ordinal).expect("writing to a string cannot fail");
    }
    output.push_str("],\"limits\":{\"max_managed_files\":16,\"max_reachable_modules\":16,\"max_entry_module_bytes\":16777216,\"max_total_source_bytes\":16777216,\"max_declarations\":4096,\"max_callables\":1024,\"max_call_sites\":65536,\"max_uses\":4096,\"max_resolved_cross_file_edges\":65536,\"max_dependency_depth\":16,\"max_builder_bytes\":16777216,\"max_manifest_bytes\":1048576,\"max_output_bytes\":16777216,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0},\"budget\":{");
    let usage = projection.usage;
    write!(
        output,
        "\"used_managed_files\":{},\"used_reachable_modules\":{},\"used_entry_module_bytes\":{},\"used_total_source_bytes\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_uses\":{},\"used_resolved_cross_file_edges\":{},\"used_dependency_depth\":{},\"used_builder_bytes\":{},\"used_manifest_bytes\":{},\"used_output_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":{}",
        usage.used_managed_files,
        usage.used_reachable_modules,
        usage.used_entry_module_bytes,
        usage.used_total_source_bytes,
        usage.used_declarations,
        usage.used_callables,
        usage.used_call_sites,
        usage.used_uses,
        usage.used_resolved_cross_file_edges,
        usage.used_dependency_depth,
        usage.used_builder_bytes,
        usage.used_manifest_bytes,
        used_output_bytes,
        usage.used_retained_generations,
        usage.used_staging_attempts,
        usage.used_unexpected_inventory_entries
    )
    .expect("writing to a string cannot fail");
    output.push_str("},\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(&mut output, nonclaim);
    }
    output.push_str("]}");
    output.into_string()
}

fn push_json_string(output: &mut crate::bounded_output::CappedString, value: &str) {
    use std::fmt::Write as _;

    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(output, "\\u{:04x}", character as u32)
                    .expect("writing to a string cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn push_optional_json_string(
    output: &mut crate::bounded_output::CappedString,
    value: Option<&str>,
) {
    if let Some(value) = value {
        push_json_string(output, value);
    } else {
        output.push_str("null");
    }
}

fn declaration_kind_text(kind: hir::DeclarationKind) -> &'static str {
    match kind {
        hir::DeclarationKind::Resource => "resource",
        hir::DeclarationKind::ResourceDrop => "resource_drop",
        hir::DeclarationKind::Record => "record",
        hir::DeclarationKind::Class => "class",
        hir::DeclarationKind::Field => "field",
        hir::DeclarationKind::Variant => "variant",
        hir::DeclarationKind::VariantCase => "variant_case",
        hir::DeclarationKind::CaseField => "case_field",
        hir::DeclarationKind::Interface => "interface",
        hir::DeclarationKind::Import => "import",
        hir::DeclarationKind::Function => "function",
    }
}

fn identity_origin_text(origin: hir::IdentityOrigin) -> &'static str {
    match origin {
        hir::IdentityOrigin::Explicit => "explicit",
        hir::IdentityOrigin::Automatic => "automatic",
        hir::IdentityOrigin::CompilerOwned => "compiler_owned",
    }
}

fn validate_render_projection(
    projection: &WorkspaceGraphProjection,
) -> Result<(), Vec<Diagnostic>> {
    if projection
        .modules
        .iter()
        .filter(|module| module.module == projection.entry_module)
        .count()
        != 1
        || projection.modules.len() != projection.usage.used_reachable_modules
        || !projection
            .modules
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
        || !projection
            .declarations
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
        || !projection.edges.windows(2).all(|pair| pair[0] < pair[1])
        || projection.edges.iter().any(|edge| {
            !matches!(
                edge.kind,
                "call"
                    | "capability_authority"
                    | "effect_requirement"
                    | "function_import"
                    | "type_import"
                    | "type_reference"
            ) || !matches!(
                edge.site,
                "module" | "type" | "requires" | "body" | "ensures"
            )
        })
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "Workspace Semantic Graph rendering or digest binding disagrees",
        )]);
    }
    Ok(())
}

fn artifact_digest(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ARTIFACT_DIGEST_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn build_owned_with_builder_limit(
    sources: Vec<WorkspaceSource>,
    builder_limit: usize,
) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    build_owned_retaining_sources_with_builder_limit(sources, builder_limit, None, false)
        .map(|(build, _)| build)
}

pub(crate) fn build_owned_retaining_sources(
    sources: Vec<WorkspaceSource>,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    build_owned_retaining_sources_with_builder_limit(sources, MAX_BUILDER_BYTES, None, false)
}

pub(crate) fn build_owned_retaining_sources_for_change(
    sources: Vec<WorkspaceSource>,
    change_builder_limit: usize,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    assert!(change_builder_limit <= MAX_CHANGE_BUILDER_BYTES);
    build_owned_retaining_sources_with_builder_limit(
        sources,
        MAX_BUILDER_BYTES,
        Some(change_builder_limit),
        false,
    )
}

pub(crate) fn build_owned_retaining_sources_for_operations(
    sources: Vec<WorkspaceSource>,
    graph_builder_limit: usize,
    operations_builder_limit: usize,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    assert!(
        operations_builder_limit <= 67_108_864,
        "private Semantic Workspace Operations builder limit cannot exceed the production maximum"
    );
    build_owned_retaining_sources_with_builder_limit(
        sources,
        graph_builder_limit,
        Some(operations_builder_limit),
        true,
    )
}

fn build_owned_retaining_sources_with_builder_limit(
    sources: Vec<WorkspaceSource>,
    builder_limit: usize,
    change_builder_limit: Option<usize>,
    retain_operation_programs: bool,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    build_owned_retaining_sources_with_frontend_limit(
        sources,
        builder_limit,
        change_builder_limit,
        retain_operation_programs,
        None,
    )
}

pub(crate) fn build_owned_retaining_sources_with_frontend(
    sources: Vec<WorkspaceSource>,
    frontend: &mut crate::project::incremental::FrontendPass,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    build_owned_retaining_sources_with_frontend_limit(
        sources,
        MAX_BUILDER_BYTES,
        None,
        false,
        Some(frontend),
    )
}

fn build_owned_retaining_sources_with_frontend_limit(
    sources: Vec<WorkspaceSource>,
    builder_limit: usize,
    change_builder_limit: Option<usize>,
    retain_operation_programs: bool,
    frontend: Option<&mut crate::project::incremental::FrontendPass>,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    assert!(
        builder_limit <= MAX_BUILDER_BYTES,
        "private Workspace Semantic Graph builder limit cannot exceed the production maximum"
    );
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_BUILDER_LIMIT.with(|active| active.set(self.0));
        }
    }
    let previous = ACTIVE_BUILDER_LIMIT.with(|active| active.replace(builder_limit));
    let restore = Restore(previous);
    let result = build_owned_inner(
        sources,
        change_builder_limit,
        retain_operation_programs,
        frontend,
    );
    drop(restore);
    result
}

fn active_builder_limit() -> usize {
    ACTIVE_BUILDER_LIMIT.with(Cell::get)
}

fn build_owned_inner(
    mut sources: Vec<WorkspaceSource>,
    change_builder_limit: Option<usize>,
    retain_operation_programs: bool,
    mut frontend: Option<&mut crate::project::incremental::FrontendPass>,
) -> Result<(WorkspaceGraphBuild, Vec<WorkspaceSource>), Vec<Diagnostic>> {
    if sources.len() < 2 {
        return Err(vec![graph_error(
            "SPX-G170",
            "Workspace Semantic Graph requires 2..16 source files",
        )]);
    }
    if sources.len() > MAX_FILES {
        return Err(vec![limit_error("files", MAX_FILES)]);
    }
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let mut seen_paths = BTreeSet::new();
    let mut source_bytes = 0usize;
    for source in &sources {
        workspace::validate_logical_path(&source.path).map_err(|_| {
            vec![graph_error(
                "SPX-G170",
                format!(
                    "workspace semantic source path `{}` is not canonical",
                    source.path
                ),
            )]
        })?;
        if !seen_paths.insert(source.path.clone()) {
            return Err(vec![graph_error(
                "SPX-G170",
                format!("duplicate workspace semantic source path `{}`", source.path),
            )]);
        }
        source_bytes = checked_usage(
            source_bytes,
            source.source.len(),
            "total_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        )?;
    }

    let mut programs = Vec::with_capacity(sources.len());
    let mut declarations = 0usize;
    let mut callables = 0usize;
    let mut calls = 0usize;
    let mut uses = 0usize;
    let mut canonical_bytes = 0usize;
    for source in &sources {
        let cached = frontend
            .as_deref_mut()
            .and_then(|cache| cache.lookup(&source.path, &source.source));
        let reused = cached.is_some();
        let program = if let Some(program) = cached {
            program
        } else {
            let program =
                parse(&source.source, Path::new(&source.path)).map_err(|error| vec![error])?;
            if let Some(frontend) = frontend.as_deref_mut() {
                frontend.parsed(source.source.len());
            }
            program
        };
        // Check source-local conformance before imported declarations become
        // synthetic stubs. A stub must never acquire local implementation
        // authority merely because it has an authenticated imported identity.
        crate::static_protocol::validate(&program).map_err(|error| vec![error])?;
        let remaining = active_builder_limit().saturating_sub(canonical_bytes);
        // Cached entries originate only from exact canonical source and a
        // successful complete Project build. Preserve cold byte accounting
        // while actually avoiding this formatter invocation on a cache hit.
        let canonical_len = if reused {
            source.source.len()
        } else {
            let (canonical, overflowed) =
                crate::bounded_output::with_limit(remaining, || format::canonical(&program));
            if overflowed {
                return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
            }
            if canonical != source.source {
                return Err(vec![graph_error(
                    "SPX-G170",
                    format!(
                        "workspace semantic source `{}` is not canonical",
                        source.path
                    ),
                )]);
            }
            if let Some(frontend) = frontend.as_deref_mut() {
                frontend.canonicalized();
            }
            canonical.len()
        };
        canonical_bytes = checked_usage(
            canonical_bytes,
            canonical_len,
            "builder_bytes",
            active_builder_limit(),
        )?;
        declarations = checked_usage(
            declarations,
            declaration_count(&program)
                .ok_or_else(|| vec![limit_error("declarations", MAX_DECLARATIONS)])?,
            "declarations",
            MAX_DECLARATIONS,
        )?;
        callables = checked_usage(
            callables,
            program.functions.len(),
            "callables",
            MAX_CALLABLES,
        )?;
        calls = checked_usage(
            calls,
            call_count(&program).ok_or_else(|| vec![limit_error("calls", MAX_CALLS)])?,
            "calls",
            MAX_CALLS,
        )?;
        uses = checked_usage(uses, program.module_uses.len(), "uses", MAX_USES)?;
        programs.push(program);
    }

    crate::static_protocol::validate_workspace(&programs).map_err(|error| vec![error])?;
    let module_paths = index_modules(&programs)?;
    let authored = index_authored(&programs)?;
    validate_synthetic_main_id_collisions(&programs, &authored)?;
    validate_uses(&programs, &module_paths, &authored)?;
    let dependency_depths = validate_dependency_dag(&programs)?;
    let mut resolve_builder_bytes = 0usize;
    let mut runtime_builder_bytes = 0usize;
    for program in &programs {
        let costs = synthetic_builder_bytes(program, &authored, &programs)?;
        resolve_builder_bytes = checked_usage(
            resolve_builder_bytes,
            costs.raw_clone_and_hir,
            "builder_bytes",
            active_builder_limit(),
        )?;
        runtime_builder_bytes = checked_usage(
            runtime_builder_bytes,
            costs.runtime,
            "builder_bytes",
            active_builder_limit(),
        )?;
    }
    let checked_retention_prebound = checked_usage(
        resolve_builder_bytes,
        runtime_builder_bytes,
        "builder_bytes",
        active_builder_limit(),
    )?;
    if let Some(cache) = frontend.as_deref() {
        cache.checked_retention_prebound(checked_retention_prebound)?;
    }
    let (core, overflowed, core_builder_bytes) =
        crate::bounded_output::with_limit_usage(active_builder_limit(), || {
            charge_builder_prebound(resolve_builder_bytes)?;
            build_resolved_core(
                &programs,
                &module_paths,
                &dependency_depths,
                &authored,
                frontend.as_deref_mut(),
            )
        });
    if overflowed {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    let (modules, module_paths, dependency_depths, declaration_facts, expected_edges) = core?;
    let dependency_depth = dependency_depths.values().copied().max().unwrap_or(0);
    let resolved_cross_file_edges = expected_edges.len();
    let graph_builder_bytes = canonical_bytes.max(core_builder_bytes);
    let (change_fingerprints, operation_sidecar, change_builder_bytes, operation_builder_bytes) =
        if let Some(change_builder_limit) = change_builder_limit {
            let (result, overflowed, consumed) =
                crate::bounded_output::with_limit_usage(change_builder_limit, || {
                    if retain_operation_programs {
                        reserve_builder_structure(graph_builder_bytes)?;
                    }
                    let fingerprints = authenticated_declaration_fingerprints(
                        &programs,
                        &sources,
                        &declaration_facts,
                    )?;
                    let sidecar = retain_operation_programs
                        .then(|| build_operation_sidecar(&programs, &sources, &modules, &authored))
                        .transpose()?;
                    Ok::<_, Vec<Diagnostic>>((fingerprints, sidecar))
                });
            if overflowed {
                return Err(vec![limit_error(
                    "change_builder_bytes",
                    change_builder_limit,
                )]);
            }
            let (fingerprints, sidecar) = result?;
            (
                Some(fingerprints),
                sidecar,
                consumed,
                if retain_operation_programs {
                    consumed
                } else {
                    0
                },
            )
        } else {
            (None, None, 0, 0)
        };
    let build = WorkspaceGraphBuild {
        hir: ValidatedWorkspaceHir {
            modules,
            module_paths,
            dependency_depths,
            declarations: declaration_facts,
            shared_prelude_ids: prelude::all_ids().into_iter().collect(),
        },
        edges: expected_edges,
        usage: WorkspaceGraphWorkUsage {
            managed_files: sources.len(),
            total_source_bytes: source_bytes,
            declarations,
            callables,
            call_sites: calls,
            uses,
            resolved_cross_file_edges,
            dependency_depth,
            builder_bytes: graph_builder_bytes,
        },
        change_fingerprints,
        change_builder_bytes,
        operation_sidecar,
        operation_builder_bytes,
    };
    if let Some(frontend) = frontend {
        frontend.retain(&sources, programs, resolve_builder_bytes)?;
    }
    Ok((build, sources))
}

type ResolvedCore = (
    Vec<WorkspaceResolvedModule>,
    BTreeMap<String, String>,
    BTreeMap<String, usize>,
    BTreeMap<String, WorkspaceDeclarationFact>,
    Vec<WorkspaceEdge>,
);

// Native Rust name resolution adds a private declaration-index map, but this
// Graph route rejects Native Rust imports before resolution. Preserve the
// frozen no-native workspace accounting bytes rather than charging an empty,
// backend-private map into existing Graph evidence.
const GRAPH_ACCOUNTED_RESOLVED_PROGRAM_BYTES: usize = std::mem::size_of::<hir::ResolvedProgram>()
    - std::mem::size_of::<BTreeMap<String, hir::DeclarationId>>();
// Shared Loan Plan v1 is an independently bounded proof sidecar. Preserve the
// frozen empty-sidecar accounting for legacy workspaces, then charge the full
// carrier and its owned allocation only when a retained function has a real
// own-root plan. This keeps evidence independent of incidental Rust layout
// growth without making nonempty proof data free.
const GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES: usize = std::mem::size_of::<hir::ResolvedFunction>()
    - std::mem::size_of::<crate::loan_plan::LoanPlan>();
const GRAPH_ACCOUNTED_RESOLVED_FUNCTION_INSTANCE_BYTES: usize =
    std::mem::size_of::<hir::ResolvedFunctionInstance>()
        - std::mem::size_of::<crate::loan_plan::LoanPlan>();

fn build_resolved_core(
    programs: &[Program],
    module_paths: &BTreeMap<&str, &str>,
    dependency_depths: &BTreeMap<&str, usize>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    mut frontend: Option<&mut crate::project::incremental::FrontendPass>,
) -> Result<ResolvedCore, Vec<Diagnostic>> {
    let mut expected_edges = Vec::new();
    for program in programs {
        collect_expected_edges(program, module_paths, authored, &mut expected_edges)?;
    }
    reserve_builder_structure(
        programs
            .len()
            .checked_mul(std::mem::size_of::<String>() + GRAPH_ACCOUNTED_RESOLVED_PROGRAM_BYTES)
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?,
    )?;
    let mut synthetic_modules = Vec::with_capacity(programs.len());
    for program in programs {
        let synthetic = synthetic_program(program, authored, programs)?;
        // Exact stubs/spans and the unconditional prebound govern hits.
        let cached = frontend
            .as_deref_mut()
            .and_then(|cache| cache.checked_module(&program.path, &synthetic));
        let resolved = if let Some((resolved, resolver_bytes, original_loan_bytes)) = cached {
            let before = crate::bounded_output::active_remaining()
                .expect("resolved core has a builder budget");
            hir::validate(&resolved).map_err(|error| vec![error])?;
            let validation_bytes = before.saturating_sub(
                crate::bounded_output::active_remaining()
                    .expect("resolved core has a builder budget"),
            );
            let residual = resolver_bytes
                .checked_sub(validation_bytes)
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G257",
                        "checked module replay accounting exceeds its original resolver work",
                    )]
                })?;
            // Revalidation already charged its real cost. Charge only the
            // remainder of the original resolver cost to preserve cold graph
            // evidence and admission limits without double-charging validation.
            charge_builder_prebound(residual)?;
            // Rust Clone may shrink owned proof-vector/string capacities. The
            // filters below charge the cloned proof's real capacity; preserve
            // the original cold charge by accounting only the lost capacity.
            let loan_residual = original_loan_bytes
                .checked_sub(resolved_loan_bytes(&resolved)?)
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G257",
                        "checked module clone exceeds its original loan-plan capacity accounting",
                    )]
                })?;
            charge_builder_prebound(loan_residual)?;
            resolved
        } else {
            let before = crate::bounded_output::active_remaining()
                .expect("resolved core has a builder budget");
            let selective = frontend
                .as_deref()
                .and_then(|cache| cache.resolve_functions(&program.path, &synthetic));
            let (resolved, function_costs, reused_functions) = if let Some(selective) = selective {
                selective?.into_parts()
            } else {
                (hir::resolve(&synthetic)?, BTreeMap::new(), 0)
            };
            let resolver_bytes = before.saturating_sub(
                crate::bounded_output::active_remaining()
                    .expect("resolved core has a builder budget"),
            );
            if let Some(cache) = frontend.as_deref_mut() {
                cache.resolved_module(
                    &program.path,
                    synthetic,
                    &resolved,
                    resolver_bytes,
                    resolved_loan_bytes(&resolved)?,
                    function_costs,
                    reused_functions,
                );
            }
            resolved
        };
        verify_resolved_call_edges(program, &resolved, authored)?;
        synthetic_modules.push((
            crate::bounded_output::budgeted_clone(&program.module),
            resolved,
        ));
    }
    validate_stub_signatures(programs, &synthetic_modules)?;
    let declarations = reconstruct_workspace_declaration_facts(&synthetic_modules, programs)?;
    let programs_by_module = programs
        .iter()
        .map(|program| (program.module.as_str(), program))
        .collect::<BTreeMap<_, _>>();
    reserve_builder_structure(
        synthetic_modules
            .len()
            // Empty private signature facts must not change frozen scalar
            // graph accounting. Nonempty carriers are charged separately.
            .checked_mul(
                std::mem::size_of::<WorkspaceResolvedModule>()
                    - std::mem::size_of::<BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>>(
                    ),
            )
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?,
    )?;
    let mut modules = Vec::with_capacity(synthetic_modules.len());
    for (module, resolved) in synthetic_modules {
        let program = programs_by_module
            .get(module.as_str())
            .expect("every resolved module has authenticated source");
        let types = filter_owned_vec(resolved.types, |item| {
            authored
                .get(item.id.as_str())
                .is_some_and(|owner| owner.module == program.module)
        })?;
        let functions = filter_owned_vec_accounted(
            resolved.functions,
            GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES,
            |item| retained_loan_plan_bytes(&item.loan_plan),
            |item| {
                authored
                    .get(item.id.as_str())
                    .is_some_and(|owner| owner.module == program.module)
            },
        )?;
        let function_templates = filter_owned_vec(resolved.function_templates, |item| {
            authored
                .get(item.id.as_str())
                .is_some_and(|owner| owner.module == program.module)
        })?;
        let function_instances = filter_owned_vec_accounted(
            resolved.function_instances,
            GRAPH_ACCOUNTED_RESOLVED_FUNCTION_INSTANCE_BYTES,
            |item| retained_loan_plan_bytes(&item.function.loan_plan),
            |item| {
                authored
                    .get(item.template.as_str())
                    .is_some_and(|owner| owner.module == program.module)
            },
        )?;
        let signature_types = retained_signature_type_facts(&functions, &resolved.declarations)?;
        modules.push(WorkspaceResolvedModule {
            path: crate::bounded_output::budgeted_clone(&program.path),
            module,
            permits: resolved.permits,
            types,
            interfaces: resolved.interfaces,
            functions,
            function_templates,
            function_instances,
            signature_types,
        });
    }
    validate_retained_facts(programs, &modules, &expected_edges)?;
    validate_retained_declaration_shapes(&modules, &declarations)?;
    let owned_module_paths = module_paths
        .iter()
        .map(|(module, path)| {
            (
                crate::bounded_output::budgeted_clone(module),
                crate::bounded_output::budgeted_clone(path),
            )
        })
        .collect();
    if dependency_depths.len() != module_paths.len()
        || dependency_depths
            .keys()
            .any(|module| !module_paths.contains_key(module))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace dependency-depth keys disagree with authenticated modules",
        )]);
    }
    let owned_dependency_depths = dependency_depths
        .iter()
        .map(|(module, depth)| (crate::bounded_output::budgeted_clone(module), *depth))
        .collect();
    render_edge_proof(&expected_edges);
    Ok((
        modules,
        owned_module_paths,
        owned_dependency_depths,
        declarations,
        expected_edges,
    ))
}

fn retained_signature_type_facts(
    functions: &[hir::ResolvedFunction],
    declarations: &hir::DeclarationIndex,
) -> Result<BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>, Vec<Diagnostic>> {
    let mut retained = BTreeMap::new();
    let mut visits = 0usize;
    if !functions.is_empty() {
        reserve_builder_structure(std::mem::size_of::<
            [Option<(CheckedValueNode<'_>, usize)>; MAX_CHECKED_VALUE_DEPTH + 1],
        >())?;
    }
    for function in functions {
        for ty in function
            .params
            .iter()
            .map(|parameter| &parameter.ty)
            .chain(std::iter::once(&function.return_type))
        {
            retain_checked_value_types(
                CheckedValueNode::Type(ty),
                declarations,
                &mut retained,
                &mut visits,
            )?;
        }
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            retain_checked_value_types(
                CheckedValueNode::Expression(expression),
                declarations,
                &mut retained,
                &mut visits,
            )?;
        }
    }
    Ok(retained)
}

const MAX_CHECKED_VALUE_VISITS: usize = 1_048_576;
const MAX_CHECKED_VALUE_DEPTH: usize = 256;

#[derive(Clone, Copy)]
enum CheckedValueNode<'a> {
    Type(&'a hir::ResolvedType),
    Expression(&'a hir::ResolvedExpr),
    Statement(&'a hir::ResolvedStatement),
    Binding(&'a hir::ResolvedBinding),
    Field(&'a hir::ResolvedFieldInitializer),
    Arm(&'a hir::ResolvedMatchArm),
    Pattern(&'a hir::ResolvedMatchPattern),
    RecordField(&'a hir::ResolvedRecordMatchPatternField),
}

impl<'a> CheckedValueNode<'a> {
    fn ty(self) -> Option<&'a hir::ResolvedType> {
        match self {
            Self::Type(ty) => Some(ty),
            Self::Expression(expression) => Some(&expression.ty),
            Self::Binding(binding) => Some(&binding.ty),
            Self::Pattern(hir::ResolvedMatchPattern::Record { instance, .. }) => Some(instance),
            Self::RecordField(hir::ResolvedRecordMatchPatternField {
                pattern: hir::ResolvedRecordMatchFieldPattern::Record { instance, .. },
                ..
            }) => Some(instance),
            _ => None,
        }
    }

    fn child(self, index: usize) -> Option<Self> {
        use hir::ResolvedExprKind as E;
        use hir::ResolvedMatchPattern as P;
        use hir::ResolvedRecordMatchFieldPattern as F;
        use hir::ResolvedStatement as S;
        match self {
            Self::Type(_) | Self::Binding(_) => None,
            Self::Field(field) => (index == 0).then_some(Self::Expression(&field.value)),
            Self::Statement(statement) => match statement {
                S::Let { binding, value, .. } | S::Assign { binding, value, .. } => {
                    [Self::Binding(binding), Self::Expression(value)]
                        .get(index)
                        .copied()
                }
                S::Unsafe { .. } | S::While { .. } => statement.child(index).map(Self::Expression),
            },
            Self::Arm(arm) => match index {
                0 => Some(Self::Pattern(&arm.pattern)),
                1 => Some(Self::Expression(arm.guard.as_deref().unwrap_or(&arm.value))),
                2 if arm.guard.is_some() => Some(Self::Expression(&arm.value)),
                _ => None,
            },
            Self::Pattern(pattern) => match pattern {
                P::Variant { fields, .. } => {
                    fields.get(index).map(|field| Self::Binding(&field.binding))
                }
                P::Record { fields, .. } => fields.get(index).map(Self::RecordField),
                P::Binding(binding) => (index == 0).then_some(Self::Binding(binding)),
                P::Or(alternatives) => alternatives.get(index).map(Self::Pattern),
                P::Wildcard | P::Literal(_) => None,
            },
            Self::RecordField(field) => match &field.pattern {
                F::Binding(binding) => (index == 0).then_some(Self::Binding(binding)),
                F::Record { fields, .. } => fields.get(index).map(Self::RecordField),
                F::Wildcard => None,
            },
            Self::Expression(expression) => match &expression.kind {
                E::ByteRange {
                    source, start, end, ..
                } => [source.as_ref(), start.as_ref(), end.as_ref()]
                    .get(index)
                    .copied()
                    .map(Self::Expression),
                E::Call { args, .. } => args.get(index).map(Self::Expression),
                E::NativeRustImportCall(call) => call.args.get(index).map(Self::Expression),
                E::HostCommandCall(call) => call.args.get(index).map(Self::Expression),
                E::Unary { value, .. }
                | E::Project { base: value, .. }
                | E::Upcast { source: value } => (index == 0).then_some(Self::Expression(value)),
                E::Try {
                    operand,
                    residual_type,
                    ..
                }
                | E::TryOption {
                    operand,
                    residual_type,
                    ..
                } => [Self::Expression(operand), Self::Type(residual_type)]
                    .get(index)
                    .copied(),
                E::Binary { left, right, .. } => [left.as_ref(), right.as_ref()]
                    .get(index)
                    .copied()
                    .map(Self::Expression),
                E::Block { statements, tail } => {
                    if index < statements.len() {
                        statements.get(index).map(Self::Statement)
                    } else {
                        (index == statements.len()).then_some(Self::Expression(tail))
                    }
                }
                E::If {
                    condition,
                    then_branch,
                    else_branch,
                } => [
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref(),
                ]
                .get(index)
                .copied()
                .map(Self::Expression),
                E::ConstructRecord { fields, .. } | E::ConstructVariant { fields, .. } => {
                    fields.get(index).map(Self::Field)
                }
                E::Match {
                    scrutinee, arms, ..
                } => {
                    if index == 0 {
                        Some(Self::Expression(scrutinee))
                    } else {
                        arms.get(index - 1).map(Self::Arm)
                    }
                }
                E::UpdateRecord { base, fields, .. } => {
                    if index == 0 {
                        Some(Self::Expression(base))
                    } else {
                        fields.get(index - 1).map(Self::Field)
                    }
                }
                E::Int(_)
                | E::Int32(_)
                | E::Char(_)
                | E::Uint8(_)
                | E::Usize(_)
                | E::Float32(_)
                | E::Float64(_)
                | E::Bool(_)
                | E::String(_)
                | E::ArrayU8(_)
                | E::RepeatArrayU8 { .. }
                | E::BorrowPlace { .. }
                | E::Place(_) => None,
            },
        }
    }
}

fn retain_checked_value_types(
    root: CheckedValueNode<'_>,
    declarations: &hir::DeclarationIndex,
    retained: &mut BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>,
    visits: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    // One cursor per active ancestor: wide statement/pattern lists cannot
    // allocate an unbounded sibling queue. This fixed scratch stack is not
    // retained; its peak storage is charged once by the inventory entry point.
    let mut stack = [None; MAX_CHECKED_VALUE_DEPTH + 1];
    stack[0] = Some((root, 0usize));
    let mut depth = 0usize;
    loop {
        let (node, next) = stack[depth].expect("active checked value cursor");
        if next == 0 {
            if *visits >= MAX_CHECKED_VALUE_VISITS {
                return Err(vec![limit_error(
                    "checked_value_visits",
                    MAX_CHECKED_VALUE_VISITS,
                )]);
            }
            *visits += 1;
            if let Some(ty) = node.ty() {
                retain_checked_nominal_type(ty, declarations, retained)?;
            }
        }
        if let Some(child) = node.child(next) {
            if depth == MAX_CHECKED_VALUE_DEPTH {
                return Err(vec![limit_error(
                    "checked_value_depth",
                    MAX_CHECKED_VALUE_DEPTH,
                )]);
            }
            stack[depth] = Some((node, next + 1));
            depth += 1;
            stack[depth] = Some((child, 0));
        } else if depth == 0 {
            break;
        } else {
            stack[depth] = None;
            depth -= 1;
        }
    }
    Ok(())
}

fn retain_checked_nominal_type(
    ty: &hir::ResolvedType,
    declarations: &hir::DeclarationIndex,
    retained: &mut BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>,
) -> Result<(), Vec<Diagnostic>> {
    let hir::ResolvedType::Nominal { declaration, .. } = ty else {
        return Ok(());
    };
    let key = ty.identity_key();
    if retained.contains_key(&key) {
        return Ok(());
    }
    if retained.len() >= MAX_DECLARATIONS {
        return Err(vec![limit_error("declarations", MAX_DECLARATIONS)]);
    }
    let kind = declarations
        .declaration(declaration)
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "checked value nominal declaration is absent",
            )]
        })?
        .kind;
    let facts = declarations.type_facts(ty).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "checked value type facts are absent",
        )]
    })?;
    let base = if retained.is_empty() {
        std::mem::size_of::<BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>>()
    } else {
        0
    };
    let bytes = base
        .checked_add(
            std::mem::size_of::<(String, hir::DeclarationKind, hir::TypeFacts)>()
                + 8 * std::mem::size_of::<usize>(),
        )
        .and_then(|bytes| bytes.checked_add(key.capacity()))
        .and_then(|bytes| bytes.checked_add(facts.layout_key.capacity()))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(bytes)?;
    retained.insert(key, (kind, facts));
    Ok(())
}

fn filter_owned_vec<T>(
    items: Vec<T>,
    mut keep: impl FnMut(&T) -> bool,
) -> Result<Vec<T>, Vec<Diagnostic>> {
    reserve_builder_structure(
        items
            .len()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?,
    )?;
    Ok(items.into_iter().filter(|item| keep(item)).collect())
}

fn filter_owned_vec_accounted<T>(
    items: Vec<T>,
    fixed_element_bytes: usize,
    mut extra_owned_bytes: impl FnMut(&T) -> Result<usize, Vec<Diagnostic>>,
    mut keep: impl FnMut(&T) -> bool,
) -> Result<Vec<T>, Vec<Diagnostic>> {
    let fixed = items
        .len()
        .checked_mul(fixed_element_bytes)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let mut bytes = fixed;
    for item in &items {
        bytes = bytes
            .checked_add(extra_owned_bytes(item)?)
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    }
    reserve_builder_structure(bytes)?;
    Ok(items.into_iter().filter(|item| keep(item)).collect())
}

fn resolved_loan_bytes(program: &hir::ResolvedProgram) -> Result<usize, Vec<Diagnostic>> {
    program
        .functions
        .iter()
        .map(|function| &function.loan_plan)
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function.loan_plan),
        )
        .try_fold(0usize, |sum, plan| {
            sum.checked_add(retained_loan_plan_bytes(plan)?)
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
        })
}

fn retained_loan_plan_bytes(plan: &crate::loan_plan::LoanPlan) -> Result<usize, Vec<Diagnostic>> {
    if plan.loans.is_empty() {
        return Ok(0);
    }
    crate::loan_plan::owned_capacity_bytes(plan)
        .and_then(|owned| std::mem::size_of::<crate::loan_plan::LoanPlan>().checked_add(owned))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

fn authenticated_declaration_fingerprints(
    programs: &[Program],
    sources: &[WorkspaceSource],
    declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let sources = sources
        .iter()
        .map(|source| (source.path.as_str(), source.source.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut fingerprints = BTreeMap::new();
    for program in programs {
        let source = sources.get(program.path.as_str()).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace declaration fingerprint source is absent",
            )]
        })?;
        for declaration in &program.types {
            insert_declaration_fingerprint(
                &mut fingerprints,
                &declaration.stable_id,
                "type",
                declaration.span,
                source,
            )?;
            match &declaration.kind {
                TypeDeclarationKind::Resource { lifecycles } => {
                    for lifecycle in lifecycles {
                        let id = match &lifecycle.stable_id {
                            Some(id) => id.as_str(),
                            None => declarations
                                .iter()
                                .find(|(_, fact)| {
                                    fact.kind == hir::DeclarationKind::ResourceDrop
                                        && fact.owner.as_deref()
                                            == Some(declaration.stable_id.as_str())
                                        && fact.path.as_deref() == Some(program.path.as_str())
                                })
                                .map(|(id, _)| id.as_str())
                                .ok_or_else(|| {
                                    vec![graph_error(
                                        "SPX-G173",
                                        "automatic resource-drop fingerprint identity is absent",
                                    )]
                                })?,
                        };
                        insert_declaration_fingerprint(
                            &mut fingerprints,
                            id,
                            "resource_drop",
                            lifecycle.span,
                            source,
                        )?;
                    }
                }
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        insert_declaration_fingerprint(
                            &mut fingerprints,
                            &field.stable_id,
                            "field",
                            field.span,
                            source,
                        )?;
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        insert_declaration_fingerprint(
                            &mut fingerprints,
                            &case.stable_id,
                            "variant_case",
                            case.span,
                            source,
                        )?;
                        for field in &case.fields {
                            insert_declaration_fingerprint(
                                &mut fingerprints,
                                &field.stable_id,
                                "case_field",
                                field.span,
                                source,
                            )?;
                        }
                    }
                }
            }
        }
        for interface in &program.interfaces {
            insert_declaration_fingerprint(
                &mut fingerprints,
                &interface.stable_id,
                "interface",
                interface.span,
                source,
            )?;
            for import in &interface.imports {
                insert_declaration_fingerprint(
                    &mut fingerprints,
                    &import.stable_id,
                    "import",
                    import.span,
                    source,
                )?;
            }
        }
        for function in &program.functions {
            insert_declaration_fingerprint(
                &mut fingerprints,
                &function.stable_id,
                "function",
                function.span,
                source,
            )?;
        }
    }
    let expected = declarations
        .iter()
        .filter(|(_, fact)| fact.origin != hir::IdentityOrigin::CompilerOwned)
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    if fingerprints.len() != expected.len()
        || fingerprints
            .keys()
            .any(|id| !expected.contains(id.as_str()))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace declaration semantic fingerprint identities disagree",
        )]);
    }
    Ok(fingerprints)
}

fn insert_declaration_fingerprint(
    fingerprints: &mut BTreeMap<String, String>,
    id: &str,
    kind: &'static str,
    span: Span,
    source: &str,
) -> Result<(), Vec<Diagnostic>> {
    let bytes = source.get(span.start..span.end).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "workspace declaration fingerprint span is outside authenticated source",
        )]
    })?;
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.workspace-semantic-change.declaration-fingerprint.v1\0");
    hasher.update((kind.len() as u64).to_le_bytes());
    hasher.update(kind.as_bytes());
    hasher.update((id.len() as u64).to_le_bytes());
    hasher.update(id.as_bytes());
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes.as_bytes());
    reserve_builder_structure(std::mem::size_of::<(String, String)>())?;
    let id = crate::bounded_output::budgeted_clone(id);
    let fingerprint = crate::bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ));
    if fingerprints.insert(id, fingerprint).is_some() {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace declaration semantic fingerprint identity is duplicated",
        )]);
    }
    Ok(())
}

fn charge_builder_prebound(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(bytes)
}

fn render_edge_proof(edges: &[WorkspaceEdge]) {
    let mut proof = crate::bounded_output::CappedString::new();
    use std::fmt::Write as _;
    for edge in edges {
        writeln!(
            proof,
            "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            edge.caller_path,
            edge.caller,
            edge.target_path,
            edge.target,
            edge.kind,
            edge.site,
            edge.expression,
            edge.ast_path,
            edge.alias,
            edge.ordinal,
        )
        .expect("CappedString formatting is infallible");
    }
}

fn checked_usage(
    used: usize,
    additional: usize,
    field: &'static str,
    maximum: usize,
) -> Result<usize, Vec<Diagnostic>> {
    let next = used
        .checked_add(additional)
        .ok_or_else(|| vec![limit_error(field, maximum)])?;
    if next > maximum {
        return Err(vec![limit_error(field, maximum)]);
    }
    Ok(next)
}

fn declaration_count(program: &Program) -> Option<usize> {
    let mut count = program
        .functions
        .len()
        .checked_add(program.interfaces.len())?;
    for interface in &program.interfaces {
        count = count.checked_add(interface.imports.len())?;
    }
    for ty in &program.types {
        count = count.checked_add(1)?;
        match &ty.kind {
            TypeDeclarationKind::Resource { lifecycles } => {
                count = count.checked_add(lifecycles.len())?;
            }
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                count = count.checked_add(fields.len())?
            }
            TypeDeclarationKind::Variant { cases } => {
                count = count.checked_add(cases.len())?;
                for case in cases {
                    count = count.checked_add(case.fields.len())?;
                }
            }
        }
    }
    Some(count)
}

fn call_count(program: &Program) -> Option<usize> {
    let mut count = Some(0usize);
    for function in &program.functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            expression.visit_calls(&mut |_, _| {
                count = count.and_then(|value| value.checked_add(1));
            });
        }
    }
    count
}

fn index_modules(programs: &[Program]) -> Result<BTreeMap<&str, &str>, Vec<Diagnostic>> {
    let mut modules = BTreeMap::new();
    for program in programs {
        if let Some(existing) = modules.insert(program.module.as_str(), program.path.as_str()) {
            return Err(vec![graph_error(
                "SPX-G172",
                format!(
                    "workspace module `{}` is declared by both `{existing}` and `{}`",
                    program.module, program.path
                ),
            )]);
        }
    }
    Ok(modules)
}

fn index_authored(
    programs: &[Program],
) -> Result<BTreeMap<&str, AuthoredDeclaration<'_>>, Vec<Diagnostic>> {
    let mut declarations = BTreeMap::new();
    for program in programs {
        for function in &program.functions {
            insert_authored(
                &mut declarations,
                &function.stable_id,
                AuthoredDeclaration {
                    path: &program.path,
                    module: &program.module,
                    explicit: function.explicit_id,
                    kind: AuthoredKind::Function,
                    function: Some(function),
                    ty: None,
                },
            )?;
        }
        for ty in &program.types {
            insert_authored(
                &mut declarations,
                &ty.stable_id,
                AuthoredDeclaration {
                    path: &program.path,
                    module: &program.module,
                    explicit: ty.explicit_id,
                    kind: AuthoredKind::Type,
                    function: None,
                    ty: Some(ty),
                },
            )?;
            match &ty.kind {
                TypeDeclarationKind::Resource { lifecycles } => {
                    for lifecycle in lifecycles {
                        if let Some(id) = &lifecycle.stable_id {
                            insert_other(&mut declarations, id, program)?;
                        }
                    }
                }
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        insert_other(&mut declarations, &field.stable_id, program)?;
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        insert_other(&mut declarations, &case.stable_id, program)?;
                        for field in &case.fields {
                            insert_other(&mut declarations, &field.stable_id, program)?;
                        }
                    }
                }
            }
        }
        for interface in &program.interfaces {
            insert_other(&mut declarations, &interface.stable_id, program)?;
            for import in &interface.imports {
                insert_other(&mut declarations, &import.stable_id, program)?;
            }
        }
        for protocol in &program.protocols {
            insert_authored(
                &mut declarations,
                &protocol.stable_id,
                AuthoredDeclaration {
                    path: &program.path,
                    module: &program.module,
                    explicit: protocol.explicit_id,
                    kind: AuthoredKind::Protocol,
                    function: None,
                    ty: None,
                },
            )?;
            for method in &protocol.methods {
                insert_other(&mut declarations, &method.stable_id, program)?;
            }
        }
    }
    Ok(declarations)
}

fn validate_synthetic_main_id_collisions(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    for program in programs {
        if program
            .functions
            .iter()
            .any(|function| function.name == "main")
        {
            continue;
        }
        let generated = crate::bounded_output::budgeted_format(format_args!(
            "workspace.synthetic.main.{}",
            program.module
        ));
        if authored.contains_key(generated.as_str()) {
            return Err(vec![graph_error(
                "SPX-G173",
                "generated workspace synthetic main identity collides with an authored declaration",
            )]);
        }
    }
    Ok(())
}

fn insert_other<'a>(
    declarations: &mut BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    id: &'a str,
    program: &'a Program,
) -> Result<(), Vec<Diagnostic>> {
    insert_authored(
        declarations,
        id,
        AuthoredDeclaration {
            path: &program.path,
            module: &program.module,
            explicit: true,
            kind: AuthoredKind::Other,
            function: None,
            ty: None,
        },
    )
}

fn insert_authored<'a>(
    declarations: &mut BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    id: &'a str,
    declaration: AuthoredDeclaration<'a>,
) -> Result<(), Vec<Diagnostic>> {
    if prelude::is_compiler_owned_id(id) {
        return Err(vec![graph_error(
            "SPX-G172",
            format!("workspace authored identity `{id}` is compiler-owned"),
        )]);
    }
    if let Some(existing) = declarations.insert(id, declaration) {
        return Err(vec![graph_error(
            "SPX-G172",
            format!(
                "workspace authored identity `{id}` is duplicated by `{}` and `{}`",
                existing.path, declarations[id].path
            ),
        )]);
    }
    Ok(())
}

fn validate_uses(
    programs: &[Program],
    modules: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    for program in programs {
        let mut function_aliases = BTreeSet::new();
        let mut type_aliases = BTreeSet::new();
        let mut protocol_aliases = BTreeSet::new();
        let mut imported_targets = BTreeSet::new();
        let local_functions = program
            .functions
            .iter()
            .map(|item| item.name.as_str())
            .chain(
                program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .map(|import| import.name.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let local_types = program
            .types
            .iter()
            .map(|item| item.name.as_str())
            .chain(
                prelude::declarations()
                    .iter()
                    .map(|item| item.name.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let local_protocols = program
            .protocols
            .iter()
            .map(|item| item.name.as_str())
            .collect::<BTreeSet<_>>();
        let workspace_type_aliases = program
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Type)
            .map(|item| item.alias.as_str())
            .collect::<BTreeSet<_>>();
        for interface in &program.interfaces {
            for import in &interface.imports {
                for param in &import.params {
                    if type_contains_name_from(&param.ty, &workspace_type_aliases) {
                        return Err(vec![Diagnostic::error(
                            "SPX-G172",
                            "workspace type aliases are not admitted in interface/import parameter carriers",
                            param.span,
                        )
                        .at_path(&program.path)]);
                    }
                }
            }
        }
        for module_use in &program.module_uses {
            if !imported_targets.insert((module_use.kind, module_use.persistent_id.as_str())) {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "the same workspace target is imported more than once",
                )]);
            }
            if module_use.target_module == program.module
                || !modules.contains_key(module_use.target_module.as_str())
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "target module is missing or equals the caller module",
                )]);
            }
            let alias_conflicts = match module_use.kind {
                ModuleUseKind::Function => {
                    !function_aliases.insert(module_use.alias.as_str())
                        || local_functions.contains(module_use.alias.as_str())
                        || module_use.alias == "main"
                }
                ModuleUseKind::Type => {
                    !type_aliases.insert(module_use.alias.as_str())
                        || local_types.contains(module_use.alias.as_str())
                }
                ModuleUseKind::Protocol => {
                    !protocol_aliases.insert(module_use.alias.as_str())
                        || local_protocols.contains(module_use.alias.as_str())
                }
            };
            if alias_conflicts {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "alias is duplicated or shadows a local/prelude declaration",
                )]);
            }
            let target = authored
                .get(module_use.persistent_id.as_str())
                .ok_or_else(|| {
                    vec![use_error(
                        program,
                        module_use,
                        "persistent target identity is unknown",
                    )]
                })?;
            let expected = match module_use.kind {
                ModuleUseKind::Function => AuthoredKind::Function,
                ModuleUseKind::Type => AuthoredKind::Type,
                ModuleUseKind::Protocol => AuthoredKind::Protocol,
            };
            if !target.explicit
                || target.module != module_use.target_module
                || target.kind != expected
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "target module, declaration kind, or explicit identity disagrees",
                )]);
            }
            match module_use.kind {
                ModuleUseKind::Function => {
                    validate_imported_function(program, module_use, target, authored, programs)?
                }
                ModuleUseKind::Type => {
                    validate_imported_type(program, module_use, target, authored, programs)?;
                }
                ModuleUseKind::Protocol => {}
            }
        }
    }
    Ok(())
}

fn type_contains_name_from(ty: &Type, names: &BTreeSet<&str>) -> bool {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Str
        | Type::SliceU8
        | Type::ArrayU8(_)
        | Type::Bytes => false,
        Type::Named { name, arguments } => {
            names.contains(name.as_str())
                || arguments
                    .iter()
                    .any(|argument| type_contains_name_from(argument, names))
        }
    }
}

fn validate_imported_function(
    caller: &Program,
    module_use: &ModuleUse,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let function = target.function.expect("function target carries a function");
    let byte_parameter = package::admitted_byte_parameter;
    let has_byte_parameter = function.params.iter().any(byte_parameter);
    let scalar_return = matches!(
        function.return_type,
        Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
    );
    if !function.type_parameters.is_empty()
        || function
            .params
            .iter()
            .any(|param| param.mode != ParamMode::Value && !byte_parameter(param))
        || (has_byte_parameter && !scalar_return)
    {
        return Err(vec![use_error(
            caller,
            module_use,
            package::import_profile_refusal(),
        )]);
    }
    for param in &function.params {
        if byte_parameter(param) {
            continue;
        }
        if !signature_type_is_admitted(
            target.module,
            &param.ty,
            caller,
            authored,
            programs,
            &mut BTreeSet::new(),
        ) {
            return Err(vec![use_error(
                caller,
                module_use,
                "function signature leaves the admitted scalar/Copy workspace domain",
            )]);
        }
    }
    let ty = &function.return_type;
    {
        if !signature_type_is_admitted(
            target.module,
            ty,
            caller,
            authored,
            programs,
            &mut BTreeSet::new(),
        ) {
            return Err(vec![use_error(
                caller,
                module_use,
                "function signature leaves the admitted scalar/Copy workspace domain",
            )]);
        }
    }
    Ok(())
}

fn signature_type_is_admitted(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> bool {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Str => true,
        Type::SliceU8 | Type::ArrayU8(_) | Type::Bytes => false,
        Type::Named { name, arguments } if arguments.is_empty() => {
            let Some(target_id) = resolve_type_id(module, name, programs) else {
                return false;
            };
            let Some(target) = authored.get(target_id.as_str()) else {
                return false;
            };
            caller
                .module_uses
                .iter()
                .any(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
                && target.ty.is_some_and(|target_type| {
                    type_is_admitted(target.module, target_type, authored, programs, visiting)
                        && exposed_types_are_directly_imported(
                            caller,
                            target.module,
                            target_type,
                            authored,
                            programs,
                            &mut BTreeSet::new(),
                        )
                })
        }
        Type::Named { .. } => false,
    }
}

fn resolve_type_id(module: &str, name: &str, programs: &[Program]) -> Option<String> {
    let program = programs.iter().find(|item| item.module == module)?;
    if let Some(local) = program.types.iter().find(|item| item.name == name) {
        return Some(crate::bounded_output::budgeted_clone(&local.stable_id));
    }
    program
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.alias == name)
        .map(|item| crate::bounded_output::budgeted_clone(&item.persistent_id))
}

fn validate_imported_type(
    caller: &Program,
    module_use: &ModuleUse,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let ty = target.ty.expect("type target carries a type");
    if !ty.type_parameters.is_empty()
        || !type_is_admitted(target.module, ty, authored, programs, &mut BTreeSet::new())
        || !exposed_types_are_directly_imported(
            caller,
            target.module,
            ty,
            authored,
            programs,
            &mut BTreeSet::new(),
        )
    {
        return Err(vec![use_error(
            caller,
            module_use,
            "type target must be an explicit nongeneric recursive Copy value type",
        )]);
    }
    Ok(())
}

fn exposed_types_are_directly_imported(
    caller: &Program,
    module: &str,
    declaration: &TypeDeclaration,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> bool {
    if !visiting.insert(crate::bounded_output::budgeted_clone(
        &declaration.stable_id,
    )) {
        return false;
    }
    let admitted = match &declaration.kind {
        TypeDeclarationKind::Resource { .. } => return false,
        TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
            fields.iter().all(|field| {
                exposed_type_reference_is_directly_imported(
                    caller, module, &field.ty, authored, programs, visiting,
                )
            })
        }
        TypeDeclarationKind::Variant { cases } => {
            cases.iter().flat_map(|case| &case.fields).all(|field| {
                exposed_type_reference_is_directly_imported(
                    caller, module, &field.ty, authored, programs, visiting,
                )
            })
        }
    };
    visiting.remove(&declaration.stable_id);
    admitted
}

fn exposed_type_reference_is_directly_imported(
    caller: &Program,
    module: &str,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> bool {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Str => true,
        Type::SliceU8 | Type::ArrayU8(_) | Type::Bytes => false,
        Type::Named { name, arguments } if arguments.is_empty() => {
            let Some(target_id) = resolve_type_id(module, name, programs) else {
                return false;
            };
            let directly_imported = caller
                .module_uses
                .iter()
                .any(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id);
            directly_imported
                && authored.get(target_id.as_str()).is_some_and(|target| {
                    target.ty.is_some_and(|nested| {
                        exposed_types_are_directly_imported(
                            caller,
                            target.module,
                            nested,
                            authored,
                            programs,
                            visiting,
                        )
                    })
                })
        }
        Type::Named { .. } => false,
    }
}

fn type_is_admitted(
    module: &str,
    declaration: &TypeDeclaration,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> bool {
    if !declaration.explicit_id
        || !visiting.insert(crate::bounded_output::budgeted_clone(
            &declaration.stable_id,
        ))
    {
        return false;
    }
    let valid = match &declaration.kind {
        TypeDeclarationKind::Resource { .. } => return false,
        TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
            fields.iter().all(|field| {
                type_reference_is_admitted(module, &field.ty, authored, programs, visiting)
            })
        }
        TypeDeclarationKind::Variant { cases } => {
            cases.iter().flat_map(|case| &case.fields).all(|field| {
                type_reference_is_admitted(module, &field.ty, authored, programs, visiting)
            })
        }
    };
    visiting.remove(&declaration.stable_id);
    valid
}

fn type_reference_is_admitted(
    module: &str,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
) -> bool {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Str => true,
        Type::SliceU8 | Type::ArrayU8(_) | Type::Bytes => false,
        Type::Named { name, arguments } if arguments.is_empty() => {
            let Some(program) = programs.iter().find(|item| item.module == module) else {
                return false;
            };
            let local_target = program
                .types
                .iter()
                .find(|item| item.name == *name)
                .and_then(|item| authored.get(item.stable_id.as_str()));
            if local_target.is_some_and(|target| {
                target.ty.is_some_and(|declaration| {
                    type_is_admitted(module, declaration, authored, programs, visiting)
                })
            }) {
                return true;
            }
            program
                .module_uses
                .iter()
                .find(|item| item.kind == ModuleUseKind::Type && item.alias == *name)
                .and_then(|item| authored.get(item.persistent_id.as_str()))
                .is_some_and(|target| {
                    target.ty.is_some_and(|declaration| {
                        type_is_admitted(target.module, declaration, authored, programs, visiting)
                    })
                })
        }
        Type::Named { .. } => false,
    }
}

fn reserve_builder_structure(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(vec![limit_error("builder_bytes", active_builder_limit())])
    }
}

fn push_edge(edges: &mut Vec<WorkspaceEdge>, edge: WorkspaceEdge) -> Result<(), Vec<Diagnostic>> {
    if edges.len() == MAX_CROSS_FILE_EDGES {
        return Err(vec![limit_error(
            "resolved_cross_file_edges",
            MAX_CROSS_FILE_EDGES,
        )]);
    }
    reserve_builder_structure(std::mem::size_of::<WorkspaceEdge>())?;
    edges.push(edge);
    Ok(())
}

fn budgeted_edge_clone(edge: &WorkspaceEdge) -> WorkspaceEdge {
    WorkspaceEdge {
        caller_path: crate::bounded_output::budgeted_clone(&edge.caller_path),
        caller: crate::bounded_output::budgeted_clone(&edge.caller),
        target_path: crate::bounded_output::budgeted_clone(&edge.target_path),
        target: crate::bounded_output::budgeted_clone(&edge.target),
        kind: edge.kind,
        site: edge.site,
        expression: crate::bounded_output::budgeted_clone(&edge.expression),
        ast_path: crate::bounded_output::budgeted_clone(&edge.ast_path),
        alias: crate::bounded_output::budgeted_clone(&edge.alias),
        ordinal: edge.ordinal,
    }
}

fn visit_ast_call_sites(
    expression: &Expr,
    path: &str,
    visit: &mut impl FnMut(&str, &str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    match &expression.kind {
        ExprKind::Call { name, args, .. } => {
            visit(name, path)?;
            for (index, argument) in args.iter().enumerate() {
                visit_ast_call_sites(
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}")),
                    visit,
                )?;
            }
        }
        ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::Unary { value, .. } => {
            visit_ast_call_sites(
                value,
                &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
                visit,
            )?;
        }
        ExprKind::Binary { left, right, .. } => {
            visit_ast_call_sites(
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                visit,
            )?;
            visit_ast_call_sites(
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                visit,
            )?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    crate::ast::Statement::Let { value, .. }
                    | crate::ast::Statement::Assign { value, .. } => visit_ast_call_sites(
                        value,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
                    crate::ast::Statement::Unsafe { body, .. } => visit_ast_call_sites(
                        body,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
                    crate::ast::Statement::While {
                        condition, body, ..
                    } => {
                        visit_ast_call_sites(
                            condition,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.condition"
                            )),
                            visit,
                        )?;
                        visit_ast_call_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.body"
                            )),
                            visit,
                        )?;
                    }
                }
            }
            visit_ast_call_sites(
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                visit,
            )?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_ast_call_sites(
                condition,
                &crate::bounded_output::budgeted_format(format_args!("{path}.condition")),
                visit,
            )?;
            visit_ast_call_sites(
                then_branch,
                &crate::bounded_output::budgeted_format(format_args!("{path}.then")),
                visit,
            )?;
            visit_ast_call_sites(
                else_branch,
                &crate::bounded_output::budgeted_format(format_args!("{path}.else")),
                visit,
            )?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                visit_ast_call_sites(
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_ast_call_sites(
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                visit,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                if let Some(guard) = &arm.guard {
                    visit_ast_call_sites(
                        guard.as_ref(),
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.arm.{index}.guard"
                        )),
                        visit,
                    )?;
                }
                visit_ast_call_sites(
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Try { operand } => {
            visit_ast_call_sites(
                operand,
                &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
                visit,
            )?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            visit_ast_call_sites(
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                visit,
            )?;
            for (index, field) in fields.iter().enumerate() {
                visit_ast_call_sites(
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Project { base, .. } => {
            visit_ast_call_sites(
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                visit,
            )?;
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => {}
        // Method calls resolve to hoisted functions in HIR; the AST-level
        // call-site walk sees them through the resolved Call edge instead.
        ExprKind::MethodCall { .. } => {}
        // `super.method(...)` also resolves to a hoisted parent method in HIR.
        ExprKind::SuperMethod { .. } => {}
    }
    Ok(())
}

fn validate_stub_signatures(
    programs: &[Program],
    modules: &[(String, hir::ResolvedProgram)],
) -> Result<(), Vec<Diagnostic>> {
    for caller in programs {
        let caller_hir = modules
            .iter()
            .find(|(module, _)| module == &caller.module)
            .map(|(_, resolved)| resolved)
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "resolved caller module is absent from the workspace HIR",
                )]
            })?;
        for module_use in &caller.module_uses {
            if module_use.kind == ModuleUseKind::Protocol {
                continue;
            }
            let target_hir = modules
                .iter()
                .find(|(module, _)| module == &module_use.target_module)
                .map(|(_, resolved)| resolved)
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "resolved target module is absent from the workspace HIR",
                    )]
                })?;
            let id = hir::DeclarationId::new(crate::bounded_output::budgeted_clone(
                &module_use.persistent_id,
            ));
            match module_use.kind {
                ModuleUseKind::Function => {
                    let stub = caller_hir.functions.iter().find(|item| item.id == id);
                    let authority = target_hir.functions.iter().find(|item| item.id == id);
                    if !stub.zip(authority).is_some_and(|(stub, authority)| {
                        stub.params == authority.params
                            && stub.return_type == authority.return_type
                            && stub.effects == authority.effects
                    }) {
                        return Err(vec![graph_error(
                            "SPX-G173",
                            "workspace function signature stub disagrees with authored HIR authority",
                        )]);
                    }
                }
                ModuleUseKind::Type => {
                    let stub = caller_hir.types.iter().find(|item| item.id == id);
                    let authority = target_hir.types.iter().find(|item| item.id == id);
                    if !stub.zip(authority).is_some_and(|(stub, authority)| {
                        stub.type_parameters == authority.type_parameters
                            && stub.kind == authority.kind
                    }) {
                        return Err(vec![graph_error(
                            "SPX-G173",
                            "workspace type signature stub disagrees with authored HIR authority",
                        )]);
                    }
                }
                ModuleUseKind::Protocol => unreachable!(),
            }
        }
    }
    Ok(())
}

fn workspace_declaration_facts(
    modules: &[(String, hir::ResolvedProgram)],
    retained_modules: &[WorkspaceResolvedModule],
    programs: &[Program],
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let facts = reconstruct_workspace_declaration_facts(modules, programs)?;
    validate_retained_declaration_shapes(retained_modules, &facts)?;
    Ok(facts)
}

fn reconstruct_workspace_declaration_facts(
    modules: &[(String, hir::ResolvedProgram)],
    programs: &[Program],
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let expected = expected_declaration_facts(programs)?;
    let expected_compiler = expected_compiler_declaration_facts()?;
    let mut actual = BTreeMap::new();
    for (module, resolved) in modules {
        let source = programs
            .iter()
            .find(|program| program.module == *module)
            .expect("resolved workspace module belongs to authenticated source");
        let direct_targets = source
            .module_uses
            .iter()
            .map(|module_use| module_use.persistent_id.as_str())
            .collect::<BTreeSet<_>>();
        let synthetic_main = crate::bounded_output::budgeted_format(format_args!(
            "workspace.synthetic.main.{module}"
        ));
        let synthetic_main_is_allowed = !source
            .functions
            .iter()
            .any(|function| function.name == "main")
            && !expected.contains_key(synthetic_main.as_str())
            && !expected_compiler.contains_key(synthetic_main.as_str());
        let mut compiler = BTreeMap::new();
        for declaration in resolved.declarations.workspace_declarations() {
            if declaration.identity_origin == hir::IdentityOrigin::CompilerOwned {
                let fact = WorkspaceDeclarationFact {
                    kind: declaration.kind,
                    origin: declaration.identity_origin,
                    owner: declaration
                        .owner
                        .map(|owner| crate::bounded_output::budgeted_clone(owner.as_str())),
                    path: None,
                    module: None,
                };
                if compiler
                    .insert(
                        crate::bounded_output::budgeted_clone(declaration.id.as_str()),
                        fact,
                    )
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "compiler-owned workspace declaration identity is duplicated",
                    )]);
                }
                continue;
            }
            let top = top_level_declaration(&resolved.declarations, &declaration);
            let Some(top_fact) = expected.get(top.as_str()) else {
                if synthetic_main_is_allowed
                    && declaration.id.as_str() == synthetic_main.as_str()
                    && declaration.kind == hir::DeclarationKind::Function
                    && declaration.identity_origin == hir::IdentityOrigin::Explicit
                    && declaration.owner.is_none()
                {
                    continue;
                }
                return Err(vec![graph_error(
                    "SPX-G173",
                    "resolved workspace declaration has an unauthenticated synthetic or rogue root",
                )]);
            };
            if top_fact.module.as_deref() != Some(module.as_str()) {
                let expected_foreign = expected.get(declaration.id.as_str());
                if direct_targets.contains(top.as_str())
                    && expected_foreign.is_some_and(|fact| {
                        fact.kind == declaration.kind
                            && fact.origin == declaration.identity_origin
                            && fact.owner.as_deref()
                                == declaration.owner.as_ref().map(hir::DeclarationId::as_str)
                            && fact.path == top_fact.path
                            && fact.module == top_fact.module
                    })
                {
                    continue;
                }
                return Err(vec![graph_error(
                    "SPX-G173",
                    "resolved workspace declaration leaks a non-imported foreign authority",
                )]);
            }
            let fact = WorkspaceDeclarationFact {
                kind: declaration.kind,
                origin: declaration.identity_origin,
                owner: declaration
                    .owner
                    .map(|owner| crate::bounded_output::budgeted_clone(owner.as_str())),
                path: top_fact
                    .path
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
                module: top_fact
                    .module
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
            };
            if actual
                .insert(
                    crate::bounded_output::budgeted_clone(declaration.id.as_str()),
                    fact,
                )
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace declaration identity is retained more than once",
                )]);
            }
        }
        if compiler != expected_compiler {
            return Err(vec![graph_error(
                "SPX-G173",
                "compiler-owned prelude declaration facts disagree with the independent prelude map",
            )]);
        }
    }
    if actual != expected {
        return Err(vec![graph_error(
            "SPX-G173",
            "authored workspace declaration facts disagree with retained HIR",
        )]);
    }
    let compiler_ids = expected_compiler
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_compiler_ids = prelude::all_ids().into_iter().collect::<BTreeSet<_>>();
    if compiler_ids != expected_compiler_ids {
        return Err(vec![graph_error(
            "SPX-G173",
            "compiler-owned workspace declaration facts disagree with the exact shared prelude",
        )]);
    }
    let mut facts = expected;
    for (id, fact) in expected_compiler {
        if facts.insert(id, fact).is_some() {
            return Err(vec![graph_error(
                "SPX-G173",
                "compiler-owned and authored workspace declaration identities overlap",
            )]);
        }
    }
    Ok(facts)
}

fn expected_declaration_facts(
    programs: &[Program],
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let mut facts = BTreeMap::new();
    for program in programs {
        for declaration in &program.types {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => hir::DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => {
                    hir::DeclarationKind::Record
                }
                TypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
            };
            insert_expected_declaration(
                &mut facts,
                program,
                &declaration.stable_id,
                kind,
                identity_origin(declaration.explicit_id),
                None,
            )?;
            match &declaration.kind {
                TypeDeclarationKind::Resource { lifecycles } => {
                    let [lifecycle] = lifecycles.as_slice() else {
                        return Err(vec![graph_error(
                            "SPX-G173",
                            "authored workspace resource has no exact lifecycle identity",
                        )]);
                    };
                    let id = lifecycle.stable_id.as_deref().ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "authored workspace resource lifecycle identity is missing",
                        )]
                    })?;
                    insert_expected_declaration(
                        &mut facts,
                        program,
                        id,
                        hir::DeclarationKind::ResourceDrop,
                        hir::IdentityOrigin::Explicit,
                        Some(&declaration.stable_id),
                    )?;
                }
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        insert_expected_declaration(
                            &mut facts,
                            program,
                            &field.stable_id,
                            hir::DeclarationKind::Field,
                            identity_origin(field.explicit_id),
                            Some(&declaration.stable_id),
                        )?;
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        insert_expected_declaration(
                            &mut facts,
                            program,
                            &case.stable_id,
                            hir::DeclarationKind::VariantCase,
                            identity_origin(case.explicit_id),
                            Some(&declaration.stable_id),
                        )?;
                        for field in &case.fields {
                            insert_expected_declaration(
                                &mut facts,
                                program,
                                &field.stable_id,
                                hir::DeclarationKind::CaseField,
                                identity_origin(field.explicit_id),
                                Some(&case.stable_id),
                            )?;
                        }
                    }
                }
            }
        }
        for interface in &program.interfaces {
            insert_expected_declaration(
                &mut facts,
                program,
                &interface.stable_id,
                hir::DeclarationKind::Interface,
                identity_origin(interface.explicit_id),
                None,
            )?;
            for import in &interface.imports {
                insert_expected_declaration(
                    &mut facts,
                    program,
                    &import.stable_id,
                    hir::DeclarationKind::Import,
                    identity_origin(import.explicit_id),
                    Some(&interface.stable_id),
                )?;
            }
        }
        for function in &program.functions {
            insert_expected_declaration(
                &mut facts,
                program,
                &function.stable_id,
                hir::DeclarationKind::Function,
                identity_origin(function.explicit_id),
                None,
            )?;
        }
    }
    Ok(facts)
}

fn expected_compiler_declaration_facts(
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let mut facts = BTreeMap::new();
    for declaration in prelude::declarations() {
        let kind = match &declaration.kind {
            TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => {
                hir::DeclarationKind::Record
            }
            TypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
            TypeDeclarationKind::Resource { .. } => {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "compiler prelude unexpectedly declares a resource authority",
                )]);
            }
        };
        insert_expected_compiler_declaration(&mut facts, &declaration.stable_id, kind, None)?;
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    insert_expected_compiler_declaration(
                        &mut facts,
                        &field.stable_id,
                        hir::DeclarationKind::Field,
                        Some(&declaration.stable_id),
                    )?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    insert_expected_compiler_declaration(
                        &mut facts,
                        &case.stable_id,
                        hir::DeclarationKind::VariantCase,
                        Some(&declaration.stable_id),
                    )?;
                    for field in &case.fields {
                        insert_expected_compiler_declaration(
                            &mut facts,
                            &field.stable_id,
                            hir::DeclarationKind::CaseField,
                            Some(&case.stable_id),
                        )?;
                    }
                }
            }
            TypeDeclarationKind::Resource { .. } => unreachable!("resource rejected above"),
        }
    }
    Ok(facts)
}

fn insert_expected_compiler_declaration(
    facts: &mut BTreeMap<String, WorkspaceDeclarationFact>,
    id: &str,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
) -> Result<(), Vec<Diagnostic>> {
    let fact = WorkspaceDeclarationFact {
        kind,
        origin: hir::IdentityOrigin::CompilerOwned,
        owner: owner.map(crate::bounded_output::budgeted_clone),
        path: None,
        module: None,
    };
    if facts
        .insert(crate::bounded_output::budgeted_clone(id), fact)
        .is_some()
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "independent compiler prelude declaration identity is duplicated",
        )]);
    }
    Ok(())
}

fn identity_origin(explicit: bool) -> hir::IdentityOrigin {
    if explicit {
        hir::IdentityOrigin::Explicit
    } else {
        hir::IdentityOrigin::Automatic
    }
}

fn insert_expected_declaration(
    facts: &mut BTreeMap<String, WorkspaceDeclarationFact>,
    program: &Program,
    id: &str,
    kind: hir::DeclarationKind,
    origin: hir::IdentityOrigin,
    owner: Option<&str>,
) -> Result<(), Vec<Diagnostic>> {
    let fact = WorkspaceDeclarationFact {
        kind,
        origin,
        owner: owner.map(crate::bounded_output::budgeted_clone),
        path: Some(crate::bounded_output::budgeted_clone(&program.path)),
        module: Some(crate::bounded_output::budgeted_clone(&program.module)),
    };
    if facts
        .insert(crate::bounded_output::budgeted_clone(id), fact)
        .is_some()
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "independent authored workspace declaration identity is duplicated",
        )]);
    }
    Ok(())
}

fn validate_retained_declaration_shapes(
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<(), Vec<Diagnostic>> {
    for module in modules {
        let mut seen = BTreeSet::new();
        for declaration in &module.types {
            let kind = match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Resource { .. } => hir::DeclarationKind::Resource,
                hir::ResolvedTypeDeclarationKind::Record { .. }
                | hir::ResolvedTypeDeclarationKind::Class { .. } => hir::DeclarationKind::Record,
                hir::ResolvedTypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
            };
            require_retained_shape_fact(
                facts,
                module,
                declaration.id.as_str(),
                kind,
                None,
                &mut seen,
            )?;
            match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Resource { drop } => {
                    require_retained_shape_fact(
                        facts,
                        module,
                        drop.id.as_str(),
                        hir::DeclarationKind::ResourceDrop,
                        Some(declaration.id.as_str()),
                        &mut seen,
                    )?;
                }
                hir::ResolvedTypeDeclarationKind::Record { fields }
                | hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        require_retained_shape_fact(
                            facts,
                            module,
                            field.id.as_str(),
                            hir::DeclarationKind::Field,
                            Some(declaration.id.as_str()),
                            &mut seen,
                        )?;
                    }
                }
                hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        require_retained_shape_fact(
                            facts,
                            module,
                            case.id.as_str(),
                            hir::DeclarationKind::VariantCase,
                            Some(declaration.id.as_str()),
                            &mut seen,
                        )?;
                        for field in &case.fields {
                            require_retained_shape_fact(
                                facts,
                                module,
                                field.id.as_str(),
                                hir::DeclarationKind::CaseField,
                                Some(case.id.as_str()),
                                &mut seen,
                            )?;
                        }
                    }
                }
            }
        }
        for interface in &module.interfaces {
            require_retained_shape_fact(
                facts,
                module,
                interface.id.as_str(),
                hir::DeclarationKind::Interface,
                None,
                &mut seen,
            )?;
            for import in &interface.imports {
                require_retained_shape_fact(
                    facts,
                    module,
                    import.id.as_str(),
                    hir::DeclarationKind::Import,
                    Some(interface.id.as_str()),
                    &mut seen,
                )?;
            }
        }
        for function in &module.functions {
            require_retained_shape_fact(
                facts,
                module,
                function.id.as_str(),
                hir::DeclarationKind::Function,
                None,
                &mut seen,
            )?;
        }
        let mut templates = BTreeSet::new();
        for template in &module.function_templates {
            require_retained_shape_fact(
                facts,
                module,
                template.id.as_str(),
                hir::DeclarationKind::Function,
                None,
                &mut seen,
            )?;
            if !templates.insert(template.id.as_str()) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace function template is duplicated",
                )]);
            }
        }
        let mut instances = BTreeSet::new();
        for instance in &module.function_instances {
            let fact = facts.get(instance.template.as_str());
            if !templates.contains(instance.template.as_str())
                || instance.function.id != instance.template
                || !fact.is_some_and(|fact| {
                    fact.kind == hir::DeclarationKind::Function
                        && fact.module.as_deref() == Some(module.module.as_str())
                        && fact.path.as_deref() == Some(module.path.as_str())
                })
                || !instances.insert(&instance.id)
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace function instance/template identity shape disagrees",
                )]);
            }
        }
        let expected = facts
            .iter()
            .filter(|(_, fact)| fact.module.as_deref() == Some(module.module.as_str()))
            .map(|(id, _)| id.as_str())
            .collect::<BTreeSet<_>>();
        if seen != expected {
            return Err(vec![graph_error(
                "SPX-G173",
                "retained workspace declaration shapes are not the exact authored set",
            )]);
        }
    }
    Ok(())
}

fn require_retained_shape_fact<'a>(
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    module: &WorkspaceResolvedModule,
    id: &'a str,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
    seen: &mut BTreeSet<&'a str>,
) -> Result<(), Vec<Diagnostic>> {
    let fact = facts.get(id);
    if !fact.is_some_and(|fact| {
        fact.kind == kind
            && fact.owner.as_deref() == owner
            && fact.path.as_deref() == Some(module.path.as_str())
            && fact.module.as_deref() == Some(module.module.as_str())
    }) || !seen.insert(id)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "retained workspace declaration shape disagrees with authored identity facts",
        )]);
    }
    Ok(())
}

fn top_level_declaration(
    index: &hir::DeclarationIndex,
    declaration: &hir::Declaration,
) -> hir::DeclarationId {
    let mut current = declaration;
    while let Some(owner) = &current.owner {
        let Some(parent) = index.declaration(owner) else {
            break;
        };
        current = parent;
    }
    hir::DeclarationId::new(crate::bounded_output::budgeted_clone(current.id.as_str()))
}

fn use_error(program: &Program, module_use: &ModuleUse, message: &str) -> Diagnostic {
    Diagnostic::error("SPX-G172", message, module_use.span).at_path(&program.path)
}

fn graph_error(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(code, message)
}

fn limit_error(field: &'static str, maximum: usize) -> Diagnostic {
    graph_error(
        "SPX-G171",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Semantic Graph `{field}` exceeds {maximum}"
        )),
    )
}

#[cfg(test)]
#[path = "workspace_graph/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_graph/type_proof_tests.rs"]
mod type_proof_tests;
