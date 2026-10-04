//! Rust stdio adapter for `context.repository/v1` (op `search`), speaking
//! `semaprax.harness-rpc.v1` by hand with serde_json only.
//!
//! Searches files under `SEMAPRAX_HARNESS_PROJECT_ROOT` (bounded), or a fixed
//! in-memory corpus when the variable is unset.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Write};
use std::path::Path;

const PROTOCOL: &str = "semaprax.harness-rpc.v1";
const MAX_FILES: usize = 500;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const FIXED: [(&str, &str); 2] = [
    (
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    ),
    (
        "src/main.rs",
        "fn main() {\n    println!(\"{}\", add(1, 2));\n}\n",
    ),
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Collect (relative path, text) pairs, recording skipped files with reasons.
fn corpus(root: Option<&str>) -> (Vec<(String, String)>, Vec<Value>) {
    let Some(root) = root else {
        return (
            FIXED
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
            vec![],
        );
    };
    let (mut files, mut skipped) = (Vec::new(), Vec::new());
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if name.starts_with('.')
                || ft.is_symlink()
                || name == "target"
                || name == "node_modules"
            {
                continue;
            }
            if ft.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(Path::new(&name))
                .to_string_lossy()
                .replace('\\', "/");
            if files.len() >= MAX_FILES {
                skipped.push(json!({"path": rel, "reason": "file-count-limit"}));
            } else if e
                .metadata()
                .map(|m| m.len() > MAX_FILE_BYTES)
                .unwrap_or(true)
            {
                skipped.push(json!({"path": rel, "reason": "file-too-large"}));
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                files.push((rel, text));
            } else {
                skipped.push(json!({"path": rel, "reason": "not-utf8"}));
            }
        }
    }
    files.sort();
    skipped.sort_by_key(|v| v["path"].as_str().unwrap_or("").to_string());
    (files, skipped)
}

fn search(root: Option<&str>, query: &str) -> Value {
    let (files, skipped) = corpus(root);
    let mut items = Vec::new();
    for (path, text) in &files {
        for (i, line) in text.lines().enumerate() {
            if !query.is_empty() && line.contains(query) {
                items.push(json!({
                    "path": path, "span": {"start_line": i + 1, "end_line": i + 1},
                    "digest": hex(&Sha256::digest(line.as_bytes())), "provenance": "structural",
                    "language": "rust", "rank": 1, "text": line,
                }));
            }
        }
    }
    items.truncate(50);
    json!({"items": items, "coverage": {"complete": skipped.is_empty(), "indexed_files": files.len(),
        "exhaustive": skipped.is_empty(), "skipped": skipped}})
}

fn invoke(req: &Value) -> Value {
    let provenance = json!({"provider_id": "org.example/context-rust", "adapter_version": "0.1.0", "upstream_version": "builtin-0.1.0"});
    let (status, payload, diag) =
        if req["capability"]["kind"] == "context.repository" && req["operation"] == "search" {
            let root = std::env::var("SEMAPRAX_HARNESS_PROJECT_ROOT").ok();
            let q = req["payload"]["query"].as_str().unwrap_or("");
            let p = search(root.as_deref(), q);
            let st = if p["coverage"]["complete"] == true {
                "complete"
            } else {
                "partial"
            };
            (st, p, json!([]))
        } else {
            (
                "unsupported",
                Value::Null,
                json!([{"code": "unsupported", "message": "only context.repository search"}]),
            )
        };
    json!({"schema": "semaprax.harness-result.v1", "invocation_id": req["invocation_id"], "project": req["project"],
        "capability": req["capability"], "status": status, "payload": payload, "diagnostics": diag, "provenance": provenance})
}

fn main() {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut send = |v: Value| {
        writeln!(out, "{v}")
            .and_then(|_| out.flush())
            .expect("stdout closed");
    };
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg["id"].clone();
        match msg["method"].as_str() {
            Some("harness/initialize") => {
                if msg["params"]["protocol"] != PROTOCOL {
                    send(
                        json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "unsupported protocol"}}),
                    );
                    continue;
                }
                let offered = msg["params"]["offered"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let accepted: Vec<Value> = offered.into_iter()
                    .filter(|c| c["kind"] == "context.repository" && c["version"] == 1)
                    .map(|_| json!({"kind": "context.repository", "version": 1, "operations": ["search"]}))
                    .collect();
                send(
                    json!({"jsonrpc": "2.0", "id": id, "result": {"protocol": PROTOCOL, "accepted": accepted}}),
                );
            }
            Some("harness/invoke") => {
                send(json!({"jsonrpc": "2.0", "id": id, "result": invoke(&msg["params"])}))
            }
            Some("harness/cancel") => {} // search is synchronous and short; nothing to interrupt
            Some("harness/shutdown") => {
                send(json!({"jsonrpc": "2.0", "id": id, "result": {}}));
                return;
            }
            _ => send(
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}}),
            ),
        }
    }
}
