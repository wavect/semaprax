//! Pure status projection for a selected Rust import whose API index is not
//! present in the parsed source query.
//!
//! This projection reports only what the source declares and the setup needed
//! to supply the compiler-resolved index. It does not invoke Cargo, rustc,
//! rustdoc, build scripts, macros, or external tools, and it does not loosen
//! the existing Graph or agent-context refusal for ordinary Rust imports.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::ast::{ImportDeclaration, Program};
use crate::diagnostic::Diagnostic;

const SCHEMA: &str = "semaprax.rust-api-context.v1";
pub(crate) const MAX_BYTES: usize = 4096;
const SETUP: &str = "Prepare a canonical semaprax.rust-api-index.v2 envelope with the explicit pinned rustdoc JSON extractor, then supply those bytes and the exact package, Cargo alias, target, feature, and stable compiler identity to the indexed Rust binding workflow. This context request does not run tools.";

/// Return a bounded setup-status view for one selected Rust import.
///
/// `symbol` must exactly match the import's persistent ID or source name. A
/// result is emitted only when the source is in the expected unprepared state;
/// other Rust imports keep using the existing closed context projections.
pub fn selected_rust_import_context_json(
    program: &Program,
    symbol: &str,
    requested_max_bytes: usize,
) -> Result<Option<String>, Vec<Diagnostic>> {
    let selected = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.index_selected)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(None);
    }

    let matching = selected
        .iter()
        .copied()
        .filter(|import| import.stable_id == symbol || import.name == symbol)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Ok(None);
    }
    if matching.len() != 1 {
        return Err(vec![Diagnostic::error(
            "SPX-G406",
            "selected Rust import context requires one unambiguous import identity",
            matching[0].span,
        )]);
    }
    let import = matching[0];
    if import.selected_signature.is_some() || import.selected_index_digest.is_some() {
        return Ok(None);
    }

    // Allow only the expected missing-index diagnostic. This keeps malformed
    // source, invalid IDs, effect errors, and unrelated type errors visible.
    let pending_spans = selected
        .iter()
        .filter(|import| {
            import.selected_signature.is_none() && import.selected_index_digest.is_none()
        })
        .map(|import| (import.span.start, import.span.end))
        .collect::<BTreeSet<_>>();
    let diagnostics = crate::source_verify::verify(program);
    let mut pending_reports = BTreeSet::new();
    let mut other = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.code == "SPX-B147"
            && diagnostic
                .span
                .is_some_and(|span| pending_spans.contains(&(span.start, span.end)))
        {
            let span = diagnostic.span.expect("checked above");
            pending_reports.insert((span.start, span.end));
        } else {
            other.push(diagnostic);
        }
    }
    if !other.is_empty() {
        return Err(other);
    }
    if pending_reports != pending_spans {
        return Ok(None);
    }

    let output_limit = requested_max_bytes.min(MAX_BYTES);
    let full = full_envelope(program, import, output_limit);
    if full.len() <= output_limit {
        return Ok(Some(full));
    }

    let short = truncated_envelope(program, import, output_limit);
    if short.len() <= output_limit {
        Ok(Some(short))
    } else {
        Err(vec![Diagnostic::io(
            "SPX-G004",
            format!(
                "selected Rust import context requires at least {} output bytes but max_bytes is {output_limit}",
                short.len()
            ),
        )])
    }
}

fn full_envelope(program: &Program, import: &ImportDeclaration, max_bytes: usize) -> String {
    let cargo_alias = import
        .rust_path
        .as_deref()
        .and_then(|path| path.split("::").next());
    render(json!({
        "authority": {
            "execution": false,
            "publication": false,
            "tool_invocation": false
        },
        "budget": {"max_bytes": max_bytes, "used_bytes": 0},
        "declared_effects": import.effects,
        "index": {
            "digest": null,
            "status": "index_unprepared"
        },
        "module": program.module,
        "package": {
            "cargo_alias": cargo_alias,
            "features": null,
            "name": null,
            "source_sha256": null,
            "version": null
        },
        "revision": crate::graph::revision(program),
        "schema": SCHEMA,
        "selected_import": {
            "docs": null,
            "id": import.stable_id,
            "ownership": null,
            "path": import.rust_path,
            "signature": null,
            "source_name": import.name
        },
        "setup": {
            "instruction": SETUP,
            "required": true
        },
        "truncation": {
            "omitted": [],
            "truncated": false
        }
    }))
}

fn truncated_envelope(program: &Program, import: &ImportDeclaration, max_bytes: usize) -> String {
    render(json!({
        "authority": {"execution": false, "publication": false, "tool_invocation": false},
        "budget": {"max_bytes": max_bytes, "used_bytes": 0},
        "index": {"status": "index_unprepared"},
        "revision": crate::graph::revision(program),
        "schema": SCHEMA,
        "selected_import": {
            "id": abbreviate(&import.stable_id, 256),
            "path": import.rust_path.as_deref().map(|path| abbreviate(path, 256)),
        },
        "setup": {"required": true},
        "truncation": {
            "omitted": ["declared_effects", "docs", "ownership", "package", "signature", "setup.instruction"],
            "truncated": true
        }
    }))
}

fn render(mut value: Value) -> String {
    let mut previous = usize::MAX;
    for _ in 0..4 {
        let output = serde_json::to_string(&value).expect("JSON value is serializable");
        let used = output.len();
        value["budget"]["used_bytes"] = json!(used);
        if used == previous {
            return serde_json::to_string(&value).expect("JSON value is serializable");
        }
        previous = used;
    }
    serde_json::to_string(&value).expect("JSON value is serializable")
}

fn abbreviate(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes.saturating_sub(3);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{agent_context_json, AgentContextOptions};
    use crate::parse;
    use std::path::Path;

    const SELECTED: &str = r#"module test.rust_api_context;

@id("rust.host")
interface RustHost permits { regex.read } {
    @id("rust.host.is_match")
    import rust selected fn is_match from "regex::Regex::is_match"
        effects { regex.read }
        failure infallible;
}
@id("rust.host.main") fn main() -> i64 { 0 }
"#;

    #[test]
    fn selected_import_context_is_pure_bounded_and_deterministic() {
        let program = parse(SELECTED, Path::new("selected.spx")).unwrap();
        let first = selected_rust_import_context_json(&program, "rust.host.is_match", 4096)
            .unwrap()
            .unwrap();
        let second = selected_rust_import_context_json(&program, "rust.host.is_match", 8192)
            .unwrap()
            .unwrap();
        let value: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(first, second, "the projection has a hard 4 KiB ceiling");
        assert!(first.len() <= MAX_BYTES);
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["index"]["status"], "index_unprepared");
        assert_eq!(value["selected_import"]["id"], "rust.host.is_match");
        assert_eq!(value["selected_import"]["path"], "regex::Regex::is_match");
        assert_eq!(value["package"]["cargo_alias"], "regex");
        assert_eq!(value["selected_import"]["signature"], Value::Null);
        assert_eq!(value["package"]["version"], Value::Null);
        assert_eq!(value["declared_effects"][0], "regex.read");
        assert_eq!(value["setup"]["required"], true);
        assert!(value["setup"]["instruction"]
            .as_str()
            .unwrap()
            .contains("explicit pinned rustdoc JSON extractor"));
        assert_eq!(value["authority"]["tool_invocation"], false);
        assert_eq!(value["authority"]["execution"], false);
        assert_eq!(value["truncation"]["truncated"], false);
        assert_eq!(value["budget"]["used_bytes"], first.len());
    }

    #[test]
    fn selected_import_context_reports_truncation_and_keeps_old_refusal_closed() {
        let effects = (0..160)
            .map(|index| format!("effect{index:03}abcdefghijklmnop"))
            .collect::<Vec<_>>();
        let effect_set = effects.join(", ");
        let source = format!(
            "module test.rust_api_context_large;\n\n@id(\"rust.host\")\ninterface RustHost permits {{ {effect_set} }} {{\n    @id(\"rust.host.is_match\")\n    import rust selected fn is_match from \"regex::Regex::is_match\"\n        effects {{ {effect_set} }}\n        failure infallible;\n}}\n@id(\"rust.host.main\") fn main() -> i64 {{ 0 }}\n"
        );
        let program = parse(&source, Path::new("large-selected.spx")).unwrap();
        let bounded = selected_rust_import_context_json(&program, "rust.host.is_match", 4096)
            .unwrap()
            .unwrap();
        let value: Value = serde_json::from_str(&bounded).unwrap();
        assert!(bounded.len() <= MAX_BYTES);
        assert_eq!(value["index"]["status"], "index_unprepared");
        assert_eq!(value["truncation"]["truncated"], true);
        assert!(value["truncation"]["omitted"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "declared_effects"));
        assert_eq!(value["budget"]["used_bytes"], bounded.len());

        let old = parse(
            "module test.old;\n\n@id(\"rust.host\")\ninterface RustHost permits {  } {\n    @id(\"rust.host.ping\")\n    import rust fn ping(value: i64) -> unit effects {  } failure infallible;\n}\n@id(\"rust.host.main\") fn main() -> i64 { 0 }\n",
            Path::new("old-rust-import.spx"),
        )
        .unwrap();
        assert_eq!(
            selected_rust_import_context_json(&old, "rust.host.ping", 4096).unwrap(),
            None
        );
        assert_eq!(
            agent_context_json(&old, "rust.host.ping", &AgentContextOptions::default())
                .unwrap_err()[0]
                .code,
            "SPX-G218"
        );
    }
}
