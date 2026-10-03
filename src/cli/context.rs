//! Project selection for the bounded `context` command.

use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::{project, workspace_analysis};
use serde_json::{json, Value};

use super::super::options::{project_context_options, ParsedContextOptions};
use super::project::is_project_manifest;

const SCHEMA_V1: &str = "semaprax.project-agent-context.v1";

pub(crate) fn split_rust_index_option(
    args: &[String],
) -> Result<(Vec<String>, Option<String>), u8> {
    let mut plain = args.get(..3).unwrap_or(args).to_vec();
    let mut index_path = None;
    let mut cursor = 3;
    while cursor < args.len() {
        let option = &args[cursor];
        let value = args.get(cursor + 1).ok_or_else(|| {
            eprintln!("context option `{option}` requires a value");
            2
        })?;
        if option == "--rust-index" {
            if index_path.replace(value.clone()).is_some()
                || value.is_empty()
                || value.starts_with('-')
            {
                eprintln!("context --rust-index requires one nonempty file path");
                return Err(2);
            }
        } else {
            plain.extend([option.clone(), value.clone()]);
        }
        cursor += 2;
    }
    Ok((plain, index_path))
}

fn read_index(path: &Path) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let file = std::fs::File::open(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!(
                "cannot read prepared Rust index `{}`: {error}",
                path.display()
            ),
        )]
    })?;
    let mut bytes = Vec::new();
    file.take(semaprax_rust_api_index::MAX_INDEX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I001",
                format!(
                    "cannot read prepared Rust index `{}`: {error}",
                    path.display()
                ),
            )]
        })?;
    if bytes.len() > semaprax_rust_api_index::MAX_INDEX_BYTES {
        return Err(vec![Diagnostic::io(
            "SPX-B148",
            "prepared Rust API index exceeds its byte bound",
        )]);
    }
    Ok(bytes)
}

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

/// Render Project context or selected Rust import status, returning `None` for
/// source inputs handled by the ordinary verified context path.
pub(crate) fn project(
    path: &Path,
    symbol: &str,
    arguments: &[String],
    options: &ParsedContextOptions,
    rust_index: Option<&Path>,
    report: impl Fn(&[Diagnostic]) -> u8,
) -> Result<Option<String>, u8> {
    if !is_project_manifest(path) {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                return Err(report(&[Diagnostic::io(
                    "SPX-I001",
                    format!("cannot read `{}`: {error}", path.display()),
                )]));
            }
        };
        let program = match semaprax::parse(&source, path) {
            Ok(program) => program,
            // Let the ordinary checked route report parse failures once.
            Err(_) => return Ok(None),
        };
        let projection = if let Some(index_path) = rust_index {
            let bytes = read_index(index_path).map_err(|errors| report(&errors))?;
            semaprax::rust_api_context::prepared_selected_rust_import_context_json(
                &program,
                symbol,
                &bytes,
                options.max_bytes(),
            )
        } else {
            semaprax::rust_api_context::selected_rust_import_context_json(
                &program,
                symbol,
                options.max_bytes(),
            )
        }
        .map_err(|errors| report(&errors))?;
        if projection.is_some()
            && arguments.iter().any(|argument| {
                matches!(
                    argument.as_str(),
                    "--depth" | "--max-nodes" | "--filters" | "--direction"
                )
            })
        {
            eprintln!("selected Rust import context does not accept graph traversal options");
            return Err(2);
        }
        if projection.is_some() {
            return Ok(projection);
        }
        if rust_index.is_some() {
            eprintln!(
                "context --rust-index requires a declared selected Rust import identity or path"
            );
            return Err(2);
        }
        return Ok(None);
    }
    if rust_index.is_some() {
        eprintln!("context --rust-index requires one .spx source file");
        return Err(2);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_cli_projects_selected_import_setup_without_tools() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "semaprax-selected-context-{}-{nonce}.spx",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"module test.context_cli;

@id("rust.host")
interface RustHost permits { regex.read } {
    @id("rust.host.is_match")
    import rust selected fn is_match from "regex::Regex::is_match"
        effects { regex.read }
        failure infallible;
}
@id("rust.host.main") fn main() -> i64 { 0 }
"#,
        )
        .unwrap();
        let options = ParsedContextOptions::V1(semaprax::graph::AgentContextOptions::default());
        let output = project(&path, "rust.host.is_match", &[], &options, None, |_| 1)
            .unwrap()
            .expect("selected import gets setup status before ordinary verification");
        let value: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["index"]["status"], "index_unprepared");
        assert_eq!(value["authority"]["tool_invocation"], false);
        assert!(output.len() <= 4096);
        let index = semaprax_rust_api_index::RustApiIndex::admit_extractor_output(include_bytes!(
            "../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
        ))
        .unwrap();
        let index_path = path.with_extension("index.json");
        std::fs::write(&index_path, index.canonical_json()).unwrap();
        let prepared = project(
            &path,
            "regex::Regex::is_match",
            &[],
            &options,
            Some(&index_path),
            |_| 1,
        )
        .unwrap()
        .unwrap();
        let value: Value = serde_json::from_str(&prepared).unwrap();
        assert_eq!(value["index"]["status"], "prepared_metadata");
        assert_eq!(value["selected_import"]["support"], "supported");
        assert_eq!(value["package"]["cargo_alias"], "regex_alias");
        assert_eq!(value["authority"]["tool_invocation"], false);
        assert!(prepared.len() <= 4096);
        assert_eq!(
            project(
                &path,
                "rust.host.main",
                &[],
                &options,
                Some(&index_path),
                |_| 1
            )
            .unwrap_err(),
            2
        );
        std::fs::remove_file(index_path).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rust_index_option_is_unique_and_leaves_graph_options_for_the_existing_parser() {
        let args = [
            "context",
            "file.spx",
            "rust.host.method",
            "--max-bytes",
            "4096",
            "--rust-index",
            "index.json",
        ]
        .map(str::to_owned);
        let (plain, index) = split_rust_index_option(&args).unwrap();
        assert_eq!(
            plain,
            [
                "context",
                "file.spx",
                "rust.host.method",
                "--max-bytes",
                "4096"
            ]
        );
        assert_eq!(index.as_deref(), Some("index.json"));
        let duplicate = [
            "context",
            "file.spx",
            "id",
            "--rust-index",
            "one",
            "--rust-index",
            "two",
        ]
        .map(str::to_owned);
        assert_eq!(split_rust_index_option(&duplicate).unwrap_err(), 2);
    }
}
