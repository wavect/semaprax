//! Bounded, invocation-local Project v1 input authority.
//!
//! A project names every source explicitly. Loading authenticates those exact
//! files, runs the existing Semantic Workspace Phase-A build once in memory,
//! and retains held identities for a final caller-boundary recheck. It creates
//! no managed workspace and grants no publication authority.

mod authority;
mod build;
mod execution;
mod manifest;
mod rename;
mod semantic;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;

use authority::{authentication, DeclaredPathSelection, HeldDirectory, HeldFile};
#[cfg(all(test, windows))]
use authority::{declared_absolute_path, has_declared_alias_component};
pub use execution::{
    verify_execution_envelope, ProjectExecution, ProjectExecutionOptions, ProjectExecutionOutcome,
    ProjectExecutionRole, PROJECT_EXECUTION_SCHEMA,
};
use manifest::{capacity, grammar};
pub use manifest::{
    ProjectManifest, MAX_MANIFEST_BYTES, MAX_MODULE_BYTES, MAX_NAME_BYTES, MAX_PATH_BYTES,
    MAX_SOURCES, MAX_STABLE_ID_BYTES, MAX_TOTAL_SOURCE_BYTES, MAX_WEB_EXPORTS, PROJECT_SCHEMA,
};
pub(crate) use rename::{PreparedProjectRename, ProjectRenameDerivation};
pub use semantic::{PROJECT_SEMANTIC_CONTEXT_SCHEMA, PROJECT_SEMANTIC_GRAPH_SCHEMA};

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
    manifest: ProjectManifest,
    sources: Vec<ProjectSource>,
    workspace_manifest: String,
    workspace_revision: String,
    project_revision: String,
    entry_program: crate::hir::ResolvedProgram,
    test_program: crate::hir::ResolvedProgram,
    semantic: semantic::ProjectSemanticState,
    declared_inputs: Vec<DeclaredPathSelection>,
    held_manifest: HeldFile,
    held_sources: Vec<HeldFile>,
    held_directories: Vec<HeldDirectory>,
    published_subject: Option<&'static str>,
    request_invalidation: Option<Vec<Diagnostic>>,
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

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn sources(&self) -> &[ProjectSource] {
        &self.sources
    }

    pub fn workspace_manifest(&self) -> &str {
        &self.workspace_manifest
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn entry_program(&self) -> &crate::hir::ResolvedProgram {
        &self.entry_program
    }

    pub fn test_program(&self) -> &crate::hir::ResolvedProgram {
        &self.test_program
    }

    /// Return the retained complete declared-project graph. This performs no
    /// filesystem access and carries Project-specific, not managed-Workspace,
    /// provenance.
    pub fn semantic_graph(&self) -> &str {
        self.semantic.graph()
    }

    /// Render bounded Project-specific Context from the retained typed index.
    pub fn semantic_context(
        &self,
        target_kind: crate::workspace_analysis::WorkspaceAnalysisTargetKind,
        target: &str,
        options: crate::workspace_analysis::WorkspaceContextOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.semantic.context(
            self.manifest.name(),
            &self.project_revision,
            self.manifest.test_module(),
            target_kind,
            target,
            options,
        )
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

    /// Report successful admission. The linked scalar profile was validated
    /// before this snapshot became observable.
    pub fn check(&self) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }

    /// Evaluate the exact authenticated entry closure in memory.
    pub fn execute_entry(
        &self,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        execution::execute(self, ProjectExecutionRole::Entry, options)
    }

    /// Evaluate the exact authenticated test closure in memory.
    pub fn execute_test(
        &self,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        execution::execute(self, ProjectExecutionRole::Test, options)
    }

    /// Evaluate one exact authenticated closure selected by its closed role.
    pub fn execute(
        &self,
        role: ProjectExecutionRole,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        execution::execute(self, role, options)
    }

    /// Build the authenticated project entry closure as one scalar Web package.
    pub fn build_web(&mut self, output: &Path) -> Result<(), Vec<Diagnostic>> {
        let prepared = crate::wasm::prepare_project_web_with_scalar_exports(
            &self.entry_program,
            self.manifest.name(),
            &self.project_revision,
            &self.workspace_revision,
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

    /// Build the authenticated project entry closure as one native executable.
    ///
    /// The executable is compiled from exactly the linked entry HIR that Web
    /// publication and internal lowering-equivalence evidence consume. The
    /// destination must not exist, so publication never clobbers a file the
    /// caller did not create for this exact operation.
    pub fn build_native(&mut self, output: &Path) -> Result<(), Vec<Diagnostic>> {
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
        let prepared =
            crate::codegen::emit_hir_c(&self.entry_program).map_err(|error| vec![error])?;
        self.recheck()?;
        crate::codegen::compile_native_executable(&prepared, output)
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

    /// Emit the sole authenticated test-module closure as legacy core Wasm
    /// with `semaprax_main`, for backend-equivalence runners.
    pub fn test_wasm_module(&self) -> Result<Vec<u8>, Vec<Diagnostic>> {
        crate::wasm::emit_resolved_module(&self.test_program).map_err(|error| vec![error])
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

pub(crate) fn load_snapshot(manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
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

    let built = build::build_owned(&manifest, workspace_sources)?;
    let mut snapshot = ProjectSnapshot {
        root,
        manifest,
        sources: built.sources,
        workspace_manifest: built.workspace_manifest,
        workspace_revision: built.workspace_revision,
        project_revision: built.project_revision,
        entry_program: built.entry_program,
        test_program: built.test_program,
        semantic: built.semantic,
        declared_inputs,
        held_manifest,
        held_sources,
        held_directories,
        published_subject: None,
        request_invalidation: None,
    };
    snapshot.recheck()?;
    Ok(snapshot)
}

const WEB_PUBLICATION_SUBJECT: &str = "digest-bound Web package";
const NATIVE_PUBLICATION_SUBJECT: &str = "native executable";
