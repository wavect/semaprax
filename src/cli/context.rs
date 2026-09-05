//! Project selection for the bounded `context` command.

use std::fmt::Write as _;
use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::{project, workspace_analysis};
use serde_json::{json, Value};

use super::super::options::{project_context_options, ParsedContextOptions};
use super::project::is_project_manifest;

const SCHEMA_V1: &str = "semaprax.project-agent-context.v1";

fn invalid_projection() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G004",
        "authenticated Project context cannot form its compact projection",
    )]
}

fn member<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Vec<Diagnostic>> {
    value
        .as_object()
        .and_then(|object| object.get(name))
        .ok_or_else(invalid_projection)
}

fn compact(full: &str, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
    let full: Value = serde_json::from_str(full).map_err(|_| invalid_projection())?;
    if member(&full, "schema")? != "semaprax.project-semantic-context.v1" {
        return Err(invalid_projection());
    }
    let target = member(&full, "target")?;
    let target = json!([
        member(target, "id")?,
        member(target, "declaration_kind")?,
        member(target, "path")?,
        member(target, "module")?
    ]);
    let query = member(&full, "query")?;
    let query = json!([
        member(query, "direction")?,
        member(query, "depth")?,
        max_bytes,
        member(query, "max_nodes")?,
        member(query, "max_bytes")?
    ]);
    let nodes = member(&full, "nodes")?
        .as_array()
        .ok_or_else(invalid_projection)?
        .iter()
        .map(|node| {
            Ok(json!([
                member(node, "id")?,
                member(node, "kind")?,
                member(node, "declaration_kind")?,
                member(node, "path")?,
                member(node, "module")?,
                member(node, "minimum_depth")?,
                member(node, "reached_by")?
            ]))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let edges = member(&full, "edges")?
        .as_array()
        .ok_or_else(invalid_projection)?
        .iter()
        .map(|edge| {
            Ok(json!([
                member(edge, "kind")?,
                member(edge, "caller")?,
                member(edge, "target")?,
                member(edge, "caller_path")?,
                member(edge, "target_path")?,
                member(edge, "site")?,
                member(edge, "expression")?
            ]))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let budget = member(&full, "budget")?;
    let budget = json!([
        member(budget, "used_nodes")?,
        member(budget, "used_edges")?,
        member(budget, "used_depth")?
    ]);
    let mut compact = String::new();
    write!(
        compact,
        "{{\"schema\":{},\"project_revision\":{},\"graph_revision\":{},\"context_revision\":{},\"target\":{target},\"query\":{query},\"nodes\":{},\"edges\":{},\"truncation\":{},\"frontier\":{},\"budget\":{budget},\"authority\":false}}",
        json!(SCHEMA_V1),
        member(&full, "project_revision")?,
        member(&full, "project_graph_digest")?,
        member(&full, "artifact_digest")?,
        Value::Array(nodes),
        Value::Array(edges),
        member(&full, "truncation")?,
        member(&full, "frontier")?,
    )
    .expect("writing to a string cannot fail");
    if compact.len() > max_bytes {
        return Err(vec![Diagnostic::io(
            "SPX-G004",
            format!(
                "Project context requires {} output bytes but max_bytes is {max_bytes}",
                compact.len()
            ),
        )]);
    }
    Ok(compact)
}

/// Render authenticated Project context, or return `None` for a source input.
pub(crate) fn project(
    path: &Path,
    symbol: &str,
    arguments: &[String],
    options: &ParsedContextOptions,
    report: impl Fn(&[Diagnostic]) -> u8,
) -> Result<Option<String>, u8> {
    if !is_project_manifest(path) {
        return Ok(None);
    }
    if arguments.iter().any(|argument| argument == "--filters") {
        eprintln!("context --filters is unavailable for Project inputs");
        return Err(2);
    }
    let max_bytes = options.max_bytes();
    let options = project_context_options(options)?;
    let full = project::with_authenticated_project(path, |snapshot| {
        snapshot.semantic_context(
            workspace_analysis::WorkspaceAnalysisTargetKind::Declaration,
            symbol,
            options,
        )
    })
    .map_err(|errors| report(&errors))?;
    compact(&full, max_bytes)
        .map(Some)
        .map_err(|errors| report(&errors))
}
