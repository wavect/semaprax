//! Strict transport decode for one replay-verified Core Wasm stage package.
//! A tagged row is accepted only as an observation; the caller binds it to
//! the selected owned-Bytes projection before minting a cleanup event.

use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;

use super::{invariant, BYTE_STREAM_CAP};

#[derive(Debug)]
pub(super) struct NodeStageValue {
    pub(super) text: String,
    pub(super) settled_owned_bytes: bool,
}

impl NodeStageValue {
    pub(super) fn require_projection(&self, expects_owned_bytes: bool) -> Result<(), Diagnostic> {
        if self.settled_owned_bytes != expects_owned_bytes {
            return Err(invariant("wasm_executor.outcome.owned_bytes_tag"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) enum NodeStageRun {
    Returned(Vec<NodeStageValue>),
    LanguageFailure(NormalizedStatus),
    /// Agent Stage Semantic Work v1 exhaustion; only a metered observer
    /// produces it, never this ordinary decoder.
    FuelExhausted,
    /// The same unconditional call-depth admission the interpreter and
    /// native C11 backend enforce (`MAX_CALL_DEPTH`), refused before the
    /// call was ever selected for metering; not a language-visible status,
    /// so it never reaches `normalized_raw_status`.
    CallDepthExceeded,
}

fn normalized_raw_status(raw: u64) -> Option<NormalizedStatus> {
    let arithmetic = match raw {
        1 => Some(StatusCase::AddOverflow),
        2 => Some(StatusCase::SubOverflow),
        3 => Some(StatusCase::MulOverflow),
        4 => Some(StatusCase::DivisionByZero),
        5 => Some(StatusCase::DivisionOverflow),
        6 => Some(StatusCase::RemainderByZero),
        7 => Some(StatusCase::RemainderOverflow),
        8 => Some(StatusCase::NegationOverflow),
        _ => None,
    };
    arithmetic
        .map(crate::runtime_status::normalize_arithmetic)
        .or_else(|| match raw {
            9 => Some(crate::runtime_status::normalize_contract(
                ContractPhase::Requires,
            )),
            10 => Some(crate::runtime_status::normalize_contract(
                ContractPhase::Ensures,
            )),
            _ => None,
        })
}

pub(super) fn decode_node_outcomes(
    stdout: &str,
    expected: usize,
) -> Result<NodeStageRun, Diagnostic> {
    let rows = stdout.lines().collect::<Vec<_>>();
    if rows.is_empty() || rows.len() > expected || expected == 0 {
        return Err(invariant("wasm_executor.outcome.arity"));
    }
    let row_count = rows.len();
    let mut values = Vec::with_capacity(expected);
    let mut failure: Option<NormalizedStatus> = None;
    let mut call_depth_exceeded = false;
    for row in rows {
        let value: serde_json::Value =
            serde_json::from_str(row).map_err(|_| invariant("wasm_executor.outcome.json"))?;
        let object = value
            .as_object()
            .ok_or_else(|| invariant("wasm_executor.outcome.object"))?;
        if object.get("schema").and_then(serde_json::Value::as_str)
            != Some("semaprax.agent-wasm-stage-outcome.v2")
        {
            return Err(invariant("wasm_executor.outcome.schema"));
        }
        match object.get("kind").and_then(serde_json::Value::as_str) {
            Some("returned") if object.len() == 3 => {
                if failure.is_some() {
                    return Err(invariant("wasm_executor.outcome.mixed"));
                }
                values.push(NodeStageValue {
                    text: object
                        .get("value")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| invariant("wasm_executor.outcome.value"))?
                        .to_owned(),
                    settled_owned_bytes: false,
                });
            }
            Some("settled_owned_bytes") if object.len() == 4 => {
                if failure.is_some() {
                    return Err(invariant("wasm_executor.outcome.mixed"));
                }
                let text = object
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| invariant("wasm_executor.outcome.value"))?;
                let byte_length = object
                    .get("byte_length")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .filter(|value| *value <= BYTE_STREAM_CAP)
                    .ok_or_else(|| invariant("wasm_executor.outcome.byte_length"))?;
                if text.len() != byte_length * 2
                    || !text
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(invariant("wasm_executor.outcome.owned_bytes_value"));
                }
                values.push(NodeStageValue {
                    text: text.to_owned(),
                    settled_owned_bytes: true,
                });
            }
            Some("call_depth_exceeded") if object.len() == 2 => {
                if !values.is_empty() || failure.is_some() {
                    return Err(invariant("wasm_executor.outcome.mixed"));
                }
                call_depth_exceeded = true;
            }
            Some("language_failure") if object.len() == 4 => {
                if !values.is_empty() {
                    return Err(invariant("wasm_executor.outcome.mixed"));
                }
                let raw = object
                    .get("raw_status")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| invariant("wasm_executor.outcome.raw_status"))?;
                let status = normalized_raw_status(raw)
                    .ok_or_else(|| invariant("wasm_executor.outcome.raw_status"))?;
                let wire = object
                    .get("status")
                    .and_then(serde_json::Value::as_object)
                    .filter(|wire| wire.len() == 5)
                    .ok_or_else(|| invariant("wasm_executor.outcome.status"))?;
                let class = if raw <= 8 { "arithmetic" } else { "contract" };
                if wire.get("schema").and_then(serde_json::Value::as_str) != Some(status.schema())
                    || wire.get("domain_id").and_then(serde_json::Value::as_str)
                        != Some(status.domain_id())
                    || wire.get("code").and_then(serde_json::Value::as_u64)
                        != Some(u64::from(status.code()))
                    || wire.get("class").and_then(serde_json::Value::as_str) != Some(class)
                    || wire.get("retryable").and_then(serde_json::Value::as_bool) != Some(false)
                    || failure.as_ref().is_some_and(|selected| selected != &status)
                {
                    return Err(invariant("wasm_executor.outcome.status_mismatch"));
                }
                failure = Some(status);
            }
            _ => return Err(invariant("wasm_executor.outcome.shape")),
        }
    }
    match (failure, call_depth_exceeded) {
        (Some(_), true) => Err(invariant("wasm_executor.outcome.mixed")),
        (Some(status), false) if row_count == 1 => Ok(NodeStageRun::LanguageFailure(status)),
        (Some(_), false) => Err(invariant("wasm_executor.outcome.failure_arity")),
        (None, true) if row_count == 1 => Ok(NodeStageRun::CallDepthExceeded),
        (None, true) => Err(invariant("wasm_executor.outcome.failure_arity")),
        (None, false) if values.len() == expected => Ok(NodeStageRun::Returned(values)),
        (None, false) => Err(invariant("wasm_executor.outcome.arity")),
    }
}
