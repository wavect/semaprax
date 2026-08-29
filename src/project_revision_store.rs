//! Explicit immutable persistence for already-authenticated Project revisions.
//!
//! This module owns no ambient root discovery and no reusable filesystem
//! authority. Each operation receives one absolute host-selected root and
//! either publishes or reads one content-addressed immutable entry.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::{ProjectManifest, ProjectRevision, ProjectSource};

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod unix;

pub const PROJECT_REVISION_STORE_ENTRY_SCHEMA: &str = "semaprax.project-revision-store-entry.v1";
pub const MAX_STORE_ENTRIES: usize = 32;
pub const MAX_STORE_MANIFEST_BYTES: usize = crate::project::MAX_MANIFEST_BYTES;
pub const MAX_STORE_WORKSPACE_MANIFEST_BYTES: usize = 1_048_576;
pub const MAX_STORE_SOURCES: usize = crate::project::MAX_SOURCES;
pub const MAX_STORE_TOTAL_SOURCE_BYTES: usize = crate::project::MAX_TOTAL_SOURCE_BYTES;
pub const MAX_STORE_SOURCE_PATH_BYTES: usize = crate::project::MAX_PATH_BYTES;
pub const MAX_STORE_SOURCE_PATH_DEPTH: usize = 16;
pub const MAX_STORE_ENTRY_JSON_BYTES: usize = 1_048_576;
pub const MAX_STORE_INVENTORY_ENTRIES: usize = 290;
pub const MAX_STORE_JSON_DEPTH: usize = 8;

const ENTRY_DIGEST_DOMAIN: &[u8] = b"semaprax.project-revision-store.entry-digest.v1\0";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"semaprax.project-revision-store.manifest-digest.v1\0";
const WORKSPACE_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"semaprax.project-revision-store.workspace-manifest-digest.v1\0";
const WORKSPACE_REVISION_DOMAIN: &[u8] = b"semaprax.workspace-semantic-revision.v1\0";
const PROJECT_REVISION_DOMAIN: &[u8] = b"semaprax.project-revision.v1\0";

const NONCLAIMS: &[&str] = &[
    "not_a_default_or_ambient_cache",
    "not_signature_authenticated_provenance_or_approval",
    "no_reusable_authorization_token",
    "no_network_process_tool_environment_template_patch_or_build_authority",
    "no_source_workspace_manifest_or_project_mutation",
    "no_daemon_transport_or_protocol_authority",
    "no_target_execution_or_artifact_publication",
    "no_dependency_registry_or_package_resolution",
    "no_raw_path_trust_or_symlink_traversal",
    "requires_trusted_exclusive_current_euid_root",
    "no_adoption_overwrite_cleanup_recovery_eviction_or_gc",
    "no_power_loss_network_nfs_overlay_or_durability_guarantee",
    "no_acl_xattr_or_ads_preservation",
    "no_windows_store_support",
    "no_external_consumer_compatibility_or_release_promotion",
];

/// A publication result without path, handle, or reusable authority.
#[derive(Debug)]
pub struct ProjectRevisionStoreReceipt {
    entry_digest: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
}

impl ProjectRevisionStoreReceipt {
    pub fn entry_digest(&self) -> &str {
        &self.entry_digest
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn project_graph_digest(&self) -> &str {
        &self.project_graph_digest
    }
}

/// Persist one exact immutable revision through an injected store root.
pub fn persist(
    root: &Path,
    revision: &ProjectRevision,
    expected_project_revision: &str,
) -> Result<ProjectRevisionStoreReceipt, Vec<Diagnostic>> {
    let prepared = PreparedEntry::from_revision(revision, expected_project_revision)?;
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        unix::persist(root, &prepared)?;
        Ok(prepared.receipt())
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = (root, prepared);
        Err(io(
            "Project Revision Store requires safe handle-relative Unix publication",
        ))
    }
}

/// Load and independently rebuild one exact immutable revision.
pub fn load(
    root: &Path,
    entry_digest: &str,
    expected_project_revision: &str,
) -> Result<ProjectRevision, Vec<Diagnostic>> {
    require_digest(entry_digest, "entry_digest")?;
    require_digest(expected_project_revision, "expected_project_revision")?;
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    {
        let stored = unix::load(root, entry_digest)?;
        replay_stored(stored, entry_digest, expected_project_revision)
    }
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    {
        let _ = root;
        Err(io(
            "Project Revision Store requires safe handle-relative Unix reads",
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredSource {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
    source: Vec<u8>,
}

struct PreparedEntry {
    project_schema: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    manifest: Vec<u8>,
    manifest_digest: String,
    workspace_manifest: Vec<u8>,
    workspace_manifest_digest: String,
    sources: Vec<StoredSource>,
    inventory_entries: usize,
    entry_json: Vec<u8>,
    entry_digest: String,
}

pub(super) struct StoredEntry {
    entry_json: Vec<u8>,
    manifest: Vec<u8>,
    workspace_manifest: Vec<u8>,
    sources: Vec<(String, Vec<u8>)>,
}

impl PreparedEntry {
    fn from_revision(
        revision: &ProjectRevision,
        expected_project_revision: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        require_digest(expected_project_revision, "expected_project_revision")?;
        if revision.project_revision() != expected_project_revision {
            return Err(replay("expected Project revision is stale or foreign"));
        }
        let manifest = revision.manifest().to_canonical_toml().into_bytes();
        require_max("manifest_bytes", manifest.len(), MAX_STORE_MANIFEST_BYTES)?;
        let sources = prepared_sources(revision.sources())?;
        let workspace_manifest = render_workspace_manifest(&sources)
            .map_err(|_| replay("Project Workspace source facts cannot be rendered exactly"))?
            .into_bytes();
        if workspace_manifest.as_slice() != revision.workspace_manifest().as_bytes() {
            return Err(replay(
                "Project Workspace manifest differs from independent source-fact replay",
            ));
        }
        let workspace_revision = framed_digest(WORKSPACE_REVISION_DOMAIN, &workspace_manifest);
        if workspace_revision != revision.workspace_revision() {
            return Err(replay(
                "Project Workspace revision differs from independent manifest replay",
            ));
        }
        let project_revision = project_revision(&manifest, &workspace_revision);
        if project_revision != revision.project_revision() {
            return Err(replay(
                "Project revision differs from independent manifest replay",
            ));
        }
        require_digest(revision.semantic_graph_digest(), "project_graph_digest")?;
        let inventory_entries = inventory_entries(&sources)?;
        let mut prepared = Self {
            project_schema: revision.manifest().schema().to_owned(),
            project_revision,
            workspace_revision,
            project_graph_digest: revision.semantic_graph_digest().to_owned(),
            manifest_digest: framed_digest(MANIFEST_DIGEST_DOMAIN, &manifest),
            workspace_manifest_digest: framed_digest(
                WORKSPACE_MANIFEST_DIGEST_DOMAIN,
                &workspace_manifest,
            ),
            manifest,
            workspace_manifest,
            sources,
            inventory_entries,
            entry_json: Vec::new(),
            entry_digest: String::new(),
        };
        prepared.entry_json = render_entry_fixed_point(&prepared)?;
        prepared.entry_digest = framed_digest(ENTRY_DIGEST_DOMAIN, &prepared.entry_json);
        Ok(prepared)
    }

    fn receipt(&self) -> ProjectRevisionStoreReceipt {
        ProjectRevisionStoreReceipt {
            entry_digest: self.entry_digest.clone(),
            project_revision: self.project_revision.clone(),
            workspace_revision: self.workspace_revision.clone(),
            project_graph_digest: self.project_graph_digest.clone(),
        }
    }

    fn entry_hex(&self) -> &str {
        self.entry_digest
            .strip_prefix("sha256:")
            .expect("prepared store digest is canonical")
    }
}

fn prepared_sources(sources: &[ProjectSource]) -> Result<Vec<StoredSource>, Vec<Diagnostic>> {
    require_max("sources", sources.len(), MAX_STORE_SOURCES)?;
    let mut total = 0usize;
    let mut result = Vec::with_capacity(sources.len());
    for source in sources {
        validate_source_path(source.path())?;
        total = total
            .checked_add(source.source().len())
            .ok_or_else(|| limit("total_source_bytes", MAX_STORE_TOTAL_SOURCE_BYTES))?;
        require_max("total_source_bytes", total, MAX_STORE_TOTAL_SOURCE_BYTES)?;
        let digest = crate::review::source_digest(source.source().as_bytes());
        if digest != source.source_digest() {
            return Err(replay(format!(
                "stored source fact for `{}` differs from exact bytes",
                source.path()
            )));
        }
        require_digest(source.source_revision(), "source_revision")?;
        result.push(StoredSource {
            path: source.path().to_owned(),
            source_graph_schema: source.source_graph_schema().to_owned(),
            source_revision: source.source_revision().to_owned(),
            source_digest: digest,
            bytes: source.source().len(),
            source: source.source().as_bytes().to_vec(),
        });
    }
    if result.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(grammar(
            "Project Revision Store source paths must be strictly sorted",
        ));
    }
    Ok(result)
}

fn render_workspace_manifest(sources: &[StoredSource]) -> Result<String, Vec<Diagnostic>> {
    let facts = sources
        .iter()
        .map(|source| {
            (
                source.path.as_str(),
                source.source_graph_schema.as_str(),
                source.source_revision.as_str(),
                source.source_digest.as_str(),
                source.bytes,
            )
        })
        .collect::<Vec<_>>();
    crate::semantic_workspace::render_manifest_facts(&facts)
}

fn render_entry_fixed_point(prepared: &PreparedEntry) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut expected_bytes = 0usize;
    for _ in 0..16 {
        let rendered = render_entry(prepared, expected_bytes);
        if rendered.len() > MAX_STORE_ENTRY_JSON_BYTES {
            return Err(limit("entry_json_bytes", MAX_STORE_ENTRY_JSON_BYTES));
        }
        if rendered.len() == expected_bytes {
            return Ok(rendered.into_bytes());
        }
        expected_bytes = rendered.len();
    }
    Err(replay(
        "Project Revision Store entry byte budget did not reach a fixed point",
    ))
}

fn render_entry(prepared: &PreparedEntry, entry_json_bytes: usize) -> String {
    format!(
        "{{\"schema\":{},\"project_schema\":{},\"project_revision\":{},\"workspace_manifest_schema\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"manifest\":{{\"digest\":{},\"bytes\":{}}},\"workspace_manifest\":{{\"digest\":{},\"bytes\":{}}},\"sources\":[{}],\"limits\":{{\"max_retained_entries\":32,\"max_stage_entries\":1,\"max_manifest_bytes\":65536,\"max_workspace_manifest_bytes\":1048576,\"max_sources\":16,\"max_total_source_bytes\":16777216,\"max_source_path_bytes\":240,\"max_source_path_depth\":16,\"max_entry_json_bytes\":1048576,\"max_inventory_entries\":290,\"max_json_depth\":8,\"max_unexpected_inventory_entries\":0}},\"budget\":{{\"used_retained_entries\":1,\"used_stage_entries\":0,\"used_manifest_bytes\":{},\"used_workspace_manifest_bytes\":{},\"used_sources\":{},\"used_total_source_bytes\":{},\"used_source_path_bytes\":{},\"used_source_path_depth\":{},\"used_entry_json_bytes\":{},\"used_inventory_entries\":{},\"used_json_depth\":4,\"used_unexpected_inventory_entries\":0}},\"nonclaims\":[{}]}}\n",
        quote_json(PROJECT_REVISION_STORE_ENTRY_SCHEMA),
        quote_json(&prepared.project_schema),
        quote_json(&prepared.project_revision),
        quote_json(crate::semantic_workspace::MANIFEST_SCHEMA),
        quote_json(&prepared.workspace_revision),
        quote_json(&prepared.project_graph_digest),
        quote_json(&prepared.manifest_digest),
        prepared.manifest.len(),
        quote_json(&prepared.workspace_manifest_digest),
        prepared.workspace_manifest.len(),
        prepared
            .sources
            .iter()
            .map(|source| format!(
                "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
                quote_json(&source.path),
                quote_json(&source.source_graph_schema),
                quote_json(&source.source_revision),
                quote_json(&source.source_digest),
                source.bytes
            ))
            .collect::<Vec<_>>()
            .join(","),
        prepared.manifest.len(),
        prepared.workspace_manifest.len(),
        prepared.sources.len(),
        prepared.sources.iter().map(|source| source.bytes).sum::<usize>(),
        prepared
            .sources
            .iter()
            .map(|source| source.path.len())
            .max()
            .unwrap_or(0),
        prepared
            .sources
            .iter()
            .map(|source| source.path.split('/').count())
            .max()
            .unwrap_or(0),
        entry_json_bytes,
        prepared.inventory_entries,
        NONCLAIMS
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn replay_stored(
    stored: StoredEntry,
    entry_digest: &str,
    expected_project_revision: &str,
) -> Result<ProjectRevision, Vec<Diagnostic>> {
    let header = parse_entry_header(&stored.entry_json)?;
    if header.project_revision != expected_project_revision {
        return Err(replay("stored Project revision is stale or foreign"));
    }
    let manifest = String::from_utf8(stored.manifest)
        .map_err(|_| grammar("stored Project manifest is not canonical UTF-8"))?;
    let workspace_manifest = String::from_utf8(stored.workspace_manifest)
        .map_err(|_| grammar("stored Workspace manifest is not canonical UTF-8"))?;
    let typed_manifest = ProjectManifest::parse(&manifest)
        .map_err(|_| grammar("stored Project manifest is not admitted"))?;
    if typed_manifest.schema() != header.project_schema {
        return Err(replay("stored Project schema disagrees with its manifest"));
    }
    if stored.sources.len() != header.sources.len() {
        return Err(authentication("stored source inventory is incomplete"));
    }
    let mut sources = Vec::with_capacity(header.sources.len());
    for (fact, (path, bytes)) in header.sources.iter().zip(stored.sources) {
        if fact.path != path || fact.bytes != bytes.len() {
            return Err(replay("stored source bytes disagree with entry metadata"));
        }
        let source = String::from_utf8(bytes)
            .map_err(|_| grammar("stored Project source is not canonical UTF-8"))?;
        if crate::review::source_digest(source.as_bytes()) != fact.source_digest {
            return Err(replay("stored source digest differs from exact bytes"));
        }
        sources.push(StoredSource {
            path: fact.path.clone(),
            source_graph_schema: fact.source_graph_schema.clone(),
            source_revision: fact.source_revision.clone(),
            source_digest: fact.source_digest.clone(),
            bytes: fact.bytes,
            source: source.into_bytes(),
        });
    }
    let independent_workspace_manifest = render_workspace_manifest(&sources)
        .map_err(|_| replay("stored Workspace source facts are not admitted"))?;
    if independent_workspace_manifest != workspace_manifest {
        return Err(replay(
            "stored Workspace manifest differs from source-fact replay",
        ));
    }
    if framed_digest(MANIFEST_DIGEST_DOMAIN, manifest.as_bytes()) != header.manifest_digest
        || framed_digest(
            WORKSPACE_MANIFEST_DIGEST_DOMAIN,
            workspace_manifest.as_bytes(),
        ) != header.workspace_manifest_digest
    {
        return Err(replay("stored manifest digest binding disagrees"));
    }
    let workspace_revision =
        framed_digest(WORKSPACE_REVISION_DOMAIN, workspace_manifest.as_bytes());
    if workspace_revision != header.workspace_revision {
        return Err(replay("stored Workspace revision binding disagrees"));
    }
    let project_revision = project_revision(manifest.as_bytes(), &workspace_revision);
    if project_revision != header.project_revision {
        return Err(replay("stored Project revision binding disagrees"));
    }
    let prepared = PreparedEntry {
        project_schema: header.project_schema.clone(),
        project_revision: header.project_revision.clone(),
        workspace_revision: header.workspace_revision.clone(),
        project_graph_digest: header.project_graph_digest.clone(),
        manifest: manifest.as_bytes().to_vec(),
        manifest_digest: header.manifest_digest.clone(),
        workspace_manifest: workspace_manifest.as_bytes().to_vec(),
        workspace_manifest_digest: header.workspace_manifest_digest.clone(),
        inventory_entries: inventory_entries(&sources)?,
        sources: sources.clone(),
        entry_json: Vec::new(),
        entry_digest: String::new(),
    };
    let canonical_entry = render_entry_fixed_point(&prepared)?;
    if canonical_entry != stored.entry_json
        || framed_digest(ENTRY_DIGEST_DOMAIN, &canonical_entry) != entry_digest
    {
        return Err(replay(
            "stored entry differs from canonical content-addressed replay",
        ));
    }
    let owned_sources = sources
        .into_iter()
        .map(|source| {
            Ok(crate::semantic_workspace::SemanticWorkspaceSource {
                path: source.path,
                source: String::from_utf8(source.source)
                    .map_err(|_| grammar("stored source is not UTF-8"))?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let revision = crate::project::rebuild_owned_revision(typed_manifest, owned_sources)
        .map_err(|_| replay("stored Project subject cannot be rebuilt exactly"))?;
    if revision.project_revision() != header.project_revision
        || revision.workspace_revision() != header.workspace_revision
        || revision.workspace_manifest() != workspace_manifest
        || revision.semantic_graph_digest() != header.project_graph_digest
    {
        return Err(replay(
            "rebuilt Project subject differs from the stored authenticated binding",
        ));
    }
    Ok(revision)
}

#[derive(Clone)]
struct EntryHeader {
    project_schema: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    manifest_digest: String,
    manifest_bytes: usize,
    workspace_manifest_digest: String,
    workspace_manifest_bytes: usize,
    sources: Vec<StoredSourceHeader>,
}

#[derive(Clone)]
struct StoredSourceHeader {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
}

fn parse_entry_header(bytes: &[u8]) -> Result<EntryHeader, Vec<Diagnostic>> {
    if bytes.len() > MAX_STORE_ENTRY_JSON_BYTES {
        return Err(limit("entry_json_bytes", MAX_STORE_ENTRY_JSON_BYTES));
    }
    if !bytes.ends_with(b"\n") || bytes[..bytes.len().saturating_sub(1)].contains(&b'\n') {
        return Err(grammar(
            "Project Revision Store entry must be one canonical JSON line with one terminal LF",
        ));
    }
    let source = std::str::from_utf8(bytes)
        .map_err(|_| grammar("Project Revision Store entry is not UTF-8"))?;
    if source.contains('\r') || source.starts_with('\u{feff}') {
        return Err(grammar(
            "Project Revision Store entry is not canonical JSON",
        ));
    }
    let value: Value = serde_json::from_str(source.trim_end_matches('\n'))
        .map_err(|_| grammar("Project Revision Store entry is not valid JSON"))?;
    if json_depth(&value) > MAX_STORE_JSON_DEPTH {
        return Err(limit("json_depth", MAX_STORE_JSON_DEPTH));
    }
    let object = value
        .as_object()
        .ok_or_else(|| grammar("Project Revision Store entry must be an object"))?;
    exact_keys(
        object,
        &[
            "schema",
            "project_schema",
            "project_revision",
            "workspace_manifest_schema",
            "workspace_revision",
            "project_graph_digest",
            "manifest",
            "workspace_manifest",
            "sources",
            "limits",
            "budget",
            "nonclaims",
        ],
    )?;
    if string(object, "schema")? != PROJECT_REVISION_STORE_ENTRY_SCHEMA
        || string(object, "workspace_manifest_schema")?
            != crate::semantic_workspace::MANIFEST_SCHEMA
    {
        return Err(grammar(
            "Project Revision Store entry schema is not admitted",
        ));
    }
    let project_revision = digest_field(object, "project_revision")?;
    let workspace_revision = digest_field(object, "workspace_revision")?;
    let project_graph_digest = digest_field(object, "project_graph_digest")?;
    let manifest = child(object, "manifest", &["digest", "bytes"])?;
    let workspace_manifest = child(object, "workspace_manifest", &["digest", "bytes"])?;
    let manifest_digest = digest_field(manifest, "digest")?;
    let workspace_manifest_digest = digest_field(workspace_manifest, "digest")?;
    let manifest_bytes = number(manifest, "bytes")?;
    let workspace_manifest_bytes = number(workspace_manifest, "bytes")?;
    if manifest_bytes > MAX_STORE_MANIFEST_BYTES {
        return Err(limit("manifest_bytes", MAX_STORE_MANIFEST_BYTES));
    }
    if workspace_manifest_bytes > MAX_STORE_WORKSPACE_MANIFEST_BYTES {
        return Err(limit(
            "workspace_manifest_bytes",
            MAX_STORE_WORKSPACE_MANIFEST_BYTES,
        ));
    }
    let source_values = object
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| grammar("Project Revision Store sources must be an array"))?;
    if source_values.len() > MAX_STORE_SOURCES {
        return Err(limit("sources", MAX_STORE_SOURCES));
    }
    let mut sources = Vec::with_capacity(source_values.len());
    let mut total = 0usize;
    for value in source_values {
        let source = value
            .as_object()
            .ok_or_else(|| grammar("Project Revision Store source fact must be an object"))?;
        exact_keys(
            source,
            &[
                "path",
                "source_graph_schema",
                "source_revision",
                "source_digest",
                "bytes",
            ],
        )?;
        let path = string(source, "path")?.to_owned();
        validate_source_path(&path)?;
        let bytes = number(source, "bytes")?;
        total = total
            .checked_add(bytes)
            .ok_or_else(|| limit("total_source_bytes", MAX_STORE_TOTAL_SOURCE_BYTES))?;
        if total > MAX_STORE_TOTAL_SOURCE_BYTES {
            return Err(limit("total_source_bytes", MAX_STORE_TOTAL_SOURCE_BYTES));
        }
        sources.push(StoredSourceHeader {
            path,
            source_graph_schema: string(source, "source_graph_schema")?.to_owned(),
            source_revision: digest_field(source, "source_revision")?,
            source_digest: digest_field(source, "source_digest")?,
            bytes,
        });
    }
    if sources.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(grammar(
            "Project Revision Store source facts must be strictly sorted",
        ));
    }
    Ok(EntryHeader {
        project_schema: string(object, "project_schema")?.to_owned(),
        project_revision,
        workspace_revision,
        project_graph_digest,
        manifest_digest,
        manifest_bytes,
        workspace_manifest_digest,
        workspace_manifest_bytes,
        sources,
    })
}

pub(super) fn source_inventory(bytes: &[u8]) -> Result<Vec<(String, usize)>, Vec<Diagnostic>> {
    Ok(parse_entry_header(bytes)?
        .sources
        .into_iter()
        .map(|source| (source.path, source.bytes))
        .collect())
}

fn child<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let child = object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| grammar(format!("Project Revision Store `{key}` must be an object")))?;
    exact_keys(child, keys)?;
    Ok(child)
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(grammar(
            "Project Revision Store entry has unknown, repeated, or missing keys",
        ));
    }
    Ok(())
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| grammar(format!("Project Revision Store `{key}` must be a string")))
}

fn digest_field(object: &Map<String, Value>, key: &str) -> Result<String, Vec<Diagnostic>> {
    let value = string(object, key)?;
    require_digest(value, key)?;
    Ok(value.to_owned())
}

fn number(object: &Map<String, Value>, key: &str) -> Result<usize, Vec<Diagnostic>> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| grammar(format!("Project Revision Store `{key}` must be an integer")))?;
    usize::try_from(value).map_err(|_| limit(key, usize::MAX))
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}

fn inventory_entries(sources: &[StoredSource]) -> Result<usize, Vec<Diagnostic>> {
    let mut directories = BTreeSet::new();
    for source in sources {
        let segments = source.path.split('/').collect::<Vec<_>>();
        let mut current = String::new();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(segment);
            directories.insert(current.clone());
        }
    }
    let used = 4usize
        .checked_add(directories.len())
        .and_then(|count| count.checked_add(sources.len()))
        .ok_or_else(|| limit("inventory_entries", MAX_STORE_INVENTORY_ENTRIES))?;
    if used > MAX_STORE_INVENTORY_ENTRIES {
        return Err(limit("inventory_entries", MAX_STORE_INVENTORY_ENTRIES));
    }
    Ok(used)
}

fn validate_source_path(path: &str) -> Result<(), Vec<Diagnostic>> {
    if path.is_empty()
        || path.len() > MAX_STORE_SOURCE_PATH_BYTES
        || path.split('/').count() > MAX_STORE_SOURCE_PATH_DEPTH
        || !crate::workspace::evidence_path_is_valid(path)
    {
        return Err(grammar(
            "Project Revision Store source path is not canonical or is too deep",
        ));
    }
    Ok(())
}

fn require_digest(value: &str, field: &str) -> Result<(), Vec<Diagnostic>> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(grammar(format!(
            "Project Revision Store `{field}` must be a canonical digest"
        )));
    };
    if hex.len() != 64
        || !hex
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(grammar(format!(
            "Project Revision Store `{field}` must be a canonical digest"
        )));
    }
    Ok(())
}

fn framed_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn project_revision(manifest: &[u8], workspace_revision: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(PROJECT_REVISION_DOMAIN);
    digest.update((manifest.len() as u64).to_le_bytes());
    digest.update(manifest);
    digest.update((workspace_revision.len() as u64).to_le_bytes());
    digest.update(workspace_revision.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn grammar(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G190", message)]
}

fn limit(field: impl AsRef<str>, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G191",
        format!(
            "Project Revision Store `{}` exceeds {maximum}",
            field.as_ref()
        ),
    )]
}

fn require_max(field: &str, observed: usize, maximum: usize) -> Result<(), Vec<Diagnostic>> {
    if observed > maximum {
        return Err(limit(field, maximum));
    }
    Ok(())
}

fn replay(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G192", message)]
}

pub(super) fn authentication(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G193", message)]
}

pub(super) fn io(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I215", message)]
}

pub(super) fn post_pivot(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I216", message)]
}

#[cfg(all(
    test,
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod tests;
