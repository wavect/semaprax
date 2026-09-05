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

    #[cfg(test)]
    pub(crate) fn source_mut(&mut self) -> &mut String {
        &mut self.source
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

pub(crate) fn preflight_owned_with_frontend(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    frontend: &mut crate::project::incremental::FrontendPass,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    preflight_owned_inner_mode(path_set_source, sources, None, false, None, Some(frontend))
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
        None,
    )
}

fn preflight_owned_inner(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: Option<usize>,
) -> Result<SemanticWorkspacePreflight, Vec<Diagnostic>> {
    preflight_owned_inner_mode(
        path_set_source,
        sources,
        change_builder_limit,
        false,
        None,
        None,
    )
}

fn preflight_owned_inner_mode(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: Option<usize>,
    retain_operations: bool,
    graph_builder_limit: Option<usize>,
    frontend: Option<&mut crate::project::incremental::FrontendPass>,
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
    } else if let Some(frontend) = frontend {
        workspace_graph::build_owned_retaining_sources_with_frontend(graph_sources, frontend)?
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
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
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
            | "semaprax.graph.v15"
            | "semaprax.graph.v16"
            | "semaprax.graph.v17"
            | "semaprax.graph.v18"
            | "semaprax.graph.v19"
            | "semaprax.graph.v20"
            | "semaprax.graph.v21"
            | "semaprax.graph.v22"
            | "semaprax.graph.v23"
            | "semaprax.graph.v24"
            | "semaprax.graph.v25"
            | "semaprax.graph.v26"
            | "semaprax.graph.v27"
            | "semaprax.graph.v28"
            | "semaprax.graph.v29"
            | "semaprax.graph.v30"
            | "semaprax.graph.v31"
            | "semaprax.graph.v32"
            | "semaprax.graph.v33"
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
#[path = "semantic_workspace/tests.rs"]
mod tests;
