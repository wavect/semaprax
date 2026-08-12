//! Private stable-identity operation compiler for Semantic Workspace Change v1.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::{semantic_workspace, semantic_workspace_change, workspace_graph};

const SCHEMA: &str = "semaprax.semantic-workspace-operations.v1";
const DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-workspace-operations.proposal-digest.v1\0";
const MAX_PROPOSAL_BYTES: usize = 1_048_576;
const MAX_OPERATIONS: usize = 64;
const MAX_AFFECTED_PATHS: usize = 16;
const MAX_PATH_BYTES: usize = 240;
const MAX_TARGET_ID_BYTES: usize = 4096;
const MAX_TARGET_MODULE_BYTES: usize = 240;
const MAX_ENTRY_MODULE_BYTES: usize = 16_777_216;
const MAX_NAME_BYTES: usize = 128;
const MAX_PLANNED_EDITS: usize = 131_072;
const MAX_EDIT_REPLACEMENT_BYTES: usize = 16_777_216;
const MAX_TOTAL_SOURCE_BYTES: usize = 16_777_216;
const MAX_TOTAL_REPLACEMENT_SOURCE_BYTES: usize = 4_194_304;
const MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH: usize = 1_048_576;
const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16_777_216;
const MAX_OPERATIONS_BUILDER_BYTES: usize = 67_108_864;
const MAX_DERIVED_CHANGE_PROPOSAL_BYTES: usize = 33_554_432;
const MAX_JSON_DEPTH: usize = 8;

#[cfg(test)]
thread_local! {
    static CANDIDATE_PREFLIGHT_ENTRY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_candidate_preflight_entry_count() {
    CANDIDATE_PREFLIGHT_ENTRY_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn candidate_preflight_entry_count() -> usize {
    CANDIDATE_PREFLIGHT_ENTRY_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
fn mark_candidate_preflight_entry() {
    CANDIDATE_PREFLIGHT_ENTRY_COUNT.with(|count| count.set(count.get() + 1));
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DeclarationSubject {
    Function,
    FunctionTemplate,
    Resource,
    Record,
    Variant,
    Interface,
}

impl DeclarationSubject {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "function" => Self::Function,
            "function_template" => Self::FunctionTemplate,
            "resource" => Self::Resource,
            "record" => Self::Record,
            "variant" => Self::Variant,
            "interface" => Self::Interface,
            _ => return None,
        })
    }

    const fn text(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::FunctionTemplate => "function_template",
            Self::Resource => "resource",
            Self::Record => "record",
            Self::Variant => "variant",
            Self::Interface => "interface",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ImportSubject {
    Function,
    Type,
}

impl ImportSubject {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "function" => Self::Function,
            "type" => Self::Type,
            _ => return None,
        })
    }

    const fn text(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Type => "type",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Operation {
    Declaration {
        path: String,
        subject: DeclarationSubject,
        target_id: String,
        from: String,
        to: String,
    },
    ImportAlias {
        path: String,
        subject: ImportSubject,
        target_id: String,
        target_module: String,
        from: String,
        to: String,
    },
}

impl Operation {
    fn sort_key(&self) -> (&str, u8, u8, &str, &str, &str, &str) {
        match self {
            Self::Declaration {
                path,
                subject,
                target_id,
                from,
                to,
            } => (path, 0, *subject as u8, target_id, "", from, to),
            Self::ImportAlias {
                path,
                subject,
                target_id,
                target_module,
                from,
                to,
            } => (path, 1, *subject as u8, target_id, target_module, from, to),
        }
    }

    fn selector(&self) -> (&str, u8, u8, &str, &str) {
        let (path, rank, subject, id, module, _, _) = self.sort_key();
        (path, rank, subject, id, module)
    }

    fn path(&self) -> &str {
        self.sort_key().0
    }
    fn from(&self) -> &str {
        self.sort_key().5
    }
    fn to(&self) -> &str {
        self.sort_key().6
    }
}

struct OperationsProposal {
    base_workspace_revision: String,
    entry_module: String,
    operations: Vec<Operation>,
    source: String,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlannedEditFact {
    pub(crate) path: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
    pub(crate) operation_index: usize,
}

pub(crate) struct PreparedSemanticWorkspaceOperations {
    proposal_source: String,
    proposal_digest: String,
    operations: Vec<Operation>,
    edits: Vec<PlannedEditFact>,
    candidate_sources: Vec<semantic_workspace::SemanticWorkspaceSource>,
    derived_change: semantic_workspace_change::SemanticWorkspaceChangeSet,
    base_graph: workspace_graph::WorkspaceGraphChangeView,
    candidate_graph: workspace_graph::WorkspaceGraphChangeView,
    used_operations_builder_bytes: usize,
}

impl PreparedSemanticWorkspaceOperations {
    pub(crate) fn proposal_source(&self) -> &str {
        &self.proposal_source
    }
    pub(crate) fn proposal_digest(&self) -> &str {
        &self.proposal_digest
    }
    pub(crate) fn edits(&self) -> &[PlannedEditFact] {
        &self.edits
    }
    pub(crate) fn derived_change_proposal(&self) -> &str {
        self.derived_change.source()
    }
    pub(crate) fn derived_change(&self) -> &semantic_workspace_change::SemanticWorkspaceChangeSet {
        &self.derived_change
    }
    pub(crate) fn operations_len(&self) -> usize {
        self.operations.len()
    }
    pub(crate) fn candidate_sources(&self) -> &[semantic_workspace::SemanticWorkspaceSource] {
        &self.candidate_sources
    }
    pub(crate) fn base_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.base_graph
    }
    pub(crate) fn candidate_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.candidate_graph
    }
    pub(crate) fn used_operations_builder_bytes(&self) -> usize {
        self.used_operations_builder_bytes
    }
}

fn parse_proposal(source: &str) -> Result<OperationsProposal, Vec<Diagnostic>> {
    if source.len() > MAX_PROPOSAL_BYTES {
        return Err(limit("operations_proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    if !source.ends_with('\n') || source[..source.len().saturating_sub(1)].contains('\n') {
        return Err(grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"));
    }
    let body = &source[..source.len() - 1];
    if json_depth(body)? > MAX_JSON_DEPTH {
        return Err(limit("json_depth", MAX_JSON_DEPTH));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"))?;
    let object = value.as_object().ok_or_else(|| {
        grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
    })?;
    if object.len() != 4
        || !object.contains_key("schema")
        || !object.contains_key("base_workspace_revision")
        || !object.contains_key("entry_module")
        || !object.contains_key("operations")
    {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    if text(object.get("schema"))? != SCHEMA {
        return Err(grammar(
            "Semantic Workspace Operations proposal schema is unsupported",
        ));
    }
    let base_workspace_revision = text(object.get("base_workspace_revision"))?.to_owned();
    let entry_module = text(object.get("entry_module"))?.to_owned();
    if !valid_digest(&base_workspace_revision) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    if entry_module.len() > MAX_ENTRY_MODULE_BYTES {
        return Err(limit("entry_module_bytes", MAX_ENTRY_MODULE_BYTES));
    }
    if !valid_qualified_module(&entry_module) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    let rows = object
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
        })?;
    if rows.len() < 2 {
        return Err(grammar(
            "Semantic Workspace Operations requires 2..64 operations",
        ));
    }
    if rows.len() > MAX_OPERATIONS {
        return Err(limit("operations", MAX_OPERATIONS));
    }
    let mut operations = Vec::with_capacity(rows.len());
    for row in rows {
        operations.push(parse_operation(row)?);
    }
    if operations
        .windows(2)
        .any(|pair| pair[0].sort_key() >= pair[1].sort_key())
    {
        return Err(grammar(
            "Semantic Workspace Operations operations must be strictly sorted",
        ));
    }
    let selectors = operations
        .iter()
        .map(Operation::selector)
        .collect::<BTreeSet<_>>();
    if selectors.len() != operations.len() {
        return Err(conflict(
            "Semantic Workspace Operations operation selector is duplicated",
        ));
    }
    let paths = operations
        .iter()
        .map(Operation::path)
        .collect::<BTreeSet<_>>();
    if paths.len() < 2 {
        return Err(conflict(
            "Semantic Workspace Operations require 2..16 distinct affected managed paths",
        ));
    }
    if paths.len() > MAX_AFFECTED_PATHS {
        return Err(limit("affected_paths", MAX_AFFECTED_PATHS));
    }
    let proposal = OperationsProposal {
        base_workspace_revision,
        entry_module,
        operations,
        source: source.to_owned(),
        digest: proposal_digest(source),
    };
    if render_proposal(&proposal)? != source {
        return Err(grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"));
    }
    Ok(proposal)
}

fn parse_operation(value: &Value) -> Result<Operation, Vec<Diagnostic>> {
    let object = value.as_object().ok_or_else(|| {
        grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
    })?;
    let kind = text(object.get("kind"))?;
    let operation = match kind {
        "rename_declaration" => {
            exact_keys(
                object,
                &[
                    "kind",
                    "path",
                    "declaration_kind",
                    "target_id",
                    "from",
                    "to",
                ],
            )?;
            Operation::Declaration {
                path: bounded(
                    text(object.get("path"))?,
                    "path_bytes",
                    MAX_PATH_BYTES,
                    false,
                )?,
                subject: DeclarationSubject::parse(text(object.get("declaration_kind"))?)
                    .ok_or_else(|| {
                        grammar("Semantic Workspace Operations operation kind is unsupported")
                    })?,
                target_id: bounded(
                    text(object.get("target_id"))?,
                    "target_id_bytes",
                    MAX_TARGET_ID_BYTES,
                    false,
                )?,
                from: bounded(
                    text(object.get("from"))?,
                    "name_bytes",
                    MAX_NAME_BYTES,
                    true,
                )?,
                to: bounded(text(object.get("to"))?, "name_bytes", MAX_NAME_BYTES, true)?,
            }
        }
        "rename_import_alias" => {
            exact_keys(
                object,
                &[
                    "kind",
                    "path",
                    "import_kind",
                    "target_id",
                    "target_module",
                    "from",
                    "to",
                ],
            )?;
            Operation::ImportAlias {
                path: bounded(
                    text(object.get("path"))?,
                    "path_bytes",
                    MAX_PATH_BYTES,
                    false,
                )?,
                subject: ImportSubject::parse(text(object.get("import_kind"))?).ok_or_else(
                    || grammar("Semantic Workspace Operations operation kind is unsupported"),
                )?,
                target_id: bounded(
                    text(object.get("target_id"))?,
                    "target_id_bytes",
                    MAX_TARGET_ID_BYTES,
                    false,
                )?,
                target_module: bounded(
                    text(object.get("target_module"))?,
                    "target_module_bytes",
                    MAX_TARGET_MODULE_BYTES,
                    false,
                )?,
                from: bounded(
                    text(object.get("from"))?,
                    "name_bytes",
                    MAX_NAME_BYTES,
                    true,
                )?,
                to: bounded(text(object.get("to"))?, "name_bytes", MAX_NAME_BYTES, true)?,
            }
        }
        _ => {
            return Err(grammar(
                "Semantic Workspace Operations operation kind is unsupported",
            ))
        }
    };
    if operation.from() == operation.to() {
        return Err(conflict(
            "Semantic Workspace Operations rename must change the source name",
        ));
    }
    if !crate::workspace::evidence_path_is_valid(operation.path()) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    if let Operation::ImportAlias { target_module, .. } = &operation {
        if !valid_qualified_module(target_module) {
            return Err(grammar(
                "Semantic Workspace Operations proposal object keys or value types are invalid",
            ));
        }
    }
    Ok(operation)
}

pub(crate) fn prepare_owned(
    proposal_source: &str,
    base: semantic_workspace::SemanticWorkspacePreflight,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
    prepare_owned_with_limit(proposal_source, base, MAX_OPERATIONS_BUILDER_BYTES)
}

fn prepare_owned_with_limit(
    proposal_source: &str,
    base: semantic_workspace::SemanticWorkspacePreflight,
    operations_builder_limit: usize,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
    assert!(
        operations_builder_limit <= MAX_OPERATIONS_BUILDER_BYTES,
        "private Semantic Workspace Operations builder limit cannot exceed the production maximum"
    );
    let base_total = base
        .files()
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.bytes()))
        .ok_or_else(|| limit("total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
    if base_total > MAX_TOTAL_SOURCE_BYTES {
        return Err(limit("total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES));
    }
    let (base_files, _, base_workspace_revision, base_build) = base.into_snapshot_parts();
    let operation_view = base_build.into_operation_view().map_err(|_| replay())?;
    if operation_view.builder_bytes > operations_builder_limit {
        return Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ));
    }
    let remaining = operations_builder_limit - operation_view.builder_bytes;
    let base_builder_bytes = operation_view.builder_bytes;
    let (result, overflowed, replay_builder_bytes) =
        crate::bounded_output::with_limit_usage(remaining, || {
            prepare_owned_inner(
                proposal_source,
                &base_workspace_revision,
                base_files,
                operation_view,
            )
        });
    if overflowed {
        return Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ));
    }
    let mut prepared = result?;
    prepared.used_operations_builder_bytes =
        base_builder_bytes
            .checked_add(replay_builder_bytes)
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    Ok(prepared)
}

fn prepare_owned_inner(
    proposal_source: &str,
    base_workspace_revision: &str,
    base_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    operation_view: workspace_graph::WorkspaceGraphOperationView,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
    // Account for the transient serde tree, retained typed operation objects,
    // and their strings before parsing allocates any of them. Four payloads is
    // a conservative bound for this shallow, capped canonical grammar.
    reserve_operations(
        proposal_source
            .len()
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(MAX_OPERATIONS * std::mem::size_of::<Operation>()))
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let proposal = parse_proposal(proposal_source)?;
    if base_workspace_revision != proposal.base_workspace_revision {
        return Err(binding(false));
    }
    if !operation_view
        .graph
        .modules()
        .iter()
        .any(|module| module.module() == proposal.entry_module)
    {
        return Err(binding(false));
    }
    validate_candidate_namespaces(&operation_view.sidecar, &proposal.operations)?;
    reserve_operations(
        base_files
            .len()
            .checked_mul(std::mem::size_of::<(
                String,
                semantic_workspace::SemanticWorkspaceFileFact,
            )>())
            .and_then(|bytes| {
                bytes.checked_add(
                    base_files
                        .iter()
                        .map(|file| file.path().len())
                        .sum::<usize>(),
                )
            })
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let base_paths = base_files
        .iter()
        .map(|file| crate::bounded_output::budgeted_clone(file.path()))
        .collect::<Vec<_>>();
    let mut sources = base_files
        .into_iter()
        .map(|file| (file.path().to_owned(), file))
        .collect::<BTreeMap<_, _>>();
    let mut edits = Vec::new();
    for (index, operation) in proposal.operations.iter().enumerate() {
        let file = sources
            .get(operation.path())
            .ok_or_else(|| binding(matches!(operation, Operation::ImportAlias { .. })))?;
        let remaining_edits = MAX_PLANNED_EDITS
            .checked_sub(edits.len())
            .ok_or_else(|| limit("planned_edits", MAX_PLANNED_EDITS))?;
        let mut selected = select_occurrences(
            file.source(),
            operation,
            &operation_view.sidecar,
            remaining_edits,
        )?;
        if selected.is_empty() {
            return Err(conflict(
                "Semantic Workspace Operations source occurrence proof is incomplete",
            ));
        }
        let new_edit_count = edits
            .len()
            .checked_add(selected.len())
            .ok_or_else(|| limit("planned_edits", MAX_PLANNED_EDITS))?;
        if new_edit_count > MAX_PLANNED_EDITS {
            return Err(limit("planned_edits", MAX_PLANNED_EDITS));
        }
        for (start, end) in selected.drain(..) {
            reserve_operations(
                std::mem::size_of::<PlannedEditFact>()
                    .checked_add(operation.path().len())
                    .and_then(|bytes| bytes.checked_add(operation.to().len()))
                    .ok_or_else(|| {
                        limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
                    })?,
            )?;
            edits.push(PlannedEditFact {
                path: operation.path().to_owned(),
                start,
                end,
                replacement: operation.to().to_owned(),
                operation_index: index,
            });
        }
    }
    if edits.len() > MAX_PLANNED_EDITS {
        return Err(limit("planned_edits", MAX_PLANNED_EDITS));
    }
    edits.sort_by(|a, b| {
        (&a.path, a.start, a.end, a.operation_index).cmp(&(
            &b.path,
            b.start,
            b.end,
            b.operation_index,
        ))
    });
    if edits
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path && pair[0].end > pair[1].start)
    {
        return Err(conflict(
            "Semantic Workspace Operations derived source edit ranges overlap",
        ));
    }
    let replacement_bytes = edits
        .iter()
        .try_fold(0usize, |n, e| n.checked_add(e.replacement.len()))
        .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
    if replacement_bytes > MAX_EDIT_REPLACEMENT_BYTES {
        return Err(limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES));
    }
    let mut by_path = BTreeMap::<String, Vec<&PlannedEditFact>>::new();
    for edit in &edits {
        reserve_operations(
            std::mem::size_of::<(String, Vec<&PlannedEditFact>)>()
                .checked_add(std::mem::size_of::<&PlannedEditFact>())
                .and_then(|bytes| bytes.checked_add(edit.path.len()))
                .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
        )?;
        by_path.entry(edit.path.clone()).or_default().push(edit);
    }
    let mut candidate_sources = Vec::with_capacity(sources.len());
    reserve_operations(
        sources
            .len()
            .checked_mul(std::mem::size_of::<
                semantic_workspace::SemanticWorkspaceSource,
            >())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut replacement_total = 0usize;
    let mut change_files = Vec::new();
    for path in &base_paths {
        let file = sources.remove(path).ok_or_else(replay)?;
        let source =
            if let Some(path_edits) = by_path.get(path) {
                render_candidate_source(file.source(), path_edits)?
            } else {
                reserve_operations(path.len().checked_add(file.source().len()).ok_or_else(
                    || limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES),
                )?)?;
                file.source().to_owned()
            };
        if by_path.contains_key(path) {
            validate_replacement_source_per_path(&source)?;
            replacement_total = replacement_total.checked_add(source.len()).ok_or_else(|| {
                limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                )
            })?;
            if replacement_total > MAX_TOTAL_REPLACEMENT_SOURCE_BYTES {
                return Err(limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                ));
            }
            reserve_operations(
                std::mem::size_of::<semantic_workspace_change::SemanticWorkspaceChangeFile>()
                    .checked_add(path.len())
                    .and_then(|bytes| bytes.checked_add(file.source_graph_schema().len()))
                    .and_then(|bytes| bytes.checked_add(file.source_revision().len()))
                    .and_then(|bytes| bytes.checked_add(file.source_digest().len()))
                    .and_then(|bytes| bytes.checked_add(source.len()))
                    .ok_or_else(|| {
                        limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
                    })?,
            )?;
            change_files.push(semantic_workspace_change::SemanticWorkspaceChangeFile::new(
                path.clone(),
                file.source_graph_schema().to_owned(),
                file.source_revision().to_owned(),
                file.source_digest().to_owned(),
                source.clone(),
            )?);
        }
        candidate_sources.push(semantic_workspace::SemanticWorkspaceSource {
            path: path.clone(),
            source,
        });
    }
    let candidate_total = candidate_sources
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.source.len()))
        .ok_or_else(|| limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
    if candidate_total > MAX_TOTAL_SOURCE_BYTES {
        return Err(limit(
            "total_candidate_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        ));
    }
    let path_set = semantic_workspace::render_path_set(&base_paths)?;
    let remaining_operations = crate::bounded_output::active_remaining()
        .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    let candidate_graph_limit = MAX_CANDIDATE_GRAPH_BUILDER_BYTES.min(remaining_operations);
    #[cfg(test)]
    mark_candidate_preflight_entry();
    let candidate = semantic_workspace::preflight_owned_for_operations(
        &path_set,
        candidate_sources,
        candidate_graph_limit,
        remaining_operations,
    )
    .map_err(|diagnostics| {
        map_candidate_diagnostics(diagnostics, candidate_graph_limit, remaining_operations)
    })?;
    let (candidate_files, _, candidate_revision, candidate_build) = candidate.into_snapshot_parts();
    let candidate_view = candidate_build
        .into_operation_view()
        .map_err(|_| replay())?;
    replay_candidate(&operation_view, &candidate_view, &proposal.operations)?;
    let derived_change = semantic_workspace_change::SemanticWorkspaceChangeSet::new(
        proposal.base_workspace_revision.clone(),
        proposal.entry_module.clone(),
        change_files,
    )?;
    if derived_change.source().len() > MAX_DERIVED_CHANGE_PROPOSAL_BYTES
        || semantic_workspace_change::parse_proposal(derived_change.source())?.source()
            != derived_change.source()
    {
        return Err(replay());
    }
    if candidate_revision == proposal.base_workspace_revision {
        return Err(replay());
    }
    reserve_operations(
        candidate_files
            .len()
            .checked_mul(std::mem::size_of::<
                semantic_workspace::SemanticWorkspaceSource,
            >())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let candidate_sources = candidate_files
        .into_iter()
        .map(|file| {
            let (path, _, _, _, source) = file.into_parts();
            semantic_workspace::SemanticWorkspaceSource { path, source }
        })
        .collect();
    Ok(PreparedSemanticWorkspaceOperations {
        proposal_source: proposal.source,
        proposal_digest: proposal.digest,
        operations: proposal.operations,
        edits,
        candidate_sources,
        derived_change,
        base_graph: operation_view.graph,
        candidate_graph: candidate_view.graph,
        used_operations_builder_bytes: 0,
    })
}

fn reserve_operations(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ))
    }
}

fn render_candidate_source(
    original: &str,
    edits: &[&PlannedEditFact],
) -> Result<String, Vec<Diagnostic>> {
    let mut removed_bytes = 0usize;
    let mut replacement_bytes = 0usize;
    let mut cursor = 0usize;
    for edit in edits {
        if edit.start < cursor
            || edit.end < edit.start
            || edit.end > original.len()
            || !original.is_char_boundary(edit.start)
            || !original.is_char_boundary(edit.end)
        {
            return Err(replay());
        }
        removed_bytes = removed_bytes
            .checked_add(edit.end - edit.start)
            .ok_or_else(|| limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        replacement_bytes = replacement_bytes
            .checked_add(edit.replacement.len())
            .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
        cursor = edit.end;
    }
    let final_len = original
        .len()
        .checked_sub(removed_bytes)
        .and_then(|bytes| bytes.checked_add(replacement_bytes))
        .ok_or_else(|| limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
    if final_len > MAX_TOTAL_SOURCE_BYTES {
        return Err(limit(
            "total_candidate_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        ));
    }
    reserve_operations(final_len)?;
    let mut rendered = String::with_capacity(final_len);
    cursor = 0;
    for edit in edits {
        rendered.push_str(&original[cursor..edit.start]);
        rendered.push_str(&edit.replacement);
        cursor = edit.end;
    }
    rendered.push_str(&original[cursor..]);
    if rendered.len() != final_len {
        return Err(replay());
    }
    Ok(rendered)
}

fn validate_replacement_source_per_path(source: &str) -> Result<(), Vec<Diagnostic>> {
    if source.len() > MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH {
        Err(limit(
            "replacement_source_bytes_per_path",
            MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH,
        ))
    } else {
        Ok(())
    }
}

fn map_candidate_diagnostics(
    diagnostics: Vec<Diagnostic>,
    candidate_graph_limit: usize,
    operations_builder_limit: usize,
) -> Vec<Diagnostic> {
    if diagnostics.len() != 1 || diagnostics[0].code != "SPX-G171" {
        return diagnostics;
    }
    let builder_message =
        format!("Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}");
    if diagnostics[0].message == builder_message {
        return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
            limit(
                "candidate_graph_builder_bytes",
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            )
        } else {
            limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
        };
    }
    let change_message = format!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {}",
        operations_builder_limit
    );
    if diagnostics[0].message == change_message {
        return limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES);
    }
    diagnostics
}

fn validate_candidate_namespaces(
    sidecar: &workspace_graph::WorkspaceOperationSidecar,
    operations: &[Operation],
) -> Result<(), Vec<Diagnostic>> {
    let mut names = BTreeSet::new();
    for declaration in &sidecar.declarations {
        let category = match declaration.kind {
            "function" | "function_template" => 0_u8,
            "resource" | "record" | "variant" => 1,
            "interface" => 2,
            _ => return Err(replay()),
        };
        let final_name = operations
            .iter()
            .find_map(|operation| match operation {
                Operation::Declaration {
                    path,
                    subject,
                    target_id,
                    to,
                    ..
                } if path == &declaration.path
                    && subject.text() == declaration.kind
                    && target_id == &declaration.id =>
                {
                    Some(to.as_str())
                }
                _ => None,
            })
            .unwrap_or(&declaration.name);
        if !names.insert((declaration.path.as_str(), category, final_name)) {
            return Err(conflict(
                "Semantic Workspace Operations candidate name namespace conflicts",
            ));
        }
    }
    for import in &sidecar.imports {
        let category = match import.kind {
            "function" => 0_u8,
            "type" => 1,
            _ => return Err(replay()),
        };
        let final_name = operations
            .iter()
            .find_map(|operation| match operation {
                Operation::ImportAlias {
                    path,
                    subject,
                    target_id,
                    target_module,
                    to,
                    ..
                } if path == &import.path
                    && subject.text() == import.kind
                    && target_id == &import.target_id
                    && target_module == &import.target_module =>
                {
                    Some(to.as_str())
                }
                _ => None,
            })
            .unwrap_or(&import.alias);
        if !names.insert((import.path.as_str(), category, final_name)) {
            return Err(conflict(
                "Semantic Workspace Operations candidate name namespace conflicts",
            ));
        }
    }
    Ok(())
}

fn select_occurrences(
    source: &str,
    operation: &Operation,
    sidecar: &workspace_graph::WorkspaceOperationSidecar,
    remaining_edits: usize,
) -> Result<Vec<(usize, usize)>, Vec<Diagnostic>> {
    let occurrences = match operation {
        Operation::Declaration {
            path,
            subject,
            target_id,
            from,
            ..
        } => sidecar
            .declarations
            .iter()
            .find(|item| {
                item.path == *path
                    && item.explicit
                    && item.kind == subject.text()
                    && item.id == *target_id
                    && item.name == *from
            })
            .map(|item| item.occurrences.as_slice())
            .ok_or_else(|| binding(false))?,
        Operation::ImportAlias {
            path,
            subject,
            target_id,
            target_module,
            from,
            ..
        } => sidecar
            .imports
            .iter()
            .find(|item| {
                item.path == *path
                    && item.kind == subject.text()
                    && item.target_id == *target_id
                    && item.target_module == *target_module
                    && item.alias == *from
            })
            .map(|item| item.occurrences.as_slice())
            .ok_or_else(|| binding(true))?,
    };
    if occurrences.len() > remaining_edits {
        return Err(limit("planned_edits", MAX_PLANNED_EDITS));
    }
    reserve_operations(
        occurrences
            .len()
            .checked_mul(std::mem::size_of::<(usize, usize)>())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut spans = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        if source.get(occurrence.span.start..occurrence.span.end) != Some(operation.from()) {
            return Err(conflict(
                "Semantic Workspace Operations source occurrence proof is incomplete",
            ));
        }
        spans.push((occurrence.span.start, occurrence.span.end));
    }
    Ok(spans)
}

fn replay_candidate(
    base: &workspace_graph::WorkspaceGraphOperationView,
    candidate: &workspace_graph::WorkspaceGraphOperationView,
    operations: &[Operation],
) -> Result<(), Vec<Diagnostic>> {
    if base.graph.modules() != candidate.graph.modules() {
        return Err(replay());
    }
    for operation in operations {
        match operation {
            Operation::Declaration {
                path,
                subject,
                target_id,
                from,
                to,
            } => {
                let before = base
                    .sidecar
                    .declarations
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.explicit
                            && item.kind == subject.text()
                            && item.id == *target_id
                            && item.name == *from
                    })
                    .ok_or_else(replay)?;
                let after = candidate
                    .sidecar
                    .declarations
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.explicit
                            && item.kind == subject.text()
                            && item.id == *target_id
                            && item.name == *to
                    })
                    .ok_or_else(replay)?;
                if !same_occurrence_owners(&before.occurrences, &after.occurrences) {
                    return Err(replay());
                }
            }
            Operation::ImportAlias {
                path,
                subject,
                target_id,
                target_module,
                from,
                to,
            } => {
                let before = base
                    .sidecar
                    .imports
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.kind == subject.text()
                            && item.target_id == *target_id
                            && item.target_module == *target_module
                            && item.alias == *from
                    })
                    .ok_or_else(replay)?;
                let after = candidate
                    .sidecar
                    .imports
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.kind == subject.text()
                            && item.target_id == *target_id
                            && item.target_module == *target_module
                            && item.alias == *to
                    })
                    .ok_or_else(replay)?;
                if !same_occurrence_owners(&before.occurrences, &after.occurrences) {
                    return Err(replay());
                }
            }
        }
    }
    if !same_normalized_sidecar(&base.sidecar, &candidate.sidecar, operations) {
        return Err(replay());
    }
    reserve_operations(
        base.graph
            .declarations()
            .len()
            .checked_mul(std::mem::size_of::<String>() + std::mem::size_of::<usize>())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut allowed_fingerprint_owners = BTreeSet::new();
    for operation in operations {
        let occurrences = selected_occurrences(&base.sidecar, operation).ok_or_else(replay)?;
        for occurrence in occurrences {
            if let Some(owner) = &occurrence.owner {
                allowed_fingerprint_owners.insert(owner.as_str());
                if let Ok(index) = base
                    .graph
                    .declarations()
                    .binary_search_by(|declaration| declaration.id().cmp(owner))
                {
                    let declaration = &base.graph.declarations()[index];
                    if let Some(parent) = declaration.owner() {
                        allowed_fingerprint_owners.insert(parent);
                    }
                }
            }
        }
    }
    let before_declarations = base.graph.declarations();
    let after_declarations = candidate.graph.declarations();
    if before_declarations.len() != after_declarations.len() {
        return Err(replay());
    }
    for (before, after) in before_declarations.iter().zip(after_declarations) {
        let fingerprint_may_change = allowed_fingerprint_owners.contains(before.id())
            || before
                .owner()
                .is_some_and(|owner| allowed_fingerprint_owners.contains(owner));
        if before.id() != after.id()
            || before.kind() != after.kind()
            || before.origin() != after.origin()
            || before.owner() != after.owner()
            || before.path() != after.path()
            || before.module() != after.module()
            || (!fingerprint_may_change
                && before.semantic_fingerprint() != after.semantic_fingerprint())
        {
            return Err(replay());
        }
    }
    if !same_normalized_edges(base.graph.edges(), candidate.graph.edges(), operations)? {
        return Err(replay());
    }
    Ok(())
}

fn selected_occurrences<'a>(
    sidecar: &'a workspace_graph::WorkspaceOperationSidecar,
    operation: &Operation,
) -> Option<&'a [workspace_graph::WorkspaceOperationOccurrence]> {
    match operation {
        Operation::Declaration {
            path,
            subject,
            target_id,
            ..
        } => sidecar
            .declarations
            .iter()
            .find(|item| item.path == *path && item.kind == subject.text() && item.id == *target_id)
            .map(|item| item.occurrences.as_slice()),
        Operation::ImportAlias {
            path,
            subject,
            target_id,
            target_module,
            ..
        } => sidecar
            .imports
            .iter()
            .find(|item| {
                item.path == *path
                    && item.kind == subject.text()
                    && item.target_id == *target_id
                    && item.target_module == *target_module
            })
            .map(|item| item.occurrences.as_slice()),
    }
}

fn same_occurrence_owners(
    before: &[workspace_graph::WorkspaceOperationOccurrence],
    after: &[workspace_graph::WorkspaceOperationOccurrence],
) -> bool {
    before.len() == after.len()
        && before
            .iter()
            .zip(after)
            .all(|(left, right)| left.owner == right.owner)
}

fn same_normalized_sidecar(
    before: &workspace_graph::WorkspaceOperationSidecar,
    after: &workspace_graph::WorkspaceOperationSidecar,
    operations: &[Operation],
) -> bool {
    before.declarations.len() == after.declarations.len()
        && before
            .declarations
            .iter()
            .zip(&after.declarations)
            .all(|(left, right)| {
                let selected = operations.iter().any(|operation| {
                    matches!(operation,
                Operation::Declaration { path, subject, target_id, .. }
                    if *path == left.path && subject.text() == left.kind && *target_id == left.id)
                });
                left.path == right.path
                    && left.kind == right.kind
                    && left.explicit == right.explicit
                    && left.id == right.id
                    && left.normalized_fingerprint == right.normalized_fingerprint
                    && (selected || left.name == right.name)
                    && same_occurrence_owners(&left.occurrences, &right.occurrences)
            })
        && before.imports.len() == after.imports.len()
        && before
            .imports
            .iter()
            .zip(&after.imports)
            .all(|(left, right)| {
                let selected = operations.iter().any(|operation| {
                    matches!(operation,
                Operation::ImportAlias { path, subject, target_id, target_module, .. }
                    if *path == left.path && subject.text() == left.kind
                    && *target_id == left.target_id && *target_module == left.target_module)
                });
                left.path == right.path
                    && left.kind == right.kind
                    && left.target_id == right.target_id
                    && left.target_module == right.target_module
                    && (selected || left.alias == right.alias)
                    && same_occurrence_owners(&left.occurrences, &right.occurrences)
            })
}

fn same_normalized_edges(
    before: &[workspace_graph::WorkspaceEdge],
    after: &[workspace_graph::WorkspaceEdge],
    operations: &[Operation],
) -> Result<bool, Vec<Diagnostic>> {
    if before.len() != after.len() {
        return Ok(false);
    }
    type OccurrenceKey<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, usize);
    reserve_operations(
        before
            .len()
            .checked_mul(std::mem::size_of::<OccurrenceKey<'_>>())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut selected_calls = Vec::with_capacity(before.len());
    for edge in before.iter().filter(|edge| edge.kind() == "call") {
        if operations.iter().any(|operation| {
            matches!(operation,
            Operation::ImportAlias { path, target_id, from, .. }
                if edge.caller_path() == path && edge.target() == target_id && edge.alias() == from)
        }) {
            selected_calls.push((
                edge.caller_path(),
                edge.caller(),
                edge.site(),
                edge.expression(),
                edge.ast_path(),
                edge.ordinal(),
            ));
        }
    }
    selected_calls.sort_unstable();
    selected_calls.dedup();
    Ok(before.iter().zip(after).all(|(left, right)| {
        let alias_renamed = operations.iter().any(|operation| {
            let Operation::ImportAlias {
                path,
                target_id,
                from,
                to,
                ..
            } = operation
            else {
                return false;
            };
            if left.caller_path() != path || left.alias() != from || right.alias() != to {
                return false;
            }
            left.target() == target_id
                || (left.kind() == "effect_requirement"
                    && selected_calls
                        .binary_search(&(
                            left.caller_path(),
                            left.caller(),
                            left.site(),
                            left.expression(),
                            left.ast_path(),
                            left.ordinal(),
                        ))
                        .is_ok())
        });
        let same = left.caller_path() == right.caller_path()
            && left.caller() == right.caller()
            && left.target_path() == right.target_path()
            && left.target() == right.target()
            && left.kind() == right.kind()
            && left.site() == right.site()
            && left.expression() == right.expression()
            && left.ast_path() == right.ast_path()
            && (alias_renamed || left.alias() == right.alias())
            && left.ordinal() == right.ordinal();
        same
    }))
}

fn render_proposal(p: &OperationsProposal) -> Result<String, Vec<Diagnostic>> {
    let (s, o) = crate::bounded_output::with_limit(MAX_PROPOSAL_BYTES, || {
        let mut x = CappedString::new();
        x.push_str("{\"schema\":");
        json(&mut x, SCHEMA);
        x.push_str(",\"base_workspace_revision\":");
        json(&mut x, &p.base_workspace_revision);
        x.push_str(",\"entry_module\":");
        json(&mut x, &p.entry_module);
        x.push_str(",\"operations\":[");
        for (i, op) in p.operations.iter().enumerate() {
            if i > 0 {
                x.push(',')
            }
            render_op(&mut x, op)
        }
        x.push_str("]}\n");
        x.into_string()
    });
    if o {
        Err(limit("operations_proposal_bytes", MAX_PROPOSAL_BYTES))
    } else {
        Ok(s)
    }
}
fn render_op(x: &mut CappedString, op: &Operation) {
    match op {
        Operation::Declaration {
            path,
            subject,
            target_id,
            from,
            to,
        } => {
            x.push_str("{\"kind\":\"rename_declaration\",\"path\":");
            json(x, path);
            x.push_str(",\"declaration_kind\":");
            json(x, subject.text());
            x.push_str(",\"target_id\":");
            json(x, target_id);
            x.push_str(",\"from\":");
            json(x, from);
            x.push_str(",\"to\":");
            json(x, to)
        }
        Operation::ImportAlias {
            path,
            subject,
            target_id,
            target_module,
            from,
            to,
        } => {
            x.push_str("{\"kind\":\"rename_import_alias\",\"path\":");
            json(x, path);
            x.push_str(",\"import_kind\":");
            json(x, subject.text());
            x.push_str(",\"target_id\":");
            json(x, target_id);
            x.push_str(",\"target_module\":");
            json(x, target_module);
            x.push_str(",\"from\":");
            json(x, from);
            x.push_str(",\"to\":");
            json(x, to)
        }
    }
    x.push('}')
}
fn json(x: &mut CappedString, s: &str) {
    x.push('"');
    for c in s.chars() {
        match c {
            '"' => x.push_str("\\\""),
            '\\' => x.push_str("\\\\"),
            '\n' => x.push_str("\\n"),
            '\r' => x.push_str("\\r"),
            '\t' => x.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(x, "\\u{:04x}", c as u32);
            }
            c => x.push(c),
        }
    }
    x.push('"')
}
fn proposal_digest(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(DIGEST_DOMAIN);
    h.update((s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
    format!("sha256:{:x}", h.finalize())
}
fn exact_keys(o: &serde_json::Map<String, Value>, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    if o.len() != keys.len() || keys.iter().any(|k| !o.contains_key(*k)) {
        Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ))
    } else {
        Ok(())
    }
}
fn text(v: Option<&Value>) -> Result<&str, Vec<Diagnostic>> {
    v.and_then(Value::as_str).ok_or_else(|| {
        grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
    })
}
fn bounded(v: &str, field: &str, max: usize, identifier: bool) -> Result<String, Vec<Diagnostic>> {
    if v.len() > max {
        return Err(limit(field, max));
    }
    if v.is_empty() || (identifier && !valid_ident(v)) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    Ok(v.to_owned())
}
fn valid_ident(v: &str) -> bool {
    let mut c = v.chars();
    c.next()
        .is_some_and(|x| x == '_' || x.is_ascii_alphabetic())
        && c.all(|x| x == '_' || x.is_ascii_alphanumeric())
}
fn valid_qualified_module(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(valid_ident)
}
fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn json_depth(s: &str) -> Result<usize, Vec<Diagnostic>> {
    let mut d = 0usize;
    let mut m = 0usize;
    let mut q = false;
    let mut e = false;
    for c in s.chars() {
        if q {
            if e {
                e = false
            } else if c == '\\' {
                e = true
            } else if c == '"' {
                q = false
            }
        } else {
            match c{'"'=>q=true,'{'|'['=>{d+=1;m=m.max(d)},'}'|']'=>d=d.checked_sub(1).ok_or_else(||grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"))?,_=>{}}
        }
    }
    Ok(m)
}
fn grammar(m: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G196", m)]
}
fn binding(import: bool) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G197",
        if import {
            "Semantic Workspace Operations import alias does not match one direct explicit pre-state import binding"
        } else {
            "Semantic Workspace Operations target does not match one explicit user-owned pre-state declaration"
        },
    )]
}
fn conflict(m: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G198", m)]
}
fn limit(f: &str, m: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G199",
        format!("Semantic Workspace Operations exceeds {f} maximum {m}"),
    )]
}
fn replay() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G200","Semantic Workspace Operations derivation disagrees with authenticated base or candidate semantics")]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn canonical(path: &str, source: &str) -> String {
        let program = crate::parse(source, Path::new(path)).unwrap();
        crate::format::canonical(&program)
    }

    fn fixture() -> (semantic_workspace::SemanticWorkspacePreflight, String) {
        let provider = canonical(
            "a/provider.spx",
            "module ops.provider; @id(\"ops.answer\") fn answer()->i64{1}",
        );
        let consumer = canonical(
            "b/consumer.spx",
            "module ops.consumer; use function @id(\"ops.answer\") from ops.provider as answer; @id(\"ops.main\") fn main()->i64{answer()}",
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.answer\",\"from\":\"answer\",\"to\":\"response\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.answer\",\"target_module\":\"ops.provider\",\"from\":\"answer\",\"to\":\"response\"}}]}}\n",
            preflight.workspace_revision(),
        );
        (preflight, proposal)
    }

    fn broad_fixture() -> (semantic_workspace::SemanticWorkspacePreflight, String) {
        let provider = canonical(
            "a/provider.spx",
            r#"module ops.provider;
permit { audit.read }
@id("ops.token") resource Token { @id("ops.token.drop") drop trivial; }
@id("ops.point") record Point { @id("ops.point.value") value: i64, }
@id("ops.choice") variant Choice { @id("ops.choice.some") Some { @id("ops.choice.some.value") value: i64, }, @id("ops.choice.none") None, }
@id("ops.host") interface Host permits {} {
    @id("ops.host.release") import fn release(token: own Token) -> unit effects {} failure infallible consumes token always;
}
@id("ops.work") fn work(value: Point) -> i64 uses { audit.read } { value.value }
@id("ops.echo") fn echo<T>(value: T) -> T { value }
@id("ops.provider.main") fn main() -> i64 uses { audit.read } { match Choice::Some { value: 1 } { Choice::Some { value } => work(Point { value: echo<i64>(value) }), Choice::None {} => 0, } }
"#,
        );
        let consumer = canonical(
            "b/consumer.spx",
            r#"module ops.consumer;
use function @id("ops.work") from ops.provider as provider;
use type @id("ops.point") from ops.provider as Point;
permit { audit.read }
@id("ops.consumer.box") record Box { @id("ops.consumer.box.point") Point: i64, @id("ops.consumer.box.value") value: Point, }
@id("ops.consumer.wrapper") record Wrapper { @id("ops.consumer.wrapper.point") point: Point, }
@id("ops.consumer.identity") fn identity<T>(value: T) -> T { value }
@id("ops.consumer.shadow") fn shadow<Point>(Point: Point) -> Point { Point }
@id("ops.consumer.local") fn local(provider: i64) -> i64 { let Point = provider; Point }
@id("ops.consumer.preserve") fn preserve(value: Point) -> Point { value }
@id("ops.consumer.run") fn run(value: Point) -> i64 uses { audit.read } {
    let wrapped = Wrapper { point: value };
    match wrapped { Wrapper { point: nested } => provider(nested), }
}
@id("ops.consumer.main") fn main() -> i64 uses { audit.read } { run(Point { value: 0 }) }
"#,
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.work\",\"from\":\"work\",\"to\":\"compute\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function_template\",\"target_id\":\"ops.echo\",\"from\":\"echo\",\"to\":\"identity\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"resource\",\"target_id\":\"ops.token\",\"from\":\"Token\",\"to\":\"Handle\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.point\",\"from\":\"Point\",\"to\":\"Metric\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"variant\",\"target_id\":\"ops.choice\",\"from\":\"Choice\",\"to\":\"Outcome\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"interface\",\"target_id\":\"ops.host\",\"from\":\"Host\",\"to\":\"Runtime\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.work\",\"target_module\":\"ops.provider\",\"from\":\"provider\",\"to\":\"compute\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"type\",\"target_id\":\"ops.point\",\"target_module\":\"ops.provider\",\"from\":\"Point\",\"to\":\"Metric\"}}]}}\n",
            preflight.workspace_revision(),
        );
        (preflight, proposal)
    }

    fn code(result: Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>>) -> String {
        result.err().expect("expected failure")[0].code.to_owned()
    }

    #[test]
    fn compiles_two_prestate_operations_into_exact_change_v1() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        assert_eq!(prepared.proposal_source(), proposal);
        assert!(prepared.proposal_digest().starts_with("sha256:"));
        assert_eq!(prepared.operations_len(), 2);
        assert_eq!(
            prepared
                .edits()
                .iter()
                .map(|edit| edit.path.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["a/provider.spx", "b/consumer.spx"])
        );
        assert_eq!(prepared.candidate_sources().len(), 2);
        assert_eq!(prepared.base_graph().modules().len(), 2);
        assert_eq!(prepared.candidate_graph().modules().len(), 2);
        assert!(prepared.used_operations_builder_bytes() <= MAX_OPERATIONS_BUILDER_BYTES);
        assert_eq!(
            prepared.derived_change().source(),
            prepared.derived_change_proposal()
        );
        assert_eq!(
            semantic_workspace_change::parse_proposal(prepared.derived_change_proposal())
                .unwrap()
                .source(),
            prepared.derived_change_proposal()
        );
    }

    #[test]
    fn canonical_order_binding_and_path_cardinality_fail_closed() {
        let (base, proposal) = fixture();
        let reversed = proposal.replace(
            "{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\"",
            "{\"kind\":\"rename_declaration\",\"path\":\"z/provider.spx\"",
        );
        assert_eq!(parse_proposal(&reversed).err().unwrap()[0].code, "SPX-G196");
        let stale = proposal.replace(
            "\"target_id\":\"ops.answer\"",
            "\"target_id\":\"ops.missing\"",
        );
        assert_eq!(
            prepare_owned(&stale, base).err().unwrap()[0].code,
            "SPX-G197"
        );
    }

    #[test]
    fn typed_occurrence_sidecar_covers_constructor_arguments_and_nested_patterns() {
        let first = canonical(
            "a/first.spx",
            r#"module ops.first;
@id("ops.first.inner") record Inner { @id("ops.first.inner.value") value: i64, }
@id("ops.first.outer") record Outer { @id("ops.first.outer.inner") inner: Inner, }
@id("ops.first.holder") record Holder<T> { @id("ops.first.holder.value") value: T, }
@id("ops.first.use") fn use(input: Outer)->i64{let wrapped=Holder<i64>{value:1};match input{Outer{inner:Inner{value}}=>value+wrapped.value,}}"#,
        );
        let second = canonical(
            "b/second.spx",
            r#"module ops.second;
@id("ops.second.leaf") record Leaf { @id("ops.second.leaf.value") value: i64, }
@id("ops.second.outer") record Outer { @id("ops.second.outer.leaf") leaf: Leaf, }
@id("ops.second.holder") record Holder<T> { @id("ops.second.holder.value") value: T, }
@id("ops.second.use") fn use(input: Outer)->i64{let wrapped=Holder<i64>{value:1};match input{Outer{leaf:Leaf{value}}=>value+wrapped.value,}}"#,
        );
        let paths = vec!["a/first.spx".to_owned(), "b/second.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: first,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: second,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.first\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/first.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.first.inner\",\"from\":\"Inner\",\"to\":\"Core\"}},{{\"kind\":\"rename_declaration\",\"path\":\"b/second.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.second.leaf\",\"from\":\"Leaf\",\"to\":\"Node\"}}]}}\n",
            base.workspace_revision(),
        );
        let prepared = prepare_owned(&proposal, base).unwrap();
        let first = &prepared.candidate_sources()[0].source;
        assert!(first.contains("record Core"));
        assert!(first.contains("Holder<i64>"));
        assert!(first.contains("inner: Core"));
        assert!(first.contains("inner: Core {"));
        let second = &prepared.candidate_sources()[1].source;
        assert!(second.contains("record Node"));
        assert!(second.contains("Holder<i64>"));
        assert!(second.contains("leaf: Node"));
        assert!(second.contains("leaf: Node {"));
    }

    #[test]
    fn all_admitted_subjects_and_alias_occurrences_are_exact_without_textual_capture() {
        let (base, proposal) = broad_fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        for (caller, expected) in [("ops.consumer.preserve", 2), ("ops.consumer.shadow", 0)] {
            assert_eq!(
                prepared
                    .base_graph()
                    .edges()
                    .iter()
                    .filter(|edge| {
                        edge.kind() == "type_reference"
                            && edge.caller() == caller
                            && edge.target() == "ops.point"
                    })
                    .count(),
                expected
            );
        }
        assert_eq!(prepared.operations_len(), 8);
        assert_eq!(prepared.proposal_source(), proposal);
        assert_eq!(parse_proposal(&proposal).unwrap().source, proposal);
        assert_eq!(
            semantic_workspace_change::parse_proposal(prepared.derived_change_proposal())
                .unwrap()
                .source(),
            prepared.derived_change_proposal()
        );
        assert_eq!(
            prepared
                .edits()
                .iter()
                .map(|edit| edit.operation_index)
                .collect::<BTreeSet<_>>(),
            (0..8).collect::<BTreeSet<_>>()
        );
        assert!(prepared.edits().windows(2).all(|pair| {
            (
                &pair[0].path,
                pair[0].start,
                pair[0].end,
                pair[0].operation_index,
            ) < (
                &pair[1].path,
                pair[1].start,
                pair[1].end,
                pair[1].operation_index,
            )
        }));

        let provider = &prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path == "a/provider.spx")
            .unwrap()
            .source;
        for expected in [
            "resource Handle",
            "record Metric",
            "variant Outcome",
            "interface Runtime",
            "fn compute",
            "fn identity<T>",
            "identity<i64>(value)",
            "token: own Handle",
            "value: Metric",
            "Outcome::Some",
            "compute(Metric",
        ] {
            assert!(
                provider.contains(expected),
                "missing `{expected}` in {provider}"
            );
        }
        assert!(provider.contains("permit { audit.read }"));
        assert!(provider.contains("uses { audit.read }"));

        let consumer = &prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path == "b/consumer.spx")
            .unwrap()
            .source;
        for expected in [
            "from ops.provider as Metric",
            "from ops.provider as compute",
            "fn run(value: Metric)",
            "fn preserve(value: Metric) -> Metric",
            "Wrapper { point: value }",
            "compute(nested)",
            "run(Metric { value: 0 })",
        ] {
            assert!(
                consumer.contains(expected),
                "missing `{expected}` in {consumer}"
            );
        }
        for unrelated in [
            "Point: i64",
            "fn shadow<Point>(Point: Point) -> Point",
            "fn local(provider: i64)",
            "let Point = provider",
            "module ops.consumer",
        ] {
            assert!(
                consumer.contains(unrelated),
                "textual capture changed `{unrelated}`"
            );
        }
    }

    #[test]
    fn one_of_two_effectful_imports_changes_only_its_exact_call_family() {
        let provider = canonical(
            "a/provider.spx",
            r#"module selective.provider;
permit { audit.one, audit.two }
@id("selective.first") fn first()->i64 uses { audit.one } { 1 }
@id("selective.second") fn second()->i64 uses { audit.two } { 2 }
@id("selective.provider.main") fn main()->i64 { 0 }
"#,
        );
        let consumer = canonical(
            "b/consumer.spx",
            r#"module selective.consumer;
use function @id("selective.first") from selective.provider as first;
use function @id("selective.second") from selective.provider as second;
permit { audit.one, audit.two }
@id("selective.consumer.main") fn main()->i64 uses { audit.one, audit.two } { first() + second() }
"#,
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"selective.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"selective.first\",\"from\":\"first\",\"to\":\"chosen\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"selective.first\",\"target_module\":\"selective.provider\",\"from\":\"first\",\"to\":\"chosen\"}}]}}\n",
            base.workspace_revision(),
        );
        let prepared = prepare_owned(&proposal, base).unwrap();
        let selected = |edge: &&workspace_graph::WorkspaceEdge| {
            matches!(edge.kind(), "call" | "effect_requirement")
                && matches!(edge.target(), "selective.first" | "audit.one")
        };
        let untouched = |edge: &&workspace_graph::WorkspaceEdge| {
            matches!(edge.kind(), "call" | "effect_requirement")
                && matches!(edge.target(), "selective.second" | "audit.two")
        };
        let base_selected = prepared
            .base_graph()
            .edges()
            .iter()
            .filter(selected)
            .collect::<Vec<_>>();
        let candidate_selected = prepared
            .candidate_graph()
            .edges()
            .iter()
            .filter(selected)
            .collect::<Vec<_>>();
        assert_eq!(base_selected.len(), 2);
        assert_eq!(candidate_selected.len(), 2);
        assert!(base_selected.iter().all(|edge| edge.alias() == "first"));
        assert!(candidate_selected
            .iter()
            .all(|edge| edge.alias() == "chosen"));
        assert_eq!(
            prepared
                .base_graph()
                .edges()
                .iter()
                .filter(untouched)
                .collect::<Vec<_>>(),
            prepared
                .candidate_graph()
                .edges()
                .iter()
                .filter(untouched)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn trailing_as_keyword_and_alias_do_not_capture_the_module_segment() {
        let provider = canonical(
            "a/provider.spx",
            "module ops.as; @id(\"ops.as.answer\") fn answer()->i64{1}",
        );
        let consumer = canonical(
            "b/consumer.spx",
            "module ops.consumer; use function @id(\"ops.as.answer\") from ops.as as as; @id(\"ops.consumer.main\") fn main()->i64{as()}",
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.as.answer\",\"from\":\"answer\",\"to\":\"response\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.as.answer\",\"target_module\":\"ops.as\",\"from\":\"as\",\"to\":\"invoke\"}}]}}\n",
            base.workspace_revision(),
        );
        let prepared = prepare_owned(&proposal, base).unwrap();
        let consumer = &prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path == "b/consumer.spx")
            .unwrap()
            .source;
        assert!(consumer.contains("from ops.as as invoke"));
        assert!(consumer.contains("invoke()"));
        assert!(prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path == "a/provider.spx")
            .unwrap()
            .source
            .contains("module ops.as"));
        assert_eq!(
            prepared
                .edits()
                .iter()
                .filter(|edit| edit.operation_index == 1)
                .count(),
            2
        );
    }

    #[test]
    fn grammar_binding_conflict_and_limit_matrix_is_exact() {
        let (_base, proposal) = fixture();
        assert_eq!(parse_proposal(&proposal).unwrap().operations.len(), 2);
        for hostile in [
            proposal.trim_end_matches('\n').to_owned(),
            proposal.replace('\n', "\r\n"),
            format!("\u{feff}{proposal}"),
            proposal.replacen("{\"schema\":", "{\"extra\":0,\"schema\":", 1),
            proposal.replacen(
                "{\"kind\":\"rename_declaration\"",
                "{\"to\":\"response\",\"kind\":\"rename_declaration\"",
                1,
            ),
            proposal.replacen("a/provider.spx", "../provider.spx", 1),
            proposal.replacen("ops.provider", "ops..provider", 1),
            proposal.replacen("ops.consumer", "ops/consumer", 1),
            proposal.replacen("sha256:", "SHA256:", 1),
        ] {
            assert_eq!(parse_proposal(&hostile).err().unwrap()[0].code, "SPX-G196");
        }
        let one = proposal.replacen(
            ",{\"kind\":\"rename_import_alias\"",
            "]}\n__cut__{\"kind\":\"rename_import_alias\"",
            1,
        );
        let one = format!("{}\n", one.split("\n__cut__").next().unwrap());
        let diagnostics = parse_proposal(&one).err().unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G196");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations requires 2..64 operations"
        );
        let one_path =
            proposal.replace("\"path\":\"b/consumer.spx\"", "\"path\":\"a/provider.spx\"");
        let diagnostics = parse_proposal(&one_path).err().unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G198");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations require 2..16 distinct affected managed paths"
        );

        for (needle, replacement, expected) in [
            (
                "\"target_id\":\"ops.answer\"",
                "\"target_id\":\"ops.missing\"",
                "SPX-G197",
            ),
            ("\"from\":\"answer\"", "\"from\":\"missing\"", "SPX-G197"),
            (
                "\"target_module\":\"ops.provider\"",
                "\"target_module\":\"ops.other\"",
                "SPX-G197",
            ),
            ("\"to\":\"response\"", "\"to\":\"answer\"", "SPX-G198"),
        ] {
            let (fresh, _) = fixture();
            assert_eq!(
                code(prepare_owned(&proposal.replace(needle, replacement), fresh)),
                expected
            );
        }
        let (fresh, _) = fixture();
        let collision = proposal.replace("\"to\":\"response\"", "\"to\":\"main\"");
        assert_eq!(code(prepare_owned(&collision, fresh)), "SPX-G198");

        let long_id = "x".repeat(MAX_TARGET_ID_BYTES + 1);
        assert_eq!(
            parse_proposal(&proposal.replacen("ops.answer", &long_id, 1))
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds target_id_bytes maximum 4096"
        );
        let long_name = "x".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(
            parse_proposal(&proposal.replacen("response", &long_name, 1))
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds name_bytes maximum 128"
        );

        let revision = parse_proposal(&proposal).unwrap().base_workspace_revision;
        let rows = (0..65)
            .map(|index| {
                format!(
                    "{{\"kind\":\"rename_declaration\",\"path\":\"{index:02}/module.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.limit.{index:02}\",\"from\":\"f{index:02}\",\"to\":\"g{index:02}\"}}"
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let over_operations = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"ops.limit\",\"operations\":[{rows}]}}\n"
        );
        assert_eq!(
            parse_proposal(&over_operations).err().unwrap()[0].message,
            "Semantic Workspace Operations exceeds operations maximum 64"
        );
    }

    #[test]
    fn affected_path_cardinality_accepts_sixteen_and_rejects_seventeen() {
        let mut paths = Vec::new();
        let mut sources = Vec::new();
        let mut operations = Vec::new();
        for index in 0..MAX_AFFECTED_PATHS {
            let path = format!("{index:02}/module.spx");
            paths.push(path.clone());
            sources.push(semantic_workspace::SemanticWorkspaceSource {
                path: path.clone(),
                source: canonical(
                    &path,
                    &format!(
                        "module exact.path{index:02}; @id(\"exact.path{index:02}.target\") fn target()->i64{{{index}}} @id(\"exact.path{index:02}.main\") fn main()->i64{{target()}}"
                    ),
                ),
            });
            operations.push(format!(
                "{{\"kind\":\"rename_declaration\",\"path\":\"{path}\",\"declaration_kind\":\"function\",\"target_id\":\"exact.path{index:02}.target\",\"from\":\"target\",\"to\":\"renamed\"}}"
            ));
        }
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            sources,
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"exact.path00\",\"operations\":[{}]}}\n",
            base.workspace_revision(),
            operations.join(",")
        );
        let revision = base.workspace_revision().to_owned();
        let prepared = prepare_owned(&proposal, base).unwrap();
        assert_eq!(prepared.operations_len(), MAX_AFFECTED_PATHS);
        assert_eq!(prepared.candidate_sources().len(), MAX_AFFECTED_PATHS);

        operations.push(String::from(
            "{\"kind\":\"rename_declaration\",\"path\":\"16/module.spx\",\"declaration_kind\":\"function\",\"target_id\":\"exact.path16.target\",\"from\":\"target\",\"to\":\"renamed\"}",
        ));
        let over_paths = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"exact.path00\",\"operations\":[{}]}}\n",
            operations.join(",")
        );
        assert_eq!(
            parse_proposal(&over_paths).err().unwrap()[0].message,
            "Semantic Workspace Operations exceeds affected_paths maximum 16"
        );
    }

    #[test]
    fn automatic_compiler_and_unsupported_targets_are_not_addressable() {
        let first = canonical(
            "a/first.spx",
            "module ops.first; fn automatic()->i64{1} @id(\"ops.first.main\") fn main()->i64{automatic()}",
        );
        let second = canonical(
            "b/second.spx",
            "module ops.second; @id(\"ops.second.record\") record Record { @id(\"ops.second.field\") field:i64, } @id(\"ops.second.main\") fn main()->i64{0}",
        );
        let paths = vec!["a/first.spx".to_owned(), "b/second.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let make = || {
            semantic_workspace::preflight_owned_for_operations(
                &path_set,
                vec![
                    semantic_workspace::SemanticWorkspaceSource {
                        path: paths[0].clone(),
                        source: first.clone(),
                    },
                    semantic_workspace::SemanticWorkspaceSource {
                        path: paths[1].clone(),
                        source: second.clone(),
                    },
                ],
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
                MAX_OPERATIONS_BUILDER_BYTES,
            )
            .unwrap()
        };
        let revision = make().workspace_revision().to_owned();
        for (kind, id, from) in [
            ("function", "auto:ops.first.automatic", "automatic"),
            ("record", "ops.second.field", "field"),
            ("record", crate::prelude::OPTION_ID, "Option"),
        ] {
            let proposal = format!(
                "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"ops.first\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/first.spx\",\"declaration_kind\":\"{kind}\",\"target_id\":\"{id}\",\"from\":\"{from}\",\"to\":\"changed\"}},{{\"kind\":\"rename_declaration\",\"path\":\"b/second.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.second.main\",\"from\":\"main\",\"to\":\"entry\"}}]}}\n"
            );
            assert_eq!(code(prepare_owned(&proposal, make())), "SPX-G197");
        }
    }

    #[test]
    fn candidate_namespace_checks_include_automatic_declarations() {
        let first = canonical(
            "a/first.spx",
            "module ops.first; fn occupied()->i64{1} @id(\"ops.first.selected\") fn selected()->i64{2}",
        );
        let second = canonical(
            "b/second.spx",
            "module ops.second; record Occupied { value:i64, } @id(\"ops.second.selected\") record Selected { @id(\"ops.second.selected.value\") value:i64, } @id(\"ops.second.main\") fn main()->i64{0}",
        );
        let paths = vec!["a/first.spx".to_owned(), "b/second.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: first,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: second,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.second\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/first.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.first.selected\",\"from\":\"selected\",\"to\":\"occupied\"}},{{\"kind\":\"rename_declaration\",\"path\":\"b/second.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.second.selected\",\"from\":\"Selected\",\"to\":\"Occupied\"}}]}}\n",
            base.workspace_revision(),
        );
        let diagnostics = prepare_owned(&proposal, base).err().unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G198");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations candidate name namespace conflicts"
        );
    }

    fn dense_fixture() -> (semantic_workspace::SemanticWorkspacePreflight, String) {
        let mut provider = String::from("module ops.dense;\n");
        let mut consumer = String::from("module ops.late;\n");
        let mut operations = Vec::new();
        for index in 0..32 {
            provider.push_str(&format!(
                "@id(\"ops.a{index:02}\") fn f{index:02}()->i64{{{index}}}\n"
            ));
            consumer.push_str(&format!(
                "use function @id(\"ops.a{index:02}\") from ops.dense as f{index:02};\n"
            ));
            operations.push(format!(
                "{{\"kind\":\"rename_declaration\",\"path\":\"a/dense.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.a{index:02}\",\"from\":\"f{index:02}\",\"to\":\"g{index:02}\"}}"
            ));
        }
        provider.push_str("@id(\"ops.dense.main\") fn main()->i64{0}\n");
        for index in 0..32 {
            consumer.push_str(&format!(
                "@id(\"ops.b.caller{index:02}\") fn caller{index:02}()->i64{{f{index:02}()}}\n"
            ));
            operations.push(format!(
                "{{\"kind\":\"rename_import_alias\",\"path\":\"b/late.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.a{index:02}\",\"target_module\":\"ops.dense\",\"from\":\"f{index:02}\",\"to\":\"g{index:02}\"}}"
            ));
        }
        consumer.push_str("@id(\"ops.late.main\") fn main()->i64{0}\n");
        let paths = vec!["a/dense.spx".to_owned(), "b/late.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: canonical(&paths[0], &provider),
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: canonical(&paths[1], &consumer),
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.late\",\"operations\":[{}]}}\n",
            preflight.workspace_revision(),
            operations.join(",")
        );
        (preflight, proposal)
    }

    #[test]
    fn dense_sixty_four_operation_late_edits_are_one_pass_and_exactly_bounded() {
        let (base, proposal) = dense_fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        assert_eq!(prepared.operations_len(), MAX_OPERATIONS);
        assert!(prepared.edits().len() > MAX_OPERATIONS);
        let late = prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path == "b/late.spx")
            .unwrap();
        assert!(late.source.contains("as g31"));
        assert!(late.source.contains("g31()"));
        assert!((0..32).all(|index| late.source.contains(&format!("g{index:02}()"))));
        assert_eq!(
            prepared
                .base_graph()
                .edges()
                .iter()
                .filter(|edge| edge.kind() == "call")
                .count(),
            32
        );
        assert_eq!(
            prepared
                .candidate_graph()
                .edges()
                .iter()
                .filter(|edge| edge.kind() == "call")
                .count(),
            32
        );
        let used = prepared.used_operations_builder_bytes();
        let (base, proposal) = dense_fixture();
        assert_eq!(
            prepare_owned_with_limit(&proposal, base, used - 1)
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds operations_builder_bytes maximum 67108864"
        );
        let (base, proposal) = dense_fixture();
        assert!(prepare_owned_with_limit(&proposal, base, used).is_ok());
    }

    #[test]
    fn derived_changed_source_cap_helper_is_exact_and_rejects_one_over() {
        reset_candidate_preflight_entry_count();
        let (base, proposal) = fixture();
        assert!(prepare_owned(&proposal, base).is_ok());
        assert_eq!(candidate_preflight_entry_count(), 1);

        reset_candidate_preflight_entry_count();
        let original = "a".repeat(MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH);
        let exact_edit = PlannedEditFact {
            path: "a/large.spx".to_owned(),
            start: original.len() - 1,
            end: original.len(),
            replacement: "z".to_owned(),
            operation_index: 0,
        };
        let rendered = render_candidate_source(&original, &[&exact_edit]).unwrap();
        assert_eq!(rendered.len(), MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH);
        assert!(validate_replacement_source_per_path(&rendered).is_ok());
        assert_eq!(candidate_preflight_entry_count(), 0);

        let one_over_edit = PlannedEditFact {
            path: "a/large.spx".to_owned(),
            start: original.len() - 1,
            end: original.len(),
            replacement: "zz".to_owned(),
            operation_index: 0,
        };
        let rendered = render_candidate_source(&original, &[&one_over_edit]).unwrap();
        assert_eq!(rendered.len(), MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH + 1);
        let diagnostics = validate_replacement_source_per_path(&rendered)
            .err()
            .unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G199");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations exceeds replacement_source_bytes_per_path maximum 1048576"
        );
        assert_eq!(candidate_preflight_entry_count(), 0);
    }

    #[test]
    fn candidate_replay_rejects_unrelated_semantic_mutation() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        let mut candidate_sources = prepared
            .candidate_sources()
            .iter()
            .map(|source| semantic_workspace::SemanticWorkspaceSource {
                path: source.path.clone(),
                source: source.source.clone(),
            })
            .collect::<Vec<_>>();
        let consumer = candidate_sources
            .iter_mut()
            .find(|source| source.path == "b/consumer.spx")
            .unwrap();
        let prior = consumer.source.clone();
        assert!(consumer.source.contains("@id(\"ops.main\")"));
        consumer.source =
            consumer
                .source
                .replacen("@id(\"ops.main\")", "@id(\"ops.tampered.main\")", 1);
        assert_ne!(consumer.source, prior);

        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let candidate = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            candidate_sources,
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let (_, _, _, candidate_build) = candidate.into_snapshot_parts();
        let candidate_view = candidate_build.into_operation_view().unwrap();

        let (fresh, _) = fixture();
        let (_, _, _, base_build) = fresh.into_snapshot_parts();
        let base_view = base_build.into_operation_view().unwrap();
        let operations = parse_proposal(&proposal).unwrap().operations;
        assert_eq!(
            replay_candidate(&base_view, &candidate_view, &operations)
                .err()
                .unwrap()[0]
                .code,
            "SPX-G200"
        );
    }

    #[test]
    fn operations_builder_limit_is_exact_and_cannot_exceed_production() {
        let (base, proposal) = fixture();
        let used = prepare_owned(&proposal, base)
            .unwrap()
            .used_operations_builder_bytes();
        let (base, proposal) = fixture();
        assert_eq!(
            prepare_owned_with_limit(&proposal, base, used - 1)
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds operations_builder_bytes maximum 67108864"
        );
        let (base, proposal) = fixture();
        assert!(prepare_owned_with_limit(&proposal, base, used).is_ok());
    }

    #[test]
    #[should_panic(expected = "cannot exceed the production maximum")]
    fn operations_builder_test_seam_cannot_expand_authority() {
        let (base, proposal) = fixture();
        let _ = prepare_owned_with_limit(&proposal, base, MAX_OPERATIONS_BUILDER_BYTES + 1);
    }

    #[test]
    fn dense_late_edits_stream_once_and_fail_before_tiny_capacity() {
        let original = "a".repeat(20_000);
        let facts = (0..10_000)
            .map(|index| PlannedEditFact {
                path: "a/dense.spx".to_owned(),
                start: index * 2 + 1,
                end: index * 2 + 2,
                replacement: "expanded".to_owned(),
                operation_index: 0,
            })
            .collect::<Vec<_>>();
        let edits = facts.iter().collect::<Vec<_>>();
        let expected_len = 20_000 + 10_000 * 7;
        let (rendered, overflowed, used) =
            crate::bounded_output::with_limit_usage(expected_len, || {
                render_candidate_source(&original, &edits)
            });
        assert!(!overflowed);
        assert_eq!(used, expected_len);
        assert_eq!(rendered.unwrap().len(), expected_len);
        let (rejected, overflowed, _) =
            crate::bounded_output::with_limit_usage(expected_len - 1, || {
                render_candidate_source(&original, &edits)
            });
        assert!(overflowed);
        assert_eq!(rejected.err().unwrap()[0].code, "SPX-G199");
    }
}
