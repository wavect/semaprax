//! Managed semantic-workspace initialization with cross-file resolution.
//!
//! [`initialize`] authenticates existing canonical sources, resolves the full
//! Workspace Semantic Graph once, and publishes one immutable managed
//! generation through the permanent workspace lock and sole `ACTIVE` pivot.
//! It never rewrites the original source paths and exposes no parser, raw
//! constructor, patch/apply, rollback, cleanup, backend, or runtime authority.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::bounded_output::CappedString;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{graph, review, workspace, workspace_graph};

pub(crate) const PATH_SET_SCHEMA: &str = "semaprax.workspace-semantic-path-set.v1";
pub(crate) const ROOT_SCHEMA: &str = "semaprax.workspace-semantic-root.v1";
pub(crate) const MANIFEST_SCHEMA: &str = "semaprax.workspace-semantic-manifest.v1";
pub(crate) const MAX_MANAGED_FILES: usize = 16;
pub(crate) const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_CONTROL_JSON_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_JSON_DEPTH: usize = 8;
pub(crate) const MAX_CHANGE_BUILDER_BYTES: usize = 32 * 1024 * 1024;

const WORKSPACE_REVISION_DOMAIN: &[u8] = b"semaprax.workspace-semantic-revision.v1\0";

pub(crate) use workspace::InitializePoint as SemanticInitializePoint;

/// Initializes one managed semantic workspace without modifying its original
/// source files.
pub fn initialize(root: &Path, path_set_path: &Path) -> Result<String, Vec<Diagnostic>> {
    initialize_from_preflight_with_hook(root, path_set_path, |_| {})
}

#[cfg(test)]
pub(crate) fn initialize_from_preflight(
    root: &Path,
    path_set_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    initialize(root, path_set_path)
}

pub(crate) fn initialize_from_preflight_with_hook(
    root: &Path,
    path_set_path: &Path,
    hook: impl FnMut(SemanticInitializePoint),
) -> Result<String, Vec<Diagnostic>> {
    workspace::initialize_semantic_with_hook(root, path_set_path, hook)
}

pub(crate) struct SemanticWorkspaceSource {
    pub(crate) path: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceFileFact {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
    source: String,
}

pub(crate) struct SemanticWorkspacePreflight {
    path_set: Vec<String>,
    files: Vec<SemanticWorkspaceFileFact>,
    manifest: String,
    workspace_revision: String,
    graph: workspace_graph::WorkspaceGraphBuild,
}

impl SemanticWorkspaceFileFact {
    pub(crate) fn path(&self) -> &str {
        &self.path
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

    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn into_parts(self) -> (String, String, String, String, String) {
        (
            self.path,
            self.source_graph_schema,
            self.source_revision,
            self.source_digest,
            self.source,
        )
    }
}

impl SemanticWorkspacePreflight {
    pub(crate) fn path_set(&self) -> &[String] {
        &self.path_set
    }

    pub(crate) fn files(&self) -> &[SemanticWorkspaceFileFact] {
        &self.files
    }

    #[cfg(test)]
    pub(crate) fn manifest(&self) -> &str {
        &self.manifest
    }

    pub(crate) fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub(crate) fn graph(&self) -> &workspace_graph::WorkspaceGraphBuild {
        &self.graph
    }

    pub(crate) fn into_generation_parts(self) -> (Vec<SemanticWorkspaceFileFact>, String, String) {
        (self.files, self.manifest, self.workspace_revision)
    }

    pub(crate) fn into_snapshot_parts(
        self,
    ) -> (
        Vec<SemanticWorkspaceFileFact>,
        String,
        String,
        workspace_graph::WorkspaceGraphBuild,
    ) {
        (
            self.files,
            self.manifest,
            self.workspace_revision,
            self.graph,
        )
    }
}

pub(crate) fn preflight_owned(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    preflight_owned_inner(path_set_source, sources, None)
}

pub(crate) fn preflight_owned_for_change(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: usize,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    preflight_owned_inner(path_set_source, sources, Some(change_builder_limit))
}

pub(crate) fn preflight_owned_for_operations(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    graph_builder_limit: usize,
    operations_builder_limit: usize,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    assert!(
        operations_builder_limit <= 67_108_864,
        "private Semantic Workspace Operations builder limit cannot exceed the production maximum"
    );
    preflight_owned_inner_mode(
        path_set_source,
        sources,
        Some(operations_builder_limit),
        true,
        Some(graph_builder_limit),
    )
}

fn preflight_owned_inner(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: Option<usize>,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    preflight_owned_inner_mode(path_set_source, sources, change_builder_limit, false, None)
}

fn preflight_owned_inner_mode(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: Option<usize>,
    retain_operations: bool,
    graph_builder_limit: Option<usize>,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    let path_set = parse_path_set(path_set_source)?;
    if sources.len() != path_set.len() {
        return Err(grammar(
            "semantic workspace owned sources disagree with the canonical path set",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut source_paths = Vec::with_capacity(sources.len());
    let mut total_source_bytes = 0usize;
    for source in &sources {
        if !workspace::evidence_path_is_valid(&source.path) {
            return Err(grammar("semantic workspace source path is not canonical"));
        }
        if !seen.insert(source.path.clone()) {
            return Err(grammar("semantic workspace source paths are not unique"));
        }
        total_source_bytes = total_source_bytes
            .checked_add(source.source.len())
            .ok_or_else(|| storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
        source_paths.push(source.path.clone());
    }
    source_paths.sort();
    if source_paths != path_set {
        return Err(grammar(
            "semantic workspace owned sources disagree with the canonical path set",
        ));
    }

    let graph_sources = sources
        .into_iter()
        .map(|source| workspace_graph::WorkspaceSource {
            path: source.path,
            source: source.source,
        })
        .collect();
    let (graph, recovered_sources) = if let Some(change_builder_limit) = change_builder_limit {
        if retain_operations {
            workspace_graph::build_owned_retaining_sources_for_operations(
                graph_sources,
                graph_builder_limit.expect("operations preflight supplies Graph limit"),
                change_builder_limit,
            )?
        } else {
            workspace_graph::build_owned_retaining_sources_for_change(
                graph_sources,
                change_builder_limit,
            )?
        }
    } else {
        workspace_graph::build_owned_retaining_sources(graph_sources)?
    };
    let mut schemas = graph.source_graph_schemas()?;
    let mut files = Vec::with_capacity(recovered_sources.len());
    for source in recovered_sources {
        let schema = schemas.remove(&source.path).ok_or_else(|| {
            invariant("semantic workspace source Graph schema is absent from the resolved build")
        })?;
        files.push(SemanticWorkspaceFileFact {
            path: source.path,
            source_graph_schema: schema.to_owned(),
            source_revision: graph::revision_from_canonical_source(&source.source),
            source_digest: review::source_digest(source.source.as_bytes()),
            bytes: source.source.len(),
            source: source.source,
        });
    }
    if !schemas.is_empty() {
        return Err(invariant(
            "semantic workspace resolved module facts contain an unknown source path",
        ));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = render_manifest(&files)?;
    let workspace_revision = semantic_workspace_revision(&manifest);
    let preflight = SemanticWorkspacePreflight {
        path_set,
        files,
        manifest,
        workspace_revision,
        graph,
    };
    validate_preflight_replay(&preflight)?;
    Ok(preflight)
}

pub(crate) fn replay_manifest_owned_for_change(
    manifest: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: usize,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    let expected = parse_manifest(manifest)?;
    let paths = expected
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let path_set = render_path_set(&paths)?;
    let preflight = preflight_owned_for_change(&path_set, sources, change_builder_limit)?;
    if preflight.manifest != manifest {
        return Err(invariant(
            "semantic workspace change manifest replay changed authenticated facts",
        ));
    }
    Ok(preflight)
}

pub(crate) fn replay_manifest_owned_for_operations(
    manifest: &str,
    sources: Vec<SemanticWorkspaceSource>,
    graph_builder_limit: usize,
    operations_builder_limit: usize,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    let expected = parse_manifest(manifest)?;
    let paths = expected
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let path_set = render_path_set(&paths)?;
    let preflight = preflight_owned_for_operations(
        &path_set,
        sources,
        graph_builder_limit,
        operations_builder_limit,
    )?;
    if preflight.manifest != manifest
        || preflight.files.len() != expected.len()
        || preflight
            .files
            .iter()
            .zip(&expected)
            .any(|(actual, expected)| !same_manifest_fact(actual, expected))
    {
        return Err(invariant(
            "Semantic Workspace Operations managed generation disagrees with its manifest",
        ));
    }
    Ok(preflight)
}

pub(crate) fn authenticated_operations_preflight(
    authenticated_revision: &str,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    graph: workspace_graph::WorkspaceGraphBuild,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    let mut files = sources
        .into_iter()
        .map(|source| SemanticWorkspaceFileFact {
            bytes: source.source.len(),
            path: source.path,
            source_graph_schema: source.source_graph_schema,
            source_revision: source.source_revision,
            source_digest: source.source_digest,
            source: source.source,
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let path_set = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let manifest = render_manifest(&files)?;
    let workspace_revision = semantic_workspace_revision(&manifest);
    if workspace_revision != authenticated_revision {
        return Err(invariant(
            "Semantic Workspace Operations authenticated manifest replay changed revision",
        ));
    }
    let preflight = SemanticWorkspacePreflight {
        path_set,
        files,
        manifest,
        workspace_revision,
        graph,
    };
    validate_preflight_replay(&preflight)?;
    Ok(preflight)
}

pub(crate) fn replay_manifest_owned(
    manifest: &str,
    sources: Vec<SemanticWorkspaceSource>,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    let expected = parse_manifest(manifest)?;
    let paths = expected
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let path_set = render_path_set(&paths)?;
    let preflight = preflight_owned(&path_set, sources)?;
    if preflight.manifest != manifest
        || preflight.files.len() != expected.len()
        || preflight
            .files
            .iter()
            .zip(&expected)
            .any(|(actual, expected)| !same_manifest_fact(actual, expected))
    {
        return Err(invariant(
            "Semantic Workspace managed generation disagrees with its manifest",
        ));
    }
    Ok(preflight)
}

pub(crate) fn parse_path_set(source: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
    parse_path_set_inner(source).map_err(|diagnostics| {
        normalize_parser_diagnostics(
            diagnostics,
            "semantic workspace path set is not canonical semaprax.workspace-semantic-path-set.v1",
        )
    })
}

fn parse_path_set_inner(source: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
    require_bounded_control_json(source, "path_set_bytes")?;
    let body = canonical_body(source, "Semantic Workspace path set")?;
    validate_json_depth(body)?;
    let value: Value = serde_json::from_str(body)
        .map_err(|_| grammar("invalid Semantic Workspace path-set JSON"))?;
    let object = exact_object(&value, &["schema", "files"])?;
    if text(object, "schema")? != PATH_SET_SCHEMA {
        return Err(grammar("wrong Semantic Workspace path-set schema"));
    }
    let values = array(object, "files")?;
    if values.len() < 2 {
        return Err(grammar("Semantic Workspace requires 2..16 source files"));
    }
    if values.len() > MAX_MANAGED_FILES {
        return Err(storage_limit("managed_files", MAX_MANAGED_FILES));
    }
    let mut paths = Vec::with_capacity(values.len());
    for value in values {
        let object = exact_object(value, &["path"])?;
        let path = text(object, "path")?.to_owned();
        if !workspace::evidence_path_is_valid(&path) {
            return Err(grammar(
                "Semantic Workspace path is outside the canonical managed path domain",
            ));
        }
        paths.push(path);
    }
    if paths.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(grammar(
            "Semantic Workspace paths must be strictly sorted and unique",
        ));
    }
    if render_path_set(&paths)? != source {
        return Err(grammar("Semantic Workspace path set is not canonical"));
    }
    Ok(paths)
}

pub(crate) fn render_path_set(paths: &[String]) -> Result<String, Vec<Diagnostic>> {
    validate_path_set_values(paths)?;
    render_bounded("path_set_bytes", |output| {
        write!(
            output,
            "{{\"schema\":{},\"files\":[",
            quote_json(PATH_SET_SCHEMA)
        )
        .unwrap();
        for (index, path) in paths.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            write!(output, "{{\"path\":{}}}", quote_json(path)).unwrap();
        }
        output.push_str("]}\n");
    })
}

pub(crate) fn parse_root(source: &str) -> Result<String, Vec<Diagnostic>> {
    parse_root_inner(source).map_err(|diagnostics| {
        normalize_parser_diagnostics(
            diagnostics,
            "managed workspace is not a semaprax.workspace-semantic-root.v1 workspace",
        )
    })
}

fn parse_root_inner(source: &str) -> Result<String, Vec<Diagnostic>> {
    require_bounded_control_json(source, "active_bytes")?;
    let body = canonical_body(source, "Semantic Workspace ACTIVE")?;
    validate_json_depth(body)?;
    let value: Value = serde_json::from_str(body)
        .map_err(|_| grammar("invalid Semantic Workspace ACTIVE JSON"))?;
    let object = exact_object(&value, &["schema", "workspace_revision"])?;
    if text(object, "schema")? != ROOT_SCHEMA {
        return Err(grammar("wrong Semantic Workspace ACTIVE schema"));
    }
    let revision = digest_text(object, "workspace_revision")?.to_owned();
    if render_root(&revision)? != source {
        return Err(grammar("Semantic Workspace ACTIVE is not canonical"));
    }
    Ok(revision)
}

pub(crate) fn render_root(revision: &str) -> Result<String, Vec<Diagnostic>> {
    require_digest(revision)?;
    render_bounded("active_bytes", |output| {
        writeln!(
            output,
            "{{\"schema\":{},\"workspace_revision\":{}}}",
            quote_json(ROOT_SCHEMA),
            quote_json(revision)
        )
        .unwrap();
    })
}

pub(crate) fn parse_manifest(
    source: &str,
) -> Result<Vec<SemanticWorkspaceFileFact>, Vec<Diagnostic>> {
    parse_manifest_inner(source).map_err(|diagnostics| {
        normalize_parser_diagnostics(
            diagnostics,
            "semantic workspace manifest is not canonical semaprax.workspace-semantic-manifest.v1",
        )
    })
}

fn parse_manifest_inner(source: &str) -> Result<Vec<SemanticWorkspaceFileFact>, Vec<Diagnostic>> {
    require_bounded_control_json(source, "manifest_bytes")?;
    let body = canonical_body(source, "Semantic Workspace manifest")?;
    validate_json_depth(body)?;
    let value: Value = serde_json::from_str(body)
        .map_err(|_| grammar("invalid Semantic Workspace manifest JSON"))?;
    let object = exact_object(&value, &["schema", "files"])?;
    if text(object, "schema")? != MANIFEST_SCHEMA {
        return Err(grammar("wrong Semantic Workspace manifest schema"));
    }
    let values = array(object, "files")?;
    if values.len() < 2 {
        return Err(grammar("Semantic Workspace requires 2..16 source files"));
    }
    if values.len() > MAX_MANAGED_FILES {
        return Err(storage_limit("managed_files", MAX_MANAGED_FILES));
    }
    let mut files = Vec::with_capacity(values.len());
    let mut total_source_bytes = 0usize;
    for value in values {
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
        if !workspace::evidence_path_is_valid(&path) {
            return Err(grammar("Semantic Workspace manifest path is not canonical"));
        }
        let source_graph_schema = text(object, "source_graph_schema")?.to_owned();
        if !is_source_graph_schema(&source_graph_schema) {
            return Err(grammar(
                "Semantic Workspace manifest source Graph schema is unsupported",
            ));
        }
        let bytes = integer(object, "bytes")?;
        total_source_bytes = total_source_bytes
            .checked_add(bytes)
            .ok_or_else(|| storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
        files.push(SemanticWorkspaceFileFact {
            path,
            source_graph_schema,
            source_revision: digest_text(object, "source_revision")?.to_owned(),
            source_digest: digest_text(object, "source_digest")?.to_owned(),
            bytes,
            source: String::new(),
        });
    }
    if files.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(grammar(
            "Semantic Workspace manifest paths must be strictly sorted and unique",
        ));
    }
    if render_manifest(&files)? != source {
        return Err(grammar("Semantic Workspace manifest is not canonical"));
    }
    Ok(files)
}

pub(crate) fn render_manifest(
    files: &[SemanticWorkspaceFileFact],
) -> Result<String, Vec<Diagnostic>> {
    validate_manifest_values(files)?;
    render_bounded("manifest_bytes", |output| {
        write!(
            output,
            "{{\"schema\":{},\"files\":[",
            quote_json(MANIFEST_SCHEMA)
        )
        .unwrap();
        for (index, file) in files.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            write!(
                output,
                "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
                quote_json(&file.path),
                quote_json(&file.source_graph_schema),
                quote_json(&file.source_revision),
                quote_json(&file.source_digest),
                file.bytes
            )
            .unwrap();
        }
        output.push_str("]}\n");
    })
}

pub(crate) fn render_manifest_facts(
    files: &[(&str, &str, &str, &str, usize)],
) -> Result<String, Vec<Diagnostic>> {
    if files.len() < 2 {
        return Err(grammar("Semantic Workspace requires 2..16 source files"));
    }
    if files.len() > MAX_MANAGED_FILES {
        return Err(storage_limit("managed_files", MAX_MANAGED_FILES));
    }
    let mut total = 0usize;
    for (index, (path, schema, revision, digest, bytes)) in files.iter().enumerate() {
        if !workspace::evidence_path_is_valid(path)
            || !is_source_graph_schema(schema)
            || index != 0 && files[index - 1].0 >= *path
        {
            return Err(grammar(
                "Semantic Workspace manifest values are not canonical",
            ));
        }
        require_digest(revision)?;
        require_digest(digest)?;
        total = total
            .checked_add(*bytes)
            .ok_or_else(|| storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total > MAX_TOTAL_SOURCE_BYTES {
            return Err(storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
    }
    render_bounded("manifest_bytes", |output| {
        write!(
            output,
            "{{\"schema\":{},\"files\":[",
            quote_json(MANIFEST_SCHEMA)
        )
        .unwrap();
        for (index, (path, schema, revision, digest, bytes)) in files.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            write!(
                output,
                "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
                quote_json(path), quote_json(schema), quote_json(revision), quote_json(digest), bytes
            ).unwrap();
        }
        output.push_str("]}\n");
    })
}

fn validate_path_set_values(paths: &[String]) -> Result<(), Vec<Diagnostic>> {
    if paths.len() < 2 {
        return Err(grammar("Semantic Workspace requires 2..16 source files"));
    }
    if paths.len() > MAX_MANAGED_FILES {
        return Err(storage_limit("managed_files", MAX_MANAGED_FILES));
    }
    if paths
        .iter()
        .any(|path| !workspace::evidence_path_is_valid(path))
        || paths.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(grammar(
            "Semantic Workspace path-set values are not canonical",
        ));
    }
    Ok(())
}

fn validate_manifest_values(files: &[SemanticWorkspaceFileFact]) -> Result<(), Vec<Diagnostic>> {
    if files.len() < 2 {
        return Err(grammar("Semantic Workspace requires 2..16 source files"));
    }
    if files.len() > MAX_MANAGED_FILES {
        return Err(storage_limit("managed_files", MAX_MANAGED_FILES));
    }
    let mut total_source_bytes = 0usize;
    for file in files {
        if !workspace::evidence_path_is_valid(&file.path)
            || !is_source_graph_schema(&file.source_graph_schema)
            || require_digest(&file.source_revision).is_err()
            || require_digest(&file.source_digest).is_err()
        {
            return Err(grammar(
                "Semantic Workspace manifest values are not canonical",
            ));
        }
        total_source_bytes = total_source_bytes
            .checked_add(file.bytes)
            .ok_or_else(|| storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES));
        }
    }
    if files.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(grammar(
            "Semantic Workspace manifest values are not canonical",
        ));
    }
    Ok(())
}

pub(crate) fn semantic_workspace_revision(manifest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(WORKSPACE_REVISION_DOMAIN);
    hasher.update((manifest.len() as u64).to_le_bytes());
    hasher.update(manifest.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn validate_preflight_replay(
    preflight: &SemanticWorkspacePreflight,
) -> Result<(), Vec<Diagnostic>> {
    let schemas = preflight.graph.source_graph_schemas()?;
    if schemas.len() != preflight.files.len() || preflight.path_set.len() != preflight.files.len() {
        return Err(invariant(
            "Semantic Workspace preflight fact cardinality disagrees",
        ));
    }
    for (path, file) in preflight.path_set.iter().zip(&preflight.files) {
        if path != &file.path
            || schemas.get(path).copied() != Some(file.source_graph_schema.as_str())
            || file.bytes != file.source.len()
            || graph::revision_from_canonical_source(&file.source) != file.source_revision
            || review::source_digest(file.source.as_bytes()) != file.source_digest
        {
            return Err(invariant(
                "Semantic Workspace preflight file facts disagree with independent replay",
            ));
        }
    }
    let replayed_manifest = parse_manifest(&preflight.manifest)?;
    if replayed_manifest.len() != preflight.files.len()
        || replayed_manifest
            .iter()
            .zip(&preflight.files)
            .any(|(replayed, expected)| !same_manifest_fact(replayed, expected))
    {
        return Err(invariant(
            "Semantic Workspace manifest facts disagree with independent grammar replay",
        ));
    }
    if semantic_workspace_revision(&preflight.manifest) != preflight.workspace_revision {
        return Err(invariant(
            "Semantic Workspace manifest or revision disagrees with independent replay",
        ));
    }
    Ok(())
}

fn same_manifest_fact(left: &SemanticWorkspaceFileFact, right: &SemanticWorkspaceFileFact) -> bool {
    left.path == right.path
        && left.source_graph_schema == right.source_graph_schema
        && left.source_revision == right.source_revision
        && left.source_digest == right.source_digest
        && left.bytes == right.bytes
}

fn render_bounded(
    field: &'static str,
    render: impl FnOnce(&mut CappedString),
) -> Result<String, Vec<Diagnostic>> {
    let (output, overflowed) = crate::bounded_output::with_limit(MAX_CONTROL_JSON_BYTES, || {
        let mut output = CappedString::new();
        render(&mut output);
        output.into_string()
    });
    if overflowed || output.len() > MAX_CONTROL_JSON_BYTES {
        return Err(storage_limit(field, MAX_CONTROL_JSON_BYTES));
    }
    Ok(output)
}

fn require_bounded_control_json(source: &str, field: &'static str) -> Result<(), Vec<Diagnostic>> {
    if source.len() > MAX_CONTROL_JSON_BYTES {
        Err(storage_limit(field, MAX_CONTROL_JSON_BYTES))
    } else {
        Ok(())
    }
}

fn canonical_body<'a>(source: &'a str, label: &str) -> Result<&'a str, Vec<Diagnostic>> {
    if source.starts_with('\u{feff}')
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len().saturating_sub(1)].contains('\n')
    {
        return Err(grammar(format!(
            "{label} must be one canonical JSON line with one terminal LF"
        )));
    }
    Ok(&source[..source.len() - 1])
}

fn validate_json_depth(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escape = false;
    for byte in source.bytes() {
        if string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| storage_limit("json_depth", MAX_JSON_DEPTH))?;
                if depth > MAX_JSON_DEPTH {
                    return Err(storage_limit("json_depth", MAX_JSON_DEPTH));
                }
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| grammar("Semantic Workspace JSON is unbalanced"))?;
            }
            _ => {}
        }
    }
    if string || depth != 0 {
        return Err(grammar("Semantic Workspace JSON is unbalanced"));
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| grammar("Semantic Workspace JSON value must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(grammar("Semantic Workspace JSON has missing or extra keys"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| grammar(format!("Semantic Workspace field `{key}` must be a string")))
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| grammar(format!("Semantic Workspace field `{key}` must be an array")))
}

fn integer(object: &Map<String, Value>, key: &str) -> Result<usize, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            grammar(format!(
                "Semantic Workspace field `{key}` must be a nonnegative integer"
            ))
        })
}

fn digest_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    let value = text(object, key)?;
    require_digest(value)?;
    Ok(value)
}

fn require_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if valid {
        Ok(())
    } else {
        Err(grammar("Semantic Workspace digest is not canonical"))
    }
}

fn is_source_graph_schema(value: &str) -> bool {
    matches!(
        value,
        "semaprax.graph.v10"
            | "semaprax.graph.v11"
            | "semaprax.graph.v12"
            | "semaprax.graph.v13"
            | "semaprax.graph.v14"
    )
}

fn normalize_parser_diagnostics(
    diagnostics: Vec<Diagnostic>,
    canonical_message: &'static str,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            if diagnostic.code == "SPX-G174"
                && diagnostic.message != "Semantic Workspace requires 2..16 source files"
            {
                Diagnostic::io("SPX-G174", canonical_message)
            } else {
                diagnostic
            }
        })
        .collect()
}

fn grammar(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G174", message)]
}

fn storage_limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G175",
        format!("Semantic Workspace `{field}` exceeds {maximum}"),
    )]
}

fn invariant(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G174", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn canonical_source(path: &str, source: &str) -> SemanticWorkspaceSource {
        let program = crate::parse(source, Path::new(path)).expect("semantic fixture must parse");
        SemanticWorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&program),
        }
    }

    fn importing_sources() -> Vec<SemanticWorkspaceSource> {
        vec![
            canonical_source(
                "z/app.spx",
                r#"
module semantic.app;
use type @id("semantic.point") from semantic.provider as Point;
use function @id("semantic.work") from semantic.provider as work;
permit { audit.write }

@id("semantic.main")
fn main() -> i64 uses { audit.write } {
    work(Point { value: 1 })
}
"#,
            ),
            canonical_source(
                "a/provider.spx",
                r#"
module semantic.provider;
permit { audit.write }

@id("semantic.point")
record Point { @id("semantic.point.value") value: i64, }

@id("semantic.work")
fn work(value: Point) -> i64 uses { audit.write } { value.value }

@id("semantic.provider.main")
fn main() -> i64 { 0 }
"#,
            ),
        ]
    }

    fn path_set(paths: &[&str]) -> String {
        render_path_set(
            &paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    fn raw_path_set(paths: &[String]) -> String {
        format!(
            "{{\"schema\":{},\"files\":[{}]}}\n",
            quote_json(PATH_SET_SCHEMA),
            paths
                .iter()
                .map(|path| format!("{{\"path\":{}}}", quote_json(path)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn raw_manifest(files: &[SemanticWorkspaceFileFact]) -> String {
        format!(
            "{{\"schema\":{},\"files\":[{}]}}\n",
            quote_json(MANIFEST_SCHEMA),
            files
                .iter()
                .map(|file| format!(
                    "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
                    quote_json(&file.path),
                    quote_json(&file.source_graph_schema),
                    quote_json(&file.source_revision),
                    quote_json(&file.source_digest),
                    file.bytes
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) -> Vec<Diagnostic> {
        let error = result.err().expect("hostile input must fail closed");
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, code);
        error
    }

    #[test]
    fn exact_path_set_active_manifest_and_revision_kat_replay() {
        let paths = path_set(&["a/provider.spx", "z/app.spx"]);
        assert_eq!(
            paths,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n"
        );
        assert_eq!(
            parse_path_set(&paths).unwrap(),
            ["a/provider.spx", "z/app.spx"]
        );

        let preflight = preflight_owned(&paths, importing_sources()).unwrap();
        assert_eq!(preflight.path_set(), parse_path_set(&paths).unwrap());
        assert_eq!(
            preflight
                .files()
                .iter()
                .map(SemanticWorkspaceFileFact::path)
                .collect::<Vec<_>>(),
            ["a/provider.spx", "z/app.spx"]
        );
        assert!(preflight.files().iter().all(|file| {
            file.source_graph_schema() == "semaprax.graph.v10"
                && file.bytes() == file.source().len()
                && file.source_revision().starts_with("sha256:")
                && file.source_digest().starts_with("sha256:")
        }));
        assert_eq!(
            preflight.manifest(),
            "{\"schema\":\"semaprax.workspace-semantic-manifest.v1\",\"files\":[{\"path\":\"a/provider.spx\",\"source_graph_schema\":\"semaprax.graph.v10\",\"source_revision\":\"sha256:e9e29bfe3a186fd9c9e1a7d8f3c10dc7ebcc006ed92b407344adacbb0248b7c0\",\"source_digest\":\"sha256:92041de1eebfe58bac89d26f743f7b09c21e57b9203094fc8a5667d40c1592a7\",\"bytes\":292},{\"path\":\"z/app.spx\",\"source_graph_schema\":\"semaprax.graph.v10\",\"source_revision\":\"sha256:df8274579bffda63bfed85c486f8dc30b54c698a8d051bf4ee165b947e3e370a\",\"source_digest\":\"sha256:943ec92f277f75089f8a4b7db0a3a4bf66fa90787d94ea494230195d2604f10b\",\"bytes\":272}]}\n"
        );
        assert_eq!(
            preflight.workspace_revision(),
            "sha256:88181393a052db1605145236cd3fd2e7f3f24256ce0c90d7968d939fc6a4c4ef"
        );
        assert_eq!(
            semantic_workspace_revision(preflight.manifest()),
            preflight.workspace_revision()
        );
        let parsed_manifest = parse_manifest(preflight.manifest()).unwrap();
        assert_eq!(
            render_manifest(&parsed_manifest).unwrap(),
            preflight.manifest()
        );
        for (actual, replayed) in preflight.files().iter().zip(parsed_manifest) {
            assert_eq!(actual.path(), replayed.path());
            assert_eq!(actual.source_graph_schema(), replayed.source_graph_schema());
            assert_eq!(actual.source_revision(), replayed.source_revision());
            assert_eq!(actual.source_digest(), replayed.source_digest());
            assert_eq!(actual.bytes(), replayed.bytes());
        }
        let active = render_root(preflight.workspace_revision()).unwrap();
        assert_eq!(
            active,
            format!(
                "{{\"schema\":\"semaprax.workspace-semantic-root.v1\",\"workspace_revision\":\"{}\"}}\n",
                preflight.workspace_revision()
            )
        );
        assert_eq!(parse_root(&active).unwrap(), preflight.workspace_revision());
        let schemas = preflight.graph().source_graph_schemas().unwrap();
        assert_eq!(schemas["a/provider.spx"], "semaprax.graph.v10");
        assert_eq!(schemas["z/app.spx"], "semaprax.graph.v10");
    }

    #[test]
    fn control_parsers_reject_noncanonical_and_hostile_forms() {
        let valid = path_set(&["a.spx", "b.spx"]);
        for hostile in [
            "{".to_owned(),
            "{\"files\":[{\"path\":\"a.spx\"},{\"path\":\"b.spx\"}],\"schema\":\"semaprax.workspace-semantic-path-set.v1\"}\n".to_owned(),
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\"}\n".to_owned(),
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[],\"extra\":0}\n".to_owned(),
            format!("\u{feff}{valid}"),
            valid.replace('\n', "\r\n"),
            valid.trim_end().to_owned(),
            format!("{valid}\n"),
        ] {
            assert_code(parse_path_set(&hostile), "SPX-G174");
        }

        let deep = format!(
            "{{\"schema\":\"{}\",\"files\":[[[[[[[[[]]]]]]]]]}}\n",
            PATH_SET_SCHEMA
        );
        assert_code(parse_path_set(&deep), "SPX-G175");
        let one = vec!["a.spx".to_owned()];
        assert_code(render_path_set(&one), "SPX-G174");
        let error = assert_code(parse_path_set(&raw_path_set(&one)), "SPX-G174");
        assert_eq!(
            error[0].message,
            "Semantic Workspace requires 2..16 source files"
        );
        let over = (0..17)
            .map(|index| format!("f{index:02}.spx"))
            .collect::<Vec<_>>();
        assert_code(render_path_set(&over), "SPX-G175");
        let error = assert_code(parse_path_set(&raw_path_set(&over)), "SPX-G175");
        assert_eq!(
            error[0].message,
            "Semantic Workspace `managed_files` exceeds 16"
        );
        for paths in [
            vec!["b.spx", "a.spx"],
            vec!["a.spx", "a.spx"],
            vec!["A.spx", "b.spx"],
            vec!["a.spx", "con.spx"],
            vec!["/a.spx", "b.spx"],
            vec!["a/../b.spx", "c.spx"],
            vec!["a.spx", "a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/b.spx"],
        ] {
            let paths = paths.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_code(render_path_set(&paths), "SPX-G174");
            assert_code(parse_path_set(&raw_path_set(&paths)), "SPX-G174");
        }

        let revision = format!("sha256:{}", "1".repeat(64));
        let active = render_root(&revision).unwrap();
        for hostile in [
            "{".to_owned(),
            format!(
                "{{\"workspace_revision\":\"{revision}\",\"schema\":\"{ROOT_SCHEMA}\"}}\n"
            ),
            format!("{{\"schema\":\"{ROOT_SCHEMA}\"}}\n"),
            format!(
                "{{\"schema\":\"{ROOT_SCHEMA}\",\"workspace_revision\":\"{revision}\",\"extra\":0}}\n"
            ),
            format!("\u{feff}{active}"),
            active.replace('\n', "\r\n"),
            active.trim_end().to_owned(),
            format!("{active}\n"),
            format!(
                "{{\"schema\":\"{ROOT_SCHEMA}\",\"workspace_revision\":\"sha256:ABC\"}}\n"
            ),
        ] {
            assert_code(parse_root(&hostile), "SPX-G174");
        }
    }

    #[test]
    fn semantic_storage_boundaries_are_exact_and_one_over() {
        let exact = "x".repeat(MAX_CONTROL_JSON_BYTES);
        for field in ["path_set_bytes", "active_bytes", "manifest_bytes"] {
            require_bounded_control_json(&exact, field).unwrap();
            let error = assert_code(
                require_bounded_control_json(&format!("{exact}x"), field),
                "SPX-G175",
            );
            assert_eq!(
                error[0].message,
                format!("Semantic Workspace `{field}` exceeds {MAX_CONTROL_JSON_BYTES}")
            );
        }

        let paths = path_set(&["a.spx", "b.spx"]);
        let first = SemanticWorkspaceSource {
            path: "a.spx".to_owned(),
            source: "x".repeat(MAX_TOTAL_SOURCE_BYTES - 1),
        };
        let second = SemanticWorkspaceSource {
            path: "b.spx".to_owned(),
            source: "x".to_owned(),
        };
        let exact_error = preflight_owned(&paths, vec![first, second])
            .err()
            .expect("invalid exact-boundary source must fail after storage admission");
        assert_ne!(exact_error[0].code, "SPX-G175");

        let first = SemanticWorkspaceSource {
            path: "a.spx".to_owned(),
            source: "x".repeat(MAX_TOTAL_SOURCE_BYTES),
        };
        let second = SemanticWorkspaceSource {
            path: "b.spx".to_owned(),
            source: "x".to_owned(),
        };
        let error = assert_code(preflight_owned(&paths, vec![first, second]), "SPX-G175");
        assert_eq!(
            error[0].message,
            "Semantic Workspace `total_source_bytes` exceeds 16777216"
        );
    }

    fn manifest_fact(path: &str, bytes: usize) -> SemanticWorkspaceFileFact {
        SemanticWorkspaceFileFact {
            path: path.to_owned(),
            source_graph_schema: "semaprax.graph.v10".to_owned(),
            source_revision: format!("sha256:{}", "1".repeat(64)),
            source_digest: format!("sha256:{}", "2".repeat(64)),
            bytes,
            source: String::new(),
        }
    }

    #[test]
    fn typed_cardinality_and_manifest_byte_replay_fail_before_unbounded_work() {
        let paths = path_set(&["a.spx", "b.spx"]);
        for sources in [
            vec![SemanticWorkspaceSource {
                path: "not-canonical".to_owned(),
                source: String::new(),
            }],
            (0..4096)
                .map(|index| SemanticWorkspaceSource {
                    path: format!("NOT-CANONICAL-{index}"),
                    source: String::new(),
                })
                .collect::<Vec<_>>(),
        ] {
            let error = assert_code(preflight_owned(&paths, sources), "SPX-G174");
            assert_eq!(
                error[0].message,
                "semantic workspace owned sources disagree with the canonical path set"
            );
        }

        let exact = vec![
            manifest_fact("a.spx", MAX_TOTAL_SOURCE_BYTES - 1),
            manifest_fact("b.spx", 1),
        ];
        let exact_manifest = render_manifest(&exact).unwrap();
        let replayed = parse_manifest(&exact_manifest).unwrap();
        assert_eq!(
            replayed.iter().map(|fact| fact.bytes()).sum::<usize>(),
            MAX_TOTAL_SOURCE_BYTES
        );

        let over = vec![
            manifest_fact("a.spx", MAX_TOTAL_SOURCE_BYTES),
            manifest_fact("b.spx", 1),
        ];
        assert_code(render_manifest(&over), "SPX-G175");
        let error = assert_code(parse_manifest(&raw_manifest(&over)), "SPX-G175");
        assert_eq!(
            error[0].message,
            "Semantic Workspace `total_source_bytes` exceeds 16777216"
        );

        let one = [manifest_fact("a.spx", 1)];
        assert_code(render_manifest(&one), "SPX-G174");
        let error = assert_code(parse_manifest(&raw_manifest(&one)), "SPX-G174");
        assert_eq!(
            error[0].message,
            "Semantic Workspace requires 2..16 source files"
        );
        let seventeen = (0..17)
            .map(|index| manifest_fact(&format!("f{index:02}.spx"), 1))
            .collect::<Vec<_>>();
        assert_code(render_manifest(&seventeen), "SPX-G175");
        let error = assert_code(parse_manifest(&raw_manifest(&seventeen)), "SPX-G175");
        assert_eq!(
            error[0].message,
            "Semantic Workspace `managed_files` exceeds 16"
        );
    }

    #[test]
    fn preflight_replay_rejects_malformed_reordered_and_substituted_manifest() {
        let paths = path_set(&["a/provider.spx", "z/app.spx"]);
        let mut malformed = preflight_owned(&paths, importing_sources()).unwrap();
        malformed.manifest = "{\n".to_owned();
        assert_code(validate_preflight_replay(&malformed), "SPX-G174");

        let mut reordered = preflight_owned(&paths, importing_sources()).unwrap();
        let value: Value = serde_json::from_str(reordered.manifest.trim_end()).unwrap();
        reordered.manifest = format!(
            "{{\"files\":{},\"schema\":{}}}\n",
            serde_json::to_string(&value["files"]).unwrap(),
            quote_json(MANIFEST_SCHEMA)
        );
        let error = assert_code(validate_preflight_replay(&reordered), "SPX-G174");
        assert_eq!(
            error[0].message,
            "semantic workspace manifest is not canonical semaprax.workspace-semantic-manifest.v1"
        );

        let mut substituted = preflight_owned(&paths, importing_sources()).unwrap();
        let mut facts = parse_manifest(substituted.manifest()).unwrap();
        facts[0].source_digest = format!("sha256:{}", "f".repeat(64));
        substituted.manifest = render_manifest(&facts).unwrap();
        let error = assert_code(validate_preflight_replay(&substituted), "SPX-G174");
        assert_eq!(
            error[0].message,
            "Semantic Workspace manifest facts disagree with independent grammar replay"
        );
    }

    #[test]
    fn per_file_graph_v10_through_v14_facts_replay_exactly() {
        let cases = [
            (
                "v10.spx",
                "module schema.v10; @id(\"v10.main\") fn main()->i64{0}",
                "semaprax.graph.v10",
            ),
            (
                "v11.spx",
                "module schema.v11; @id(\"v11.target\") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }} @id(\"v11.main\") fn main()->i64{0}",
                "semaprax.graph.v11",
            ),
            (
                "v12.spx",
                "module schema.v12; @id(\"v12.box\") record Box<T>{@id(\"v12.box.value\") value:T,} @id(\"v12.main\") fn main()->i64{0}",
                "semaprax.graph.v12",
            ),
            (
                "v13.spx",
                "module schema.v13; @id(\"v13.box\") record Box{@id(\"v13.box.value\") value:i64,} @id(\"v13.read\") fn read(input:Box)->i64{match input { Box { value } => value, }} @id(\"v13.main\") fn main()->i64{0}",
                "semaprax.graph.v13",
            ),
            (
                "v14.spx",
                "module schema.v14; @id(\"v14.target\") fn target<T>()->bool{true} @id(\"v14.main\") fn main()->i64{if target<i64>(){1}else{0}}",
                "semaprax.graph.v14",
            ),
        ];
        let paths = cases.iter().map(|(path, _, _)| *path).collect::<Vec<_>>();
        let sources = cases
            .iter()
            .map(|(path, source, _)| canonical_source(path, source))
            .collect::<Vec<_>>();
        let preflight = preflight_owned(&path_set(&paths), sources).unwrap();
        assert_eq!(
            preflight
                .files()
                .iter()
                .map(|file| (file.path(), file.source_graph_schema()))
                .collect::<Vec<_>>(),
            cases
                .iter()
                .map(|(path, _, schema)| (*path, *schema))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            render_manifest(&parse_manifest(preflight.manifest()).unwrap()).unwrap(),
            preflight.manifest()
        );
    }

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-semantic-workspace-ordinary-preservation-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct SemanticFixture {
        root: TempRoot,
        path_set: PathBuf,
        sources: Vec<SemanticWorkspaceSource>,
        path_set_bytes: String,
    }

    impl SemanticFixture {
        fn new() -> Self {
            let root = TempRoot::new();
            let sources = importing_sources();
            for source in &sources {
                let destination = root.0.join(&source.path);
                std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
                std::fs::write(destination, &source.source).unwrap();
            }
            let path_set_bytes = path_set(&["a/provider.spx", "z/app.spx"]);
            let path_set = root.0.join("semantic-paths.json");
            std::fs::write(&path_set, &path_set_bytes).unwrap();
            Self {
                root,
                path_set,
                sources,
                path_set_bytes,
            }
        }

        fn control(&self) -> PathBuf {
            self.root.0.join(".semaprax-workspace")
        }

        fn active(&self) -> PathBuf {
            self.control().join("ACTIVE")
        }

        fn generation(&self, revision: &str) -> PathBuf {
            self.control()
                .join("generations")
                .join(revision.strip_prefix("sha256:").unwrap())
        }

        fn expected_preflight(&self) -> SemanticWorkspacePreflight {
            preflight_owned(
                &self.path_set_bytes,
                self.sources
                    .iter()
                    .map(|source| SemanticWorkspaceSource {
                        path: source.path.clone(),
                        source: source.source.clone(),
                    })
                    .collect(),
            )
            .unwrap()
        }

        fn assert_inputs_unchanged(&self) {
            assert_eq!(
                std::fs::read_to_string(&self.path_set).unwrap(),
                self.path_set_bytes
            );
            for source in &self.sources {
                assert_eq!(
                    std::fs::read_to_string(self.root.0.join(&source.path)).unwrap(),
                    source.source
                );
            }
        }
    }

    fn assert_lock_reacquirable(fixture: &SemanticFixture) {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(fixture.control().join("LOCK"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).expect("initializer must release the lock");
        fs2::FileExt::unlock(&lock).unwrap();
    }

    fn replace_with_same_bytes(path: &Path) {
        let bytes = std::fs::read(path).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn semantic_initialize_publishes_exact_generation_and_all_six_edge_families() {
        let first = SemanticFixture::new();
        let expected = first.expected_preflight();
        let revision = initialize_from_preflight(&first.root.0, &first.path_set).unwrap();
        assert_eq!(revision, expected.workspace_revision());
        assert_eq!(
            std::fs::read_to_string(first.active()).unwrap(),
            render_root(&revision).unwrap()
        );
        let generation = first.generation(&revision);
        assert_eq!(
            std::fs::read_to_string(generation.join("manifest.json")).unwrap(),
            expected.manifest()
        );
        for file in expected.files() {
            assert_eq!(
                std::fs::read_to_string(generation.join("files").join(file.path())).unwrap(),
                file.source()
            );
        }
        first.assert_inputs_unchanged();

        let graph = crate::workspace_graph::snapshot(&first.root.0, "semantic.app").unwrap();
        assert_eq!(graph.workspace_revision(), revision);
        let kinds = graph
            .edges()
            .iter()
            .map(|edge| edge.kind())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            kinds,
            BTreeSet::from([
                "call",
                "capability_authority",
                "effect_requirement",
                "function_import",
                "type_import",
                "type_reference",
            ])
        );
        assert_lock_reacquirable(&first);

        let second = SemanticFixture::new();
        let replayed = initialize_from_preflight(&second.root.0, &second.path_set).unwrap();
        let replayed_graph =
            crate::workspace_graph::snapshot(&second.root.0, "semantic.app").unwrap();
        assert_eq!(replayed, revision);
        assert_eq!(replayed_graph.to_json(), graph.to_json());
        assert_lock_reacquirable(&second);
    }

    #[test]
    fn semantic_preflight_failures_and_post_preflight_input_replacement_publish_no_control() {
        let malformed = SemanticFixture::new();
        std::fs::write(
            malformed.root.0.join("z/app.spx"),
            "module semantic.app; this is not source\n",
        )
        .unwrap();
        assert!(initialize_from_preflight(&malformed.root.0, &malformed.path_set).is_err());
        assert!(!malformed.control().exists());

        for target in ["source", "path-set"] {
            let fixture = SemanticFixture::new();
            let path = if target == "source" {
                fixture.root.0.join("z/app.spx")
            } else {
                fixture.path_set.clone()
            };
            let error =
                initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                    if matches!(point, SemanticInitializePoint::SemanticPreflightComplete) {
                        replace_with_same_bytes(&path);
                    }
                })
                .unwrap_err();
            assert_eq!(error.len(), 1);
            assert_eq!(error[0].code, "SPX-G153", "{target}");
            assert!(!fixture.control().exists(), "{target}");
        }

        let fixture = SemanticFixture::new();
        let source = fixture.root.0.join("z/app.spx");
        let donor = fixture.root.0.join("source-donor.spx");
        std::fs::write(&donor, std::fs::read(&source).unwrap()).unwrap();
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::SemanticPreflightComplete) {
                    std::fs::remove_file(&source).unwrap();
                    std::fs::hard_link(&donor, &source).unwrap();
                }
            })
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G153");
        assert!(!fixture.control().exists());
        assert_eq!(
            std::fs::read(&source).unwrap(),
            std::fs::read(&donor).unwrap()
        );
    }

    #[test]
    fn semantic_initializer_preserves_foreign_control_generation_active_and_staging() {
        for kind in ["file", "directory"] {
            let fixture = SemanticFixture::new();
            let control = fixture.control();
            if kind == "file" {
                std::fs::write(&control, "foreign-control\n").unwrap();
            } else {
                std::fs::create_dir(&control).unwrap();
                std::fs::write(control.join("foreign"), "preserve\n").unwrap();
            }
            let error = initialize_from_preflight(&fixture.root.0, &fixture.path_set).unwrap_err();
            assert_eq!(error[0].code, "SPX-I209");
            if kind == "file" {
                assert_eq!(
                    std::fs::read_to_string(&control).unwrap(),
                    "foreign-control\n"
                );
            } else {
                assert_eq!(
                    std::fs::read_to_string(control.join("foreign")).unwrap(),
                    "preserve\n"
                );
            }
        }

        let fixture = SemanticFixture::new();
        let revision = fixture.expected_preflight().workspace_revision().to_owned();
        let generation = fixture.generation(&revision);
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::GenerationDestinationChecked) {
                    std::fs::write(&generation, "foreign-generation\n").unwrap();
                }
            })
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-I211");
        assert_eq!(
            std::fs::read_to_string(&generation).unwrap(),
            "foreign-generation\n"
        );
        assert!(!fixture.active().exists());
        assert_lock_reacquirable(&fixture);

        let fixture = SemanticFixture::new();
        let active = fixture.active();
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::ActiveDestinationChecked) {
                    std::fs::write(&active, "foreign-active\n").unwrap();
                }
            })
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_eq!(
            std::fs::read_to_string(&active).unwrap(),
            "foreign-active\n"
        );
        assert_lock_reacquirable(&fixture);

        let fixture = SemanticFixture::new();
        let foreign_slot = fixture.control().join("staging/31");
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::GenerationBeforeRename) {
                    std::fs::create_dir(&foreign_slot).unwrap();
                    std::fs::write(foreign_slot.join("foreign"), "preserve\n").unwrap();
                }
            })
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G153");
        assert_eq!(
            std::fs::read_to_string(foreign_slot.join("foreign")).unwrap(),
            "preserve\n"
        );
        assert!(!fixture.active().exists());
        assert_lock_reacquirable(&fixture);
    }

    #[test]
    fn semantic_staging_slot_zero_race_and_all_slots_exhaustion_fail_closed() {
        let fixture = SemanticFixture::new();
        let slot_zero = fixture.control().join("staging/0");
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::SemanticStagingReady) {
                    std::fs::create_dir(&slot_zero).unwrap();
                    std::fs::write(slot_zero.join("foreign"), "slot-zero\n").unwrap();
                }
            })
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G153");
        assert_eq!(
            std::fs::read_to_string(slot_zero.join("foreign")).unwrap(),
            "slot-zero\n"
        );
        assert!(!fixture.active().exists());
        assert_lock_reacquirable(&fixture);

        let fixture = SemanticFixture::new();
        let staging = fixture.control().join("staging");
        let error =
            initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                if matches!(point, SemanticInitializePoint::SemanticStagingReady) {
                    for ordinal in 0..32 {
                        let slot = staging.join(ordinal.to_string());
                        std::fs::create_dir(&slot).unwrap();
                        std::fs::write(slot.join("foreign"), ordinal.to_string()).unwrap();
                    }
                }
            })
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G175");
        assert_eq!(
            error[0].message,
            "Semantic Workspace `staging_attempts` exceeds 32"
        );
        for ordinal in 0..32 {
            assert_eq!(
                std::fs::read_to_string(staging.join(ordinal.to_string()).join("foreign")).unwrap(),
                ordinal.to_string()
            );
        }
        assert!(!fixture.active().exists());
        assert_lock_reacquirable(&fixture);
    }

    #[test]
    fn semantic_final_boundary_rechecks_sources_paths_control_generation_and_active_stage() {
        for target in [
            "source",
            "path-set",
            "control",
            "generation",
            "active-stage",
        ] {
            let fixture = SemanticFixture::new();
            let revision = fixture.expected_preflight().workspace_revision().to_owned();
            let error =
                initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                    if !matches!(point, SemanticInitializePoint::ActiveBeforeRename) {
                        return;
                    }
                    match target {
                        "source" => {
                            replace_with_same_bytes(&fixture.root.0.join("z/app.spx"));
                        }
                        "path-set" => replace_with_same_bytes(&fixture.path_set),
                        "control" => {
                            std::fs::write(fixture.control().join("foreign"), "drift\n").unwrap();
                        }
                        "generation" => replace_with_same_bytes(
                            &fixture.generation(&revision).join("manifest.json"),
                        ),
                        "active-stage" => {
                            replace_with_same_bytes(&fixture.control().join("staging/0"))
                        }
                        _ => unreachable!(),
                    }
                })
                .unwrap_err();
            assert_eq!(error.len(), 1, "{target}");
            assert_eq!(error[0].code, "SPX-G153", "{target}");
            assert!(!fixture.active().exists(), "{target}");
            assert_lock_reacquirable(&fixture);
        }
    }

    #[test]
    fn semantic_post_pivot_drift_is_i212_and_releases_the_lock() {
        for target in ["active", "generation"] {
            let fixture = SemanticFixture::new();
            let revision = fixture.expected_preflight().workspace_revision().to_owned();
            let result =
                initialize_from_preflight_with_hook(&fixture.root.0, &fixture.path_set, |point| {
                    if matches!(point, SemanticInitializePoint::ActiveRelocated) {
                        if target == "active" {
                            replace_with_same_bytes(&fixture.active());
                        } else {
                            replace_with_same_bytes(
                                &fixture.generation(&revision).join("manifest.json"),
                            );
                        }
                    }
                });
            let error = match result {
                Err(error) => error,
                Ok(revision) => {
                    panic!("post-pivot {target} drift unexpectedly succeeded as {revision}")
                }
            };
            assert_eq!(error.len(), 1, "{target}");
            assert_eq!(error[0].code, "SPX-I212", "{target}");
            assert!(fixture.active().exists(), "{target}");
            assert_lock_reacquirable(&fixture);
        }
    }

    #[test]
    fn semantic_and_ordinary_initializers_reject_the_other_control_schema() {
        let semantic = SemanticFixture::new();
        let ordinary_bytes = "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n";
        std::fs::write(&semantic.path_set, ordinary_bytes).unwrap();
        let error = initialize_from_preflight(&semantic.root.0, &semantic.path_set).unwrap_err();
        assert_eq!(error[0].code, "SPX-G174");
        assert!(!semantic.control().exists());

        let ordinary = SemanticFixture::new();
        let error = crate::workspace::initialize(&ordinary.root.0, &ordinary.path_set).unwrap_err();
        assert_eq!(error[0].code, "SPX-G150");
        assert!(!ordinary.control().exists());
    }

    #[test]
    fn ordinary_workspace_initializer_still_rejects_imports_without_control_writes() {
        let root = TempRoot::new();
        let sources = importing_sources();
        for source in &sources {
            let destination = root.0.join(&source.path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(destination, &source.source).unwrap();
        }
        let ordinary_path_set = root.0.join("paths.json");
        let path_set_bytes = "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n";
        std::fs::write(&ordinary_path_set, path_set_bytes).unwrap();
        let before = sources
            .iter()
            .map(|source| {
                (
                    source.path.clone(),
                    std::fs::read(root.0.join(&source.path)).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        let error = crate::workspace::initialize(&root.0, &ordinary_path_set).unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(
            error[0].message,
            "source module imports require Workspace Semantic Graph resolution"
        );
        assert!(!root.0.join(".semaprax-workspace").exists());
        assert_eq!(
            std::fs::read_to_string(&ordinary_path_set).unwrap(),
            path_set_bytes
        );
        assert_eq!(
            before,
            sources
                .iter()
                .map(|source| (
                    source.path.clone(),
                    std::fs::read(root.0.join(&source.path)).unwrap()
                ))
                .collect::<Vec<_>>()
        );
    }
}
