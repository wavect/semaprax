//! Consented trace ingestion. Only public task/tool results and compiler or
//! check outcomes are admitted, by allowlist projection; records carrying
//! reasoning, credential-like members or values, sealed answers, or held-out
//! tasks are refused whole.

use super::spec::{Spec, Split};
use crate::json::{canonical, sha256_labeled};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const TRACE_SCHEMA: &str = "semaprax.evolution-trace.v1";
const OBSERVATION_SCHEMA: &str = "semaprax.harness-observation.v1";
const MAX_LINE: usize = 64 * 1024;
const ALLOWED: &[&str] = &[
    "schema",
    "task_id",
    "family",
    "kind",
    "tool",
    "outcome",
    "diagnostic_code",
    "message",
    "attempt",
    "repair",
];
const FORBIDDEN_KEYS: &[&str] = &[
    "reasoning",
    "thinking",
    "thought",
    "thoughts",
    "chain_of_thought",
    "scratchpad",
    "api_key",
    "apikey",
    "secret",
    "password",
    "authorization",
    "credential",
    "credentials",
    "private_key",
    "token_value",
    "expected",
    "expected_answer",
    "sealed",
    "hidden",
];
const SECRET_MARKERS: &[&str] = &[
    "-----BEGIN",
    "Bearer ",
    "sk-",
    "ghp_",
    "AKIA",
    "xox",
    "api_key=",
];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ingest {
    /// Canonical sanitized in-family records, input order.
    pub records: Vec<Value>,
    pub files: Vec<(String, String)>,
    pub refused: BTreeMap<String, u64>,
    pub out_of_family: u64,
    pub observation_events: u64,
    pub unreadable: Vec<String>,
}

impl Ingest {
    pub fn jsonl(&self) -> String {
        self.records
            .iter()
            .map(|r| canonical(r) + "\n")
            .collect::<String>()
    }
    pub fn digest(&self) -> String {
        sha256_labeled("semaprax.evolution-traces.v1", self.jsonl().as_bytes())
    }
    pub fn summary(&self) -> Value {
        json!({
            "ingested": self.records.len(),
            "out_of_family": self.out_of_family,
            "observation_events": self.observation_events,
            "refused": self.refused,
            "unreadable": self.unreadable,
        })
    }
}

fn forbidden_key(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) => m.iter().find_map(|(k, x)| {
            let l = k.to_ascii_lowercase();
            if FORBIDDEN_KEYS.contains(&l.as_str()) {
                Some(k.clone())
            } else {
                forbidden_key(x)
            }
        }),
        Value::Array(a) => a.iter().find_map(forbidden_key),
        _ => None,
    }
}

fn secret_value(v: &Value) -> bool {
    match v {
        Value::String(s) => SECRET_MARKERS.iter().any(|m| s.contains(m)),
        Value::Object(m) => m.values().any(secret_value),
        Value::Array(a) => a.iter().any(secret_value),
        _ => false,
    }
}

fn project(m: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for k in ALLOWED {
        if let Some(v) = m.get(*k) {
            if v.is_string() || v.is_u64() {
                let mut v = v.clone();
                if let Value::String(s) = &mut v {
                    s.truncate(s.char_indices().nth(2048).map_or(s.len(), |(i, _)| i));
                }
                out.insert((*k).to_string(), v);
            }
        }
    }
    out
}

fn bump(map: &mut BTreeMap<String, u64>, reason: &str) {
    *map.entry(reason.to_string()).or_default() += 1;
}

/// Records produced by one input line (a workflow report yields one per diagnostic).
fn records_of(line: &Value, ing: &mut Ingest) -> Vec<Map<String, Value>> {
    let Some(obj) = line.as_object() else {
        bump(&mut ing.refused, "not-an-object");
        return Vec::new();
    };
    match obj.get("schema").and_then(Value::as_str) {
        Some(TRACE_SCHEMA) => {
            if let Some(k) = forbidden_key(line) {
                bump(
                    &mut ing.refused,
                    &format!("forbidden-member:{}", k.to_ascii_lowercase()),
                );
                return Vec::new();
            }
            vec![project(obj)]
        }
        Some(OBSERVATION_SCHEMA) => {
            // Metadata-only events carry no repair evidence; counted, not forwarded.
            ing.observation_events += 1;
            Vec::new()
        }
        Some(s) if s.starts_with("semaprax.harness-run.") => {
            let status = obj
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let lineage = obj.get("lineage").and_then(Value::as_str).unwrap_or("run");
            let mut out = Vec::new();
            for (i, d) in obj
                .get("diagnostics")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                let code = d.get("code").and_then(Value::as_str).unwrap_or("");
                let mut m = Map::new();
                m.insert("schema".into(), json!(TRACE_SCHEMA));
                m.insert("task_id".into(), json!(format!("{lineage}#{i}")));
                m.insert(
                    "family".into(),
                    json!(format!("compiler-diagnostic/{code}")),
                );
                m.insert("kind".into(), json!("check"));
                m.insert("outcome".into(), json!(status));
                m.insert("diagnostic_code".into(), json!(code));
                if let Some(msg) = d.get("message") {
                    m.insert("message".into(), msg.clone());
                }
                out.push(project(&m));
            }
            out
        }
        _ => {
            bump(&mut ing.refused, "unknown-schema");
            Vec::new()
        }
    }
}

/// Read every consented file, sanitize, and keep in-family records.
pub fn ingest(spec: &Spec) -> Ingest {
    let mut ing = Ingest::default();
    let held: Vec<(&str, &str, &str)> = spec
        .tasks
        .iter()
        .filter(|t| t.split != Split::Train)
        .map(|t| (t.id.as_str(), t.prompt.as_str(), t.expected.as_str()))
        .collect();
    for path in &spec.traces {
        let name = path.display().to_string();
        let Ok(bytes) = std::fs::read(path) else {
            ing.unreadable.push(name);
            continue;
        };
        ing.files.push((name, crate::json::sha256_plain(&bytes)));
        for raw in String::from_utf8_lossy(&bytes).lines() {
            if raw.trim().is_empty() {
                continue;
            }
            if raw.len() > MAX_LINE {
                bump(&mut ing.refused, "oversize-line");
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(raw) else {
                bump(&mut ing.refused, "invalid-json");
                continue;
            };
            for rec in records_of(&v, &mut ing) {
                let rec = Value::Object(rec);
                if secret_value(&rec) {
                    bump(&mut ing.refused, "secret-like-value");
                    continue;
                }
                let text = canonical(&rec);
                let id = rec.get("task_id").and_then(Value::as_str).unwrap_or("");
                if held.iter().any(|(hid, p, e)| {
                    *hid == id
                        || (e.len() >= 3 && text.contains(e))
                        || (!p.is_empty() && text.contains(p))
                }) {
                    bump(&mut ing.refused, "heldout-or-sealed-content");
                    continue;
                }
                if rec.get("family").and_then(Value::as_str) != Some(spec.family.as_str()) {
                    ing.out_of_family += 1;
                    continue;
                }
                ing.records.push(rec);
            }
        }
    }
    ing
}
