//! Bounded, read-only mapping of captured generated-wrapper rustc errors.
//! Captured output has no execution authority and is labelled unverified.

use semaprax::diagnostic::Diagnostic;
use semaprax_rust_api_index::RustApiIndex;
use serde_json::{json, Value};
use std::path::Path;

pub const MAX_RUSTC_JSON_BYTES: usize = 262_144;
const MAX_MAPPED: usize = 16;
const SCHEMA: &str = "semaprax.indexed-rustc-diagnostics.v1";

fn refusal(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B149", message)
}

fn classification(code: &str) -> Option<(&'static str, &'static str)> {
    match code {
        "E0277" => Some(("SPX-B150", "Rust trait bound rejected")),
        "E0425" | "E0432" | "E0433" => Some((
            "SPX-B151",
            "Rust item is unavailable for the selected feature set",
        )),
        "E0106" | "E0515" | "E0621" => Some(("SPX-B152", "Rust lifetime requirement rejected")),
        _ => None,
    }
}

/// Map only primary errors in the exact generated wrapper file to one checked
/// selected import. Foreign or ambiguous errors remain raw and unmapped.
pub fn map_captured_wrapper_errors(
    index: &RustApiIndex,
    source_path: &str,
    source: &str,
    import_id: &str,
    generated_file: &str,
    input: &[u8],
) -> Result<String, Diagnostic> {
    if input.is_empty()
        || input.len() > MAX_RUSTC_JSON_BYTES
        || generated_file.is_empty()
        || generated_file.len() > 4096
    {
        return Err(refusal(
            "captured rustc diagnostics are absent or exceed the bounded wrapper mapping input",
        ));
    }
    let program = semaprax::parse(source, Path::new(source_path))
        .map_err(|_| refusal("Semaprax source cannot be parsed for rustc diagnostic mapping"))?;
    semaprax::rust_api_context::prepared_selected_rust_import_context_json(
        &program,
        import_id,
        index.canonical_json().as_bytes(),
        4096,
    )
    .map_err(|_| refusal("Semaprax selected import is not valid for rustc diagnostic mapping"))?
    .ok_or_else(|| refusal("Semaprax selected import identity is unavailable"))?;
    let imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.index_selected)
        .collect::<Vec<_>>();
    if imports.len() != 1 || imports[0].stable_id != import_id {
        return Err(refusal(
            "rustc diagnostic mapping requires one selected import identity",
        ));
    }
    let import = imports[0];
    let path = import
        .rust_path
        .as_deref()
        .ok_or_else(|| refusal("selected Rust path is absent"))?;
    index
        .select_supported(&[path])
        .map_err(|_| refusal("selected Rust item is unavailable in the prepared index"))?;
    let mut rows = Vec::new();
    for line in input
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let envelope: Value = serde_json::from_slice(line)
            .map_err(|_| refusal("captured rustc diagnostic JSON is malformed"))?;
        let message = if envelope["reason"] == "compiler-message" {
            &envelope["message"]
        } else {
            &envelope
        };
        if message["level"] != "error" {
            continue;
        }
        let Some(code) = message["code"]["code"].as_str() else {
            continue;
        };
        let Some((mapped_code, label)) = classification(code) else {
            continue;
        };
        let spans = message["spans"]
            .as_array()
            .ok_or_else(|| refusal("captured rustc error spans are absent"))?;
        if !spans
            .iter()
            .any(|span| span["is_primary"] == true && span["file_name"] == generated_file)
        {
            continue;
        }
        let raw_message = message["message"]
            .as_str()
            .ok_or_else(|| refusal("captured rustc error message is absent"))?;
        if raw_message.len() > 2048 {
            return Err(refusal("captured rustc error message exceeds its bound"));
        }
        let rendered = message["rendered"].as_str().unwrap_or("");
        if rendered.len() > 8192 {
            return Err(refusal("captured rustc rendered detail exceeds its bound"));
        }
        rows.push(json!({
            "code": mapped_code,
            "message": format!("{label}: {raw_message}"),
            "source": {"path": source_path, "start": import.span.start, "end": import.span.end, "revision": semaprax::graph::revision(&program)},
            "rustc": {"code": code, "message": raw_message, "rendered": rendered},
        }));
        if rows.len() > MAX_MAPPED {
            return Err(refusal("captured rustc diagnostic count exceeds its bound"));
        }
    }
    if rows.is_empty() {
        return Err(refusal(
            "captured rustc errors do not identify a supported generated wrapper requirement",
        ));
    }
    let report = json!({
        "authority": {"execution": false, "publication": false, "tool_invocation": false},
        "capture_status": "external_unverified",
        "diagnostics": rows,
        "identity": {"index_digest": index.digest(), "target": index.target(), "feature_digest": index.feature_digest(), "selected_stable_rustc": index.stable_rustc_version(), "package": index.package().name, "version": index.package().version},
        "schema": SCHEMA,
    });
    serde_json::to_string(&report)
        .map_err(|_| refusal("captured rustc diagnostic report cannot be serialized"))
}
