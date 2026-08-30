//! Disposable recovery capsules replay intentions from admitted source only.
//! No serialized source, HIR, approval, or filesystem authority is imported.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{
    wire, ProjectCandidate, ProjectRevision, SemanticChange, MAX_CHANGES,
    MAX_PROJECT_CANDIDATE_BYTES, MAX_SEMANTIC_CHANGE_BYTES, PROJECT_CANDIDATE_SCHEMA,
    SEMANTIC_CHANGE_SCHEMA,
};

pub const PROJECT_CANDIDATE_RECOVERY_SCHEMA: &str = "semaprax.project-candidate-recovery.v1";
pub const PROJECT_CANDIDATE_RECOVERY_COMPATIBILITY: &str =
    "semaprax.project-candidate-recovery-compatibility.v1";
pub const MAX_PROJECT_CANDIDATE_RECOVERY_BYTES: usize = MAX_PROJECT_CANDIDATE_BYTES;
const MAX_JSON_NODES: usize = MAX_CHANGES * (2 * 8192 + 128) + 256;
const MAX_JSON_DEPTH: usize = 128;
const DOMAIN: &[u8] = b"semaprax.project-candidate-recovery.payload.v1\0";

impl ProjectCandidate {
    /// Export a complete candidate's ordered history. The digest identifies
    /// exact canonical payload bytes; it is neither a signature nor authority.
    pub fn recovery_capsule(&self) -> Result<String, Vec<Diagnostic>> {
        let changes = self
            .changes
            .iter()
            .map(|change| {
                serde_json::from_str::<Value>(change.to_json())
                    .map_err(|_| invalid("retained semantic change is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut payload = json!({
            "schema":PROJECT_CANDIDATE_RECOVERY_SCHEMA,
            "compiler":compiler(),
            "base_revision":self.base.project_revision(),
            "change_schema":SEMANTIC_CHANGE_SCHEMA,
            "candidate_schema":PROJECT_CANDIDATE_SCHEMA,
            "changes":changes,
            "candidate_digest":self.candidate_digest(),
            "candidate_project_revision":self.revision.project_revision(),
        });
        let bytes = render(payload.clone())?;
        payload["capsule_digest"] = json!(wire::digest(DOMAIN, bytes.as_bytes()));
        render(payload)
    }

    /// Rebuild solely from the caller's independently admitted original base.
    /// Exact final candidate identity and complete canonical capsule bytes must
    /// agree. No image/HIR/source from the capsule is loaded into compiler state.
    pub fn restore(
        base: Arc<ProjectRevision>,
        expected_base: &str,
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_PROJECT_CANDIDATE_RECOVERY_BYTES {
            return Err(capacity(
                "candidate recovery capsule exceeds its byte bound",
            ));
        }
        preflight(bytes)?;
        let mut capsule: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("candidate recovery capsule is not valid bounded JSON"))?;
        let object = capsule
            .as_object()
            .ok_or_else(|| invalid("candidate recovery capsule must be an object"))?;
        const KEYS: &[&str] = &[
            "schema",
            "compiler",
            "base_revision",
            "change_schema",
            "candidate_schema",
            "changes",
            "candidate_digest",
            "candidate_project_revision",
            "capsule_digest",
        ];
        if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
            return Err(invalid(
                "candidate recovery capsule has missing or unknown fields",
            ));
        }
        if capsule["schema"] != PROJECT_CANDIDATE_RECOVERY_SCHEMA
            || capsule["compiler"] != compiler()
            || capsule["change_schema"] != SEMANTIC_CHANGE_SCHEMA
            || capsule["candidate_schema"] != PROJECT_CANDIDATE_SCHEMA
        {
            return Err(invalid(
                "candidate recovery compiler compatibility or schema does not match",
            ));
        }
        // Canonical reserialization rejects duplicate keys, alternate integer
        // spellings, whitespace, unknown fields and missing/extra terminal LF.
        if render(capsule.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "candidate recovery capsule must have exact canonical bytes",
            ));
        }
        let original_base = digest(&capsule, "base_revision")?.to_owned();
        let final_candidate = digest(&capsule, "candidate_digest")?.to_owned();
        let final_revision = digest(&capsule, "candidate_project_revision")?.to_owned();
        let expected_capsule = digest(&capsule, "capsule_digest")?.to_owned();
        if expected_base != base.project_revision() || original_base != expected_base {
            return Err(stale(
                "candidate recovery requires its exact admitted original base",
            ));
        }
        capsule
            .as_object_mut()
            .expect("closed object checked")
            .remove("capsule_digest");
        let payload = render(capsule.clone())?;
        if wire::digest(DOMAIN, payload.as_bytes()) != expected_capsule {
            return Err(stale("candidate recovery content digest does not match"));
        }
        let changes = capsule["changes"]
            .as_array()
            .ok_or_else(|| invalid("candidate recovery changes must be an ordered array"))?;
        if changes.len() > MAX_CHANGES {
            return Err(capacity("candidate recovery exceeds its history bound"));
        }
        // Each change has its own normal node/depth/string bounds. Applying the
        // single-change limits to the entire multi-change capsule would reject
        // legitimate histories; the raw scan above bounds the total instead.
        let changes = changes
            .iter()
            .map(|value| {
                let json = wire::render(value.clone(), MAX_SEMANTIC_CHANGE_BYTES)
                    .map_err(|_| capacity("recovered change exceeds its byte bound"))?;
                let base = value
                    .get("base_revision")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("recovered change requires a base revision"))?;
                let intent = value
                    .get("intent")
                    .ok_or_else(|| invalid("recovered change requires an intention"))?;
                // Retained changes entered through new(), whose structural
                // budget covers the intention. Do not charge its envelope
                // against that same budget again during lossless recovery.
                let change = SemanticChange::new(base, intent)?;
                if change.to_json() != json {
                    return Err(invalid(
                        "recovered change has noncanonical schema or requirements",
                    ));
                }
                Ok(change)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut candidate = ProjectCandidate::open(base, expected_base)?;
        for change in changes {
            candidate = candidate.apply(candidate.candidate_digest(), &change)?;
        }
        if candidate.candidate_digest() != final_candidate
            || candidate.revision.project_revision() != final_revision
        {
            return Err(stale(
                "candidate recovery replay disagrees with final candidate identity",
            ));
        }
        if candidate.recovery_capsule()?.as_bytes() != bytes {
            return Err(stale("candidate recovery exact capsule replay disagrees"));
        }
        Ok(candidate)
    }
}

pub(super) fn compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":PROJECT_CANDIDATE_RECOVERY_COMPATIBILITY})
}

fn digest<'a>(value: &'a Value, field: &str) -> Result<&'a str, Vec<Diagnostic>> {
    let text = value[field]
        .as_str()
        .ok_or_else(|| invalid("candidate recovery digest must be text"))?;
    if text.len() != 71
        || !text.starts_with("sha256:")
        || !text.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "candidate recovery digest is not canonical SHA-256",
        ));
    }
    Ok(text)
}

fn render(value: Value) -> Result<String, Vec<Diagnostic>> {
    wire::render(value, MAX_PROJECT_CANDIDATE_RECOVERY_BYTES)
        .map_err(|_| capacity("candidate recovery output exceeds its byte bound"))
}

// Pre-bound nesting and potential Value allocations before serde allocates a
// tree. Strings (including keys) and scalar tokens count once, containers once.
// Syntax/UTF-8/escape correctness remains the JSON parser's responsibility.
fn preflight(bytes: &[u8]) -> Result<(), Vec<Diagnostic>> {
    let mut string = false;
    let mut escape = false;
    let mut scalar = false;
    let mut depth = 0usize;
    let mut nodes = 0usize;
    for byte in bytes {
        if string {
            if escape {
                escape = false;
            } else if *byte == b'\\' {
                escape = true;
            } else if *byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                string = true;
                scalar = false;
                nodes += 1;
            }
            b'{' | b'[' => {
                depth += 1;
                scalar = false;
                nodes += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                scalar = false;
            }
            b':' | b',' | b' ' | b'\t' | b'\r' | b'\n' => scalar = false,
            _ => {
                if !scalar {
                    scalar = true;
                    nodes += 1;
                }
            }
        }
        if depth > MAX_JSON_DEPTH || nodes > MAX_JSON_NODES {
            return Err(capacity(
                "candidate recovery JSON exceeds its history-wide structural bounds",
            ));
        }
    }
    Ok(())
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G236", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G237", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G238", message)]
}
