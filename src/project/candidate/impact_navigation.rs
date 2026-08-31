//! Compact candidate-bound pages over the existing Project impact artifact.
//! Every request recomputes one exact immutable artifact and retains nothing.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceImpactOptions, PROJECT_IMPACT_SCHEMA,
};

use super::ProjectCandidate;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA: &str =
    "semaprax.project-candidate-impact-summary.v1";
pub const PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA: &str = "semaprax.project-candidate-impact-page.v1";
pub const PROJECT_CANDIDATE_IMPACT_ITEM_SCHEMA: &str = "semaprax.project-candidate-impact-item.v1";
pub const MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES: usize = 1024 * 1024;

const MAX_CURSOR_BYTES: usize = 128;
const MAX_CURSOR_OFFSET: usize = 65_536;
const NONCLAIMS: [&str; 7] = [
    "not_a_candidate_semantic_delta_or_behavioral_change",
    "potential_reverse_dependencies_over_the_existing_six_edge_families_only",
    "not_runtime_liveness_test_coverage_or_external_consumer_compatibility",
    "no_repair_ranking_or_intent_correctness",
    "no_persistent_image_index_or_candidate_retention",
    "bounded_or_truncated_inventory_is_not_complete_impact",
    "no_source_execution_or_publication_authority",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateImpactView {
    Affected,
    DependencyEdges,
    Frontier,
}

impl CandidateImpactView {
    pub const ALL: [Self; 3] = [Self::Affected, Self::DependencyEdges, Self::Frontier];

    pub fn name(self) -> &'static str {
        match self {
            Self::Affected => "affected",
            Self::DependencyEdges => "dependency_edges",
            Self::Frontier => "frontier",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "affected" => Ok(Self::Affected),
            "dependency_edges" => Ok(Self::DependencyEdges),
            "frontier" => Ok(Self::Frontier),
            _ => Err(invalid("candidate impact view is unsupported")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateImpactPageOptions {
    page_size: usize,
    max_bytes: usize,
}

impl CandidateImpactPageOptions {
    pub fn new(page_size: usize, max_bytes: usize) -> Result<Self> {
        if !(1..=128).contains(&page_size)
            || !(1024..=MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES).contains(&max_bytes)
        {
            return Err(invalid(
                "candidate impact page options require 1..128 items and 1024..1048576 bytes",
            ));
        }
        Ok(Self {
            page_size,
            max_bytes,
        })
    }

    pub fn page_size(self) -> usize {
        self.page_size
    }

    pub fn max_bytes(self) -> usize {
        self.max_bytes
    }
}

impl Default for CandidateImpactPageOptions {
    fn default() -> Self {
        Self {
            page_size: 32,
            max_bytes: 65_536,
        }
    }
}

impl ProjectCandidate {
    /// Return candidate-bound references to the exact existing impact arrays.
    pub fn impact_summary(
        &self,
        expected_candidate: &str,
        target: &str,
        options: WorkspaceImpactOptions,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let artifact = self.impact_artifact(target, options)?;
        let owner = ImpactOwner::new(self, target, &artifact)?;
        let facets = CandidateImpactView::ALL
            .iter()
            .map(|view| {
                Ok(json!({
                    "view": view.name(),
                    "handle": owner.handle(self.candidate_digest(), target, *view)?,
                    "total_items": owner.items(*view)?.len(),
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        render(
            json!({
                "schema":PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA,
                "candidate_revision":self.candidate_digest(),
                "base_project_revision":self.base.project_revision(),
                "project_schema":owner.project_schema,"project":owner.project,
                "project_revision":self.revision.project_revision(),
                "workspace_revision":self.revision.workspace_revision(),
                "project_graph_digest":self.revision.semantic_graph_digest(),
                "target":owner.target,"artifact_digest":owner.artifact_digest,
                "query":owner.query,"truncation":owner.truncation,"budget":owner.budget,
                "facets":facets,"source_authority":false,"execution":false,
                "publication_authority":false,"candidate_retained":false,
                "nonclaims":NONCLAIMS,
            }),
            MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES,
        )
    }

    /// Expand one exact impact array without changing its compiler order.
    #[allow(clippy::too_many_arguments)]
    pub fn impact_page(
        &self,
        expected_candidate: &str,
        target: &str,
        impact_options: WorkspaceImpactOptions,
        view: CandidateImpactView,
        expected_handle: &str,
        cursor: Option<&str>,
        page_options: CandidateImpactPageOptions,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let artifact = self.impact_artifact(target, impact_options)?;
        let owner = ImpactOwner::new(self, target, &artifact)?;
        let actual_handle = owner.handle(self.candidate_digest(), target, view)?;
        if expected_handle.len() != 71 || expected_handle != actual_handle {
            return Err(reference(
                "candidate impact handle does not match its candidate, target, artifact, query and view",
            ));
        }
        let items = owner.items(view)?;
        let offset = cursor
            .map(|cursor| parse_cursor(cursor, &actual_handle, page_options))
            .transpose()?
            .unwrap_or(0);
        if cursor.is_some() && offset >= items.len() {
            return Err(reference(
                "candidate impact cursor is outside its selected inventory",
            ));
        }
        let end = offset
            .saturating_add(page_options.page_size)
            .min(items.len());
        let page = page_items(&items[offset..end])?;
        let next_cursor =
            (end < items.len()).then(|| make_cursor(end, &actual_handle, page_options));
        render(
            json!({
                "schema":PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA,
                "candidate_revision":self.candidate_digest(),
                "base_project_revision":self.base.project_revision(),
                "project_schema":owner.project_schema,"project":owner.project,
                "project_revision":self.revision.project_revision(),
                "workspace_revision":self.revision.workspace_revision(),
                "project_graph_digest":self.revision.semantic_graph_digest(),
                "target":owner.target,"artifact_digest":owner.artifact_digest,
                "query":owner.query,"truncation":owner.truncation,"budget":owner.budget,
                "view":view.name(),"handle":actual_handle,"cursor":cursor,
                "offset":offset,"total_items":items.len(),
                "page_size":page_options.page_size,"max_bytes":page_options.max_bytes,
                "next_cursor":next_cursor,"items":page,
                "source_authority":false,"execution":false,
                "publication_authority":false,"candidate_retained":false,
                "nonclaims":NONCLAIMS,
            }),
            page_options.max_bytes,
        )
    }

    fn impact_artifact(&self, target: &str, options: WorkspaceImpactOptions) -> Result<Value> {
        let report = self
            .revision
            .semantic_impact(WorkspaceAnalysisTargetKind::Declaration, target, options)
            .map_err(|diagnostics| {
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "SPX-G178")
                {
                    capacity("candidate impact artifact exceeds its compiler-owned bounds")
                } else {
                    invalid("candidate impact target or compiler artifact is invalid")
                }
            })?;
        serde_json::from_str(&report)
            .map_err(|_| invalid("candidate impact artifact is not compiler JSON"))
    }
}

fn page_items(items: &[Value]) -> Result<Vec<Value>> {
    items
        .iter()
        .map(|item| {
            let item = item
                .as_object()
                .map(|item| Value::Object(item.clone()))
                .ok_or_else(|| invalid("candidate impact inventory item is not an object"))?;
            Ok(json!({
                "schema": PROJECT_CANDIDATE_IMPACT_ITEM_SCHEMA,
                "value": item,
            }))
        })
        .collect()
}

struct ImpactOwner<'a> {
    artifact: &'a Map<String, Value>,
    project_schema: &'a str,
    project: &'a str,
    artifact_digest: &'a str,
    target: &'a Value,
    query: &'a Value,
    truncation: &'a Value,
    budget: &'a Value,
}

impl<'a> ImpactOwner<'a> {
    fn new(candidate: &ProjectCandidate, target: &str, artifact: &'a Value) -> Result<Self> {
        let artifact = artifact
            .as_object()
            .ok_or_else(|| invalid("candidate impact artifact is not a compiler object"))?;
        if artifact.get("schema").and_then(Value::as_str) != Some(PROJECT_IMPACT_SCHEMA)
            || artifact.get("project_schema").and_then(Value::as_str)
                != Some(candidate.revision.manifest().schema())
            || artifact.get("project").and_then(Value::as_str)
                != Some(candidate.revision.manifest().name())
            || artifact.get("project_revision").and_then(Value::as_str)
                != Some(candidate.revision.project_revision())
            || artifact.get("workspace_revision").and_then(Value::as_str)
                != Some(candidate.revision.workspace_revision())
            || artifact.get("project_graph_digest").and_then(Value::as_str)
                != Some(candidate.revision.semantic_graph_digest())
            || artifact
                .get("target")
                .and_then(Value::as_object)
                .and_then(|target| target.get("kind"))
                .and_then(Value::as_str)
                != Some("declaration")
            || artifact
                .get("target")
                .and_then(Value::as_object)
                .and_then(|target| target.get("id"))
                .and_then(Value::as_str)
                != Some(target)
        {
            return Err(invalid(
                "candidate impact artifact bindings disagree with the admitted candidate",
            ));
        }
        let project_schema = text_field(artifact, "project_schema")?;
        let project = text_field(artifact, "project")?;
        let artifact_digest = digest_field(artifact, "artifact_digest")?;
        let target = object_field(artifact, "target")?;
        let query = object_field(artifact, "query")?;
        if query.get("direction").and_then(Value::as_str) != Some("reverse")
            || query.get("depth").and_then(Value::as_u64).is_none()
            || query.get("max_bytes").and_then(Value::as_u64).is_none()
            || query.get("max_nodes").and_then(Value::as_u64).is_none()
        {
            return Err(invalid("candidate impact artifact query is invalid"));
        }
        let truncation = object_field(artifact, "truncation")?;
        let budget = object_field(artifact, "budget")?;
        for view in CandidateImpactView::ALL {
            let _ = array_field(artifact, view.name())?;
        }
        Ok(Self {
            artifact,
            project_schema,
            project,
            artifact_digest,
            target,
            query,
            truncation,
            budget,
        })
    }

    fn items(&self, view: CandidateImpactView) -> Result<&'a [Value]> {
        array_field(self.artifact, view.name())
    }

    fn handle(&self, candidate: &str, target: &str, view: CandidateImpactView) -> Result<String> {
        let query_number = |field| {
            self.query
                .get(field)
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .ok_or_else(|| invalid("candidate impact artifact query is invalid"))
        };
        let depth = query_number("depth")?;
        let max_bytes = query_number("max_bytes")?;
        let max_nodes = query_number("max_nodes")?;
        Ok(framed_digest(
            b"semaprax.project-candidate-impact-handle.v1\0",
            &[
                candidate,
                target,
                self.artifact_digest,
                &depth,
                &max_bytes,
                &max_nodes,
                view.name(),
            ],
        ))
    }
}

fn object_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a Value> {
    object
        .get(field)
        .filter(|value| value.is_object())
        .ok_or_else(|| invalid("candidate impact artifact object is absent"))
}

fn text_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("candidate impact artifact text binding is absent"))
}

fn array_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a [Value]> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("candidate impact artifact inventory is absent"))
}

fn digest_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("candidate impact artifact digest is absent"))?;
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("candidate impact artifact digest is invalid"));
    }
    Ok(value)
}

fn make_cursor(offset: usize, handle: &str, options: CandidateImpactPageOptions) -> String {
    let offset = offset.to_string();
    let digest = framed_digest(
        b"semaprax.project-candidate-impact-cursor.v1\0",
        &[
            handle,
            &offset,
            &options.page_size.to_string(),
            &options.max_bytes.to_string(),
        ],
    );
    format!("{offset}:{digest}")
}

fn parse_cursor(cursor: &str, handle: &str, options: CandidateImpactPageOptions) -> Result<usize> {
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(reference("candidate impact cursor exceeds its bound"));
    }
    let (number, _) = cursor
        .split_once(':')
        .ok_or_else(|| reference("candidate impact cursor is malformed"))?;
    let offset = number
        .parse::<usize>()
        .map_err(|_| reference("candidate impact cursor offset is invalid"))?;
    if offset == 0
        || offset > MAX_CURSOR_OFFSET
        || offset % options.page_size != 0
        || offset.to_string() != number
        || make_cursor(offset, handle, options) != cursor
    {
        return Err(reference(
            "candidate impact cursor does not match its handle and page options",
        ));
    }
    Ok(offset)
}

fn framed_digest(domain: &[u8], values: &[&str]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for value in values {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn render(value: Value, max_bytes: usize) -> Result<String> {
    super::super::image::render(value, false, max_bytes)
        .map_err(|_| capacity("candidate impact navigation output exceeds its byte bound"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G333", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G334", message)]
}

fn reference(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G335", message)]
}
