//! Managed immutable-generation workspace transactions.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::ast::{Program, TypeDeclarationKind};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{graph, hir, parse, patch, verify};

const CONTROL: &str = ".semaprax-workspace";
const PATH_SET_SCHEMA: &str = "semaprax.workspace-path-set.v1";
const ROOT_SCHEMA: &str = "semaprax.workspace-root.v1";
const MANIFEST_SCHEMA: &str = "semaprax.workspace-manifest.v1";
const PATCH_SCHEMA: &str = "semaprax.semantic-workspace-patch.v1";
const SNAPSHOT_SCHEMA: &str = "semaprax.workspace-snapshot.v1";
const PREVIEW_SCHEMA: &str = "semaprax.semantic-workspace-preview.v1";

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
#[allow(dead_code)]
struct WorkspacePatch {
    base: String,
    files: Vec<WorkspacePatchFile>,
    source: String,
    bytes: usize,
    digest: String,
}

#[allow(dead_code)]
struct WorkspacePlan {
    patch: WorkspacePatch,
    candidate: Vec<FileFact>,
    preflights: Vec<patch::PatchPreflight>,
    previews: BTreeMap<String, (String, String, String, String, String, String)>,
    candidate_manifest: String,
    candidate_revision: String,
    usage: (usize, usize, usize),
    candidate_bytes: usize,
    operations: usize,
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
        require_single_link(&path_metadata)?;
        require_single_link(&self.file.metadata().map_err(|error| {
            io(
                self.code,
                format!("cannot re-inspect {}: {error}", self.label),
            )
        })?)?;
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
    volume: u32,
    #[cfg(windows)]
    index: u64,
}

struct AuthenticatedDirectory {
    path: PathBuf,
    identity: FileIdentity,
    #[cfg(unix)]
    file: File,
    #[cfg(windows)]
    handle: same_file::Handle,
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
            let current = same_file::Handle::from_path(&self.path)
                .map_err(|error| io("SPX-I209", format!("cannot retain directory: {error}")))?;
            if current != self.handle {
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
}

#[allow(dead_code)]
struct PreparedGeneration {
    path: PathBuf,
    directories: Vec<AuthenticatedDirectory>,
    texts: Vec<AuthenticatedText>,
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
        let current = snapshot_authenticated(&self.root, &self.control, Some(&self.lock_identity))?;
        if current.snapshot.json != self.snapshot.json {
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

/// Initializes a managed workspace without modifying the original source files.
pub fn initialize(root: &Path, path_set_path: &Path) -> Result<String, Vec<Diagnostic>> {
    initialize_with_hook(root, path_set_path, |_| {})
}

#[derive(Clone, Copy)]
enum InitializePoint {
    BeforeGenerationPublish,
    BeforeActivePublish,
}

fn initialize_with_hook(
    root: &Path,
    path_set_path: &Path,
    mut hook: impl FnMut(InitializePoint),
) -> Result<String, Vec<Diagnostic>> {
    let root = canonical_root(root)?;
    let root_dir = authenticate_directory_held(&root)?;
    let mut paths_input = authenticate_text(path_set_path, MAX_MANIFEST_BYTES, "SPX-I209")?;
    let paths = parse_path_set(&paths_input.source)?;
    let mut total = 0usize;
    let mut sources = Vec::with_capacity(paths.len());
    let mut authenticated_sources = Vec::with_capacity(paths.len());
    for logical in paths {
        let input = authenticate_managed_source(
            &root,
            &logical,
            MAX_TOTAL_SOURCE_BYTES.saturating_sub(total),
        )?;
        let source = input.source.clone();
        total = total
            .checked_add(source.len())
            .ok_or_else(|| limit("source byte count overflow"))?;
        if total > MAX_TOTAL_SOURCE_BYTES {
            return Err(limit("workspace sources exceed 16777216 bytes"));
        }
        sources.push((logical, source));
        authenticated_sources.push(input);
    }
    let facts = file_facts(sources, true)?;
    validate_workspace_facts(&facts)?;
    let manifest = bounded_manifest(&facts)?;
    let revision = workspace_revision(&manifest);
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
    require_single_link(&lock.metadata().map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot inspect workspace LOCK: {error}"),
        )
    })?)?;
    let lock_identity = identity_from_file(&lock, "SPX-I209")?;
    lock_file(&lock, true)?;
    std::fs::create_dir(control.join("generations"))
        .map_err(|error| io("SPX-I211", format!("cannot create generations: {error}")))?;
    std::fs::create_dir(control.join("staging"))
        .map_err(|error| io("SPX-I211", format!("cannot create staging: {error}")))?;
    let generations_dir = authenticate_directory_held(&control.join("generations"))?;
    let staging_dir = authenticate_directory_held(&control.join("staging"))?;
    let slot = control.join("staging").join("0");
    std::fs::create_dir(&slot)
        .map_err(|error| io("SPX-I211", format!("cannot create staging slot: {error}")))?;
    write_generation(&slot, &manifest, &facts)?;
    paths_input.recheck()?;
    for source in &mut authenticated_sources {
        source.recheck()?;
    }
    require_distinct_text_identities(&authenticated_sources, Some(&paths_input), None)?;
    let mut staged = authenticate_generation_payload(&slot, &manifest, &facts, &revision)?;
    hook(InitializePoint::BeforeGenerationPublish);
    for input in &mut staged {
        input.recheck()?;
    }
    paths_input.recheck()?;
    for source in &mut authenticated_sources {
        source.recheck()?;
    }
    let generation = control.join("generations").join(revision_hex(&revision)?);
    std::fs::rename(&slot, &generation).map_err(|error| {
        io(
            "SPX-I211",
            format!("cannot publish initial generation: {error}"),
        )
    })?;
    let generation_dir = authenticate_directory_held(&generation)?;
    let generation_files_dir = authenticate_directory_held(&generation.join("files"))?;
    let mut published = authenticate_generation_payload(&generation, &manifest, &facts, &revision)?;
    for input in &mut published {
        input.recheck()?;
    }
    paths_input.recheck()?;
    for source in &mut authenticated_sources {
        source.recheck()?;
    }
    let active_stage = control.join("staging").join("0");
    write_new_file(&active_stage, render_root(&revision).as_bytes())?;
    let mut staged_active = authenticate_text(&active_stage, MAX_MANIFEST_BYTES, "SPX-I212")?;
    if parse_root(&staged_active.source)? != revision {
        return Err(invariant(
            "staged ACTIVE does not bind the initial generation",
        ));
    }
    staged_active.recheck()?;
    hook(InitializePoint::BeforeActivePublish);
    staged_active.recheck()?;
    if parse_root(&staged_active.source)? != revision {
        return Err(invariant(
            "staged ACTIVE changed before initial publication",
        ));
    }
    let original_nested_directories =
        authenticate_directory_trie(&root, facts.iter().map(|fact| fact.path.as_str()))?;
    let generation_nested_directories = authenticate_directory_trie(
        &generation.join("files"),
        facts.iter().map(|fact| fact.path.as_str()),
    )?;
    let mut initializing_identities = vec![
        &root_dir.identity,
        &control_dir.identity,
        &generations_dir.identity,
        &staging_dir.identity,
        &generation_dir.identity,
        &generation_files_dir.identity,
        &lock_identity,
        &staged_active.identity,
    ];
    initializing_identities.extend(published.iter().map(|input| &input.identity));
    initializing_identities.extend(authenticated_sources.iter().map(|input| &input.identity));
    initializing_identities.extend(
        original_nested_directories
            .iter()
            .map(|directory| &directory.identity),
    );
    initializing_identities.extend(
        generation_nested_directories
            .iter()
            .map(|directory| &directory.identity),
    );
    require_distinct_identities(&initializing_identities)?;
    require_same_volume(&initializing_identities)?;
    recheck_lock(&lock_path, &lock, &lock_identity)?;
    validate_initializing_control(&control)?;
    if validate_staging_inventory(&control.join("staging"))?.0 != 1 {
        return Err(invariant("initial ACTIVE staging inventory is not exact"));
    }
    paths_input.recheck()?;
    for source in &mut authenticated_sources {
        source.recheck()?;
    }
    for input in &mut published {
        input.recheck()?;
    }
    for directory in [
        &root_dir,
        &control_dir,
        &generations_dir,
        &staging_dir,
        &generation_dir,
        &generation_files_dir,
    ] {
        directory.recheck()?;
    }
    for directory in original_nested_directories
        .iter()
        .chain(generation_nested_directories.iter())
    {
        directory.recheck()?;
    }
    staged_active.recheck()?;
    if parse_root(&staged_active.source)? != revision {
        return Err(invariant(
            "staged ACTIVE changed before initial publication",
        ));
    }
    std::fs::rename(&active_stage, control.join("ACTIVE"))
        .map_err(|error| io("SPX-I212", format!("cannot publish ACTIVE: {error}")))?;
    let loaded =
        snapshot_authenticated(&root, &control, Some(&lock_identity)).map_err(|diagnostics| {
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
    Ok(revision)
}

/// Authenticates ACTIVE and returns an immutable owned workspace snapshot.
pub fn snapshot(root: &Path) -> Result<WorkspaceSnapshot, Vec<Diagnostic>> {
    snapshot_inner(root, false)
}

/// Previews one canonical workspace patch without creating candidate filesystem state.
pub fn preview(root: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut guard = acquire_snapshot(root, false)?;
    let mut patch_input = authenticate_text(patch_path, MAX_WORKSPACE_PATCH_BYTES, "SPX-I209")?;
    let workspace_patch = parse_workspace_patch(&patch_input.source)?;
    let plan = build_workspace_plan(&guard.snapshot, workspace_patch)?;
    let mut used_preview = 0usize;
    loop {
        let (report, overflowed) = crate::bounded_output::with_limit(MAX_PREVIEW_BYTES, || {
            render_preview(
                &guard.snapshot,
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
            patch_input.recheck()?;
            guard.recheck()?;
            return Ok(report);
        }
        used_preview = report.len();
    }
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
        preflights.push(preflight);
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
    Ok(WorkspacePlan {
        patch: workspace_patch,
        candidate,
        preflights,
        previews,
        candidate_manifest,
        candidate_revision,
        usage,
        candidate_bytes,
        operations,
    })
}

/// Phase C authority is intentionally unavailable until the immutable
/// generation builder and ACTIVE pivot pass their separate security gates.
pub fn apply(_root: &Path, _patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    Err(vec![Diagnostic::io(
        "SPX-I212",
        "workspace apply is not enabled before the ACTIVE-pivot gate",
    )])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum GenerationPoint {
    AfterSlotCreate,
    AfterManifestWrite,
    AfterFilesWrite,
    BeforeStageValidation,
    BeforeGenerationPublish,
    AfterGenerationPublish,
}

#[allow(dead_code)]
fn ensure_candidate_generation(
    guard: &mut WorkspaceGuard,
    patch_input: &mut AuthenticatedText,
    plan: &WorkspacePlan,
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
        || plan.preflights.len() != plan.patch.files.len()
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
    if std::fs::symlink_metadata(&destination).is_ok() {
        return Err(io(
            "SPX-I211",
            "candidate generation appeared before publication",
        ));
    }
    std::fs::rename(&slot, &destination).map_err(|error| {
        io(
            "SPX-I211",
            format!("cannot publish complete candidate generation: {error}"),
        )
    })?;
    hook(GenerationPoint::AfterGenerationPublish, &slot, &destination);
    let mut published = authenticate_expected_generation(&destination, plan, guard)?;
    require_relocated_identity_match(&authenticated, &published)?;
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
}

#[allow(dead_code)]
fn authenticate_expected_generation(
    path: &Path,
    plan: &WorkspacePlan,
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

#[allow(dead_code)]
fn require_relocated_identity_match(
    staged: &PreparedGeneration,
    published: &PreparedGeneration,
) -> Result<(), Vec<Diagnostic>> {
    if staged.directories.len() != published.directories.len()
        || staged.texts.len() != published.texts.len()
        || staged
            .directories
            .iter()
            .zip(&published.directories)
            .any(|(left, right)| left.identity != right.identity)
        || staged
            .texts
            .iter()
            .zip(&published.texts)
            .any(|(left, right)| left.identity != right.identity)
    {
        return Err(invariant(
            "published candidate generation is not the authenticated staged object set",
        ));
    }
    Ok(())
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
    Ok(guard.snapshot)
}

fn acquire_snapshot(root: &Path, exclusive: bool) -> Result<WorkspaceGuard, Vec<Diagnostic>> {
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
    require_single_link(&lock_metadata)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| io("SPX-I209", format!("cannot open workspace LOCK: {error}")))?;
    let lock_identity = identity_from_file(&lock, "SPX-I209")?;
    lock_file(&lock, exclusive)?;
    validate_control(&control)?;
    recheck_lock(&lock_path, &lock, &lock_identity)?;
    let authenticated = snapshot_authenticated(&root, &control, Some(&lock_identity))?;
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
    })
}

fn snapshot_authenticated(
    root: &Path,
    control: &Path,
    lock_identity: Option<&FileIdentity>,
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
    let revision = parse_root(&active.source)?;
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
    let manifest = parse_manifest(&manifest_input.source)?;
    let nested_directories =
        authenticate_directory_trie(&files_root, manifest.iter().map(|file| file.path.as_str()))?;
    if workspace_revision(&manifest_input.source) != revision {
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
    let facts = file_facts(sources, true)?;
    for (expected, fact) in manifest.iter().zip(&facts) {
        if fact.source_graph_schema != expected.source_graph_schema
            || fact.source_revision != expected.source_revision
            || fact.source_digest != expected.source_digest
            || fact.source.len() != expected.bytes
        {
            return Err(invariant("managed generation file disagrees with manifest"));
        }
    }
    validate_workspace_facts(&facts)?;
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
    snapshot.json = bounded_snapshot_json(&snapshot)?;
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
    if !(2..=MAX_MANAGED_FILES).contains(&values.len()) {
        return Err(limit("workspace patch must change 2..16 files"));
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
    let metadata = file
        .metadata()
        .map_err(|error| io("SPX-I211", format!("cannot inspect managed file: {error}")))?;
    require_single_link(&metadata)?;
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

fn authenticate_generation_payload(
    generation: &Path,
    manifest: &str,
    facts: &[FileFact],
    revision: &str,
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
    if manifest_input.source != manifest || workspace_revision(&manifest_input.source) != revision {
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
    let generation_directory = authenticate_directory_held(generation)?;
    let files_root = generation.join("files");
    let files_directory = authenticate_directory_held(&files_root)?;
    let mut directories = vec![generation_directory, files_directory];
    directories.extend(authenticate_directory_trie(
        &files_root,
        facts.iter().map(|fact| fact.path.as_str()),
    )?);
    let texts = authenticate_generation_payload(generation, manifest, facts, revision)?;
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
fn authenticate_text(
    path: &Path,
    max: usize,
    code: &'static str,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    authenticate_text_labeled(path, max, code, &path.display().to_string())
}

fn authenticate_text_labeled(
    path: &Path,
    max: usize,
    code: &'static str,
    label: &str,
) -> Result<AuthenticatedText, Vec<Diagnostic>> {
    let before = std::fs::symlink_metadata(path)
        .map_err(|error| io(code, format!("cannot inspect {label}: {error}")))?;
    if !before.is_file() || before.file_type().is_symlink() || metadata_is_reparse(&before) {
        return Err(io(code, "workspace input must be a real regular file"));
    }
    require_single_link(&before)?;
    if before.len() > max as u64 {
        return Err(limit("workspace input exceeds its byte limit"));
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
    require_single_link(&held)?;
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
    require_single_link(&after)?;
    let mut bytes = Vec::with_capacity(usize::try_from(held.len()).unwrap_or(max).min(max));
    std::io::Read::by_ref(&mut file)
        .take(max.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io(code, format!("cannot read {label}: {error}")))?;
    if bytes.len() > max {
        return Err(limit("workspace input exceeds its byte limit"));
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
    let handle = same_file::Handle::from_path(path).map_err(|error| {
        io(
            "SPX-I209",
            format!("cannot retain directory {}: {error}", path.display()),
        )
    })?;
    #[cfg(windows)]
    let identity = identity_from_path(path, "SPX-I209")?;
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
        use std::os::windows::fs::MetadataExt as _;
        let metadata = file
            .metadata()
            .map_err(|error| io(code, format!("cannot inspect held file: {error}")))?;
        let volume = metadata
            .volume_serial_number()
            .ok_or_else(|| io(code, "workspace volume identity is unavailable"))?;
        let index = metadata
            .file_index()
            .ok_or_else(|| io(code, "workspace file identity is unavailable"))?;
        Ok(FileIdentity { volume, index })
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
    use std::os::windows::fs::MetadataExt as _;
    let metadata = std::fs::metadata(path)
        .map_err(|error| io(code, format!("cannot identify {}: {error}", path.display())))?;
    let volume = metadata
        .volume_serial_number()
        .ok_or_else(|| io(code, "workspace volume identity is unavailable"))?;
    let index = metadata
        .file_index()
        .ok_or_else(|| io(code, "workspace file identity is unavailable"))?;
    Ok(FileIdentity { volume, index })
}

#[cfg(not(windows))]
fn identity_from_path(path: &Path, code: &'static str) -> Result<FileIdentity, Vec<Diagnostic>> {
    let file = File::open(path)
        .map_err(|error| io(code, format!("cannot identify {}: {error}", path.display())))?;
    identity_from_file(&file, code)
}

fn require_single_link(metadata: &std::fs::Metadata) -> Result<(), Vec<Diagnostic>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.nlink() != 1 {
            return Err(invariant(
                "workspace regular files must have link count one",
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.number_of_links() != Some(1) {
            return Err(invariant(
                "workspace regular files must have link count one",
            ));
        }
    }
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
    require_single_link(&metadata)?;
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
fn validate_logical_path(path: &str) -> Result<(), Vec<Diagnostic>> {
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
        acquire_snapshot, bounded_manifest, count_directories_bounded, count_entries_bounded,
        file_facts, identity_from_path, initialize, initialize_with_hook, parse_path_set,
        prepare_candidate_generation_with_hook, require_distinct_path_identities,
        validate_staging_inventory, FileFact, GenerationPoint, InitializePoint,
    };
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

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

    fn canonical(source: &str, path: &str) -> String {
        let program = crate::parse(source, Path::new(path)).unwrap();
        crate::format::canonical(&program)
    }

    #[test]
    fn source_mutation_hook_prevents_active_publication() {
        let fixture = Fixture::new("source-race");
        let source = fixture.root.join("alpha.spx");
        let error = initialize_with_hook(&fixture.root, &fixture.path_set, |point| {
            if matches!(point, InitializePoint::BeforeGenerationPublish) {
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
            if matches!(point, InitializePoint::BeforeGenerationPublish) {
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
            if matches!(point, InitializePoint::BeforeActivePublish) {
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
            if matches!(point, InitializePoint::BeforeActivePublish) {
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
            if matches!(point, InitializePoint::BeforeActivePublish) {
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

        let fixture = Fixture::new("phase-b-destination");
        let patch = fixture.initialize_and_patch("destination");
        let active_bytes = std::fs::read(fixture.active()).unwrap();
        let active_identity = identity_from_path(&fixture.active(), "SPX-I209").unwrap();
        let mut foreign = None;
        let error = prepare_candidate_generation_with_hook(
            &fixture.root,
            &patch,
            |point, _, destination| {
                if point == GenerationPoint::BeforeGenerationPublish {
                    std::fs::create_dir(destination).unwrap();
                    std::fs::write(destination.join("foreign"), "preserve\n").unwrap();
                    foreign = Some(destination.to_path_buf());
                }
            },
        )
        .unwrap_err();
        assert!(matches!(error[0].code, "SPX-I211" | "SPX-G153"));
        assert_eq!(
            std::fs::read_to_string(foreign.unwrap().join("foreign")).unwrap(),
            "preserve\n"
        );
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
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
        let error =
            prepare_candidate_generation_with_hook(&fixture.root, &patch, |point, slot, _| {
                if point == GenerationPoint::AfterFilesWrite {
                    let files = slot.join("files");
                    std::fs::rename(&files, &foreign).unwrap();
                    let status = Command::new("cmd")
                        .args(["/C", "mklink", "/J"])
                        .arg(&files)
                        .arg(&foreign)
                        .status()
                        .unwrap();
                    assert!(status.success(), "mklink /J failed");
                    junction = Some(files);
                }
            })
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert!(foreign.join("alpha.spx").is_file());
        assert_active_unchanged(&fixture, &active_bytes, active_identity);
        std::fs::remove_dir(junction.unwrap()).unwrap();
        assert!(foreign.join("alpha.spx").is_file());
    }
}
