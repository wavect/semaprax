//! Minimal in-process peer for tests: answers initialize/invoke/shutdown.
//! Frames carry no trailing LF in either direction.

use super::envelope::{RequestEnvelope, ResultEnvelope};
use super::kind::CapabilityKind;
use crate::json::{parse_frame, JsonLimits};
use serde_json::{json, Value};

/// Scripted behaviour of the mock adapter.
pub struct MockScript {
    /// Capabilities answered in `harness/initialize`: `(kind, version, operations)`.
    pub accepted: Vec<(CapabilityKind, u32, Vec<String>)>,
    /// Produces the raw result-envelope JSON for an invocation.
    pub invoke: Box<dyn Fn(&RequestEnvelope) -> Value>,
}

/// Answer one frame; `None` for notifications and unparseable input.
pub fn respond(frame: &[u8], script: &MockScript) -> Option<Vec<u8>> {
    let msg = parse_frame(frame, &JsonLimits::frame(4 * 1024 * 1024)).ok()?;
    let id = msg.get("id")?.clone();
    let result = match msg.get("method")?.as_str()? {
        "harness/initialize" => json!({
            "protocol": "semaprax.harness-rpc.v1",
            "accepted": script.accepted.iter().map(|(k, v, ops)| json!({"kind": k.as_str(), "version": v, "operations": ops})).collect::<Vec<_>>(),
        }),
        "harness/invoke" => match RequestEnvelope::from_json(msg.get("params")?) {
            Ok(req) => (script.invoke)(&req),
            Err(d) => return Some(error_frame(&id, &d.code, &d.message)),
        },
        "harness/shutdown" => json!({}),
        other => return Some(error_frame(&id, "method-not-found", other)),
    };
    Some(
        json!({"jsonrpc": "2.0", "id": id, "result": result})
            .to_string()
            .into_bytes(),
    )
}

fn error_frame(id: &Value, code: &str, message: &str) -> Vec<u8> {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": format!("{code}: {message}")}})
        .to_string()
        .into_bytes()
}

/// Convenience: a script returning a well-formed result for every invocation.
pub fn echo_script(
    provider_id: &str,
    payload: impl Fn(&RequestEnvelope) -> Value + 'static,
) -> MockScript {
    let provider_id = provider_id.to_string();
    MockScript {
        accepted: Vec::new(),
        invoke: Box::new(move |req| {
            ResultEnvelope::complete(req, payload(req), &provider_id, "0.0.0").to_json()
        }),
    }
}
