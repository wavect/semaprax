//! Pure compiler replay for the unpublished Windows physical host.
//! Neither the prepared subject nor raw stored bytes carry filesystem authority.
use super::*;

impl StoredSource {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn source(&self) -> &[u8] {
        &self.source
    }
}

impl PreparedEntry {
    pub fn entry_json(&self) -> &[u8] {
        &self.entry_json
    }
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn workspace_manifest(&self) -> &[u8] {
        &self.workspace_manifest
    }
    pub fn sources(&self) -> &[StoredSource] {
        &self.sources
    }
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

pub fn persist(
    root: &Path,
    revision: &ProjectRevision,
    expected: &str,
    operation: impl FnOnce(&Path, &PreparedEntry) -> Result<(), Vec<Diagnostic>>,
) -> Result<ProjectRevisionStoreReceipt, Vec<Diagnostic>> {
    let prepared = PreparedEntry::from_windows_revision(revision, expected)?;
    operation(root, &prepared)?;
    Ok(prepared.receipt())
}

pub fn load(
    root: &Path,
    entry_digest: &str,
    expected: &str,
    operation: impl FnOnce(&Path, &str) -> Result<StoredEntry, Vec<Diagnostic>>,
) -> Result<ProjectRevision, Vec<Diagnostic>> {
    require_digest(entry_digest, "entry_digest")?;
    require_digest(expected, "expected_project_revision")?;
    replay(operation(root, entry_digest)?, entry_digest, expected)
}

pub fn replay(
    stored: StoredEntry,
    digest: &str,
    expected: &str,
) -> Result<ProjectRevision, Vec<Diagnostic>> {
    replay_stored_for_profile(stored, digest, expected, EntryProfile::Windows)
}

pub struct Metadata {
    pub manifest_bytes: usize,
    pub workspace_manifest_bytes: usize,
    pub sources: Vec<(String, usize)>,
}

/// Bound and canonically replay metadata before a private host uses its sizes.
pub fn inspect(entry_json: &[u8], expected_hex: Option<&str>) -> Result<Metadata, Vec<Diagnostic>> {
    let expected = framed_digest(WINDOWS_ENTRY_DIGEST_DOMAIN, entry_json);
    if expected_hex.is_some_and(|hex| expected.strip_prefix("sha256:") != Some(hex)) {
        return Err(authentication(
            "retained Project Revision Store entry digest differs from its name",
        ));
    }
    let header = parse_entry_header_for_profile(entry_json, EntryProfile::Windows)?;
    let sources = header
        .sources
        .iter()
        .map(|source| StoredSource {
            path: source.path.clone(),
            source_graph_schema: source.source_graph_schema.clone(),
            source_revision: source.source_revision.clone(),
            source_digest: source.source_digest.clone(),
            bytes: source.bytes,
            source: Vec::new(),
        })
        .collect::<Vec<_>>();
    let prepared = PreparedEntry {
        profile: EntryProfile::Windows,
        project_schema: header.project_schema,
        project_revision: header.project_revision,
        workspace_revision: header.workspace_revision,
        project_graph_digest: header.project_graph_digest,
        manifest: vec![0; header.manifest_bytes],
        manifest_digest: header.manifest_digest,
        workspace_manifest: vec![0; header.workspace_manifest_bytes],
        workspace_manifest_digest: header.workspace_manifest_digest,
        inventory_entries: inventory_entries(&sources)?,
        sources,
        entry_json: Vec::new(),
        entry_digest: expected,
    };
    if render_entry_fixed_point(&prepared)? != entry_json {
        return Err(authentication(
            "retained Project Revision Store metadata is not canonical",
        ));
    }
    Ok(Metadata {
        manifest_bytes: header.manifest_bytes,
        workspace_manifest_bytes: header.workspace_manifest_bytes,
        sources: header
            .sources
            .into_iter()
            .map(|source| (source.path, source.bytes))
            .collect(),
    })
}
