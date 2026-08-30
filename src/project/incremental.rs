//! Invocation-owned exact-source parsing cache. Semantic checking remains cold.
//! Cache entries are compiler-created ASTs, never submitted or serialized HIR.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;
use crate::workspace_graph::WorkspaceSource;

use super::{
    ProjectManifest, ProjectRevision, MAX_PATH_BYTES, MAX_SOURCES, MAX_TOTAL_SOURCE_BYTES,
};

pub const PROJECT_FRONTEND_CACHE_SCHEMA: &str = "semaprax.project-frontend-cache-work.v1";
pub const PROJECT_FRONTEND_CACHE_COMPATIBILITY: &str = "semaprax.project-frontend-canonical-ast.v1";
pub const MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES: usize = MAX_TOTAL_SOURCE_BYTES;
pub const MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET: usize = 16 * 1024 * 1024;
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

struct CachedModule {
    source: String,
    program: Arc<Program>,
}

/// A bounded in-memory cache belonging to its caller. It has no filesystem
/// root, serde constructor, global singleton, or checked-HIR bypass.
pub struct ProjectFrontendCache {
    context: String,
    entries: BTreeMap<String, Arc<CachedModule>>,
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
            entries: BTreeMap::new(),
        }
    }

    /// Validate every source and completely resolve/link/admit the Project.
    /// Cache changes commit only after the entire result/report succeeds.
    pub fn build(
        &mut self,
        manifest: &ProjectManifest,
        sources: &[ProjectFrontendSource],
    ) -> Result<ProjectFrontendBuild> {
        if sources.len() > MAX_SOURCES {
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
        let context = format!(
            "{}\0{}\0{}\0{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            PROJECT_FRONTEND_CACHE_COMPATIBILITY,
            manifest.to_canonical_toml()
        );
        let reset = context != self.context;
        let mut invalidated = BTreeSet::new();
        for (path, entry) in &self.entries {
            if reset || current.get(path.as_str()).copied() != Some(entry.source.as_str()) {
                invalidated.insert(path.clone());
            }
        }
        for path in current.keys() {
            if reset || !self.entries.contains_key(*path) {
                invalidated.insert((*path).to_owned());
            }
        }
        // A changed provider invalidates old consumers transitively. New import
        // edges belong to changed sources and therefore cannot be cache hits.
        let modules = self
            .entries
            .iter()
            .map(|(path, entry)| (entry.program.module.as_str(), path.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();
        for (path, entry) in &self.entries {
            for binding in &entry.program.module_uses {
                if let Some(provider) = modules.get(binding.target_module.as_str()) {
                    reverse
                        .entry((*provider).to_owned())
                        .or_default()
                        .insert(path.clone());
                }
            }
        }
        let mut pending = invalidated.iter().cloned().collect::<Vec<_>>();
        while let Some(path) = pending.pop() {
            if let Some(consumers) = reverse.get(&path) {
                for consumer in consumers {
                    if invalidated.insert(consumer.clone()) {
                        pending.push(consumer.clone());
                    }
                }
            }
        }
        let retained = self
            .entries
            .iter()
            .filter(|(path, _)| !reset && !invalidated.contains(*path))
            .map(|(path, entry)| (path.clone(), Arc::clone(entry)))
            .collect();
        let mut pass = FrontendPass {
            entries: retained,
            parsed: 0,
            reused: 0,
            canonicalizations: 0,
            parsed_bytes: 0,
            reused_bytes: 0,
            resolved: 0,
            retained_source_bytes: 0,
            ast_budget: 0,
        };
        let owned = sources
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source.path.clone(),
                source: source.source.clone(),
            })
            .collect();
        let built = super::build::build_owned_with_frontend(manifest, owned, &mut pass)?;
        let revision = Arc::new(ProjectRevision::from_built(manifest.clone(), built));
        let json=super::image::render(json!({"schema":PROJECT_FRONTEND_CACHE_SCHEMA,
            "compiler":{"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":PROJECT_FRONTEND_CACHE_COMPATIBILITY,"binary_identity_claimed":false},
            "context_digest":context_digest(context.as_bytes()),"project_revision":revision.project_revision(),
            "manifest_context_reset":reset,"invalidated_sources":invalidated,
            "work":{"modules_parsed":pass.parsed,"modules_reused":pass.reused,"canonicalizer_calls":pass.canonicalizations,
                "parsed_source_bytes":pass.parsed_bytes,"reused_source_bytes":pass.reused_bytes,"cached_AST_clones":pass.reused,
                "modules_resolved":pass.resolved,"checked_HIR_reused":0,"full_cross_file_checks":true,"full_link_and_profile_admission":true},
            "retained":{"modules":pass.entries.len(),"source_bytes":pass.retained_source_bytes,"AST_construction_prebound":pass.ast_budget},
            "limits":{"modules":MAX_SOURCES,"source_bytes":MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES,"AST_construction_prebound":MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET},
            "nonclaims":["not_incremental_semantic_verification","no_checked_HIR_reuse","no_persistent_or_cross_process_cache","not_allocator_or_RSS_accounting","no_source_or_execution_authority"]
        }),true,MAX_PROJECT_FRONTEND_REPORT_BYTES).map_err(|_|capacity("frontend work report exceeds its byte bound"))?;
        self.context = context;
        self.entries = pass.entries;
        Ok(ProjectFrontendBuild { revision, json })
    }

    pub(crate) fn fork(&self) -> Self {
        Self {
            context: self.context.clone(),
            entries: self.entries.clone(),
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
    entries: BTreeMap<String, Arc<CachedModule>>,
    parsed: usize,
    reused: usize,
    canonicalizations: usize,
    parsed_bytes: usize,
    reused_bytes: usize,
    resolved: usize,
    retained_source_bytes: usize,
    ast_budget: usize,
}
impl FrontendPass {
    pub(crate) fn lookup(&mut self, path: &str, source: &str) -> Option<Program> {
        let entry = self.entries.get(path)?;
        if entry.source != source || entry.program.path != path {
            return None;
        }
        self.reused += 1;
        self.reused_bytes += source.len();
        Some(entry.program.as_ref().clone())
    }
    pub(crate) fn parsed(&mut self, bytes: usize) {
        self.parsed += 1;
        self.parsed_bytes += bytes;
    }
    pub(crate) fn canonicalized(&mut self) {
        self.canonicalizations += 1;
    }
    pub(crate) fn retain(
        &mut self,
        sources: &[WorkspaceSource],
        programs: Vec<Program>,
        ast_budget: usize,
    ) -> Result<()> {
        if sources.len() != programs.len()
            || programs.len() > MAX_SOURCES
            || ast_budget > MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET
        {
            return Err(capacity(
                "frontend retained AST inventory exceeds its bound",
            ));
        }
        let mut entries = BTreeMap::new();
        let mut bytes = 0usize;
        for (source, program) in sources.iter().zip(programs) {
            if source.path != program.path {
                return Err(invalid("frontend AST source origin disagrees"));
            }
            bytes = bytes
                .checked_add(source.source.len())
                .ok_or_else(|| capacity("frontend retained source accounting overflow"))?;
            if bytes > MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES {
                return Err(capacity(
                    "frontend retained source inventory exceeds its byte bound",
                ));
            }
            let entry = if let Some(existing) = self
                .entries
                .get(&source.path)
                .filter(|entry| entry.source == source.source)
            {
                Arc::clone(existing)
            } else {
                Arc::new(CachedModule {
                    source: source.source.clone(),
                    program: Arc::new(program),
                })
            };
            entries.insert(source.path.clone(), entry);
        }
        self.resolved = entries.len();
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
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G256", message)]
}
