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

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ast::{
    Expr, ExprKind, FieldInitializer, Function, ModuleUse, ModuleUseKind, ParamMode, Program, Span,
    Type, TypeDeclaration, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::{format, graph, hir, parse, prelude, workspace};

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

pub(crate) struct WorkspaceGraphOperationView {
    pub(crate) graph: WorkspaceGraphChangeView,
    pub(crate) sidecar: WorkspaceOperationSidecar,
    pub(crate) builder_bytes: usize,
    pub(crate) change_builder_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOperationOccurrence {
    pub(crate) span: Span,
    pub(crate) owner: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOperationDeclaration {
    pub(crate) path: String,
    pub(crate) module: String,
    pub(crate) id: String,
    pub(crate) kind: &'static str,
    pub(crate) explicit: bool,
    pub(crate) name: String,
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

impl WorkspaceGraphBuild {
    pub(crate) fn contains_module(&self, module: &str) -> bool {
        self.hir.module_paths.contains_key(module)
    }

    /// Consume one validated workspace build into the entry module's complete
    /// provider closure and link its real scalar function bodies. This is a
    /// private backend-preparation seam, not a new Workspace authority or a
    /// general cross-file composition surface.
    pub(crate) fn linked_scalar_program(
        &self,
        entry_module: &str,
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
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

        let mut functions = Vec::new();
        let mut entrypoints = Vec::new();
        let mut retained_modules = 0usize;
        for module in &self.hir.modules {
            if !reachable_paths.contains(module.path.as_str()) {
                continue;
            }
            retained_modules += 1;
            if !module.permits.is_empty()
                || !module.types.is_empty()
                || !module.interfaces.is_empty()
                || !module.function_templates.is_empty()
                || !module.function_instances.is_empty()
            {
                return Err(vec![graph_error(
                    "SPX-G172",
                    format!(
                        "workspace module `{}` is outside the pure scalar linker profile",
                        module.module
                    ),
                )]);
            }
            for function in &module.functions {
                if module.module != entry_module && function.name == "main" {
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
                    entrypoints.push((function.id.clone(), fact.origin));
                }
                functions.push(hir::LinkedScalarFunction {
                    function: function.clone(),
                    origin: fact.origin,
                });
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
        hir::link_scalar_workspace(entry_module.to_owned(), entrypoint, functions)
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

    fn validate_entire_scalar_workspace(
        &self,
        entry_module: &str,
        test_module: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_entry_module(entry_module)?;
        validate_entry_module(test_module)?;
        let roots = BTreeSet::from([entry_module, test_module]);
        for module in &self.hir.modules {
            if !module.permits.is_empty()
                || !module.types.is_empty()
                || !module.interfaces.is_empty()
                || !module.function_templates.is_empty()
                || !module.function_instances.is_empty()
            {
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
            for function in &module.functions {
                if !function.effects.is_empty()
                    || function
                        .params
                        .iter()
                        .any(|parameter| parameter.ownership != hir::OwnershipMode::Value)
                    || !matches!(
                        function.return_type,
                        hir::ResolvedType::I64 | hir::ResolvedType::Bool
                    )
                    || function.params.iter().any(|parameter| {
                        !matches!(
                            parameter.ty,
                            hir::ResolvedType::I64 | hir::ResolvedType::Bool
                        )
                    })
                {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        format!(
                            "workspace function `{}` is outside the pure scalar linker profile",
                            function.id
                        ),
                    )]);
                }
            }
        }
        if self.edges.iter().any(|edge| edge.kind == "type_import") {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace scalar linker does not admit `use type` imports",
            )]);
        }
        Ok(())
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
                let source_graph_schema = graph::graph_schema_from_parts(
                    &module.types,
                    &module.functions,
                    &module.function_templates,
                );
                WorkspaceGraphChangeModule {
                    path: module.path,
                    module: module.module,
                    source_graph_schema,
                    permits: module.permits,
                }
            })
            .collect();
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
            let schema = graph::graph_schema_from_parts(
                &module.types,
                &module.functions,
                &module.function_templates,
            );
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
    let result = build_owned_inner(sources, change_builder_limit, retain_operation_programs);
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
        let program =
            parse(&source.source, Path::new(&source.path)).map_err(|error| vec![error])?;
        if program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .any(|import| import.native_rust)
        {
            return Err(vec![graph_error(
                "SPX-G218",
                "Native Rust import declarations are outside the current semantic Graph schemas",
            )]);
        }
        let remaining = active_builder_limit().saturating_sub(canonical_bytes);
        let (canonical, overflowed) =
            crate::bounded_output::with_limit(remaining, || format::canonical(&program));
        if overflowed {
            return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
        }
        canonical_bytes = checked_usage(
            canonical_bytes,
            canonical.len(),
            "builder_bytes",
            active_builder_limit(),
        )?;
        if canonical != source.source {
            return Err(vec![graph_error(
                "SPX-G170",
                format!(
                    "workspace semantic source `{}` is not canonical",
                    source.path
                ),
            )]);
        }
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
    checked_usage(
        resolve_builder_bytes,
        runtime_builder_bytes,
        "builder_bytes",
        active_builder_limit(),
    )?;
    let (core, overflowed, core_builder_bytes) =
        crate::bounded_output::with_limit_usage(active_builder_limit(), || {
            charge_builder_prebound(resolve_builder_bytes)?;
            build_resolved_core(&programs, &module_paths, &dependency_depths, &authored)
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

fn build_resolved_core(
    programs: &[Program],
    module_paths: &BTreeMap<&str, &str>,
    dependency_depths: &BTreeMap<&str, usize>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
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
        let resolved = hir::resolve(&synthetic)?;
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
            .checked_mul(std::mem::size_of::<WorkspaceResolvedModule>())
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
        let functions = filter_owned_vec(resolved.functions, |item| {
            authored
                .get(item.id.as_str())
                .is_some_and(|owner| owner.module == program.module)
        })?;
        let function_templates = filter_owned_vec(resolved.function_templates, |item| {
            authored
                .get(item.id.as_str())
                .is_some_and(|owner| owner.module == program.module)
        })?;
        let function_instances = filter_owned_vec(resolved.function_instances, |item| {
            authored
                .get(item.template.as_str())
                .is_some_and(|owner| owner.module == program.module)
        })?;
        modules.push(WorkspaceResolvedModule {
            path: crate::bounded_output::budgeted_clone(&program.path),
            module,
            permits: resolved.permits,
            types,
            interfaces: resolved.interfaces,
            functions,
            function_templates,
            function_instances,
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

fn build_operation_sidecar(
    programs: &[Program],
    sources: &[WorkspaceSource],
    modules: &[WorkspaceResolvedModule],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<WorkspaceOperationSidecar, Vec<Diagnostic>> {
    let source_bytes = sources
        .iter()
        .try_fold(0usize, |total, source| {
            total.checked_add(source.source.len())
        })
        .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?;
    let structural_prebound = source_bytes
        .checked_mul(4)
        .and_then(|bytes| {
            bytes.checked_add(programs.len().checked_mul(std::mem::size_of::<Program>())?)
        })
        .and_then(|bytes| {
            bytes.checked_add(authored.len().checked_mul(
                std::mem::size_of::<WorkspaceOperationDeclaration>()
                    + std::mem::size_of::<WorkspaceOperationImport>()
                    + std::mem::size_of::<(String, usize)>(),
            )?)
        })
        .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(structural_prebound)?;
    let mut declarations = Vec::new();
    let mut declaration_index = BTreeMap::new();
    for program in programs {
        for declaration in &program.types {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } => "record",
                TypeDeclarationKind::Variant { .. } => "variant",
            };
            push_operation_declaration(
                &mut declarations,
                &mut declaration_index,
                program,
                &declaration.stable_id,
                kind,
                declaration.explicit_id,
                &declaration.name,
                declaration.name_span,
                declaration.span,
            )?;
        }
        for interface in &program.interfaces {
            push_operation_declaration(
                &mut declarations,
                &mut declaration_index,
                program,
                &interface.stable_id,
                "interface",
                interface.explicit_id,
                &interface.name,
                interface.name_span,
                interface.span,
            )?;
        }
        for function in &program.functions {
            push_operation_declaration(
                &mut declarations,
                &mut declaration_index,
                program,
                &function.stable_id,
                if function.type_parameters.is_empty() {
                    "function"
                } else {
                    "function_template"
                },
                function.explicit_id,
                &function.name,
                function.name_span,
                function.span,
            )?;
        }
    }
    let mut imports = Vec::new();
    let mut import_index = BTreeMap::new();
    for program in programs {
        for module_use in &program.module_uses {
            let target = authored
                .get(module_use.persistent_id.as_str())
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "workspace operations import target is absent",
                    )]
                })?;
            let family_matches = match module_use.kind {
                ModuleUseKind::Function => target.function.is_some(),
                ModuleUseKind::Type => target.ty.is_some(),
            };
            if !target.explicit || !family_matches || target.module != module_use.target_module {
                continue;
            }
            let key = (
                program.path.clone(),
                module_use.kind,
                module_use.persistent_id.clone(),
                module_use.target_module.clone(),
            );
            if import_index.insert(key, imports.len()).is_some() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace operations import binding is duplicated",
                )]);
            }
            imports.push(WorkspaceOperationImport {
                path: crate::bounded_output::budgeted_clone(&program.path),
                kind: match module_use.kind {
                    ModuleUseKind::Function => "function",
                    ModuleUseKind::Type => "type",
                },
                target_id: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                target_module: crate::bounded_output::budgeted_clone(&module_use.target_module),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                occurrences: Vec::new(),
            });
        }
    }
    // Occurrence binding uses this canonical order for logarithmic direct-import
    // lookup; the authenticated key is unique from the construction above.
    imports.sort_by(|left, right| {
        (&left.path, left.kind, &left.target_id, &left.target_module).cmp(&(
            &right.path,
            right.kind,
            &right.target_id,
            &right.target_module,
        ))
    });
    let sources = sources
        .iter()
        .map(|source| (source.path.as_str(), source.source.as_str()))
        .collect::<BTreeMap<_, _>>();
    for program in programs {
        let source = sources.get(program.path.as_str()).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace operations retained source is absent",
            )]
        })?;
        // The lexer can produce at most one token per input byte plus EOF. Debit
        // that conservative envelope before it allocates so operation-sidecar
        // discovery cannot briefly exceed its active builder authority.
        reserve_builder_structure(
            source
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(std::mem::size_of::<crate::lexer::Token>()))
                .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
        )?;
        let tokens = crate::lexer::lex(source, &program.path).map_err(|error| vec![error])?;
        for module_use in &program.module_uses {
            let alias_span = module_use_alias_span(&tokens, module_use)?;
            let family = match module_use.kind {
                ModuleUseKind::Function => "function",
                ModuleUseKind::Type => "type",
            };
            let index = imports
                .binary_search_by(|item| {
                    (&item.path, item.kind, &item.target_id, &item.target_module).cmp(&(
                        &program.path,
                        family,
                        &module_use.persistent_id,
                        &module_use.target_module,
                    ))
                })
                .map_err(|_| operation_sidecar_disagreement())?;
            reserve_builder_structure(std::mem::size_of::<WorkspaceOperationOccurrence>())?;
            imports[index]
                .occurrences
                .push(WorkspaceOperationOccurrence {
                    span: alias_span,
                    owner: None,
                });
        }
        let resolved = modules
            .iter()
            .find(|module| module.path == program.path)
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace operations retained HIR module is absent",
                )]
            })?;
        collect_program_operation_occurrences(
            program,
            resolved,
            &tokens,
            &declaration_index,
            &import_index,
            &mut declarations,
            &mut imports,
        )?;
    }
    declarations.sort_by(|left, right| {
        (&left.path, left.kind, &left.id).cmp(&(&right.path, right.kind, &right.id))
    });
    imports.sort_by(|left, right| {
        (&left.path, left.kind, &left.target_id, &left.target_module).cmp(&(
            &right.path,
            right.kind,
            &right.target_id,
            &right.target_module,
        ))
    });
    for occurrences in declarations
        .iter_mut()
        .map(|item| &mut item.occurrences)
        .chain(imports.iter_mut().map(|item| &mut item.occurrences))
    {
        occurrences.sort_by(|left, right| {
            (left.span.start, left.span.end, &left.owner).cmp(&(
                right.span.start,
                right.span.end,
                &right.owner,
            ))
        });
        if occurrences.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace operations occurrence proof is duplicated",
            )]);
        }
    }
    reserve_builder_structure(
        declarations
            .len()
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
    )?;
    let normalized_fingerprints = declarations
        .iter()
        .map(|declaration| {
            operation_declaration_fingerprint(declaration, &declarations, &imports, &sources)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (declaration, fingerprint) in declarations.iter_mut().zip(normalized_fingerprints) {
        declaration.normalized_fingerprint = fingerprint;
    }
    Ok(WorkspaceOperationSidecar {
        declarations,
        imports,
    })
}

fn operation_declaration_fingerprint(
    declaration: &WorkspaceOperationDeclaration,
    declarations: &[WorkspaceOperationDeclaration],
    imports: &[WorkspaceOperationImport],
    sources: &BTreeMap<&str, &str>,
) -> Result<String, Vec<Diagnostic>> {
    let source = sources
        .get(declaration.path.as_str())
        .ok_or_else(operation_sidecar_disagreement)?;
    source
        .get(declaration.span.start..declaration.span.end)
        .ok_or_else(operation_sidecar_disagreement)?;
    let occurrence_count = declarations
        .iter()
        .map(|target| target.occurrences.len())
        .chain(imports.iter().map(|target| target.occurrences.len()))
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(operation_sidecar_disagreement)?;
    reserve_builder_structure(
        occurrence_count
            .checked_mul(std::mem::size_of::<(Span, &str)>())
            .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
    )?;
    let mut substitutions = declarations
        .iter()
        .flat_map(|target| {
            target.occurrences.iter().filter_map(|occurrence| {
                (occurrence.owner.as_deref() == Some(declaration.id.as_str())
                    && occurrence.span.start >= declaration.span.start
                    && occurrence.span.end <= declaration.span.end)
                    .then_some((occurrence.span, target.id.as_str()))
            })
        })
        .chain(imports.iter().flat_map(|target| {
            target.occurrences.iter().filter_map(|occurrence| {
                (occurrence.owner.as_deref() == Some(declaration.id.as_str())
                    && occurrence.span.start >= declaration.span.start
                    && occurrence.span.end <= declaration.span.end)
                    .then_some((occurrence.span, target.target_id.as_str()))
            })
        }))
        .collect::<Vec<_>>();
    substitutions.sort_by_key(|(span, _)| (span.start, span.end));
    if substitutions
        .windows(2)
        .any(|pair| pair[0].0.end > pair[1].0.start)
    {
        return Err(operation_sidecar_disagreement());
    }
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-workspace-operations.normalized-declaration.v1\0");
    let mut cursor = declaration.span.start;
    for (span, identity) in substitutions {
        hasher.update(&source.as_bytes()[cursor..span.start]);
        hasher.update((identity.len() as u64).to_le_bytes());
        hasher.update(identity.as_bytes());
        cursor = span.end;
    }
    hasher.update(&source.as_bytes()[cursor..declaration.span.end]);
    reserve_builder_structure(71)?;
    Ok(crate::bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )))
}

#[allow(
    clippy::too_many_arguments,
    reason = "sealed declaration-sidecar fact construction keeps every authenticated component explicit"
)]
fn push_operation_declaration(
    declarations: &mut Vec<WorkspaceOperationDeclaration>,
    index: &mut BTreeMap<String, usize>,
    program: &Program,
    id: &str,
    kind: &'static str,
    explicit: bool,
    name: &str,
    name_span: Span,
    span: Span,
) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(
        std::mem::size_of::<WorkspaceOperationDeclaration>()
            + std::mem::size_of::<WorkspaceOperationOccurrence>(),
    )?;
    if index.insert(id.to_owned(), declarations.len()).is_some() {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace operations declaration identity is duplicated",
        )]);
    }
    declarations.push(WorkspaceOperationDeclaration {
        path: crate::bounded_output::budgeted_clone(&program.path),
        module: crate::bounded_output::budgeted_clone(&program.module),
        id: crate::bounded_output::budgeted_clone(id),
        kind,
        explicit,
        name: crate::bounded_output::budgeted_clone(name),
        span,
        normalized_fingerprint: String::new(),
        occurrences: vec![WorkspaceOperationOccurrence {
            span: name_span,
            owner: Some(crate::bounded_output::budgeted_clone(id)),
        }],
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_program_operation_occurrences(
    program: &Program,
    resolved: &WorkspaceResolvedModule,
    tokens: &[crate::lexer::Token],
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    for declaration in &program.types {
        let resolved_declaration = resolved
            .types
            .iter()
            .find(|item| item.id.as_str() == declaration.stable_id)
            .ok_or_else(operation_sidecar_disagreement)?;
        match (&declaration.kind, &resolved_declaration.kind) {
            (
                TypeDeclarationKind::Record { fields },
                hir::ResolvedTypeDeclarationKind::Record {
                    fields: resolved_fields,
                },
            ) => {
                if fields.len() != resolved_fields.len() {
                    return Err(operation_sidecar_disagreement());
                }
                for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                    let mut cursor = field.name_span.end;
                    collect_operation_type_occurrences(
                        program,
                        &field.ty,
                        &resolved_field.ty,
                        tokens,
                        &mut cursor,
                        field.span.end,
                        Some(&declaration.stable_id),
                        declaration_index,
                        import_index,
                        declarations,
                        imports,
                    )?;
                }
            }
            (
                TypeDeclarationKind::Variant { cases },
                hir::ResolvedTypeDeclarationKind::Variant {
                    cases: resolved_cases,
                },
            ) => {
                if cases.len() != resolved_cases.len() {
                    return Err(operation_sidecar_disagreement());
                }
                for (case, resolved_case) in cases.iter().zip(resolved_cases) {
                    if case.fields.len() != resolved_case.fields.len() {
                        return Err(operation_sidecar_disagreement());
                    }
                    for (field, resolved_field) in case.fields.iter().zip(&resolved_case.fields) {
                        let mut cursor = field.name_span.end;
                        collect_operation_type_occurrences(
                            program,
                            &field.ty,
                            &resolved_field.ty,
                            tokens,
                            &mut cursor,
                            field.span.end,
                            Some(&declaration.stable_id),
                            declaration_index,
                            import_index,
                            declarations,
                            imports,
                        )?;
                    }
                }
            }
            (
                TypeDeclarationKind::Resource { .. },
                hir::ResolvedTypeDeclarationKind::Resource { .. },
            ) => {}
            _ => return Err(operation_sidecar_disagreement()),
        }
    }
    for interface in &program.interfaces {
        let resolved_interface = resolved
            .interfaces
            .iter()
            .find(|item| item.id.as_str() == interface.stable_id)
            .ok_or_else(operation_sidecar_disagreement)?;
        if interface.imports.len() != resolved_interface.imports.len() {
            return Err(operation_sidecar_disagreement());
        }
        for (import, resolved_import) in interface.imports.iter().zip(&resolved_interface.imports) {
            if import.params.len() != resolved_import.parameters.len() {
                return Err(operation_sidecar_disagreement());
            }
            for (index, (param, resolved_param)) in import
                .params
                .iter()
                .zip(&resolved_import.parameters)
                .enumerate()
            {
                let mut cursor = param.span.end;
                let end = import
                    .params
                    .get(index + 1)
                    .map_or(import.span.end, |next| next.span.start);
                collect_operation_type_occurrences(
                    program,
                    &param.ty,
                    &resolved_param.ty,
                    tokens,
                    &mut cursor,
                    end,
                    Some(&interface.stable_id),
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
    }
    for function in &program.functions {
        let (resolved_params, resolved_return, requires, body, ensures) =
            if function.type_parameters.is_empty() {
                let item = resolved
                    .functions
                    .iter()
                    .find(|item| item.id.as_str() == function.stable_id)
                    .ok_or_else(operation_sidecar_disagreement)?;
                (
                    item.params.as_slice(),
                    &item.return_type,
                    item.requires.as_slice(),
                    &item.body,
                    item.ensures.as_slice(),
                )
            } else {
                let item = resolved
                    .function_templates
                    .iter()
                    .find(|item| item.id.as_str() == function.stable_id)
                    .ok_or_else(operation_sidecar_disagreement)?;
                (
                    item.params.as_slice(),
                    &item.return_type,
                    item.requires.as_slice(),
                    &item.body,
                    item.ensures.as_slice(),
                )
            };
        if function.params.len() != resolved_params.len()
            || function.requires.len() != requires.len()
            || function.ensures.len() != ensures.len()
        {
            return Err(operation_sidecar_disagreement());
        }
        for (index, (param, resolved_param)) in
            function.params.iter().zip(resolved_params).enumerate()
        {
            let mut cursor = param.span.end;
            let end = function
                .params
                .get(index + 1)
                .map_or(function.body.span.start, |next| next.span.start);
            collect_operation_type_occurrences(
                program,
                &param.ty,
                &resolved_param.ty,
                tokens,
                &mut cursor,
                end,
                Some(&function.stable_id),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        let mut return_cursor = tokens
            .iter()
            .find(|token| {
                token.span.start >= function.name_span.end
                    && token.span.end <= function.body.span.start
                    && token.kind == crate::lexer::TokenKind::Arrow
            })
            .map(|token| token.span.end)
            .ok_or_else(operation_sidecar_disagreement)?;
        collect_operation_type_occurrences(
            program,
            &function.return_type,
            resolved_return,
            tokens,
            &mut return_cursor,
            function.body.span.start,
            Some(&function.stable_id),
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
        for (source, resolved_expression) in function.requires.iter().zip(requires) {
            collect_operation_expr_occurrences(
                program,
                source,
                resolved_expression,
                tokens,
                &function.stable_id,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        collect_operation_expr_occurrences(
            program,
            &function.body,
            body,
            tokens,
            &function.stable_id,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
        for (source, resolved_expression) in function.ensures.iter().zip(ensures) {
            collect_operation_expr_occurrences(
                program,
                source,
                resolved_expression,
                tokens,
                &function.stable_id,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_type_occurrences(
    program: &Program,
    source: &Type,
    resolved: &hir::ResolvedType,
    tokens: &[crate::lexer::Token],
    cursor: &mut usize,
    end: usize,
    owner: Option<&str>,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = source else {
        return if matches!(resolved, hir::ResolvedType::I64 | hir::ResolvedType::Bool) {
            Ok(())
        } else {
            Err(operation_sidecar_disagreement())
        };
    };
    if matches!(resolved, hir::ResolvedType::TypeParameter { .. }) {
        if !arguments.is_empty() {
            return Err(operation_sidecar_disagreement());
        }
        let span = find_identifier_token(tokens, name, *cursor, end)?;
        *cursor = span.end;
        return Ok(());
    }
    let hir::ResolvedType::Nominal {
        declaration,
        arguments: resolved_arguments,
    } = resolved
    else {
        return Err(operation_sidecar_disagreement());
    };
    if arguments.len() != resolved_arguments.len() {
        return Err(operation_sidecar_disagreement());
    }
    let span = find_identifier_token(tokens, name, *cursor, end)?;
    *cursor = span.end;
    push_bound_operation_occurrence(
        program,
        declaration.as_str(),
        ModuleUseKind::Type,
        span,
        owner,
        declaration_index,
        import_index,
        declarations,
        imports,
    )?;
    for (argument, resolved_argument) in arguments.iter().zip(resolved_arguments) {
        collect_operation_type_occurrences(
            program,
            argument,
            resolved_argument,
            tokens,
            cursor,
            end,
            owner,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_expr_occurrences(
    program: &Program,
    source: &Expr,
    resolved: &hir::ResolvedExpr,
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    use hir::ResolvedExprKind as R;
    match (&source.kind, &resolved.kind) {
        (
            ExprKind::Call {
                name,
                type_arguments,
                args,
            },
            R::Call {
                callee,
                type_arguments: resolved_types,
                args: resolved_args,
                ..
            },
        ) => {
            let span = find_identifier_token(tokens, name, source.span.start, source.span.end)?;
            push_bound_operation_occurrence(
                program,
                callee.as_str(),
                ModuleUseKind::Function,
                span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            if type_arguments.len() != resolved_types.len() || args.len() != resolved_args.len() {
                return Err(operation_sidecar_disagreement());
            }
            let mut cursor = span.end;
            for (ty, resolved_ty) in type_arguments.iter().zip(resolved_types) {
                collect_operation_type_occurrences(
                    program,
                    ty,
                    resolved_ty,
                    tokens,
                    &mut cursor,
                    source.span.end,
                    Some(owner),
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
            for (child, resolved_child) in args.iter().zip(resolved_args) {
                collect_operation_expr_occurrences(
                    program,
                    child,
                    resolved_child,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (ExprKind::Unary { value, .. }, R::Unary { value: right, .. })
        | (ExprKind::Try { operand: value }, R::Try { operand: right, .. })
        | (ExprKind::Try { operand: value }, R::TryOption { operand: right, .. }) => {
            collect_operation_expr_occurrences(
                program,
                value,
                right,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::Binary { left, right, .. },
            R::Binary {
                left: resolved_left,
                right: resolved_right,
                ..
            },
        ) => {
            for (child, resolved_child) in [
                (left.as_ref(), resolved_left.as_ref()),
                (right, resolved_right),
            ] {
                collect_operation_expr_occurrences(
                    program,
                    child,
                    resolved_child,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::Block { statements, tail },
            R::Block {
                statements: resolved_statements,
                tail: resolved_tail,
            },
        ) => {
            if statements.len() != resolved_statements.len() {
                return Err(operation_sidecar_disagreement());
            }
            for (statement, resolved_statement) in statements.iter().zip(resolved_statements) {
                let (
                    crate::ast::Statement::Let { value, .. },
                    hir::ResolvedStatement::Let {
                        value: resolved_value,
                        ..
                    },
                ) = (statement, resolved_statement);
                collect_operation_expr_occurrences(
                    program,
                    value,
                    resolved_value,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
            collect_operation_expr_occurrences(
                program,
                tail,
                resolved_tail,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            },
            R::If {
                condition: rc,
                then_branch: rt,
                else_branch: re,
            },
        ) => {
            for (child, resolved_child) in [
                (condition.as_ref(), rc.as_ref()),
                (then_branch, rt),
                (else_branch, re),
            ] {
                collect_operation_expr_occurrences(
                    program,
                    child,
                    resolved_child,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::ConstructRecord {
                type_name,
                type_span,
                type_arguments,
                fields,
            },
            R::ConstructRecord {
                record,
                fields: resolved_fields,
            },
        ) => {
            push_bound_operation_occurrence(
                program,
                record.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_operation_field_values(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            collect_constructor_type_arguments(
                program,
                type_arguments,
                &resolved.ty,
                tokens,
                type_span.end,
                source.span.end,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::ConstructVariant {
                type_name,
                type_span,
                type_arguments,
                fields,
                ..
            },
            R::ConstructVariant {
                variant,
                fields: resolved_fields,
                ..
            },
        ) => {
            push_bound_operation_occurrence(
                program,
                variant.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_operation_field_values(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            collect_constructor_type_arguments(
                program,
                type_arguments,
                &resolved.ty,
                tokens,
                type_span.end,
                source.span.end,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::Match { scrutinee, arms },
            R::Match {
                scrutinee: resolved_scrutinee,
                arms: resolved_arms,
            },
        ) => {
            if arms.len() != resolved_arms.len() {
                return Err(operation_sidecar_disagreement());
            }
            collect_operation_expr_occurrences(
                program,
                scrutinee,
                resolved_scrutinee,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            for (arm, resolved_arm) in arms.iter().zip(resolved_arms) {
                collect_operation_pattern_occurrences(
                    program,
                    &arm.pattern,
                    &resolved_arm.pattern,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
                collect_operation_expr_occurrences(
                    program,
                    &arm.value,
                    &resolved_arm.value,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::UpdateRecord { base, fields },
            R::UpdateRecord {
                base: resolved_base,
                fields: resolved_fields,
                ..
            },
        ) => {
            collect_operation_expr_occurrences(
                program,
                base,
                resolved_base,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_operation_field_values(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::Project { base, .. },
            R::Project {
                base: resolved_base,
                ..
            },
        ) => collect_operation_expr_occurrences(
            program,
            base,
            resolved_base,
            tokens,
            owner,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?,
        (ExprKind::Project { .. }, R::Place(_))
        | (ExprKind::Int(_), R::Int(_))
        | (ExprKind::Bool(_), R::Bool(_))
        | (ExprKind::Var(_), R::Place(_)) => {}
        _ => return Err(operation_sidecar_disagreement()),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_field_values(
    program: &Program,
    fields: &[crate::ast::FieldInitializer],
    resolved_fields: &[hir::ResolvedFieldInitializer],
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    if fields.len() != resolved_fields.len() {
        return Err(operation_sidecar_disagreement());
    }
    for (field, resolved) in fields.iter().zip(resolved_fields) {
        collect_operation_expr_occurrences(
            program,
            &field.value,
            &resolved.value,
            tokens,
            owner,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_constructor_type_arguments(
    program: &Program,
    arguments: &[Type],
    resolved: &hir::ResolvedType,
    tokens: &[crate::lexer::Token],
    start: usize,
    end: usize,
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    let resolved_arguments = match resolved {
        hir::ResolvedType::Nominal { arguments, .. } => arguments.as_slice(),
        _ if arguments.is_empty() => return Ok(()),
        _ => return Err(operation_sidecar_disagreement()),
    };
    if arguments.len() != resolved_arguments.len() {
        return Err(operation_sidecar_disagreement());
    }
    let mut cursor = start;
    for (argument, resolved_argument) in arguments.iter().zip(resolved_arguments) {
        collect_operation_type_occurrences(
            program,
            argument,
            resolved_argument,
            tokens,
            &mut cursor,
            end,
            Some(owner),
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_pattern_occurrences(
    program: &Program,
    source: &crate::ast::MatchPattern,
    resolved: &hir::ResolvedMatchPattern,
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    match (source, resolved) {
        (
            crate::ast::MatchPattern::Variant {
                type_name,
                type_span,
                ..
            },
            hir::ResolvedMatchPattern::Variant { variant, .. },
        ) => {
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            push_bound_operation_occurrence(
                program,
                variant.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            crate::ast::MatchPattern::Record {
                type_name,
                type_span,
                fields,
                ..
            },
            hir::ResolvedMatchPattern::Record {
                record,
                fields: resolved_fields,
                ..
            },
        ) => {
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            push_bound_operation_occurrence(
                program,
                record.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_nested_record_pattern_occurrences(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (crate::ast::MatchPattern::Wildcard { .. }, hir::ResolvedMatchPattern::Wildcard) => {}
        _ => return Err(operation_sidecar_disagreement()),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_nested_record_pattern_occurrences(
    program: &Program,
    fields: &[crate::ast::RecordMatchPatternField],
    resolved_fields: &[hir::ResolvedRecordMatchPatternField],
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    if fields.len() != resolved_fields.len() {
        return Err(operation_sidecar_disagreement());
    }
    for (field, resolved_field) in fields.iter().zip(resolved_fields) {
        match (&field.pattern, &resolved_field.pattern) {
            (
                crate::ast::RecordMatchFieldPattern::Record {
                    type_name,
                    type_span,
                    fields,
                    ..
                },
                hir::ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields: resolved_fields,
                    ..
                },
            ) => {
                if source_text_token(tokens, *type_span)? != type_name {
                    return Err(operation_sidecar_disagreement());
                }
                push_bound_operation_occurrence(
                    program,
                    record.as_str(),
                    ModuleUseKind::Type,
                    *type_span,
                    Some(owner),
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
                collect_nested_record_pattern_occurrences(
                    program,
                    fields,
                    resolved_fields,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
            (
                crate::ast::RecordMatchFieldPattern::Binding { .. },
                hir::ResolvedRecordMatchFieldPattern::Binding(_),
            )
            | (
                crate::ast::RecordMatchFieldPattern::Wildcard { .. },
                hir::ResolvedRecordMatchFieldPattern::Wildcard,
            ) => {}
            _ => return Err(operation_sidecar_disagreement()),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_bound_operation_occurrence(
    program: &Program,
    target_id: &str,
    family: ModuleUseKind,
    span: Span,
    owner: Option<&str>,
    declaration_index: &BTreeMap<String, usize>,
    _import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(std::mem::size_of::<WorkspaceOperationOccurrence>())?;
    let occurrence = WorkspaceOperationOccurrence {
        span,
        owner: owner.map(crate::bounded_output::budgeted_clone),
    };
    let family_text = match family {
        ModuleUseKind::Function => "function",
        ModuleUseKind::Type => "type",
    };
    if let Ok(index) = imports.binary_search_by(|item| {
        (&item.path, item.kind, item.target_id.as_str()).cmp(&(
            &program.path,
            family_text,
            target_id,
        ))
    }) {
        imports[index].occurrences.push(occurrence);
    } else if let Some(index) = declaration_index.get(target_id).copied() {
        if declarations[index].path == program.path {
            declarations[index].occurrences.push(occurrence);
        }
    }
    Ok(())
}

fn find_identifier_token(
    tokens: &[crate::lexer::Token],
    name: &str,
    start: usize,
    end: usize,
) -> Result<Span, Vec<Diagnostic>> {
    let first = tokens.partition_point(|token| token.span.end <= start);
    tokens[first..]
        .iter()
        .take_while(|token| token.span.start < end)
        .find(|token| {
            token.span.start >= start
                && token.span.end <= end
                && matches!(&token.kind, crate::lexer::TokenKind::Ident(value) if value == name)
        })
        .map(|token| token.span)
        .ok_or_else(operation_sidecar_disagreement)
}

fn source_text_token(tokens: &[crate::lexer::Token], span: Span) -> Result<&str, Vec<Diagnostic>> {
    tokens
        .binary_search_by_key(&span.start, |token| token.span.start)
        .ok()
        .and_then(|index| tokens.get(index))
        .filter(|token| token.span == span)
        .and_then(|token| match &token.kind {
            crate::lexer::TokenKind::Ident(value) => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(operation_sidecar_disagreement)
}

fn module_use_alias_span(
    tokens: &[crate::lexer::Token],
    module_use: &crate::ast::ModuleUse,
) -> Result<Span, Vec<Diagnostic>> {
    let first = tokens.partition_point(|token| token.span.end <= module_use.span.start);
    let scoped =
        &tokens[first..tokens.partition_point(|token| token.span.start < module_use.span.end)];
    let mut meaningful = scoped
        .iter()
        .filter(|token| !matches!(token.kind, crate::lexer::TokenKind::Eof));
    let semicolon = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    let alias = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    let keyword = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    match (&keyword.kind, &alias.kind, &semicolon.kind) {
        (
            crate::lexer::TokenKind::Ident(as_keyword),
            crate::lexer::TokenKind::Ident(alias_name),
            crate::lexer::TokenKind::Semicolon,
        ) if as_keyword == "as" && alias_name == &module_use.alias => Ok(alias.span),
        _ => Err(operation_sidecar_disagreement()),
    }
}

fn operation_sidecar_disagreement() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "workspace operations AST/HIR occurrence proof disagrees",
    )]
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
                TypeDeclarationKind::Record { fields } => {
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

fn validate_retained_facts(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    for program in programs {
        let resolved = modules
            .iter()
            .find(|item| item.module == program.module)
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "retained workspace module is missing",
                )]
            })?;
        if resolved.permits != program.permits || resolved.path != program.path {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace module permit/path facts disagree with retained HIR",
            )]);
        }
    }

    let mut actual_type_sites = Vec::new();
    for module in modules {
        let source = programs
            .iter()
            .find(|program| program.module == module.module)
            .expect("retained module belongs to parsed source");
        let imported_type_ids = source
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Type)
            .map(|item| item.persistent_id.as_str())
            .collect::<BTreeSet<_>>();
        for declaration in &module.types {
            match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Resource { .. } => {}
                hir::ResolvedTypeDeclarationKind::Record { fields } => {
                    for (index, field) in fields.iter().enumerate() {
                        let path = crate::bounded_output::budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.id
                        ));
                        collect_resolved_type_sites(
                            declaration.id.as_str(),
                            &field.ty,
                            &path,
                            None,
                            &imported_type_ids,
                            &mut actual_type_sites,
                        )?;
                    }
                }
                hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for (case_index, case) in cases.iter().enumerate() {
                        for (field_index, field) in case.fields.iter().enumerate() {
                            let path = crate::bounded_output::budgeted_format(format_args!(
                                "type.{}.case.{case_index}.field.{field_index}",
                                declaration.id
                            ));
                            collect_resolved_type_sites(
                                declaration.id.as_str(),
                                &field.ty,
                                &path,
                                None,
                                &imported_type_ids,
                                &mut actual_type_sites,
                            )?;
                        }
                    }
                }
            }
        }
        for function in &module.functions {
            collect_resolved_signature_sites(
                &function.id,
                &function.params,
                &function.return_type,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
            collect_resolved_function_type_sites(
                &function.id,
                &function.requires,
                &function.body,
                &function.ensures,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
        }
        for template in &module.function_templates {
            collect_resolved_signature_sites(
                &template.id,
                &template.params,
                &template.return_type,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
            collect_resolved_function_type_sites(
                &template.id,
                &template.requires,
                &template.body,
                &template.ensures,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
        }
    }
    let mut expected_type_sites = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "type_reference") {
        reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
        expected_type_sites.push((
            crate::bounded_output::budgeted_clone(&edge.caller),
            crate::bounded_output::budgeted_clone(&edge.expression),
            crate::bounded_output::budgeted_clone(&edge.ast_path),
            crate::bounded_output::budgeted_clone(&edge.target),
        ));
    }
    expected_type_sites.sort();
    actual_type_sites.sort();
    if expected_type_sites != actual_type_sites {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace explicit type-reference facts disagree with retained HIR",
        )]);
    }

    let authenticated_calls = reconstruct_authenticated_call_edges(programs, modules)?;
    validate_retained_call_projection(programs, modules, &authenticated_calls)?;
    let mut emitted_calls = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "call") {
        push_edge(&mut emitted_calls, budgeted_edge_clone(edge))?;
    }
    emitted_calls.sort();
    if emitted_calls != authenticated_calls {
        return Err(vec![graph_error(
            "SPX-G173",
            "emitted workspace call edges disagree with authenticated AST/HIR occurrences",
        )]);
    }
    validate_effect_and_capability_edges_against_calls(modules, edges, &authenticated_calls)?;
    Ok(())
}

fn reconstruct_authenticated_call_edges(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
) -> Result<Vec<WorkspaceEdge>, Vec<Diagnostic>> {
    let module_paths = modules
        .iter()
        .map(|module| (module.module.as_str(), module.path.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Vec::new();
    for program in programs {
        let function_uses = program
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Function)
            .map(|item| (item.alias.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        for function in &program.functions {
            let owner =
                hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&function.stable_id));
            for (site, expressions) in [
                ("requires", function.requires.as_slice()),
                ("body", std::slice::from_ref(&function.body)),
                ("ensures", function.ensures.as_slice()),
            ] {
                for (root_index, expression) in expressions.iter().enumerate() {
                    let root = match site {
                        "requires" => crate::bounded_output::budgeted_format(format_args!(
                            "requires.{root_index}"
                        )),
                        "body" => crate::bounded_output::budgeted_clone("body"),
                        "ensures" => crate::bounded_output::budgeted_format(format_args!(
                            "ensures.{root_index}"
                        )),
                        _ => unreachable!(),
                    };
                    let mut ordinal = 0usize;
                    visit_ast_call_sites(expression, &root, &mut |name, path| {
                        let call_ordinal = ordinal;
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or_else(|| vec![limit_error("calls", MAX_CALLS)])?;
                        let Some(module_use) = function_uses.get(name) else {
                            return Ok(());
                        };
                        let target_path = module_paths
                            .get(module_use.target_module.as_str())
                            .ok_or_else(|| {
                                vec![graph_error(
                                    "SPX-G173",
                                    "authenticated call target module has no retained path",
                                )]
                            })?;
                        push_edge(
                            &mut calls,
                            WorkspaceEdge {
                                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                                caller: crate::bounded_output::budgeted_clone(&function.stable_id),
                                target_path: crate::bounded_output::budgeted_clone(target_path),
                                target: crate::bounded_output::budgeted_clone(
                                    &module_use.persistent_id,
                                ),
                                kind: "call",
                                site,
                                expression: hir::workspace_expression_identity(&owner, path),
                                ast_path: crate::bounded_output::budgeted_clone(path),
                                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                                ordinal: call_ordinal,
                            },
                        )
                    })?;
                }
            }
        }
    }
    calls.sort();
    Ok(calls)
}

fn validate_retained_call_projection(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
    authenticated_calls: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut actual = Vec::new();
    for module in modules {
        let imported_targets = programs
            .iter()
            .find(|program| program.module == module.module)
            .expect("retained module belongs to authenticated source")
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Function)
            .map(|item| item.persistent_id.as_str())
            .collect::<BTreeSet<_>>();
        for function in &module.functions {
            collect_retained_call_projection(
                &function.id,
                &function.requires,
                &function.body,
                &function.ensures,
                &imported_targets,
                &mut actual,
            )?;
        }
        for template in &module.function_templates {
            collect_retained_call_projection(
                &template.id,
                &template.requires,
                &template.body,
                &template.ensures,
                &imported_targets,
                &mut actual,
            )?;
        }
    }
    let mut expected = Vec::new();
    for edge in authenticated_calls {
        reserve_builder_structure(std::mem::size_of::<(String, String, String)>())?;
        expected.push((
            crate::bounded_output::budgeted_clone(&edge.caller),
            crate::bounded_output::budgeted_clone(&edge.expression),
            crate::bounded_output::budgeted_clone(&edge.target),
        ));
    }
    expected.sort();
    actual.sort();
    if actual != expected {
        return Err(vec![graph_error(
            "SPX-G173",
            "authenticated workspace call occurrences disagree with retained HIR",
        )]);
    }
    Ok(())
}

fn collect_retained_call_projection(
    owner: &hir::DeclarationId,
    requires: &[hir::ResolvedExpr],
    body: &hir::ResolvedExpr,
    ensures: &[hir::ResolvedExpr],
    imported_targets: &BTreeSet<&str>,
    output: &mut Vec<(String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let mut error = None;
    for expression in requires.iter().chain(std::iter::once(body)).chain(ensures) {
        visit_resolved_calls(expression, &mut |expression, target| {
            if error.is_none() && imported_targets.contains(target.as_str()) {
                if let Err(diagnostics) =
                    reserve_builder_structure(std::mem::size_of::<(String, String, String)>())
                {
                    error = Some(diagnostics);
                    return;
                }
                output.push((
                    crate::bounded_output::budgeted_clone(owner.as_str()),
                    crate::bounded_output::budgeted_format(format_args!("{}", expression.id)),
                    crate::bounded_output::budgeted_clone(target.as_str()),
                ));
            }
        });
    }
    match error {
        Some(diagnostics) => Err(diagnostics),
        None => Ok(()),
    }
}

fn visit_resolved_calls(
    expression: &hir::ResolvedExpr,
    visit: &mut impl FnMut(&hir::ResolvedExpr, &hir::DeclarationId),
) {
    match &expression.kind {
        hir::ResolvedExprKind::Call { callee, args, .. } => {
            visit(expression, callee);
            for argument in args {
                visit_resolved_calls(argument, visit);
            }
        }
        hir::ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                visit_resolved_calls(argument, visit);
            }
        }
        hir::ResolvedExprKind::Unary { value, .. } => visit_resolved_calls(value, visit),
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            visit_resolved_calls(left, visit);
            visit_resolved_calls(right, visit);
        }
        hir::ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let hir::ResolvedStatement::Let { value, .. } = statement;
                visit_resolved_calls(value, visit);
            }
            visit_resolved_calls(tail, visit);
        }
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_resolved_calls(condition, visit);
            visit_resolved_calls(then_branch, visit);
            visit_resolved_calls(else_branch, visit);
        }
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        hir::ResolvedExprKind::Match { scrutinee, arms } => {
            visit_resolved_calls(scrutinee, visit);
            for arm in arms {
                visit_resolved_calls(&arm.value, visit);
            }
        }
        hir::ResolvedExprKind::Try { operand, .. }
        | hir::ResolvedExprKind::TryOption { operand, .. } => {
            visit_resolved_calls(operand, visit);
        }
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_resolved_calls(base, visit);
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        hir::ResolvedExprKind::Project { base, .. } => visit_resolved_calls(base, visit),
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::Place(_) => {}
    }
}

fn validate_effect_and_capability_edges(
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut calls = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "call") {
        push_edge(&mut calls, budgeted_edge_clone(edge))?;
    }
    validate_effect_and_capability_edges_against_calls(modules, edges, &calls)
}

fn validate_effect_and_capability_edges_against_calls(
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
    authenticated_calls: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut modules_by_path = BTreeMap::new();
    let mut target_functions = BTreeMap::new();
    let mut target_effects = BTreeMap::new();
    let mut caller_effects = BTreeMap::new();
    let mut module_permits = BTreeMap::new();
    for module in modules {
        if modules_by_path
            .insert(module.path.as_str(), module)
            .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "retained workspace module paths are not unique",
            )]);
        }
        let permits = module
            .permits
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if permits.len() != module.permits.len()
            || module_permits
                .insert(module.module.as_str(), permits)
                .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "retained workspace module capability authority is not canonical",
            )]);
        }
        for function in &module.functions {
            let effects = function
                .effects
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if effects.len() != function.effects.len() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace function effects are not canonical",
                )]);
            }
            if target_functions
                .insert(function.id.as_str(), module)
                .is_some()
                || target_effects
                    .insert(function.id.as_str(), function.effects.as_slice())
                    .is_some()
                || caller_effects
                    .insert((module.module.as_str(), function.id.as_str()), effects)
                    .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace function authority is duplicated",
                )]);
            }
        }
        for template in &module.function_templates {
            let effects = template
                .effects
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if effects.len() != template.effects.len() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace template effects are not canonical",
                )]);
            }
            if caller_effects
                .insert((module.module.as_str(), template.id.as_str()), effects)
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace callable authority is duplicated",
                )]);
            }
        }
    }

    let mut calls = BTreeMap::new();
    let mut actual_effects = BTreeMap::<CallOccurrenceKey<'_>, BTreeSet<&str>>::new();
    for call in authenticated_calls {
        if calls
            .insert(CallOccurrenceKey::from_edge(call), call)
            .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "authenticated workspace call occurrence is duplicated",
            )]);
        }
    }
    for edge in edges {
        if edge.kind == "effect_requirement"
            && !actual_effects
                .entry(CallOccurrenceKey::from_edge(edge))
                .or_default()
                .insert(edge.target.as_str())
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace call effect requirement is duplicated",
            )]);
        }
    }

    for (occurrence, call) in calls {
        let caller_module = modules_by_path.get(occurrence.caller_path).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace call path has no retained module authority",
            )]
        })?;
        let target_module = target_functions.get(call.target.as_str()).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace call target has no retained function authority",
            )]
        })?;
        let required = target_effects
            .get(call.target.as_str())
            .expect("retained target effect authority was indexed");
        if target_module.path != call.target_path {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace call target path disagrees with retained authority",
            )]);
        }
        let actual = actual_effects.remove(&occurrence).unwrap_or_default();
        if actual.len() != required.len()
            || required
                .iter()
                .any(|effect| !actual.contains(effect.as_str()))
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace call effect requirements disagree with retained target HIR",
            )]);
        }
        let declared = caller_effects
            .get(&(caller_module.module.as_str(), occurrence.caller))
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace call owner has no retained callable authority",
                )]
            })?;
        let permits = module_permits
            .get(caller_module.module.as_str())
            .expect("retained module permit authority was indexed");
        if required
            .iter()
            .any(|effect| !declared.contains(effect.as_str()) || !permits.contains(effect.as_str()))
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace caller effect/capability authority join disagrees",
            )]);
        }
    }
    if !actual_effects.is_empty() {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace effect requirement has no exact call occurrence",
        )]);
    }

    let mut expected_capabilities = Vec::new();
    for module in modules {
        for (ordinal, permit) in module.permits.iter().enumerate() {
            let path = crate::bounded_output::budgeted_format(format_args!("permit.{ordinal}"));
            push_edge(
                &mut expected_capabilities,
                WorkspaceEdge {
                    caller_path: crate::bounded_output::budgeted_clone(&module.path),
                    caller: crate::bounded_output::budgeted_clone(&module.module),
                    target_path: crate::bounded_output::budgeted_clone(&module.path),
                    target: crate::bounded_output::budgeted_clone(permit),
                    kind: "capability_authority",
                    site: "module",
                    expression: crate::bounded_output::budgeted_clone(&path),
                    ast_path: path,
                    alias: String::new(),
                    ordinal,
                },
            )?;
        }
    }
    let mut actual_capabilities = Vec::new();
    for edge in edges
        .iter()
        .filter(|edge| edge.kind == "capability_authority")
    {
        push_edge(&mut actual_capabilities, budgeted_edge_clone(edge))?;
    }
    expected_capabilities.sort();
    actual_capabilities.sort();
    if actual_capabilities != expected_capabilities {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace capability-authority edges disagree with retained module permits",
        )]);
    }
    Ok(())
}

fn collect_resolved_signature_sites(
    owner: &hir::DeclarationId,
    params: &[hir::ResolvedParam],
    result: &hir::ResolvedType,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    for (index, param) in params.iter().enumerate() {
        let path =
            crate::bounded_output::budgeted_format(format_args!("function.{owner}.param.{index}"));
        collect_resolved_type_sites(owner.as_str(), &param.ty, &path, None, imported, out)?;
    }
    let path = crate::bounded_output::budgeted_format(format_args!("function.{owner}.return"));
    collect_resolved_type_sites(owner.as_str(), result, &path, None, imported, out)?;
    Ok(())
}

fn collect_resolved_type_sites(
    owner: &str,
    ty: &hir::ResolvedType,
    path: &str,
    expression: Option<&str>,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    if imported.contains(declaration.as_str()) {
        reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
        out.push((
            crate::bounded_output::budgeted_clone(owner),
            crate::bounded_output::budgeted_clone(expression.unwrap_or(path)),
            crate::bounded_output::budgeted_clone(path),
            crate::bounded_output::budgeted_clone(declaration.as_str()),
        ));
    }
    for (index, argument) in arguments.iter().enumerate() {
        collect_resolved_type_sites(
            owner,
            argument,
            &crate::bounded_output::budgeted_format(format_args!("{path}.argument.{index}")),
            expression,
            imported,
            out,
        )?;
    }
    Ok(())
}

fn collect_resolved_function_type_sites(
    owner: &hir::DeclarationId,
    requires: &[hir::ResolvedExpr],
    body: &hir::ResolvedExpr,
    ensures: &[hir::ResolvedExpr],
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    for (root, expression) in requires
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            (
                crate::bounded_output::budgeted_format(format_args!("requires.{index}")),
                expression,
            )
        })
        .chain(std::iter::once((
            crate::bounded_output::budgeted_clone("body"),
            body,
        )))
        .chain(ensures.iter().enumerate().map(|(index, expression)| {
            (
                crate::bounded_output::budgeted_format(format_args!("ensures.{index}")),
                expression,
            )
        }))
    {
        collect_resolved_expression_type_sites(owner, expression, &root, imported, out)?;
    }
    Ok(())
}

fn collect_resolved_expression_type_sites(
    owner: &hir::DeclarationId,
    expression: &hir::ResolvedExpr,
    path: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let expression_id = crate::bounded_output::budgeted_format(format_args!("{}", expression.id));
    match &expression.kind {
        hir::ResolvedExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                collect_resolved_type_sites(
                    owner.as_str(),
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.type_argument.{index}"
                    )),
                    Some(&expression_id),
                    imported,
                    out,
                )?;
            }
            for (index, argument) in args.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}")),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::NativeRustImportCall(call) => {
            for (index, argument) in call.args.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.native_rust_arg.{index}"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Unary { value, .. } => collect_resolved_expression_type_sites(
            owner,
            value,
            &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
            imported,
            out,
        )?,
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            collect_resolved_expression_type_sites(
                owner,
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                imported,
                out,
            )?;
            collect_resolved_expression_type_sites(
                owner,
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                let hir::ResolvedStatement::Let { value, .. } = statement;
                collect_resolved_expression_type_sites(
                    owner,
                    value,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.s{index}.value")),
                    imported,
                    out,
                )?;
            }
            collect_resolved_expression_type_sites(
                owner,
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for (suffix, child) in [
                ("condition", condition.as_ref()),
                ("then", then_branch.as_ref()),
                ("else", else_branch.as_ref()),
            ] {
                collect_resolved_expression_type_sites(
                    owner,
                    child,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.{suffix}")),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
            collect_resolved_type_sites(
                owner.as_str(),
                &expression.ty,
                &crate::bounded_output::budgeted_format(format_args!("{path}.type")),
                Some(&expression_id),
                imported,
                out,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Match { scrutinee, arms } => {
            collect_resolved_expression_type_sites(
                owner,
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                imported,
                out,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                collect_resolved_pattern_type_sites(
                    owner.as_str(),
                    &arm.pattern,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.pattern"
                    )),
                    &expression_id,
                    imported,
                    out,
                )?;
                collect_resolved_expression_type_sites(
                    owner,
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Try { operand, .. }
        | hir::ResolvedExprKind::TryOption { operand, .. } => {
            collect_resolved_expression_type_sites(
                owner,
                operand,
                &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_resolved_expression_type_sites(
                owner,
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                imported,
                out,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Project { base, .. } => collect_resolved_expression_type_sites(
            owner,
            base,
            &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
            imported,
            out,
        )?,
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::Place(_) => {}
    }
    Ok(())
}

fn collect_resolved_pattern_type_sites(
    owner: &str,
    pattern: &hir::ResolvedMatchPattern,
    path: &str,
    expression: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    match pattern {
        hir::ResolvedMatchPattern::Variant { variant, .. } => {
            if imported.contains(variant.as_str()) {
                reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
                out.push((
                    crate::bounded_output::budgeted_clone(owner),
                    crate::bounded_output::budgeted_clone(expression),
                    crate::bounded_output::budgeted_clone(path),
                    crate::bounded_output::budgeted_clone(variant.as_str()),
                ));
            }
        }
        hir::ResolvedMatchPattern::Record { record, fields, .. } => {
            if imported.contains(record.as_str()) {
                reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
                out.push((
                    crate::bounded_output::budgeted_clone(owner),
                    crate::bounded_output::budgeted_clone(expression),
                    crate::bounded_output::budgeted_clone(path),
                    crate::bounded_output::budgeted_clone(record.as_str()),
                ));
            }
            for (index, field) in fields.iter().enumerate() {
                collect_resolved_record_pattern_type_sites(
                    owner,
                    &field.pattern,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.pattern"
                    )),
                    expression,
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedMatchPattern::Wildcard => {}
    }
    Ok(())
}

fn collect_resolved_record_pattern_type_sites(
    owner: &str,
    pattern: &hir::ResolvedRecordMatchFieldPattern,
    path: &str,
    expression: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let hir::ResolvedRecordMatchFieldPattern::Record { record, fields, .. } = pattern else {
        return Ok(());
    };
    if imported.contains(record.as_str()) {
        reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
        out.push((
            crate::bounded_output::budgeted_clone(owner),
            crate::bounded_output::budgeted_clone(expression),
            crate::bounded_output::budgeted_clone(path),
            crate::bounded_output::budgeted_clone(record.as_str()),
        ));
    }
    for (index, field) in fields.iter().enumerate() {
        collect_resolved_record_pattern_type_sites(
            owner,
            &field.pattern,
            &crate::bounded_output::budgeted_format(format_args!("{path}.field.{index}.pattern")),
            expression,
            imported,
            out,
        )?;
    }
    Ok(())
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
            TypeDeclarationKind::Record { fields } => count = count.checked_add(fields.len())?,
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
                TypeDeclarationKind::Record { fields } => {
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
            }
        }
    }
    Ok(())
}

fn type_contains_name_from(ty: &Type, names: &BTreeSet<&str>) -> bool {
    match ty {
        Type::I64 | Type::Bool => false,
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
    if !function.type_parameters.is_empty()
        || function
            .params
            .iter()
            .any(|param| param.mode != ParamMode::Value)
    {
        return Err(vec![use_error(
            caller,
            module_use,
            "function target must be monomorphic with value parameters",
        )]);
    }
    for ty in function
        .params
        .iter()
        .map(|param| &param.ty)
        .chain(std::iter::once(&function.return_type))
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
        Type::I64 | Type::Bool => true,
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
        TypeDeclarationKind::Record { fields } => fields.iter().all(|field| {
            exposed_type_reference_is_directly_imported(
                caller, module, &field.ty, authored, programs, visiting,
            )
        }),
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
        Type::I64 | Type::Bool => true,
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
        TypeDeclarationKind::Record { fields } => fields.iter().all(|field| {
            type_reference_is_admitted(module, &field.ty, authored, programs, visiting)
        }),
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
        Type::I64 | Type::Bool => true,
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

struct SyntheticBuilderCosts {
    raw_clone_and_hir: usize,
    runtime: usize,
}

#[derive(Clone, Copy)]
struct ExpandedDefaultCost {
    bytes: usize,
    identity_slots: usize,
}

#[derive(Clone, Copy)]
struct GenericInstanceCost {
    bytes: usize,
    identity_slots: usize,
}

struct StructuralCost(usize);

impl StructuralCost {
    fn add(&mut self, bytes: usize) -> Result<(), Vec<Diagnostic>> {
        self.0 = checked_usage(self.0, bytes, "builder_bytes", active_builder_limit())?;
        Ok(())
    }

    fn value<T>(&mut self, value: &T) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of_val(value))
    }

    fn string(&mut self, value: &str) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of::<String>())?;
        self.add(value.len())
    }
}

fn synthetic_builder_bytes(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    let mut raw = StructuralCost(0);
    ast_program_cost(program, &mut raw)?;
    let mut identity_slots = ast_program_identity_slots(program)?;
    let mut runtime = StructuralCost(0);
    let mut default_memo = BTreeMap::new();
    for module_use in &program.module_uses {
        let target = &authored[module_use.persistent_id.as_str()];
        runtime.string(&module_use.alias)?;
        if let Some(function) = target.function {
            ast_function_cost(function, &mut raw)?;
            identity_slots = checked_builder_sum(
                identity_slots,
                ast_type_identity_slots(&function.return_type)?,
            )?;
            for param in &function.params {
                identity_slots =
                    checked_builder_sum(identity_slots, ast_type_identity_slots(&param.ty)?)?;
                rewrite_type_runtime_cost(
                    &param.ty,
                    target.module,
                    program,
                    programs,
                    &mut runtime,
                )?;
            }
            rewrite_type_runtime_cost(
                &function.return_type,
                target.module,
                program,
                programs,
                &mut runtime,
            )?;
            let cost = default_expr_expanded_cost(
                &function.return_type,
                target.module,
                program,
                authored,
                programs,
                &mut default_memo,
                &mut BTreeSet::new(),
            )?;
            runtime.add(cost.bytes)?;
            identity_slots = checked_builder_sum(identity_slots, cost.identity_slots)?;
        } else {
            let declaration = target.ty.expect("validated type target");
            ast_type_declaration_cost(declaration, &mut raw)?;
            identity_slots = checked_builder_sum(
                identity_slots,
                ast_type_declaration_identity_slots(declaration)?,
            )?;
            rewrite_type_declaration_runtime_cost(
                declaration,
                target.module,
                program,
                programs,
                &mut runtime,
            )?;
        }
    }
    if !program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        runtime.add(synthetic_main_runtime_cost(&program.module)?)?;
    }
    let generic_instances = generic_instance_source_cost(program)?;
    identity_slots = checked_builder_sum(identity_slots, generic_instances.identity_slots)?;
    let hir_input = checked_usage(raw.0, runtime.0, "builder_bytes", active_builder_limit())?;
    let hir_input = checked_usage(
        hir_input,
        generic_instances.bytes,
        "builder_bytes",
        active_builder_limit(),
    )?;
    let fixed_hir_upper = hir_input
        .checked_mul(HIR_FIXED_EXPANSION_FACTOR)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let maximum_identity_bytes = authored
        .keys()
        .map(|id| id.len())
        .chain(prelude::all_ids().into_iter().map(str::len))
        .max()
        .unwrap_or(0);
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
        .and_then(|bytes| bytes.checked_mul(HIR_IDENTITY_COPY_FACTOR))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let hir_upper = fixed_hir_upper
        .checked_add(identity_occurrence_upper)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    if hir_upper > active_builder_limit() {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    Ok(SyntheticBuilderCosts {
        raw_clone_and_hir: checked_usage(
            raw.0,
            hir_upper,
            "builder_bytes",
            active_builder_limit(),
        )?,
        runtime: runtime.0,
    })
}

fn generic_instance_source_cost(program: &Program) -> Result<GenericInstanceCost, Vec<Diagnostic>> {
    let mut templates = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        if function.type_parameters.is_empty() {
            continue;
        }
        let mut cost = StructuralCost(0);
        ast_function_cost(function, &mut cost)?;
        templates.push((
            function.name.as_str(),
            GenericInstanceCost {
                bytes: cost.0,
                identity_slots: ast_function_identity_slots(function)?,
            },
        ));
    }
    templates.sort_by(|left, right| left.0.cmp(right.0));
    let mut total = GenericInstanceCost {
        bytes: 0,
        identity_slots: 0,
    };
    for function in program
        .functions
        .iter()
        .filter(|function| function.type_parameters.is_empty())
    {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut overflowed = false;
            expression.visit_call_instances(&mut |name, arguments, _| {
                if arguments.is_empty() || overflowed {
                    return;
                }
                if let Ok(index) = templates.binary_search_by_key(&name, |(name, _)| *name) {
                    if let (Some(bytes), Some(identity_slots)) = (
                        total.bytes.checked_add(templates[index].1.bytes),
                        total
                            .identity_slots
                            .checked_add(templates[index].1.identity_slots),
                    ) {
                        total = GenericInstanceCost {
                            bytes,
                            identity_slots,
                        };
                    } else {
                        overflowed = true;
                    }
                }
            });
            if overflowed || total.bytes > active_builder_limit() {
                return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
            }
        }
    }
    Ok(total)
}

fn checked_builder_sum(left: usize, right: usize) -> Result<usize, Vec<Diagnostic>> {
    left.checked_add(right)
        .filter(|total| *total <= active_builder_limit())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

fn rewrite_type_declaration_runtime_cost(
    declaration: &TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                rewrite_type_runtime_cost(&field.ty, target_module, caller, programs, cost)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    rewrite_type_runtime_cost(&field.ty, target_module, caller, programs, cost)?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}

fn rewrite_type_runtime_cost(
    ty: &Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(vec![graph_error(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )]);
    }
    let target_id = resolve_type_id(target_module, name, programs).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "cross-file type identity cost lookup disagrees",
        )]
    })?;
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G172",
                crate::bounded_output::budgeted_format(format_args!(
                    "cross-file signature type `{target_id}` is not explicitly imported"
                )),
            )]
        })?;
    cost.string(alias)
}

fn synthetic_main_runtime_cost(module: &str) -> Result<usize, Vec<Diagnostic>> {
    let mut cost = StructuralCost(0);
    cost.add(std::mem::size_of::<Function>())?;
    cost.add(std::mem::size_of::<Expr>())?;
    cost.string("main")?;
    cost.add("workspace.synthetic.main.".len())?;
    cost.add(module.len())?;
    Ok(cost.0)
}

fn ast_program_cost(program: &Program, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(program)?;
    cost.string(&program.path)?;
    cost.string(&program.module)?;
    for module_use in &program.module_uses {
        cost.value(module_use)?;
        cost.string(&module_use.persistent_id)?;
        cost.string(&module_use.target_module)?;
        cost.string(&module_use.alias)?;
    }
    for permit in &program.permits {
        cost.string(permit)?;
    }
    for declaration in &program.types {
        ast_type_declaration_cost(declaration, cost)?;
    }
    for interface in &program.interfaces {
        cost.value(interface)?;
        cost.string(&interface.stable_id)?;
        cost.string(&interface.name)?;
        for permit in &interface.permits {
            cost.string(permit)?;
        }
        for import in &interface.imports {
            cost.value(import)?;
            cost.string(&import.stable_id)?;
            cost.string(&import.name)?;
            for param in &import.params {
                ast_param_cost(param, cost)?;
            }
            for effect in &import.effects {
                cost.string(effect)?;
            }
            if let crate::ast::ImportFailure::Status { domain_id } = &import.failure {
                cost.string(domain_id)?;
            }
            cost.string(&import.consumes)?;
        }
    }
    for function in &program.functions {
        ast_function_cost(function, cost)?;
    }
    Ok(())
}

fn ast_program_identity_slots(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 0usize;
    for declaration in &program.types {
        slots = checked_builder_sum(slots, ast_type_declaration_identity_slots(declaration)?)?;
    }
    for interface in &program.interfaces {
        for import in &interface.imports {
            for param in &import.params {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&param.ty)?)?;
            }
        }
    }
    for function in &program.functions {
        slots = checked_builder_sum(slots, ast_function_identity_slots(function)?)?;
    }
    Ok(slots)
}

fn ast_type_declaration_identity_slots(
    declaration: &TypeDeclaration,
) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 0usize;
    match &declaration.kind {
        TypeDeclarationKind::Resource { .. } => {}
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
                }
            }
        }
    }
    Ok(slots)
}

fn ast_function_identity_slots(function: &Function) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = ast_type_identity_slots(&function.return_type)?;
    for param in &function.params {
        slots = checked_builder_sum(slots, ast_type_identity_slots(&param.ty)?)?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        slots = checked_builder_sum(slots, ast_expr_identity_slots(expression)?)?;
    }
    Ok(slots)
}

fn ast_type_identity_slots(ty: &Type) -> Result<usize, Vec<Diagnostic>> {
    let Type::Named { arguments, .. } = ty else {
        return Ok(0);
    };
    let mut slots = 1usize;
    for argument in arguments {
        slots = checked_builder_sum(slots, ast_type_identity_slots(argument)?)?;
    }
    Ok(slots)
}

fn ast_expr_identity_slots(expression: &Expr) -> Result<usize, Vec<Diagnostic>> {
    // Eight covers the expression/result/callee, Try's six declaration IDs,
    // and one cleanup owner. Variable-size field/projection/pattern IDs are
    // debited separately below.
    let mut slots = 8usize;
    match &expression.kind {
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::Unary { value, .. } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(value)?)?;
        }
        ExprKind::Binary { left, right, .. } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(left)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(right)?)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                let crate::ast::Statement::Let { value, .. } = statement;
                slots = checked_builder_sum(slots, 1)?;
                slots = checked_builder_sum(slots, ast_expr_identity_slots(value)?)?;
            }
            slots = checked_builder_sum(slots, ast_expr_identity_slots(tail)?)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(condition)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(then_branch)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(else_branch)?)?;
        }
        ExprKind::ConstructRecord {
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_arguments,
            fields,
            ..
        } => {
            slots = checked_builder_sum(slots, fields.len())?;
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            for field in fields {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&field.value)?)?;
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(scrutinee)?)?;
            for arm in arms {
                slots = checked_builder_sum(slots, ast_pattern_identity_slots(&arm.pattern)?)?;
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&arm.value)?)?;
            }
        }
        ExprKind::Try { operand } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(operand)?)?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            slots = checked_builder_sum(slots, fields.len())?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(base)?)?;
            for field in fields {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&field.value)?)?;
            }
        }
        ExprKind::Project { base, .. } => {
            slots = checked_builder_sum(slots, 1)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(base)?)?;
        }
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {}
    }
    Ok(slots)
}

fn ast_pattern_identity_slots(
    pattern: &crate::ast::MatchPattern,
) -> Result<usize, Vec<Diagnostic>> {
    match pattern {
        crate::ast::MatchPattern::Variant { fields, .. } => {
            let field_slots = fields
                .len()
                .checked_mul(2)
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            checked_builder_sum(2, field_slots)
        }
        crate::ast::MatchPattern::Record { fields, .. } => {
            let mut slots = 1usize;
            for field in fields {
                slots = checked_builder_sum(slots, record_pattern_identity_slots(field)?)?;
            }
            Ok(slots)
        }
        crate::ast::MatchPattern::Wildcard { .. } => Ok(0),
    }
}

fn record_pattern_identity_slots(
    field: &crate::ast::RecordMatchPatternField,
) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 1usize;
    if let crate::ast::RecordMatchFieldPattern::Record { fields, .. } = &field.pattern {
        slots = checked_builder_sum(slots, 1)?;
        for nested in fields {
            slots = checked_builder_sum(slots, record_pattern_identity_slots(nested)?)?;
        }
    }
    Ok(slots)
}

fn ast_type_declaration_cost(
    declaration: &TypeDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(declaration)?;
    cost.string(&declaration.stable_id)?;
    cost.string(&declaration.name)?;
    for parameter in &declaration.type_parameters {
        cost.value(parameter)?;
        cost.string(&parameter.name)?;
    }
    match &declaration.kind {
        TypeDeclarationKind::Resource { lifecycles } => {
            for lifecycle in lifecycles {
                cost.value(lifecycle)?;
                if let Some(id) = &lifecycle.stable_id {
                    cost.string(id)?;
                }
                if let crate::ast::ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind
                {
                    cost.string(import_key)?;
                }
            }
        }
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                ast_field_cost(field, cost)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                cost.value(case)?;
                cost.string(&case.stable_id)?;
                cost.string(&case.name)?;
                for field in &case.fields {
                    ast_field_cost(field, cost)?;
                }
            }
        }
    }
    Ok(())
}

fn ast_field_cost(
    field: &crate::ast::FieldDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(field)?;
    cost.string(&field.stable_id)?;
    cost.string(&field.name)?;
    ast_type_cost(&field.ty, cost)
}

fn ast_function_cost(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(function)?;
    cost.string(&function.stable_id)?;
    cost.string(&function.name)?;
    for parameter in &function.type_parameters {
        cost.value(parameter)?;
        cost.string(&parameter.name)?;
    }
    for param in &function.params {
        ast_param_cost(param, cost)?;
    }
    ast_type_cost(&function.return_type, cost)?;
    for effect in &function.effects {
        cost.string(effect)?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        ast_expr_cost(expression, cost)?;
    }
    Ok(())
}

fn ast_param_cost(
    param: &crate::ast::Param,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(param)?;
    cost.string(&param.name)?;
    ast_type_cost(&param.ty, cost)
}

fn ast_type_cost(ty: &Type, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(ty)?;
    if let Type::Named { name, arguments } = ty {
        cost.string(name)?;
        for argument in arguments {
            ast_type_cost(argument, cost)?;
        }
    }
    Ok(())
}

fn ast_expr_cost(expression: &Expr, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(expression)?;
    match &expression.kind {
        ExprKind::Var(name) => cost.string(name)?,
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            cost.string(name)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::Unary { value, .. } => ast_expr_cost(value, cost)?,
        ExprKind::Binary { left, right, .. } => {
            ast_expr_cost(left, cost)?;
            ast_expr_cost(right, cost)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                cost.value(statement)?;
                let crate::ast::Statement::Let { name, value, .. } = statement;
                cost.string(name)?;
                ast_expr_cost(value, cost)?;
            }
            ast_expr_cost(tail, cost)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_cost(condition, cost)?;
            ast_expr_cost(then_branch, cost)?;
            ast_expr_cost(else_branch, cost)?;
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            cost.string(type_name)?;
            if let ExprKind::ConstructVariant { case_name, .. } = &expression.kind {
                cost.string(case_name)?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                ast_expr_cost(&field.value, cost)?;
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            ast_expr_cost(scrutinee, cost)?;
            for arm in arms {
                cost.value(arm)?;
                ast_pattern_cost(&arm.pattern, cost)?;
                ast_expr_cost(&arm.value, cost)?;
            }
        }
        ExprKind::Try { operand } => ast_expr_cost(operand, cost)?,
        ExprKind::UpdateRecord { base, fields } => {
            ast_expr_cost(base, cost)?;
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                ast_expr_cost(&field.value, cost)?;
            }
        }
        ExprKind::Project { base, field, .. } => {
            ast_expr_cost(base, cost)?;
            cost.string(field)?;
        }
        ExprKind::Int(_) | ExprKind::Bool(_) => {}
    }
    Ok(())
}

fn ast_pattern_cost(
    pattern: &crate::ast::MatchPattern,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(pattern)?;
    match pattern {
        crate::ast::MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } => {
            cost.string(type_name)?;
            cost.string(case_name)?;
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                cost.string(&field.binding)?;
            }
        }
        crate::ast::MatchPattern::Record {
            type_name, fields, ..
        } => {
            cost.string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::MatchPattern::Wildcard { .. } => {}
    }
    Ok(())
}

fn ast_record_pattern_field_cost(
    field: &crate::ast::RecordMatchPatternField,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(field)?;
    cost.string(&field.name)?;
    cost.value(&field.pattern)?;
    match &field.pattern {
        crate::ast::RecordMatchFieldPattern::Binding { name, .. } => cost.string(name)?,
        crate::ast::RecordMatchFieldPattern::Record {
            type_name, fields, ..
        } => {
            cost.string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn default_expr_expanded_cost(
    ty: &Type,
    module: &str,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    memo: &mut BTreeMap<String, ExpandedDefaultCost>,
    visiting: &mut BTreeSet<String>,
) -> Result<ExpandedDefaultCost, Vec<Diagnostic>> {
    match ty {
        Type::I64 | Type::Bool => Ok(ExpandedDefaultCost {
            bytes: std::mem::size_of::<Expr>(),
            identity_slots: 0,
        }),
        Type::Named { name, arguments } if arguments.is_empty() => {
            let target_id = resolve_type_id(module, name, programs).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "default-expression type identity cost lookup disagrees",
                )]
            })?;
            if let Some(cost) = memo.get(&target_id) {
                return Ok(*cost);
            }
            if !visiting.insert(crate::bounded_output::budgeted_clone(&target_id)) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "default-expression type cost contains a recursive cycle",
                )]);
            }
            let target = authored.get(target_id.as_str()).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "default-expression type authority is absent",
                )]
            })?;
            let declaration = target.ty.ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "default-expression type authority has the wrong kind",
                )]
            })?;
            let alias = caller
                .module_uses
                .iter()
                .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
                .map(|item| item.alias.as_str())
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "default-expression type lacks direct caller alias authority",
                    )]
                })?;
            let mut cost = StructuralCost(std::mem::size_of::<Expr>());
            let mut identity_slots = 1usize;
            cost.string(alias)?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        cost.add(std::mem::size_of::<FieldInitializer>())?;
                        cost.string(&field.name)?;
                        let nested = default_expr_expanded_cost(
                            &field.ty,
                            target.module,
                            caller,
                            authored,
                            programs,
                            memo,
                            visiting,
                        )?;
                        cost.add(nested.bytes)?;
                        identity_slots = checked_builder_sum(
                            identity_slots,
                            nested.identity_slots.checked_add(1).ok_or_else(|| {
                                vec![limit_error("builder_bytes", active_builder_limit())]
                            })?,
                        )?;
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    let case = cases.first().ok_or_else(|| {
                        vec![graph_error("SPX-G172", "imported Copy variant has no case")]
                    })?;
                    cost.string(&case.name)?;
                    identity_slots = checked_builder_sum(identity_slots, 1)?;
                    for field in &case.fields {
                        cost.add(std::mem::size_of::<FieldInitializer>())?;
                        cost.string(&field.name)?;
                        let nested = default_expr_expanded_cost(
                            &field.ty,
                            target.module,
                            caller,
                            authored,
                            programs,
                            memo,
                            visiting,
                        )?;
                        cost.add(nested.bytes)?;
                        identity_slots = checked_builder_sum(
                            identity_slots,
                            nested.identity_slots.checked_add(1).ok_or_else(|| {
                                vec![limit_error("builder_bytes", active_builder_limit())]
                            })?,
                        )?;
                    }
                }
                TypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "resource return is not admitted",
                    )]);
                }
            }
            visiting.remove(&target_id);
            let expanded = ExpandedDefaultCost {
                bytes: cost.0,
                identity_slots,
            };
            memo.insert(target_id, expanded);
            Ok(expanded)
        }
        Type::Named { .. } => Err(vec![graph_error(
            "SPX-G172",
            "generic return is not admitted",
        )]),
    }
}

fn validate_dependency_dag(programs: &[Program]) -> Result<BTreeMap<&str, usize>, Vec<Diagnostic>> {
    let dependencies = programs
        .iter()
        .map(|program| {
            (
                program.module.as_str(),
                program
                    .module_uses
                    .iter()
                    .map(|item| item.target_module.as_str())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let depths = dependency_depths(&dependencies)?;
    if depths.values().any(|depth| *depth > MAX_DEPENDENCY_DEPTH) {
        return Err(vec![limit_error("dependency_depth", MAX_DEPENDENCY_DEPTH)]);
    }
    Ok(depths)
}

fn dependency_depths<'a>(
    dependencies: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> Result<BTreeMap<&'a str, usize>, Vec<Diagnostic>> {
    fn visit<'a>(
        module: &'a str,
        dependencies: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        stack: &mut Vec<&'a str>,
        depths: &mut BTreeMap<&'a str, usize>,
    ) -> Result<usize, Vec<Diagnostic>> {
        if let Some(index) = stack.iter().position(|item| *item == module) {
            let mut cycle = stack[index..].to_vec();
            cycle.push(module);
            let witness = canonical_cycle(&cycle);
            let witness = crate::bounded_output::budgeted_join(
                witness
                    .into_iter()
                    .map(crate::bounded_output::budgeted_clone),
                " -> ",
            );
            return Err(vec![graph_error(
                "SPX-G172",
                crate::bounded_output::budgeted_format(format_args!(
                    "workspace module dependency cycle: {witness}"
                )),
            )]);
        }
        if let Some(depth) = depths.get(module) {
            return Ok(*depth);
        }
        stack.push(module);
        let mut depth = 1usize;
        for dependency in dependencies.get(module).into_iter().flatten() {
            depth = depth.max(
                1usize
                    .checked_add(visit(dependency, dependencies, stack, depths)?)
                    .ok_or_else(|| vec![limit_error("dependency_depth", MAX_DEPENDENCY_DEPTH)])?,
            );
        }
        stack.pop();
        depths.insert(module, depth);
        Ok(depth)
    }
    let mut depths = BTreeMap::new();
    for module in dependencies.keys() {
        visit(module, dependencies, &mut Vec::new(), &mut depths)?;
    }
    Ok(depths)
}

fn canonical_cycle<'a>(cycle: &[&'a str]) -> Vec<&'a str> {
    let body = &cycle[..cycle.len().saturating_sub(1)];
    let start = body
        .iter()
        .enumerate()
        .min_by_key(|(_, module)| *module)
        .map_or(0, |(index, _)| index);
    let mut result = body[start..]
        .iter()
        .chain(&body[..start])
        .copied()
        .collect::<Vec<_>>();
    if let Some(first) = result.first().copied() {
        result.push(first);
    }
    result
}

fn synthetic_program(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<Program, Vec<Diagnostic>> {
    let mut synthetic = program.clone();
    synthetic.module_uses.clear();
    for module_use in program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type)
    {
        let target = &authored[module_use.persistent_id.as_str()];
        let mut ty = target.ty.expect("validated type target").clone();
        ty.name = crate::bounded_output::budgeted_clone(&module_use.alias);
        rewrite_type_declaration(&mut ty, target.module, program, programs)?;
        synthetic.types.push(ty);
    }
    let type_index_bytes = synthetic
        .types
        .len()
        .checked_mul(std::mem::size_of::<(&str, &TypeDeclaration)>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(type_index_bytes)?;
    let mut type_declarations = Vec::with_capacity(synthetic.types.len());
    for declaration in &synthetic.types {
        type_declarations.push((declaration.name.as_str(), declaration));
    }
    type_declarations.sort_by(|left, right| left.0.cmp(right.0));
    if type_declarations
        .windows(2)
        .any(|items| items[0].0 == items[1].0)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    for module_use in program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
    {
        let target = &authored[module_use.persistent_id.as_str()];
        let target_function = target.function.expect("validated function target");
        let mut function = target_function.clone();
        function.name = crate::bounded_output::budgeted_clone(&module_use.alias);
        for param in &mut function.params {
            rewrite_type(&mut param.ty, target.module, program, programs)?;
        }
        rewrite_type(&mut function.return_type, target.module, program, programs)?;
        function.requires.clear();
        function.ensures.clear();
        function.body = default_expr(&function.return_type, &type_declarations)?;
        synthetic.functions.push(function);
    }
    if !synthetic
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        reserve_builder_structure(std::mem::size_of::<Function>())?;
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        synthetic.functions.push(Function {
            stable_id: crate::bounded_output::budgeted_format(format_args!(
                "workspace.synthetic.main.{}",
                synthetic.module
            )),
            explicit_id: true,
            name: crate::bounded_output::budgeted_clone("main"),
            name_span: Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: Span::default(),
            },
            span: Span::default(),
        });
    }
    Ok(synthetic)
}

fn rewrite_type_declaration(
    declaration: &mut TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    match &mut declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                rewrite_type(&mut field.ty, target_module, caller, programs)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &mut case.fields {
                    rewrite_type(&mut field.ty, target_module, caller, programs)?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}

fn rewrite_type(
    ty: &mut Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(vec![graph_error(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )]);
    }
    let target_id = resolve_type_id(target_module, name, programs).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "cross-file type identity lookup disagrees",
        )]
    })?;
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G172",
                crate::bounded_output::budgeted_format(format_args!(
                    "cross-file signature type `{target_id}` is not explicitly imported"
                )),
            )]
        })?;
    *name = crate::bounded_output::budgeted_clone(alias);
    Ok(())
}

fn default_expr(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
) -> Result<Expr, Vec<Diagnostic>> {
    reserve_builder_structure(std::mem::size_of::<Expr>())?;
    let span = Span::default();
    let kind = match ty {
        Type::I64 => ExprKind::Int(0),
        Type::Bool => ExprKind::Bool(false),
        Type::Named { name, arguments } if arguments.is_empty() => {
            let declaration = declarations
                .binary_search_by_key(&name.as_str(), |(name, _)| *name)
                .map(|index| declarations[index].1)
                .map_err(|_| {
                    vec![graph_error(
                        "SPX-G173",
                        "default imported type lookup disagrees",
                    )]
                })?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields } => ExprKind::ConstructRecord {
                    type_name: crate::bounded_output::budgeted_clone(name),
                    type_span: span,
                    type_arguments: Vec::new(),
                    fields: fields
                        .iter()
                        .map(|field| {
                            reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
                            Ok(FieldInitializer {
                                name: crate::bounded_output::budgeted_clone(&field.name),
                                name_span: span,
                                value: default_expr(&field.ty, declarations)?,
                                span,
                            })
                        })
                        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                },
                TypeDeclarationKind::Variant { cases } => {
                    let case = cases.first().ok_or_else(|| {
                        vec![graph_error("SPX-G172", "imported Copy variant has no case")]
                    })?;
                    ExprKind::ConstructVariant {
                        type_name: crate::bounded_output::budgeted_clone(name),
                        type_span: span,
                        type_arguments: Vec::new(),
                        case_name: crate::bounded_output::budgeted_clone(&case.name),
                        case_span: span,
                        fields: case
                            .fields
                            .iter()
                            .map(|field| {
                                reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
                                Ok(FieldInitializer {
                                    name: crate::bounded_output::budgeted_clone(&field.name),
                                    name_span: span,
                                    value: default_expr(&field.ty, declarations)?,
                                    span,
                                })
                            })
                            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                    }
                }
                TypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "resource return is not admitted",
                    )])
                }
            }
        }
        Type::Named { .. } => {
            return Err(vec![graph_error(
                "SPX-G172",
                "generic return is not admitted",
            )])
        }
    };
    Ok(Expr { kind, span })
}

fn reserve_builder_structure(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(vec![limit_error("builder_bytes", active_builder_limit())])
    }
}

fn collect_expected_edges(
    program: &Program,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let function_uses = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| (item.alias.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let type_uses = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type)
        .map(|item| (item.alias.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for (index, permit) in program.permits.iter().enumerate() {
        let path = crate::bounded_output::budgeted_format(format_args!("permit.{index}"));
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(&program.module),
                target_path: crate::bounded_output::budgeted_clone(&program.path),
                target: crate::bounded_output::budgeted_clone(permit),
                kind: "capability_authority",
                site: "module",
                expression: crate::bounded_output::budgeted_clone(&path),
                ast_path: path,
                alias: String::new(),
                ordinal: index,
            },
        )?;
    }
    for declaration in &program.types {
        let declaration_type_uses = ScopedTypeUses {
            uses: &type_uses,
            shadowed: &declaration.type_parameters,
        };
        match &declaration.kind {
            TypeDeclarationKind::Resource { .. } => {}
            TypeDeclarationKind::Record { fields } => {
                for (index, field) in fields.iter().enumerate() {
                    collect_type_reference_edge(
                        program,
                        &declaration.stable_id,
                        &field.ty,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.stable_id
                        )),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for (case_index, case) in cases.iter().enumerate() {
                    for (field_index, field) in case.fields.iter().enumerate() {
                        collect_type_reference_edge(
                            program,
                            &declaration.stable_id,
                            &field.ty,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "type.{}.case.{case_index}.field.{field_index}",
                                declaration.stable_id
                            )),
                            declaration_type_uses,
                            module_paths,
                            authored,
                            edges,
                        )?;
                    }
                }
            }
        }
    }
    for function in &program.functions {
        let function_type_uses = ScopedTypeUses {
            uses: &type_uses,
            shadowed: &function.type_parameters,
        };
        for (index, param) in function.params.iter().enumerate() {
            collect_type_reference_edge(
                program,
                &function.stable_id,
                &param.ty,
                &crate::bounded_output::budgeted_format(format_args!(
                    "function.{}.param.{index}",
                    function.stable_id
                )),
                function_type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        collect_type_reference_edge(
            program,
            &function.stable_id,
            &function.return_type,
            &crate::bounded_output::budgeted_format(format_args!(
                "function.{}.return",
                function.stable_id
            )),
            function_type_uses,
            module_paths,
            authored,
            edges,
        )?;
        for (site, expressions) in [
            ("requires", function.requires.as_slice()),
            ("body", std::slice::from_ref(&function.body)),
            ("ensures", function.ensures.as_slice()),
        ] {
            for (root_index, expression) in expressions.iter().enumerate() {
                let root = match site {
                    "requires" => crate::bounded_output::budgeted_format(format_args!(
                        "requires.{root_index}"
                    )),
                    "body" => crate::bounded_output::budgeted_clone("body"),
                    "ensures" => {
                        crate::bounded_output::budgeted_format(format_args!("ensures.{root_index}"))
                    }
                    _ => unreachable!(),
                };
                let mut call_ordinal = 0usize;
                visit_ast_call_sites(expression, &root, &mut |name, path| {
                    let ordinal = call_ordinal;
                    call_ordinal += 1;
                    if let Some(module_use) = function_uses.get(name) {
                        let target = &authored[module_use.persistent_id.as_str()];
                        let edge = WorkspaceEdge {
                            caller_path: crate::bounded_output::budgeted_clone(&program.path),
                            caller: crate::bounded_output::budgeted_clone(&function.stable_id),
                            target_path: crate::bounded_output::budgeted_clone(
                                module_paths[target.module],
                            ),
                            target: crate::bounded_output::budgeted_clone(
                                &module_use.persistent_id,
                            ),
                            kind: "call",
                            site,
                            expression: hir::workspace_expression_identity(
                                &hir::DeclarationId::new(crate::bounded_output::budgeted_clone(
                                    &function.stable_id,
                                )),
                                path,
                            ),
                            ast_path: crate::bounded_output::budgeted_clone(path),
                            alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                            ordinal,
                        };
                        push_edge(edges, edge)?;
                        if let Some(target_function) = target.function {
                            for effect in &target_function.effects {
                                push_edge(
                                    edges,
                                    WorkspaceEdge {
                                        caller_path: crate::bounded_output::budgeted_clone(
                                            &program.path,
                                        ),
                                        caller: crate::bounded_output::budgeted_clone(
                                            &function.stable_id,
                                        ),
                                        target_path: crate::bounded_output::budgeted_clone(
                                            target.path,
                                        ),
                                        target: crate::bounded_output::budgeted_clone(effect),
                                        kind: "effect_requirement",
                                        site,
                                        expression: hir::workspace_expression_identity(
                                            &hir::DeclarationId::new(
                                                crate::bounded_output::budgeted_clone(
                                                    &function.stable_id,
                                                ),
                                            ),
                                            path,
                                        ),
                                        ast_path: crate::bounded_output::budgeted_clone(path),
                                        alias: crate::bounded_output::budgeted_clone(
                                            &module_use.alias,
                                        ),
                                        ordinal,
                                    },
                                )?;
                            }
                        }
                    }
                    Ok(())
                })?;
                collect_expression_type_edges(
                    program,
                    &function.stable_id,
                    expression,
                    &root,
                    function_type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
    }
    for (ordinal, module_use) in program.module_uses.iter().enumerate() {
        let target = &authored[module_use.persistent_id.as_str()];
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(&program.module),
                target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
                target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                kind: match module_use.kind {
                    ModuleUseKind::Function => "function_import",
                    ModuleUseKind::Type => "type_import",
                },
                site: "module",
                expression: crate::bounded_output::budgeted_format(format_args!("use.{ordinal}")),
                ast_path: crate::bounded_output::budgeted_format(format_args!("use.{ordinal}")),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                ordinal,
            },
        )?;
    }
    edges.sort();
    Ok(())
}

#[derive(Clone, Copy)]
struct ScopedTypeUses<'a> {
    uses: &'a BTreeMap<&'a str, &'a ModuleUse>,
    shadowed: &'a [crate::ast::TypeParameterDeclaration],
}

impl<'a> ScopedTypeUses<'a> {
    fn get(self, name: &str) -> Option<&'a ModuleUse> {
        if self.shadowed.iter().any(|parameter| parameter.name == name) {
            None
        } else {
            self.uses.get(name).copied()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_type_reference_edge(
    program: &Program,
    owner: &str,
    ty: &Type,
    path: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    collect_type_reference_edge_at(
        program,
        owner,
        ty,
        path,
        None,
        type_uses,
        module_paths,
        authored,
        edges,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_type_reference_edge_at(
    program: &Program,
    owner: &str,
    ty: &Type,
    path: &str,
    expression: Option<&str>,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    collect_named_type_reference_edge_at(
        program,
        owner,
        name,
        arguments,
        path,
        expression,
        type_uses,
        module_paths,
        authored,
        edges,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_named_type_reference_edge_at(
    program: &Program,
    owner: &str,
    name: &str,
    arguments: &[Type],
    path: &str,
    expression: Option<&str>,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(module_use) = type_uses.get(name) {
        let target = &authored[module_use.persistent_id.as_str()];
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(owner),
                target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
                target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                kind: "type_reference",
                site: "type",
                expression: crate::bounded_output::budgeted_clone(expression.unwrap_or(path)),
                ast_path: crate::bounded_output::budgeted_clone(path),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                ordinal: edges.len(),
            },
        )?;
    }
    for (index, argument) in arguments.iter().enumerate() {
        let argument_path =
            crate::bounded_output::budgeted_format(format_args!("{path}.argument.{index}"));
        collect_type_reference_edge_at(
            program,
            owner,
            argument,
            &argument_path,
            expression,
            type_uses,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_expression_type_edges(
    program: &Program,
    owner: &str,
    expression: &Expr,
    path: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let expression_id = hir::workspace_expression_identity(
        &hir::DeclarationId::new(crate::bounded_output::budgeted_clone(owner)),
        path,
    );
    match &expression.kind {
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                let type_path = crate::bounded_output::budgeted_format(format_args!(
                    "{path}.type_argument.{index}"
                ));
                collect_type_reference_edge_at(
                    program,
                    owner,
                    argument,
                    &type_path,
                    Some(&expression_id),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            for (index, argument) in args.iter().enumerate() {
                let child =
                    crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Unary { value, .. } => collect_expression_type_edges(
            program,
            owner,
            value,
            &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::Binary { left, right, .. } => {
            collect_expression_type_edges(
                program,
                owner,
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            collect_expression_type_edges(
                program,
                owner,
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                let crate::ast::Statement::Let { value, .. } = statement;
                collect_expression_type_edges(
                    program,
                    owner,
                    value,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.s{index}.value")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            collect_expression_type_edges(
                program,
                owner,
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for (suffix, child) in [
                ("condition", condition.as_ref()),
                ("then", then_branch.as_ref()),
                ("else", else_branch.as_ref()),
            ] {
                collect_expression_type_edges(
                    program,
                    owner,
                    child,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.{suffix}")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            collect_named_type_reference_edge_at(
                program,
                owner,
                type_name,
                type_arguments,
                &crate::bounded_output::budgeted_format(format_args!("{path}.type")),
                Some(&expression_id),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_expression_type_edges(
                    program,
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_expression_type_edges(
                program,
                owner,
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                let pattern_path = crate::bounded_output::budgeted_format(format_args!(
                    "{path}.arm.{index}.pattern"
                ));
                collect_match_pattern_type_edges(
                    program,
                    owner,
                    &arm.pattern,
                    &pattern_path,
                    &expression_id,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
                collect_expression_type_edges(
                    program,
                    owner,
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Try { operand } => collect_expression_type_edges(
            program,
            owner,
            operand,
            &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::UpdateRecord { base, fields } => {
            collect_expression_type_edges(
                program,
                owner,
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_expression_type_edges(
                    program,
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Project { base, .. } => collect_expression_type_edges(
            program,
            owner,
            base,
            &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_match_pattern_type_edges(
    program: &Program,
    owner: &str,
    pattern: &crate::ast::MatchPattern,
    path: &str,
    expression: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    match pattern {
        crate::ast::MatchPattern::Variant { type_name, .. }
        | crate::ast::MatchPattern::Record { type_name, .. } => {
            collect_named_type_reference_edge_at(
                program,
                owner,
                type_name,
                &[],
                path,
                Some(expression),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        crate::ast::MatchPattern::Wildcard { .. } => {}
    }
    if let crate::ast::MatchPattern::Record { fields, .. } = pattern {
        for (index, field) in fields.iter().enumerate() {
            collect_record_pattern_type_edges(
                program,
                owner,
                &field.pattern,
                &crate::bounded_output::budgeted_format(format_args!(
                    "{path}.field.{index}.pattern"
                )),
                expression,
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_record_pattern_type_edges(
    program: &Program,
    owner: &str,
    pattern: &crate::ast::RecordMatchFieldPattern,
    path: &str,
    expression: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let crate::ast::RecordMatchFieldPattern::Record {
        type_name, fields, ..
    } = pattern
    else {
        return Ok(());
    };
    collect_named_type_reference_edge_at(
        program,
        owner,
        type_name,
        &[],
        path,
        Some(expression),
        type_uses,
        module_paths,
        authored,
        edges,
    )?;
    for (index, field) in fields.iter().enumerate() {
        collect_record_pattern_type_edges(
            program,
            owner,
            &field.pattern,
            &crate::bounded_output::budgeted_format(format_args!("{path}.field.{index}.pattern")),
            expression,
            type_uses,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
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
                    crate::ast::Statement::Let { value, .. } => visit_ast_call_sites(
                        value,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
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
        ExprKind::Match { scrutinee, arms } => {
            visit_ast_call_sites(
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                visit,
            )?;
            for (index, arm) in arms.iter().enumerate() {
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
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {}
    }
    Ok(())
}

fn verify_resolved_call_edges(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let aliases = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| (item.alias.as_str(), item.persistent_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut expected = Vec::new();
    for function in &program.functions {
        let owner =
            hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&function.stable_id));
        for (root, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                (
                    crate::bounded_output::budgeted_format(format_args!("requires.{index}")),
                    expression,
                )
            })
            .chain(std::iter::once((
                crate::bounded_output::budgeted_clone("body"),
                &function.body,
            )))
            .chain(
                function
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| {
                        (
                            crate::bounded_output::budgeted_format(format_args!("ensures.{index}")),
                            expression,
                        )
                    }),
            )
        {
            visit_ast_call_sites(expression, &root, &mut |name, path| {
                if let Some(target) = aliases.get(name) {
                    reserve_builder_structure(std::mem::size_of::<(
                        hir::DeclarationId,
                        hir::ExpressionId,
                        hir::DeclarationId,
                    )>())?;
                    expected.push((
                        hir::DeclarationId::new(crate::bounded_output::budgeted_clone(
                            owner.as_str(),
                        )),
                        hir::workspace_expression_identity(&owner, path),
                        hir::DeclarationId::new(crate::bounded_output::budgeted_clone(target)),
                    ));
                }
                Ok(())
            })?;
        }
    }
    let target_ids = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| {
            hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&item.persistent_id))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = hir::workspace_call_sites(resolved);
    actual.retain(|(_, _, target)| target_ids.contains(target));
    expected.sort();
    actual.sort();
    if expected != actual
        || expected
            .iter()
            .any(|(_, _, target)| !authored.contains_key(target.as_str()))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "independent workspace call-edge reconstruction disagrees with HIR",
        )]);
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
                TypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
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
                TypeDeclarationKind::Record { fields } => {
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
            TypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
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
            TypeDeclarationKind::Record { fields } => {
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
                hir::ResolvedTypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
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
                hir::ResolvedTypeDeclarationKind::Record { fields } => {
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
mod tests {
    use super::*;

    fn source(path: &str, source: &str) -> WorkspaceSource {
        WorkspaceSource {
            path: path.to_owned(),
            source: source.to_owned(),
        }
    }

    fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
        let program = parse(source, Path::new(path)).expect("test source must parse");
        WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&program),
        }
    }

    #[test]
    fn scalar_linker_uses_real_provider_bodies_for_two_closures() {
        let provider = canonical_source(
            "lib/math.spx",
            r#"
module lib.math;

@id("lib.answer")
fn answer() -> i64 { 41 }
"#,
        );
        let app = canonical_source(
            "app/main.spx",
            r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.main")
fn main() -> i64 { answer() + 1 }
"#,
        );
        let test = canonical_source(
            "test/main.spx",
            r#"
module test.main;
use function @id("lib.answer") from lib.math as answer;

@id("test.main")
fn main() -> i64 { answer() + 2 }
"#,
        );

        let (entry, test) = build_owned(vec![provider, app, test])
            .unwrap()
            .into_linked_scalar_programs("app.main", "test.main")
            .unwrap();
        let answer = hir::DeclarationId::new("lib.answer");
        let entry_answer = entry
            .functions
            .iter()
            .find(|function| function.id == answer)
            .unwrap();
        let test_answer = test
            .functions
            .iter()
            .find(|function| function.id == answer)
            .unwrap();
        let hir::ResolvedExprKind::Block { tail, .. } = &entry_answer.body.kind else {
            panic!("resolved provider body must retain its block");
        };
        assert!(matches!(tail.kind, hir::ResolvedExprKind::Int(41)));
        assert_eq!(entry_answer.body, test_answer.body);
        assert!(entry.functions.iter().all(|function| !function
            .id
            .as_str()
            .starts_with("workspace.synthetic.main.")));
        assert_eq!(entry.entrypoint.as_str(), "app.main");
        assert_eq!(test.entrypoint.as_str(), "test.main");
    }

    #[test]
    fn scalar_linker_is_identity_based_when_provider_display_names_match() {
        let left = canonical_source(
            "lib/left.spx",
            r#"
module lib.left;

@id("lib.left.value")
fn value() -> i64 { 20 }
"#,
        );
        let right = canonical_source(
            "lib/right.spx",
            r#"
module lib.right;

@id("lib.right.value")
fn value() -> i64 { 22 }
"#,
        );
        let app = canonical_source(
            "app/main.spx",
            r#"
module app.main;
use function @id("lib.left.value") from lib.left as left_value;
use function @id("lib.right.value") from lib.right as right_value;

@id("app.main")
fn main() -> i64 { left_value() + right_value() }
"#,
        );

        let linked = build_owned(vec![left, right, app])
            .unwrap()
            .linked_scalar_program("app.main")
            .unwrap();
        let left = hir::DeclarationId::new("lib.left.value");
        let right = hir::DeclarationId::new("lib.right.value");
        assert_eq!(
            linked.declarations.declaration(&left).unwrap().name,
            "value"
        );
        assert_eq!(
            linked.declarations.declaration(&right).unwrap().name,
            "value"
        );
        assert_eq!(
            linked
                .functions
                .iter()
                .filter(|function| function.name == "value")
                .count(),
            2
        );
        hir::validate(&linked).unwrap();
    }

    #[test]
    fn scalar_linker_rejects_disconnected_nonscalar_modules_and_provider_mains() {
        let app = canonical_source(
            "app/main.spx",
            r#"
module app.main;

@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let test = canonical_source(
            "test/main.spx",
            r#"
module test.main;

@id("test.main")
fn main() -> i64 { 0 }
"#,
        );
        let disconnected = canonical_source(
            "other/record.spx",
            r#"
module other.record;

@id("other.record")
record Record { @id("other.record.value") value: i64, }

@id("other.value")
fn value() -> i64 { 0 }
"#,
        );
        let error = build_owned(vec![app.clone(), test, disconnected])
            .unwrap()
            .into_linked_scalar_programs("app.main", "test.main")
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G172");

        let provider = canonical_source(
            "lib/provider.spx",
            r#"
module lib.provider;

@id("lib.value")
fn value() -> i64 { 1 }

@id("lib.main")
fn main() -> i64 { value() }
"#,
        );
        let consumer = canonical_source(
            "app/main.spx",
            r#"
module app.main;
use function @id("lib.value") from lib.provider as value;

@id("app.main")
fn main() -> i64 { value() }
"#,
        );
        let error = build_owned(vec![provider, consumer])
            .unwrap()
            .linked_scalar_program("app.main")
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G172");
    }

    fn effect_edge_sources() -> Vec<WorkspaceSource> {
        let library = r#"
module lib.core;
permit { audit.write, network.read }

@id("lib.zero")
fn zero() -> i64 { 0 }

@id("lib.multi")
fn multi() -> i64 uses { audit.write, network.read } { 42 }
"#;
        let app = r#"
module app.main;
use function @id("lib.zero") from lib.core as zero;
use function @id("lib.multi") from lib.core as multi;
permit { audit.write, network.read }

@id("app.main")
fn main() -> i64 uses { audit.write, network.read } {
    zero() + multi()
}

@id("app.other")
fn other() -> i64 uses { audit.write, network.read } { 0 }
"#;
        vec![
            canonical_source("app/main.spx", app),
            canonical_source("lib/core.spx", library),
        ]
    }

    fn effect_edge_fixture() -> WorkspaceGraphBuild {
        build_owned(effect_edge_sources()).expect("effect-edge fixture must build")
    }

    fn parsed_sources(sources: &[WorkspaceSource]) -> Vec<Program> {
        sources
            .iter()
            .map(|source| {
                parse(&source.source, Path::new(&source.path))
                    .expect("canonical workspace fixture must parse")
            })
            .collect()
    }

    fn identity_fact_sources() -> Vec<WorkspaceSource> {
        let library = r#"
module lib.core;

@id("lib.imported")
record Imported {
    @id("lib.imported.value")
    value: i64,
}

@id("lib.foreign")
record Foreign {
    @id("lib.foreign.value")
    value: i64,
}

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
        let app = r#"
module app.identity;
use function @id("lib.answer") from lib.core as answer;
use type @id("lib.imported") from lib.core as Imported;

@id("app.record")
record Record {
    value: i64,
}

@id("app.choice")
variant Choice {
    Number { value: i64, },
}

@id("app.explicit_variant")
variant ExplicitVariant {
    @id("app.explicit_variant.ready")
    Ready {
        @id("app.explicit_variant.ready.code")
        code: i64,
    },
}

@id("app.token")
resource Token {
    @id("app.token.drop")
    drop trivial;
}

@id("app.host")
interface Host permits {} {
    @id("app.host.observe")
    import fn observe(value: own Token) -> unit
        effects {}
        failure infallible
        consumes value always;
}

fn helper() -> i64 { answer() }

@id("app.main")
fn main() -> i64 { helper() }
"#;
        vec![
            canonical_source("app/identity.spx", app),
            canonical_source("lib/core.spx", library),
        ]
    }

    fn assert_identity_shape_error(
        mut mutate: impl FnMut(&mut BTreeMap<String, WorkspaceDeclarationFact>),
    ) {
        let sources = identity_fact_sources();
        let programs = parsed_sources(&sources);
        let build = build_owned(sources).expect("identity fixture must build");
        let mut facts = expected_declaration_facts(&programs).unwrap();
        mutate(&mut facts);
        let error = validate_retained_declaration_shapes(&build.hir.modules, &facts)
            .expect_err("mutated identity fact must fail closed");
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "retained workspace declaration shape disagrees with authored identity facts"
        );
    }

    fn assert_resolved_identity_error(
        mut mutate: impl FnMut(&str, &mut Program, &[Program]),
        expected: &str,
    ) {
        let sources = identity_fact_sources();
        let programs = parsed_sources(&sources);
        let build = build_owned(sources).expect("identity fixture must build");
        let resolved_modules = {
            let authored = index_authored(&programs).unwrap();
            programs
                .iter()
                .map(|program| {
                    let mut synthetic = synthetic_program(program, &authored, &programs).unwrap();
                    mutate(&program.module, &mut synthetic, &programs);
                    (
                        program.module.clone(),
                        hir::resolve(&synthetic).expect("mutated identity HIR must resolve"),
                    )
                })
                .collect::<Vec<_>>()
        };
        let error = workspace_declaration_facts(&resolved_modules, &build.hir.modules, &programs)
            .expect_err("mutated resolved identity must fail closed");
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(error[0].message, expected);
    }

    fn assert_effect_validation_error(
        mut mutate: impl FnMut(&mut Vec<WorkspaceEdge>),
        expected: &str,
    ) {
        let mut build = effect_edge_fixture();
        mutate(&mut build.edges);
        let error = validate_effect_and_capability_edges(&build.hir.modules, &build.edges)
            .expect_err("mutated proof must fail closed");
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(error[0].message, expected);
    }

    fn assert_call_validation_error(mut mutate: impl FnMut(&mut Vec<WorkspaceEdge>)) {
        let sources = effect_edge_sources();
        let programs = parsed_sources(&sources);
        let mut build = build_owned(sources).expect("call-edge fixture must build");
        mutate(&mut build.edges);
        let error = validate_retained_facts(&programs, &build.hir.modules, &build.edges)
            .expect_err("mutated call proof must fail closed");
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "emitted workspace call edges disagree with authenticated AST/HIR occurrences"
        );
    }

    fn mutate_multi_call_family(
        edges: &mut [WorkspaceEdge],
        mut mutate: impl FnMut(&mut WorkspaceEdge),
    ) {
        let call = edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
            .expect("multi-effect call must exist")
            .clone();
        let family = edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                matches!(edge.kind, "call" | "effect_requirement")
                    && CallOccurrenceKey::from_edge(edge) == CallOccurrenceKey::from_edge(&call)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(family.len(), 3, "one call must own exactly two effects");
        for index in family {
            mutate(&mut edges[index]);
        }
    }

    #[test]
    fn zero_and_multi_effect_targets_replay_exact_occurrences() {
        let build = effect_edge_fixture();
        validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();

        let zero_call = build
            .edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.target == "lib.zero")
            .expect("zero-effect call must exist");
        assert!(!build.edges.iter().any(|edge| {
            edge.kind == "effect_requirement"
                && CallOccurrenceKey::from_edge(edge) == CallOccurrenceKey::from_edge(zero_call)
        }));

        let multi_call = build
            .edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
            .expect("multi-effect call must exist");
        let effects = build
            .edges
            .iter()
            .filter(|edge| {
                edge.kind == "effect_requirement"
                    && CallOccurrenceKey::from_edge(edge)
                        == CallOccurrenceKey::from_edge(multi_call)
            })
            .map(|edge| edge.target.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(effects, BTreeSet::from(["audit.write", "network.read"]));
    }

    #[test]
    fn altered_effect_requirement_sets_fail_closed() {
        const SET_MISMATCH: &str =
            "workspace call effect requirements disagree with retained target HIR";

        assert_effect_validation_error(
            |edges| {
                let index = edges
                    .iter()
                    .position(|edge| {
                        edge.kind == "effect_requirement" && edge.target == "audit.write"
                    })
                    .unwrap();
                edges.remove(index);
            },
            SET_MISMATCH,
        );
        assert_effect_validation_error(
            |edges| {
                let mut extra = edges
                    .iter()
                    .find(|edge| edge.kind == "effect_requirement")
                    .unwrap()
                    .clone();
                extra.target = "storage.read".to_owned();
                edges.push(extra);
            },
            SET_MISMATCH,
        );
        assert_effect_validation_error(
            |edges| {
                edges
                    .iter_mut()
                    .find(|edge| edge.kind == "effect_requirement" && edge.target == "audit.write")
                    .unwrap()
                    .target = "storage.read".to_owned();
            },
            SET_MISMATCH,
        );
        assert_effect_validation_error(
            |edges| {
                let duplicate = edges
                    .iter()
                    .find(|edge| edge.kind == "effect_requirement")
                    .unwrap()
                    .clone();
                edges.push(duplicate);
            },
            "workspace call effect requirement is duplicated",
        );
    }

    #[test]
    fn coupled_call_effect_key_substitutions_fail_exact_reconstruction() {
        assert_call_validation_error(|edges| {
            mutate_multi_call_family(edges, |edge| edge.site = "requires");
        });
        assert_call_validation_error(|edges| {
            let expression = hir::workspace_expression_identity(
                &hir::DeclarationId::new("app.main".to_owned()),
                "body.tail.left",
            );
            mutate_multi_call_family(edges, |edge| edge.expression = expression.clone());
        });
        assert_call_validation_error(|edges| {
            mutate_multi_call_family(edges, |edge| {
                edge.ast_path = "body.tail.left".to_owned();
            });
        });
        assert_call_validation_error(|edges| {
            mutate_multi_call_family(edges, |edge| edge.alias = "zero".to_owned());
        });
        assert_call_validation_error(|edges| {
            mutate_multi_call_family(edges, |edge| edge.ordinal = 0);
        });
        assert_call_validation_error(|edges| {
            mutate_multi_call_family(edges, |edge| edge.caller = "app.other".to_owned());
        });
    }

    #[test]
    fn missing_extra_and_duplicate_call_edges_fail_exact_reconstruction() {
        assert_call_validation_error(|edges| {
            let index = edges
                .iter()
                .position(|edge| edge.kind == "call" && edge.target == "lib.multi")
                .unwrap();
            edges.remove(index);
        });
        assert_call_validation_error(|edges| {
            let mut extra = edges
                .iter()
                .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
                .unwrap()
                .clone();
            extra.expression = hir::workspace_expression_identity(
                &hir::DeclarationId::new("app.main".to_owned()),
                "body.extra",
            );
            extra.ast_path = "body.extra".to_owned();
            extra.ordinal = 2;
            edges.push(extra);
        });
        assert_call_validation_error(|edges| {
            let duplicate = edges
                .iter()
                .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
                .unwrap()
                .clone();
            edges.push(duplicate);
        });
    }

    #[test]
    fn altered_capability_authority_facts_fail_closed() {
        const MISMATCH: &str =
            "workspace capability-authority edges disagree with retained module permits";

        assert_effect_validation_error(
            |edges| {
                let index = edges
                    .iter()
                    .position(|edge| {
                        edge.kind == "capability_authority" && edge.caller == "app.main"
                    })
                    .unwrap();
                edges.remove(index);
            },
            MISMATCH,
        );
        assert_effect_validation_error(
            |edges| {
                let mut extra = edges
                    .iter()
                    .find(|edge| edge.kind == "capability_authority" && edge.caller == "app.main")
                    .unwrap()
                    .clone();
                extra.target = "storage.read".to_owned();
                extra.expression = "permit.2".to_owned();
                extra.ast_path = "permit.2".to_owned();
                extra.ordinal = 2;
                edges.push(extra);
            },
            MISMATCH,
        );
        assert_effect_validation_error(
            |edges| {
                let indexes = edges
                    .iter()
                    .enumerate()
                    .filter(|(_, edge)| {
                        edge.kind == "capability_authority" && edge.caller == "app.main"
                    })
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                assert_eq!(indexes.len(), 2);
                let first = edges[indexes[0]].target.clone();
                edges[indexes[0]].target = edges[indexes[1]].target.clone();
                edges[indexes[1]].target = first;
            },
            MISMATCH,
        );
        assert_effect_validation_error(
            |edges| {
                edges
                    .iter_mut()
                    .find(|edge| edge.kind == "capability_authority" && edge.caller == "app.main")
                    .unwrap()
                    .target = "storage.read".to_owned();
            },
            MISMATCH,
        );
    }

    #[test]
    fn zero_effect_generic_template_has_retained_caller_authority() {
        let library = r#"
module lib.core;

@id("lib.zero")
fn zero() -> i64 { 0 }
"#;
        let app = r#"
module app.main;
use function @id("lib.zero") from lib.core as zero;

@id("app.keep")
fn keep<T>(value: T) -> T {
    let observed = zero();
    if observed == 0 { value } else { value }
}

@id("app.main")
fn main() -> i64 { keep<i64>(42) }
"#;
        let build = build_owned(vec![
            canonical_source("app/main.spx", app),
            canonical_source("lib/core.spx", library),
        ])
        .unwrap();
        let call = build
            .edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.caller == "app.keep")
            .expect("authored template call must be retained");
        assert_eq!(call.target, "lib.zero");
        validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();
    }

    #[test]
    fn indexed_effect_proof_replays_many_multi_effect_calls() {
        const CALLS: usize = 128;
        let library = r#"
module lib.core;
permit { audit.write, network.read }

@id("lib.multi")
fn multi() -> i64 uses { audit.write, network.read } { 42 }
"#;
        let mut app = String::from(
            r#"
module app.main;
use function @id("lib.multi") from lib.core as multi;
permit { audit.write, network.read }

@id("app.main")
fn main() -> i64 uses { audit.write, network.read } {
"#,
        );
        for index in 0..CALLS {
            app.push_str(&format!("    let value_{index} = multi();\n"));
        }
        app.push_str("    0\n}\n");

        let build = build_owned(vec![
            canonical_source("app/main.spx", &app),
            canonical_source("lib/core.spx", library),
        ])
        .unwrap();
        assert_eq!(
            build
                .edges
                .iter()
                .filter(|edge| edge.kind == "call" && edge.target == "lib.multi")
                .count(),
            CALLS
        );
        assert_eq!(
            build
                .edges
                .iter()
                .filter(|edge| edge.kind == "effect_requirement")
                .count(),
            CALLS * 2
        );
        validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();
    }

    #[test]
    fn indexed_proof_replays_many_permits_and_zero_effect_calls() {
        const PERMITS: usize = 96;
        const CALLS: usize = 96;
        let permits = (0..PERMITS)
            .map(|index| format!("capability.effect_{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let library = format!(
            "module lib.core;\npermit {{ {permits} }}\n\n@id(\"lib.zero\")\nfn zero() -> i64 {{ 0 }}\n"
        );
        let mut app = format!(
            "module app.main;\nuse function @id(\"lib.zero\") from lib.core as zero;\npermit {{ {permits} }}\n\n@id(\"app.main\")\nfn main() -> i64 {{\n"
        );
        for index in 0..CALLS {
            app.push_str(&format!("    let value_{index} = zero();\n"));
        }
        app.push_str("    0\n}\n");

        let build = build_owned(vec![
            canonical_source("app/main.spx", &app),
            canonical_source("lib/core.spx", &library),
        ])
        .unwrap();
        assert_eq!(
            build
                .edges
                .iter()
                .filter(|edge| edge.kind == "call")
                .count(),
            CALLS
        );
        assert_eq!(
            build
                .edges
                .iter()
                .filter(|edge| edge.kind == "capability_authority")
                .count(),
            PERMITS * 2
        );
        assert!(!build
            .edges
            .iter()
            .any(|edge| edge.kind == "effect_requirement"));
        validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();
    }

    #[test]
    fn canonical_use_parses_formats_and_single_file_rejects() {
        let text = "module app.main;\nuse function @id(\"lib.answer\") from lib.core as answer;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    answer()\n}\n";
        let program = parse(text, Path::new("app/main.spx")).unwrap();
        assert_eq!(format::canonical(&program), text);
        let error = hir::resolve(&program).expect_err("single-file HIR must reject workspace use");
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(
            error[0].message,
            "source module imports require Workspace Semantic Graph resolution"
        );
    }

    #[test]
    fn scalar_cross_file_call_resolves_once_and_reconstructs_edge() {
        let app = "module app.main;\nuse function @id(\"lib.answer\") from lib.core as answer;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    answer()\n}\n";
        let library = "module lib.core;\n\n@id(\"lib.answer\")\nfn answer() -> i64\n{\n    42\n}\n";
        let build = build_owned(vec![
            source("lib/core.spx", library),
            source("app/main.spx", app),
        ])
        .unwrap();
        assert_eq!(build.hir.modules.len(), 2);
        let app_hir = build
            .hir
            .modules
            .iter()
            .find(|module| module.module == "app.main")
            .unwrap();
        let lib_hir = build
            .hir
            .modules
            .iter()
            .find(|module| module.module == "lib.core")
            .unwrap();
        assert_eq!(app_hir.functions.len(), 1);
        assert_eq!(app_hir.functions[0].id.as_str(), "app.main");
        assert_eq!(lib_hir.functions.len(), 1);
        assert_eq!(lib_hir.functions[0].id.as_str(), "lib.answer");
        assert!(build.hir.shared_prelude_ids.contains(prelude::OPTION_ID));
        assert!(build.edges.iter().any(|edge| edge.kind == "call"
            && edge.caller == "app.main"
            && edge.target == "lib.answer"));
    }

    #[test]
    fn module_kind_alias_and_cycle_confusion_fail_closed() {
        let a = "module a;\nuse function @id(\"b.value\") from b as value;\n\n@id(\"a.main\")\nfn main() -> i64\n{\n    value()\n}\n";
        let b = "module b;\nuse function @id(\"a.main\") from a as other;\n\n@id(\"b.value\")\nfn value() -> i64\n{\n    other()\n}\n";
        let error = build_owned(vec![source("a.spx", a), source("b.spx", b)])
            .err()
            .expect("cycle must fail");
        assert_eq!(error[0].code, "SPX-G172");
        assert!(error[0].message.contains("a -> b -> a"));
    }

    #[test]
    fn file_limit_fails_before_parse() {
        let error = build_owned(vec![source("a.spx", "not parsed")])
            .err()
            .expect("one file is outside the admitted domain");
        assert_eq!(error[0].code, "SPX-G170");
        assert_eq!(
            error[0].message,
            "Workspace Semantic Graph requires 2..16 source files"
        );
    }

    #[test]
    fn repeated_calls_preserve_contract_body_paths_and_root_local_ordinals() {
        let library = r#"
module lib.core;

@id("lib.flag")
fn flag() -> bool { true }

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
        let app = r#"
module app.main;
use function @id("lib.flag") from lib.core as flag;
use function @id("lib.answer") from lib.core as answer;

@id("app.main")
fn main() -> i64
    requires flag()
    requires flag()
    ensures flag()
    ensures flag()
{
    answer() + answer() + answer()
}

"#;

        let build = build_owned(vec![
            canonical_source("app/main.spx", app),
            canonical_source("lib/core.spx", library),
        ])
        .unwrap();
        let calls = build
            .edges
            .iter()
            .filter(|edge| edge.kind == "call" && edge.caller == "app.main")
            .map(|edge| {
                (
                    edge.site,
                    edge.ast_path.as_str(),
                    edge.ordinal,
                    edge.target.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            calls,
            BTreeSet::from([
                ("requires", "requires.0", 0, "lib.flag"),
                ("requires", "requires.1", 0, "lib.flag"),
                ("body", "body.tail.left.left", 0, "lib.answer"),
                ("body", "body.tail.left.right", 1, "lib.answer"),
                ("body", "body.tail.right", 2, "lib.answer"),
                ("ensures", "ensures.0", 0, "lib.flag"),
                ("ensures", "ensures.1", 0, "lib.flag"),
            ])
        );
    }

    #[test]
    fn interleaved_template_sites_are_authored_once_across_two_materializations() {
        let library = r#"
module lib.core;

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
        let app = r#"
module app.main;
use function @id("lib.answer") from lib.core as answer;

@id("app.before")
fn before() -> i64 { answer() }

@id("app.keep")
fn keep<T>(value: T) -> T {
    let observed = answer();
    if observed == 42 { value } else { value }
}

@id("app.after")
fn after() -> i64 { answer() }

@id("app.main")
fn main() -> i64 {
    let number = keep<i64>(before());
    if keep<bool>(true) { number + after() } else { 0 }
}
"#;

        let build = build_owned(vec![
            canonical_source("app/main.spx", app),
            canonical_source("lib/core.spx", library),
        ])
        .unwrap();
        let app_hir = build
            .hir
            .modules
            .iter()
            .find(|module| module.module == "app.main")
            .unwrap();
        assert_eq!(app_hir.function_templates.len(), 1);
        assert_eq!(app_hir.function_templates[0].id.as_str(), "app.keep");
        assert_eq!(app_hir.function_instances.len(), 2);
        assert!(app_hir
            .function_instances
            .iter()
            .all(|instance| instance.template.as_str() == "app.keep"));

        let call_sites = build
            .edges
            .iter()
            .filter(|edge| edge.kind == "call" && edge.target == "lib.answer")
            .map(|edge| (edge.caller.as_str(), edge.ast_path.as_str()))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            call_sites,
            BTreeSet::from([
                ("app.after", "body.tail"),
                ("app.before", "body.tail"),
                ("app.keep", "body.s0.value"),
            ])
        );
        assert_eq!(
            build
                .edges
                .iter()
                .filter(|edge| edge.kind == "call"
                    && edge.caller == "app.keep"
                    && edge.target == "lib.answer")
                .count(),
            1,
            "materialized instances must not duplicate one authored template site"
        );
    }

    #[test]
    fn automatic_call_owner_retains_automatic_identity_origin() {
        let library = r#"
module lib.core;

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
        let app = r#"
module app.main;
use function @id("lib.answer") from lib.core as answer;

fn helper() -> i64 { answer() }

@id("app.main")
fn main() -> i64 { helper() }
"#;

        let build = build_owned(vec![
            canonical_source("app/main.spx", app),
            canonical_source("lib/core.spx", library),
        ])
        .unwrap();
        let call = build
            .edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.target == "lib.answer")
            .expect("automatic helper call must be retained");
        let fact = build
            .hir
            .declarations
            .get(&call.caller)
            .expect("automatic caller must have one declaration fact");
        assert_eq!(fact.origin, hir::IdentityOrigin::Automatic);
        assert_eq!(fact.path.as_deref(), Some("app/main.spx"));
        assert_eq!(fact.module.as_deref(), Some("app.main"));
    }

    #[test]
    fn identity_facts_preserve_authored_origins_parents_and_exact_prelude() {
        let build = build_owned(identity_fact_sources()).unwrap();
        let facts = &build.hir.declarations;
        let assert_fact = |id: &str,
                           kind: hir::DeclarationKind,
                           origin: hir::IdentityOrigin,
                           owner: Option<&str>| {
            let fact = facts
                .get(id)
                .unwrap_or_else(|| panic!("missing fact `{id}`"));
            assert_eq!(fact.kind, kind, "kind for `{id}`");
            assert_eq!(fact.origin, origin, "origin for `{id}`");
            assert_eq!(fact.owner.as_deref(), owner, "owner for `{id}`");
            assert_eq!(fact.path.as_deref(), Some("app/identity.spx"));
            assert_eq!(fact.module.as_deref(), Some("app.identity"));
        };

        assert_fact(
            "auto:app.identity.helper",
            hir::DeclarationKind::Function,
            hir::IdentityOrigin::Automatic,
            None,
        );
        assert_fact(
            "auto:field:app.record.value",
            hir::DeclarationKind::Field,
            hir::IdentityOrigin::Automatic,
            Some("app.record"),
        );
        assert_fact(
            "auto:case:app.choice.Number",
            hir::DeclarationKind::VariantCase,
            hir::IdentityOrigin::Automatic,
            Some("app.choice"),
        );
        assert_fact(
            "auto:case-field:auto:case:app.choice.Number.value",
            hir::DeclarationKind::CaseField,
            hir::IdentityOrigin::Automatic,
            Some("auto:case:app.choice.Number"),
        );
        assert_fact(
            "app.explicit_variant",
            hir::DeclarationKind::Variant,
            hir::IdentityOrigin::Explicit,
            None,
        );
        assert_fact(
            "app.explicit_variant.ready",
            hir::DeclarationKind::VariantCase,
            hir::IdentityOrigin::Explicit,
            Some("app.explicit_variant"),
        );
        assert_fact(
            "app.explicit_variant.ready.code",
            hir::DeclarationKind::CaseField,
            hir::IdentityOrigin::Explicit,
            Some("app.explicit_variant.ready"),
        );
        assert_fact(
            "app.token.drop",
            hir::DeclarationKind::ResourceDrop,
            hir::IdentityOrigin::Explicit,
            Some("app.token"),
        );
        assert_fact(
            "app.host",
            hir::DeclarationKind::Interface,
            hir::IdentityOrigin::Explicit,
            None,
        );
        assert_fact(
            "app.host.observe",
            hir::DeclarationKind::Import,
            hir::IdentityOrigin::Explicit,
            Some("app.host"),
        );

        let compiler = facts
            .iter()
            .filter(|(_, fact)| fact.origin == hir::IdentityOrigin::CompilerOwned)
            .map(|(id, fact)| {
                assert_eq!(fact.path, None, "compiler fact path for `{id}`");
                assert_eq!(fact.module, None, "compiler fact module for `{id}`");
                id.as_str()
            })
            .collect::<BTreeSet<_>>();
        let expected = prelude::all_ids().into_iter().collect::<BTreeSet<_>>();
        assert_eq!(compiler, expected);
        assert_eq!(build.hir.shared_prelude_ids, expected);
    }

    #[test]
    fn imported_stubs_and_synthetic_mains_are_not_retained() {
        let build = build_owned(identity_fact_sources()).unwrap();
        let app = build
            .hir
            .modules
            .iter()
            .find(|module| module.module == "app.identity")
            .unwrap();
        let library = build
            .hir
            .modules
            .iter()
            .find(|module| module.module == "lib.core")
            .unwrap();
        let app_functions = app
            .functions
            .iter()
            .map(|function| function.id.as_str())
            .collect::<BTreeSet<_>>();
        let library_functions = library
            .functions
            .iter()
            .map(|function| function.id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            app_functions,
            BTreeSet::from(["app.main", "auto:app.identity.helper"])
        );
        assert_eq!(library_functions, BTreeSet::from(["lib.answer"]));
        assert!(!build.hir.declarations.contains_key("lib.answer.stub"));
        assert!(!build
            .hir
            .declarations
            .contains_key("workspace.synthetic.main.lib.core"));
        let imported = build.hir.declarations.get("lib.answer").unwrap();
        assert_eq!(imported.path.as_deref(), Some("lib/core.spx"));
        assert_eq!(imported.module.as_deref(), Some("lib.core"));
    }

    #[test]
    fn missing_or_substituted_identity_shape_facts_fail_closed() {
        assert_identity_shape_error(|facts| {
            facts.remove("auto:app.identity.helper");
        });
        assert_identity_shape_error(|facts| {
            facts.get_mut("app.record").unwrap().kind = hir::DeclarationKind::Interface;
        });
        assert_identity_shape_error(|facts| {
            facts.get_mut("auto:field:app.record.value").unwrap().owner =
                Some("app.choice".to_owned());
        });
        assert_identity_shape_error(|facts| {
            facts.get_mut("app.record").unwrap().path = Some("wrong/path.spx".to_owned());
        });
        assert_identity_shape_error(|facts| {
            facts.get_mut("app.record").unwrap().module = Some("wrong.module".to_owned());
        });
    }

    #[test]
    fn substituted_identity_origin_disagrees_with_retained_hir() {
        let sources = identity_fact_sources();
        let programs = parsed_sources(&sources);
        let build = build_owned(sources).unwrap();
        let synthetic_modules = {
            let authored = index_authored(&programs).unwrap();
            programs
                .iter()
                .map(|program| {
                    let synthetic = synthetic_program(program, &authored, &programs).unwrap();
                    (
                        program.module.clone(),
                        hir::resolve(&synthetic).expect("synthetic identity fixture must resolve"),
                    )
                })
                .collect::<Vec<_>>()
        };
        let mut substituted = programs.clone();
        substituted
            .iter_mut()
            .find(|program| program.module == "app.identity")
            .unwrap()
            .functions
            .iter_mut()
            .find(|function| function.stable_id == "auto:app.identity.helper")
            .unwrap()
            .explicit_id = true;

        let error =
            workspace_declaration_facts(&synthetic_modules, &build.hir.modules, &substituted)
                .expect_err("origin substitution must fail closed");
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "authored workspace declaration facts disagree with retained HIR"
        );
    }

    #[test]
    fn independent_prelude_map_detects_kind_and_owner_substitutions() {
        let build = build_owned(identity_fact_sources()).unwrap();
        let actual = build
            .hir
            .declarations
            .iter()
            .filter(|(_, fact)| fact.origin == hir::IdentityOrigin::CompilerOwned)
            .map(|(id, fact)| (id.clone(), fact.clone()))
            .collect::<BTreeMap<_, _>>();
        let expected = expected_compiler_declaration_facts().unwrap();
        assert_eq!(actual, expected);

        let root_id = expected
            .iter()
            .find(|(_, fact)| fact.owner.is_none())
            .map(|(id, _)| id.clone())
            .expect("prelude must have a root declaration");
        let mut wrong_kind = expected.clone();
        wrong_kind.get_mut(&root_id).unwrap().kind = hir::DeclarationKind::Function;
        assert_ne!(actual, wrong_kind);

        let child_id = expected
            .iter()
            .find(|(_, fact)| fact.owner.is_some())
            .map(|(id, _)| id.clone())
            .expect("prelude must have a child declaration");
        let mut wrong_owner = expected;
        wrong_owner.get_mut(&child_id).unwrap().owner = Some("wrong.prelude.owner".to_owned());
        assert_ne!(actual, wrong_owner);
    }

    #[test]
    fn rogue_and_nonimported_foreign_resolved_roots_fail_closed() {
        assert_resolved_identity_error(
            |module, synthetic, programs| {
                if module != "app.identity" {
                    return;
                }
                let mut rogue = programs
                    .iter()
                    .find(|program| program.module == "app.identity")
                    .unwrap()
                    .functions
                    .iter()
                    .find(|function| function.name == "helper")
                    .unwrap()
                    .clone();
                rogue.stable_id = "rogue.function".to_owned();
                rogue.explicit_id = true;
                rogue.name = "rogue".to_owned();
                synthetic.functions.push(rogue);
            },
            "resolved workspace declaration has an unauthenticated synthetic or rogue root",
        );
        assert_resolved_identity_error(
            |module, synthetic, programs| {
                if module != "app.identity" {
                    return;
                }
                let foreign = programs
                    .iter()
                    .find(|program| program.module == "lib.core")
                    .unwrap()
                    .types
                    .iter()
                    .find(|declaration| declaration.stable_id == "lib.foreign")
                    .unwrap()
                    .clone();
                synthetic.types.push(foreign);
            },
            "resolved workspace declaration leaks a non-imported foreign authority",
        );
    }

    #[test]
    fn extra_descendant_under_imported_type_fails_closed() {
        assert_resolved_identity_error(
            |module, synthetic, _| {
                if module != "app.identity" {
                    return;
                }
                let imported = synthetic
                    .types
                    .iter_mut()
                    .find(|declaration| declaration.stable_id == "lib.imported")
                    .unwrap();
                let TypeDeclarationKind::Record { fields } = &mut imported.kind else {
                    panic!("imported fixture must be a record")
                };
                let mut extra = fields[0].clone();
                extra.stable_id = "lib.imported.extra".to_owned();
                extra.explicit_id = true;
                extra.name = "extra".to_owned();
                fields.push(extra);
            },
            "resolved workspace declaration leaks a non-imported foreign authority",
        );
    }

    #[test]
    fn synthetic_main_allowlist_is_exact_and_collision_fails_closed() {
        let sources = identity_fact_sources();
        let programs = parsed_sources(&sources);
        let build = build_owned(sources).unwrap();
        let synthetic_modules = {
            let authored = index_authored(&programs).unwrap();
            programs
                .iter()
                .map(|program| {
                    let synthetic = synthetic_program(program, &authored, &programs).unwrap();
                    (
                        program.module.clone(),
                        hir::resolve(&synthetic).expect("exact synthetic main must resolve"),
                    )
                })
                .collect::<Vec<_>>()
        };
        workspace_declaration_facts(&synthetic_modules, &build.hir.modules, &programs).unwrap();

        let collision = canonical_source(
            "collision/lib.spx",
            r#"
module collision.lib;

@id("workspace.synthetic.main.collision.lib")
fn helper() -> i64 { 0 }
"#,
        );
        let app = canonical_source(
            "collision/app.spx",
            r#"
module collision.app;

@id("collision.app.main")
fn main() -> i64 { 0 }
"#,
        );
        let error = build_owned(vec![app, collision])
            .err()
            .expect("authored synthetic-main collision must fail closed");
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "generated workspace synthetic main identity collides with an authored declaration"
        );
    }

    #[test]
    fn dependency_depths_cover_chain_diamond_branching_and_canonical_cycle() {
        let names = (0..16)
            .map(|index| format!("m{index:02}"))
            .collect::<Vec<_>>();
        let mut chain = BTreeMap::new();
        for (index, module) in names.iter().enumerate() {
            let dependencies = if index == 0 {
                BTreeSet::new()
            } else {
                BTreeSet::from([names[index - 1].as_str()])
            };
            chain.insert(module.as_str(), dependencies);
        }
        let depths = dependency_depths(&chain).unwrap();
        for (index, module) in names.iter().enumerate() {
            assert_eq!(depths[module.as_str()], index + 1);
        }

        let diamond = BTreeMap::from([
            ("leaf", BTreeSet::new()),
            ("left", BTreeSet::from(["leaf"])),
            ("right", BTreeSet::from(["leaf"])),
            ("root", BTreeSet::from(["left", "right"])),
            ("wide", BTreeSet::from(["leaf", "left", "right"])),
        ]);
        let depths = dependency_depths(&diamond).unwrap();
        assert_eq!(depths["leaf"], 1);
        assert_eq!(depths["left"], 2);
        assert_eq!(depths["right"], 2);
        assert_eq!(depths["root"], 3);
        assert_eq!(depths["wide"], 3);

        let cycle = BTreeMap::from([
            ("z", BTreeSet::from(["a"])),
            ("a", BTreeSet::from(["m"])),
            ("m", BTreeSet::from(["z"])),
        ]);
        let error = dependency_depths(&cycle).unwrap_err();
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(
            error[0].message,
            "workspace module dependency cycle: a -> m -> z -> a"
        );
    }

    #[test]
    fn logical_limits_accept_exact_and_reject_one_over() {
        for (field, maximum) in [
            ("files", MAX_FILES),
            ("total_source_bytes", MAX_TOTAL_SOURCE_BYTES),
            ("declarations", MAX_DECLARATIONS),
            ("callables", MAX_CALLABLES),
            ("calls", MAX_CALLS),
            ("uses", MAX_USES),
            ("resolved_cross_file_edges", MAX_CROSS_FILE_EDGES),
            ("dependency_depth", MAX_DEPENDENCY_DEPTH),
            ("builder_bytes", MAX_BUILDER_BYTES),
        ] {
            assert_eq!(checked_usage(0, maximum, field, maximum).unwrap(), maximum);
            let error = checked_usage(maximum, 1, field, maximum).unwrap_err();
            assert_eq!(error[0].code, "SPX-G171");
            assert_eq!(
                error[0].message,
                format!("Workspace Semantic Graph `{field}` exceeds {maximum}")
            );
        }
    }

    #[test]
    fn exact_file_limit_builds_and_one_over_rejects_before_parse() {
        let exact = (0..MAX_FILES)
            .map(|index| {
                canonical_source(
                    &format!("m{index:02}.spx"),
                    &format!(
                        "module m{index:02};\n\n@id(\"m{index:02}.entry\")\nfn entry() -> i64 {{ {index} }}\n"
                    ),
                )
            })
            .collect::<Vec<_>>();
        build_owned(exact).unwrap();

        let over = (0..=MAX_FILES)
            .map(|index| source(&format!("bad{index}.spx"), "not parsed"))
            .collect::<Vec<_>>();
        let error = build_owned(over).err().expect("file one-over must fail");
        assert_eq!(error[0].code, "SPX-G171");
    }

    #[test]
    fn edge_append_and_builder_prebound_enforce_exact_boundaries() {
        let edge = WorkspaceEdge {
            caller_path: "a.spx".to_owned(),
            caller: "a.main".to_owned(),
            target_path: "b.spx".to_owned(),
            target: "b.value".to_owned(),
            kind: "call",
            site: "body",
            expression: "expression".to_owned(),
            ast_path: "body.tail".to_owned(),
            alias: "value".to_owned(),
            ordinal: 0,
        };
        let mut exact = vec![edge.clone(); MAX_CROSS_FILE_EDGES - 1];
        push_edge(&mut exact, edge.clone()).unwrap();
        let error = push_edge(&mut exact, edge).unwrap_err();
        assert_eq!(error[0].code, "SPX-G171");

        let (exact, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
            charge_builder_prebound(MAX_BUILDER_BYTES)
        });
        assert!(!overflowed);
        exact.unwrap();
        let (over, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
            charge_builder_prebound(MAX_BUILDER_BYTES + 1)
        });
        assert!(overflowed || over.is_err());
    }

    #[test]
    fn four_two_parameter_materializations_and_t226_premises_are_preserved() {
        let app = canonical_source(
            "generic/app.spx",
            r#"
module generic.app;
@id("generic.first") fn first<T, U>(left: T, right: U) -> T { left }
@id("generic.app.main") fn main() -> i64 {
    let ii = first<i64, i64>(1, 2);
    let ib = first<i64, bool>(ii, true);
    let bi = first<bool, i64>(false, ib);
    if first<bool, bool>(bi, true) { ib } else { 0 }
}
"#,
        );
        let leaf = canonical_source(
            "generic/leaf.spx",
            "module generic.leaf;\n@id(\"generic.leaf.value\") fn value() -> i64 { 0 }\n",
        );
        let build = build_owned(vec![app, leaf]).unwrap();
        let module = build
            .hir
            .modules
            .iter()
            .find(|module| module.module == "generic.app")
            .unwrap();
        assert_eq!(module.function_instances.len(), 4);

        for invalid in [
            r#"module bad.direct;
@id("bad.b") fn b<T>(value: T) -> T { value }
@id("bad.a") fn a<T>(value: T) -> T { b<i64>(0) }
@id("bad.main") fn main() -> i64 { 0 }"#,
            r#"module bad.transitive;
@id("bad.b") fn b<T>(value: T) -> T { value }
@id("bad.middle") fn middle() -> i64 { b<i64>(0) }
@id("bad.a") fn a<T>(value: T) -> T { let seen = middle(); if seen == 0 { value } else { value } }
@id("bad.main") fn main() -> i64 { 0 }"#,
        ] {
            let bad = canonical_source("bad.spx", invalid);
            let leaf = canonical_source(
                "leaf.spx",
                "module leaf;\n@id(\"leaf.value\") fn value() -> i64 { 0 }\n",
            );
            let error = build_owned(vec![bad, leaf])
                .err()
                .expect("T226 must survive");
            assert!(error.iter().any(|diagnostic| diagnostic.code == "SPX-T226"));
        }
    }

    #[test]
    fn long_identity_many_calls_and_deep_paths_replay_deterministically() {
        const CALLS: usize = 32;
        const DEPTH: usize = 12;
        let target = format!("lib.{}", "x".repeat(64));
        let library = canonical_source(
            "long/lib.spx",
            &format!(
                "module long.lib;\n@id(\"{target}\") fn value(input: i64) -> i64 {{ input }}\n"
            ),
        );
        let mut app = format!("module long.app;\nuse function @id(\"{target}\") from long.lib as value;\n@id(\"long.app.main\") fn main() -> i64 {{\n");
        for index in 0..CALLS {
            app.push_str(&format!("let value_{index} = value(0);\n"));
        }
        let mut tail = "0".to_owned();
        for _ in 0..DEPTH {
            tail = format!("value({tail})");
        }
        app.push_str(&format!("{tail}\n}}\n"));
        let app = canonical_source("long/app.spx", &app);
        let first = build_owned(vec![app.clone(), library.clone()]).unwrap();
        let second = build_owned(vec![library, app]).unwrap();
        assert_eq!(first.edges, second.edges);
        assert_eq!(
            first
                .edges
                .iter()
                .filter(|edge| edge.kind == "call" && edge.target == target)
                .count(),
            CALLS + DEPTH
        );
        assert!(first.edges.iter().any(
            |edge| edge.kind == "call" && edge.ast_path.matches(".arg.0").count() == DEPTH - 1
        ));
    }

    fn is_named_limit(diagnostics: &[Diagnostic], field: &str) -> bool {
        diagnostics.first().is_some_and(|diagnostic| {
            diagnostic.code == "SPX-G171"
                && diagnostic.message.contains(&format!("`{field}` exceeds"))
        })
    }

    #[test]
    fn source_byte_limit_is_checked_before_parse_at_one_over() {
        let exact = vec![
            source("a.spx", &"x".repeat(MAX_TOTAL_SOURCE_BYTES - 1)),
            source("b.spx", "x"),
        ];
        let error = build_owned(exact).err().expect("exact bytes reach parsing");
        assert!(!is_named_limit(&error, "total_source_bytes"));

        let over = vec![
            source("a.spx", &"x".repeat(MAX_TOTAL_SOURCE_BYTES)),
            source("b.spx", "x"),
        ];
        let error = build_owned(over).err().expect("one-over bytes must fail");
        assert!(is_named_limit(&error, "total_source_bytes"));
    }

    fn declaration_boundary_source(functions: usize) -> WorkspaceSource {
        let mut text = String::from(
            r#"
module boundary.declarations;
@id("d.token") resource Token { @id("d.token.drop") drop trivial; }
@id("d.record") record Record { value: i64, }
@id("d.variant") variant Variant { Case { value: i64, }, }
@id("d.host") interface Host permits {} {
    @id("d.host.consume") import fn consume(value: own Token) -> unit
        effects {} failure infallible consumes value always;
}
"#,
        );
        for index in 0..functions {
            text.push_str(&format!(
                "@id(\"d.f{index}\") fn f{index}() -> i64 {{ 0 }}\n"
            ));
        }
        canonical_source("z_declarations.spx", &text)
    }

    #[test]
    fn mixed_declaration_limit_exact_advances_and_one_over_is_g171() {
        let leaf = canonical_source(
            "a_leaf.spx",
            "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
        );
        let exact = declaration_boundary_source(MAX_DECLARATIONS - 10);
        let exact_program = parse(&exact.source, Path::new(&exact.path)).unwrap();
        assert_eq!(
            declaration_count(&exact_program),
            Some(MAX_DECLARATIONS - 1)
        );
        let error = build_owned(vec![exact, leaf.clone()])
            .err()
            .expect("later gate expected");
        assert!(!is_named_limit(&error, "declarations"));

        let over = declaration_boundary_source(MAX_DECLARATIONS - 9);
        let error = build_owned(vec![over, leaf])
            .err()
            .expect("one-over declarations fail");
        assert!(is_named_limit(&error, "declarations"), "{error:?}");
    }

    fn callable_boundary_source(functions: usize) -> WorkspaceSource {
        let mut text = String::from("module boundary.callables;\n");
        for index in 0..functions {
            text.push_str(&format!(
                "@id(\"c.f{index}\") fn f{index}() -> i64 {{ 0 }}\n"
            ));
        }
        canonical_source("callables.spx", &text)
    }

    #[test]
    fn callable_limit_exact_advances_and_one_over_is_g171() {
        let leaf = canonical_source(
            "leaf.spx",
            "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
        );
        let exact = callable_boundary_source(MAX_CALLABLES - 1);
        let error = build_owned(vec![exact, leaf.clone()])
            .err()
            .expect("later gate expected");
        assert!(!is_named_limit(&error, "callables"));
        let over = callable_boundary_source(MAX_CALLABLES);
        let error = build_owned(vec![over, leaf])
            .err()
            .expect("one-over callables fail");
        assert!(is_named_limit(&error, "callables"));
    }

    fn call_boundary_source(body_calls: usize) -> WorkspaceSource {
        let mut text = String::from(
            r#"
module boundary.calls;
@id("calls.flag") fn flag() -> bool { true }
@id("calls.zero") fn zero() -> i64 { 0 }
@id("calls.keep") fn keep<T>(value: T) -> T
    requires flag()
    ensures flag()
{ let seen = zero(); value }
@id("calls.main") fn main() -> i64 {
"#,
        );
        for index in 0..body_calls {
            text.push_str(&format!("let value_{index} = zero();\n"));
        }
        text.push_str("0\n}\n");
        canonical_source("calls.spx", &text)
    }

    #[test]
    fn call_limit_exact_advances_and_one_over_is_g171() {
        let leaf = canonical_source(
            "leaf.spx",
            "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
        );
        let exact = call_boundary_source(MAX_CALLS - 3);
        let error = build_owned(vec![exact, leaf.clone()])
            .err()
            .expect("later gate expected");
        assert!(!is_named_limit(&error, "calls"));
        let over = call_boundary_source(MAX_CALLS - 2);
        let error = build_owned(vec![over, leaf])
            .err()
            .expect("one-over calls fail");
        assert!(is_named_limit(&error, "calls"));
    }

    fn use_boundary_source(uses: usize) -> WorkspaceSource {
        let mut text = String::from("module boundary.uses;\n");
        for index in 0..uses {
            text.push_str(&format!(
                "use function @id(\"missing.f{index}\") from missing.module as f{index};\n"
            ));
        }
        text.push_str("@id(\"uses.main\") fn main() -> i64 { 0 }\n");
        canonical_source("uses.spx", &text)
    }

    #[test]
    fn use_limit_exact_advances_and_one_over_is_g171() {
        let leaf = canonical_source(
            "leaf.spx",
            "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
        );
        let exact = use_boundary_source(MAX_USES);
        let error = build_owned(vec![exact, leaf.clone()])
            .err()
            .expect("later gate expected");
        assert!(!is_named_limit(&error, "uses"));
        let over = use_boundary_source(MAX_USES + 1);
        let error = build_owned(vec![over, leaf])
            .err()
            .expect("one-over uses fail");
        assert!(is_named_limit(&error, "uses"));
    }

    #[test]
    fn branching_copy_default_expansion_is_rejected_by_builder_preflight() {
        const LEVELS: usize = 12;
        const BRANCHES: usize = 4;
        let mut provider = String::from("module hostile.copy;\n");
        provider.push_str("@id(\"copy.r0\") record R0 { @id(\"copy.r0.value\") value: i64, }\n");
        for level in 1..LEVELS {
            provider.push_str(&format!("@id(\"copy.r{level}\") record R{level} {{\n"));
            for field in 0..BRANCHES {
                provider.push_str(&format!(
                    "@id(\"copy.r{level}.f{field}\") f{field}: R{},\n",
                    level - 1
                ));
            }
            provider.push_str("}\n");
        }
        provider.push_str(&format!(
            "@id(\"copy.make\") fn make(value: R{}) -> R{} {{ value }}\n",
            LEVELS - 1,
            LEVELS - 1
        ));

        let mut consumer = String::from("module hostile.consumer;\n");
        consumer.push_str("use function @id(\"copy.make\") from hostile.copy as make;\n");
        for level in 0..LEVELS {
            consumer.push_str(&format!(
                "use type @id(\"copy.r{level}\") from hostile.copy as R{level};\n"
            ));
        }
        consumer.push_str("@id(\"hostile.main\") fn main() -> i64 { 0 }\n");
        let sources = vec![
            canonical_source("hostile/consumer.spx", &consumer),
            canonical_source("hostile/copy.spx", &provider),
        ];
        let programs = parsed_sources(&sources);
        let authored = index_authored(&programs).unwrap();
        let consumer = programs
            .iter()
            .find(|program| program.module == "hostile.consumer")
            .unwrap();
        let error = match synthetic_builder_bytes(consumer, &authored, &programs) {
            Ok(_) => panic!("branching default expansion must fail pre-HIR"),
            Err(error) => error,
        };
        assert!(is_named_limit(&error, "builder_bytes"));
    }

    #[test]
    fn long_nominal_and_child_id_repetition_is_rejected_by_builder_preflight() {
        const READERS: usize = 512;
        let type_id = format!("long.type.{}", "t".repeat(2048));
        let field_id = format!("long.field.{}", "f".repeat(2048));
        let provider = canonical_source(
            "long-type/provider.spx",
            &format!(
                "module long_type.provider;\n@id(\"{type_id}\") record Long {{ @id(\"{field_id}\") value: i64, }}\n@id(\"long.type.local\") fn local() -> i64 {{ 0 }}\n"
            ),
        );
        let mut consumer = format!(
            "module long_type.consumer;\nuse type @id(\"{type_id}\") from long_type.provider as L;\n"
        );
        for index in 0..READERS {
            consumer.push_str(&format!(
                "@id(\"reader.{index}\") fn read_{index}(value: L) -> i64 {{ match value {{ L {{ value }} => value, }} }}\n"
            ));
        }
        consumer.push_str("@id(\"long.type.main\") fn main() -> i64 { let value = L { value: 0 }; match value { L { value } => value, } }\n");
        let consumer = canonical_source("long-type/consumer.spx", &consumer);
        let sources = vec![consumer, provider];
        let programs = parsed_sources(&sources);
        let authored = index_authored(&programs).unwrap();
        let consumer = programs
            .iter()
            .find(|program| program.module == "long_type.consumer")
            .unwrap();
        let error = match synthetic_builder_bytes(consumer, &authored, &programs) {
            Ok(_) => panic!("long rewritten nominal identities must fail pre-HIR"),
            Err(error) => error,
        };
        assert!(is_named_limit(&error, "builder_bytes"));
    }

    fn minimum_successful_builder_limit(sources: &[WorkspaceSource]) -> usize {
        assert!(build_owned_with_builder_limit(sources.to_vec(), MAX_BUILDER_BYTES).is_ok());
        let mut low = 0usize;
        let mut high = MAX_BUILDER_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if build_owned_with_builder_limit(sources.to_vec(), middle).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }

    #[test]
    #[should_panic(
        expected = "private Workspace Semantic Graph builder limit cannot exceed the production maximum"
    )]
    fn private_builder_limit_cannot_widen_the_production_cap() {
        let _ = build_owned_with_builder_limit(Vec::new(), MAX_BUILDER_BYTES + 1);
    }

    fn assert_exact_builder_limit_error(error: &[Diagnostic], limit: usize) {
        assert_eq!(error[0].code, "SPX-G171");
        assert_eq!(
            error[0].message,
            format!("Workspace Semantic Graph `builder_bytes` exceeds {limit}")
        );
    }

    #[test]
    fn all_four_generic_materializations_have_an_exact_minimum_builder_limit() {
        let app = canonical_source(
            "generic-limit/app.spx",
            r#"
module generic_limit.app;
@id("generic.limit.first") fn first<T, U>(left: T, right: U) -> T { left }
@id("generic.limit.main") fn main() -> i64 {
    let ii = first<i64, i64>(1, 2);
    let ib = first<i64, bool>(ii, true);
    let bi = first<bool, i64>(false, ib);
    if first<bool, bool>(bi, true) { ib } else { 0 }
}
"#,
        );
        let leaf = canonical_source(
            "generic-limit/leaf.spx",
            "module generic_limit.leaf;\n@id(\"generic.limit.leaf\") fn leaf() -> i64 { 0 }\n",
        );
        let sources = vec![app, leaf];
        let minimum = minimum_successful_builder_limit(&sources);
        assert!(minimum > 0);
        let first = build_owned_with_builder_limit(sources.clone(), minimum).unwrap();
        let second = build_owned_with_builder_limit(sources.clone(), minimum).unwrap();
        assert_eq!(first.edges, second.edges);
        assert_eq!(first.hir.declarations, second.hir.declarations);

        let error = match build_owned_with_builder_limit(sources, minimum - 1) {
            Ok(_) => panic!("minimum minus one must fail"),
            Err(error) => error,
        };
        assert_exact_builder_limit_error(&error, minimum - 1);
    }

    #[test]
    fn late_module_work_has_an_exact_combined_minimum_builder_limit() {
        let provider = canonical_source(
            "late/a_provider.spx",
            r#"
module late.provider;
@id("late.value") fn value() -> i64 { 1 }
"#,
        );
        let minimal = canonical_source(
            "late/z_consumer.spx",
            r#"
module late.consumer;
@id("late.main") fn main() -> i64 { 0 }
"#,
        );
        let mut consumer = String::from(
            r#"
module late.consumer;
use function @id("late.value") from late.provider as value;
@id("late.main") fn main() -> i64 {
"#,
        );
        for index in 0..96 {
            consumer.push_str(&format!("let value_{index} = value();\n"));
        }
        consumer.push_str("0\n}\n");
        let consumer = canonical_source("late/z_consumer.spx", &consumer);

        let base = vec![provider.clone(), minimal];
        let combined = vec![provider, consumer];
        let base_minimum = minimum_successful_builder_limit(&base);
        let combined_minimum = minimum_successful_builder_limit(&combined);
        assert!(base_minimum < combined_minimum);
        assert!(build_owned_with_builder_limit(base, combined_minimum - 1).is_ok());

        let exact = build_owned_with_builder_limit(combined.clone(), combined_minimum).unwrap();
        let replay = build_owned_with_builder_limit(combined.clone(), combined_minimum).unwrap();
        assert_eq!(exact.edges, replay.edges);
        assert_eq!(exact.hir.declarations, replay.hir.declarations);
        let error = match build_owned_with_builder_limit(combined, combined_minimum - 1) {
            Ok(_) => panic!("late module must consume the final builder byte"),
            Err(error) => error,
        };
        assert_exact_builder_limit_error(&error, combined_minimum - 1);
    }
}

#[cfg(test)]
mod type_proof_tests {
    use super::*;

    fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
        let program = parse(source, Path::new(path)).expect("type-proof fixture must parse");
        WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&program),
        }
    }

    fn point_provider() -> WorkspaceSource {
        canonical_source(
            "lib/core.spx",
            r#"
module lib.core;

@id("lib.point")
record Point {
    @id("lib.point.x")
    x: i64,
}

@id("lib.local")
fn local(value: Point) -> Point { value }
"#,
        )
    }

    #[cfg(test)]
    mod projection_tests {
        use super::*;
        use std::fs::OpenOptions;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SERIAL: AtomicU64 = AtomicU64::new(0);

        struct ManagedFixture {
            root: std::path::PathBuf,
            revision: String,
        }

        impl ManagedFixture {
            fn new(label: &str) -> Self {
                let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
                let root = std::env::temp_dir().join(format!(
                    "semaprax-workspace-graph-projection-{label}-{}-{serial}",
                    std::process::id()
                ));
                std::fs::create_dir(&root).unwrap();
                for (path, module, id) in [
                    ("alpha.spx", "workspace.alpha", "workspace.alpha.run"),
                    ("beta.spx", "workspace.beta", "workspace.beta.run"),
                ] {
                    let source = canonical_source(
                        path,
                        &format!(
                            "module {module}; @id(\"{id}\") fn run()->i64{{0}} fn main()->i64{{run()}}"
                        ),
                    );
                    std::fs::write(root.join(path), source.source).unwrap();
                }
                let path_set = root.join("paths.json");
                std::fs::write(
                    &path_set,
                    "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n",
                )
                .unwrap();
                let revision =
                    crate::semantic_workspace::initialize_from_preflight(&root, &path_set).unwrap();
                Self { root, revision }
            }

            fn control(&self) -> std::path::PathBuf {
                self.root.join(".semaprax-workspace")
            }

            fn generation(&self) -> std::path::PathBuf {
                self.control().join("generations").join(
                    self.revision
                        .strip_prefix("sha256:")
                        .expect("workspace revision must be a canonical digest"),
                )
            }

            fn reacquire_exclusive(&self) {
                let lock = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(self.control().join("LOCK"))
                    .unwrap();
                fs2::FileExt::try_lock_exclusive(&lock).unwrap();
                fs2::FileExt::unlock(&lock).unwrap();
            }
        }

        impl Drop for ManagedFixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }

        fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
            let program = parse(source, Path::new(path)).expect("projection source must parse");
            WorkspaceSource {
                path: path.to_owned(),
                source: format::canonical(&program),
            }
        }

        fn projection_sources() -> Vec<WorkspaceSource> {
            vec![
                canonical_source(
                    "z/entry.spx",
                    r#"
module app.entry;
use function @id("provider.run") from provider.api as provider_run;

@id("app.entry.run")
fn run() -> i64 { provider_run() }
"#,
                ),
                canonical_source(
                    "m/provider.spx",
                    r#"
module provider.api;
use type @id("types.point") from types.core as Point;

@id("provider.run")
fn run() -> i64 { 7 }
"#,
                ),
                canonical_source(
                    "a/types.spx",
                    r#"
module types.core;

@id("types.point")
record Point { @id("types.point.x") x: i64, }

@id("types.local")
fn local() -> i64 { 0 }
"#,
                ),
                canonical_source(
                    "q/disconnected.spx",
                    r#"
module disconnected.reverse;
use function @id("app.entry.run") from app.entry as entry_run;

@id("disconnected.run")
fn run() -> i64 { entry_run() }
"#,
                ),
            ]
        }

        fn authenticated_build() -> AuthenticatedWorkspaceGraphBuild {
            let sources = projection_sources();
            authenticated_build_from(sources)
        }

        fn authenticated_build_from(
            sources: Vec<WorkspaceSource>,
        ) -> AuthenticatedWorkspaceGraphBuild {
            let source_facts = sources
                .iter()
                .map(|source| {
                    (
                        source.path.clone(),
                        AuthenticatedSourceFact {
                            path: source.path.clone(),
                            source_graph_schema: "semaprax.semantic-graph.v14".to_owned(),
                            source_revision: format!("revision:{}", source.path),
                            source_digest: format!("sha256:{:064x}", source.source.len()),
                        },
                    )
                })
                .collect();
            AuthenticatedWorkspaceGraphBuild {
                workspace_revision: "sha256:workspace".to_owned(),
                sources: source_facts,
                storage: AuthenticatedWorkspaceStorageUsage {
                    manifest_bytes: 711,
                    retained_generations: 3,
                    staging_attempts: 2,
                    unexpected_inventory_entries: 0,
                },
                graph: build_owned(sources)
                    .expect("full workspace must validate before projection"),
            }
        }

        fn diamond_sources() -> Vec<WorkspaceSource> {
            vec![
                canonical_source(
                    "z/entry.spx",
                    r#"
module diamond.entry;
use function @id("diamond.left") from diamond.left as left;
use function @id("diamond.right") from diamond.right as right;

@id("diamond.main")
fn main() -> i64 { left() + right() }
"#,
                ),
                canonical_source(
                    "b/left.spx",
                    r#"
module diamond.left;
use function @id("diamond.leaf") from diamond.leaf as leaf;

@id("diamond.left")
fn left() -> i64 { leaf() }
"#,
                ),
                canonical_source(
                    "c/right.spx",
                    r#"
module diamond.right;
use function @id("diamond.leaf") from diamond.leaf as leaf;

@id("diamond.right")
fn right() -> i64 { leaf() }
"#,
                ),
                canonical_source(
                    "a/leaf.spx",
                    r#"
module diamond.leaf;

@id("diamond.leaf")
fn leaf() -> i64 { 1 }
"#,
                ),
                canonical_source(
                    "y/reverse.spx",
                    r#"
module diamond.reverse;
use function @id("diamond.main") from diamond.entry as entry;

@id("diamond.reverse")
fn reverse() -> i64 { entry() }
"#,
                ),
                canonical_source(
                    "x/disconnected.spx",
                    r#"
module diamond.disconnected;

@id("diamond.disconnected")
fn disconnected() -> i64 { 0 }
"#,
                ),
            ]
        }

        fn rendered_entry() -> WorkspaceSemanticGraph {
            render_semantic_graph(authenticated_build().project("app.entry").unwrap())
                .expect("authenticated projection must render")
        }

        fn assert_ordered_fragments(document: &str, fragments: &[&str]) {
            let mut remainder = document;
            for fragment in fragments {
                let offset = remainder
                    .find(fragment)
                    .unwrap_or_else(|| panic!("missing ordered wire fragment `{fragment}`"));
                remainder = &remainder[offset + fragment.len()..];
            }
        }

        #[test]
        fn entry_projection_is_forward_provider_only_sorted_and_authenticated() {
            let projection = authenticated_build().project("app.entry").unwrap();
            assert_eq!(projection.workspace_revision(), "sha256:workspace");
            assert_eq!(projection.entry_module(), "app.entry");
            assert_eq!(
                projection
                    .modules()
                    .iter()
                    .map(WorkspaceGraphProjectionModule::path)
                    .collect::<Vec<_>>(),
                ["a/types.spx", "m/provider.spx", "z/entry.spx"]
            );
            assert!(projection
                .modules()
                .iter()
                .all(|module| module.source_graph_schema() == "semaprax.semantic-graph.v14"));
            assert!(projection
                .modules()
                .iter()
                .all(|module| module.source_revision().starts_with("revision:")));
            assert!(projection
                .modules()
                .iter()
                .all(|module| module.source_digest().starts_with("sha256:")));
            assert!(projection
                .declarations()
                .windows(2)
                .all(|pair| pair[0].id() < pair[1].id()));
            assert!(projection
                .declarations()
                .iter()
                .all(|fact| fact.path() != Some("q/disconnected.spx")));
            assert!(projection
                .shared_prelude_ids()
                .contains(&prelude::OPTION_ID));
            assert!(projection
                .edges()
                .iter()
                .all(|edge| edge.caller_path() != "q/disconnected.spx"));
            assert!(projection.edges().windows(2).all(|pair| pair[0] < pair[1]));

            let usage = projection.usage();
            assert_eq!(usage.used_managed_files(), 4);
            assert_eq!(usage.used_reachable_modules(), 3);
            assert_eq!(usage.used_manifest_bytes(), 711);
            assert_eq!(usage.used_retained_generations(), 3);
            assert_eq!(usage.used_staging_attempts(), 2);
            assert_eq!(usage.used_unexpected_inventory_entries(), 0);
            assert!(usage.used_total_source_bytes() > 0);
            assert!(usage.used_declarations() > 0);
            assert!(usage.used_callables() > 0);
            assert!(usage.used_call_sites() > 0);
            assert_eq!(usage.used_uses(), 3);
            assert!(usage.used_resolved_cross_file_edges() >= 3);
            assert!(usage.used_dependency_depth() >= 2);
            assert!(usage.used_builder_bytes() >= usage.used_total_source_bytes());
        }

        #[test]
        fn entry_need_not_define_main_and_reverse_consumers_are_excluded() {
            let projection = authenticated_build().project("types.core").unwrap();
            assert_eq!(projection.modules().len(), 1);
            assert_eq!(projection.modules()[0].module(), "types.core");
            assert!(projection.modules()[0]
                .functions()
                .iter()
                .all(|function| function.name != "main"));
            assert!(projection
                .declarations()
                .iter()
                .any(|fact| fact.id() == "types.point"));
            assert!(!projection
                .declarations()
                .iter()
                .any(|fact| fact.id() == "provider.run"));
        }

        #[test]
        fn entry_spelling_and_absence_have_frozen_diagnostics() {
            for entry in ["", ".app", "app.", "app-main", "App.µ"] {
                let error = authenticated_build()
                    .project(entry)
                    .err()
                    .expect("noncanonical entry must fail");
                assert_eq!(error[0].code, "SPX-G170");
                assert_eq!(
                    error[0].message,
                    format!("Workspace Semantic Graph entry module `{entry}` is not canonical")
                );
            }
            let error = authenticated_build()
                .project("missing.module")
                .err()
                .expect("absent canonical entry must fail");
            assert_eq!(error[0].code, "SPX-G172");
            assert_eq!(
                error[0].message,
                "Workspace Semantic Graph entry module `missing.module` is absent"
            );
        }

        #[test]
        fn diamond_projection_has_exact_provider_closure_order_and_full_usage() {
            let projection = authenticated_build_from(diamond_sources())
                .project("diamond.entry")
                .unwrap();
            assert_eq!(
                projection
                    .modules()
                    .iter()
                    .map(|module| (module.path(), module.dependency_depth()))
                    .collect::<Vec<_>>(),
                [
                    ("a/leaf.spx", 1),
                    ("b/left.spx", 2),
                    ("c/right.spx", 2),
                    ("z/entry.spx", 3),
                ]
            );
            assert_eq!(
                projection
                    .declarations()
                    .iter()
                    .filter(|fact| fact.origin() != hir::IdentityOrigin::CompilerOwned)
                    .map(WorkspaceGraphProjectionDeclaration::id)
                    .collect::<Vec<_>>(),
                [
                    "diamond.leaf",
                    "diamond.left",
                    "diamond.main",
                    "diamond.right",
                ]
            );
            assert_eq!(
                projection
                    .edges()
                    .iter()
                    .filter(|edge| edge.kind() == "function_import")
                    .map(|edge| (
                        edge.caller_path(),
                        edge.caller(),
                        edge.target_path(),
                        edge.target(),
                        edge.kind(),
                        edge.site(),
                        edge.expression(),
                        edge.ast_path(),
                        edge.alias(),
                        edge.ordinal(),
                    ))
                    .collect::<Vec<_>>(),
                [
                    (
                        "b/left.spx",
                        "diamond.left",
                        "a/leaf.spx",
                        "diamond.leaf",
                        "function_import",
                        "module",
                        "use.0",
                        "use.0",
                        "leaf",
                        0,
                    ),
                    (
                        "c/right.spx",
                        "diamond.right",
                        "a/leaf.spx",
                        "diamond.leaf",
                        "function_import",
                        "module",
                        "use.0",
                        "use.0",
                        "leaf",
                        0,
                    ),
                    (
                        "z/entry.spx",
                        "diamond.entry",
                        "b/left.spx",
                        "diamond.left",
                        "function_import",
                        "module",
                        "use.0",
                        "use.0",
                        "left",
                        0,
                    ),
                    (
                        "z/entry.spx",
                        "diamond.entry",
                        "c/right.spx",
                        "diamond.right",
                        "function_import",
                        "module",
                        "use.1",
                        "use.1",
                        "right",
                        1,
                    ),
                ]
            );
            assert!(projection.edges().windows(2).all(|pair| pair[0] < pair[1]));
            assert!(projection.modules().iter().any(|module| {
                module.module() == "diamond.entry"
                    && module
                        .functions()
                        .iter()
                        .any(|function| function.name == "main")
            }));
            assert!(projection.modules().iter().all(|module| {
                module.functions().iter().all(|function| {
                    !function
                        .id
                        .as_str()
                        .starts_with("workspace.synthetic.main.")
                })
            }));

            let leaf = authenticated_build_from(diamond_sources())
                .project("diamond.leaf")
                .unwrap();
            assert_eq!(leaf.modules().len(), 1);
            assert_eq!(leaf.modules()[0].path(), "a/leaf.spx");
            assert!(projection.modules().iter().all(|module| !matches!(
                module.module(),
                "diamond.reverse" | "diamond.disconnected"
            )));
            assert!(projection
                .declarations()
                .iter()
                .all(|fact| !matches!(fact.id(), "diamond.reverse" | "diamond.disconnected")));

            let mut full_usage = projection.usage();
            let mut leaf_usage = leaf.usage();
            assert_eq!(full_usage.used_managed_files(), 6);
            assert_eq!(full_usage.used_reachable_modules(), 4);
            assert_eq!(leaf_usage.used_reachable_modules(), 1);
            assert_eq!(full_usage.used_entry_module_bytes(), "diamond.entry".len());
            assert_eq!(leaf_usage.used_entry_module_bytes(), "diamond.leaf".len());
            full_usage.used_reachable_modules = 0;
            leaf_usage.used_reachable_modules = 0;
            full_usage.used_entry_module_bytes = 0;
            leaf_usage.used_entry_module_bytes = 0;
            assert_eq!(full_usage, leaf_usage);
        }

        #[test]
        fn projection_preserves_identity_ownership_once_without_stubs_or_synthetic_main() {
            let sources = vec![
                canonical_source(
                    "app/identity.spx",
                    r#"
module projection.identity;
use function @id("projection.answer") from projection.provider as answer;

@id("projection.record")
record Record {
    automatic: i64,
    @id("projection.record.explicit")
    explicit: i64,
}

fn helper() -> i64 { answer() }

@id("projection.main")
fn main() -> i64 { helper() }
"#,
                ),
                canonical_source(
                    "lib/provider.spx",
                    r#"
module projection.provider;

@id("projection.answer")
fn answer() -> i64 { 42 }
"#,
                ),
            ];
            let projection = authenticated_build_from(sources)
                .project("projection.identity")
                .unwrap();
            let assert_fact = |id: &str,
                               kind: hir::DeclarationKind,
                               origin: hir::IdentityOrigin,
                               owner: Option<&str>,
                               path: &str,
                               module: &str| {
                let fact = projection
                    .declarations()
                    .iter()
                    .find(|fact| fact.id() == id)
                    .unwrap_or_else(|| panic!("missing projected fact `{id}`"));
                assert_eq!(fact.kind(), kind, "kind for `{id}`");
                assert_eq!(fact.origin(), origin, "origin for `{id}`");
                assert_eq!(fact.owner(), owner, "owner for `{id}`");
                assert_eq!(fact.path(), Some(path), "path for `{id}`");
                assert_eq!(fact.module(), Some(module), "module for `{id}`");
            };
            assert_fact(
                "projection.record",
                hir::DeclarationKind::Record,
                hir::IdentityOrigin::Explicit,
                None,
                "app/identity.spx",
                "projection.identity",
            );
            assert_fact(
                "auto:field:projection.record.automatic",
                hir::DeclarationKind::Field,
                hir::IdentityOrigin::Automatic,
                Some("projection.record"),
                "app/identity.spx",
                "projection.identity",
            );
            assert_fact(
                "projection.record.explicit",
                hir::DeclarationKind::Field,
                hir::IdentityOrigin::Explicit,
                Some("projection.record"),
                "app/identity.spx",
                "projection.identity",
            );
            assert_fact(
                "auto:projection.identity.helper",
                hir::DeclarationKind::Function,
                hir::IdentityOrigin::Automatic,
                None,
                "app/identity.spx",
                "projection.identity",
            );
            assert_fact(
                "projection.main",
                hir::DeclarationKind::Function,
                hir::IdentityOrigin::Explicit,
                None,
                "app/identity.spx",
                "projection.identity",
            );
            assert_fact(
                "projection.answer",
                hir::DeclarationKind::Function,
                hir::IdentityOrigin::Explicit,
                None,
                "lib/provider.spx",
                "projection.provider",
            );

            let expected_prelude = prelude::all_ids().into_iter().collect::<BTreeSet<_>>();
            let projected_prelude = projection
                .shared_prelude_ids()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            assert_eq!(
                projection.shared_prelude_ids().len(),
                expected_prelude.len()
            );
            assert_eq!(projected_prelude, expected_prelude);
            let compiler_facts = projection
                .declarations()
                .iter()
                .filter(|fact| fact.origin() == hir::IdentityOrigin::CompilerOwned)
                .map(WorkspaceGraphProjectionDeclaration::id)
                .collect::<BTreeSet<_>>();
            assert_eq!(compiler_facts, projected_prelude);

            let functions = projection
                .modules()
                .iter()
                .map(|module| {
                    (
                        module.module(),
                        module
                            .functions()
                            .iter()
                            .map(|function| function.id.as_str())
                            .collect::<BTreeSet<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                functions["projection.identity"],
                BTreeSet::from(["auto:projection.identity.helper", "projection.main"])
            );
            assert_eq!(
                functions["projection.provider"],
                BTreeSet::from(["projection.answer"])
            );
            assert!(projection
                .declarations()
                .iter()
                .all(|fact| !fact.id().starts_with("workspace.synthetic.main.")));
        }

        #[test]
        fn mixed_graph_v10_through_v14_metadata_is_path_ordered_and_exact() {
            let sources = vec![
                canonical_source(
                    "e/v10.spx",
                    r#"
module graph.v10;
use function @id("graph.v11.run") from graph.v11 as next;
@id("graph.v10.run") fn run() -> i64 { next() }
"#,
                ),
                canonical_source(
                    "d/v11.spx",
                    r#"
module graph.v11;
use function @id("graph.v12.run") from graph.v12 as next;
@id("graph.v11.run") fn run() -> i64 { next() }
"#,
                ),
                canonical_source(
                    "c/v12.spx",
                    r#"
module graph.v12;
use function @id("graph.v13.run") from graph.v13 as next;
@id("graph.v12.run") fn run() -> i64 { next() }
"#,
                ),
                canonical_source(
                    "b/v13.spx",
                    r#"
module graph.v13;
use function @id("graph.v14.run") from graph.v14 as next;
@id("graph.v13.run") fn run() -> i64 { next() }
"#,
                ),
                canonical_source(
                    "a/v14.spx",
                    r#"
module graph.v14;
@id("graph.v14.run") fn run() -> i64 { 14 }
"#,
                ),
            ];
            let mut authenticated = authenticated_build_from(sources);
            for (path, schema) in [
                ("e/v10.spx", "semaprax.semantic-graph.v10"),
                ("d/v11.spx", "semaprax.semantic-graph.v11"),
                ("c/v12.spx", "semaprax.semantic-graph.v12"),
                ("b/v13.spx", "semaprax.semantic-graph.v13"),
                ("a/v14.spx", "semaprax.semantic-graph.v14"),
            ] {
                let fact = authenticated.sources.get_mut(path).unwrap();
                fact.source_graph_schema = schema.to_owned();
                fact.source_revision = format!("revision:{schema}");
                fact.source_digest = format!("digest:{schema}");
            }
            let projection = authenticated.project("graph.v10").unwrap();
            assert_eq!(
                projection
                    .modules()
                    .iter()
                    .map(|module| (
                        module.path(),
                        module.source_graph_schema(),
                        module.source_revision(),
                        module.source_digest(),
                    ))
                    .collect::<Vec<_>>(),
                [
                    (
                        "a/v14.spx",
                        "semaprax.semantic-graph.v14",
                        "revision:semaprax.semantic-graph.v14",
                        "digest:semaprax.semantic-graph.v14",
                    ),
                    (
                        "b/v13.spx",
                        "semaprax.semantic-graph.v13",
                        "revision:semaprax.semantic-graph.v13",
                        "digest:semaprax.semantic-graph.v13",
                    ),
                    (
                        "c/v12.spx",
                        "semaprax.semantic-graph.v12",
                        "revision:semaprax.semantic-graph.v12",
                        "digest:semaprax.semantic-graph.v12",
                    ),
                    (
                        "d/v11.spx",
                        "semaprax.semantic-graph.v11",
                        "revision:semaprax.semantic-graph.v11",
                        "digest:semaprax.semantic-graph.v11",
                    ),
                    (
                        "e/v10.spx",
                        "semaprax.semantic-graph.v10",
                        "revision:semaprax.semantic-graph.v10",
                        "digest:semaprax.semantic-graph.v10",
                    ),
                ]
            );
            assert_eq!(projection.usage().used_reachable_modules(), 5);
        }

        #[test]
        fn rendered_document_has_literal_sha_exact_wire_order_and_digest_binding() {
            let graph = rendered_entry();
            let json = graph.to_json();
            let document_sha = format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(json.as_bytes()))
            );
            assert_eq!(
                document_sha,
                "sha256:6639d985e25d4d33a72e37034c6e3f116940d3598bbf46162a6baaeb547da972"
            );
            assert!(json.starts_with(
                "{\"schema\":\"semaprax.workspace-semantic-graph.v1\",\"workspace_manifest_schema\":\"semaprax.workspace-semantic-manifest.v1\",\"workspace_revision\":\"sha256:workspace\",\"graph_digest\":\"sha256:"
            ));
            assert!(json.ends_with("\"]}"));
            assert_ordered_fragments(
                json,
                &[
                    "\"schema\":",
                    "\"workspace_manifest_schema\":",
                    "\"workspace_revision\":",
                    "\"graph_digest\":",
                    "\"entry\":",
                    "\"modules\":",
                    "\"declarations\":",
                    "\"edges\":",
                    "\"limits\":",
                    "\"budget\":",
                    "\"nonclaims\":",
                ],
            );
            let types = graph
                .modules()
                .iter()
                .find(|module| module.path() == "a/types.spx")
                .unwrap();
            assert!(json.contains(&format!(
                "{{\"path\":\"a/types.spx\",\"module\":\"types.core\",\"source_graph_schema\":\"semaprax.semantic-graph.v14\",\"source_revision\":\"revision:a/types.spx\",\"source_digest\":\"{}\",\"dependency_depth\":1,\"permits\":[]}}",
                types.source_digest()
            )));
            assert!(json.contains(
                "{\"id\":\"types.point\",\"kind\":\"record\",\"identity_origin\":\"explicit\",\"owner\":null,\"path\":\"a/types.spx\",\"module\":\"types.core\"}"
            ));
            assert!(json.contains(
                "{\"caller_path\":\"m/provider.spx\",\"caller\":\"provider.api\",\"target_path\":\"a/types.spx\",\"target\":\"types.point\",\"kind\":\"type_import\",\"site\":\"module\",\"expression\":\"use.0\",\"ast_path\":\"use.0\",\"alias\":\"Point\",\"ordinal\":0}"
            ));
            assert!(json.contains(
                "\"max_managed_files\":16,\"max_reachable_modules\":16,\"max_entry_module_bytes\":16777216,\"max_total_source_bytes\":16777216"
            ));
            assert!(json.contains(
                "\"used_managed_files\":4,\"used_reachable_modules\":3,\"used_entry_module_bytes\":9,\"used_total_source_bytes\":"
            ));

            let digest_field = format!(",\"graph_digest\":\"{}\"", graph.graph_digest());
            assert_eq!(json.matches(&digest_field).count(), 1);
            let payload = json.replacen(&digest_field, "", 1);
            assert_eq!(artifact_digest(payload.as_bytes()), graph.graph_digest());
            assert_eq!(graph.budget().used_output_bytes(), json.len());
        }

        #[test]
        fn rendering_replays_input_order_and_digest_binds_payload_mutation() {
            let sources = diamond_sources();
            let first = render_semantic_graph(
                authenticated_build_from(sources.clone())
                    .project("diamond.entry")
                    .unwrap(),
            )
            .unwrap();
            let mut reversed = sources;
            reversed.reverse();
            let second = render_semantic_graph(
                authenticated_build_from(reversed)
                    .project("diamond.entry")
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(first.graph_digest(), second.graph_digest());
            assert_eq!(first.to_json(), second.to_json());

            let json = first.to_json();
            let digest_field = format!(",\"graph_digest\":\"{}\"", first.graph_digest());
            let payload = json.replacen(&digest_field, "", 1);
            let mut mutated = payload.into_bytes();
            let offset = mutated
                .windows(b"diamond.entry".len())
                .position(|window| window == b"diamond.entry")
                .expect("entry module must occur in digest payload");
            mutated[offset] = b'D';
            assert_ne!(artifact_digest(&mutated), first.graph_digest());
        }

        #[test]
        fn rendering_output_cap_is_exact_and_returns_no_partial_artifact() {
            let used = rendered_entry().to_json().len();
            let exact = render_semantic_graph_with_output_limit(
                authenticated_build().project("app.entry").unwrap(),
                used,
            )
            .expect("the exact measured output limit must succeed");
            assert_eq!(exact.to_json().len(), used);
            assert_eq!(exact.budget().used_output_bytes(), used);

            let error = render_semantic_graph_with_output_limit(
                authenticated_build().project("app.entry").unwrap(),
                used - 1,
            )
            .err()
            .expect("one byte below the exact output limit must return no artifact");
            assert_eq!(error.len(), 1);
            assert_eq!(error[0].code, "SPX-G171");
            assert_eq!(
                error[0].message,
                format!(
                    "Workspace Semantic Graph `output_bytes` exceeds {}",
                    used - 1
                )
            );
        }

        #[test]
        fn rendered_wire_carries_exact_projection_closure_and_full_counters() {
            let graph = render_semantic_graph(
                authenticated_build_from(diamond_sources())
                    .project("diamond.entry")
                    .unwrap(),
            )
            .unwrap();
            let json = graph.to_json();
            let value: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(value["entry"]["module"], "diamond.entry");
            assert_eq!(value["entry"]["path"], "z/entry.spx");
            assert_eq!(
                value["modules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|module| module["path"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["a/leaf.spx", "b/left.spx", "c/right.spx", "z/entry.spx"]
            );
            assert!(value["modules"].as_array().unwrap().iter().all(|module| {
                !matches!(
                    module["module"].as_str().unwrap(),
                    "diamond.reverse" | "diamond.disconnected"
                )
            }));

            let usage = graph.budget();
            let budget = &value["budget"];
            for (key, expected) in [
                ("used_managed_files", usage.used_managed_files()),
                ("used_reachable_modules", usage.used_reachable_modules()),
                ("used_entry_module_bytes", usage.used_entry_module_bytes()),
                ("used_total_source_bytes", usage.used_total_source_bytes()),
                ("used_declarations", usage.used_declarations()),
                ("used_callables", usage.used_callables()),
                ("used_call_sites", usage.used_call_sites()),
                ("used_uses", usage.used_uses()),
                (
                    "used_resolved_cross_file_edges",
                    usage.used_resolved_cross_file_edges(),
                ),
                ("used_dependency_depth", usage.used_dependency_depth()),
                ("used_builder_bytes", usage.used_builder_bytes()),
                ("used_manifest_bytes", usage.used_manifest_bytes()),
                ("used_output_bytes", usage.used_output_bytes()),
                (
                    "used_retained_generations",
                    usage.used_retained_generations(),
                ),
                ("used_staging_attempts", usage.used_staging_attempts()),
                (
                    "used_unexpected_inventory_entries",
                    usage.used_unexpected_inventory_entries(),
                ),
            ] {
                assert_eq!(budget[key].as_u64(), Some(expected as u64), "{key}");
            }
            assert_eq!(usage.used_managed_files(), 6);
            assert_eq!(usage.used_reachable_modules(), 4);
            assert_eq!(usage.used_entry_module_bytes(), "diamond.entry".len());
            assert_eq!(
                value["nonclaims"].as_array().unwrap().len(),
                NONCLAIMS.len()
            );
        }

        #[test]
        fn entry_module_byte_limit_precedes_grammar_and_never_echoes_oversize_input() {
            let exact = "a".repeat(MAX_ENTRY_MODULE_BYTES);
            validate_entry_module(&exact).expect("the exact entry-module byte limit is admitted");

            for oversized in [
                format!("{}x", exact),
                format!("-private-sentinel-{}", "x".repeat(MAX_ENTRY_MODULE_BYTES)),
            ] {
                let error = validate_entry_module(&oversized)
                    .expect_err("one-over entry modules must fail before grammar or lookup");
                assert_eq!(error.len(), 1);
                assert_eq!(error[0].code, "SPX-G171");
                assert_eq!(
                    error[0].message,
                    "Workspace Semantic Graph `entry_module_bytes` exceeds 16777216"
                );
                assert!(!error[0].message.contains("private-sentinel"));
                assert!(!error[0].message.contains(&oversized));
            }
        }

        #[test]
        fn renderer_escapes_authenticated_strings_canonically() {
            let escaped = "revision:\"quoted\"\\slash\nline\rreturn\ttab\u{0001}";
            let mut authenticated = authenticated_build();
            authenticated.workspace_revision = escaped.to_owned();
            authenticated
                .sources
                .get_mut("z/entry.spx")
                .unwrap()
                .source_revision = escaped.to_owned();
            let graph = render_semantic_graph(authenticated.project("app.entry").unwrap()).unwrap();
            let json = graph.to_json();
            assert!(json.contains(
                "\"workspace_revision\":\"revision:\\\"quoted\\\"\\\\slash\\nline\\rreturn\\ttab\\u0001\""
            ));
            assert!(json.contains(
                "\"source_revision\":\"revision:\\\"quoted\\\"\\\\slash\\nline\\rreturn\\ttab\\u0001\""
            ));
            let value: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(value["workspace_revision"], escaped);
            assert_eq!(value["modules"][2]["source_revision"], escaped);
        }

        #[test]
        fn measured_builder_usage_is_the_exact_minimum_successful_limit() {
            let sources = projection_sources();
            let build = build_owned(sources.clone()).unwrap();
            let used = build.usage.builder_bytes;
            let exact = build_owned_with_builder_limit(sources.clone(), used).unwrap();
            assert_eq!(exact.usage.builder_bytes, used);
            let error = build_owned_with_builder_limit(sources, used - 1)
                .err()
                .expect("one byte below measured sequential peak must fail");
            assert_eq!(error[0].code, "SPX-G171");
            assert_eq!(
                error[0].message,
                format!(
                    "Workspace Semantic Graph `builder_bytes` exceeds {}",
                    used - 1
                )
            );
        }

        #[test]
        fn real_managed_renderer_matches_the_production_shared_hook_path() {
            let fixture = ManagedFixture::new("semantic-graph-success");
            let production = snapshot(&fixture.root, "workspace.alpha")
                .expect("production managed rendering must succeed");
            let hook_called = Cell::new(false);
            let through_hook = build_authenticated_semantic_graph_with_hook(
                &fixture.root,
                "workspace.alpha",
                |rendered| {
                    hook_called.set(true);
                    assert_eq!(rendered.schema(), WORKSPACE_GRAPH_SCHEMA);
                    assert_eq!(rendered.workspace_revision(), fixture.revision);
                    assert_eq!(rendered.entry().module(), "workspace.alpha");
                    assert_eq!(rendered.entry().path(), "alpha.spx");
                    assert_eq!(rendered.budget().used_managed_files(), 2);
                    assert_eq!(rendered.budget().used_reachable_modules(), 1);
                },
            )
            .expect("unchanged shared-hook rendering must succeed");
            assert!(hook_called.get());
            assert_eq!(through_hook.graph_digest(), production.graph_digest());
            assert_eq!(through_hook.to_json(), production.to_json());
            assert_eq!(
                through_hook
                    .modules()
                    .iter()
                    .map(WorkspaceSemanticGraphModule::path)
                    .collect::<Vec<_>>(),
                ["alpha.spx"]
            );
            assert!(through_hook
                .declarations()
                .iter()
                .all(|declaration| declaration.path() != Some("beta.spx")));
            fixture.reacquire_exclusive();
        }

        #[test]
        fn after_render_mutation_discards_graph_rechecks_and_releases_lock() {
            let fixture = ManagedFixture::new("semantic-graph-final-recheck");
            let hook_called = Cell::new(false);
            let result = build_authenticated_semantic_graph_with_hook(
                &fixture.root,
                "workspace.alpha",
                |rendered| {
                    hook_called.set(true);
                    assert!(!rendered.to_json().is_empty());
                    append_byte(&fixture.control().join("ACTIVE"));
                },
            );
            assert!(
                hook_called.get(),
                "mutation must occur only after full rendering"
            );
            let error = result
                .err()
                .expect("the final recheck must return no semantic graph artifact");
            assert_eq!(error.len(), 1);
            assert_eq!(error[0].code, "SPX-G153");
            assert_eq!(
                error[0].message,
                "workspace object changed during authentication"
            );
            fixture.reacquire_exclusive();
        }

        #[test]
        fn final_projection_boundary_rechecks_owned_workspace_and_releases_lock() {
            for mutation in ["active", "manifest", "source", "inventory"] {
                let fixture = ManagedFixture::new(mutation);
                let error = build_authenticated_projection_with_hook(
                    &fixture.root,
                    "workspace.alpha",
                    || match mutation {
                        "active" => append_byte(&fixture.control().join("ACTIVE")),
                        "manifest" => append_byte(&fixture.generation().join("manifest.json")),
                        "source" => append_byte(&fixture.generation().join("files/alpha.spx")),
                        "inventory" => {
                            std::fs::create_dir(fixture.control().join("staging/0")).unwrap();
                        }
                        _ => unreachable!(),
                    },
                )
                .err()
                .expect("final authenticated boundary must reject mutation");
                assert!(
                    matches!(error[0].code, "SPX-G152" | "SPX-I209" | "SPX-G153"),
                    "unexpected underlying Workspace diagnostic: {:?}",
                    error[0]
                );
                fixture.reacquire_exclusive();
            }
        }

        #[cfg(unix)]
        #[test]
        fn final_projection_boundary_rejects_generation_identity_replacement() {
            let fixture = ManagedFixture::new("generation");
            let generation = fixture.generation();
            let displaced = fixture.control().join("generations/displaced");
            let error =
                build_authenticated_projection_with_hook(&fixture.root, "workspace.alpha", || {
                    std::fs::rename(&generation, &displaced).unwrap();
                    std::fs::create_dir(&generation).unwrap();
                })
                .err()
                .expect("generation identity replacement must fail");
            assert!(matches!(error[0].code, "SPX-I209" | "SPX-G153"));
            fixture.reacquire_exclusive();
        }

        fn append_byte(path: &Path) {
            use std::io::Write as _;

            let mut file = OpenOptions::new().append(true).open(path).unwrap();
            file.write_all(b" ").unwrap();
            file.sync_all().unwrap();
        }
    }

    #[test]
    fn imported_type_classification_is_per_consumer_module() {
        let app = canonical_source(
            "app/main.spx",
            r#"
module app.main;
use type @id("lib.point") from lib.core as RemotePoint;

@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let build = build_owned(vec![app, point_provider()]).unwrap();
        assert!(build.edges.iter().all(|edge| {
            edge.kind != "type_reference"
                || edge.caller_path != "lib/core.spx"
                || edge.target != "lib.point"
        }));
    }

    #[test]
    fn explicit_body_pattern_and_nested_type_carrier_sites_replay() {
        let app = canonical_source(
            "app/main.spx",
            r#"
module app.main;
use type @id("lib.point") from lib.core as RemotePoint;

@id("app.wrapper")
record Wrapper {
    @id("app.wrapper.point")
    point: RemotePoint,
}

@id("app.read")
fn read(value: Wrapper) -> i64 {
    match value { Wrapper { point: RemotePoint { x } } => x, }
}

@id("app.main")
fn main() -> i64 {
    let value = Wrapper { point: RemotePoint { x: 7 } };
    read(value)
}
"#,
        );
        let build = build_owned(vec![app, point_provider()]).unwrap();
        let paths = build
            .edges
            .iter()
            .filter(|edge| edge.kind == "type_reference" && edge.target == "lib.point")
            .map(|edge| (edge.caller.as_str(), edge.ast_path.as_str()))
            .collect::<BTreeSet<_>>();
        assert!(paths.contains(&("app.wrapper", "type.app.wrapper.field.0")));
        assert!(paths.contains(&("app.read", "body.tail.arm.0.pattern.field.0.pattern")));
        assert!(paths.contains(&("app.main", "body.s0.value.field.0.value.type")));
    }

    #[test]
    fn transitive_exposed_type_requires_direct_consumer_import() {
        let leaf = canonical_source(
            "a/point.spx",
            r#"
module a.point;

@id("a.point")
record Point { @id("a.point.x") x: i64, }

@id("a.local")
fn local() -> i64 { 0 }
"#,
        );
        let wrapper = canonical_source(
            "b/wrapper.spx",
            r#"
module b.wrapper;
use type @id("a.point") from a.point as InnerPoint;

@id("b.wrapper")
record Wrapper { @id("b.wrapper.value") value: InnerPoint, }

@id("b.local")
fn local() -> i64 { 0 }
"#,
        );
        let missing = canonical_source(
            "c/main.spx",
            r#"
module c.main;
use type @id("b.wrapper") from b.wrapper as Wrapper;

@id("c.main")
fn main() -> i64 { 0 }
"#,
        );
        let error = build_owned(vec![leaf.clone(), wrapper.clone(), missing])
            .err()
            .expect("transitive type authority must be direct");
        assert_eq!(error[0].code, "SPX-G172");

        let present = canonical_source(
            "c/main.spx",
            r#"
module c.main;
use type @id("b.wrapper") from b.wrapper as Wrapper;
use type @id("a.point") from a.point as DirectPoint;

@id("c.main")
fn main() -> i64 { 0 }
"#,
        );
        build_owned(vec![leaf, wrapper, present]).unwrap();
    }

    #[test]
    fn interface_import_carriers_reject_workspace_type_aliases_pre_hir() {
        let app = canonical_source(
            "app/main.spx",
            r#"
module app.main;
use type @id("lib.point") from lib.core as RemotePoint;

@id("app.host")
interface Host permits {} {
    @id("app.host.consume")
    import fn consume(value: own RemotePoint) -> unit
        effects {}
        failure infallible
        consumes value always;
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let error = build_owned(vec![app, point_provider()])
            .err()
            .expect("interface carrier must reject workspace alias");
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(
            error[0].message,
            "workspace type aliases are not admitted in interface/import parameter carriers"
        );
    }
}
