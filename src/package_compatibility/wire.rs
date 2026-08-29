use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::bounded_output;
use crate::diagnostic::{quote_json, Diagnostic};

use super::{CompatibilityOptions, DIGEST_DOMAIN, MAX_JSON_DEPTH, MAX_WORK_UNITS, SCHEMA};

macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }

pub(super) fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, Diagnostic> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| authentication_error(format!("{key} missing")))
}

pub(super) fn charge(work: &mut usize, units: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(units)
        .ok_or_else(|| limit_error("work overflow"))?;
    if *work > MAX_WORK_UNITS {
        return Err(limit_error("work exceeds limit"));
    }
    Ok(())
}

pub(super) fn validate_options(options: &CompatibilityOptions) -> Result<(), Diagnostic> {
    CompatibilityOptions::new(options.max_bytes).map(|_| ())
}

pub(super) fn render_wrapper(payload: &str) -> String {
    bf!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(SCHEMA),
        quote_json(&digest(DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload
    )
}

pub(super) fn parse_wrapper(wire: &str) -> Result<(), Diagnostic> {
    validate_json_depth(wire)?;
    if wire.is_empty()
        || wire.starts_with('\u{feff}')
        || wire.ends_with('\n')
        || wire.contains('\r')
    {
        return Err(wire_error("evidence wire invalid"));
    }
    let value: Value = serde_json::from_str(wire).map_err(|_| wire_error("evidence not JSON"))?;
    if value.as_object().is_none_or(|m| m.len() != 4) || value["schema"].as_str() != Some(SCHEMA) {
        return Err(wire_error("evidence shape/schema invalid"));
    }
    let marker = "\"payload\":";
    let offset = wire
        .find(marker)
        .ok_or_else(|| wire_error("payload missing"))?
        + marker.len();
    let payload = &wire[offset..wire.len() - 1];
    if value["bytes"].as_u64() != Some(payload.len() as u64)
        || value["digest"].as_str() != Some(digest(DIGEST_DOMAIN, payload.as_bytes()).as_str())
    {
        return Err(replay_error("evidence digest/bytes mismatch"));
    }
    Ok(())
}

fn validate_json_depth(wire: &str) -> Result<(), Diagnostic> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escaped = false;
    for byte in wire.bytes() {
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
        } else if byte == b'"' {
            string = true;
        } else if matches!(byte, b'{' | b'[') {
            depth = depth
                .checked_add(1)
                .ok_or_else(|| limit_error("JSON depth overflow"))?;
            if depth > MAX_JSON_DEPTH {
                return Err(limit_error("JSON depth exceeds limit"));
            }
        } else if matches!(byte, b'}' | b']') {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| wire_error("JSON nesting underflow"))?;
        }
    }
    if string || escaped || depth != 0 {
        return Err(wire_error("JSON nesting incomplete"));
    }
    Ok(())
}

pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    bf!("sha256:{:x}", crate::digest_hex::LowerHex(h.finalize()))
}

pub(super) fn option_error(m: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PC501", m.into())
}
pub(super) fn authentication_error(m: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PC502", m.into())
}
pub(super) fn limit_error(m: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PC503", m.into())
}
pub(super) fn wire_error(m: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PC504", m.into())
}
pub(super) fn replay_error(m: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PC505", m.into())
}
