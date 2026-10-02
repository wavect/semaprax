//! Bounded explorer projection over held, checked Project revisions.
//!
//! This adapter only reads compiler-owned `ProjectRevision` data. It never
//! accepts graph JSON as input and does not retain a new image or grant source,
//! execution, or publication authority.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
    WorkspaceImpactOptions,
};

use super::ProjectRevision;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const EXPLORER_VIEW_SCHEMA: &str = "semaprax.explorer-view.v1";
pub const MAX_EXPLORER_SUMMARY_BYTES: usize = 64 * 1024;
pub const MAX_EXPLORER_PAGE_BYTES: usize = 512 * 1024;
const HANDLE_DOMAIN: &[u8] = b"semaprax.explorer-view-handle.v1\0";
const CURSOR_DOMAIN: &[u8] = b"semaprax.explorer-view-cursor.v1\0";
const NONCLAIMS: [&str; 6] = [
    "no_source_or_publication_authority",
    "no_execution_or_test_authority",
    "not_a_runtime_trace_or_coverage_claim",
    "not_an_untrusted_graph_json_import",
    "unknown_dynamic_and_external_relations_are_not_proven_absent",
    "ownership_loans_and_cleanup_are_separate_function_facets",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerMode {
    Overview,
    Context,
    Impact,
}
impl ExplorerMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "overview" => Ok(Self::Overview),
            "context" => Ok(Self::Context),
            "impact" => Ok(Self::Impact),
            _ => Err(invalid("explorer mode is unsupported")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Context => "context",
            Self::Impact => "impact",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerSide {
    Current,
    Base,
    Candidate,
}
impl ExplorerSide {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "current" => Ok(Self::Current),
            "base" => Ok(Self::Base),
            "candidate" => Ok(Self::Candidate),
            _ => Err(invalid("explorer side is unsupported")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Base => "base",
            Self::Candidate => "candidate",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerDirection {
    Forward,
    Reverse,
    Both,
}
impl ExplorerDirection {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "forward" => Ok(Self::Forward),
            "reverse" => Ok(Self::Reverse),
            "both" => Ok(Self::Both),
            _ => Err(invalid("explorer context direction is unsupported")),
        }
    }
    fn analysis(self) -> WorkspaceAnalysisDirection {
        match self {
            Self::Forward => WorkspaceAnalysisDirection::Forward,
            Self::Reverse => WorkspaceAnalysisDirection::Reverse,
            Self::Both => WorkspaceAnalysisDirection::Both,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
            Self::Both => "both",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerView {
    Modules,
    Declarations,
    Relations,
    Frontier,
}
impl ExplorerView {
    pub const ALL: [Self; 4] = [
        Self::Modules,
        Self::Declarations,
        Self::Relations,
        Self::Frontier,
    ];
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "modules" => Ok(Self::Modules),
            "declarations" => Ok(Self::Declarations),
            "relations" => Ok(Self::Relations),
            "frontier" => Ok(Self::Frontier),
            _ => Err(invalid("explorer view is unsupported")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Modules => "modules",
            Self::Declarations => "declarations",
            Self::Relations => "relations",
            Self::Frontier => "frontier",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExplorerQuery {
    pub direction: ExplorerDirection,
    pub depth: usize,
    pub max_nodes: usize,
    pub max_bytes: usize,
}
impl ExplorerQuery {
    pub fn new(
        direction: ExplorerDirection,
        depth: usize,
        max_nodes: usize,
        max_bytes: usize,
    ) -> Result<Self> {
        if depth > 16
            || !(1..=256).contains(&max_nodes)
            || !(1024..=256 * 1024).contains(&max_bytes)
        {
            return Err(invalid("explorer query exceeds its fixed bounds"));
        }
        Ok(Self {
            direction,
            depth,
            max_nodes,
            max_bytes,
        })
    }
}
impl Default for ExplorerQuery {
    fn default() -> Self {
        Self {
            direction: ExplorerDirection::Both,
            depth: 1,
            max_nodes: 256,
            max_bytes: 256 * 1024,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExplorerPageOptions {
    page_size: usize,
    max_bytes: usize,
}
impl ExplorerPageOptions {
    pub fn new(page_size: usize, max_bytes: usize) -> Result<Self> {
        if !(1..=128).contains(&page_size) || !(1024..=MAX_EXPLORER_PAGE_BYTES).contains(&max_bytes)
        {
            return Err(invalid(
                "explorer page options require 1..128 items and 1024..524288 bytes",
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
impl Default for ExplorerPageOptions {
    fn default() -> Self {
        Self {
            page_size: 32,
            max_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ExplorerSubject<'a> {
    pub image_digest: &'a str,
    pub candidate_digest: Option<&'a str>,
    pub side: ExplorerSide,
    pub revision: &'a ProjectRevision,
}

pub(crate) fn summary(
    subject: ExplorerSubject<'_>,
    mode: ExplorerMode,
    target: Option<&str>,
    query: ExplorerQuery,
) -> Result<String> {
    let artifact = artifact(&subject, mode, target, query)?;
    let inventories = ExplorerView::ALL.iter().map(|view| json!({"view":view.name(),"handle":handle(&subject,mode,target,query,*view),"total_items":items(&artifact,*view).len()})).collect::<Vec<_>>();
    render(
        json!({"schema":EXPLORER_VIEW_SCHEMA,"kind":"summary","subject":subject_json(&subject),"mode":mode.name(),"target":target,"query":query_json(query),"artifact_digest":digest(&artifact),"truncation":artifact["truncation"].clone(),"coverage":artifact["coverage"].clone(),"inventories":inventories,"source_authority":false,"execution":false,"publication_authority":false,"nonclaims":NONCLAIMS}),
        MAX_EXPLORER_SUMMARY_BYTES,
    )
}

pub(crate) fn page(
    subject: ExplorerSubject<'_>,
    mode: ExplorerMode,
    target: Option<&str>,
    query: ExplorerQuery,
    view: ExplorerView,
    expected_handle: &str,
    cursor: Option<&str>,
    options: ExplorerPageOptions,
) -> Result<String> {
    let actual = handle(&subject, mode, target, query, view);
    if expected_handle != actual {
        return Err(reference(
            "explorer handle does not bind this exact subject, side, mode, target and query",
        ));
    }
    let artifact = artifact(&subject, mode, target, query)?;
    let rows = items(&artifact, view);
    let offset = cursor
        .map(|value| parse_cursor(value, &actual, options))
        .transpose()?
        .unwrap_or(0);
    if cursor.is_some() && offset >= rows.len() {
        return Err(reference(
            "explorer cursor is outside its selected inventory",
        ));
    }
    let mut accepted = Vec::new();
    for row in rows.iter().skip(offset).take(options.page_size) {
        let mut next = accepted.clone();
        next.push(row.clone());
        let probe = json!({"schema":EXPLORER_VIEW_SCHEMA,"kind":"page","subject":subject_json(&subject),"mode":mode.name(),"target":target,"query":query_json(query),"artifact_digest":digest(&artifact),"view":view.name(),"handle":actual,"cursor":cursor,"offset":offset,"total_items":rows.len(),"page_size":options.page_size,"max_bytes":options.max_bytes,"next_cursor":null,"items":next,"source_authority":false,"execution":false,"publication_authority":false});
        if render(probe, options.max_bytes).is_err() {
            if accepted.is_empty() {
                return Err(capacity(
                    "the first explorer row cannot fit one complete page",
                ));
            }
            break;
        }
        accepted.push(row.clone());
    }
    let end = offset + accepted.len();
    let next = (end < rows.len()).then(|| make_cursor(end, &actual, options));
    render(
        json!({"schema":EXPLORER_VIEW_SCHEMA,"kind":"page","subject":subject_json(&subject),"mode":mode.name(),"target":target,"query":query_json(query),"artifact_digest":digest(&artifact),"truncation":artifact["truncation"].clone(),"coverage":artifact["coverage"].clone(),"view":view.name(),"handle":actual,"cursor":cursor,"offset":offset,"total_items":rows.len(),"page_size":options.page_size,"max_bytes":options.max_bytes,"next_cursor":next,"items":accepted,"source_authority":false,"execution":false,"publication_authority":false,"nonclaims":NONCLAIMS}),
        options.max_bytes,
    )
}

fn artifact(
    subject: &ExplorerSubject<'_>,
    mode: ExplorerMode,
    target: Option<&str>,
    query: ExplorerQuery,
) -> Result<Value> {
    match mode {
        ExplorerMode::Overview => {
            if target.is_some() {
                return Err(invalid("explorer overview does not accept a target"));
            }
            overview(subject)
        }
        ExplorerMode::Context => {
            let target =
                target.ok_or_else(|| invalid("explorer context requires a declaration target"))?;
            let report = subject.revision.semantic_context(
                WorkspaceAnalysisTargetKind::Declaration,
                target,
                WorkspaceContextOptions::new(
                    query.direction.analysis(),
                    query.depth,
                    query.max_bytes,
                    query.max_nodes,
                )
                .map_err(|_| invalid("explorer context query is invalid"))?,
            )?;
            analysis_artifact(subject, report, "context")
        }
        ExplorerMode::Impact => {
            let target =
                target.ok_or_else(|| invalid("explorer impact requires a declaration target"))?;
            let report = subject.revision.semantic_impact(
                WorkspaceAnalysisTargetKind::Declaration,
                target,
                WorkspaceImpactOptions::new(query.depth, query.max_bytes, query.max_nodes)
                    .map_err(|_| invalid("explorer impact query is invalid"))?,
            )?;
            analysis_artifact(subject, report, "impact")
        }
    }
}
fn overview(subject: &ExplorerSubject<'_>) -> Result<Value> {
    let graph: Value = serde_json::from_str(subject.revision.semantic_graph())
        .map_err(|_| invalid("held semantic graph is invalid"))?;
    let source_modules = graph["modules"]
        .as_array()
        .ok_or_else(|| invalid("held semantic graph has no modules"))?;
    let source_declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| invalid("held semantic graph has no declarations"))?;
    let source_edges = graph["edges"]
        .as_array()
        .ok_or_else(|| invalid("held semantic graph has no edges"))?;
    let modules = source_modules.iter().map(|module| {
        let name = module["module"].as_str().ok_or_else(|| invalid("held module row has no name"))?;
        let path = module["path"].as_str().ok_or_else(|| invalid("held module row has no path"))?;
        let declaration_count = source_declarations.iter().filter(|row| row["module"] == name).count();
        let relation_count = source_edges.iter().filter(|row| row["caller_path"] == path || row["target_path"] == path).count();
        Ok(json!({"module":name,"path":path,"declaration_count":declaration_count,"relation_count":relation_count,"source_reference":{"path":path,"source_revision":module["source_revision"],"source_digest":module["source_digest"]}}))
    }).collect::<Result<Vec<_>>>()?;
    let declarations = source_declarations
        .iter()
        .map(|row| declaration_row(subject, row))
        .collect::<Result<Vec<_>>>()?;
    let relations = source_edges
        .iter()
        .map(|row| relation_row(subject, row))
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"modules":modules,"declarations":declarations,"relations":relations,"frontier":[],"truncation":{"truncated":false,"reason":null},"coverage":{"owner":"workspace_graph","complete_within_retained_graph":true}}),
    )
}
fn analysis_artifact(_subject: &ExplorerSubject<'_>, report: String, kind: &str) -> Result<Value> {
    let value: Value =
        serde_json::from_str(&report).map_err(|_| invalid("held analysis report is invalid"))?;
    let declarations = value["nodes"]
        .as_array()
        .or_else(|| value["affected"].as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| declaration_row(_subject, &v))
        .collect::<Result<Vec<_>>>()?;
    let relations = value["path_edges"]
        .as_array()
        .or_else(|| value["dependency_edges"].as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| relation_row(_subject, &v))
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"modules":[],"declarations":declarations,"relations":relations,"frontier":value["frontier"].clone(),"truncation":value["truncation"].clone(),"coverage":{"owner":"workspace_analysis","mode":kind,"complete_within_query":value["truncation"]["truncated"].as_bool().map(|v|!v)}}),
    )
}
fn node_key(subject: &ExplorerSubject<'_>, id: &str) -> String {
    format!(
        "{}:{}:{}:{}",
        subject.revision.manifest().name(),
        subject.revision.project_revision(),
        subject.side.name(),
        id
    )
}
fn declaration_row(subject: &ExplorerSubject<'_>, value: &Value) -> Result<Value> {
    let id = value
        .get("id")
        .or_else(|| value.get("node").and_then(|v| v.get("id")))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("held declaration row has no identity"))?;
    let path = value.get("path").cloned().unwrap_or(Value::Null);
    let source_reference = declaration_source_reference(subject, value, id, &path);
    Ok(
        json!({"node_key":node_key(subject,id),"id":id,"identity_origin":value.get("identity_origin").cloned().unwrap_or_else(||Value::String("unknown_in_analysis_projection".into())),"kind":value.get("kind").cloned().unwrap_or_else(||Value::String("declaration".into())),"display_name":value.get("name").or_else(||value.get("display_name")).cloned().unwrap_or_else(||Value::String(id.rsplit('.').next().unwrap_or(id).to_owned())),"owner_id":value.get("owner").cloned().unwrap_or(Value::Null),"module":value.get("module").cloned().unwrap_or(Value::Null),"path":path,"source_reference":source_reference}),
    )
}
fn declaration_source_reference(
    subject: &ExplorerSubject<'_>,
    value: &Value,
    id: &str,
    path: &Value,
) -> Value {
    let Some(path) = path.as_str() else {
        return json!({"kind":"non_file_node"});
    };
    let Some(module_name) = value.get("module").and_then(Value::as_str) else {
        return json!({"kind":"authenticated_source_reference_unavailable_in_analysis_projection"});
    };
    let mut modules = subject
        .revision
        .semantic
        .image_modules()
        .iter()
        .filter(|module| module.path() == path && module.module() == module_name);
    let Some(module) = modules.next() else {
        return json!({"kind":"authenticated_source_reference_unavailable_in_analysis_projection"});
    };
    if modules.next().is_some() || !source_binding_matches(value.get("source_binding"), module) {
        return json!({"kind":"authenticated_source_reference_unavailable_in_analysis_projection"});
    }
    let Some(span) = declaration_span(module, id) else {
        return json!({"kind":"authenticated_source_reference_unavailable_in_analysis_projection"});
    };
    json!({"path":module.path(),"source_revision":module.source_revision(),"source_digest":module.source_digest(),"span":{"start":span.start,"end":span.end,"line":span.line,"column":span.column}})
}
fn source_binding_matches(
    binding: Option<&Value>,
    module: &crate::workspace_graph::WorkspaceGraphProjectionModule,
) -> bool {
    let Some(binding) = binding else {
        return true;
    };
    binding.get("path").and_then(Value::as_str) == Some(module.path())
        && binding.get("source_revision").and_then(Value::as_str) == Some(module.source_revision())
        && binding.get("source_digest").and_then(Value::as_str) == Some(module.source_digest())
}
fn declaration_span(
    module: &crate::workspace_graph::WorkspaceGraphProjectionModule,
    id: &str,
) -> Option<crate::ast::Span> {
    if let Some(function) = module
        .functions()
        .iter()
        .find(|function| function.id.as_str() == id)
    {
        return Some(function.span);
    }
    if let Some(template) = module
        .function_templates()
        .iter()
        .find(|template| template.id.as_str() == id)
    {
        return Some(template.span);
    }
    for declaration in module.types() {
        if declaration.id.as_str() == id {
            return Some(declaration.span);
        }
        match &declaration.kind {
            crate::hir::ResolvedTypeDeclarationKind::Record { fields }
            | crate::hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                if let Some(field) = fields.iter().find(|field| field.id.as_str() == id) {
                    return Some(field.span);
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    if case.id.as_str() == id {
                        return Some(case.span);
                    }
                    if let Some(field) = case.fields.iter().find(|field| field.id.as_str() == id) {
                        return Some(field.span);
                    }
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Resource { .. } => {}
        }
    }
    for interface in module.interfaces() {
        if interface.id.as_str() == id {
            return Some(interface.span);
        }
        if let Some(import) = interface
            .imports
            .iter()
            .find(|import| import.id.as_str() == id)
        {
            return Some(import.span);
        }
    }
    None
}
fn relation_row(subject: &ExplorerSubject<'_>, value: &Value) -> Result<Value> {
    let family = value
        .get("kind")
        .or_else(|| value.get("family"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("held relation row has no family"))?;
    if !matches!(
        family,
        "function_import"
            | "type_import"
            | "call"
            | "type_reference"
            | "effect_requirement"
            | "capability_authority"
    ) {
        return Err(invalid(
            "held relation row uses an unsupported explorer family",
        ));
    }
    let from = value
        .get("caller")
        .or_else(|| value.get("source"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("held relation row has no source identity"))?;
    let to = value
        .get("target")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("held relation row has no target identity"))?;
    let site = value
        .get("site")
        .or_else(|| value.get("expression"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("held relation row has no site identity"))?;
    Ok(
        json!({"family":family,"from":node_key(subject,from),"to":node_key(subject,to),"direction":"forward","site_id":site,"provenance":value.clone()}),
    )
}
fn items<'a>(artifact: &'a Value, view: ExplorerView) -> &'a Vec<Value> {
    artifact[view.name()]
        .as_array()
        .expect("compiler explorer artifact inventories")
}
fn subject_json(subject: &ExplorerSubject<'_>) -> Value {
    json!({"kind":if subject.candidate_digest.is_some(){"candidate"}else{"image"},"image_revision":subject.image_digest,"project_revision":subject.revision.project_revision(),"workspace_revision":subject.revision.workspace_revision(),"project_graph_digest":subject.revision.semantic_graph_digest(),"candidate_revision":subject.candidate_digest,"side":subject.side.name()})
}
fn query_json(query: ExplorerQuery) -> Value {
    json!({"direction":query.direction.name(),"depth":query.depth,"max_nodes":query.max_nodes,"max_bytes":query.max_bytes})
}
fn digest(value: &Value) -> String {
    let mut canonical = value.clone();
    canonical.sort_all_objects();
    let mut h = Sha256::new();
    h.update(b"semaprax.explorer-view-artifact.v1\0");
    h.update(canonical.to_string().as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(h.finalize()))
}
fn handle(
    subject: &ExplorerSubject<'_>,
    mode: ExplorerMode,
    target: Option<&str>,
    query: ExplorerQuery,
    view: ExplorerView,
) -> String {
    let mut h = Sha256::new();
    h.update(HANDLE_DOMAIN);
    for s in [
        subject.image_digest,
        subject.candidate_digest.unwrap_or(""),
        subject.side.name(),
        mode.name(),
        target.unwrap_or(""),
        query.direction.name(),
        &query.depth.to_string(),
        &query.max_nodes.to_string(),
        &query.max_bytes.to_string(),
        view.name(),
    ] {
        h.update((s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(h.finalize()))
}
fn make_cursor(offset: usize, handle: &str, options: ExplorerPageOptions) -> String {
    let mut h = Sha256::new();
    h.update(CURSOR_DOMAIN);
    for s in [
        handle,
        &offset.to_string(),
        &options.page_size.to_string(),
        &options.max_bytes.to_string(),
    ] {
        h.update((s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    }
    format!(
        "{}:sha256:{:x}",
        offset,
        crate::digest_hex::LowerHex(h.finalize())
    )
}
fn parse_cursor(value: &str, handle: &str, options: ExplorerPageOptions) -> Result<usize> {
    let (offset, digest) = value
        .split_once(':')
        .ok_or_else(|| reference("explorer cursor is malformed"))?;
    let offset = offset
        .parse::<usize>()
        .map_err(|_| reference("explorer cursor offset is malformed"))?;
    if value != make_cursor(offset, handle, options) {
        return Err(reference("explorer cursor does not bind this exact page"));
    }
    if digest.is_empty() {
        return Err(reference("explorer cursor is malformed"));
    }
    Ok(offset)
}
fn render(mut value: Value, max: usize) -> Result<String> {
    value.sort_all_objects();
    let out = value.to_string();
    if out.len() > max {
        return Err(capacity("explorer response exceeds its byte bound"));
    }
    Ok(out)
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G326", message)]
}
fn reference(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G327", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G328", message)]
}
