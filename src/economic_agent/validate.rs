//! Shared document validation primitives: bounded canonical JSON parsing,
//! depth and size admission, typed field accessors, and the SPX-G21x
//! diagnostic constructors.

use super::journal::Reconciliation;
use super::{
    EconomicRail, Intent, Limits, Payment, MAX_IDENTIFIER_BYTES, MAX_JSON_DEPTH, NONCLAIMS,
    POLICY_SCHEMA,
};
use crate::diagnostic::{quote_json, Diagnostic};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(super) fn terminal_floor(limits: &Limits) -> Result<usize, Diagnostic> {
    usize::try_from(limits.max_trace_bytes)
        .ok()
        .and_then(|trace| {
            usize::try_from(limits.max_evidence_bytes)
                .ok()
                .and_then(|evidence| evidence.checked_mul(2))
                .and_then(|evidence| trace.checked_add(evidence))
        })
        .and_then(|value| value.checked_add(4096))
        .ok_or_else(|| g216("builder_bytes", limits.max_builder_bytes))
}

pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

pub(super) fn admitted_now_from(
    intent: &Intent,
    observed: u64,
    elapsed: u64,
) -> Result<u64, Diagnostic> {
    intent
        .created_at
        .max(observed)
        .checked_add(elapsed)
        .ok_or_else(|| g212("expired"))
}

pub(super) fn confirmation_target(intent: &Intent) -> u64 {
    match &intent.payment {
        Payment::Bitcoin { confirmations, .. } => *confirmations,
        Payment::X402 {
            rail: EconomicRail::Bitcoin,
            ..
        } => 1,
        _ => 0,
    }
}

pub(super) fn validate_confirmation(
    intent: &Intent,
    reconciliation: &Reconciliation,
) -> Result<(), Diagnostic> {
    let target = confirmation_target(intent);
    if reconciliation.status == "confirmed" && reconciliation.confirmations.unwrap_or(0) < target {
        return Err(g213());
    }
    Ok(())
}

pub(super) fn g210(document: &str, schema: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G210",
        format!("Economic Agent {document} is not canonical {schema} JSON"),
    )
}
pub(super) fn g211(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G211",
        format!("Economic Agent policy invariant failed: {field}"),
    )
}
pub(super) fn g212(reason: &'static str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G212",
        format!("Economic Agent payment intent was rejected: {reason}"),
    )
}
pub(super) fn g213() -> Diagnostic {
    Diagnostic::io(
        "SPX-G213",
        "Economic Agent prepared transaction or simulation disagrees with the admitted intent",
    )
}
pub(super) fn g214() -> Diagnostic {
    Diagnostic::io(
        "SPX-G214",
        "Economic Agent approval is absent, expired, rejected, or digest-mismatched",
    )
}
pub(super) fn g215() -> Diagnostic {
    Diagnostic::io(
        "SPX-G215",
        "Economic Agent journal state or idempotency replay disagrees with the admitted operation",
    )
}
pub(super) fn g216(field: &str, maximum: u64) -> Diagnostic {
    Diagnostic::io("SPX-G216", format!("{field} exceeds {maximum}"))
}
pub(super) fn g217() -> Diagnostic {
    Diagnostic::io(
        "SPX-G217",
        "Economic Agent Trace or Evidence disagrees with the replayed state machine",
    )
}
pub(super) fn info(code: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::io(code, message)
}

pub(super) fn canonical<'a>(
    source: &'a str,
    document: &str,
    schema: &str,
    maximum: usize,
) -> Result<(&'a str, Value), Diagnostic> {
    if source.len() > maximum {
        return Err(g216(document_bytes_field(document), maximum as u64));
    }
    let Some(body) = source.strip_suffix('\n') else {
        return Err(g210(document, schema));
    };
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(g210(document, schema));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| g210(document, schema))?;
    if depth(&value) > MAX_JSON_DEPTH {
        return Err(g216("json_depth", MAX_JSON_DEPTH as u64));
    }
    if value
        .as_object()
        .and_then(|row| row.get("schema"))
        .and_then(Value::as_str)
        != Some(schema)
    {
        return Err(g210(document, schema));
    }
    Ok((body, value))
}
pub(super) fn canonical_policy_limited<'a>(
    source: &'a str,
    document: &str,
    schema: &str,
    maximum: u64,
    max_depth: u64,
) -> Result<(&'a str, Value), Diagnostic> {
    let (body, value) = canonical(source, document, schema, maximum as usize)?;
    if depth(&value) as u64 > max_depth {
        return Err(g216("json_depth", max_depth));
    }
    Ok((body, value))
}
pub(super) fn configured_depth(source: &str, limits: &Limits) -> Result<(), Diagnostic> {
    if structural_json_depth(source).ok_or_else(g217)? > limits.max_json_depth {
        return Err(g216("json_depth", limits.max_json_depth));
    }
    Ok(())
}

pub(super) fn structural_json_depth(source: &str) -> Option<u64> {
    let mut depth = 0_u64;
    let mut maximum = 0_u64;
    let mut quoted = false;
    let mut escaped = false;
    for byte in source.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1)?;
                maximum = maximum.max(depth);
            }
            b'}' | b']' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    (!quoted && !escaped && depth == 0).then(|| {
        if source
            .bytes()
            .any(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'{' | b'}' | b'[' | b']'))
        {
            maximum.saturating_add(1)
        } else {
            maximum
        }
    })
}

pub(super) fn configured_document_limits(
    source: &str,
    document: &str,
    maximum: u64,
    limits: &Limits,
) -> Result<(), Diagnostic> {
    if source.len() > maximum as usize {
        return Err(g216(document_bytes_field(document), maximum));
    }
    configured_depth(source, limits)
}

pub(super) fn document_bytes_field(document: &str) -> &'static str {
    match document {
        "policy" => "policy_bytes",
        "payment intent" => "intent_bytes",
        "x402 invoice" => "invoice_bytes",
        "chain snapshot" => "snapshot_bytes",
        "payment plan" => "plan_bytes",
        "simulation" => "simulation_bytes",
        "approval request" => "approval_request_bytes",
        "approval" => "approval_bytes",
        "journal" => "journal_bytes",
        "broadcast receipt" => "broadcast_receipt_bytes",
        "reconciliation" => "reconciliation_bytes",
        "trace" => "trace_bytes",
        "evidence" => "evidence_bytes",
        _ => "builder_bytes",
    }
}

pub(super) fn depth(value: &Value) -> usize {
    match value {
        Value::Array(v) => 1 + v.iter().map(depth).max().unwrap_or(0),
        Value::Object(v) => 1 + v.values().map(depth).max().unwrap_or(0),
        _ => 1,
    }
}
pub(super) fn object<'a>(
    value: &'a Value,
    doc: &str,
    schema: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    value.as_object().ok_or_else(|| g210(doc, schema))
}
pub(super) fn keys(row: &Map<String, Value>, expected: &[&str]) -> bool {
    row.len() == expected.len() && expected.iter().all(|key| row.contains_key(*key))
}
pub(super) fn text<'a>(
    row: &'a Map<String, Value>,
    key: &str,
    doc: &str,
    schema: &str,
) -> Result<&'a str, Diagnostic> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| g210(doc, schema))
}
pub(super) fn number(
    row: &Map<String, Value>,
    key: &str,
    doc: &str,
    schema: &str,
) -> Result<u64, Diagnostic> {
    row.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| g210(doc, schema))
}
pub(super) fn policy_limit(
    row: &Map<String, Value>,
    key: &str,
    maximum: u64,
    nonzero: bool,
) -> Result<u64, Diagnostic> {
    let value = number(row, key, "policy", POLICY_SCHEMA)?;
    if value > maximum || (nonzero && value == 0) {
        return Err(g211(&format!("limits.{key}")));
    }
    Ok(value)
}
pub(super) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b':' | b'-')
        })
}
pub(super) fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
pub(super) fn string_array(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}
pub(super) fn string_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| quote_json(v))
            .collect::<Vec<_>>()
            .join(",")
    )
}
pub(super) fn nonclaims_json() -> String {
    format!(
        "[{}]",
        NONCLAIMS
            .iter()
            .map(|v| quote_json(v))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn rail(value: &str) -> Option<EconomicRail> {
    match value {
        "evm" => Some(EconomicRail::Evm),
        "solana" => Some(EconomicRail::Solana),
        "bitcoin" => Some(EconomicRail::Bitcoin),
        _ => None,
    }
}
