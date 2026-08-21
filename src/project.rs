//! Bounded, invocation-local Project v1 input authority.
//!
//! A project names every source explicitly. Loading authenticates those exact
//! files, runs the existing Semantic Workspace Phase-A build once in memory,
//! and retains held identities for a final caller-boundary recheck. It creates
//! no managed workspace and grants no publication authority.

use std::collections::BTreeSet;
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use same_file::Handle;
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::{self, SemanticWorkspaceSource};

pub const PROJECT_SCHEMA: &str = "semaprax.project.v1";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_NAME_BYTES: usize = 64;
pub const MAX_MODULE_BYTES: usize = 240;
pub const MAX_PATH_BYTES: usize = 240;
pub const MAX_STABLE_ID_BYTES: usize = 128;
pub const MAX_SOURCES: usize = 16;
pub const MAX_WEB_EXPORTS: usize = 32;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;

const MANIFEST_FILE: &str = "semaprax.toml";
const MAX_HELD_DIRECTORIES: usize = 128;

/// The exact, closed Project v1 manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectManifest {
    name: String,
    entry: String,
    sources: Vec<String>,
    web_exports: Vec<String>,
    test_module: String,
}

impl ProjectManifest {
    /// Parse the fixed canonical TOML subset used by Project v1.
    pub fn parse(source: &str) -> Result<Self, Vec<Diagnostic>> {
        if source.len() > MAX_MANIFEST_BYTES {
            return Err(capacity("manifest_bytes", MAX_MANIFEST_BYTES));
        }
        if source.as_bytes().contains(&0) || source.starts_with('\u{feff}') || source.contains('\r')
        {
            return Err(grammar("Project v1 manifest is not canonical UTF-8 TOML"));
        }
        let lines = source.split('\n').collect::<Vec<_>>();
        if lines.len() != 7 || lines.last() != Some(&"") {
            return Err(grammar(
                "Project v1 manifest must contain exactly six ordered assignments and one terminal LF",
            ));
        }
        let schema = parse_string_assignment(lines[0], "schema")?;
        let name = parse_string_assignment(lines[1], "name")?;
        let entry = parse_string_assignment(lines[2], "entry")?;
        let sources = parse_array_assignment(lines[3], "sources")?;
        let web_exports = parse_array_assignment(lines[4], "web_exports")?;
        let tests = parse_array_assignment(lines[5], "tests")?;

        if schema != PROJECT_SCHEMA {
            return Err(grammar(
                "Project v1 manifest schema is not semaprax.project.v1",
            ));
        }
        if !valid_name(&name) {
            return Err(grammar(
                "Project v1 name must match lowercase [a-z][a-z0-9-]* and contain 1..=64 bytes",
            ));
        }
        if !valid_module(&entry) {
            return Err(grammar("Project v1 entry is not a bounded module name"));
        }
        if !(2..=MAX_SOURCES).contains(&sources.len()) {
            return Err(if sources.len() > MAX_SOURCES {
                capacity("sources", MAX_SOURCES)
            } else {
                grammar("Project v1 requires 2..=16 explicit source paths")
            });
        }
        require_strict_order(&sources, "source paths")?;
        for path in &sources {
            if path.len() > MAX_PATH_BYTES
                || !path.ends_with(".spx")
                || !crate::workspace::evidence_path_is_valid(path)
            {
                return Err(grammar(
                    "Project v1 source paths must be canonical relative .spx paths of at most 240 bytes",
                ));
            }
        }
        if !(1..=MAX_WEB_EXPORTS).contains(&web_exports.len()) {
            return Err(if web_exports.len() > MAX_WEB_EXPORTS {
                capacity("web_exports", MAX_WEB_EXPORTS)
            } else {
                grammar("Project v1 requires 1..=32 explicit web export identities")
            });
        }
        require_strict_order(&web_exports, "web export identities")?;
        if web_exports.iter().any(|id| !valid_stable_id(id)) {
            return Err(grammar(
                "Project v1 web exports must use bounded lowercase [a-z0-9._-] stable IDs",
            ));
        }
        if tests.len() != 1 || !valid_module(&tests[0]) {
            return Err(grammar(
                "Project v1 tests must contain exactly one bounded module name",
            ));
        }
        if entry == tests[0] {
            return Err(grammar(
                "Project v1 entry and test modules must be distinct",
            ));
        }

        let manifest = Self {
            name,
            entry,
            sources,
            web_exports,
            test_module: tests.into_iter().next().expect("one test module"),
        };
        if manifest.to_canonical_toml() != source {
            return Err(grammar("Project v1 manifest is not canonical"));
        }
        Ok(manifest)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    pub fn web_exports(&self) -> &[String] {
        &self.web_exports
    }

    pub fn test_module(&self) -> &str {
        &self.test_module
    }

    pub fn to_canonical_toml(&self) -> String {
        format!(
            "schema = \"{PROJECT_SCHEMA}\"\nname = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
            self.name,
            self.entry,
            render_array(&self.sources),
            render_array(&self.web_exports),
            self.test_module,
        )
    }
}

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
    declared_inputs: Vec<DeclaredPathSelection>,
    held_manifest: HeldFile,
    held_sources: Vec<HeldFile>,
    held_directories: Vec<HeldDirectory>,
    published: bool,
}

impl ProjectSnapshot {
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

    /// Report successful admission. The linked scalar profile was validated
    /// before this snapshot became observable.
    pub fn check(&self) -> Result<(), Vec<Diagnostic>> {
        Ok(())
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
        self.published = true;
        self.recheck().map_err(publication_uncertainty)
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
    let published = snapshot.published;
    let recheck = snapshot.recheck().map_err(|drift| {
        if published {
            publication_uncertainty(drift)
        } else {
            drift
        }
    });
    match (result, recheck) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(drift)) => Err(drift),
        (Err(primary), Ok(())) => Err(primary),
        (Err(mut primary), Err(mut drift)) => {
            if !primary
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-J103")
            {
                primary.append(&mut drift);
            }
            Err(primary)
        }
    }
}

fn load_snapshot(manifest_path: &Path) -> Result<ProjectSnapshot, Vec<Diagnostic>> {
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

    let mut held_sources = Vec::with_capacity(manifest.sources.len());
    let mut declared_inputs = vec![manifest_selection];
    let mut workspace_sources = Vec::with_capacity(manifest.sources.len());
    let mut seen_directories = root_ancestors.into_iter().collect::<BTreeSet<_>>();
    let mut total_source_bytes = 0usize;
    for relative in &manifest.sources {
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
        let mut held = HeldFile::open(path, MAX_TOTAL_SOURCE_BYTES)?;
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

    let path_set = semantic_workspace::render_path_set(&manifest.sources)?;
    let preflight = semantic_workspace::preflight_owned(&path_set, workspace_sources)?;
    let (files, workspace_manifest, workspace_revision, graph) = preflight.into_snapshot_parts();
    let (entry_program, test_program) =
        graph.into_linked_scalar_programs(manifest.entry(), manifest.test_module())?;
    crate::wasm::emit_resolved_module_with_scalar_exports(&entry_program, manifest.web_exports())
        .map_err(|error| vec![error])?;
    let sources = files
        .into_iter()
        .map(|file| {
            let (path, source_graph_schema, source_revision, source_digest, source) =
                file.into_parts();
            ProjectSource {
                path,
                source_graph_schema,
                source_revision,
                source_digest,
                source,
            }
        })
        .collect();
    let project_revision = project_revision(&manifest.to_canonical_toml(), &workspace_revision);
    let mut snapshot = ProjectSnapshot {
        root,
        manifest,
        sources,
        workspace_manifest,
        workspace_revision,
        project_revision,
        entry_program,
        test_program,
        declared_inputs,
        held_manifest,
        held_sources,
        held_directories,
        published: false,
    };
    snapshot.recheck()?;
    Ok(snapshot)
}

fn project_revision(manifest: &str, workspace_revision: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"semaprax.project-revision.v1\0");
    digest.update((manifest.len() as u64).to_le_bytes());
    digest.update(manifest.as_bytes());
    digest.update((workspace_revision.len() as u64).to_le_bytes());
    digest.update(workspace_revision.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

struct HeldFile {
    path: PathBuf,
    file: File,
    identity: Handle,
    permissions: PermissionFingerprint,
    limit: usize,
    expected_len: usize,
    expected_sha256: [u8; 32],
    bytes: Vec<u8>,
}

impl HeldFile {
    fn open(path: PathBuf, limit: usize) -> Result<Self, Vec<Diagnostic>> {
        let before = std::fs::symlink_metadata(&path).map_err(|error| {
            authentication(format!("cannot inspect {}: {error}", path.display()))
        })?;
        if !plain_regular(&before) {
            return Err(authentication(format!(
                "Project v1 input {} must be a regular non-symlink file",
                path.display()
            )));
        }
        if !single_link(&path, &before) {
            return Err(authentication(format!(
                "Project v1 input {} must have exactly one hard link",
                path.display()
            )));
        }
        if before.len() > limit as u64 {
            return Err(capacity("input_bytes", limit));
        }
        let mut file = File::open(&path)
            .map_err(|error| authentication(format!("cannot open {}: {error}", path.display())))?;
        let identity = Handle::from_file(file.try_clone().map_err(|error| {
            authentication(format!("cannot retain {}: {error}", path.display()))
        })?)
        .map_err(|error| authentication(format!("cannot identify {}: {error}", path.display())))?;
        let after = std::fs::symlink_metadata(&path).map_err(|error| {
            authentication(format!("cannot recheck {}: {error}", path.display()))
        })?;
        if !plain_regular(&after)
            || !single_link(&path, &after)
            || Handle::from_path(&path).map_err(|error| {
                authentication(format!("cannot identify {}: {error}", path.display()))
            })? != identity
        {
            return Err(authentication(format!(
                "Project v1 input {} changed while opening",
                path.display()
            )));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take((limit as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| authentication(format!("cannot read {}: {error}", path.display())))?;
        if bytes.len() > limit {
            return Err(capacity("input_bytes", limit));
        }
        let expected_sha256 = Sha256::digest(&bytes).into();
        let expected_len = bytes.len();
        Ok(Self {
            path,
            file,
            identity,
            permissions: PermissionFingerprint::from_metadata(&after),
            limit,
            expected_len,
            expected_sha256,
            bytes,
        })
    }

    fn utf8(&mut self) -> Result<String, Vec<Diagnostic>> {
        String::from_utf8(std::mem::take(&mut self.bytes))
            .map_err(|_| authentication("Project v1 input is not UTF-8"))
    }

    fn recheck(&mut self) -> Result<(), Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            authentication(format!("cannot recheck {}: {error}", self.path.display()))
        })?;
        if !plain_regular(&metadata)
            || !single_link(&self.path, &metadata)
            || PermissionFingerprint::from_metadata(&metadata) != self.permissions
            || Handle::from_path(&self.path).map_err(|error| {
                authentication(format!("cannot identify {}: {error}", self.path.display()))
            })? != self.identity
        {
            return Err(authentication(format!(
                "Project v1 input {} identity or permissions changed",
                self.path.display()
            )));
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| authentication(format!("cannot seek held input: {error}")))?;
        let mut observed = Vec::new();
        self.file
            .by_ref()
            .take((self.limit as u64) + 1)
            .read_to_end(&mut observed)
            .map_err(|error| authentication(format!("cannot reread held input: {error}")))?;
        if observed.len() > self.limit {
            return Err(authentication(format!(
                "Project v1 input {} grew beyond its authenticated bound",
                self.path.display()
            )));
        }
        if observed.len() != self.expected_len
            || <[u8; 32]>::from(Sha256::digest(&observed)) != self.expected_sha256
        {
            return Err(authentication(format!(
                "Project v1 input {} bytes changed",
                self.path.display()
            )));
        }
        Ok(())
    }
}

struct DeclaredPathSelection {
    declared_path: PathBuf,
    canonical_path: PathBuf,
    identity: Handle,
    parents: Vec<HeldDirectory>,
}

impl DeclaredPathSelection {
    fn open(path: &Path, subject: &str) -> Result<Self, Vec<Diagnostic>> {
        let declared_path = declared_absolute_path(path, subject)?;
        let parent = declared_path.parent().ok_or_else(|| {
            grammar(format!(
                "Project v1 {subject} path must have an explicit parent"
            ))
        })?;
        let mut parent_paths = parent
            .ancestors()
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        if parent_paths.len() > MAX_HELD_DIRECTORIES {
            return Err(capacity(
                "declared_parent_directories",
                MAX_HELD_DIRECTORIES,
            ));
        }
        parent_paths.reverse();
        let parents = parent_paths
            .into_iter()
            .map(HeldDirectory::open)
            .collect::<Result<Vec<_>, _>>()?;
        let metadata = std::fs::symlink_metadata(&declared_path).map_err(|error| {
            authentication(format!(
                "cannot inspect declared Project v1 {subject} {}: {error}",
                declared_path.display()
            ))
        })?;
        if !plain_regular(&metadata) || !single_link(&declared_path, &metadata) {
            return Err(authentication(format!(
                "declared Project v1 {subject} {} must select one regular non-link file",
                declared_path.display()
            )));
        }
        let identity = Handle::from_path(&declared_path).map_err(|error| {
            authentication(format!(
                "cannot identify declared Project v1 {subject} {}: {error}",
                declared_path.display()
            ))
        })?;
        let canonical_path = std::fs::canonicalize(&declared_path).map_err(|error| {
            authentication(format!(
                "cannot canonicalize Project v1 {subject} {}: {error}",
                declared_path.display()
            ))
        })?;
        #[cfg(not(windows))]
        if canonical_path != declared_path {
            return Err(authentication(format!(
                "Project v1 {subject} path {} is not canonically spelled",
                path.display()
            )));
        }
        Ok(Self {
            declared_path,
            canonical_path,
            identity,
            parents,
        })
    }

    fn recheck(&self) -> Result<(), Vec<Diagnostic>> {
        for parent in &self.parents {
            parent.recheck()?;
        }
        let metadata = std::fs::symlink_metadata(&self.declared_path).map_err(|error| {
            authentication(format!(
                "cannot recheck declared input {}: {error}",
                self.declared_path.display()
            ))
        })?;
        if !plain_regular(&metadata)
            || !single_link(&self.declared_path, &metadata)
            || Handle::from_path(&self.declared_path).map_err(|error| {
                authentication(format!(
                    "cannot identify declared input {}: {error}",
                    self.declared_path.display()
                ))
            })? != self.identity
        {
            return Err(authentication(format!(
                "Project v1 declared input {} selection changed",
                self.declared_path.display()
            )));
        }
        Ok(())
    }
}

fn declared_absolute_path(path: &Path, subject: &str) -> Result<PathBuf, Vec<Diagnostic>> {
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(grammar(format!(
            "Project v1 {subject} path must not contain `.` or `..` components"
        )));
    }
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| authentication(format!("cannot inspect current directory: {error}")))?
            .join(path)
    })
}

struct HeldDirectory {
    path: PathBuf,
    identity: Handle,
    permissions: PermissionFingerprint,
}

impl HeldDirectory {
    fn open(path: PathBuf) -> Result<Self, Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            authentication(format!(
                "cannot inspect directory {}: {error}",
                path.display()
            ))
        })?;
        if !plain_directory(&metadata) {
            return Err(authentication(format!(
                "Project v1 ancestor {} must be a real directory",
                path.display()
            )));
        }
        let identity = Handle::from_path(&path).map_err(|error| {
            authentication(format!(
                "cannot identify directory {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self {
            path,
            identity,
            permissions: PermissionFingerprint::from_metadata(&metadata),
        })
    }

    fn recheck(&self) -> Result<(), Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            authentication(format!(
                "cannot recheck directory {}: {error}",
                self.path.display()
            ))
        })?;
        if !plain_directory(&metadata)
            || PermissionFingerprint::from_metadata(&metadata) != self.permissions
            || Handle::from_path(&self.path).map_err(|error| {
                authentication(format!(
                    "cannot identify directory {}: {error}",
                    self.path.display()
                ))
            })? != self.identity
        {
            return Err(authentication(format!(
                "Project v1 ancestor {} changed",
                self.path.display()
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PermissionFingerprint {
    readonly: bool,
    #[cfg(unix)]
    mode: u32,
}

impl PermissionFingerprint {
    fn from_metadata(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        Self {
            readonly: metadata.permissions().readonly(),
            #[cfg(unix)]
            mode: metadata.permissions().mode(),
        }
    }
}

fn plain_regular(metadata: &Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

fn plain_directory(metadata: &Metadata) -> bool {
    metadata.is_dir() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

#[cfg(unix)]
fn single_link(_: &Path, metadata: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}

#[cfg(windows)]
fn single_link(path: &Path, _: &Metadata) -> bool {
    winapi_util::Handle::from_path_any(path)
        .and_then(winapi_util::file::information)
        .is_ok_and(|information| information.number_of_links() == 1)
}

#[cfg(not(any(unix, windows)))]
fn single_link(_: &Path, _: &Metadata) -> bool {
    true
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(_: &Metadata) -> bool {
    false
}

fn parse_string_assignment(line: &str, key: &str) -> Result<String, Vec<Diagnostic>> {
    let prefix = format!("{key} = \"");
    let Some(value) = line
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(grammar(format!(
            "Project v1 manifest expected canonical `{key}` string assignment"
        )));
    };
    if value.contains(['"', '\\']) {
        return Err(grammar("Project v1 strings do not admit escapes"));
    }
    Ok(value.to_owned())
}

fn parse_array_assignment(line: &str, key: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
    let prefix = format!("{key} = [");
    let Some(body) = line
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(grammar(format!(
            "Project v1 manifest expected canonical `{key}` array assignment"
        )));
    };
    if body.is_empty() {
        return Ok(Vec::new());
    }
    body.split(", ")
        .map(|item| {
            let Some(value) = item
                .strip_prefix('"')
                .and_then(|item| item.strip_suffix('"'))
            else {
                return Err(grammar("Project v1 arrays contain only canonical strings"));
            };
            if value.is_empty() || value.contains(['"', '\\']) {
                return Err(grammar("Project v1 array strings are empty or escaped"));
            }
            Ok(value.to_owned())
        })
        .collect()
}

fn render_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn require_strict_order(values: &[String], subject: &str) -> Result<(), Vec<Diagnostic>> {
    if values.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(grammar(format!(
            "Project v1 {subject} must be strictly byte-sorted and unique"
        )))
    }
}

fn valid_name(value: &str) -> bool {
    (1..=MAX_NAME_BYTES).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_module(value: &str) -> bool {
    (1..=MAX_MODULE_BYTES).contains(&value.len())
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

fn valid_stable_id(value: &str) -> bool {
    (1..=MAX_STABLE_ID_BYTES).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn grammar(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J100", message)]
}

fn capacity(field: &str, limit: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-J101",
        format!("Project v1 `{field}` exceeds {limit}"),
    )]
}

fn authentication(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J102", message)]
}

fn publication_uncertainty(mut drift: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut diagnostics = vec![Diagnostic::io(
        "SPX-J103",
        "Project v1 inputs drifted after a complete digest-bound Web package was published",
    )];
    diagnostics.append(&mut drift);
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn manifest() -> String {
        "schema = \"semaprax.project.v1\"\nname = \"calculator\"\nentry = \"calculator.app\"\nsources = [\"a/core.spx\", \"t/tests.spx\", \"z/app.spx\"]\nweb_exports = [\"calculator.add\", \"calculator.divide\"]\ntests = [\"calculator.tests\"]\n".to_owned()
    }

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("t")).unwrap();
        std::fs::create_dir_all(root.join("z")).unwrap();
        std::fs::write(root.join(MANIFEST_FILE), manifest()).unwrap();
        std::fs::write(
            root.join("a/core.spx"),
            "module calculator.core;\n\n@id(\"calculator.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"calculator.divide\")\nfn divide(left: i64, right: i64) -> i64\n    requires right != 0\n{\n    left / right\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("t/tests.spx"),
            "module calculator.tests;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.tests.main\")\nfn main() -> i64\n{\n    if add(19, 23) == 42 { 0 } else { 1 }\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("z/app.spx"),
            "module calculator.app;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n",
        )
        .unwrap();
        root.canonicalize().unwrap()
    }

    fn file_inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn visit(root: &Path, directory: &Path, inventory: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let relative = path.strip_prefix(root).unwrap().to_string_lossy();
                let kind = entry.file_type().unwrap();
                if kind.is_dir() {
                    inventory.insert(format!("directory:{relative}"), Vec::new());
                    visit(root, &path, inventory);
                } else {
                    assert!(kind.is_file(), "unexpected inventory object {relative}");
                    inventory.insert(format!("file:{relative}"), std::fs::read(path).unwrap());
                }
            }
        }

        let mut inventory = BTreeMap::new();
        visit(root, root, &mut inventory);
        inventory
    }

    #[test]
    fn canonical_manifest_round_trips_and_rejects_confusion() {
        let parsed = ProjectManifest::parse(&manifest()).unwrap();
        assert_eq!(parsed.name(), "calculator");
        assert_eq!(parsed.entry(), "calculator.app");
        assert_eq!(parsed.test_module(), "calculator.tests");
        assert_eq!(parsed.to_canonical_toml(), manifest());

        for malformed in [
            manifest().replace("schema =", "unknown ="),
            manifest().replace(
                "name = \"calculator\"\nentry",
                "entry = \"calculator.app\"\nname",
            ),
            manifest().replace("a/core.spx\", \"t/tests.spx", "t/tests.spx\", \"a/core.spx"),
            manifest().replace(
                "calculator.add\", \"calculator.divide",
                "calculator.add\", \"calculator.add",
            ),
            manifest().replace("entry = \"calculator.app\"", "entry = \"calculator.tests\""),
            manifest().trim_end().to_owned(),
        ] {
            assert_eq!(
                ProjectManifest::parse(&malformed).unwrap_err()[0].code,
                "SPX-J100"
            );
        }
    }

    #[test]
    fn relative_manifest_is_resolved_but_aliased_components_are_rejected() {
        assert!(DeclaredPathSelection::open(Path::new("Cargo.toml"), "test").is_ok());

        let root = fixture();
        let dotted = root.join("z").join("..").join(MANIFEST_FILE);
        let error = with_authenticated_project(&dotted, |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J100");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_recheck_rejects_growth_without_unbounded_reading() {
        let root = fixture();
        let path = root.join("bounded-input");
        std::fs::write(&path, b"ok").unwrap();
        let mut held = HeldFile::open(path.clone(), 8).unwrap();
        std::fs::write(&path, b"123456789").unwrap();
        let error = held.recheck().unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn scalar_profile_is_admitted_before_the_operation_observes_a_snapshot() {
        let root = fixture();
        let changed = manifest().replace("calculator.divide", "calculator.missing");
        std::fs::write(root.join(MANIFEST_FILE), changed).unwrap();
        let called = std::cell::Cell::new(false);
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| {
            called.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(!called.get());
        assert!(error[0].code.starts_with("SPX-W"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn web_build_rechecks_inputs_before_publication() {
        let root = fixture();
        let output = root.with_extension("web-output");
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
            std::fs::write(root.join("z/app.spx"), "changed").unwrap();
            snapshot.build_web(&output)
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        assert!(!output.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn post_publication_drift_is_uncertain_but_preserves_the_complete_old_package() {
        let baseline_root = fixture();
        let baseline_output = baseline_root.with_extension("baseline-web");
        with_authenticated_project(&baseline_root.join(MANIFEST_FILE), |snapshot| {
            snapshot.build_web(&baseline_output)
        })
        .unwrap();
        let expected = file_inventory(&baseline_output);

        let root = fixture();
        let output = root.with_extension("uncertain-web");
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
            snapshot.build_web(&output)?;
            std::fs::write(root.join("z/app.spx"), "changed").unwrap();
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-J103");
        assert_eq!(file_inventory(&output), expected);
        assert_eq!(file_inventory(&output).len(), 7);

        let _ = std::fs::remove_dir_all(baseline_output);
        let _ = std::fs::remove_dir_all(baseline_root);
        let _ = std::fs::remove_dir_all(output);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn check_has_exactly_zero_workspace_or_source_side_effects() {
        let root = fixture();
        let before = file_inventory(&root);
        with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| snapshot.check()).unwrap();
        let after = file_inventory(&root);
        assert_eq!(after, before);
        assert!(!root.join(".semaprax-workspace").exists());
        for forbidden in [
            ".semaprax-workspace",
            "LOCK",
            "ACTIVE",
            "generations",
            "cache",
        ] {
            assert!(after.keys().all(|path| !path.contains(forbidden)));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_reuses_workspace_phase_a_and_rechecks_bytes() {
        let root = fixture();
        let manifest_path = root.join(MANIFEST_FILE);
        let revision = with_authenticated_project(&manifest_path, |snapshot| {
            assert_eq!(snapshot.sources().len(), 3);
            assert!(snapshot.workspace_manifest().ends_with('\n'));
            snapshot.check()?;
            assert_eq!(snapshot.entry_program().module, "calculator.app");
            assert_eq!(snapshot.test_program().module, "calculator.tests");
            assert!(snapshot.project_revision().starts_with("sha256:"));
            Ok(snapshot.workspace_revision().to_owned())
        })
        .unwrap();
        assert!(revision.starts_with("sha256:"));

        let error = with_authenticated_project(&manifest_path, |_| {
            std::fs::write(root.join("z/app.spx"), "changed").unwrap();
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| {
            std::fs::write(root.join("z/app.spx"), "changed").unwrap();
            Err::<(), _>(vec![Diagnostic::io("SPX-TEST", "primary")])
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-TEST");
        assert_eq!(error[1].code, "SPX-J102");
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn source_alias_and_duplicate_physical_identity_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = fixture();
        let target = root.join("a/core.spx");
        std::fs::remove_file(root.join("z/app.spx")).unwrap();
        symlink(&target, root.join("z/app.spx")).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_file(root.join("z/app.spx"));
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        std::fs::rename(root.join("a"), root.join("real-a")).unwrap();
        symlink(root.join("real-a"), root.join("a")).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_file(root.join("a"));
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        std::fs::rename(root.join(MANIFEST_FILE), root.join("real-manifest")).unwrap();
        symlink(root.join("real-manifest"), root.join(MANIFEST_FILE)).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_file(root.join(MANIFEST_FILE));
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| {
            std::fs::rename(root.join("z/app.spx"), root.join("z/selected-app.spx")).unwrap();
            symlink(root.join("z/selected-app.spx"), root.join("z/app.spx")).unwrap();
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_file(root.join("z/app.spx"));
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        let alias_root = root.with_extension("symlink-alias");
        symlink(&root, &alias_root).unwrap();
        let error =
            with_authenticated_project(&alias_root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        std::fs::remove_file(alias_root).unwrap();
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        std::fs::remove_file(root.join("a/core.spx")).unwrap();
        std::fs::hard_link(root.join(MANIFEST_FILE), root.join("a/core.spx")).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        std::fs::remove_file(root.join("z/app.spx")).unwrap();
        std::fs::hard_link(root.join("a/core.spx"), root.join("z/app.spx")).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        let external = root.with_extension("external-hardlink");
        std::fs::hard_link(root.join("a/core.spx"), &external).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        std::fs::remove_file(external).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_manifest_and_source_link_counts_are_one() {
        let root = fixture();
        let source_link = root.with_extension("windows-source-link");
        std::fs::hard_link(root.join("a/core.spx"), &source_link).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        std::fs::remove_file(source_link).unwrap();
        let _ = std::fs::remove_dir_all(root);

        let root = fixture();
        let manifest_link = root.with_extension("windows-manifest-link");
        std::fs::hard_link(root.join(MANIFEST_FILE), &manifest_link).unwrap();
        let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
        assert_eq!(error[0].code, "SPX-J102");
        std::fs::remove_file(manifest_link).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
