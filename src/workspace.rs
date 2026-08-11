//! Managed immutable-generation workspace transactions.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::ast::{Program, TypeDeclarationKind};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{graph, hir, parse, patch, verify};

const CONTROL: &str = ".semaprax-workspace";
const PATH_SET_SCHEMA: &str = "semaprax.workspace-path-set.v1";
const ROOT_SCHEMA: &str = "semaprax.workspace-root.v1";
pub(crate) const MANIFEST_SCHEMA: &str = "semaprax.workspace-manifest.v1";
pub(crate) const PATCH_SCHEMA: &str = "semaprax.semantic-workspace-patch.v1";
const SNAPSHOT_SCHEMA: &str = "semaprax.workspace-snapshot.v1";
pub(crate) const PREVIEW_SCHEMA: &str = "semaprax.semantic-workspace-preview.v1";

pub const MAX_MANAGED_FILES: usize = 16;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_WORKSPACE_PATCH_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_OPERATIONS: usize = 4096;
pub const MAX_DECLARATIONS: usize = 4096;
pub const MAX_CALLABLES: usize = 1024;
pub const MAX_CALL_SITES: usize = 65_536;
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
pub const MAX_PREVIEW_BYTES: usize = 65_536;
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 8;
pub const MAX_RETAINED_GENERATIONS: usize = 32;
pub const MAX_STAGING_ATTEMPTS: usize = 32;

const NONCLAIMS: [&str; 14] = [
    "no_cross_file_module_type_call_capability_or_identity_resolution",
    "no_repository_impact_review_context_target_or_test_analysis",
    "no_workspace_evidence_or_proof_artifact",
    "not_signature_authenticated_provenance_or_human_approval",
    "no_lock_stage_publish_or_commit_authority",
    "no_atomic_visibility_for_raw_files_git_or_editors",
    "no_create_delete_move_or_flat_materialization",
    "no_network_distributed_nfs_or_overlay_guarantee",
    "no_power_loss_durability_guarantee",
    "no_automatic_rollback_cleanup_or_gc",
    "no_acl_xattr_ads_preservation",
    "no_general_multi_file_repair",
    "no_new_patch_graph_cleanup_backend_or_runtime_semantics",
    "no_external_consumer_compatibility",
];

#[derive(Debug)]
pub struct WorkspaceFileSnapshot {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    source: String,
}

impl WorkspaceFileSnapshot {
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

#[derive(Debug)]
pub struct WorkspaceSnapshot {
    workspace_revision: String,
    files: Vec<WorkspaceFileSnapshot>,
    manifest_bytes: usize,
    retained_generations: usize,
    staging_attempts: usize,
    json: String,
}

pub(crate) struct WorkspaceSemanticSource {
    pub(crate) path: String,
    pub(crate) source_graph_schema: String,
    pub(crate) source_revision: String,
    pub(crate) source_digest: String,
    pub(crate) source: String,
}

pub(crate) struct WorkspaceSemanticReadAuthority {
    guard: WorkspaceGuard,
}

impl WorkspaceSemanticReadAuthority {
    pub(crate) fn workspace_revision(&self) -> &str {
        self.guard.snapshot.workspace_revision()
    }

    pub(crate) fn take_sources(&mut self) -> Vec<WorkspaceSemanticSource> {
        self.guard
            .snapshot
            .files
            .iter_mut()
            .map(|file| WorkspaceSemanticSource {
                path: file.path.clone(),
                source_graph_schema: file.source_graph_schema.clone(),
                source_revision: file.source_revision.clone(),
                source_digest: file.source_digest.clone(),
                source: std::mem::take(&mut file.source),
            })
            .collect()
    }

    pub(crate) fn manifest_bytes(&self) -> usize {
        self.guard.snapshot.manifest_bytes
    }

    pub(crate) fn retained_generations(&self) -> usize {
        self.guard.snapshot.retained_generations
    }

    pub(crate) fn staging_attempts(&self) -> usize {
        self.guard.snapshot.staging_attempts
    }

    pub(crate) fn finish<T>(
        mut self,
        result: Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        let value = match result {
            Ok(value) => value,
            Err(diagnostics) => {
                return Err(unlock_with_diagnostics(&self.guard.lock, diagnostics));
            }
        };
        if let Err(diagnostics) = self.guard.recheck() {
            return Err(unlock_with_diagnostics(&self.guard.lock, diagnostics));
        }
        unlock_file(&self.guard.lock)?;
        Ok(value)
    }
}

impl WorkspaceSnapshot {
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
    pub fn files(&self) -> &[WorkspaceFileSnapshot] {
        &self.files
    }

    pub fn to_json(&self) -> String {
        self.json.clone()
    }
}

fn bounded_snapshot_json(snapshot: &WorkspaceSnapshot) -> Result<String, Vec<Diagnostic>> {
    let mut used = 0usize;
    loop {
        let (rendered, overflowed) = crate::bounded_output::with_limit(MAX_SNAPSHOT_BYTES, || {
            render_snapshot(snapshot, used)
        });
        if overflowed || rendered.len() > MAX_SNAPSHOT_BYTES {
            return Err(limit("workspace snapshot exceeds 33554432 bytes"));
        }
        if rendered.len() == used {
            return Ok(rendered);
        }
        used = rendered.len();
    }
}

struct FileFact {
    path: String,
    module: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    source: String,
    declarations: Vec<String>,
    declaration_count: usize,
    callable_count: usize,
    call_count: usize,
}

struct ManifestFile {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
}

struct WorkspacePatchFile {
    path: String,
    patch: String,
}
struct WorkspacePatch {
    base: String,
    files: Vec<WorkspacePatchFile>,
    source: String,
    bytes: usize,
    digest: String,
}

pub(crate) struct WorkspacePlanSummary {
    patch: WorkspacePatch,
    candidate: Vec<FileFact>,
    previews: BTreeMap<String, (String, String, String, String, String, String)>,
    candidate_manifest: String,
    candidate_revision: String,
    usage: (usize, usize, usize),
    candidate_bytes: usize,
    operations: usize,
    changed_count: usize,
}

#[allow(
    dead_code,
    reason = "consumed by the private Workspace Evidence Phase A build"
)]
impl WorkspacePlanSummary {
    pub(crate) fn base_workspace_revision(&self) -> &str {
        &self.patch.base
    }

    pub(crate) fn candidate_workspace_revision(&self) -> &str {
        &self.candidate_revision
    }

    pub(crate) fn workspace_patch_digest(&self) -> &str {
        &self.patch.digest
    }

    pub(crate) fn workspace_patch_bytes(&self) -> usize {
        self.patch.bytes
    }

    pub(crate) fn managed_files(&self) -> usize {
        self.candidate.len()
    }

    pub(crate) fn changed_files(&self) -> usize {
        self.changed_count
    }

    pub(crate) fn candidate_source_bytes(&self) -> usize {
        self.candidate_bytes
    }

    pub(crate) fn candidate_manifest_bytes(&self) -> usize {
        self.candidate_manifest.len()
    }

    pub(crate) fn operations(&self) -> usize {
        self.operations
    }

    pub(crate) fn semantic_usage(&self) -> (usize, usize, usize) {
        self.usage
    }
}

struct WorkspacePlan {
    summary: WorkspacePlanSummary,
    preflights: Vec<WorkspaceEvidencePreflight>,
}

impl Deref for WorkspacePlan {
    type Target = WorkspacePlanSummary;

    fn deref(&self) -> &Self::Target {
        &self.summary
    }
}

#[allow(
    dead_code,
    reason = "consumed by the private Workspace Evidence Phase A build"
)]
pub(crate) struct WorkspaceEvidencePreflight {
    path: String,
    preflight: patch::PatchPreflight,
}

pub(crate) struct WorkspaceCommitAuthority {
    guard: WorkspaceGuard,
    patch_input: AuthenticatedText,
    plan: WorkspacePlanSummary,
}

pub(crate) struct WorkspaceEvidenceBinding {
    path: String,
    patch_schema: String,
    patch_digest: String,
    base_source_graph_schema: String,
    candidate_source_graph_schema: String,
    base_revision: String,
    candidate_revision: String,
    base_source_digest: String,
    candidate_source_digest: String,
}

impl WorkspaceEvidenceBinding {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn patch_schema(&self) -> &str {
        &self.patch_schema
    }

    pub(crate) fn patch_digest(&self) -> &str {
        &self.patch_digest
    }

    pub(crate) fn base_source_graph_schema(&self) -> &str {
        &self.base_source_graph_schema
    }

    pub(crate) fn candidate_source_graph_schema(&self) -> &str {
        &self.candidate_source_graph_schema
    }

    pub(crate) fn base_revision(&self) -> &str {
        &self.base_revision
    }

    pub(crate) fn candidate_revision(&self) -> &str {
        &self.candidate_revision
    }

    pub(crate) fn base_source_digest(&self) -> &str {
        &self.base_source_digest
    }

    pub(crate) fn candidate_source_digest(&self) -> &str {
        &self.candidate_source_digest
    }
}

#[allow(
    dead_code,
    reason = "consumed by the private Workspace Evidence Phase A build"
)]
impl WorkspaceEvidencePreflight {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn into_parts(self) -> (String, patch::PatchPreflight) {
        (self.path, self.preflight)
    }
}

#[allow(
    dead_code,
    reason = "consumed by the private Workspace Evidence Phase A build"
)]
pub(crate) struct WorkspaceReadBuild {
    guard: WorkspaceGuard,
    patch_input: AuthenticatedText,
    summary: WorkspacePlanSummary,
    preflights: Option<Vec<WorkspaceEvidencePreflight>>,
}

pub(crate) struct WorkspaceEvidenceGuard {
    guard: Option<WorkspaceLockGuard>,
}

pub(crate) struct WorkspaceEvidenceApplyGuard {
    guard: Option<WorkspaceLockGuard>,
    patch_input: Option<AuthenticatedText>,
}

#[allow(
    dead_code,
    reason = "consumed by the private Workspace Evidence Phase A build"
)]
impl WorkspaceReadBuild {
    pub(crate) fn plan(&self) -> &WorkspacePlanSummary {
        &self.summary
    }

    pub(crate) fn take_preflights(
        &mut self,
    ) -> Result<Vec<WorkspaceEvidencePreflight>, Vec<Diagnostic>> {
        self.preflights
            .take()
            .ok_or_else(|| invariant("workspace evidence preflights were already consumed"))
    }

    pub(crate) fn preview_json(&self) -> Result<String, Vec<Diagnostic>> {
        bounded_preview(&self.guard.snapshot, &self.summary)
    }

    pub(crate) fn evidence_binding(
        &self,
        path: &str,
    ) -> Result<WorkspaceEvidenceBinding, Vec<Diagnostic>> {
        let base = self
            .guard
            .snapshot
            .files
            .iter()
            .find(|file| file.path == path)
            .ok_or_else(|| invariant("workspace evidence base path is absent"))?;
        let candidate = self
            .summary
            .candidate
            .iter()
            .find(|file| file.path == path)
            .ok_or_else(|| invariant("workspace evidence candidate path is absent"))?;
        let preview = self
            .summary
            .previews
            .get(path)
            .ok_or_else(|| invariant("workspace evidence preview path is absent"))?;
        Ok(WorkspaceEvidenceBinding {
            path: path.to_owned(),
            patch_schema: preview.0.clone(),
            patch_digest: preview.1.clone(),
            base_source_graph_schema: preview.2.clone(),
            candidate_source_graph_schema: preview.3.clone(),
            base_revision: preview.4.clone(),
            candidate_revision: preview.5.clone(),
            base_source_digest: base.source_digest.clone(),
            candidate_source_digest: candidate.source_digest.clone(),
        })
    }

    pub(crate) fn base_source_bytes(&self) -> usize {
        self.guard
            .snapshot
            .files
            .iter()
            .map(|file| file.source.len())
            .sum()
    }

    pub(crate) fn base_manifest_bytes(&self) -> usize {
        self.guard.snapshot.manifest_bytes
    }

    pub(crate) fn retained_generations(&self) -> usize {
        self.guard.snapshot.retained_generations
    }

    pub(crate) fn staging_attempts(&self) -> usize {
        self.guard.snapshot.staging_attempts
    }

    pub(crate) fn recheck(mut self) -> Result<(), Vec<Diagnostic>> {
        if let Err(diagnostics) = self.patch_input.recheck() {
            return Err(unlock_with_diagnostics(&self.guard.lock, diagnostics));
        }
        if self.patch_input.source != self.summary.patch.source {
            return Err(unlock_with_diagnostics(
                &self.guard.lock,
                invariant("owned workspace patch changed after semantic planning"),
            ));
        }
        if let Err(diagnostics) = self.guard.recheck() {
            return Err(unlock_with_diagnostics(&self.guard.lock, diagnostics));
        }
        unlock_file(&self.guard.lock)
    }

    pub(crate) fn into_commit_authority(self) -> Result<WorkspaceCommitAuthority, Vec<Diagnostic>> {
        if !self.guard.exclusive {
            return Err(self.release_with_error(invariant(
                "workspace evidence commit authority requires the exclusive workspace lock",
            )));
        }
        if self.preflights.is_some() {
            return Err(self.release_with_error(invariant(
                "workspace evidence preflights must be consumed before commit authority",
            )));
        }
        Ok(WorkspaceCommitAuthority {
            guard: self.guard,
            patch_input: self.patch_input,
            plan: self.summary,
        })
    }

    pub(crate) fn release_with_error(self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        unlock_with_diagnostics(&self.guard.lock, diagnostics)
    }
}

struct AuthenticatedText {
    path: PathBuf,
    label: String,
    file: File,
    identity: FileIdentity,
    source: String,
    max: usize,
    code: &'static str,
}

impl AuthenticatedText {
    fn recheck(&mut self) -> Result<(), Vec<Diagnostic>> {
        let path_metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            io(
                self.code,
                format!("cannot re-inspect {}: {error}", self.label),
            )
        })?;
        if !path_metadata.is_file()
            || path_metadata.file_type().is_symlink()
            || metadata_is_reparse(&path_metadata)
        {
            return Err(invariant(
                "workspace object changed to an alias during authentication",
            ));
        }
        require_single_link_path(&self.path, self.code)?;
        require_single_link_file(&self.file, self.code)?;
        let current = identity_from_path(&self.path, self.code)?;
        if current != self.identity {
            return Err(invariant(
                "workspace object identity changed during authentication",
            ));
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| io(self.code, format!("cannot seek {}: {error}", self.label)))?;
        let mut bytes = Vec::with_capacity(self.source.len());
        std::io::Read::by_ref(&mut self.file)
            .take(self.max.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io(self.code, format!("cannot reread {}: {error}", self.label)))?;
        if bytes != self.source.as_bytes() {
            return Err(invariant("workspace object changed during authentication"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume: u64,
    #[cfg(windows)]
    index: u64,
}

struct AuthenticatedDirectory {
    path: PathBuf,
    identity: FileIdentity,
    #[cfg(unix)]
    file: File,
    #[cfg(windows)]
    handle: winapi_util::Handle,
}

impl AuthenticatedDirectory {
    fn recheck(&self) -> Result<(), Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            io(
                "SPX-I209",
                format!("cannot re-inspect managed directory: {error}"),
            )
        })?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata_is_reparse(&metadata)
            || identity_from_path(&self.path, "SPX-I209")? != self.identity
        {
            return Err(invariant(
                "workspace directory identity changed during authentication",
            ));
        }
        #[cfg(unix)]
        if identity_from_file(&self.file, "SPX-I209")? != self.identity {
            return Err(invariant("held workspace directory identity changed"));
        }
        #[cfg(windows)]
        {
            let current = winapi_util::Handle::from_path_any(&self.path)
                .map_err(|error| io("SPX-I209", format!("cannot retain directory: {error}")))?;
            if identity_from_windows_handle(&current, "SPX-I209")? != self.identity
                || identity_from_windows_handle(&self.handle, "SPX-I209")? != self.identity
            {
                return Err(invariant("held workspace directory identity changed"));
            }
        }
        Ok(())
    }
}

struct AuthenticatedSnapshot {
    snapshot: WorkspaceSnapshot,
    directories: Vec<AuthenticatedDirectory>,
    texts: Vec<AuthenticatedText>,
}

struct ParsedFact {
    path: String,
    source: String,
    program: Program,
    usage: (usize, usize, usize),
}

#[allow(dead_code)]
struct WorkspaceGuard {
    root: PathBuf,
    root_identity: FileIdentity,
    control: PathBuf,
    lock_path: PathBuf,
    lock: File,
    lock_identity: FileIdentity,
    snapshot: WorkspaceSnapshot,
    directories: Vec<AuthenticatedDirectory>,
    texts: Vec<AuthenticatedText>,
    exclusive: bool,
    generation_names: BTreeSet<String>,
    staging_names: BTreeSet<String>,
    mode: WorkspaceMode,
}

struct WorkspaceLockGuard {
    root: PathBuf,
    root_identity: FileIdentity,
    control: PathBuf,
    lock_path: PathBuf,
    lock: File,
    lock_identity: FileIdentity,
    exclusive: bool,
}

#[allow(dead_code)]
struct PreparedGeneration {
    path: PathBuf,
    directories: Vec<AuthenticatedDirectory>,
    texts: Vec<AuthenticatedText>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RelocationObject {
    Directory,
    Text(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelocationEntry {
    relative_path: PathBuf,
    identity: FileIdentity,
    object: RelocationObject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GenerationFingerprint {
    entries: Vec<RelocationEntry>,
}

impl WorkspaceGuard {
    fn recheck(&mut self) -> Result<(), Vec<Diagnostic>> {
        recheck_lock(&self.lock_path, &self.lock, &self.lock_identity)?;
        if authenticate_directory(&self.root)? != self.root_identity {
            return Err(invariant("workspace root identity changed during preview"));
        }
        for directory in &self.directories {
            directory.recheck()?;
        }
        for text in &mut self.texts {
            text.recheck()?;
        }
        validate_control(&self.control)?;
        let current = snapshot_authenticated_mode(
            &self.root,
            &self.control,
            Some(&self.lock_identity),
            self.mode,
        )?;
        let snapshot_matches = if self.mode == WorkspaceMode::Ordinary {
            current.snapshot.json == self.snapshot.json
        } else {
            semantic_snapshot_binding_eq(&current.snapshot, &self.snapshot)
        };
        if !snapshot_matches {
            return Err(stale(
                "workspace authenticated snapshot changed during operation",
            ));
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn recheck_base_authority(&mut self) -> Result<(), Vec<Diagnostic>> {
        recheck_lock(&self.lock_path, &self.lock, &self.lock_identity)?;
        if authenticate_directory(&self.root)? != self.root_identity {
            return Err(invariant("workspace root identity changed during staging"));
        }
        for directory in &self.directories {
            directory.recheck()?;
        }
        for text in &mut self.texts {
            text.recheck()?;
        }
        validate_control(&self.control)?;
        Ok(())
    }

    #[allow(dead_code)]
    fn recheck_phase_inventory(
        &self,
        generation_names: &BTreeSet<String>,
        staging_names: &BTreeSet<String>,
        owned: Option<&PreparedGeneration>,
    ) -> Result<(), Vec<Diagnostic>> {
        let generations_path = self.control.join("generations");
        let staging_path = self.control.join("staging");
        let (_, generations) =
            count_directories_bounded(&generations_path, MAX_RETAINED_GENERATIONS)?;
        let (_, staging_directories, staging_files) = validate_staging_inventory(&staging_path)?;
        let actual_generations = inventory_names_from_directories(&generations)?;
        let actual_staging = inventory_names_from_directories(&staging_directories)?
            .into_iter()
            .chain(inventory_names_from_texts(&staging_files)?)
            .collect::<BTreeSet<_>>();
        if &actual_generations != generation_names || &actual_staging != staging_names {
            return Err(invariant(
                "workspace generation or staging inventory changed during construction",
            ));
        }
        let mut entries = self
            .directories
            .iter()
            .map(|entry| (&entry.path, &entry.identity))
            .chain(
                self.texts
                    .iter()
                    .map(|entry| (&entry.path, &entry.identity)),
            )
            .chain(
                generations
                    .iter()
                    .map(|entry| (&entry.path, &entry.identity)),
            )
            .chain(
                staging_directories
                    .iter()
                    .map(|entry| (&entry.path, &entry.identity)),
            )
            .chain(
                staging_files
                    .iter()
                    .map(|entry| (&entry.path, &entry.identity)),
            )
            .collect::<Vec<_>>();
        entries.push((&self.lock_path, &self.lock_identity));
        if let Some(owned) = owned {
            entries.extend(
                owned
                    .directories
                    .iter()
                    .map(|entry| (&entry.path, &entry.identity)),
            );
            entries.extend(
                owned
                    .texts
                    .iter()
                    .map(|entry| (&entry.path, &entry.identity)),
            );
        }
        require_distinct_path_identities(&entries)?;
        let mut unique = BTreeMap::<PathBuf, &FileIdentity>::new();
        for (path, identity) in entries {
            unique.entry(path.clone()).or_insert(identity);
        }
        require_same_volume(&unique.into_values().collect::<Vec<_>>())
    }
}

fn semantic_snapshot_binding_eq(left: &WorkspaceSnapshot, right: &WorkspaceSnapshot) -> bool {
    left.workspace_revision == right.workspace_revision
        && left.manifest_bytes == right.manifest_bytes
        && left.retained_generations == right.retained_generations
        && left.staging_attempts == right.staging_attempts
        && left.files.len() == right.files.len()
        && left.files.iter().zip(&right.files).all(|(left, right)| {
            left.path == right.path
                && left.source_graph_schema == right.source_graph_schema
                && left.source_revision == right.source_revision
                && left.source_digest == right.source_digest
        })
}

/// Initializes a managed workspace without modifying the original source files.
pub fn initialize(root: &Path, path_set_path: &Path) -> Result<String, Vec<Diagnostic>> {
    initialize_with_hook(root, path_set_path, |_| {})
}

#[derive(Clone, Copy)]
pub(crate) enum InitializePoint {
    SemanticPreflightComplete,
    SemanticStagingReady,
    GenerationBeforeRename,
    GenerationDestinationChecked,
    GenerationRelocated,
    ActiveBeforeRename,
    ActiveDestinationChecked,
    ActiveRelocated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceMode {
    Ordinary,
    Semantic,
}

impl WorkspaceMode {
    fn parse_paths(self, source: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
        match self {
            Self::Ordinary => parse_path_set(source),
            Self::Semantic => crate::semantic_workspace::parse_path_set(source),
        }
    }

    fn prepare_initial_facts(
        self,
        path_set_source: &str,
        sources: Vec<(String, String)>,
    ) -> Result<(Vec<FileFact>, String, String), Vec<Diagnostic>> {
        match self {
            Self::Ordinary => {
                let facts = file_facts(sources, true)?;
                validate_workspace_facts(&facts)?;
                let manifest = bounded_manifest(&facts)?;
                let revision = workspace_revision(&manifest);
                Ok((facts, manifest, revision))
            }
            Self::Semantic => {
                let sources = sources
                    .into_iter()
                    .map(
                        |(path, source)| crate::semantic_workspace::SemanticWorkspaceSource {
                            path,
                            source,
                        },
                    )
                    .collect();
                let preflight =
                    crate::semantic_workspace::preflight_owned(path_set_source, sources)?;
                let (files, manifest, revision) = preflight.into_generation_parts();
                let facts = files
                    .into_iter()
                    .map(|file| {
                        let (path, schema, source_revision, source_digest, source) =
                            file.into_parts();
                        FileFact {
                            path,
                            module: String::new(),
                            source_graph_schema: schema,
                            source_revision,
                            source_digest,
                            source,
                            declarations: Vec::new(),
                            declaration_count: 0,
                            callable_count: 0,
                            call_count: 0,
                        }
                    })
                    .collect();
                Ok((facts, manifest, revision))
            }
        }
    }

    fn render_active(self, revision: &str) -> Result<String, Vec<Diagnostic>> {
        match self {
            Self::Ordinary => Ok(render_root(revision)),
            Self::Semantic => crate::semantic_workspace::render_root(revision),
        }
    }

    fn parse_active(self, source: &str) -> Result<String, Vec<Diagnostic>> {
        match self {
            Self::Ordinary => parse_root(source),
            Self::Semantic => crate::semantic_workspace::parse_root(source),
        }
    }

    fn manifest_revision(self, manifest: &str) -> String {
        match self {
            Self::Ordinary => workspace_revision(manifest),
            Self::Semantic => crate::semantic_workspace::semantic_workspace_revision(manifest),
        }
    }

    fn parse_manifest(self, source: &str) -> Result<Vec<ManifestFile>, Vec<Diagnostic>> {
        match self {
            Self::Ordinary => parse_manifest(source),
            Self::Semantic => crate::semantic_workspace::parse_manifest(source).map(|files| {
                files
                    .into_iter()
                    .map(|file| ManifestFile {
                        path: file.path().to_owned(),
                        source_graph_schema: file.source_graph_schema().to_owned(),
                        source_revision: file.source_revision().to_owned(),
                        source_digest: file.source_digest().to_owned(),
                        bytes: file.bytes(),
                    })
                    .collect()
            }),
        }
    }

    fn validate_snapshot_sources(
        self,
        manifest: &str,
        sources: Vec<(String, String)>,
    ) -> Result<Vec<FileFact>, Vec<Diagnostic>> {
        match self {
            Self::Ordinary => {
                let facts = file_facts(sources, true)?;
                validate_workspace_facts(&facts)?;
                Ok(facts)
            }
            Self::Semantic => {
                let sources = sources
                    .into_iter()
                    .map(
                        |(path, source)| crate::semantic_workspace::SemanticWorkspaceSource {
                            path,
                            source,
                        },
                    )
                    .collect();
                let preflight =
                    crate::semantic_workspace::replay_manifest_owned(manifest, sources)?;
                let (files, replayed_manifest, _) = preflight.into_generation_parts();
                if replayed_manifest != manifest {
                    return Err(invariant(
                        "semantic managed generation manifest replay changed bytes",
                    ));
                }
                Ok(files
                    .into_iter()
                    .map(|file| {
                        let (path, schema, source_revision, source_digest, source) =
                            file.into_parts();
                        FileFact {
                            path,
                            module: String::new(),
                            source_graph_schema: schema,
                            source_revision,
                            source_digest,
                            source,
                            declarations: Vec::new(),
                            declaration_count: 0,
                            callable_count: 0,
                            call_count: 0,
                        }
                    })
                    .collect())
            }
        }
    }
}

fn initialize_with_hook(
    root: &Path,
    path_set_path: &Path,
    hook: impl FnMut(InitializePoint),
) -> Result<String, Vec<Diagnostic>> {
    initialize_with_mode(root, path_set_path, WorkspaceMode::Ordinary, hook)
}

pub(crate) fn initialize_semantic_with_hook(
    root: &Path,
    path_set_path: &Path,
    hook: impl FnMut(InitializePoint),
) -> Result<String, Vec<Diagnostic>> {
    initialize_with_mode(root, path_set_path, WorkspaceMode::Semantic, hook)
}

fn initialize_with_mode(
    root: &Path,
    path_set_path: &Path,
    mode: WorkspaceMode,
    mut hook: impl FnMut(InitializePoint),
) -> Result<String, Vec<Diagnostic>> {
    let root = canonical_root(root)?;
    let root_dir = authenticate_directory_held(&root)?;
    let mut paths_input = if mode == WorkspaceMode::Semantic {
        authenticate_text_semantic(
            path_set_path,
            MAX_MANIFEST_BYTES,
            "SPX-I209",
            "path_set_bytes",
            MAX_MANIFEST_BYTES,
        )?
    } else {
        authenticate_text(path_set_path, MAX_MANIFEST_BYTES, "SPX-I209")?
    };
    let paths = mode.parse_paths(&paths_input.source)?;
    let mut total = 0usize;
    let mut sources = Vec::with_capacity(paths.len());
    let mut authenticated_sources = Vec::with_capacity(paths.len());
    for logical in paths {
        let remaining = MAX_TOTAL_SOURCE_BYTES.saturating_sub(total);
        let input = if mode == WorkspaceMode::Semantic {
            authenticate_managed_source_semantic(&root, &logical, remaining)?
        } else {
            authenticate_managed_source(&root, &logical, remaining)?
        };
        let source = input.source.clone();
        total = total.checked_add(source.len()).ok_or_else(|| {
            if mode == WorkspaceMode::Semantic {
                semantic_storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES)
            } else {
                limit("source byte count overflow")
            }
        })?;
        if total > MAX_TOTAL_SOURCE_BYTES {
            return Err(if mode == WorkspaceMode::Semantic {
                semantic_storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES)
            } else {
                limit("workspace sources exceed 16777216 bytes")
            });
        }
        sources.push((logical, source));
        authenticated_sources.push(input);
    }
    let mut permission_seals = if mode == WorkspaceMode::Semantic {
        capture_permission_seals(
            std::iter::once(root.as_path())
                .chain(std::iter::once(paths_input.path.as_path()))
                .chain(
                    authenticated_sources
                        .iter()
                        .map(|source| source.path.as_path()),
                ),
        )?
    } else {
        Vec::new()
    };
    let (facts, manifest, revision) = mode.prepare_initial_facts(&paths_input.source, sources)?;
    let semantic_original_nested_directories = if mode == WorkspaceMode::Semantic {
        hook(InitializePoint::SemanticPreflightComplete);
        paths_input.recheck()?;
        for source in &mut authenticated_sources {
            source.recheck()?;
        }
        require_distinct_text_identities(&authenticated_sources, Some(&paths_input), None)?;
        let directories =
            authenticate_directory_trie(&root, facts.iter().map(|fact| fact.path.as_str()))?;
        let mut identities = vec![&root_dir.identity, &paths_input.identity];
        identities.extend(authenticated_sources.iter().map(|source| &source.identity));
        identities.extend(directories.iter().map(|directory| &directory.identity));
        require_distinct_identities(&identities)?;
        require_same_volume(&identities)?;
        permission_seals.extend(capture_permission_seals(
            directories.iter().map(|directory| directory.path.as_path()),
        )?);
        Some(directories)
    } else {
        None
    };
    let control = root.join(CONTROL);
    std::fs::create_dir(&control).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot create managed workspace: {error}"),
        )
    })?;
    let control_dir = authenticate_directory_held(&control)?;
    let lock_path = control.join("LOCK");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|error| io("SPX-I209", format!("cannot create workspace LOCK: {error}")))?;
    require_single_link_file(&lock, "SPX-I209")?;
    let lock_identity = identity_from_file(&lock, "SPX-I209")?;
    lock_file(&lock, true)?;
    let mut active_pivoted = false;
    let result = (|| {
        std::fs::create_dir(control.join("generations"))
            .map_err(|error| io("SPX-I211", format!("cannot create generations: {error}")))?;
        std::fs::create_dir(control.join("staging"))
            .map_err(|error| io("SPX-I211", format!("cannot create staging: {error}")))?;
        let generations_dir = authenticate_directory_held(&control.join("generations"))?;
        let staging_dir = authenticate_directory_held(&control.join("staging"))?;
        if mode == WorkspaceMode::Semantic {
            hook(InitializePoint::SemanticStagingReady);
        }
        let (slot, expected_staging_names) = if mode == WorkspaceMode::Ordinary {
            let slot = control.join("staging").join("0");
            std::fs::create_dir(&slot)
                .map_err(|error| io("SPX-I211", format!("cannot create staging slot: {error}")))?;
            (slot, BTreeSet::from(["0".to_owned()]))
        } else {
            validate_staging_inventory(&control.join("staging"))?;
            let mut expected = BTreeSet::new();
            let mut selected = None;
            for ordinal in 0..MAX_STAGING_ATTEMPTS {
                let candidate = control.join("staging").join(ordinal.to_string());
                match std::fs::create_dir(&candidate) {
                    Ok(()) => {
                        expected.insert(ordinal.to_string());
                        selected = Some(candidate);
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        validate_staging_inventory(&control.join("staging"))?;
                    }
                    Err(error) => {
                        return Err(io(
                            "SPX-I211",
                            format!("cannot create staging slot: {error}"),
                        ));
                    }
                }
            }
            (
                selected.ok_or_else(|| semantic_storage_limit("staging_attempts", 32))?,
                expected,
            )
        };
        write_generation(&slot, &manifest, &facts)?;
        paths_input.recheck()?;
        for source in &mut authenticated_sources {
            source.recheck()?;
        }
        require_distinct_text_identities(&authenticated_sources, Some(&paths_input), None)?;
        let mut staged =
            authenticate_generation_deep_mode(&slot, &manifest, &facts, &revision, mode)?;
        hook(InitializePoint::GenerationBeforeRename);
        staged.recheck()?;
        paths_input.recheck()?;
        for source in &mut authenticated_sources {
            source.recheck()?;
        }
        let staged_fingerprint = staged.fingerprint()?;
        let generation = control.join("generations").join(revision_hex(&revision)?);
        require_absent_destination(
            &generation,
            "SPX-I211",
            "initial generation destination already exists",
        )?;
        #[cfg(windows)]
        drop(staged);
        hook(InitializePoint::GenerationDestinationChecked);
        publish_no_replace(
            &slot,
            &generation,
            "SPX-I211",
            "cannot publish initial generation",
        )?;
        hook(InitializePoint::GenerationRelocated);
        let mut published =
            authenticate_generation_deep_mode(&generation, &manifest, &facts, &revision, mode)?;
        staged_fingerprint.require_equivalent(&mut published)?;
        paths_input.recheck()?;
        for source in &mut authenticated_sources {
            source.recheck()?;
        }
        let active_stage = slot.clone();
        let active_source = mode.render_active(&revision)?;
        write_new_file(&active_stage, active_source.as_bytes())?;
        let staged_active_code = if mode == WorkspaceMode::Semantic {
            "SPX-I211"
        } else {
            "SPX-I212"
        };
        let mut staged_active =
            authenticate_text(&active_stage, MAX_MANIFEST_BYTES, staged_active_code)?;
        if mode.parse_active(&staged_active.source)? != revision {
            return Err(invariant(
                "staged ACTIVE does not bind the initial generation",
            ));
        }
        staged_active.recheck()?;
        staged_active.recheck()?;
        if mode.parse_active(&staged_active.source)? != revision {
            return Err(invariant(
                "staged ACTIVE changed before initial publication",
            ));
        }
        let active_path = control.join("ACTIVE");
        require_absent_destination(
            &active_path,
            if mode == WorkspaceMode::Semantic {
                "SPX-G153"
            } else {
                "SPX-I212"
            },
            "initial ACTIVE destination already exists",
        )?;
        let original_nested_directories = match semantic_original_nested_directories {
            Some(directories) => directories,
            None => {
                authenticate_directory_trie(&root, facts.iter().map(|fact| fact.path.as_str()))?
            }
        };
        let mut initializing_identities = vec![
            &root_dir.identity,
            &control_dir.identity,
            &generations_dir.identity,
            &staging_dir.identity,
            &lock_identity,
            &staged_active.identity,
        ];
        initializing_identities.extend(
            published
                .directories
                .iter()
                .map(|directory| &directory.identity),
        );
        initializing_identities.extend(published.texts.iter().map(|input| &input.identity));
        initializing_identities.extend(authenticated_sources.iter().map(|input| &input.identity));
        initializing_identities.extend(
            original_nested_directories
                .iter()
                .map(|directory| &directory.identity),
        );
        require_distinct_identities(&initializing_identities)?;
        require_same_volume(&initializing_identities)?;
        if mode == WorkspaceMode::Semantic {
            permission_seals.extend(capture_permission_seals(
                std::iter::once(control.as_path())
                    .chain(std::iter::once(control.join("generations").as_path()))
                    .chain(std::iter::once(control.join("staging").as_path()))
                    .chain(std::iter::once(lock_path.as_path()))
                    .chain(std::iter::once(staged_active.path.as_path()))
                    .chain(
                        published
                            .directories
                            .iter()
                            .map(|directory| directory.path.as_path()),
                    )
                    .chain(published.texts.iter().map(|text| text.path.as_path())),
            )?);
        }
        let expected_manifest_files = facts
            .iter()
            .map(|fact| ManifestFile {
                path: fact.path.clone(),
                source_graph_schema: fact.source_graph_schema.clone(),
                source_revision: fact.source_revision.clone(),
                source_digest: fact.source_digest.clone(),
                bytes: fact.source.len(),
            })
            .collect::<Vec<_>>();
        let mut complete_final_check = || -> Result<(), Vec<Diagnostic>> {
            recheck_lock(&lock_path, &lock, &lock_identity)?;
            validate_initializing_control(&control)?;
            let (_, staging_directories, staging_files) =
                validate_staging_inventory(&control.join("staging"))?;
            let actual_staging_names = inventory_names_from_directories(&staging_directories)?
                .into_iter()
                .chain(inventory_names_from_texts(&staging_files)?)
                .collect::<BTreeSet<_>>();
            let staging_is_exact = actual_staging_names == expected_staging_names;
            let generations_are_exact = mode == WorkspaceMode::Ordinary
                || count_directories_bounded(
                    &control.join("generations"),
                    MAX_RETAINED_GENERATIONS,
                )?
                .0 == 1;
            if !generations_are_exact || !staging_is_exact {
                return Err(invariant(
                    "initial semantic workspace generation/staging inventory is not exact",
                ));
            }
            paths_input.recheck()?;
            for source in &mut authenticated_sources {
                source.recheck()?;
            }
            published.recheck()?;
            for directory in [&root_dir, &control_dir, &generations_dir, &staging_dir] {
                directory.recheck()?;
            }
            for directory in &original_nested_directories {
                directory.recheck()?;
            }
            staged_active.recheck()?;
            if mode.parse_active(&staged_active.source)? != revision {
                return Err(invariant(
                    "staged ACTIVE changed before initial publication",
                ));
            }
            if mode == WorkspaceMode::Semantic {
                validate_generation_inventory(&generation, &expected_manifest_files)?;
            }
            if mode == WorkspaceMode::Semantic {
                recheck_permission_seals(&permission_seals)?;
            }
            let mut identities = vec![
                &root_dir.identity,
                &control_dir.identity,
                &generations_dir.identity,
                &staging_dir.identity,
                &lock_identity,
                &staged_active.identity,
            ];
            if mode == WorkspaceMode::Semantic {
                identities.push(&paths_input.identity);
            }
            identities.extend(
                published
                    .directories
                    .iter()
                    .map(|directory| &directory.identity),
            );
            identities.extend(published.texts.iter().map(|text| &text.identity));
            identities.extend(authenticated_sources.iter().map(|text| &text.identity));
            identities.extend(
                original_nested_directories
                    .iter()
                    .map(|directory| &directory.identity),
            );
            require_distinct_identities(&identities)?;
            require_same_volume(&identities)?;
            if mode == WorkspaceMode::Semantic {
                require_absent_destination(
                    &active_path,
                    "SPX-G153",
                    "initial ACTIVE destination already exists",
                )?;
            }
            Ok(())
        };
        if mode == WorkspaceMode::Semantic {
            complete_final_check()?;
            hook(InitializePoint::ActiveBeforeRename);
            complete_final_check()?;
            hook(InitializePoint::ActiveDestinationChecked);
            complete_final_check()?;
        } else {
            hook(InitializePoint::ActiveBeforeRename);
            complete_final_check()?;
        }
        let active_fingerprint =
            GenerationFingerprint::from_text(&mut staged_active, &active_stage)?;
        require_absent_destination(
            &active_path,
            if mode == WorkspaceMode::Semantic {
                "SPX-G153"
            } else {
                "SPX-I212"
            },
            "initial ACTIVE destination already exists",
        )?;
        #[cfg(windows)]
        drop(staged_active);
        if mode == WorkspaceMode::Ordinary {
            hook(InitializePoint::ActiveDestinationChecked);
        }
        publish_no_replace(
            &active_stage,
            &active_path,
            if mode == WorkspaceMode::Semantic {
                "SPX-I211"
            } else {
                "SPX-I212"
            },
            "cannot publish ACTIVE",
        )?;
        active_pivoted = true;
        if mode == WorkspaceMode::Semantic {
            let active_seal = permission_seals
                .iter_mut()
                .find(|seal| seal.path == active_stage)
                .ok_or_else(|| invariant("staged ACTIVE permission seal is absent"))?;
            active_seal.path = active_path.clone();
        }
        hook(InitializePoint::ActiveRelocated);
        published.recheck()?;
        let mut published_active = authenticate_text(&active_path, MAX_MANIFEST_BYTES, "SPX-I212")?;
        active_fingerprint.require_text_equivalent(&mut published_active, &active_path)?;
        if mode.parse_active(&published_active.source)? != revision {
            return Err(io(
                "SPX-I212",
                "post-pivot authentication is ambiguous: published ACTIVE revision mismatch",
            ));
        }
        let loaded = snapshot_authenticated_mode(&root, &control, Some(&lock_identity), mode)
            .map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        Diagnostic::io(
                            "SPX-I212",
                            format!(
                                "post-pivot authentication is ambiguous: {}",
                                diagnostic.message
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
            })?;
        if loaded.snapshot.workspace_revision() != revision {
            return Err(io(
                "SPX-I212",
                "post-pivot authentication is ambiguous: initialized workspace revision mismatch",
            ));
        }
        recheck_lock(&lock_path, &lock, &lock_identity).map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| {
                    Diagnostic::io(
                        "SPX-I212",
                        format!(
                            "post-pivot authentication is ambiguous: {}",
                            diagnostic.message
                        ),
                    )
                })
                .collect::<Vec<_>>()
        })?;
        if mode == WorkspaceMode::Semantic {
            published.recheck()?;
            published_active.recheck()?;
            active_fingerprint.require_text_equivalent(&mut published_active, &active_path)?;
            if mode.parse_active(&published_active.source)? != revision {
                return Err(invariant(
                    "published semantic ACTIVE changed after deep snapshot authentication",
                ));
            }
            paths_input.recheck()?;
            for source in &mut authenticated_sources {
                source.recheck()?;
            }
            for directory in [&root_dir, &control_dir, &generations_dir, &staging_dir] {
                directory.recheck()?;
            }
            for directory in &original_nested_directories {
                directory.recheck()?;
            }
            recheck_lock(&lock_path, &lock, &lock_identity)?;
            validate_control(&control)?;
            let (_, retained_directories) =
                count_directories_bounded(&control.join("generations"), MAX_RETAINED_GENERATIONS)?;
            let expected_generations = BTreeSet::from([revision_hex(&revision)?.to_owned()]);
            if inventory_names_from_directories(&retained_directories)? != expected_generations {
                return Err(invariant(
                    "published semantic generation inventory is not exact",
                ));
            }
            let (staging_count, staging_directories, staging_files) =
                validate_staging_inventory(&control.join("staging"))?;
            if staging_count != 0 || !staging_directories.is_empty() || !staging_files.is_empty() {
                return Err(invariant(
                    "published semantic staging inventory is not empty",
                ));
            }
            validate_generation_inventory(&generation, &expected_manifest_files)?;
            recheck_permission_seals(&permission_seals)?;
            let mut identities = vec![
                &root_dir.identity,
                &control_dir.identity,
                &generations_dir.identity,
                &staging_dir.identity,
                &lock_identity,
                &published_active.identity,
                &paths_input.identity,
            ];
            identities.extend(
                published
                    .directories
                    .iter()
                    .map(|directory| &directory.identity),
            );
            identities.extend(published.texts.iter().map(|text| &text.identity));
            identities.extend(authenticated_sources.iter().map(|text| &text.identity));
            identities.extend(
                original_nested_directories
                    .iter()
                    .map(|directory| &directory.identity),
            );
            require_distinct_identities(&identities)?;
            require_same_volume(&identities)?;
        }
        Ok(revision.clone())
    })();
    let result = if active_pivoted {
        result.map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| {
                    if diagnostic.code == "SPX-I212" {
                        diagnostic
                    } else {
                        Diagnostic::io(
                            "SPX-I212",
                            format!(
                                "post-pivot authentication is ambiguous: {}",
                                diagnostic.message
                            ),
                        )
                    }
                })
                .collect()
        })
    } else {
        result
    };
    match unlock_file(&lock) {
        Ok(()) => result,
        Err(diagnostics) if !active_pivoted => Err(diagnostics),
        Err(diagnostics) => Err(diagnostics
            .into_iter()
            .map(|diagnostic| {
                Diagnostic::io(
                    "SPX-I212",
                    format!(
                        "post-pivot authentication is ambiguous: {}",
                        diagnostic.message
                    ),
                )
            })
            .collect()),
    }
}

/// Authenticates ACTIVE and returns an immutable owned workspace snapshot.
pub fn snapshot(root: &Path) -> Result<WorkspaceSnapshot, Vec<Diagnostic>> {
    snapshot_inner(root, false)
}

pub(crate) fn acquire_semantic_read(
    root: &Path,
) -> Result<WorkspaceSemanticReadAuthority, Vec<Diagnostic>> {
    Ok(WorkspaceSemanticReadAuthority {
        guard: acquire_snapshot_mode(root, false, WorkspaceMode::Semantic)?,
    })
}

/// Previews one canonical workspace patch without creating candidate filesystem state.
pub fn preview(root: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    let build = build_read_owned(root, patch_path)?;
    let report = build.preview_json()?;
    build.recheck()?;
    Ok(report)
}

fn bounded_preview(
    base: &WorkspaceSnapshot,
    plan: &WorkspacePlanSummary,
) -> Result<String, Vec<Diagnostic>> {
    let mut used_preview = 0usize;
    loop {
        let (report, overflowed) = crate::bounded_output::with_limit(MAX_PREVIEW_BYTES, || {
            render_preview(
                base,
                &plan.patch,
                &plan.candidate_revision,
                &plan.previews,
                plan.usage,
                plan.candidate_manifest.len(),
                plan.candidate_bytes,
                plan.operations,
                used_preview,
            )
        });
        if overflowed || report.len() > MAX_PREVIEW_BYTES {
            return Err(limit("workspace preview exceeds 65536 bytes"));
        }
        if report.len() == used_preview {
            return Ok(report);
        }
        used_preview = report.len();
    }
}

/// Acquires one shared workspace snapshot, owns the exact bounded workspace
/// patch, and constructs one pure semantic plan without creating candidate
/// filesystem state. The returned authority retains both authenticated inputs
/// through the caller's final read-only recheck.
pub(crate) fn build_read_owned(
    root: &Path,
    patch_path: &Path,
) -> Result<WorkspaceReadBuild, Vec<Diagnostic>> {
    let guard = acquire_snapshot(root, false)?;
    let patch_input = authenticate_text(patch_path, MAX_WORKSPACE_PATCH_BYTES, "SPX-I209")?;
    let workspace_patch = parse_workspace_patch(&patch_input.source)?;
    build_read_from_inputs(guard, patch_input, workspace_patch)
}

pub(crate) fn acquire_evidence_guard(
    root: &Path,
) -> Result<WorkspaceEvidenceGuard, Vec<Diagnostic>> {
    Ok(WorkspaceEvidenceGuard {
        guard: Some(acquire_lock_only(root, false)?),
    })
}

pub(crate) fn reject_evidence_guard(
    mut authority: WorkspaceEvidenceGuard,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let Some(guard) = authority.guard.take() else {
        return invariant("workspace evidence guard was already consumed");
    };
    unlock_with_diagnostics(&guard.lock, diagnostics)
}

pub(crate) fn acquire_evidence_apply_guard(
    root: &Path,
    patch_path: &Path,
) -> Result<WorkspaceEvidenceApplyGuard, Vec<Diagnostic>> {
    let guard = acquire_lock_only(root, true)?;
    let patch_input = match authenticate_text(patch_path, MAX_WORKSPACE_PATCH_BYTES, "SPX-I209") {
        Ok(patch_input) => patch_input,
        Err(diagnostics) => {
            return Err(unlock_with_diagnostics(&guard.lock, diagnostics));
        }
    };
    Ok(WorkspaceEvidenceApplyGuard {
        guard: Some(guard),
        patch_input: Some(patch_input),
    })
}

pub(crate) fn reject_evidence_apply_guard(
    mut authority: WorkspaceEvidenceApplyGuard,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let Some(guard) = authority.guard.take() else {
        return invariant("workspace evidence apply guard was already consumed");
    };
    unlock_with_diagnostics(&guard.lock, diagnostics)
}

pub(crate) fn finish_evidence_apply_guard(
    mut authority: WorkspaceEvidenceApplyGuard,
) -> Result<WorkspaceReadBuild, Vec<Diagnostic>> {
    let lock_guard = authority
        .guard
        .take()
        .ok_or_else(|| invariant("workspace evidence apply guard was already consumed"))?;
    let patch_input = match authority.patch_input.take() {
        Some(patch_input) => patch_input,
        None => {
            return Err(unlock_with_diagnostics(
                &lock_guard.lock,
                invariant("workspace evidence apply patch was already consumed"),
            ));
        }
    };
    let workspace_patch = match parse_workspace_patch(&patch_input.source) {
        Ok(workspace_patch) => workspace_patch,
        Err(diagnostics) => {
            return Err(unlock_with_diagnostics(&lock_guard.lock, diagnostics));
        }
    };
    let guard = finish_snapshot_guard(lock_guard)?;
    build_read_from_inputs(guard, patch_input, workspace_patch)
}

pub(crate) fn build_read_owned_from_guard(
    mut authority: WorkspaceEvidenceGuard,
    patch_path: &Path,
) -> Result<WorkspaceReadBuild, Vec<Diagnostic>> {
    let lock_guard = authority
        .guard
        .take()
        .ok_or_else(|| invariant("workspace evidence guard was already consumed"))?;
    let patch_input = match authenticate_text(patch_path, MAX_WORKSPACE_PATCH_BYTES, "SPX-I209") {
        Ok(patch_input) => patch_input,
        Err(diagnostics) => {
            return Err(unlock_with_diagnostics(&lock_guard.lock, diagnostics));
        }
    };
    let workspace_patch = match parse_workspace_patch_with_minimum(&patch_input.source, 1) {
        Ok(workspace_patch) => workspace_patch,
        Err(diagnostics) => {
            return Err(unlock_with_diagnostics(&lock_guard.lock, diagnostics));
        }
    };
    let guard = finish_snapshot_guard(lock_guard)?;
    build_read_from_inputs(guard, patch_input, workspace_patch)
}

fn build_read_from_inputs(
    guard: WorkspaceGuard,
    patch_input: AuthenticatedText,
    workspace_patch: WorkspacePatch,
) -> Result<WorkspaceReadBuild, Vec<Diagnostic>> {
    let plan = match build_workspace_plan(&guard.snapshot, workspace_patch) {
        Ok(plan) => plan,
        Err(diagnostics) => {
            return Err(unlock_with_diagnostics(&guard.lock, diagnostics));
        }
    };
    let WorkspacePlan {
        summary,
        preflights,
    } = plan;
    if let Err(diagnostics) = validate_evidence_preflight_paths(&summary, &preflights) {
        return Err(unlock_with_diagnostics(&guard.lock, diagnostics));
    }
    Ok(WorkspaceReadBuild {
        guard,
        patch_input,
        summary,
        preflights: Some(preflights),
    })
}

fn validate_evidence_preflight_paths(
    summary: &WorkspacePlanSummary,
    preflights: &[WorkspaceEvidencePreflight],
) -> Result<(), Vec<Diagnostic>> {
    let expected = summary
        .patch
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let actual = preflights
        .iter()
        .map(|item| item.path.clone())
        .collect::<Vec<_>>();
    if preflights.len() != summary.changed_count {
        return Err(invariant(
            "workspace evidence preflight paths differ from the semantic plan",
        ));
    }
    require_exact_path_association(&expected, &actual)?;
    Ok(())
}

fn require_exact_path_association(
    expected: &[String],
    actual: &[String],
) -> Result<(), Vec<Diagnostic>> {
    let expected_set = expected.iter().collect::<BTreeSet<_>>();
    let actual_set = actual.iter().collect::<BTreeSet<_>>();
    if expected_set.len() != expected.len()
        || actual_set.len() != actual.len()
        || expected_set != actual_set
    {
        return Err(invariant(
            "workspace evidence preflight paths differ from the semantic plan",
        ));
    }
    Ok(())
}

fn build_workspace_plan(
    base: &WorkspaceSnapshot,
    workspace_patch: WorkspacePatch,
) -> Result<WorkspacePlan, Vec<Diagnostic>> {
    if workspace_patch.base != base.workspace_revision {
        return Err(stale("workspace patch base is stale"));
    }
    let mut base_by_path = base
        .files
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    let mut candidate = Vec::with_capacity(base.files.len());
    let mut previews = BTreeMap::new();
    let mut preflights = Vec::with_capacity(workspace_patch.files.len());
    let mut operations = 0usize;
    let changed_paths = workspace_patch
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut candidate_bytes = base
        .files
        .iter()
        .filter(|file| !changed_paths.contains(file.path.as_str()))
        .try_fold(0usize, |sum, file| sum.checked_add(file.source.len()))
        .ok_or_else(|| limit("candidate source byte count overflow"))?;
    let mut remaining_ast = (MAX_DECLARATIONS, MAX_CALLABLES, MAX_CALL_SITES);
    for file in base
        .files
        .iter()
        .filter(|file| !changed_paths.contains(file.path.as_str()))
    {
        let program = parse(&file.source, Path::new(&file.path)).map_err(|error| vec![error])?;
        let counts = crate::review::workspace_ast_counts(&program)?;
        remaining_ast.0 = remaining_ast
            .0
            .checked_sub(counts.0)
            .ok_or_else(|| limit("workspace declarations exceed 4096"))?;
        remaining_ast.1 = remaining_ast
            .1
            .checked_sub(counts.1)
            .ok_or_else(|| limit("workspace callables exceed 1024"))?;
        remaining_ast.2 = remaining_ast
            .2
            .checked_sub(counts.2)
            .ok_or_else(|| limit("workspace call sites exceed 65536"))?;
    }
    for changed in &workspace_patch.files {
        let base_file = base_by_path
            .remove(&changed.path)
            .ok_or_else(|| invariant("workspace patch path is outside the managed path set"))?;
        let preflight = patch::preflight_workspace_owned(
            base_file.source.clone(),
            changed.patch.clone(),
            PathBuf::from(&changed.path),
            patch::WorkspacePreflightLimits::new(
                MAX_OPERATIONS.saturating_sub(operations),
                MAX_TOTAL_SOURCE_BYTES.saturating_sub(candidate_bytes),
                remaining_ast.0,
                remaining_ast.1,
                remaining_ast.2,
            ),
        )?;
        operations = operations
            .checked_add(preflight.operations().len())
            .ok_or_else(|| limit("workspace operation count overflow"))?;
        candidate_bytes = candidate_bytes
            .checked_add(preflight.canonical_candidate().len())
            .ok_or_else(|| limit("candidate source byte count overflow"))?;
        if candidate_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(limit("workspace candidates exceed 16777216 bytes"));
        }
        let counts = crate::review::workspace_ast_counts(preflight.candidate())?;
        remaining_ast.0 = remaining_ast
            .0
            .checked_sub(counts.0)
            .ok_or_else(|| limit("workspace declarations exceed 4096"))?;
        remaining_ast.1 = remaining_ast
            .1
            .checked_sub(counts.1)
            .ok_or_else(|| limit("workspace callables exceed 1024"))?;
        remaining_ast.2 = remaining_ast
            .2
            .checked_sub(counts.2)
            .ok_or_else(|| limit("workspace call sites exceed 65536"))?;
        previews.insert(
            changed.path.clone(),
            (
                preflight.schema_label().to_owned(),
                domain_digest(
                    "semaprax.semantic-review.patch-digest.v1\0",
                    changed.patch.as_bytes(),
                ),
                base_file.source_graph_schema.clone(),
                graph_schema_for(preflight.candidate())?.to_owned(),
                preflight.base_revision().to_owned(),
                preflight.candidate_revision().to_owned(),
            ),
        );
        candidate.push((
            changed.path.clone(),
            preflight.canonical_candidate().to_owned(),
        ));
        preflights.push(WorkspaceEvidencePreflight {
            path: changed.path.clone(),
            preflight,
        });
    }
    for (path, file) in base_by_path {
        candidate.push((path, file.source.clone()));
    }
    candidate.sort_by(|left, right| left.0.cmp(&right.0));
    let candidate = file_facts(candidate, true)?;
    validate_workspace_facts(&candidate)?;
    let total_candidate = candidate
        .iter()
        .map(|fact| fact.source.len())
        .sum::<usize>();
    if total_candidate > MAX_TOTAL_SOURCE_BYTES {
        return Err(limit("workspace candidates exceed 16777216 bytes"));
    }
    let candidate_manifest = bounded_manifest(&candidate)?;
    let candidate_revision = workspace_revision(&candidate_manifest);
    let usage = usage(&candidate)?;
    let changed_count = workspace_patch.files.len();
    Ok(WorkspacePlan {
        summary: WorkspacePlanSummary {
            patch: workspace_patch,
            candidate,
            previews,
            candidate_manifest,
            candidate_revision,
            usage,
            candidate_bytes,
            operations,
            changed_count,
        },
        preflights,
    })
}

/// Applies one workspace transaction by atomically replacing only `ACTIVE`.
pub fn apply(root: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    apply_with_hook(root, patch_path, |_, _, _, _| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplyPoint {
    AfterPatchRead,
    AfterCandidatePrepared,
    AfterActiveStaged,
    BeforeFirstFinalCheck,
    BeforeSecondFinalCheck,
    BeforeActiveReplace,
    AfterActiveReplace,
}

fn apply_with_hook(
    root: &Path,
    patch_path: &Path,
    mut hook: impl FnMut(ApplyPoint, &Path, Option<&Path>, Option<&Path>) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let guard = acquire_snapshot(root, true)?;
    let prepared = (|| {
        let active_path = guard.control.join("ACTIVE");
        let patch_input = authenticate_text(patch_path, MAX_WORKSPACE_PATCH_BYTES, "SPX-I209")?;
        hook(ApplyPoint::AfterPatchRead, &active_path, None, None)
            .map_err(|error| io("SPX-I209", format!("post-read hook failed: {error}")))?;
        let patch = parse_workspace_patch(&patch_input.source)?;
        let plan = build_workspace_plan(&guard.snapshot, patch)?;
        let WorkspacePlan {
            summary: plan,
            preflights: _,
        } = plan;
        Ok((patch_input, plan))
    })();
    let (patch_input, plan) = match prepared {
        Ok(prepared) => prepared,
        Err(diagnostics) => return Err(unlock_with_diagnostics(&guard.lock, diagnostics)),
    };
    commit_workspace_authority_with_hook(
        WorkspaceCommitAuthority {
            guard,
            patch_input,
            plan,
        },
        hook,
    )
}

pub(crate) fn commit_workspace_authority_with_hook(
    authority: WorkspaceCommitAuthority,
    mut hook: impl FnMut(ApplyPoint, &Path, Option<&Path>, Option<&Path>) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let WorkspaceCommitAuthority {
        mut guard,
        mut patch_input,
        plan,
    } = authority;
    let active_path = guard.control.join("ACTIVE");
    let mut active_replaced = false;
    let result = (|| {
        let mut candidate =
            ensure_candidate_generation(&mut guard, &mut patch_input, &plan, |_, _, _| {})?;
        hook(
            ApplyPoint::AfterCandidatePrepared,
            &active_path,
            None,
            Some(&candidate.path),
        )
        .map_err(|error| io("SPX-I211", format!("candidate hook failed: {error}")))?;

        let candidate_name = revision_hex(&plan.candidate_revision)?.to_owned();
        let mut generation_names = guard.generation_names.clone();
        generation_names.insert(candidate_name);
        let staging_root = guard.control.join("staging");
        guard.recheck_phase_inventory(&generation_names, &guard.staging_names, Some(&candidate))?;
        let (_, occupied_directories, occupied_files) = validate_staging_inventory(&staging_root)?;
        let occupied = inventory_names_from_directories(&occupied_directories)?
            .into_iter()
            .chain(inventory_names_from_texts(&occupied_files)?)
            .collect::<BTreeSet<_>>();
        let ordinal = (0..MAX_STAGING_ATTEMPTS)
            .find(|ordinal| !occupied.contains(&ordinal.to_string()))
            .ok_or_else(|| limit("workspace retains 32 staging attempts"))?;
        let active_stage_path = staging_root.join(ordinal.to_string());
        let active_source = render_root(&plan.candidate_revision);
        let mut active_stage = write_new_text(
            &active_stage_path,
            &active_source,
            MAX_MANIFEST_BYTES,
            "candidate ACTIVE pointer",
        )?;
        let active_permissions = guard
            .texts
            .iter()
            .find(|text| text.path == active_path)
            .ok_or_else(|| invariant("authenticated ACTIVE handle is unavailable"))?
            .file
            .metadata()
            .map_err(|error| {
                io(
                    "SPX-I211",
                    format!("cannot inspect ACTIVE permissions: {error}"),
                )
            })?
            .permissions();
        active_stage
            .file
            .set_permissions(active_permissions.clone())
            .and_then(|_| active_stage.file.sync_all())
            .map_err(|error| {
                io(
                    "SPX-I211",
                    format!("cannot preserve ACTIVE permissions: {error}"),
                )
            })?;
        active_stage.recheck()?;
        let mut staging_names = guard.staging_names.clone();
        staging_names.insert(ordinal.to_string());
        let final_facts = FinalApplyFacts {
            plan: &plan,
            generation_names: &generation_names,
            staging_names: &staging_names,
            active_permissions: &active_permissions,
        };
        hook(
            ApplyPoint::AfterActiveStaged,
            &active_path,
            Some(&active_stage_path),
            Some(&candidate.path),
        )
        .map_err(|error| io("SPX-I211", format!("ACTIVE staging hook failed: {error}")))?;

        hook(
            ApplyPoint::BeforeFirstFinalCheck,
            &active_path,
            Some(&active_stage_path),
            Some(&candidate.path),
        )
        .map_err(|error| {
            io(
                "SPX-I211",
                format!("first final-check hook failed: {error}"),
            )
        })?;
        final_apply_recheck(
            &mut guard,
            &mut patch_input,
            &mut candidate,
            &mut active_stage,
            &final_facts,
        )?;
        hook(
            ApplyPoint::BeforeSecondFinalCheck,
            &active_path,
            Some(&active_stage_path),
            Some(&candidate.path),
        )
        .map_err(|error| {
            io(
                "SPX-I211",
                format!("second final-check hook failed: {error}"),
            )
        })?;
        hook(
            ApplyPoint::BeforeActiveReplace,
            &active_path,
            Some(&active_stage_path),
            Some(&candidate.path),
        )
        .map_err(|error| io("SPX-I211", format!("ACTIVE replacement rejected: {error}")))?;
        final_apply_recheck(
            &mut guard,
            &mut patch_input,
            &mut candidate,
            &mut active_stage,
            &final_facts,
        )?;
        let active_fingerprint =
            GenerationFingerprint::from_text(&mut active_stage, &active_stage_path)?;
        #[cfg(windows)]
        drop(active_stage);
        std::fs::rename(&active_stage_path, &active_path).map_err(|error| {
            io(
                "SPX-I211",
                format!("cannot atomically replace ACTIVE: {error}"),
            )
        })?;
        active_replaced = true;
        hook(
            ApplyPoint::AfterActiveReplace,
            &active_path,
            Some(&active_stage_path),
            Some(&candidate.path),
        )
        .map_err(|error| final_uncertainty(format!("post-pivot hook failed: {error}")))?;
        let mut published_active = authenticate_text(&active_path, MAX_MANIFEST_BYTES, "SPX-I212")
            .map_err(map_final_uncertainty)?;
        active_fingerprint
            .require_text_equivalent(&mut published_active, &active_path)
            .map_err(map_final_uncertainty)?;
        if !permissions_equal(
            &active_permissions,
            &published_active
                .file
                .metadata()
                .map_err(|error| {
                    final_uncertainty(format!(
                        "cannot inspect published ACTIVE permissions: {error}"
                    ))
                })?
                .permissions(),
        ) {
            return Err(final_uncertainty(
                "published ACTIVE permissions differ from the authenticated base",
            ));
        }
        let loaded =
            snapshot_authenticated(&guard.root, &guard.control, Some(&guard.lock_identity))
                .map_err(map_final_uncertainty)?;
        validate_post_pivot_snapshot(&loaded.snapshot, &plan)?;
        validate_post_pivot_inventory(&guard, &generation_names)?;
        recheck_lock(&guard.lock_path, &guard.lock, &guard.lock_identity)
            .map_err(map_final_uncertainty)?;
        Ok(plan.candidate_revision.clone())
    })();
    finish_commit_unlock(&guard.lock, active_replaced, result)
}

fn finish_commit_unlock(
    lock: &File,
    active_replaced: bool,
    result: Result<String, Vec<Diagnostic>>,
) -> Result<String, Vec<Diagnostic>> {
    match unlock_file(lock) {
        Ok(()) => result,
        Err(unlock_diagnostics) if active_replaced => {
            Err(map_final_uncertainty(unlock_diagnostics))
        }
        Err(unlock_diagnostics) => Err(unlock_diagnostics),
    }
}

struct FinalApplyFacts<'a> {
    plan: &'a WorkspacePlanSummary,
    generation_names: &'a BTreeSet<String>,
    staging_names: &'a BTreeSet<String>,
    active_permissions: &'a std::fs::Permissions,
}

fn final_apply_recheck(
    guard: &mut WorkspaceGuard,
    patch_input: &mut AuthenticatedText,
    candidate: &mut PreparedGeneration,
    active_stage: &mut AuthenticatedText,
    facts: &FinalApplyFacts<'_>,
) -> Result<(), Vec<Diagnostic>> {
    guard.recheck_base_authority()?;
    patch_input.recheck()?;
    if patch_input.source != facts.plan.patch.source {
        return Err(invariant(
            "owned workspace patch changed after semantic planning",
        ));
    }
    candidate.recheck()?;
    let candidate_fingerprint = candidate.fingerprint()?;
    let mut exact_candidate = authenticate_expected_generation(&candidate.path, facts.plan, guard)
        .map_err(map_post_publication_candidate_diagnostics)?;
    candidate_fingerprint.require_equivalent(&mut exact_candidate)?;
    if workspace_revision(&facts.plan.candidate_manifest) != facts.plan.candidate_revision {
        return Err(invariant(
            "candidate manifest no longer authenticates the planned workspace revision",
        ));
    }
    active_stage.recheck()?;
    let old_active_permissions = guard
        .texts
        .iter()
        .find(|text| text.path == guard.control.join("ACTIVE"))
        .ok_or_else(|| invariant("authenticated ACTIVE handle is unavailable"))?
        .file
        .metadata()
        .map_err(|error| {
            io(
                "SPX-I209",
                format!("cannot inspect ACTIVE permissions: {error}"),
            )
        })?
        .permissions();
    let staged_active_permissions = active_stage
        .file
        .metadata()
        .map_err(|error| {
            io(
                "SPX-I211",
                format!("cannot inspect staged ACTIVE permissions: {error}"),
            )
        })?
        .permissions();
    if !permissions_equal(facts.active_permissions, &old_active_permissions)
        || !permissions_equal(facts.active_permissions, &staged_active_permissions)
    {
        return Err(invariant(
            "ACTIVE permissions changed during final workspace authentication",
        ));
    }
    let expected_active = render_root(&facts.plan.candidate_revision);
    if active_stage.source != expected_active
        || parse_root(&active_stage.source)? != facts.plan.candidate_revision
    {
        return Err(invariant(
            "candidate ACTIVE pointer differs from the planned workspace revision",
        ));
    }
    guard.recheck_phase_inventory(facts.generation_names, facts.staging_names, Some(candidate))?;
    Ok(())
}

fn validate_post_pivot_snapshot(
    snapshot: &WorkspaceSnapshot,
    plan: &WorkspacePlanSummary,
) -> Result<(), Vec<Diagnostic>> {
    if snapshot.workspace_revision != plan.candidate_revision
        || snapshot.files.len() != plan.candidate.len()
    {
        return Err(final_uncertainty(
            "published workspace snapshot differs from the planned candidate",
        ));
    }
    for (actual, expected) in snapshot.files.iter().zip(&plan.candidate) {
        if actual.path != expected.path
            || actual.source_graph_schema != expected.source_graph_schema
            || actual.source_revision != expected.source_revision
            || actual.source_digest != expected.source_digest
            || actual.source != expected.source
        {
            return Err(final_uncertainty(
                "published workspace file differs from the planned candidate",
            ));
        }
    }
    Ok(())
}

fn validate_post_pivot_inventory(
    guard: &WorkspaceGuard,
    generation_names: &BTreeSet<String>,
) -> Result<(), Vec<Diagnostic>> {
    let (_, generations) =
        count_directories_bounded(&guard.control.join("generations"), MAX_RETAINED_GENERATIONS)
            .map_err(map_final_uncertainty)?;
    if inventory_names_from_directories(&generations).map_err(map_final_uncertainty)?
        != *generation_names
    {
        return Err(final_uncertainty(
            "published workspace generation inventory differs from the final checked set",
        ));
    }
    let (_, staging_directories, staging_files) =
        validate_staging_inventory(&guard.control.join("staging"))
            .map_err(map_final_uncertainty)?;
    let staging_names = inventory_names_from_directories(&staging_directories)
        .map_err(map_final_uncertainty)?
        .into_iter()
        .chain(inventory_names_from_texts(&staging_files).map_err(map_final_uncertainty)?)
        .collect::<BTreeSet<_>>();
    if staging_names != guard.staging_names {
        return Err(final_uncertainty(
            "published workspace staging inventory differs from the final checked set",
        ));
    }
    Ok(())
}

fn permissions_equal(left: &std::fs::Permissions, right: &std::fs::Permissions) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        left.mode() == right.mode()
    }
    #[cfg(windows)]
    {
        left.readonly() == right.readonly()
    }
    #[cfg(not(any(unix, windows)))]
    {
        left.readonly() == right.readonly()
    }
}

struct PermissionSeal {
    path: PathBuf,
    permissions: std::fs::Permissions,
}

fn capture_permission_seals<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<Vec<PermissionSeal>, Vec<Diagnostic>> {
    paths
        .into_iter()
        .map(|path| {
            let permissions = std::fs::symlink_metadata(path)
                .map_err(|error| {
                    io(
                        "SPX-I209",
                        format!("cannot inspect workspace permissions: {error}"),
                    )
                })?
                .permissions();
            Ok(PermissionSeal {
                path: path.to_path_buf(),
                permissions,
            })
        })
        .collect()
}

fn recheck_permission_seals(seals: &[PermissionSeal]) -> Result<(), Vec<Diagnostic>> {
    for seal in seals {
        let current = std::fs::symlink_metadata(&seal.path)
            .map_err(|error| {
                io(
                    "SPX-I209",
                    format!("cannot re-inspect workspace permissions: {error}"),
                )
            })?
            .permissions();
        if !permissions_equal(&seal.permissions, &current) {
            return Err(invariant(
                "workspace permissions changed during final authentication",
            ));
        }
    }
    Ok(())
}

fn map_final_uncertainty(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            Diagnostic::io(
                "SPX-I212",
                format!(
                    "workspace final authority is ambiguous: {}",
                    diagnostic.message
                ),
            )
        })
        .collect()
}

fn final_uncertainty(message: impl Into<String>) -> Vec<Diagnostic> {
    io("SPX-I212", message)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum GenerationPoint {
    AfterSlotCreate,
    AfterManifestWrite,
    AfterFilesWrite,
    BeforeStageValidation,
    BeforeGenerationPublish,
    DestinationChecked,
    AfterGenerationPublish,
}

#[allow(dead_code)]
fn ensure_candidate_generation(
    guard: &mut WorkspaceGuard,
    patch_input: &mut AuthenticatedText,
    plan: &WorkspacePlanSummary,
    mut hook: impl FnMut(GenerationPoint, &Path, &Path),
) -> Result<PreparedGeneration, Vec<Diagnostic>> {
    if !guard.exclusive {
        return Err(invariant(
            "candidate generation construction requires the exclusive workspace lock",
        ));
    }
    if patch_input.source != plan.patch.source {
        return Err(invariant(
            "owned workspace patch differs from the planned transaction",
        ));
    }
    if plan.candidate.len() != guard.snapshot.files.len()
        || plan.changed_count != plan.patch.files.len()
    {
        return Err(invariant("workspace plan cardinality is inconsistent"));
    }
    guard.recheck()?;
    patch_input.recheck()?;

    guard.recheck_phase_inventory(&guard.generation_names, &guard.staging_names, None)?;
    let generations = guard.control.join("generations");
    let candidate_name = revision_hex(&plan.candidate_revision)?;
    let destination = generations.join(candidate_name);
    match std::fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata_is_reparse(&metadata)
            {
                return Err(io(
                    "SPX-I211",
                    "candidate generation path conflicts with a foreign object",
                ));
            }
            let mut prepared = authenticate_expected_generation(&destination, plan, guard)?;
            prepared.recheck()?;
            guard.recheck_base_authority()?;
            patch_input.recheck()?;
            guard.recheck_phase_inventory(
                &guard.generation_names,
                &guard.staging_names,
                Some(&prepared),
            )?;
            return Ok(prepared);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(io(
                "SPX-I211",
                format!("cannot inspect candidate generation: {error}"),
            ));
        }
    }
    if guard.snapshot.retained_generations >= MAX_RETAINED_GENERATIONS {
        return Err(limit(
            "workspace retains 32 generations; no candidate can be added",
        ));
    }

    let staging = guard.control.join("staging");
    let (_, occupied_directories, occupied_files) = validate_staging_inventory(&staging)?;
    let occupied = occupied_directories
        .iter()
        .filter_map(|entry| entry.path.file_name()?.to_str()?.parse::<usize>().ok())
        .chain(
            occupied_files
                .iter()
                .filter_map(|entry| entry.path.file_name()?.to_str()?.parse::<usize>().ok()),
        )
        .collect::<BTreeSet<_>>();
    let ordinal = (0..MAX_STAGING_ATTEMPTS)
        .find(|ordinal| !occupied.contains(ordinal))
        .ok_or_else(|| limit("workspace retains 32 staging attempts"))?;
    let slot = staging.join(ordinal.to_string());
    std::fs::create_dir(&slot).map_err(|error| {
        io(
            "SPX-I211",
            format!("cannot create staging attempt {ordinal}: {error}"),
        )
    })?;
    let slot_directory = authenticate_directory_held(&slot)?;
    hook(GenerationPoint::AfterSlotCreate, &slot, &destination);
    let mut staged_names = guard.staging_names.clone();
    staged_names.insert(ordinal.to_string());
    guard.recheck_phase_inventory(&guard.generation_names, &staged_names, None)?;

    let mut prepared = write_generation_held(
        &slot,
        &plan.candidate_manifest,
        &plan.candidate,
        slot_directory,
        &destination,
        &mut hook,
    )?;
    hook(GenerationPoint::BeforeStageValidation, &slot, &destination);
    prepared.recheck()?;
    let mut authenticated = authenticate_expected_generation(&slot, plan, guard)?;
    authenticated.recheck()?;
    guard.recheck_base_authority()?;
    patch_input.recheck()?;
    guard.recheck_phase_inventory(&guard.generation_names, &staged_names, Some(&authenticated))?;
    hook(
        GenerationPoint::BeforeGenerationPublish,
        &slot,
        &destination,
    );
    prepared.recheck()?;
    authenticated.recheck()?;
    guard.recheck_base_authority()?;
    patch_input.recheck()?;
    guard.recheck_phase_inventory(&guard.generation_names, &staged_names, Some(&authenticated))?;
    let staged_fingerprint = authenticated.fingerprint()?;
    require_absent_destination(
        &destination,
        "SPX-I211",
        "candidate generation appeared before publication",
    )?;
    #[cfg(windows)]
    {
        drop(prepared);
        drop(authenticated);
    }
    hook(GenerationPoint::DestinationChecked, &slot, &destination);
    publish_no_replace(
        &slot,
        &destination,
        "SPX-I211",
        "cannot publish complete candidate generation",
    )?;
    hook(GenerationPoint::AfterGenerationPublish, &slot, &destination);
    let mut published = authenticate_expected_generation(&destination, plan, guard)
        .map_err(map_post_publication_candidate_diagnostics)?;
    staged_fingerprint.require_equivalent(&mut published)?;
    published.recheck()?;
    guard.recheck_base_authority()?;
    patch_input.recheck()?;
    let mut published_names = guard.generation_names.clone();
    published_names.insert(candidate_name.to_owned());
    guard.recheck_phase_inventory(&published_names, &guard.staging_names, Some(&published))?;
    Ok(published)
}

impl PreparedGeneration {
    #[allow(dead_code)]
    fn recheck(&mut self) -> Result<(), Vec<Diagnostic>> {
        authenticate_directory(&self.path)?;
        for directory in &self.directories {
            directory.recheck()?;
        }
        for text in &mut self.texts {
            text.recheck()?;
        }
        Ok(())
    }

    fn fingerprint(&mut self) -> Result<GenerationFingerprint, Vec<Diagnostic>> {
        self.recheck()?;
        let mut entries = Vec::with_capacity(self.directories.len() + self.texts.len());
        for directory in &self.directories {
            entries.push(RelocationEntry {
                relative_path: relative_relocation_path(&self.path, &directory.path)?,
                identity: directory.identity,
                object: RelocationObject::Directory,
            });
        }
        for text in &self.texts {
            entries.push(RelocationEntry {
                relative_path: relative_relocation_path(&self.path, &text.path)?,
                identity: text.identity,
                object: RelocationObject::Text(text.source.as_bytes().to_vec()),
            });
        }
        canonical_fingerprint(entries)
    }
}

impl GenerationFingerprint {
    fn from_text(text: &mut AuthenticatedText, root: &Path) -> Result<Self, Vec<Diagnostic>> {
        text.recheck()?;
        canonical_fingerprint(vec![RelocationEntry {
            relative_path: relative_relocation_path(root, &text.path)?,
            identity: text.identity,
            object: RelocationObject::Text(text.source.as_bytes().to_vec()),
        }])
    }

    fn require_equivalent(
        &self,
        published: &mut PreparedGeneration,
    ) -> Result<(), Vec<Diagnostic>> {
        let actual = published.fingerprint()?;
        if actual != *self {
            return Err(invariant(
                "published generation differs from the exact staged identity and byte map",
            ));
        }
        Ok(())
    }

    fn require_text_equivalent(
        &self,
        published: &mut AuthenticatedText,
        root: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
        let actual = Self::from_text(published, root)?;
        if actual != *self {
            return Err(io(
                "SPX-I212",
                "published ACTIVE differs from the exact staged identity and byte map",
            ));
        }
        Ok(())
    }
}

fn relative_relocation_path(root: &Path, path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| invariant("relocation proof path escaped its staged root"))
}

fn canonical_fingerprint(
    mut entries: Vec<RelocationEntry>,
) -> Result<GenerationFingerprint, Vec<Diagnostic>> {
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if entries
        .windows(2)
        .any(|pair| pair[0].relative_path == pair[1].relative_path)
    {
        return Err(invariant(
            "relocation proof contains duplicate relative paths",
        ));
    }
    Ok(GenerationFingerprint { entries })
}

fn require_absent_destination(
    path: &Path,
    code: &'static str,
    message: &'static str,
) -> Result<(), Vec<Diagnostic>> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(io(code, message)),
        Err(error) => Err(io(
            code,
            format!("cannot inspect publication destination: {error}"),
        )),
    }
}

fn publish_no_replace(
    source: &Path,
    destination: &Path,
    code: &'static str,
    description: &'static str,
) -> Result<(), Vec<Diagnostic>> {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    ))]
    {
        use std::os::unix::ffi::OsStrExt as _;

        rustix::fs::renameat_with(
            rustix::fs::CWD,
            source.as_os_str().as_bytes(),
            rustix::fs::CWD,
            destination.as_os_str().as_bytes(),
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|error| io(code, format!("{description}: {error}")))
    }
    #[cfg(windows)]
    {
        renamore::rename_exclusive(source, destination)
            .map_err(|error| io(code, format!("{description}: {error}")))
    }
    #[cfg(all(
        unix,
        not(any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        ))
    ))]
    {
        let _ = (source, destination);
        Err(io(
            code,
            format!("{description}: atomic no-replace rename is unavailable on this Unix target"),
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, destination);
        Err(io(
            code,
            format!("{description}: atomic no-replace rename is unavailable on this target"),
        ))
    }
}

#[allow(dead_code)]
fn authenticate_expected_generation(
    path: &Path,
    plan: &WorkspacePlanSummary,
    guard: &WorkspaceGuard,
) -> Result<PreparedGeneration, Vec<Diagnostic>> {
    let mut prepared = authenticate_generation_deep(
        path,
        &plan.candidate_manifest,
        &plan.candidate,
        &plan.candidate_revision,
    )?;
    for candidate in &prepared.directories {
        if guard
            .directories
            .iter()
            .any(|entry| entry.identity == candidate.identity && entry.path != candidate.path)
            || guard
                .texts
                .iter()
                .any(|entry| entry.identity == candidate.identity)
        {
            return Err(invariant(
                "candidate generation aliases workspace authority",
            ));
        }
    }
    for candidate in &prepared.texts {
        if guard
            .directories
            .iter()
            .any(|entry| entry.identity == candidate.identity)
            || guard
                .texts
                .iter()
                .any(|entry| entry.identity == candidate.identity && entry.path != candidate.path)
        {
            return Err(invariant(
                "candidate generation aliases workspace authority",
            ));
        }
    }
    let mut identities = Vec::new();
    identities.push(&guard.lock_identity);
    identities.extend(guard.directories.iter().map(|entry| &entry.identity));
    identities.extend(guard.texts.iter().map(|entry| &entry.identity));
    identities.extend(prepared.directories.iter().filter_map(|candidate| {
        (!guard
            .directories
            .iter()
            .any(|entry| entry.path == candidate.path))
        .then_some(&candidate.identity)
    }));
    identities.extend(prepared.texts.iter().filter_map(|candidate| {
        (!guard.texts.iter().any(|entry| entry.path == candidate.path))
            .then_some(&candidate.identity)
    }));
    require_distinct_identities(&identities)?;
    require_same_volume(&identities)?;
    prepared.recheck()?;
    Ok(prepared)
}

fn map_post_publication_candidate_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let structural = diagnostic.code == "SPX-I209"
                && (matches!(
                    diagnostic.message.as_str(),
                    "workspace directory must be real and non-aliased"
                        | "workspace input must be a real regular file"
                        | "workspace input must be a regular file"
                ) || (diagnostic.message.starts_with("managed path `")
                    && diagnostic
                        .message
                        .ends_with("contains a non-directory or alias")));
            if structural {
                Diagnostic::io("SPX-G153", diagnostic.message)
            } else {
                diagnostic
            }
        })
        .collect()
}

#[cfg(test)]
fn prepare_candidate_generation_with_hook(
    root: &Path,
    patch_path: &Path,
    hook: impl FnMut(GenerationPoint, &Path, &Path),
) -> Result<String, Vec<Diagnostic>> {
    let mut guard = acquire_snapshot(root, true)?;
    let mut patch_input = authenticate_text(patch_path, MAX_WORKSPACE_PATCH_BYTES, "SPX-I209")?;
    let patch = parse_workspace_patch(&patch_input.source)?;
    let plan = build_workspace_plan(&guard.snapshot, patch)?;
    let revision = plan.candidate_revision.clone();
    let mut prepared = ensure_candidate_generation(&mut guard, &mut patch_input, &plan, hook)?;
    prepared.recheck()?;
    Ok(revision)
}

fn snapshot_inner(root: &Path, exclusive: bool) -> Result<WorkspaceSnapshot, Vec<Diagnostic>> {
    let mut guard = acquire_snapshot(root, exclusive)?;
    guard.recheck()?;
    unlock_file(&guard.lock)?;
    Ok(guard.snapshot)
}

fn acquire_snapshot(root: &Path, exclusive: bool) -> Result<WorkspaceGuard, Vec<Diagnostic>> {
    acquire_snapshot_mode(root, exclusive, WorkspaceMode::Ordinary)
}

fn acquire_snapshot_mode(
    root: &Path,
    exclusive: bool,
    mode: WorkspaceMode,
) -> Result<WorkspaceGuard, Vec<Diagnostic>> {
    finish_snapshot_guard_mode(acquire_lock_only(root, exclusive)?, mode)
}

fn acquire_lock_only(root: &Path, exclusive: bool) -> Result<WorkspaceLockGuard, Vec<Diagnostic>> {
    let root = canonical_root(root)?;
    let root_identity = authenticate_directory(&root)?;
    let control = root.join(CONTROL);
    authenticate_directory(&control)?;
    let lock_path = control.join("LOCK");
    let lock_metadata = std::fs::symlink_metadata(&lock_path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot inspect workspace LOCK: {error}"),
        )
    })?;
    if !lock_metadata.is_file()
        || lock_metadata.file_type().is_symlink()
        || metadata_is_reparse(&lock_metadata)
    {
        return Err(io("SPX-I209", "workspace LOCK must be a real regular file"));
    }
    require_single_link_path(&lock_path, "SPX-I209")?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| io("SPX-I209", format!("cannot open workspace LOCK: {error}")))?;
    let lock_identity = identity_from_file(&lock, "SPX-I209")?;
    lock_file(&lock, exclusive)?;
    if let Err(diagnostics) = recheck_lock(&lock_path, &lock, &lock_identity) {
        return Err(unlock_with_diagnostics(&lock, diagnostics));
    }
    Ok(WorkspaceLockGuard {
        root,
        root_identity,
        control,
        lock_path,
        lock,
        lock_identity,
        exclusive,
    })
}

fn finish_snapshot_guard(
    lock_guard: WorkspaceLockGuard,
) -> Result<WorkspaceGuard, Vec<Diagnostic>> {
    finish_snapshot_guard_mode(lock_guard, WorkspaceMode::Ordinary)
}

fn finish_snapshot_guard_mode(
    lock_guard: WorkspaceLockGuard,
    mode: WorkspaceMode,
) -> Result<WorkspaceGuard, Vec<Diagnostic>> {
    let WorkspaceLockGuard {
        root,
        root_identity,
        control,
        lock_path,
        lock,
        lock_identity,
        exclusive,
    } = lock_guard;
    let authenticated = match (|| {
        validate_control(&control)?;
        recheck_lock(&lock_path, &lock, &lock_identity)?;
        let authenticated =
            snapshot_authenticated_mode(&root, &control, Some(&lock_identity), mode)?;
        let (_, generation_directories) =
            count_directories_bounded(&control.join("generations"), MAX_RETAINED_GENERATIONS)?;
        let generation_names = inventory_names_from_directories(&generation_directories)?;
        let (_, staging_directories, staging_files) =
            validate_staging_inventory(&control.join("staging"))?;
        let staging_names = inventory_names_from_directories(&staging_directories)?
            .into_iter()
            .chain(inventory_names_from_texts(&staging_files)?)
            .collect();
        recheck_lock(&lock_path, &lock, &lock_identity)?;
        Ok::<_, Vec<Diagnostic>>((authenticated, generation_names, staging_names))
    })() {
        Ok(authenticated) => authenticated,
        Err(diagnostics) => return Err(unlock_with_diagnostics(&lock, diagnostics)),
    };
    let (authenticated, generation_names, staging_names) = authenticated;
    Ok(WorkspaceGuard {
        root,
        root_identity,
        control,
        lock_path,
        lock,
        lock_identity,
        snapshot: authenticated.snapshot,
        directories: authenticated.directories,
        texts: authenticated.texts,
        exclusive,
        generation_names,
        staging_names,
        mode,
    })
}

fn snapshot_authenticated(
    root: &Path,
    control: &Path,
    lock_identity: Option<&FileIdentity>,
) -> Result<AuthenticatedSnapshot, Vec<Diagnostic>> {
    snapshot_authenticated_mode(root, control, lock_identity, WorkspaceMode::Ordinary)
}

fn snapshot_authenticated_mode(
    root: &Path,
    control: &Path,
    lock_identity: Option<&FileIdentity>,
    mode: WorkspaceMode,
) -> Result<AuthenticatedSnapshot, Vec<Diagnostic>> {
    let root_dir = authenticate_directory_held(root)?;
    let control_dir = authenticate_directory_held(control)?;
    let root_identity = &root_dir.identity;
    let control_identity = &control_dir.identity;
    let generations_path = control.join("generations");
    let staging_path = control.join("staging");
    let generations_dir = authenticate_directory_held(&generations_path)?;
    let staging_dir = authenticate_directory_held(&staging_path)?;
    let generations_identity = &generations_dir.identity;
    let staging_identity = &staging_dir.identity;
    require_distinct_identities(&[
        root_identity,
        control_identity,
        generations_identity,
        staging_identity,
    ])?;
    if lock_identity.is_some_and(|lock| {
        lock == root_identity
            || lock == control_identity
            || lock == generations_identity
            || lock == staging_identity
    }) {
        return Err(invariant("workspace LOCK aliases a managed directory"));
    }
    require_same_volume(&[
        root_identity,
        control_identity,
        generations_identity,
        staging_identity,
    ])?;
    let mut active = authenticate_text(&control.join("ACTIVE"), MAX_MANIFEST_BYTES, "SPX-I209")?;
    let revision = mode.parse_active(&active.source)?;
    let generation = control.join("generations").join(revision_hex(&revision)?);
    let generation_dir = authenticate_directory_held(&generation)?;
    let selected_generation_identity = generation_dir.identity;
    let generation_identity = &selected_generation_identity;
    if generation_identity == control_identity
        || generation_identity == generations_identity
        || generation_identity == staging_identity
    {
        return Err(invariant(
            "workspace directories must have distinct identities",
        ));
    }
    let files_root = generation.join("files");
    let files_dir = authenticate_directory_held(&files_root)?;
    let files_identity = &files_dir.identity;
    if files_identity == generation_identity || files_identity == generations_identity {
        return Err(invariant(
            "workspace directories must have distinct identities",
        ));
    }
    require_same_volume(&[
        root_identity,
        control_identity,
        generations_identity,
        staging_identity,
        generation_identity,
        files_identity,
    ])?;
    let mut manifest_input = authenticate_text(
        &generation.join("manifest.json"),
        MAX_MANIFEST_BYTES,
        "SPX-I209",
    )?;
    let manifest = mode.parse_manifest(&manifest_input.source)?;
    let nested_directories =
        authenticate_directory_trie(&files_root, manifest.iter().map(|file| file.path.as_str()))?;
    if mode.manifest_revision(&manifest_input.source) != revision {
        return Err(invariant(
            "ACTIVE does not bind the exact generation manifest",
        ));
    }
    let mut total = 0usize;
    let mut sources = Vec::with_capacity(manifest.len());
    let mut inputs = Vec::with_capacity(manifest.len());
    for expected in &manifest {
        let input = authenticate_managed_source(
            &files_root,
            &expected.path,
            MAX_TOTAL_SOURCE_BYTES.saturating_sub(total),
        )?;
        let source = input.source.clone();
        total += source.len();
        sources.push((expected.path.clone(), source));
        inputs.push(input);
    }
    require_distinct_text_identities(&inputs, Some(&manifest_input), Some(&active))?;
    if lock_identity.is_some_and(|lock| {
        lock == &active.identity
            || lock == &manifest_input.identity
            || inputs.iter().any(|input| lock == &input.identity)
    }) {
        return Err(invariant("workspace LOCK aliases authenticated content"));
    }
    let mut all_identities = vec![
        root_identity,
        control_identity,
        generations_identity,
        staging_identity,
        generation_identity,
        files_identity,
        &active.identity,
        &manifest_input.identity,
    ];
    if let Some(lock) = lock_identity {
        all_identities.push(lock);
    }
    all_identities.extend(inputs.iter().map(|input| &input.identity));
    all_identities.extend(
        nested_directories
            .iter()
            .map(|directory| &directory.identity),
    );
    require_distinct_identities(&all_identities)?;
    require_same_volume(&all_identities)?;
    let facts = mode.validate_snapshot_sources(&manifest_input.source, sources)?;
    for (expected, fact) in manifest.iter().zip(&facts) {
        if fact.source_graph_schema != expected.source_graph_schema
            || fact.source_revision != expected.source_revision
            || fact.source_digest != expected.source_digest
            || fact.source.len() != expected.bytes
        {
            return Err(invariant("managed generation file disagrees with manifest"));
        }
    }
    validate_generation_inventory(&generation, &manifest)?;
    let (retained, retained_directories) =
        count_directories_bounded(&generations_path, MAX_RETAINED_GENERATIONS)?;
    let (staging, staging_directories, mut staging_files) =
        validate_staging_inventory(&staging_path)?;
    let mut inventory_identities = all_identities;
    inventory_identities.extend(
        retained_directories
            .iter()
            .filter(|directory| &directory.identity != generation_identity)
            .map(|directory| &directory.identity),
    );
    inventory_identities.extend(
        staging_directories
            .iter()
            .map(|directory| &directory.identity),
    );
    inventory_identities.extend(staging_files.iter().map(|file| &file.identity));
    require_distinct_identities(&inventory_identities)?;
    require_same_volume(&inventory_identities)?;
    active.recheck()?;
    manifest_input.recheck()?;
    for input in &mut inputs {
        input.recheck()?;
    }
    if authenticate_directory(&generation)? != *generation_identity
        || authenticate_directory(&files_root)? != *files_identity
        || authenticate_directory(control)? != *control_identity
        || authenticate_directory(&generations_path)? != *generations_identity
        || authenticate_directory(&staging_path)? != *staging_identity
    {
        return Err(invariant(
            "workspace directory identity changed during snapshot",
        ));
    }
    validate_control(control)?;
    validate_generation_inventory(&generation, &manifest)?;
    let files = facts
        .into_iter()
        .map(|fact| WorkspaceFileSnapshot {
            path: fact.path,
            source_graph_schema: fact.source_graph_schema,
            source_revision: fact.source_revision,
            source_digest: fact.source_digest,
            source: fact.source,
        })
        .collect();
    let mut snapshot = WorkspaceSnapshot {
        workspace_revision: revision,
        files,
        manifest_bytes: manifest_input.source.len(),
        retained_generations: retained,
        staging_attempts: staging,
        json: String::new(),
    };
    if mode == WorkspaceMode::Ordinary {
        snapshot.json = bounded_snapshot_json(&snapshot)?;
    }
    let retained_other_directories = retained_directories
        .into_iter()
        .filter(|directory| directory.identity != selected_generation_identity)
        .collect::<Vec<_>>();
    let mut directories = vec![
        root_dir,
        control_dir,
        generations_dir,
        staging_dir,
        generation_dir,
        files_dir,
    ];
    directories.extend(nested_directories);
    directories.extend(retained_other_directories);
    directories.extend(staging_directories);
    let mut texts = vec![active, manifest_input];
    texts.extend(inputs);
    texts.append(&mut staging_files);
    Ok(AuthenticatedSnapshot {
        snapshot,
        directories,
        texts,
    })
}

fn lock_file(file: &File, exclusive: bool) -> Result<(), Vec<Diagnostic>> {
    #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
    {
        let result = if exclusive {
            fs2::FileExt::try_lock_exclusive(file)
        } else {
            fs2::FileExt::try_lock_shared(file)
        };
        result.map_err(|_| vec![Diagnostic::io("SPX-I210", "workspace LOCK is busy")])
    }
    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    {
        let _ = (file, exclusive);
        Err(vec![Diagnostic::io(
            "SPX-I210",
            "workspace locks are unavailable on this target",
        )])
    }
}

fn unlock_file(file: &File) -> Result<(), Vec<Diagnostic>> {
    #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
    {
        fs2::FileExt::unlock(file).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I210",
                format!("cannot release workspace LOCK: {error}"),
            )]
        })
    }
    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    {
        let _ = file;
        Err(vec![Diagnostic::io(
            "SPX-I210",
            "workspace locks are unavailable on this target",
        )])
    }
}

fn unlock_with_diagnostics(file: &File, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    match unlock_file(file) {
        Ok(()) => diagnostics,
        Err(unlock_diagnostics) => unlock_diagnostics,
    }
}

fn parse_path_set(source: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
    let body = canonical_body(source, "workspace path set")?;
    validate_depth(body)?;
    let value: Value =
        serde_json::from_str(body).map_err(|_| format_error("invalid workspace path-set JSON"))?;
    let object = exact_object(&value, &["schema", "files"])?;
    if text(object, "schema")? != PATH_SET_SCHEMA {
        return Err(format_error("wrong workspace path-set schema"));
    }
    let values = array(object, "files")?;
    if !(2..=MAX_MANAGED_FILES).contains(&values.len()) {
        return Err(limit("workspace path set must contain 2..16 files"));
    }
    let mut files = Vec::with_capacity(values.len());
    for value in values {
        let object = exact_object(value, &["path"])?;
        let path = text(object, "path")?.to_owned();
        validate_logical_path(&path)?;
        files.push(path);
    }
    require_sorted_unique(&files)?;
    let (rendered, overflowed) =
        crate::bounded_output::with_limit(MAX_MANIFEST_BYTES, || render_path_set(&files));
    if overflowed || rendered != body {
        return Err(format_error("workspace path set is not canonical"));
    }
    Ok(files)
}

fn parse_workspace_patch(source: &str) -> Result<WorkspacePatch, Vec<Diagnostic>> {
    parse_workspace_patch_with_minimum(source, 2)
}

fn parse_workspace_patch_with_minimum(
    source: &str,
    minimum_changed_files: usize,
) -> Result<WorkspacePatch, Vec<Diagnostic>> {
    let body = canonical_body(source, "workspace patch")?;
    validate_depth(body)?;
    let value: Value =
        serde_json::from_str(body).map_err(|_| format_error("invalid workspace patch JSON"))?;
    let object = exact_object(&value, &["schema", "base_workspace_revision", "files"])?;
    if text(object, "schema")? != PATCH_SCHEMA {
        return Err(format_error("wrong workspace patch schema"));
    }
    let base = digest_text(object, "base_workspace_revision")?.to_owned();
    let values = array(object, "files")?;
    if !(minimum_changed_files..=MAX_MANAGED_FILES).contains(&values.len()) {
        return Err(limit(format!(
            "workspace patch must change {minimum_changed_files}..16 files"
        )));
    }
    let mut files = Vec::with_capacity(values.len());
    for value in values {
        let object = exact_object(value, &["path", "patch"])?;
        let path = text(object, "path")?.to_owned();
        validate_logical_path(&path)?;
        files.push(WorkspacePatchFile {
            path,
            patch: text(object, "patch")?.to_owned(),
        });
    }
    require_sorted_unique(
        &files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>(),
    )?;
    let (rendered, overflowed) =
        crate::bounded_output::with_limit(MAX_WORKSPACE_PATCH_BYTES, || {
            render_workspace_patch_body(&base, &files)
        });
    if overflowed || rendered != body {
        return Err(format_error("workspace patch is not canonical"));
    }
    Ok(WorkspacePatch {
        base,
        files,
        source: source.to_owned(),
        bytes: source.len(),
        digest: domain_digest(
            "semaprax.semantic-workspace-patch.digest.v1\0",
            source.as_bytes(),
        ),
    })
}

fn render_path_set(files: &[String]) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    write!(
        output,
        "{{\"schema\":{},\"files\":[",
        quote_json(PATH_SET_SCHEMA)
    )
    .unwrap();
    for (index, path) in files.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{{\"path\":{}}}", quote_json(path)).unwrap();
    }
    output.push_str("]}");
    output.into_string()
}

fn render_workspace_patch_body(base: &str, files: &[WorkspacePatchFile]) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    write!(
        output,
        "{{\"schema\":{},\"base_workspace_revision\":{},\"files\":[",
        quote_json(PATCH_SCHEMA),
        quote_json(base)
    )
    .unwrap();
    for (index, file) in files.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(
            output,
            "{{\"path\":{},\"patch\":{}}}",
            quote_json(&file.path),
            quote_json(&file.patch)
        )
        .unwrap();
    }
    output.push_str("]}");
    output.into_string()
}

fn parse_root(source: &str) -> Result<String, Vec<Diagnostic>> {
    let body = canonical_body(source, "ACTIVE")?;
    validate_depth(body)?;
    let value: Value =
        serde_json::from_str(body).map_err(|_| format_error("invalid ACTIVE JSON"))?;
    let object = exact_object(&value, &["schema", "workspace_revision"])?;
    if text(object, "schema")? != ROOT_SCHEMA {
        return Err(format_error("wrong ACTIVE schema"));
    }
    let revision = digest_text(object, "workspace_revision")?.to_owned();
    if render_root(&revision).trim_end_matches('\n') != body {
        return Err(format_error("ACTIVE is not canonical"));
    }
    Ok(revision)
}

fn parse_manifest(source: &str) -> Result<Vec<ManifestFile>, Vec<Diagnostic>> {
    let body = canonical_body(source, "workspace manifest")?;
    validate_depth(body)?;
    let value: Value =
        serde_json::from_str(body).map_err(|_| format_error("invalid workspace manifest JSON"))?;
    let object = exact_object(&value, &["schema", "files"])?;
    if text(object, "schema")? != MANIFEST_SCHEMA {
        return Err(format_error("wrong workspace manifest schema"));
    }
    let mut files = Vec::new();
    for value in array(object, "files")? {
        let object = exact_object(
            value,
            &[
                "path",
                "source_graph_schema",
                "source_revision",
                "source_digest",
                "bytes",
            ],
        )?;
        let path = text(object, "path")?.to_owned();
        validate_logical_path(&path)?;
        files.push(ManifestFile {
            path,
            source_graph_schema: text(object, "source_graph_schema")?.to_owned(),
            source_revision: digest_text(object, "source_revision")?.to_owned(),
            source_digest: digest_text(object, "source_digest")?.to_owned(),
            bytes: integer(object, "bytes")?,
        });
    }
    if !(2..=MAX_MANAGED_FILES).contains(&files.len()) {
        return Err(limit("workspace manifest must contain 2..16 files"));
    }
    require_sorted_unique(
        &files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>(),
    )?;
    if render_manifest_entries(&files) != source {
        return Err(format_error("workspace manifest is not canonical"));
    }
    Ok(files)
}

fn file_facts(
    sources: Vec<(String, String)>,
    require_canonical: bool,
) -> Result<Vec<FileFact>, Vec<Diagnostic>> {
    let mut parsed = Vec::with_capacity(sources.len());
    let mut remaining = (MAX_DECLARATIONS, MAX_CALLABLES, MAX_CALL_SITES);
    let mut remaining_source_bytes = MAX_TOTAL_SOURCE_BYTES;
    for (path, source) in sources {
        if source.len() > remaining_source_bytes {
            return Err(limit("workspace sources exceed 16777216 bytes"));
        }
        let program = parse(&source, Path::new(&path)).map_err(|error| vec![error])?;
        let counts = crate::review::workspace_ast_counts(&program)?;
        remaining.0 = remaining
            .0
            .checked_sub(counts.0)
            .ok_or_else(|| limit("workspace declarations exceed 4096"))?;
        remaining.1 = remaining
            .1
            .checked_sub(counts.1)
            .ok_or_else(|| limit("workspace callables exceed 1024"))?;
        remaining.2 = remaining
            .2
            .checked_sub(counts.2)
            .ok_or_else(|| limit("workspace call sites exceed 65536"))?;
        remaining_source_bytes -= source.len();
        parsed.push(ParsedFact {
            path,
            source,
            program,
            usage: counts,
        });
    }
    if require_canonical {
        let (result, overflowed) = crate::bounded_output::with_limit(
            MAX_TOTAL_SOURCE_BYTES,
            || {
                let mut remaining_bytes = MAX_TOTAL_SOURCE_BYTES;
                for item in &parsed {
                    let canonical = crate::format::canonical(&item.program);
                    if canonical.len() > remaining_bytes {
                        return Err(limit(format!(
                            "canonical formatting for managed source `{}` exceeds the remaining source-byte limit",
                            item.path
                        )));
                    }
                    if canonical != item.source {
                        return Err(format_error(format!(
                            "managed source `{}` is not canonical formatter output",
                            item.path
                        )));
                    }
                    remaining_bytes -= canonical.len();
                }
                Ok(())
            },
        );
        if overflowed {
            return Err(limit(
                "workspace canonical formatting exceeds 16777216 work bytes",
            ));
        }
        result?;
    }
    parsed
        .into_iter()
        .map(|parsed| {
            let diagnostics = verify::verify(&parsed.program);
            if diagnostics.iter().any(|item| item.severity.is_error()) {
                return Err(diagnostics);
            }
            let resolved = hir::resolve(&parsed.program)?;
            Ok(FileFact {
                path: parsed.path,
                module: parsed.program.module.clone(),
                source_graph_schema: graph::graph_schema(&resolved).to_owned(),
                source_revision: graph::revision(&parsed.program),
                source_digest: domain_digest(
                    "semaprax.semantic-review.source-digest.v1\0",
                    parsed.source.as_bytes(),
                ),
                source: parsed.source,
                declarations: authored_declaration_ids(&parsed.program),
                declaration_count: parsed.usage.0,
                callable_count: parsed.usage.1,
                call_count: parsed.usage.2,
            })
        })
        .collect()
}

fn authored_declaration_ids(program: &Program) -> Vec<String> {
    let mut ids = Vec::new();
    for ty in &program.types {
        ids.push(ty.stable_id.clone());
        match &ty.kind {
            TypeDeclarationKind::Resource { lifecycles } => {
                ids.extend(lifecycles.iter().filter_map(|item| item.stable_id.clone()));
            }
            TypeDeclarationKind::Record { fields } => {
                ids.extend(fields.iter().map(|item| item.stable_id.clone()));
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    ids.push(case.stable_id.clone());
                    ids.extend(case.fields.iter().map(|item| item.stable_id.clone()));
                }
            }
        }
    }
    for interface in &program.interfaces {
        ids.push(interface.stable_id.clone());
        ids.extend(interface.imports.iter().map(|item| item.stable_id.clone()));
    }
    ids.extend(program.functions.iter().map(|item| item.stable_id.clone()));
    ids
}

fn validate_workspace_facts(facts: &[FileFact]) -> Result<(), Vec<Diagnostic>> {
    let mut modules = BTreeSet::new();
    let mut declarations = BTreeSet::new();
    for fact in facts {
        if !modules.insert(fact.module.clone()) {
            return Err(invariant("workspace module names must be unique"));
        }
        for id in &fact.declarations {
            if !declarations.insert(id.clone()) {
                return Err(invariant(
                    "workspace declaration identities must be globally unique",
                ));
            }
        }
    }
    let usage = usage(facts)?;
    if usage.0 > MAX_DECLARATIONS || usage.1 > MAX_CALLABLES || usage.2 > MAX_CALL_SITES {
        return Err(limit(
            "workspace semantic work exceeds its declaration/callable/call-site limits",
        ));
    }
    Ok(())
}

fn usage(facts: &[FileFact]) -> Result<(usize, usize, usize), Vec<Diagnostic>> {
    facts
        .iter()
        .try_fold((0usize, 0usize, 0usize), |sum, fact| {
            Ok((
                sum.0
                    .checked_add(fact.declaration_count)
                    .ok_or_else(|| limit("declaration count overflow"))?,
                sum.1
                    .checked_add(fact.callable_count)
                    .ok_or_else(|| limit("callable count overflow"))?,
                sum.2
                    .checked_add(fact.call_count)
                    .ok_or_else(|| limit("call count overflow"))?,
            ))
        })
}

fn render_manifest(facts: &[FileFact]) -> String {
    render_manifest_entries(
        &facts
            .iter()
            .map(|fact| ManifestFile {
                path: fact.path.clone(),
                source_graph_schema: fact.source_graph_schema.clone(),
                source_revision: fact.source_revision.clone(),
                source_digest: fact.source_digest.clone(),
                bytes: fact.source.len(),
            })
            .collect::<Vec<_>>(),
    )
}
fn bounded_manifest(facts: &[FileFact]) -> Result<String, Vec<Diagnostic>> {
    let (manifest, overflowed) =
        crate::bounded_output::with_limit(MAX_MANIFEST_BYTES, || render_manifest(facts));
    if overflowed || manifest.len() > MAX_MANIFEST_BYTES {
        return Err(limit("workspace manifest exceeds 1048576 bytes"));
    }
    Ok(manifest)
}
fn render_manifest_entries(files: &[ManifestFile]) -> String {
    crate::bounded_output::budgeted_format(format_args!("{{\"schema\":{},\"files\":[{}]}}\n", quote_json(MANIFEST_SCHEMA), crate::bounded_output::budgeted_join(files.iter().map(|file| crate::bounded_output::budgeted_format(format_args!("{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}", quote_json(&file.path), quote_json(&file.source_graph_schema), quote_json(&file.source_revision), quote_json(&file.source_digest), file.bytes))), ",")))
}
fn render_root(revision: &str) -> String {
    format!(
        "{{\"schema\":{},\"workspace_revision\":{}}}\n",
        quote_json(ROOT_SCHEMA),
        quote_json(revision)
    )
}

fn render_snapshot(snapshot: &WorkspaceSnapshot, used: usize) -> String {
    crate::bounded_output::budgeted_format(format_args!("{{\"schema\":{},\"workspace_revision\":{},\"files\":[{}],\"limits\":{{\"max_managed_files\":16,\"max_total_source_bytes\":16777216,\"max_manifest_bytes\":1048576,\"max_snapshot_bytes\":33554432,\"max_json_depth\":8,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}},\"budget\":{{\"used_managed_files\":{},\"used_total_source_bytes\":{},\"used_manifest_bytes\":{},\"used_snapshot_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":0}},\"nonclaims\":[{}]}}", quote_json(SNAPSHOT_SCHEMA), quote_json(&snapshot.workspace_revision), crate::bounded_output::budgeted_join(snapshot.files.iter().map(|file| crate::bounded_output::budgeted_format(format_args!("{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}", quote_json(&file.path), quote_json(&file.source_graph_schema), quote_json(&file.source_revision), quote_json(&file.source_digest), file.source.len()))), ","), snapshot.files.len(), snapshot.files.iter().map(|file| file.source.len()).sum::<usize>(), snapshot.manifest_bytes, used, snapshot.retained_generations, snapshot.staging_attempts, crate::bounded_output::budgeted_join(NONCLAIMS.iter().map(|item| quote_json(item)), ",")))
}

#[allow(clippy::too_many_arguments)]
fn render_preview(
    base: &WorkspaceSnapshot,
    patch: &WorkspacePatch,
    candidate_revision: &str,
    previews: &BTreeMap<String, (String, String, String, String, String, String)>,
    usage: (usize, usize, usize),
    manifest_bytes: usize,
    candidate_bytes: usize,
    operations: usize,
    used: usize,
) -> String {
    crate::bounded_output::budgeted_format(format_args!("{{\"schema\":{},\"base_workspace_revision\":{},\"candidate_workspace_revision\":{},\"workspace_patch_digest\":{},\"files\":[{}],\"limits\":{{\"max_managed_files\":16,\"max_changed_files\":16,\"max_total_base_source_bytes\":16777216,\"max_total_candidate_source_bytes\":16777216,\"max_workspace_patch_bytes\":4194304,\"max_operations\":4096,\"max_declarations\":4096,\"max_callables\":1024,\"max_call_sites\":65536,\"max_manifest_bytes\":1048576,\"max_preview_bytes\":65536,\"max_json_depth\":8,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}},\"budget\":{{\"used_managed_files\":{},\"used_changed_files\":{},\"used_total_base_source_bytes\":{},\"used_total_candidate_source_bytes\":{},\"used_workspace_patch_bytes\":{},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_manifest_bytes\":{},\"used_preview_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":0}},\"nonclaims\":[{}]}}", quote_json(PREVIEW_SCHEMA), quote_json(base.workspace_revision()), quote_json(candidate_revision), quote_json(&patch.digest), crate::bounded_output::budgeted_join(patch.files.iter().map(|file| { let item=&previews[&file.path]; crate::bounded_output::budgeted_format(format_args!("{{\"path\":{},\"patch_schema\":{},\"patch_digest\":{},\"base_source_graph_schema\":{},\"candidate_source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{}}}",quote_json(&file.path),quote_json(&item.0),quote_json(&item.1),quote_json(&item.2),quote_json(&item.3),quote_json(&item.4),quote_json(&item.5))) }), ","), base.files.len(), patch.files.len(), base.files.iter().map(|file| file.source.len()).sum::<usize>(), candidate_bytes, patch.bytes, operations, usage.0, usage.1, usage.2, manifest_bytes, used, base.retained_generations, base.staging_attempts, crate::bounded_output::budgeted_join(NONCLAIMS.iter().map(|item| quote_json(item)), ",")))
}

fn write_generation(
    slot: &Path,
    manifest: &str,
    facts: &[FileFact],
) -> Result<(), Vec<Diagnostic>> {
    write_new_file(&slot.join("manifest.json"), manifest.as_bytes())?;
    let files = slot.join("files");
    std::fs::create_dir(&files).map_err(|error| {
        io(
            "SPX-I211",
            format!("cannot create generation files: {error}"),
        )
    })?;
    for fact in facts {
        let path = files.join(&fact.path);
        ensure_generation_parent(&files, &fact.path)?;
        write_new_file(&path, fact.source.as_bytes())?;
    }
    Ok(())
}

#[allow(dead_code)]
fn write_generation_held(
    slot: &Path,
    manifest: &str,
    facts: &[FileFact],
    slot_directory: AuthenticatedDirectory,
    destination: &Path,
    hook: &mut impl FnMut(GenerationPoint, &Path, &Path),
) -> Result<PreparedGeneration, Vec<Diagnostic>> {
    let manifest_input = write_new_text(
        &slot.join("manifest.json"),
        manifest,
        MAX_MANIFEST_BYTES,
        "staged generation manifest",
    )?;
    let files = slot.join("files");
    std::fs::create_dir(&files).map_err(|error| {
        io(
            "SPX-I211",
            format!("cannot create generation files: {error}"),
        )
    })?;
    let files_directory = authenticate_directory_held(&files)?;
    hook(GenerationPoint::AfterManifestWrite, slot, destination);
    let mut directories = vec![slot_directory, files_directory];
    let mut texts = vec![manifest_input];
    for fact in facts {
        let path = files.join(&fact.path);
        directories.extend(ensure_generation_parent_held(&files, &fact.path)?);
        texts.push(write_new_text(
            &path,
            &fact.source,
            fact.source.len(),
            &fact.path,
        )?);
    }
    hook(GenerationPoint::AfterFilesWrite, slot, destination);
    let identities = directories
        .iter()
        .map(|entry| &entry.identity)
        .chain(texts.iter().map(|entry| &entry.identity))
        .collect::<Vec<_>>();
    require_distinct_identities(&identities)?;
    require_same_volume(&identities)?;
    Ok(PreparedGeneration {
        path: slot.to_path_buf(),
        directories,
        texts,
    })
}

#[allow(dead_code)]
fn ensure_generation_parent_held(
    files_root: &Path,
    logical: &str,
) -> Result<Vec<AuthenticatedDirectory>, Vec<Diagnostic>> {
    let segments = logical.split('/').collect::<Vec<_>>();
    let mut current = files_root.to_path_buf();
    let mut held = Vec::new();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        current.push(segment);
        match std::fs::create_dir(&current) {
            Ok(()) => held.push(authenticate_directory_held(&current)?),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(io(
                    "SPX-I211",
                    format!("cannot create generation path `{logical}`: {error}"),
                ));
            }
        }
    }
    Ok(held)
}

#[allow(dead_code)]
fn write_new_text(
    path: &Path,
    source: &str,
    max: usize,
    label: &str,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io("SPX-I211", format!("cannot create managed file: {error}")))?;
    file.write_all(source.as_bytes())
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            io(
                "SPX-I211",
                format!("cannot synchronize managed file: {error}"),
            )
        })?;
    require_single_link_file(&file, "SPX-I211")?;
    let identity = identity_from_file(&file, "SPX-I211")?;
    let mut input = AuthenticatedText {
        path: path.to_path_buf(),
        label: label.to_owned(),
        file,
        identity,
        source: source.to_owned(),
        max,
        code: "SPX-I211",
    };
    input.recheck()?;
    Ok(input)
}

fn ensure_generation_parent(files_root: &Path, logical: &str) -> Result<(), Vec<Diagnostic>> {
    let segments = logical.split('/').collect::<Vec<_>>();
    let mut current = files_root.to_path_buf();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        current.push(segment);
        match std::fs::create_dir(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(io(
                    "SPX-I211",
                    format!("cannot create generation path `{logical}`: {error}"),
                ));
            }
        }
        authenticate_directory(&current)?;
    }
    Ok(())
}

fn authenticate_generation_payload_mode(
    generation: &Path,
    manifest: &str,
    facts: &[FileFact],
    revision: &str,
    mode: WorkspaceMode,
) -> Result<Vec<AuthenticatedText>, Vec<Diagnostic>> {
    authenticate_directory(generation)?;
    let files_root = generation.join("files");
    authenticate_directory(&files_root)?;
    let mut held = Vec::with_capacity(facts.len() + 1);
    let manifest_input = authenticate_text(
        &generation.join("manifest.json"),
        MAX_MANIFEST_BYTES,
        "SPX-I211",
    )?;
    if manifest_input.source != manifest
        || mode.manifest_revision(&manifest_input.source) != revision
    {
        return Err(invariant(
            "staged generation manifest is not the expected revision",
        ));
    }
    held.push(manifest_input);
    for fact in facts {
        let input = authenticate_managed_source(&files_root, &fact.path, fact.source.len())?;
        if input.source != fact.source {
            return Err(invariant(
                "staged generation source differs from authenticated input",
            ));
        }
        held.push(input);
    }
    require_distinct_text_identities(&held, None, None)?;
    let expected = facts
        .iter()
        .map(|fact| ManifestFile {
            path: fact.path.clone(),
            source_graph_schema: fact.source_graph_schema.clone(),
            source_revision: fact.source_revision.clone(),
            source_digest: fact.source_digest.clone(),
            bytes: fact.source.len(),
        })
        .collect::<Vec<_>>();
    validate_generation_inventory(generation, &expected)?;
    Ok(held)
}

#[allow(dead_code)]
fn authenticate_generation_deep(
    generation: &Path,
    manifest: &str,
    facts: &[FileFact],
    revision: &str,
) -> Result<PreparedGeneration, Vec<Diagnostic>> {
    authenticate_generation_deep_mode(
        generation,
        manifest,
        facts,
        revision,
        WorkspaceMode::Ordinary,
    )
}

fn authenticate_generation_deep_mode(
    generation: &Path,
    manifest: &str,
    facts: &[FileFact],
    revision: &str,
    mode: WorkspaceMode,
) -> Result<PreparedGeneration, Vec<Diagnostic>> {
    let generation_directory = authenticate_directory_held(generation)?;
    let files_root = generation.join("files");
    let files_directory = authenticate_directory_held(&files_root)?;
    let mut directories = vec![generation_directory, files_directory];
    directories.extend(authenticate_directory_trie(
        &files_root,
        facts.iter().map(|fact| fact.path.as_str()),
    )?);
    let texts = authenticate_generation_payload_mode(generation, manifest, facts, revision, mode)?;
    let identities = directories
        .iter()
        .map(|entry| &entry.identity)
        .chain(texts.iter().map(|entry| &entry.identity))
        .collect::<Vec<_>>();
    require_distinct_identities(&identities)?;
    require_same_volume(&identities)?;
    Ok(PreparedGeneration {
        path: generation.to_path_buf(),
        directories,
        texts,
    })
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), Vec<Diagnostic>> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io("SPX-I211", format!("cannot create managed file: {error}")))?;
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            io(
                "SPX-I211",
                format!("cannot synchronize managed file: {error}"),
            )
        })?;
    let identity = identity_from_file(&file, "SPX-I211")?;
    recheck_held_regular(path, &file, &identity, "SPX-I211")
}

fn canonical_root(root: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    let meta = std::fs::symlink_metadata(root).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot inspect workspace root: {error}"),
        )
    })?;
    if !meta.is_dir() || meta.file_type().is_symlink() || metadata_is_reparse(&meta) {
        return Err(io("SPX-I209", "workspace root must be a real directory"));
    }
    std::fs::canonicalize(root).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot canonicalize workspace root: {error}"),
        )
    })
}
fn validate_control(control: &Path) -> Result<(), Vec<Diagnostic>> {
    let expected = ["LOCK", "ACTIVE", "generations", "staging"]
        .into_iter()
        .map(Into::into)
        .collect::<BTreeSet<_>>();
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(control).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot read workspace control directory: {error}"),
        )
    })? {
        let name = entry
            .map_err(|error| {
                io(
                    "SPX-I209",
                    format!("cannot inspect workspace control inventory: {error}"),
                )
            })?
            .file_name();
        if !expected.contains(&name) || !names.insert(name) || names.len() > expected.len() {
            return Err(invariant(
                "workspace control inventory contains unexpected entries",
            ));
        }
    }
    if names != expected {
        return Err(invariant(
            "workspace control inventory contains unexpected entries",
        ));
    }
    Ok(())
}

fn validate_initializing_control(control: &Path) -> Result<(), Vec<Diagnostic>> {
    let expected = ["LOCK", "generations", "staging"]
        .into_iter()
        .map(Into::into)
        .collect::<BTreeSet<_>>();
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(control).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot read initializing control directory: {error}"),
        )
    })? {
        let name = entry
            .map_err(|error| io("SPX-I209", format!("cannot inspect control entry: {error}")))?
            .file_name();
        if !expected.contains(&name) || !names.insert(name) || names.len() > expected.len() {
            return Err(invariant("initializing control inventory is not exact"));
        }
    }
    if names != expected {
        return Err(invariant("initializing control inventory is not exact"));
    }
    Ok(())
}
fn validate_generation_inventory(
    generation: &Path,
    files: &[ManifestFile],
) -> Result<(), Vec<Diagnostic>> {
    let mut actual = Vec::new();
    let mut actual_directories = Vec::new();
    collect_files(
        &generation.join("files"),
        &generation.join("files"),
        &mut actual,
        &mut actual_directories,
    )?;
    actual.sort();
    let expected = files.iter().map(|f| f.path.clone()).collect::<Vec<_>>();
    if actual != expected {
        return Err(invariant(
            "managed generation inventory disagrees with manifest",
        ));
    }
    actual_directories.sort();
    let mut expected_directories = BTreeSet::new();
    for file in files {
        let mut prefix = String::new();
        let segments = file.path.split('/').collect::<Vec<_>>();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            expected_directories.insert(prefix.clone());
        }
    }
    if actual_directories != expected_directories.into_iter().collect::<Vec<_>>() {
        return Err(invariant(
            "managed generation directory trie disagrees with manifest",
        ));
    }
    let root = count_entries_bounded(generation, 2)?;
    if root != 2 {
        return Err(invariant("managed generation root inventory is not exact"));
    }
    Ok(())
}
fn collect_files(
    root: &Path,
    _current: &Path,
    out: &mut Vec<String>,
    directories: &mut Vec<String>,
) -> Result<(), Vec<Diagnostic>> {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut visited_directories = 0usize;
    let mut visited_entries = 0usize;
    while let Some((current, depth)) = stack.pop() {
        visited_directories += 1;
        if visited_directories > MAX_MANAGED_FILES.saturating_mul(16).saturating_add(1) {
            return Err(limit("managed generation directory inventory is too large"));
        }
        if depth > 16 {
            return Err(limit("managed generation inventory exceeds path depth 16"));
        }
        authenticate_directory(&current)?;
        for entry in std::fs::read_dir(&current)
            .map_err(|e| io("SPX-I209", format!("cannot read generation inventory: {e}")))?
        {
            visited_entries = visited_entries
                .checked_add(1)
                .ok_or_else(|| limit("generation inventory count overflow"))?;
            if visited_entries > MAX_MANAGED_FILES.saturating_mul(17) {
                return Err(limit("managed generation inventory is too large"));
            }
            let entry =
                entry.map_err(|e| io("SPX-I209", format!("cannot read generation entry: {e}")))?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|e| io("SPX-I209", format!("cannot inspect generation entry: {e}")))?;
            if metadata.file_type().is_symlink() || metadata_is_reparse(&metadata) {
                return Err(invariant("managed generation contains a symlink"));
            }
            if metadata.is_dir() {
                directories.push(
                    path.strip_prefix(root)
                        .map_err(|_| invariant("generation path escaped files root"))?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
                stack.push((path, depth + 1));
                if stack.len() > MAX_MANAGED_FILES.saturating_mul(16) {
                    return Err(limit("managed generation directory inventory is too large"));
                }
            } else if metadata.is_file() {
                if out.len() == MAX_MANAGED_FILES {
                    return Err(limit("managed generation contains more than 16 files"));
                }
                out.push(
                    path.strip_prefix(root)
                        .map_err(|_| invariant("generation path escaped files root"))?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            } else {
                return Err(invariant("managed generation contains a nonregular entry"));
            }
        }
    }
    Ok(())
}
fn authenticate_managed_source(
    root: &Path,
    logical: &str,
    max: usize,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    let mut path = root.to_path_buf();
    let segments = logical.split('/').collect::<Vec<_>>();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        path.push(segment);
        authenticate_directory(&path).map_err(|_| {
            io(
                "SPX-I209",
                format!("managed path `{logical}` contains a non-directory or alias"),
            )
        })?;
    }
    path.push(segments.last().expect("validated logical path is nonempty"));
    authenticate_text_labeled(&path, max, "SPX-I209", logical)
}

fn authenticate_managed_source_semantic(
    root: &Path,
    logical: &str,
    remaining: usize,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    let mut path = root.to_path_buf();
    let segments = logical.split('/').collect::<Vec<_>>();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        path.push(segment);
        authenticate_directory(&path).map_err(|_| {
            io(
                "SPX-I209",
                format!("managed path `{logical}` contains a non-directory or alias"),
            )
        })?;
    }
    path.push(segments.last().expect("validated logical path is nonempty"));
    authenticate_text_labeled_with_limit(
        &path,
        remaining,
        "SPX-I209",
        logical,
        Some(("total_source_bytes", MAX_TOTAL_SOURCE_BYTES)),
    )
}

fn semantic_storage_limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G175",
        format!("Semantic Workspace `{field}` exceeds {maximum}"),
    )]
}

fn authenticate_text(
    path: &Path,
    max: usize,
    code: &'static str,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    authenticate_text_labeled(path, max, code, &path.display().to_string())
}

fn authenticate_text_semantic(
    path: &Path,
    max: usize,
    code: &'static str,
    field: &'static str,
    diagnostic_maximum: usize,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    authenticate_text_labeled_with_limit(
        path,
        max,
        code,
        &path.display().to_string(),
        Some((field, diagnostic_maximum)),
    )
}

fn authenticate_text_labeled(
    path: &Path,
    max: usize,
    code: &'static str,
    label: &str,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    authenticate_text_labeled_with_limit(path, max, code, label, None)
}

fn authenticate_text_labeled_with_limit(
    path: &Path,
    max: usize,
    code: &'static str,
    label: &str,
    semantic_limit: Option<(&'static str, usize)>,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    let before = std::fs::symlink_metadata(path)
        .map_err(|error| io(code, format!("cannot inspect {label}: {error}")))?;
    if !before.is_file() || before.file_type().is_symlink() || metadata_is_reparse(&before) {
        return Err(io(code, "workspace input must be a real regular file"));
    }
    require_single_link_path(path, code)?;
    if before.len() > max as u64 {
        return Err(semantic_limit.map_or_else(
            || limit("workspace input exceeds its byte limit"),
            |(field, maximum)| semantic_storage_limit(field, maximum),
        ));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| io(code, format!("cannot open {label}: {error}")))?;
    let held = file
        .metadata()
        .map_err(|error| io(code, format!("cannot inspect {label}: {error}")))?;
    if !held.is_file() {
        return Err(io(code, "workspace input must be a regular file"));
    }
    require_single_link_file(&file, code)?;
    let identity = identity_from_file(&file, code)?;
    if identity_from_path(path, code)? != identity {
        return Err(invariant("workspace object changed while opening"));
    }
    let after = std::fs::symlink_metadata(path)
        .map_err(|error| io(code, format!("cannot reinspect {label}: {error}")))?;
    if after.file_type().is_symlink() || metadata_is_reparse(&after) || !after.is_file() {
        return Err(invariant(
            "workspace object changed to an alias while opening",
        ));
    }
    require_single_link_path(path, code)?;
    let mut bytes = Vec::with_capacity(usize::try_from(held.len()).unwrap_or(max).min(max));
    std::io::Read::by_ref(&mut file)
        .take(max.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io(code, format!("cannot read {label}: {error}")))?;
    if bytes.len() > max {
        return Err(semantic_limit.map_or_else(
            || limit("workspace input exceeds its byte limit"),
            |(field, maximum)| semantic_storage_limit(field, maximum),
        ));
    }
    let source = String::from_utf8(bytes).map_err(|_| io(code, "workspace input is not UTF-8"))?;
    let mut authenticated = AuthenticatedText {
        path: path.to_path_buf(),
        label: label.to_owned(),
        file,
        identity,
        source,
        max,
        code,
    };
    authenticated.recheck()?;
    Ok(authenticated)
}

fn authenticate_directory(path: &Path) -> Result<FileIdentity, Vec<Diagnostic>> {
    Ok(authenticate_directory_held(path)?.identity)
}

fn authenticate_directory_held(path: &Path) -> Result<AuthenticatedDirectory, Vec<Diagnostic>> {
    let before = std::fs::symlink_metadata(path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot inspect directory {}: {error}", path.display()),
        )
    })?;
    if !before.is_dir() || before.file_type().is_symlink() || metadata_is_reparse(&before) {
        return Err(io(
            "SPX-I209",
            "workspace directory must be real and non-aliased",
        ));
    }
    #[cfg(unix)]
    let file = File::open(path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot retain directory {}: {error}", path.display()),
        )
    })?;
    #[cfg(unix)]
    let identity = identity_from_file(&file, "SPX-I209")?;
    #[cfg(windows)]
    let handle = winapi_util::Handle::from_path_any(path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot retain directory {}: {error}", path.display()),
        )
    })?;
    #[cfg(windows)]
    let identity = identity_from_windows_handle(&handle, "SPX-I209")?;
    #[cfg(not(any(unix, windows)))]
    let identity = identity_from_path(path, "SPX-I209")?;
    let after = std::fs::symlink_metadata(path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot reinspect directory {}: {error}", path.display()),
        )
    })?;
    if !after.is_dir()
        || after.file_type().is_symlink()
        || metadata_is_reparse(&after)
        || identity_from_path(path, "SPX-I209")? != identity
    {
        return Err(invariant(
            "workspace directory identity changed during authentication",
        ));
    }
    let authenticated = AuthenticatedDirectory {
        path: path.to_path_buf(),
        identity,
        #[cfg(unix)]
        file,
        #[cfg(windows)]
        handle,
    };
    authenticated.recheck()?;
    Ok(authenticated)
}

fn authenticate_directory_trie<'a>(
    root: &Path,
    logical_paths: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<AuthenticatedDirectory>, Vec<Diagnostic>> {
    let mut paths = BTreeSet::new();
    for logical in logical_paths {
        let segments = logical.split('/').collect::<Vec<_>>();
        let mut current = root.to_path_buf();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            current.push(segment);
            paths.insert(current.clone());
        }
    }
    paths
        .iter()
        .map(|path| authenticate_directory_held(path))
        .collect()
}

fn identity_from_file(file: &File, code: &'static str) -> Result<FileIdentity, Vec<Diagnostic>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = file
            .metadata()
            .map_err(|error| io(code, format!("cannot inspect held file: {error}")))?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        identity_from_windows_handle(file, code)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(io(
            code,
            "workspace identity authentication is unsupported on this target",
        ))
    }
}

#[cfg(windows)]
fn identity_from_path(path: &Path, code: &'static str) -> Result<FileIdentity, Vec<Diagnostic>> {
    let handle = winapi_util::Handle::from_path_any(path)
        .map_err(|error| io(code, format!("cannot identify {}: {error}", path.display())))?;
    identity_from_windows_handle(&handle, code)
}

#[cfg(windows)]
fn identity_from_windows_handle(
    handle: impl winapi_util::AsHandleRef,
    code: &'static str,
) -> Result<FileIdentity, Vec<Diagnostic>> {
    let information = winapi_util::file::information(handle).map_err(|error| {
        io(
            code,
            format!("cannot inspect Windows object identity: {error}"),
        )
    })?;
    Ok(FileIdentity {
        volume: information.volume_serial_number(),
        index: information.file_index(),
    })
}

#[cfg(not(windows))]
fn identity_from_path(path: &Path, code: &'static str) -> Result<FileIdentity, Vec<Diagnostic>> {
    let file = File::open(path)
        .map_err(|error| io(code, format!("cannot identify {}: {error}", path.display())))?;
    identity_from_file(&file, code)
}

fn require_single_link_file(file: &File, code: &'static str) -> Result<(), Vec<Diagnostic>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = file
            .metadata()
            .map_err(|error| io(code, format!("cannot inspect held link count: {error}")))?;
        if metadata.nlink() != 1 {
            return Err(invariant(
                "workspace regular files must have link count one",
            ));
        }
    }
    #[cfg(windows)]
    {
        let information = winapi_util::file::information(file)
            .map_err(|error| io(code, format!("cannot inspect held link count: {error}")))?;
        if information.number_of_links() != 1 {
            return Err(invariant(
                "workspace regular files must have link count one",
            ));
        }
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (file, code);
    Ok(())
}

fn require_single_link_path(path: &Path, code: &'static str) -> Result<(), Vec<Diagnostic>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| io(code, format!("cannot inspect path link count: {error}")))?;
        if metadata.nlink() != 1 {
            return Err(invariant(
                "workspace regular files must have link count one",
            ));
        }
    }
    #[cfg(windows)]
    {
        let handle = winapi_util::Handle::from_path_any(path)
            .map_err(|error| io(code, format!("cannot inspect path link count: {error}")))?;
        let information = winapi_util::file::information(&handle)
            .map_err(|error| io(code, format!("cannot inspect path link count: {error}")))?;
        if information.number_of_links() != 1 {
            return Err(invariant(
                "workspace regular files must have link count one",
            ));
        }
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (path, code);
    Ok(())
}

fn recheck_held_regular(
    path: &Path,
    file: &File,
    identity: &FileIdentity,
    code: &'static str,
) -> Result<(), Vec<Diagnostic>> {
    let metadata = file.metadata().map_err(|error| {
        io(
            code,
            format!("cannot re-inspect {}: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() || metadata_is_reparse(&metadata) {
        return Err(invariant(
            "workspace held object is no longer a regular file",
        ));
    }
    require_single_link_file(file, code)?;
    let path_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        io(
            code,
            format!("cannot re-inspect {}: {error}", path.display()),
        )
    })?;
    if path_metadata.file_type().is_symlink()
        || metadata_is_reparse(&path_metadata)
        || identity_from_path(path, code)? != *identity
    {
        return Err(invariant("workspace held object identity changed"));
    }
    require_single_link_path(path, code)?;
    Ok(())
}

fn recheck_lock(path: &Path, file: &File, identity: &FileIdentity) -> Result<(), Vec<Diagnostic>> {
    let metadata = file.metadata().map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot inspect workspace LOCK: {error}"),
        )
    })?;
    if metadata.len() != 0 {
        return Err(invariant("workspace LOCK must remain exactly zero bytes"));
    }
    recheck_held_regular(path, file, identity, "SPX-I209")
}

fn metadata_is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn require_distinct_identities(identities: &[&FileIdentity]) -> Result<(), Vec<Diagnostic>> {
    for (index, identity) in identities.iter().enumerate() {
        if identities[..index].contains(identity) {
            return Err(invariant(
                "workspace objects must have distinct physical identities",
            ));
        }
    }
    Ok(())
}

fn require_same_volume(identities: &[&FileIdentity]) -> Result<(), Vec<Diagnostic>> {
    #[cfg(unix)]
    if identities
        .iter()
        .skip(1)
        .any(|identity| identity.device != identities[0].device)
    {
        return Err(invariant(
            "workspace managed objects must share one filesystem volume",
        ));
    }
    #[cfg(windows)]
    if identities
        .iter()
        .skip(1)
        .any(|identity| identity.volume != identities[0].volume)
    {
        return Err(invariant(
            "workspace managed objects must share one filesystem volume",
        ));
    }
    Ok(())
}

fn require_distinct_text_identities(
    inputs: &[AuthenticatedText],
    manifest: Option<&AuthenticatedText>,
    active: Option<&AuthenticatedText>,
) -> Result<(), Vec<Diagnostic>> {
    let mut identities = inputs
        .iter()
        .map(|input| &input.identity)
        .collect::<Vec<_>>();
    if let Some(input) = manifest {
        identities.push(&input.identity);
    }
    if let Some(input) = active {
        identities.push(&input.identity);
    }
    require_distinct_identities(&identities)
}
pub(crate) fn validate_logical_path(path: &str) -> Result<(), Vec<Diagnostic>> {
    if path.len() > 240
        || !path.ends_with(".spx")
        || path
            .bytes()
            .any(|b| !b.is_ascii() || b.is_ascii_uppercase() || b == b'\\' || b == b':')
    {
        return Err(format_error(
            "workspace path is outside the portable lowercase ASCII domain",
        ));
    }
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() > 16 {
        return Err(format_error("workspace path exceeds depth 16"));
    }
    for segment in segments {
        if segment.is_empty()
            || segment.len() > 64
            || !segment.as_bytes()[0].is_ascii_alphanumeric()
            || !segment.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
            || segment.ends_with('.')
            || segment.ends_with(' ')
        {
            return Err(format_error("workspace path has an invalid segment"));
        }
        let stem = segment.split('.').next().unwrap_or("");
        if matches!(
            stem,
            "con"
                | "prn"
                | "aux"
                | "nul"
                | "com1"
                | "com2"
                | "com3"
                | "com4"
                | "com5"
                | "com6"
                | "com7"
                | "com8"
                | "com9"
                | "lpt1"
                | "lpt2"
                | "lpt3"
                | "lpt4"
                | "lpt5"
                | "lpt6"
                | "lpt7"
                | "lpt8"
                | "lpt9"
        ) {
            return Err(format_error("workspace path uses a reserved portable name"));
        }
    }
    if Path::new(path)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format_error(
            "workspace path is not relative and normalized",
        ));
    }
    Ok(())
}

pub(crate) fn evidence_path_is_valid(path: &str) -> bool {
    validate_logical_path(path).is_ok()
}
fn graph_schema_for(program: &Program) -> Result<&'static str, Vec<Diagnostic>> {
    let resolved = hir::resolve(program)?;
    Ok(graph::graph_schema(&resolved))
}
fn workspace_revision(manifest: &str) -> String {
    domain_digest("semaprax.workspace-revision.v1\0", manifest.as_bytes())
}
fn domain_digest(domain: &str, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    format!("sha256:{:x}", h.finalize())
}
fn revision_hex(revision: &str) -> Result<&str, Vec<Diagnostic>> {
    revision
        .strip_prefix("sha256:")
        .filter(|v| {
            v.len() == 64
                && v.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        .ok_or_else(|| format_error("workspace revision is not canonical"))
}
fn canonical_body<'a>(source: &'a str, label: &str) -> Result<&'a str, Vec<Diagnostic>> {
    if source.starts_with('\u{feff}')
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len().saturating_sub(1)].contains('\n')
    {
        return Err(format_error(format!(
            "{label} must be one canonical JSON line with one LF"
        )));
    }
    Ok(&source[..source.len() - 1])
}
fn validate_depth(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escape = false;
    for byte in source.bytes() {
        if string {
            if escape {
                escape = false
            } else if byte == b'\\' {
                escape = true
            } else if byte == b'"' {
                string = false
            }
            continue;
        }
        match byte {
            b'"' => string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return Err(format_error("workspace JSON exceeds depth 8"));
                }
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| format_error("workspace JSON is unbalanced"))?;
            }
            _ => {}
        }
    }
    if string || depth != 0 {
        return Err(format_error("workspace JSON is unbalanced"));
    }
    Ok(())
}
fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| format_error("workspace JSON value must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(format_error("workspace JSON has missing or extra keys"));
    }
    Ok(object)
}
fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format_error(format!("workspace field `{key}` must be a string")))
}
fn digest_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    let value = text(object, key)?;
    revision_hex(value)?;
    Ok(value)
}
fn integer(object: &Map<String, Value>, key: &str) -> Result<usize, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| {
            format_error(format!(
                "workspace field `{key}` must be a canonical integer"
            ))
        })
}
fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format_error(format!("workspace field `{key}` must be an array")))
}
fn require_sorted_unique(values: &[String]) -> Result<(), Vec<Diagnostic>> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format_error(
            "workspace paths must be unique canonical byte order",
        ));
    }
    Ok(())
}
fn count_entries_bounded(path: &Path, max: usize) -> Result<usize, Vec<Diagnostic>> {
    let mut count = 0usize;
    for entry in std::fs::read_dir(path)
        .map_err(|e| io("SPX-I209", format!("cannot read workspace inventory: {e}")))?
    {
        entry.map_err(|e| io("SPX-I209", format!("cannot read workspace inventory: {e}")))?;
        count += 1;
        if count > max {
            return Err(limit("workspace inventory exceeds its entry limit"));
        }
    }
    Ok(count)
}

fn validate_staging_inventory(
    path: &Path,
) -> Result<(usize, Vec<AuthenticatedDirectory>, Vec<AuthenticatedText>), Vec<Diagnostic>> {
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut attempts = BTreeSet::new();
    let mut count = 0usize;
    for entry in std::fs::read_dir(path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot read workspace staging: {error}"),
        )
    })? {
        let entry = entry
            .map_err(|error| io("SPX-I209", format!("cannot read staging attempt: {error}")))?;
        count += 1;
        if count > MAX_STAGING_ATTEMPTS {
            return Err(limit("workspace retains more than 32 staging attempts"));
        }
        let file_name = entry.file_name();
        let spelling = file_name
            .to_str()
            .ok_or_else(|| invariant("workspace staging attempt name is not canonical"))?;
        let attempt = spelling
            .parse::<usize>()
            .ok()
            .filter(|value| *value < MAX_STAGING_ATTEMPTS)
            .filter(|value| value.to_string() == spelling)
            .ok_or_else(|| invariant("workspace staging attempt name is not canonical"))?;
        if !attempts.insert(attempt) {
            return Err(invariant("workspace staging attempt ordinal is duplicated"));
        }
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|error| {
            io(
                "SPX-I209",
                format!("cannot inspect staging attempt: {error}"),
            )
        })?;
        if metadata.file_type().is_symlink() || metadata_is_reparse(&metadata) {
            return Err(invariant("workspace staging attempt is an alias"));
        }
        if metadata.is_dir() {
            let directory = authenticate_directory_held(&entry.path())?;
            let identity = &directory.identity;
            if directories
                .iter()
                .any(|item: &AuthenticatedDirectory| item.identity == *identity)
            {
                return Err(invariant(
                    "workspace staging attempts must have distinct identities",
                ));
            }
            directories.push(directory);
        } else if metadata.is_file() {
            let file = authenticate_text(&entry.path(), MAX_MANIFEST_BYTES, "SPX-I209")?;
            let identity = &file.identity;
            if files
                .iter()
                .any(|item: &AuthenticatedText| item.identity == *identity)
                || directories.iter().any(|item| item.identity == *identity)
            {
                return Err(invariant(
                    "workspace staging attempts must have distinct identities",
                ));
            }
            files.push(file);
        } else {
            return Err(invariant("workspace staging attempt is not regular"));
        }
    }
    Ok((count, directories, files))
}

fn inventory_names_from_directories(
    entries: &[AuthenticatedDirectory],
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    entries
        .iter()
        .map(|entry| {
            entry
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
                .ok_or_else(|| invariant("workspace inventory name is not canonical UTF-8"))
        })
        .collect()
}

fn inventory_names_from_texts(
    entries: &[AuthenticatedText],
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    entries
        .iter()
        .map(|entry| {
            entry
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
                .ok_or_else(|| invariant("workspace inventory name is not canonical UTF-8"))
        })
        .collect()
}

#[allow(dead_code)]
fn require_distinct_path_identities(
    entries: &[(&PathBuf, &FileIdentity)],
) -> Result<(), Vec<Diagnostic>> {
    for (index, (path, identity)) in entries.iter().enumerate() {
        if entries[..index].iter().any(|(other_path, other_identity)| {
            (path == other_path) != (identity == other_identity)
        }) {
            return Err(invariant(
                "workspace paths and physical identities are not one-to-one",
            ));
        }
    }
    Ok(())
}
fn count_directories_bounded(
    path: &Path,
    max: usize,
) -> Result<(usize, Vec<AuthenticatedDirectory>), Vec<Diagnostic>> {
    let mut count = 0;
    let mut directories = Vec::new();
    for e in std::fs::read_dir(path)
        .map_err(|e| io("SPX-I209", format!("cannot read generations: {e}")))?
    {
        let e = e.map_err(|e| io("SPX-I209", format!("cannot read generation: {e}")))?;
        let metadata = std::fs::symlink_metadata(e.path())
            .map_err(|e| io("SPX-I209", format!("cannot inspect generation: {e}")))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata_is_reparse(&metadata)
        {
            return Err(invariant("generations contains a non-directory"));
        }
        let name = e.file_name();
        let name = name
            .to_str()
            .filter(|name| {
                name.len() == 64
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
            .ok_or_else(|| invariant("generation directory name is not canonical"))?;
        let _ = name;
        let directory = authenticate_directory_held(&e.path())?;
        if directories
            .iter()
            .any(|item: &AuthenticatedDirectory| item.identity == directory.identity)
        {
            return Err(invariant(
                "retained generations must have distinct identities",
            ));
        }
        directories.push(directory);
        count += 1;
        if count > max {
            return Err(limit("workspace retains more than 32 generations"));
        }
    }
    Ok((count, directories))
}
fn format_error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G150", message)]
}
fn limit(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G151", message)]
}
fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G152", message)]
}
fn invariant(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G153", message)]
}
fn io(code: &'static str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io(code, message)]
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::canonical_root;
    use super::{
        acquire_snapshot, apply, apply_with_hook, bounded_manifest, count_directories_bounded,
        count_entries_bounded, file_facts, identity_from_path, initialize, initialize_with_hook,
        map_post_publication_candidate_diagnostics, parse_path_set,
        prepare_candidate_generation_with_hook, require_distinct_path_identities,
        require_exact_path_association, validate_staging_inventory, ApplyPoint, FileFact,
        GenerationPoint, InitializePoint,
    };
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        path_set: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-workspace-unit-{label}-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            let alpha = canonical(
                "module workspace.hook_alpha; @id(\"workspace.hook_alpha.helper\") fn helper()->i64{1} fn main()->i64{helper()}",
                "alpha.spx",
            );
            let beta = canonical(
                "module workspace.hook_beta; @id(\"workspace.hook_beta.helper\") fn helper()->i64{2} fn main()->i64{helper()}",
                "beta.spx",
            );
            std::fs::write(root.join("alpha.spx"), alpha).unwrap();
            std::fs::write(root.join("beta.spx"), beta).unwrap();
            let path_set = root.join("paths.json");
            std::fs::write(&path_set, "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n").unwrap();
            Self { root, path_set }
        }

        fn active(&self) -> PathBuf {
            self.root.join(".semaprax-workspace/ACTIVE")
        }

        fn initialize_and_patch(&self, label: &str) -> PathBuf {
            let revision = initialize(&self.root, &self.path_set).unwrap();
            let snapshot = super::snapshot(&self.root).unwrap();
            let alpha = snapshot
                .files()
                .iter()
                .find(|file| file.path() == "alpha.spx")
                .unwrap();
            let beta = snapshot
                .files()
                .iter()
                .find(|file| file.path() == "beta.spx")
                .unwrap();
            let alpha_patch = format!(
                "base {}\nrename workspace.hook_alpha.helper to alpha_{label}\n",
                alpha.source_revision()
            );
            let beta_patch = format!(
                "base {}\nrename workspace.hook_beta.helper to beta_{label}\n",
                beta.source_revision()
            );
            let path = self.root.join(format!("{label}.wspatch"));
            std::fs::write(
                &path,
                format!(
                    "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{revision}\",\"files\":[{{\"path\":\"alpha.spx\",\"patch\":{}}},{{\"path\":\"beta.spx\",\"patch\":{}}}]}}\n",
                    serde_json::to_string(&alpha_patch).unwrap(),
                    serde_json::to_string(&beta_patch).unwrap()
                ),
            )
            .unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn spawn_phase_c_process(
        fixture: &Fixture,
        patch: &Path,
        boundary: &str,
    ) -> (Child, PathBuf, PathBuf) {
        let ready = fixture.root.join(format!("phase-c-{boundary}.ready"));
        let release = fixture.root.join(format!("phase-c-{boundary}.release"));
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "workspace::tests::phase_c_apply_process_child",
                "--nocapture",
            ])
            .env("SEMAPRAX_PHASE_C_CHILD", "1")
            .env("SEMAPRAX_PHASE_C_ROOT", &fixture.root)
            .env("SEMAPRAX_PHASE_C_PATCH", patch)
            .env("SEMAPRAX_PHASE_C_BOUNDARY", boundary)
            .env("SEMAPRAX_PHASE_C_READY", &ready)
            .env("SEMAPRAX_PHASE_C_RELEASE", &release)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() {
            assert!(
                Instant::now() < deadline,
                "Phase-C child did not reach {boundary}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        (child, ready, release)
    }

    #[test]
    fn phase_c_apply_process_child() {
        if std::env::var_os("SEMAPRAX_PHASE_C_CHILD").is_none() {
            return;
        }
        let root = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_ROOT").unwrap());
        let patch = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_PATCH").unwrap());
        let boundary = std::env::var("SEMAPRAX_PHASE_C_BOUNDARY").unwrap();
        let ready = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_READY").unwrap());
        let release = PathBuf::from(std::env::var_os("SEMAPRAX_PHASE_C_RELEASE").unwrap());
        apply_with_hook(&root, &patch, |point, _, _, _| {
            let selected = match boundary.as_str() {
                "pre" => point == ApplyPoint::BeforeActiveReplace,
                "post" => point == ApplyPoint::AfterActiveReplace,
                _ => false,
            };
            if selected {
                std::fs::write(&ready, "ready\n")?;
                let deadline = Instant::now() + Duration::from_secs(30);
                while !release.exists() {
                    assert!(Instant::now() < deadline, "Phase-C child release timed out");
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            Ok(())
        })
        .unwrap();
    }

    fn canonical(source: &str, path: &str) -> String {
        let program = crate::parse(source, Path::new(path)).unwrap();
        crate::format::canonical(&program)
    }

    #[test]
    fn evidence_preflight_paths_are_keyed_not_positional() {
        let expected = vec!["alpha.spx".to_owned(), "beta.spx".to_owned()];
        let reordered = vec!["beta.spx".to_owned(), "alpha.spx".to_owned()];
        require_exact_path_association(&expected, &reordered).unwrap();

        let duplicate = vec!["alpha.spx".to_owned(), "alpha.spx".to_owned()];
        assert!(require_exact_path_association(&expected, &duplicate).is_err());
        let missing = vec!["alpha.spx".to_owned()];
        assert!(require_exact_path_association(&expected, &missing).is_err());
        let foreign = vec!["alpha.spx".to_owned(), "gamma.spx".to_owned()];
        assert!(require_exact_path_association(&expected, &foreign).is_err());
    }

    #[test]
    fn source_mutation_hook_prevents_active_publication() {
        let fixture = Fixture::new("source-race");
        let source = fixture.root.join("alpha.spx");
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::GenerationBeforeRename) {
                std::fs::write(&source, "externally mutated\n").unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.active().exists());
    }

    #[test]
    fn staged_manifest_mutation_hook_prevents_active_publication() {
        let fixture = Fixture::new("stage-race");
        let manifest = fixture
            .root
            .join(".semaprax-workspace/staging/0/manifest.json");
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::GenerationBeforeRename) {
                std::fs::write(&manifest, "{}\n").unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.active().exists());
    }

    #[test]
    fn path_set_mutation_hook_prevents_active_publication() {
        let fixture = Fixture::new("path-set-race");
        let path_set = fixture.path_set.clone();
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::ActiveBeforeRename) {
                std::fs::write(&path_set, "{}\n").unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.active().exists());
    }

    #[test]
    fn staged_active_byte_mutation_hook_prevents_publication() {
        let fixture = Fixture::new("active-byte-race");
        let staged = fixture.root.join(".semaprax-workspace/staging/0");
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::ActiveBeforeRename) {
                std::fs::write(&staged, "{}\n").unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.active().exists());
    }

    #[test]
    fn staged_active_same_byte_replacement_hook_prevents_publication() {
        let fixture = Fixture::new("active-replacement-race");
        let staged = fixture.root.join(".semaprax-workspace/staging/0");
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::ActiveBeforeRename) {
                let bytes = std::fs::read(&staged).unwrap();
                std::fs::remove_file(&staged).unwrap();
                std::fs::write(&staged, bytes).unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.active().exists());
    }

    #[test]
    fn initializer_preserves_foreign_generation_and_active_destinations() {
        for kind in ["file", "directory"] {
            let fixture = Fixture::new(&format!("foreign-generation-{kind}"));
            let mut foreign_generation = None;
            let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
                if matches!(point, InitializePoint::GenerationDestinationChecked) {
                    let slot = fixture.root.join(".semaprax-workspace/staging/0");
                    let manifest = std::fs::read_to_string(slot.join("manifest.json")).unwrap();
                    let revision = super::workspace_revision(&manifest);
                    let destination = fixture
                        .root
                        .join(".semaprax-workspace/generations")
                        .join(super::revision_hex(&revision).unwrap());
                    if kind == "file" {
                        std::fs::write(&destination, "foreign-generation\n").unwrap();
                    } else {
                        std::fs::create_dir(&destination).unwrap();
                    }
                    foreign_generation = Some(destination);
                }
            })
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I211");
            let foreign_generation = foreign_generation.unwrap();
            assert_eq!(foreign_generation.is_file(), kind == "file");
            assert_eq!(foreign_generation.is_dir(), kind == "directory");
            assert!(!fixture.active().exists());
        }

        for kind in ["file", "directory"] {
            let fixture = Fixture::new(&format!("foreign-active-{kind}"));
            let foreign_active = fixture.active();
            let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
                if matches!(point, InitializePoint::ActiveDestinationChecked) {
                    if kind == "file" {
                        std::fs::write(&foreign_active, "foreign-active\n").unwrap();
                    } else {
                        std::fs::create_dir(&foreign_active).unwrap();
                    }
                }
            })
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I212");
            assert_eq!(foreign_active.is_file(), kind == "file");
            assert_eq!(foreign_active.is_dir(), kind == "directory");
        }
    }

    #[test]
    fn initializer_relocation_fingerprint_rejects_post_rename_corruption() {
        let fixture = Fixture::new("generation-relocation-corruption");
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::GenerationRelocated) {
                let generation =
                    std::fs::read_dir(fixture.root.join(".semaprax-workspace/generations"))
                        .unwrap()
                        .next()
                        .unwrap()
                        .unwrap()
                        .path();
                let manifest = generation.join("manifest.json");
                let bytes = std::fs::read(&manifest).unwrap();
                std::fs::remove_file(&manifest).unwrap();
                std::fs::write(&manifest, bytes).unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.active().exists());

        let fixture = Fixture::new("active-relocation-corruption");
        let active = fixture.active();
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::ActiveRelocated) {
                let bytes = std::fs::read(&active).unwrap();
                std::fs::remove_file(&active).unwrap();
                std::fs::write(&active, bytes).unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I212");
        assert!(active.exists());
    }

    #[test]
    fn final_guard_rejects_valid_staging_inventory_drift() {
        let fixture = Fixture::new("staging-drift");
        initialize(&fixture.root, &fixture.path_set).unwrap();
        let mut guard = acquire_snapshot(&fixture.root, false).unwrap();
        std::fs::create_dir(fixture.root.join(".semaprax-workspace/staging/0")).unwrap();
        let error = guard.recheck().unwrap_err();
        assert_eq!(error[0].code, "SPX-G152");
    }

    #[test]
    fn final_guard_rejects_valid_retained_generation_drift() {
        let fixture = Fixture::new("generation-drift");
        initialize(&fixture.root, &fixture.path_set).unwrap();
        let donor = Fixture::new("generation-donor");
        std::fs::write(
            donor.root.join("beta.spx"),
            canonical(
                "module workspace.hook_beta; @id(\"workspace.hook_beta.helper\") fn helper()->i64{3} fn main()->i64{helper()}",
                "beta.spx",
            ),
        )
        .unwrap();
        initialize(&donor.root, &donor.path_set).unwrap();
        let donor_generations = donor.root.join(".semaprax-workspace/generations");
        let donor_generation = std::fs::read_dir(&donor_generations)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let target_generations = fixture.root.join(".semaprax-workspace/generations");
        let target = target_generations.join(donor_generation.file_name());
        let mut guard = acquire_snapshot(&fixture.root, false).unwrap();
        copy_tree(&donor_generation.path(), &target);
        let error = guard.recheck().unwrap_err();
        assert_eq!(error[0].code, "SPX-G152");
    }

    #[test]
    fn snapshot_releases_shared_lock_before_returning_owned_data() {
        let fixture = Fixture::new("snapshot-lock-handoff");
        let revision = initialize(&fixture.root, &fixture.path_set).unwrap();
        let lock_path = fixture.root.join(".semaprax-workspace/LOCK");

        for _ in 0..128 {
            let snapshot = super::snapshot(&fixture.root).unwrap();
            assert_eq!(snapshot.workspace_revision(), revision);
            let lock = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&lock_path)
                .unwrap();
            fs2::FileExt::try_lock_exclusive(&lock)
                .expect("snapshot must release its shared lock before returning");
            fs2::FileExt::unlock(&lock).unwrap();
        }
    }

    #[test]
    fn commit_failures_release_exclusive_lock_before_returning() {
        let fixture = Fixture::new("commit-error-lock-handoff");
        let patch = fixture.initialize_and_patch("handoff");
        let old_revision = super::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision
            .to_owned();
        let lock_path = fixture.root.join(".semaprax-workspace/LOCK");

        for _ in 0..64 {
            let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
                if point == ApplyPoint::AfterCandidatePrepared {
                    return Err(std::io::Error::other("reject before ACTIVE staging"));
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I211");

            let lock = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&lock_path)
                .unwrap();
            fs2::FileExt::try_lock_exclusive(&lock)
                .expect("failed commit must synchronously release its exclusive lock");
            fs2::FileExt::unlock(&lock).unwrap();
            assert_eq!(
                super::snapshot(&fixture.root).unwrap().workspace_revision,
                old_revision
            );
        }
    }

    #[test]
    fn apply_pretransfer_failures_release_exclusive_lock_before_returning() {
        let fixture = Fixture::new("apply-pretransfer-lock-handoff");
        let patch = fixture.initialize_and_patch("pretransfer");
        let old_revision = super::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision
            .to_owned();
        let lock_path = fixture.root.join(".semaprax-workspace/LOCK");
        let missing = fixture.root.join("missing.wspatch");
        let malformed = fixture.root.join("malformed.wspatch");
        std::fs::write(&malformed, "{}\n").unwrap();
        let stale = fixture.root.join("stale.wspatch");
        let stale_revision = format!("sha256:{:064x}", 0usize);
        let stale_source =
            std::fs::read_to_string(&patch)
                .unwrap()
                .replacen(&old_revision, &stale_revision, 1);
        std::fs::write(&stale, stale_source).unwrap();

        for _ in 0..64 {
            let missing_error = apply(&fixture.root, &missing).unwrap_err();
            assert_eq!(missing_error[0].code, "SPX-I209");
            assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);

            let malformed_error = apply(&fixture.root, &malformed).unwrap_err();
            assert_eq!(malformed_error[0].code, "SPX-G150");
            assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);

            let hook_error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
                if point == ApplyPoint::AfterPatchRead {
                    return Err(std::io::Error::other("reject after patch ownership"));
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(hook_error[0].code, "SPX-I209");
            assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);

            let stale_error = apply(&fixture.root, &stale).unwrap_err();
            assert_eq!(stale_error[0].code, "SPX-G152");
            assert_apply_lock_handoff(&fixture, &lock_path, &old_revision);
        }
    }

    fn assert_apply_lock_handoff(fixture: &Fixture, lock_path: &Path, revision: &str) {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock)
            .expect("failed apply must synchronously release its exclusive lock");
        fs2::FileExt::unlock(&lock).unwrap();
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().workspace_revision,
            revision
        );
    }

    fn copy_tree(source: &Path, target: &Path) {
        std::fs::create_dir(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    fn assert_active_unchanged(fixture: &Fixture, bytes: &[u8], identity: super::FileIdentity) {
        assert_eq!(std::fs::read(fixture.active()).unwrap(), bytes);
        assert_eq!(
            identity_from_path(&fixture.active(), "SPX-I209").unwrap(),
            identity
        );
    }

    #[test]
    fn path_identity_relation_is_biconditional() {
        let fixture = Fixture::new("path-identity-relation");
        let alpha = fixture.root.join("alpha.spx");
        let beta = fixture.root.join("beta.spx");
        let alpha_identity = identity_from_path(&alpha, "SPX-I209").unwrap();
        let beta_identity = identity_from_path(&beta, "SPX-I209").unwrap();
        require_distinct_path_identities(&[
            (&alpha, &alpha_identity),
            (&alpha, &alpha_identity),
            (&beta, &beta_identity),
        ])
        .unwrap();
        assert!(require_distinct_path_identities(&[
            (&alpha, &alpha_identity),
            (&alpha, &beta_identity),
        ])
        .is_err());
        assert!(require_distinct_path_identities(&[
            (&alpha, &alpha_identity),
            (&beta, &alpha_identity),
        ])
        .is_err());
    }

    #[test]
    fn post_publication_candidate_mapping_is_narrow() {
        let structural = map_post_publication_candidate_diagnostics(vec![super::Diagnostic::io(
            "SPX-I209",
            "workspace directory must be real and non-aliased",
        )]);
        assert_eq!(structural[0].code, "SPX-G153");
        let genuine_io = map_post_publication_candidate_diagnostics(vec![super::Diagnostic::io(
            "SPX-I209",
            "cannot inspect directory: access denied",
        )]);
        assert_eq!(genuine_io[0].code, "SPX-I209");
    }

    #[test]
    fn phase_b_creates_then_deep_reuses_candidate_without_active_pivot() {
        let fixture = Fixture::new("phase-b-reuse");
        let patch = fixture.initialize_and_patch("reuse");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let first =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
        assert!(fixture
            .root
            .join(".semaprax-workspace/generations")
            .join(super::revision_hex(&first).unwrap())
            .is_dir());
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().retained_generations,
            2
        );
        let second =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().retained_generations,
            2
        );
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }

    #[test]
    fn phase_c_pivots_only_active_and_second_apply_is_stale() {
        let fixture = Fixture::new("phase-c-success");
        let alpha = std::fs::read(fixture.root.join("alpha.spx")).unwrap();
        let beta = std::fs::read(fixture.root.join("beta.spx")).unwrap();
        let patch = fixture.initialize_and_patch("commit");
        let old_active = std::fs::read(fixture.active()).unwrap();
        let old_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let preview = super::preview(&fixture.root, &patch).unwrap();
        let expected = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();

        let applied = apply(&fixture.root, &patch).unwrap();
        assert_eq!(applied, expected);
        assert_ne!(std::fs::read(fixture.active()).unwrap(), old_active);
        assert_ne!(
            identity_from_path(&fixture.active(), "SPX-I209").unwrap(),
            old_identity
        );
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().workspace_revision,
            expected
        );
        assert_eq!(
            std::fs::read(fixture.root.join("alpha.spx")).unwrap(),
            alpha
        );
        assert_eq!(std::fs::read(fixture.root.join("beta.spx")).unwrap(), beta);

        let committed_active = std::fs::read(fixture.active()).unwrap();
        let committed_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let error = apply(&fixture.root, &patch).unwrap_err();
        assert_eq!(error[0].code, "SPX-G152");
        assert_active_unchanged(&fixture, &committed_active, committed_identity);
    }

    #[test]
    fn phase_c_final_checks_reject_owned_input_and_authority_drift_before_pivot() {
        for case in [
            "patch",
            "active",
            "stage",
            "candidate",
            "candidate_source",
            "candidate_inventory",
            "staging_inventory",
            "generation_inventory",
            "before_replace_stage",
        ] {
            let fixture = Fixture::new(&format!("phase-c-final-{case}"));
            let patch = fixture.initialize_and_patch(case);
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let error = apply_with_hook(
                &fixture.root,
                &patch,
                |point, active, staged_active, candidate| {
                    match (case, point) {
                        ("patch", ApplyPoint::AfterPatchRead) => {
                            std::fs::write(&patch, "{}\n")?;
                        }
                        ("active", ApplyPoint::BeforeSecondFinalCheck) => {
                            let bytes = std::fs::read(active)?;
                            std::fs::remove_file(active)?;
                            std::fs::write(active, bytes)?;
                        }
                        ("stage", ApplyPoint::BeforeSecondFinalCheck) => {
                            let path = staged_active.unwrap();
                            let bytes = std::fs::read(path)?;
                            std::fs::remove_file(path)?;
                            std::fs::write(path, bytes)?;
                        }
                        ("candidate", ApplyPoint::BeforeSecondFinalCheck) => {
                            let path = candidate.unwrap().join("manifest.json");
                            let bytes = std::fs::read(&path)?;
                            std::fs::remove_file(&path)?;
                            std::fs::write(path, bytes)?;
                        }
                        ("candidate_source", ApplyPoint::BeforeSecondFinalCheck) => {
                            let path = candidate.unwrap().join("files/alpha.spx");
                            let bytes = std::fs::read(&path)?;
                            std::fs::remove_file(&path)?;
                            std::fs::write(path, bytes)?;
                        }
                        ("candidate_inventory", ApplyPoint::BeforeSecondFinalCheck) => {
                            std::fs::write(
                                candidate.unwrap().join("files/extra.spx"),
                                "foreign\n",
                            )?;
                        }
                        ("staging_inventory", ApplyPoint::BeforeSecondFinalCheck) => {
                            std::fs::write(
                                fixture.root.join(".semaprax-workspace/staging/31"),
                                "foreign\n",
                            )?;
                        }
                        ("generation_inventory", ApplyPoint::BeforeSecondFinalCheck) => {
                            std::fs::create_dir(fixture.root.join(format!(
                                ".semaprax-workspace/generations/{:064x}",
                                987_654usize
                            )))?;
                        }
                        ("before_replace_stage", ApplyPoint::BeforeActiveReplace) => {
                            std::fs::write(staged_active.unwrap(), "{}\n")?;
                        }
                        _ => {}
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(matches!(error[0].code, "SPX-G153" | "SPX-I209"));
            if case == "active" {
                assert_eq!(std::fs::read(fixture.active()).unwrap(), active_bytes);
                assert_ne!(
                    identity_from_path(&fixture.active(), "SPX-I209").unwrap(),
                    active_identity
                );
            } else {
                assert_active_unchanged(&fixture, &active_bytes, active_identity);
            }
        }
    }

    #[test]
    fn phase_c_each_final_boundary_rejects_identity_and_inventory_substitution() {
        for boundary in [
            ApplyPoint::BeforeFirstFinalCheck,
            ApplyPoint::BeforeActiveReplace,
        ] {
            for case in [
                "patch",
                "active",
                "stage",
                "manifest",
                "source",
                "staging_inventory",
                "generation_inventory",
            ] {
                let fixture = Fixture::new(&format!("phase-c-{boundary:?}-{case}"));
                let patch = fixture.initialize_and_patch(case);
                let old_revision = super::snapshot(&fixture.root)
                    .unwrap()
                    .workspace_revision
                    .to_owned();
                let error =
                    apply_with_hook(&fixture.root, &patch, |point, active, staged, candidate| {
                        if point != boundary {
                            return Ok(());
                        }
                        match case {
                            "patch" => std::fs::write(&patch, "{}\n")?,
                            "active" => replace_with_same_bytes(active)?,
                            "stage" => replace_with_same_bytes(staged.unwrap())?,
                            "manifest" => {
                                replace_with_same_bytes(&candidate.unwrap().join("manifest.json"))?
                            }
                            "source" => replace_with_same_bytes(
                                &candidate.unwrap().join("files/alpha.spx"),
                            )?,
                            "staging_inventory" => std::fs::write(
                                fixture.root.join(".semaprax-workspace/staging/31"),
                                "foreign\n",
                            )?,
                            "generation_inventory" => std::fs::create_dir(fixture.root.join(
                                format!(".semaprax-workspace/generations/{:064x}", 123_456usize),
                            ))?,
                            _ => unreachable!(),
                        }
                        Ok(())
                    })
                    .unwrap_err();
                assert!(matches!(error[0].code, "SPX-G153" | "SPX-I209"));
                assert_eq!(
                    super::snapshot(&fixture.root).unwrap().workspace_revision,
                    old_revision
                );
            }
        }
    }

    fn replace_with_same_bytes(path: &Path) -> std::io::Result<()> {
        let bytes = std::fs::read(path)?;
        std::fs::remove_file(path)?;
        std::fs::write(path, bytes)
    }

    #[test]
    fn phase_c_pre_pivot_rejection_retains_active_and_staging_residue() {
        let fixture = Fixture::new("phase-c-reject-pivot");
        let patch = fixture.initialize_and_patch("reject");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
            if point == ApplyPoint::BeforeActiveReplace {
                return Err(std::io::Error::other("injected ACTIVE rename rejection"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I211");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
        assert!(
            std::fs::read_dir(fixture.root.join(".semaprax-workspace/staging"))
                .unwrap()
                .next()
                .is_some()
        );
    }

    #[cfg(unix)]
    #[test]
    fn phase_c_atomic_active_rename_failure_preserves_old_active() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("phase-c-rename-failure");
        let patch = fixture.initialize_and_patch("rename_failure");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let control = fixture.root.join(".semaprax-workspace");
        let original_permissions = std::fs::metadata(&control).unwrap().permissions();
        let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
            if point == ApplyPoint::BeforeActiveReplace {
                std::fs::set_permissions(&control, std::fs::Permissions::from_mode(0o500))?;
            }
            Ok(())
        })
        .unwrap_err();
        std::fs::set_permissions(&control, original_permissions).unwrap();
        assert_eq!(error[0].code, "SPX-I211");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }

    #[test]
    fn phase_c_bounded_final_source_growth_fails_before_pivot() {
        for boundary in [
            ApplyPoint::BeforeFirstFinalCheck,
            ApplyPoint::BeforeActiveReplace,
        ] {
            let fixture = Fixture::new(&format!("phase-c-growth-{boundary:?}"));
            let patch = fixture.initialize_and_patch("growth");
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let base_revision = super::snapshot(&fixture.root)
                .unwrap()
                .workspace_revision
                .strip_prefix("sha256:")
                .unwrap()
                .to_owned();
            let base_source = fixture
                .root
                .join(".semaprax-workspace/generations")
                .join(base_revision)
                .join("files/alpha.spx");
            let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
                if point == boundary {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .open(&base_source)?
                        .set_len((super::MAX_TOTAL_SOURCE_BYTES + 1) as u64)?;
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-G153");
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
            assert_eq!(
                std::fs::metadata(base_source).unwrap().len(),
                (super::MAX_TOTAL_SOURCE_BYTES + 1) as u64
            );
        }
    }

    #[test]
    fn phase_c_post_pivot_uncertainty_retains_new_generation_and_foreign_residue() {
        let fixture = Fixture::new("phase-c-post-pivot");
        let patch = fixture.initialize_and_patch("post_pivot");
        let preview = super::preview(&fixture.root, &patch).unwrap();
        let expected = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let residue = fixture.root.join(".semaprax-workspace/staging/31");
        let error = apply_with_hook(&fixture.root, &patch, |point, _, _, _| {
            if point == ApplyPoint::AfterActiveReplace {
                std::fs::write(&residue, "foreign\n")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I212");
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().workspace_revision,
            expected
        );
        assert_eq!(std::fs::read_to_string(residue).unwrap(), "foreign\n");
    }

    #[test]
    fn phase_c_unwind_boundaries_leave_exactly_old_or_new_active() {
        for point in [
            ApplyPoint::BeforeActiveReplace,
            ApplyPoint::AfterActiveReplace,
        ] {
            let fixture = Fixture::new(&format!("phase-c-unwind-{point:?}"));
            let patch = fixture.initialize_and_patch("unwind");
            let old_revision = super::snapshot(&fixture.root)
                .unwrap()
                .workspace_revision
                .to_owned();
            let preview = super::preview(&fixture.root, &patch).unwrap();
            let candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
                ["candidate_workspace_revision"]
                .as_str()
                .unwrap()
                .to_owned();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = apply_with_hook(&fixture.root, &patch, |observed, _, _, _| {
                    if observed == point {
                        panic!("simulated process termination boundary");
                    }
                    Ok(())
                });
            }));
            assert!(result.is_err());
            let current = super::snapshot(&fixture.root).unwrap();
            if point == ApplyPoint::BeforeActiveReplace {
                assert_eq!(current.workspace_revision, old_revision);
                assert_eq!(apply(&fixture.root, &patch).unwrap(), candidate);
            } else {
                assert_eq!(current.workspace_revision, candidate);
                assert_eq!(
                    apply(&fixture.root, &patch).unwrap_err()[0].code,
                    "SPX-G152"
                );
            }
        }
    }

    #[test]
    fn phase_c_killed_process_boundaries_recover_as_exact_old_or_new() {
        for boundary in ["pre", "post"] {
            let fixture = Fixture::new(&format!("phase-c-kill-{boundary}"));
            let patch = fixture.initialize_and_patch("killed");
            let old_revision = super::snapshot(&fixture.root)
                .unwrap()
                .workspace_revision
                .to_owned();
            let preview = super::preview(&fixture.root, &patch).unwrap();
            let candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
                ["candidate_workspace_revision"]
                .as_str()
                .unwrap()
                .to_owned();
            let (mut child, _, _) = spawn_phase_c_process(&fixture, &patch, boundary);
            child.kill().unwrap();
            assert!(!child.wait().unwrap().success());

            let lock = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(fixture.root.join(".semaprax-workspace/LOCK"))
                .unwrap();
            fs2::FileExt::try_lock_exclusive(&lock).unwrap();
            fs2::FileExt::unlock(&lock).unwrap();
            let current = super::snapshot(&fixture.root).unwrap();
            assert_eq!(
                std::fs::read_dir(fixture.root.join(".semaprax-workspace/generations"))
                    .unwrap()
                    .count(),
                2
            );
            if boundary == "pre" {
                assert_eq!(current.workspace_revision, old_revision);
                assert!(
                    std::fs::read_dir(fixture.root.join(".semaprax-workspace/staging"))
                        .unwrap()
                        .next()
                        .is_some()
                );
                assert_eq!(apply(&fixture.root, &patch).unwrap(), candidate);
            } else {
                assert_eq!(current.workspace_revision, candidate);
                assert_eq!(
                    apply(&fixture.root, &patch).unwrap_err()[0].code,
                    "SPX-G152"
                );
            }
        }
    }

    #[test]
    fn phase_c_live_writer_exposes_no_partial_snapshot_to_cooperative_reader() {
        let fixture = Fixture::new("phase-c-live-reader");
        let patch = fixture.initialize_and_patch("live_reader");
        let old_active = std::fs::read(fixture.active()).unwrap();
        let preview = super::preview(&fixture.root, &patch).unwrap();
        let candidate = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let (mut child, _, release) = spawn_phase_c_process(&fixture, &patch, "pre");
        assert_eq!(
            super::snapshot(&fixture.root).unwrap_err()[0].code,
            "SPX-I210"
        );
        assert_eq!(std::fs::read(fixture.active()).unwrap(), old_active);
        std::fs::write(release, "release\n").unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(
            super::snapshot(&fixture.root).unwrap().workspace_revision,
            candidate
        );
    }

    #[test]
    fn phase_c_active_permission_drift_fails_both_final_boundaries() {
        for (target, point) in [
            ("old", ApplyPoint::BeforeFirstFinalCheck),
            ("stage", ApplyPoint::BeforeFirstFinalCheck),
            ("old", ApplyPoint::BeforeSecondFinalCheck),
            ("stage", ApplyPoint::BeforeSecondFinalCheck),
        ] {
            let fixture = Fixture::new(&format!("phase-c-permission-{target}-{point:?}"));
            let patch = fixture.initialize_and_patch("permissions");
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let original_permissions = std::fs::metadata(fixture.active()).unwrap().permissions();
            let error = apply_with_hook(&fixture.root, &patch, |observed, active, staged, _| {
                if observed == point {
                    let path = if target == "old" {
                        active
                    } else {
                        staged.unwrap()
                    };
                    let mut permissions = std::fs::metadata(path)?.permissions();
                    permissions.set_readonly(!permissions.readonly());
                    std::fs::set_permissions(path, permissions)?;
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-G153");
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
            std::fs::set_permissions(fixture.active(), original_permissions.clone()).unwrap();
            for entry in
                std::fs::read_dir(fixture.root.join(".semaprax-workspace/staging")).unwrap()
            {
                let path = entry.unwrap().path();
                if path.is_file() {
                    std::fs::set_permissions(path, original_permissions.clone()).unwrap();
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn phase_c_success_preserves_active_mode() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("phase-c-mode");
        let patch = fixture.initialize_and_patch("mode");
        let active = fixture.active();
        std::fs::set_permissions(&active, std::fs::Permissions::from_mode(0o640)).unwrap();
        apply(&fixture.root, &patch).unwrap();
        assert_eq!(
            std::fs::metadata(active).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn phase_b_skips_valid_residue_and_preserves_staging_objects() {
        let fixture = Fixture::new("phase-b-slots");
        let patch = fixture.initialize_and_patch("slots");
        let staging = fixture.root.join(".semaprax-workspace/staging");
        std::fs::write(staging.join("0"), "residue-zero\n").unwrap();
        std::fs::create_dir(staging.join("1")).unwrap();
        let mut observed = None;
        prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, slot, _| {
            if point == GenerationPoint::AfterSlotCreate {
                observed = slot.file_name().map(|name| name.to_owned());
            }
        })
        .unwrap();
        assert_eq!(observed.unwrap(), "2");
        assert_eq!(
            std::fs::read_to_string(staging.join("0")).unwrap(),
            "residue-zero\n"
        );
        assert!(staging.join("1").is_dir());
    }

    #[test]
    fn phase_b_rejects_staging_and_retention_exhaustion_without_active_change() {
        let fixture = Fixture::new("phase-b-exhausted");
        let patch = fixture.initialize_and_patch("exhausted");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let staging = fixture.root.join(".semaprax-workspace/staging");
        for ordinal in 0..super::MAX_STAGING_ATTEMPTS {
            std::fs::write(staging.join(ordinal.to_string()), "residue\n").unwrap();
        }
        let error = prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {})
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G151");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);

        for ordinal in 0..super::MAX_STAGING_ATTEMPTS {
            std::fs::remove_file(staging.join(ordinal.to_string())).unwrap();
        }
        let generations = fixture.root.join(".semaprax-workspace/generations");
        let active_generation = std::fs::read_dir(&generations)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        let mut made = 0usize;
        for ordinal in 0usize..64 {
            let name = format!("{ordinal:064x}");
            if name == active_generation.to_string_lossy() {
                continue;
            }
            std::fs::create_dir(generations.join(name)).unwrap();
            made += 1;
            if made == super::MAX_RETAINED_GENERATIONS - 1 {
                break;
            }
        }
        let error = prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {})
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G151");
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }

    #[test]
    fn phase_b_detects_manifest_file_and_destination_races_without_active_pivot() {
        for (label, point) in [
            ("manifest", GenerationPoint::AfterManifestWrite),
            ("file", GenerationPoint::AfterFilesWrite),
        ] {
            let fixture = Fixture::new(&format!("phase-b-{label}"));
            let patch = fixture.initialize_and_patch(label);
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let error = prepare_candidate_generation_with_hook(
                &fixture.root,
                &patch,
                |current, slot, _| {
                    if current == point {
                        let path = if label == "manifest" {
                            slot.join("manifest.json")
                        } else {
                            slot.join("files/alpha.spx")
                        };
                        let bytes = std::fs::read(&path).unwrap();
                        std::fs::remove_file(&path).unwrap();
                        std::fs::write(path, bytes).unwrap();
                    }
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-G153");
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
        }

        for kind in ["file", "directory"] {
            let fixture = Fixture::new(&format!("phase-b-destination-{kind}"));
            let patch = fixture.initialize_and_patch(&format!("destination_{kind}"));
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let mut foreign = None;
            let error = prepare_candidate_generation_with_hook(
                &fixture.root,
                &patch,
                |point, _, destination| {
                    if point == GenerationPoint::DestinationChecked {
                        if kind == "file" {
                            std::fs::write(destination, "foreign-generation\n").unwrap();
                        } else {
                            std::fs::create_dir(destination).unwrap();
                        }
                        foreign = Some(destination.to_path_buf());
                    }
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I211");
            let foreign = foreign.unwrap();
            assert_eq!(foreign.is_file(), kind == "file");
            assert_eq!(foreign.is_dir(), kind == "directory");
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
        }
    }

    #[test]
    fn phase_b_corrupt_existing_generation_fails_closed_without_staging_or_active_change() {
        let fixture = Fixture::new("phase-b-corrupt-reuse");
        let patch = fixture.initialize_and_patch("corrupt_reuse");
        let revision =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
        let candidate = fixture
            .root
            .join(".semaprax-workspace/generations")
            .join(super::revision_hex(&revision).unwrap());
        std::fs::write(candidate.join("manifest.json"), "{}\n").unwrap();
        let staging = fixture.root.join(".semaprax-workspace/staging");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let error = prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {})
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_eq!(std::fs::read_dir(staging).unwrap().count(), 0);
        assert_eq!(
            std::fs::read_to_string(candidate.join("manifest.json")).unwrap(),
            "{}\n"
        );
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }

    #[test]
    fn phase_b_rejects_extra_phase_inventory_and_preserves_foreign_objects() {
        for kind in ["staging", "generation"] {
            let fixture = Fixture::new(&format!("phase-b-extra-{kind}"));
            let patch = fixture.initialize_and_patch(&format!("extra_{kind}"));
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let mut foreign = None;
            let error =
                prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, _, _| {
                    if point == GenerationPoint::BeforeStageValidation {
                        let path = if kind == "staging" {
                            fixture.root.join(".semaprax-workspace/staging/31")
                        } else {
                            fixture.root.join(format!(
                                ".semaprax-workspace/generations/{:064x}",
                                65_535usize
                            ))
                        };
                        if kind == "staging" {
                            std::fs::write(&path, "foreign\n").unwrap();
                        } else {
                            std::fs::create_dir(&path).unwrap();
                        }
                        foreign = Some(path);
                    }
                })
                .unwrap_err();
            assert_eq!(error[0].code, "SPX-G153");
            assert!(foreign.unwrap().exists());
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
        }
    }

    #[cfg(unix)]
    #[test]
    fn phase_b_rejects_staged_symlink_and_hardlink_aliases() {
        use std::os::unix::fs::symlink;

        for kind in ["symlink", "hardlink"] {
            let fixture = Fixture::new(&format!("phase-b-{kind}"));
            let patch = fixture.initialize_and_patch(kind);
            let active_bytes = std::fs::read(fixture.active()).unwrap();
            let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
            let error =
                prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, slot, _| {
                    if point == GenerationPoint::AfterFilesWrite {
                        let alpha = slot.join("files/alpha.spx");
                        std::fs::remove_file(&alpha).unwrap();
                        if kind == "symlink" {
                            symlink(fixture.root.join("alpha.spx"), &alpha).unwrap();
                        } else {
                            std::fs::hard_link(slot.join("files/beta.spx"), &alpha).unwrap();
                        }
                    }
                })
                .unwrap_err();
            assert_eq!(error[0].code, "SPX-G153");
            assert_active_unchanged(&fixture, &active_bytes, active_identity);
        }
    }

    #[test]
    fn phase_b_post_publish_corruption_is_reported_without_active_pivot() {
        let fixture = Fixture::new("phase-b-post-publish");
        let patch = fixture.initialize_and_patch("post_publish");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let mut published = None;
        let error = prepare_candidate_generation_with_hook(
            &fixture.root,
            &patch,
            |point, _, destination| {
                if point == GenerationPoint::AfterGenerationPublish {
                    let manifest = destination.join("manifest.json");
                    let bytes = std::fs::read(&manifest).unwrap();
                    std::fs::remove_file(&manifest).unwrap();
                    std::fs::write(&manifest, bytes).unwrap();
                    published = Some(destination.to_path_buf());
                }
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(published.unwrap().exists());
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }

    #[test]
    fn injected_small_inventory_limit_stops_before_unbounded_collection() {
        let fixture = Fixture::new("inventory-limit");
        for name in ["one", "two", "three"] {
            std::fs::write(fixture.root.join(name), "x").unwrap();
        }
        let error = count_entries_bounded(&fixture.root, 2).unwrap_err();
        assert_eq!(error[0].code, "SPX-G151");
    }

    #[test]
    fn manifest_bound_rejects_expansion() {
        let fact = FileFact {
            path: "alpha.spx".to_owned(),
            module: "workspace.alpha".to_owned(),
            source_graph_schema: "x".repeat(super::MAX_MANIFEST_BYTES),
            source_revision: format!("sha256:{}", "0".repeat(64)),
            source_digest: format!("sha256:{}", "0".repeat(64)),
            source: String::new(),
            declarations: Vec::new(),
            declaration_count: 0,
            callable_count: 0,
            call_count: 0,
        };
        let error = bounded_manifest(&[fact]).unwrap_err();
        assert_eq!(error[0].code, "SPX-G151");
    }

    #[test]
    fn managed_path_count_accepts_exact_and_rejects_over() {
        let render = |count: usize| {
            format!(
                "{{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{}]}}\n",
                (0..count)
                    .map(|index| format!("{{\"path\":\"file{index:02}.spx\"}}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        assert_eq!(parse_path_set(&render(16)).unwrap().len(), 16);
        assert_eq!(parse_path_set(&render(17)).unwrap_err()[0].code, "SPX-G151");
    }

    #[test]
    fn aggregate_callable_budget_accepts_exact_and_rejects_over_before_hir() {
        let exact = vec![
            module_with_callables("workspace.budget_a", 512),
            module_with_callables("workspace.budget_b", 512),
        ];
        assert_eq!(file_facts(exact, true).unwrap().len(), 2);
        let over = vec![
            module_with_callables("workspace.budget_c", 512),
            module_with_callables("workspace.budget_d", 513),
        ];
        let Err(error) = file_facts(over, true) else {
            panic!("over-limit aggregate callables must fail");
        };
        assert_eq!(error[0].code, "SPX-G151");
    }

    fn module_with_callables(module: &str, count: usize) -> (String, String) {
        let path = format!("{}.spx", module.replace('.', "_"));
        let mut source = format!("module {module};\n");
        for index in 0..count.saturating_sub(1) {
            source.push_str(&format!("fn helper{index}()->i64{{{index}}}\n"));
        }
        source.push_str("fn main()->i64{0}\n");
        (path.clone(), canonical(&source, &path))
    }

    #[test]
    fn staging_and_retained_inventory_bounds_are_exact() {
        let fixture = Fixture::new("inventory-exact-over");
        let staging = fixture.root.join("staging-bounds");
        std::fs::create_dir(&staging).unwrap();
        for attempt in 0..super::MAX_STAGING_ATTEMPTS {
            std::fs::create_dir(staging.join(attempt.to_string())).unwrap();
        }
        assert_eq!(validate_staging_inventory(&staging).unwrap().0, 32);
        std::fs::create_dir(staging.join("32")).unwrap();
        let Err(error) = validate_staging_inventory(&staging) else {
            panic!("over-limit staging inventory must fail");
        };
        assert!(matches!(error[0].code, "SPX-G151" | "SPX-G153"));

        let retained = fixture.root.join("retained-bounds");
        std::fs::create_dir(&retained).unwrap();
        std::fs::create_dir(retained.join("0".repeat(64))).unwrap();
        std::fs::create_dir(retained.join("1".repeat(64))).unwrap();
        assert_eq!(count_directories_bounded(&retained, 2).unwrap().0, 2);
        let Err(error) = count_directories_bounded(&retained, 1) else {
            panic!("over-limit retained generations must fail");
        };
        assert_eq!(error[0].code, "SPX-G151");
    }

    #[cfg(windows)]
    #[test]
    fn canonical_root_rejects_windows_directory_reparse_points() {
        let fixture = Fixture::new("root-reparse");
        let alias = fixture.root.with_extension("reparse");
        std::os::windows::fs::symlink_dir(&fixture.root, &alias).unwrap();
        let error = canonical_root(&alias).unwrap_err();
        assert_eq!(error[0].code, "SPX-I209");
        std::fs::remove_dir(&alias).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn phase_b_rejects_nested_windows_junction_and_preserves_its_target() {
        use std::process::Command;

        let fixture = Fixture::new("phase-b-windows-junction");
        let patch = fixture.initialize_and_patch("windows_junction");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let foreign = fixture.root.join("foreign-junction-target");
        let mut junction = None;
        let error = prepare_candidate_generation_with_hook(
            &fixture.root,
            &patch,
            |point, slot, destination| {
                if point == GenerationPoint::DestinationChecked {
                    let files = slot.join("files");
                    std::fs::rename(&files, &foreign).unwrap();
                    let status = Command::new("cmd")
                        .args(["/C", "mklink", "/J"])
                        .arg(&files)
                        .arg(&foreign)
                        .status()
                        .unwrap();
                    assert!(status.success(), "mklink /J failed");
                    junction = Some(destination.join("files"));
                }
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(foreign.join("alpha.spx").is_file());
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
        let junction = junction.unwrap();
        assert!(super::metadata_is_reparse(
            &std::fs::symlink_metadata(&junction).unwrap()
        ));
        std::fs::remove_dir(junction).unwrap();
        assert!(foreign.join("alpha.spx").is_file());
    }

    #[cfg(windows)]
    #[test]
    fn windows_held_handle_relocation_publishes_exact_initializer_and_candidate_maps() {
        let fixture = Fixture::new("windows-relocation-success");
        let patch = fixture.initialize_and_patch("windows_relocation");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let candidate =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |_, _, _| {}).unwrap();
        assert!(fixture
            .root
            .join(".semaprax-workspace/generations")
            .join(super::revision_hex(&candidate).unwrap())
            .is_dir());
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
    }
}
