//! Invocation-owned exact-source frontend cache, with opt-in checked module reuse.
//! AST/HIR entries are compiler-created or restored through the separate
//! authenticated store; no public raw-HIR constructor exists.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::ast::program_source::{checked_key::CheckedSource, ProgramSource};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;
use crate::workspace_graph::WorkspaceSource;

use super::{ProjectManifest, ProjectRevision, MAX_PATH_BYTES, MAX_TOTAL_SOURCE_BYTES};

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod snapshot;
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
pub(crate) use snapshot::{decode_snapshot, encode_snapshot};

pub(crate) mod checked_hir;
/// Read-only replay measuring `FrontendPass::lookup`'s `Program::clone` cost
/// (SPX-AI-031/#130, SPX-AI-032/#131). See that module's doc comment.
mod clone_cost_report;
pub(crate) mod paid_resolution;
mod source_handoff;
use checked_hir::{CheckedModuleLease, ResolvedModuleInput};

pub const PROJECT_FRONTEND_CACHE_SCHEMA: &str = "semaprax.project-frontend-cache-work.v1";
pub const PROJECT_FRONTEND_CACHE_COMPATIBILITY: &str = "semaprax.project-frontend-canonical-ast.v3";
pub const PROJECT_SEMANTIC_CACHE_SCHEMA: &str = "semaprax.project-semantic-cache-work.v1";
/// v11 retains scoped field source forms after v10 CopyDiscard and v9 row loans.
/// Older cached modules must be freshly source-checked, never reinterpreted.
pub const PROJECT_SEMANTIC_CACHE_COMPATIBILITY: &str = "semaprax.project-checked-module-hir.v12";
pub const MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES: usize = MAX_TOTAL_SOURCE_BYTES;
/// The frontend retains the complete admitted semantic module closure, which
/// includes bundled/dependency modules beyond the manifest's sixteen authored
/// source slots. Keep this aligned with the ordinary Workspace Semantic Graph.
pub const MAX_PROJECT_FRONTEND_CACHE_MODULES: usize = crate::workspace_graph::MAX_FILES;
/// Synthetic imported stubs make the retained AST larger than authored source.
/// Share the graph builder's established construction ceiling so the complete
/// 32-module closure can be cached without creating an unbounded lane.
pub const MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET: usize = crate::workspace_graph::MAX_BUILDER_BYTES;
/// Keep checked-cache construction admission aligned with the ordinary
/// Workspace Semantic Graph builder instead of imposing a smaller independent
/// ceiling over the cache's retained-work estimate.
pub const MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND: usize =
    crate::workspace_graph::MAX_BUILDER_BYTES;
pub const MAX_PROJECT_CHECKED_FUNCTIONS: usize = 8192;
pub const MAX_PROJECT_FRONTEND_REPORT_BYTES: usize = 65_536;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Caller-owned canonical source proposal; construction is not admission.
pub struct ProjectFrontendSource {
    path: String,
    source: String,
}
impl ProjectFrontendSource {
    pub fn new(path: &str, source: &str) -> Result<Self> {
        if path.is_empty() || path.len() > MAX_PATH_BYTES || source.len() > MAX_TOTAL_SOURCE_BYTES {
            return Err(capacity(
                "frontend source exceeds its path or source byte bound",
            ));
        }
        Ok(Self {
            path: path.to_owned(),
            source: source.to_owned(),
        })
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn source(&self) -> &str {
        &self.source
    }
}

struct CheckedModule {
    synthetic: CheckedSource,
    resolved: crate::hir::ResolvedProgram,
    resolver_bytes: usize,
    retained_loan_bytes: usize,
    function_costs: BTreeMap<String, usize>,
}

/// A bounded in-memory cache belonging to its caller. It has no filesystem
/// root, serde constructor, global singleton, or checked-HIR bypass.
pub struct ProjectFrontendCache {
    context: String,
    entries: source_directory::SourceDirectory,
    semantic: bool,
    checked: BTreeMap<String, Arc<CheckedModule>>,
    restore_work: Option<String>,
}

pub struct ProjectFrontendBuild {
    revision: Arc<ProjectRevision>,
    json: String,
}
impl ProjectFrontendBuild {
    pub fn revision(&self) -> &Arc<ProjectRevision> {
        &self.revision
    }
    pub fn into_revision(self) -> Arc<ProjectRevision> {
        self.revision
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

impl Default for ProjectFrontendCache {
    fn default() -> Self {
        Self::new()
    }
}
impl ProjectFrontendCache {
    pub fn new() -> Self {
        Self {
            context: String::new(),
            entries: source_directory::SourceDirectory::default(),
            semantic: false,
            checked: BTreeMap::new(),
            restore_work: None,
        }
    }

    /// Reuse only compiler-created checked modules whose complete synthetic
    /// AST (including imported stubs) remains exactly equal. No deserialization.
    pub fn new_with_semantic_cache() -> Self {
        Self {
            semantic: true,
            ..Self::new()
        }
    }

    pub fn is_semantic_cache_enabled(&self) -> bool {
        self.semantic
    }

    /// Actual full Project replay work from the last authenticated persistent
    /// load. A subsequent successful build clears this historical report.
    pub fn restored_work(&self) -> Option<&str> {
        self.restore_work.as_deref()
    }

    /// Validate exact source inputs and completely link/admit the Project.
    /// Explicit semantic mode may reuse previously checked module resolution.
    /// Cache changes commit only after the entire result/report succeeds.
    pub fn build(
        &mut self,
        manifest: &ProjectManifest,
        sources: &[ProjectFrontendSource],
    ) -> Result<ProjectFrontendBuild> {
        self.build_indexed_rust(manifest, sources, &[])
    }

    /// Bind exact source-bound prepared Rust signatures before ordinary
    /// Project verification. Metadata changes invalidate both AST/HIR caches.
    pub fn build_indexed_rust(
        &mut self,
        manifest: &ProjectManifest,
        sources: &[ProjectFrontendSource],
        selections: &[super::ProjectIndexedRustImport],
    ) -> Result<ProjectFrontendBuild> {
        let indexed_identity = super::indexed_rust::validate(selections, sources)?;
        // Live Project loads supply authored files; stored entries also retain
        // compiler-bundled dependencies. Form the same current closure before
        // invalidation, otherwise absent dependency keys lose their source
        // entries and cannot pass checked_module's exact association replay.
        if sources.len() > MAX_PROJECT_FRONTEND_CACHE_MODULES {
            return Err(capacity(
                "frontend source inventory exceeds its module bound",
            ));
        }
        let mut input_bytes = 0usize;
        for source in sources {
            input_bytes = input_bytes
                .checked_add(source.source.len())
                .ok_or_else(|| capacity("frontend source accounting overflow"))?;
            if input_bytes > MAX_TOTAL_SOURCE_BYTES {
                return Err(capacity("frontend source inventory exceeds its byte bound"));
            }
        }
        let mut complete = sources
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source.path.clone(),
                source: source.source.clone(),
            })
            .collect::<Vec<_>>();
        super::standard_dependencies::extend_sources(manifest, &mut complete)?;
        let sources = complete.as_slice();
        if sources.len() > MAX_PROJECT_FRONTEND_CACHE_MODULES {
            return Err(capacity(
                "frontend source inventory exceeds its module bound",
            ));
        }
        let mut source_bytes = 0usize;
        let mut current = BTreeMap::new();
        for source in sources {
            source_bytes = source_bytes
                .checked_add(source.source.len())
                .ok_or_else(|| capacity("frontend source accounting overflow"))?;
            if source_bytes > MAX_TOTAL_SOURCE_BYTES {
                return Err(capacity("frontend source inventory exceeds its byte bound"));
            }
            if current
                .insert(source.path.as_str(), source.source.as_str())
                .is_some()
            {
                return Err(invalid("frontend sources contain duplicate paths"));
            }
        }
        let mut context = format!(
            "{}\0{}\0{}\0{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            if self.semantic {
                PROJECT_SEMANTIC_CACHE_COMPATIBILITY
            } else {
                PROJECT_FRONTEND_CACHE_COMPATIBILITY
            },
            manifest.to_canonical_toml()
        );
        if !selections.is_empty() {
            context.push_str(&indexed_identity);
        }
        let reset = context != self.context;
        let mut invalidated = BTreeSet::new();
        for (path, entry) in &self.entries {
            if reset || current.get(path.as_str()).copied() != Some(entry.input().source.as_str()) {
                invalidated.insert(path.clone());
            }
        }
        for path in current.keys() {
            if reset || !self.entries.contains_key(path) {
                invalidated.insert((*path).to_owned());
            }
        }
        // #130/#131: this cache retains only the per-file parsed/canonical
        // `Program` (`CachedModule`), never a cross-module fact. A provider's
        // change -- signature or private body -- is therefore never a reason
        // to evict an unrelated consumer's own unchanged-text entry here:
        //
        // 1. Parsing and canonicalizing one file is a pure function of that
        //    file's own bytes; `crate::parse_with_comments` and the canonical
        //    formatter never consult another module. So a consumer whose text
        //    is unchanged reparses to a bit-identical `Program` regardless of
        //    what any provider did -- there is no "stale AST" a provider edit
        //    could produce here to hide.
        // 2. `lookup` below re-checks `entry.input().source == source` itself before
        //    ever returning a cached `Program`, so this set only has to be
        //    exactly right about *this file's own* text; it is not trusted to
        //    know whether reuse is safe on its own.
        // 3. Every check that actually depends on cross-module content --
        //    `static_protocol::validate` (per file, unconditional either way),
        //    the workspace-wide import-stub-vs-authority validation, and the
        //    checked-HIR cache's own `synthetic != entry.synthetic` gate in
        //    `checked_module` -- reruns on *this build's* current `Program`
        //    values (cached or freshly parsed, which are identical per #1)
        //    and independently fails closed if a provider's exported surface
        //    changed. Narrowing this set cannot let a stale admission through
        //    those gates; it only decides whether this file's own frontend
        //    entry is worth keeping around to skip a redundant reparse.
        //
        // A provider's own path is still invalidated above like any other
        // changed file; only the transitive reverse-import propagation onto
        // *unchanged* consumers is not needed for AST-cache soundness and is
        // intentionally not computed. See
        // `clone_cost_report::provider_edit_reuses_unaffected_consumers_ast`
        // and `narrowing_disabled_would_leak_a_stale_ast_clone` for the
        // fault-injected negative control proving a change that truly matters
        // (the provider's own text) still invalidates.
        let retained: BTreeMap<_, _> = self
            .entries
            .iter()
            .filter(|(path, _)| !reset && !invalidated.contains(*path))
            .map(|(path, entry)| (path.clone(), Arc::clone(entry)))
            .collect();
        let mut pass = FrontendPass {
            indexed_imports: selections.to_vec(),
            entries: retained.into(),
            parsed: 0,
            reused: 0,
            canonicalizations: 0,
            parsed_bytes: 0,
            reused_bytes: 0,
            resolved: 0,
            retained_source_bytes: 0,
            ast_budget: 0,
            semantic: self.semantic,
            // Old entries remain only as private reuse candidates. Exact
            // reconstructed synthetic input still governs whole-module hits;
            // the narrower function seam separately authenticates its complete
            // environment. Failed admission never installs either candidate.
            checked: if reset {
                BTreeMap::new()
            } else {
                self.checked
                    .iter()
                    .map(|(path, entry)| (path.clone(), Arc::clone(entry)))
                    .collect()
            },
            next_checked: BTreeMap::new(),
            checked_reused: 0,
            functions_resolved: 0,
            functions_reused: 0,
        };
        // All borrowed current-source guards have completed. Transfer their
        // exact owned closure, preserving order and bytes without another copy.
        drop(current);
        let owned = source_handoff::consume(complete);
        let built = super::build::build_owned_with_frontend(manifest, owned, &mut pass)?;
        let revision = Arc::new(ProjectRevision::from_built(manifest.clone(), built));
        let json=super::image::render(json!({"schema":if self.semantic {PROJECT_SEMANTIC_CACHE_SCHEMA}else{PROJECT_FRONTEND_CACHE_SCHEMA},
            "compiler":{"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":if self.semantic {PROJECT_SEMANTIC_CACHE_COMPATIBILITY}else{PROJECT_FRONTEND_CACHE_COMPATIBILITY},"binary_identity_claimed":false},
            "context_digest":context_digest(context.as_bytes()),"project_revision":revision.project_revision(),
            "manifest_context_reset":reset,"invalidated_sources":invalidated,
            "work":{"modules_parsed":pass.parsed,"modules_reused":pass.reused,"canonicalizer_calls":pass.canonicalizations,
                "parsed_source_bytes":pass.parsed_bytes,"reused_source_bytes":pass.reused_bytes,"cached_AST_clones":pass.reused,
                "modules_resolved":pass.resolved,"checked_HIR_reused":pass.checked_reused,
                "monomorphic_functions_resolved":pass.functions_resolved,"monomorphic_function_HIR_reused":pass.functions_reused,
                "selective_function_HIR_resolution":self.semantic,"full_source_verification":true,"full_HIR_validation":true,
                "full_cross_file_checks":true,"full_link_and_profile_admission":true},
            "retained":{"modules":pass.entries.len(),"source_bytes":pass.retained_source_bytes,"AST_construction_prebound":pass.ast_budget},
            "limits":{"modules":MAX_PROJECT_FRONTEND_CACHE_MODULES,"source_bytes":MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES,"AST_construction_prebound":MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET,"checked_monomorphic_functions":MAX_PROJECT_CHECKED_FUNCTIONS},
            "nonclaims":if self.semantic {vec!["function_reuse_requires_exact_monomorphic_environment","no_cross_file_check_or_profile_bypass","no_implicit_persistence_or_ambient_cache","no_untrusted_HIR_deserialization","not_allocator_or_RSS_accounting","no_source_or_execution_authority"]}else{vec!["not_incremental_semantic_verification","no_checked_HIR_reuse","no_persistent_or_cross_process_cache","not_allocator_or_RSS_accounting","no_source_or_execution_authority"]}
        }),true,MAX_PROJECT_FRONTEND_REPORT_BYTES).map_err(|_|capacity("frontend work report exceeds its byte bound"))?;
        self.context = context;
        self.entries = pass.entries;
        self.checked = pass.next_checked;
        self.restore_work = None;
        Ok(ProjectFrontendBuild { revision, json })
    }

    pub(crate) fn fork(&self) -> Self {
        Self {
            context: self.context.clone(),
            entries: self.entries.clone(),
            semantic: self.semantic,
            checked: self.checked.clone(),
            restore_work: self.restore_work.clone(),
        }
    }

    /// Consume source bytes already read through the Project filesystem authority.
    /// This only changes the frontend work strategy, never source admission.
    pub(super) fn build_authenticated_sources(
        &mut self,
        manifest: &ProjectManifest,
        sources: Vec<SemanticWorkspaceSource>,
    ) -> Result<ProjectFrontendBuild> {
        let sources = sources
            .into_iter()
            .map(|source| ProjectFrontendSource {
                path: source.path,
                source: source.source,
            })
            .collect::<Vec<_>>();
        self.build(manifest, &sources)
    }
}

/// Private compiler seam. Only workspace_graph can fill it with freshly parsed
/// canonical ASTs; the public cache commits it after complete Project admission.
pub(crate) struct FrontendPass {
    pub(crate) indexed_imports: Vec<super::ProjectIndexedRustImport>,
    entries: source_directory::SourceDirectory,
    parsed: usize,
    reused: usize,
    canonicalizations: usize,
    parsed_bytes: usize,
    reused_bytes: usize,
    resolved: usize,
    retained_source_bytes: usize,
    ast_budget: usize,
    semantic: bool,
    checked: BTreeMap<String, Arc<CheckedModule>>,
    next_checked: BTreeMap<String, Arc<CheckedModule>>,
    checked_reused: usize,
    functions_resolved: usize,
    functions_reused: usize,
}

/// Core construction has not published anything yet. Only scalar telemetry is
/// checkpointed; staged checked modules start empty and are dropped on refusal.
pub(crate) struct CoreAttemptCheckpoint {
    resolved: usize,
    checked_reused: usize,
    functions_resolved: usize,
    functions_reused: usize,
}

pub(crate) struct SelectiveResolvedModule {
    pub(crate) resolved: crate::hir::ResolvedProgram,
    pub(crate) function_costs: BTreeMap<String, usize>,
    pub(crate) reused_functions: usize,
    pub(crate) static_text: bool,
}
impl SelectiveResolvedModule {
    pub(crate) fn into_parts(
        self,
    ) -> (
        crate::hir::ResolvedProgram,
        BTreeMap<String, usize>,
        usize,
        bool,
    ) {
        (
            self.resolved,
            self.function_costs,
            self.reused_functions,
            self.static_text,
        )
    }
}
impl FrontendPass {
    /// Cached replay must pay at least its recorded resolver bill: validation
    /// plus the original residual equals that bill. Loan-capacity residuals and
    /// newly shared prelude work still use their original later reservations.
    /// This read-only lower bound grants no storage or validation authority.
    pub(crate) fn checked_reuse_fits_active_attempt(&self, path: &str) -> bool {
        self.checked
            .get(path)
            .is_none_or(|entry| crate::bounded_output::active_required_fits(entry.resolver_bytes))
    }

    pub(crate) fn retains_checked_hir(&self) -> bool {
        self.semantic
    }

    pub(crate) fn checkpoint_core_attempt(&self) -> Option<CoreAttemptCheckpoint> {
        if !self.next_checked.is_empty() {
            return None;
        }
        Some(CoreAttemptCheckpoint {
            resolved: self.resolved,
            checked_reused: self.checked_reused,
            functions_resolved: self.functions_resolved,
            functions_reused: self.functions_reused,
        })
    }

    pub(crate) fn rollback_core_attempt(&mut self, checkpoint: CoreAttemptCheckpoint) {
        self.next_checked.clear();
        self.resolved = checkpoint.resolved;
        self.checked_reused = checkpoint.checked_reused;
        self.functions_resolved = checkpoint.functions_resolved;
        self.functions_reused = checkpoint.functions_reused;
    }

    pub(crate) fn checked_retention_prebound(&self, bytes: usize) -> Result<()> {
        if self.semantic && bytes > MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND {
            return Err(capacity(format!(
                "checked module cache synthetic AST/HIR construction prebound exceeds {MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND} bytes; this is the checked-module cache limit, separate from Workspace Semantic Graph builder_bytes"
            )));
        }
        Ok(())
    }
    pub(crate) fn checked_module(
        &mut self,
        path: &str,
        synthetic: &Program,
    ) -> Option<(CheckedModuleLease, usize, usize)> {
        self.checked_module_source(path, synthetic)
    }

    pub(crate) fn checked_module_source(
        &mut self,
        path: &str,
        synthetic: &dyn crate::ast::program_source::ProgramSource,
    ) -> Option<(CheckedModuleLease, usize, usize)> {
        if !self.semantic {
            return None;
        }
        let entry = self.checked.get(path)?;
        let previous = entry.synthetic.ready()?;
        if !checked_source_matches(synthetic, &previous) {
            return None;
        }
        crate::hir::replay_agent_source_associations(
            self.entries.get(path)?.program(),
            &entry.resolved.agents,
        )
        .ok()?;
        self.checked_reused += 1;
        self.functions_reused += entry.resolved.functions.len();
        self.next_checked.insert(path.to_owned(), Arc::clone(entry));
        Some((
            CheckedModuleLease::new(Arc::clone(entry)),
            entry.resolver_bytes,
            entry.retained_loan_bytes,
        ))
    }

    /// Only a genuinely first checked resolution may use a borrowed source.
    /// Existing candidates retain exact owned environment/body-reuse checks.
    pub(crate) fn permits_borrowed_cold_source(&self, path: &str) -> bool {
        self.semantic && !self.checked.contains_key(path)
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "legacy unpaid selective route; workspace uses the explicit-owner sibling"
        )
    )]
    pub(crate) fn resolve_borrowed_cold_source<'p>(
        &self,
        path: &str,
        synthetic: &'p dyn crate::ast::program_source::ProgramSource,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
    ) -> Option<Result<SelectiveResolvedModule>> {
        if !self.permits_borrowed_cold_source(path) {
            return None;
        }
        // Preserve the exact checked-function limit even without a prior key.
        if synthetic.functions().len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(capacity(
                "checked function inventory exceeds its bound",
            )));
        }
        Some(
            crate::hir::resolve_source(synthetic, private).map(|(resolved, work)| {
                SelectiveResolvedModule {
                    reused_functions: work.reused(),
                    static_text: work.static_text(),
                    function_costs: work.costs(),
                    resolved,
                }
            }),
        )
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "legacy unpaid selective route; workspace uses the explicit-owner sibling"
        )
    )]
    pub(crate) fn resolve_functions<'p>(
        &self,
        path: &str,
        synthetic: &'p Program,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
    ) -> Option<Result<SelectiveResolvedModule>> {
        if !self.semantic {
            return None;
        }
        if synthetic.functions.len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(capacity(
                "checked function inventory exceeds its bound",
            )));
        }
        let entry = self.checked.get(path);
        Some(
            crate::hir::resolve_with_function_reuse(
                synthetic,
                entry.and_then(|entry| entry.synthetic.owned()),
                entry.map(|entry| &entry.resolved),
                entry.map(|entry| &entry.function_costs),
                private,
            )
            .map(|(resolved, work)| SelectiveResolvedModule {
                reused_functions: work.reused(),
                static_text: work.static_text(),
                function_costs: work.costs(),
                resolved,
            }),
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolved_module(
        &mut self,
        path: &str,
        synthetic: Program,
        resolved: crate::hir::ResolvedProgram,
        resolver_bytes: usize,
        retained_loan_bytes: usize,
        function_costs: BTreeMap<String, usize>,
        reused_functions: usize,
    ) -> ResolvedModuleInput {
        self.resolved += 1;
        self.functions_reused += reused_functions;
        self.functions_resolved += resolved.functions.len().saturating_sub(reused_functions);
        if self.semantic {
            let owner = Arc::new(CheckedModule {
                synthetic: CheckedSource::Owned(synthetic),
                resolved,
                resolver_bytes,
                retained_loan_bytes,
                function_costs,
            });
            self.next_checked
                .insert(path.to_owned(), Arc::clone(&owner));
            ResolvedModuleInput::Checked(CheckedModuleLease::new(owner))
        } else {
            ResolvedModuleInput::Owned(resolved)
        }
    }
    /// Obtain an owning key only for actual semantic publication. The producer
    /// closure is dropped without execution for an AST-only receiving pass.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolved_module_receiving(
        &mut self,
        path: &str,
        key: impl FnOnce() -> Result<Program>,
        resolved: crate::hir::ResolvedProgram,
        resolver_bytes: usize,
        retained_loan_bytes: usize,
        function_costs: BTreeMap<String, usize>,
        reused_functions: usize,
    ) -> Result<ResolvedModuleInput> {
        if self.semantic {
            Ok(self.resolved_module(
                path,
                key()?,
                resolved,
                resolver_bytes,
                retained_loan_bytes,
                function_costs,
                reused_functions,
            ))
        } else {
            Ok(self.resolved_module_without_key(resolved, reused_functions))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolved_module_split_key(
        &mut self,
        path: &str,
        synthetic: CheckedSource,
        resolved: crate::hir::ResolvedProgram,
        resolver_bytes: usize,
        retained_loan_bytes: usize,
        function_costs: BTreeMap<String, usize>,
        reused_functions: usize,
    ) -> ResolvedModuleInput {
        self.resolved += 1;
        self.functions_reused += reused_functions;
        self.functions_resolved += resolved.functions.len().saturating_sub(reused_functions);
        if self.semantic {
            let owner = Arc::new(CheckedModule {
                synthetic,
                resolved,
                resolver_bytes,
                retained_loan_bytes,
                function_costs,
            });
            self.next_checked
                .insert(path.to_owned(), Arc::clone(&owner));
            ResolvedModuleInput::Checked(CheckedModuleLease::new(owner))
        } else {
            ResolvedModuleInput::Owned(resolved)
        }
    }
    /// Obtain an owning key only for actual semantic publication. The producer
    /// closure is dropped without execution for an AST-only receiving pass.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolved_module_split(
        &mut self,
        path: &str,
        key: impl FnOnce() -> Result<CheckedSource>,
        resolved: crate::hir::ResolvedProgram,
        resolver_bytes: usize,
        retained_loan_bytes: usize,
        function_costs: BTreeMap<String, usize>,
        reused_functions: usize,
    ) -> Result<ResolvedModuleInput> {
        if self.semantic {
            Ok(self.resolved_module_split_key(
                path,
                key()?,
                resolved,
                resolver_bytes,
                retained_loan_bytes,
                function_costs,
                reused_functions,
            ))
        } else {
            Ok(self.resolved_module_without_key(resolved, reused_functions))
        }
    }

    /// AST-only passes retain the resolved owner without constructing a cache
    /// key which would immediately be discarded. Counters match resolved_module.
    pub(crate) fn resolved_module_without_key(
        &mut self,
        resolved: crate::hir::ResolvedProgram,
        reused_functions: usize,
    ) -> ResolvedModuleInput {
        assert!(
            !self.semantic,
            "semantic retention requires an owning checked key"
        );
        self.resolved += 1;
        self.functions_reused += reused_functions;
        self.functions_resolved += resolved.functions.len().saturating_sub(reused_functions);
        ResolvedModuleInput::Owned(resolved)
    }

    pub(crate) fn lookup(&mut self, path: &str, source: &str) -> Option<Program> {
        let entry = self.entries.get(path)?;
        if entry.input().source != source || entry.program().path != path {
            return None;
        }
        self.reused += 1;
        self.reused_bytes += source.len();
        Some(entry.program().clone())
    }
    /// Borrow the exact compiler-created source AST retained by this pass.
    /// Unlike lookup, this post-admission use does not clone or count a cache hit.
    pub(crate) fn retained_source_program(&self, path: &str, source: &str) -> Result<&Program> {
        self.entries
            .get(path)
            .filter(|entry| entry.input().source == source && entry.program().path == path)
            .map(|entry| entry.program())
            .ok_or_else(|| invalid("frontend retained source AST does not match Project source"))
    }

    pub(crate) fn parsed(&mut self, bytes: usize) {
        self.parsed += 1;
        self.parsed_bytes += bytes;
    }
    pub(crate) fn canonicalized(&mut self) {
        self.canonicalizations += 1;
    }
    pub(crate) fn retain_current(
        &mut self,
        custody: &crate::workspace_graph::custody::SourceCustody,
        ast_budget: usize,
    ) -> Result<()> {
        if custody.len() > MAX_PROJECT_FRONTEND_CACHE_MODULES
            || ast_budget > MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET
        {
            return Err(capacity(
                "frontend retained AST inventory exceeds its bound",
            ));
        }
        let mut bytes = 0usize;
        for owner in custody.parsed().iter() {
            bytes = bytes
                .checked_add(owner.input().source.len())
                .ok_or_else(|| capacity("frontend retained source accounting overflow"))?;
            if bytes > MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES {
                return Err(capacity(
                    "frontend retained source inventory exceeds its byte bound",
                ));
            }
        }
        if self.resolved + self.checked_reused != custody.len()
            || (self.semantic && self.next_checked.len() != custody.len())
        {
            return Err(invalid(
                "frontend checked module inventory disagrees with admitted modules",
            ));
        }
        let entries = source_directory::SourceDirectory::current(custody.parsed())
            .map_err(|()| invalid("current source directory order disagrees"))?;
        if self.semantic && self.next_checked.keys().ne(entries.keys()) {
            return Err(invalid(
                "checked source directory does not match actual current source paths",
            ));
        }
        // Unique sorted paths and every pending OnceLock precondition are
        // checked before binding. The exclusive pass has no intervening call
        // which can complete a key; old hits are already ready and skipped.
        // Validate every association before completing or publishing a key.
        for (slot, owner) in custody.parsed().iter().enumerate() {
            if !owner.is_associated_with(&custody.metadata()[slot]) {
                return Err(invalid("current source metadata association disagrees"));
            }
            if let Some(entry) = self.next_checked.get(&owner.input().path) {
                if entry.synthetic.needs_current_owner() && !entry.synthetic.can_bind(slot, owner) {
                    return Err(invalid(
                        "checked key does not belong to the actual current parsed owner",
                    ));
                }
            }
        }
        for (slot, owner) in custody.parsed().iter().enumerate() {
            if let Some(entry) = self.next_checked.get(&owner.input().path) {
                if entry.synthetic.needs_current_owner() {
                    entry
                        .synthetic
                        .bind(slot, owner)
                        .map_err(|()| invalid("checked key transfer was already completed"))?;
                }
            }
        }
        self.entries = entries;
        self.retained_source_bytes = bytes;
        self.ast_budget = ast_budget;
        Ok(())
    }
}

pub(super) fn sources_from_revision(
    revision: &ProjectRevision,
) -> Result<Vec<ProjectFrontendSource>> {
    revision
        .sources()
        .iter()
        .map(|source| ProjectFrontendSource::new(source.path(), source.source()))
        .collect()
}
pub(super) fn work_value(build: &ProjectFrontendBuild) -> Result<Value> {
    serde_json::from_str(build.to_json())
        .map_err(|_| invalid("retained frontend work report is invalid"))
}
fn context_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"semaprax.project-frontend-cache.context.v1\0");
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G255", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G256", message.into())]
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn retained_checked_entry_requires_exact_stub_and_span_equality() {
        assert_eq!(
            MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND,
            crate::workspace_graph::MAX_BUILDER_BYTES,
            "checked reuse and ordinary graph construction must share a ceiling"
        );
        // Compiler-created synthetic shape: an imported provider stub has an
        // ordinary checked function representation beside the local entry.
        let source = "module local; @id(\"provider.stub\") fn helper(value: i64) -> i64 { value } @id(\"local.main\") fn main() -> i64 { helper(1) }";
        let synthetic = crate::parse(source, "local.spx").unwrap();
        let resolved = crate::hir::resolve(&synthetic).unwrap();
        let checked = BTreeMap::from([(
            "local.spx".to_owned(),
            Arc::new(CheckedModule {
                synthetic: CheckedSource::Owned(synthetic.clone()),
                resolved,
                resolver_bytes: 0,
                retained_loan_bytes: 0,
                function_costs: BTreeMap::new(),
            }),
        )]);
        let mut pass = FrontendPass {
            indexed_imports: Vec::new(),
            entries: BTreeMap::from([(
                "local.spx".to_owned(),
                fixture_current_owner(source.to_owned(), synthetic.clone()),
            )])
            .into(),
            parsed: 0,
            reused: 0,
            canonicalizations: 0,
            parsed_bytes: 0,
            reused_bytes: 0,
            resolved: 0,
            retained_source_bytes: 0,
            ast_budget: 0,
            semantic: true,
            checked,
            next_checked: BTreeMap::new(),
            checked_reused: 0,
            functions_resolved: 0,
            functions_reused: 0,
        };
        let checkpoint = pass.checkpoint_core_attempt().unwrap();
        assert!(pass
            .checked_retention_prebound(MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND)
            .is_ok());
        let capacity_error = pass
            .checked_retention_prebound(MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND + 1)
            .expect_err("one byte over the checked-module cache prebound must refuse");
        assert_eq!(capacity_error.len(), 1);
        assert_eq!(capacity_error[0].code, "SPX-G256");
        assert_eq!(
            capacity_error[0].message,
            format!(
                "checked module cache synthetic AST/HIR construction prebound exceeds {} bytes; this is the checked-module cache limit, separate from Workspace Semantic Graph builder_bytes",
                MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND
            )
        );
        assert!(pass.checked_module("local.spx", &synthetic).is_some());
        assert!(pass.checkpoint_core_attempt().is_none());
        pass.resolved = 9;
        pass.functions_resolved = 17;
        pass.rollback_core_attempt(checkpoint);
        assert!(pass.next_checked.is_empty());
        assert_eq!(
            (
                pass.resolved,
                pass.checked_reused,
                pass.functions_resolved,
                pass.functions_reused
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(
            pass.checked.len(),
            1,
            "immutable prior cache authority survives rollback"
        );
        assert!(pass.checked_module("local.spx", &synthetic).is_some());
        let mut signature = synthetic.clone();
        signature.functions[0].params[0].ty = crate::ast::Type::I32;
        assert!(pass.checked_module("local.spx", &signature).is_none());
        let mut provenance = synthetic.clone();
        provenance.functions[0].name_span.end += 1;
        assert!(pass.checked_module("local.spx", &provenance).is_none());
        let mut identity = synthetic.clone();
        identity.functions[0].stable_id = "provider.other".to_owned();
        assert!(pass.checked_module("local.spx", &identity).is_none());
        assert_eq!(pass.checked_reused, 1);
    }
}

#[cfg(test)]
#[path = "incremental/cold_source_tests.rs"]
mod cold_source_tests;

// Complete owned Program equality expressed over the sealed two-slice source.
// No fingerprint, provider hint or subject identity substitutes for a field.
fn checked_source_matches(
    current: &dyn crate::ast::program_source::ProgramSource,
    previous: &dyn ProgramSource,
) -> bool {
    current.path() == previous.path()
        && current.module() == previous.module()
        && current.module_uses() == previous.module_uses()
        && current.permits() == previous.permits()
        && current.types().iter().eq(previous.types().iter())
        && current.interfaces() == previous.interfaces()
        && current.protocols() == previous.protocols()
        && current.implementations() == previous.implementations()
        && current.session_protocols() == previous.session_protocols()
        && current.agents() == previous.agents()
        && current.functions().iter().eq(previous.functions().iter())
}

mod source_directory;
#[cfg(test)]
mod source_receiver_original;

#[cfg(test)]
fn fixture_current_owner(
    source: String,
    program: Program,
) -> Arc<crate::workspace_graph::current_source::CurrentParsedSource> {
    let (owner, refused) =
        crate::bounded_output::with_limit(MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET, || {
            decoded_current_owner(
                source,
                program,
                &mut crate::bounded_output::reserve_active_required,
            )
        });
    assert!(!refused);
    owner.expect("complete fixture current-source owner")
}
fn decoded_current_owner(
    source: String,
    program: Program,
    reserve: &mut dyn FnMut(usize) -> bool,
) -> Result<Arc<crate::workspace_graph::current_source::CurrentParsedSource>> {
    use crate::workspace_graph::current_source::{CurrentParsedSource, SourceMetadata};
    if !reserve(program.path.len()) {
        return Err(capacity("current source metadata path acquisition refused"));
    }
    let input = WorkspaceSource {
        path: program.path.clone(),
        source,
    };
    let mut carrier = |bytes| {
        if reserve(bytes) {
            Ok(())
        } else {
            Err(capacity(
                "current parsed source carrier acquisition refused",
            ))
        }
    };
    let metadata = SourceMetadata::new(input, &mut carrier)?;
    CurrentParsedSource::new(metadata, program, None, &mut carrier)
        .map_err(|_| invalid("current parsed source origin disagrees"))
}

#[cfg(test)]
mod cache_attempt_fit_tests {

    #[test]
    fn complete_cache_fit_sources_and_original_lookup_remain_bound() {
        use sha2::{Digest, Sha256};
        crate::source_reflows_merge::main7f8_source_inputs::controls();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/checked-cache-attempt-fit-v1");
        let records: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        for record in records.as_array().unwrap() {
            let original =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
                ),
                record["sha256"].as_str().unwrap()
            );
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main7f8_source_inputs::source("cache-fit", record["path"].as_str().unwrap(), &current);
            let restore = |source: &str| -> Option<String> {
                let mut restored = source.to_owned();
                for edit in record["edits"].as_array().unwrap().iter().rev() {
                    let new = edit["new"].as_str().unwrap();
                    if restored.matches(new).count() != 1 {
                        return None;
                    }
                    restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
                }
                Some(restored)
            };
            assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(original.as_str())
            );
            assert_ne!(
                restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
                Some(original.as_str())
            );
        }
        fn lookup(source: &str) -> &str {
            source
                .split("    pub(crate) fn checked_module_source(")
                .nth(1)
                .unwrap()
                .split("    /// Only a genuinely first checked resolution")
                .next()
                .unwrap()
        }
        let original = std::fs::read_to_string(evidence.join("1.before.txt")).unwrap();
        let current = std::fs::read_to_string(root.join("src/project/incremental.rs")).unwrap();
        assert_eq!(
            lookup(&current),
            lookup(&original),
            "source/Agent replay, telemetry and staging remain the complete original receiver"
        );
    }
    use super::*;
    use crate::bounded_output::{
        reserve_active_required, set_active_floor, with_limit_usage_refusal,
    };
    use crate::test_allocation_observer::measure;

    fn fixture(resolver_bytes: usize, retained_loan_bytes: usize) -> (FrontendPass, Program) {
        let source = "module cache.fit; @id(\"cache.main\") fn main()->i64 {0}";
        let synthetic = crate::parse(source, "fit.spx").unwrap();
        let resolved = crate::hir::resolve(&synthetic).unwrap();
        let pass = FrontendPass {
            indexed_imports: Vec::new(),
            entries: BTreeMap::from([(
                "fit.spx".to_owned(),
                fixture_current_owner(source.to_owned(), synthetic.clone()),
            )])
            .into(),
            parsed: 0,
            reused: 0,
            canonicalizations: 0,
            parsed_bytes: 0,
            reused_bytes: 0,
            resolved: 0,
            retained_source_bytes: 0,
            ast_budget: 0,
            semantic: true,
            checked: BTreeMap::from([(
                "fit.spx".to_owned(),
                Arc::new(CheckedModule {
                    synthetic: CheckedSource::Owned(synthetic.clone()),
                    resolved,
                    resolver_bytes,
                    retained_loan_bytes,
                    function_costs: BTreeMap::new(),
                }),
            )]),
            next_checked: BTreeMap::new(),
            checked_reused: 0,
            functions_resolved: 0,
            functions_reused: 0,
        };
        (pass, synthetic)
    }

    #[test]
    fn unfitting_entry_is_inert_and_exact_fitting_entry_keeps_real_checked_reuse() {
        for (limit, floor, expected) in [
            (0, 0, false),
            (18, 0, false),
            (19, 0, true),
            (27, 9, false),
            (28, 9, true),
        ] {
            // Controlled cost metadata tests eligibility, not a full graph bill.
            let (mut pass, source) = fixture(19, 23);
            let owners = Arc::strong_count(pass.checked.get("fit.spx").unwrap());
            let ((), refused, used, receipt) = with_limit_usage_refusal(limit, || {
                assert!(set_active_floor(floor));
                let (allowed, observed) =
                    measure(|| pass.checked_reuse_fits_active_attempt("fit.spx"));
                assert_eq!(allowed, expected);
                assert!(!observed.invalid);
                assert_eq!(
                    (
                        observed.allocations,
                        observed.reallocations,
                        observed.allocated,
                        observed.freed,
                        observed.live
                    ),
                    (0, 0, 0, 0, 0)
                );
                assert_eq!(
                    Arc::strong_count(pass.checked.get("fit.spx").unwrap()),
                    owners
                );
                assert!(pass.next_checked.is_empty());
                assert_eq!(
                    (pass.checked_reused, pass.functions_reused, pass.resolved),
                    (0, 0, 0)
                );
                assert!(pass.checkpoint_core_attempt().is_some());
                if allowed {
                    assert!(pass.checked_module_source("fit.spx", &source).is_some());
                    assert_eq!((pass.checked_reused, pass.functions_reused), (1, 1));
                    assert_eq!(pass.next_checked.len(), 1);
                }
            });
            assert!(!refused && receipt.is_none());
            assert_eq!(used, 0, "eligibility grants and debits nothing");
        }
    }

    #[test]
    fn maximum_cost_sticky_and_source_drift_cannot_publish_a_cached_candidate() {
        let (pass, _) = fixture(usize::MAX, 1);
        let ((), refused, used, receipt) = with_limit_usage_refusal(64, || {
            assert!(!pass.checked_reuse_fits_active_attempt("fit.spx"));
        });
        assert!(!refused && used == 0 && receipt.is_none());
        let (mut pass, mut changed) = fixture(0, 0);
        let ((), refused, used, receipt) = with_limit_usage_refusal(0, || {
            assert!(!reserve_active_required(1));
            assert!(!pass.checked_reuse_fits_active_attempt("fit.spx"));
            assert!(pass.next_checked.is_empty());
            assert_eq!((pass.checked_reused, pass.functions_reused), (0, 0));
        });
        assert!(refused && used == 0);
        assert_eq!(receipt.unwrap().requested, 1);
        changed.functions[0].name.push_str("_changed");
        assert!(pass.checked_reuse_fits_active_attempt("fit.spx"));
        assert!(pass.checked_module_source("fit.spx", &changed).is_none());
        assert!(pass.next_checked.is_empty());
        assert_eq!((pass.checked_reused, pass.functions_reused), (0, 0));
        assert!(
            pass.checked_reuse_fits_active_attempt("absent.spx"),
            "an absent entry follows the original cold route"
        );
    }
}
