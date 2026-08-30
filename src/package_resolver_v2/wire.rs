use std::collections::BTreeSet;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::bounded_output;
use crate::diagnostic::{quote_json, Diagnostic};

use super::{DIGEST_DOMAIN, MAX_JSON_DEPTH, MAX_OUTPUT_BYTES, MAX_WORK_UNITS, SCHEMA};

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

pub(super) fn charge(work: &mut usize, units: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(units)
        .ok_or_else(|| limit_error("resolver work accounting overflow"))?;
    if *work > MAX_WORK_UNITS {
        return Err(limit_error("resolver work bound exceeded"));
    }
    Ok(())
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
    if wire.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error("resolution evidence exceeds output bound"));
    }
    validate_json(wire)?;
    let value: Value =
        serde_json::from_str(wire).map_err(|_| wire_error("resolution evidence is not JSON"))?;
    require_keys(&value, &["schema", "digest", "bytes", "payload"], "wrapper")?;
    if value["schema"].as_str() != Some(SCHEMA) {
        return Err(wire_error("resolution evidence schema is invalid"));
    }
    let marker = "\"payload\":";
    let offset = wire
        .find(marker)
        .map(|offset| offset + marker.len())
        .ok_or_else(|| wire_error("resolution payload is missing"))?;
    let payload = wire
        .get(offset..wire.len().saturating_sub(1))
        .ok_or_else(|| wire_error("resolution payload boundary is invalid"))?;
    if value["bytes"].as_u64() != Some(payload.len() as u64)
        || value["digest"].as_str() != Some(digest(DIGEST_DOMAIN, payload.as_bytes()).as_str())
    {
        return Err(replay_error(
            "resolution evidence digest/byte binding failed",
        ));
    }
    if render_wrapper(payload) != wire {
        return Err(wire_error("resolution evidence wrapper is not canonical"));
    }
    validate_payload(&value["payload"])
}

fn validate_payload(payload: &Value) -> Result<(), Diagnostic> {
    require_keys(
        payload,
        &[
            "schema",
            "requirements",
            "target",
            "allowed_capabilities",
            "catalog",
            "selected",
            "lock_digest",
            "lock_bytes",
            "lock",
            "limits",
            "budget",
            "nonclaims",
        ],
        "payload",
    )?;
    if payload["schema"].as_str() != Some(SCHEMA)
        || payload["requirements"].as_array().is_none()
        || payload["target"].as_str().is_none()
        || payload["allowed_capabilities"].as_array().is_none()
        || payload["selected"].as_array().is_none()
        || payload["lock_digest"].as_str().is_none()
        || payload["lock_bytes"].as_u64().is_none()
        || payload["lock"].as_object().is_none()
        || payload["nonclaims"].as_array().is_none()
    {
        return Err(wire_error("resolution payload member type is invalid"));
    }
    let requirements = payload["requirements"]
        .as_array()
        .ok_or_else(|| wire_error("requirements must be array"))?;
    for row in requirements {
        require_keys(row, &["package", "range"], "requirement row")?;
        require_strings(row, &["package", "range"], "requirement row")?;
    }
    let selected = payload["selected"]
        .as_array()
        .ok_or_else(|| wire_error("selected must be array"))?;
    for row in selected {
        require_keys(
            row,
            &["package", "version", "subject_digest", "subject_bytes"],
            "selected row",
        )?;
        require_strings(
            row,
            &["package", "version", "subject_digest"],
            "selected row",
        )?;
        require_numbers(row, &["subject_bytes"], "selected row")?;
    }
    require_keys(
        &payload["catalog"],
        &["subjects", "bytes", "digest"],
        "catalog binding",
    )?;
    require_numbers(
        &payload["catalog"],
        &["subjects", "bytes"],
        "catalog binding",
    )?;
    require_strings(&payload["catalog"], &["digest"], "catalog binding")?;
    require_keys(
        &payload["limits"],
        &[
            "max_requirements",
            "max_subjects",
            "max_versions_per_package",
            "max_selected_packages",
            "max_allowed_capabilities",
            "max_subject_bytes",
            "max_total_subject_bytes",
            "max_edges",
            "max_depth",
            "max_decisions",
            "max_work_units",
            "max_json_depth",
            "max_render_bytes",
            "max_output_bytes",
            "requested_max_bytes",
        ],
        "limits",
    )?;
    require_numbers(
        &payload["limits"],
        &[
            "max_requirements",
            "max_subjects",
            "max_versions_per_package",
            "max_selected_packages",
            "max_allowed_capabilities",
            "max_subject_bytes",
            "max_total_subject_bytes",
            "max_edges",
            "max_depth",
            "max_decisions",
            "max_work_units",
            "max_json_depth",
            "max_render_bytes",
            "max_output_bytes",
            "requested_max_bytes",
        ],
        "limits",
    )?;
    require_keys(
        &payload["budget"],
        &[
            "used_subjects",
            "used_subject_bytes",
            "used_selected_packages",
            "used_edges",
            "used_depth",
            "used_decisions",
            "used_allowed_capabilities",
            "used_work_units",
        ],
        "budget",
    )?;
    require_numbers(
        &payload["budget"],
        &[
            "used_subjects",
            "used_subject_bytes",
            "used_selected_packages",
            "used_edges",
            "used_depth",
            "used_decisions",
            "used_allowed_capabilities",
            "used_work_units",
        ],
        "budget",
    )?;
    if payload["allowed_capabilities"]
        .as_array()
        .is_some_and(|values| values.iter().any(|value| value.as_str().is_none()))
        || payload["nonclaims"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str().is_none()))
    {
        return Err(wire_error("resolution string array member is invalid"));
    }
    Ok(())
}

fn require_keys(value: &Value, keys: &[&str], label: &str) -> Result<(), Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| wire_error(format!("{label} must be object")))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(wire_error(format!(
            "{label} keys are not the closed schema"
        )));
    }
    Ok(())
}

fn require_strings(value: &Value, keys: &[&str], label: &str) -> Result<(), Diagnostic> {
    if keys.iter().any(|key| value[*key].as_str().is_none()) {
        return Err(wire_error(format!("{label} string member is invalid")));
    }
    Ok(())
}

fn require_numbers(value: &Value, keys: &[&str], label: &str) -> Result<(), Diagnostic> {
    if keys.iter().any(|key| value[*key].as_u64().is_none()) {
        return Err(wire_error(format!("{label} integer member is invalid")));
    }
    Ok(())
}

fn validate_json(wire: &str) -> Result<(), Diagnostic> {
    if wire.is_empty()
        || wire.starts_with('\u{feff}')
        || wire.ends_with('\n')
        || wire.contains('\r')
    {
        return Err(wire_error("resolution evidence wire is invalid"));
    }
    let mut parser = DuplicateParser {
        bytes: wire.as_bytes(),
        offset: 0,
        depth: 0,
    };
    parser.value()?;
    if parser.offset != parser.bytes.len() {
        return Err(wire_error("resolution evidence has trailing data"));
    }
    Ok(())
}

struct DuplicateParser<'a> {
    bytes: &'a [u8],
    offset: usize,
    depth: usize,
}

impl DuplicateParser<'_> {
    fn value(&mut self) -> Result<(), Diagnostic> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(|_| ()),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(wire_error("resolution evidence JSON token is invalid")),
        }
    }

    fn object(&mut self) -> Result<(), Diagnostic> {
        self.enter(b'{')?;
        let mut keys = BTreeSet::new();
        if self.take(b'}') {
            return self.leave();
        }
        loop {
            let key = self.string()?;
            if !keys.insert(key) {
                return Err(wire_error("resolution evidence contains duplicate key"));
            }
            self.expect(b':')?;
            self.value()?;
            if self.take(b'}') {
                return self.leave();
            }
            self.expect(b',')?;
        }
    }

    fn array(&mut self) -> Result<(), Diagnostic> {
        self.enter(b'[')?;
        if self.take(b']') {
            return self.leave();
        }
        loop {
            self.value()?;
            if self.take(b']') {
                return self.leave();
            }
            self.expect(b',')?;
        }
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        let start = self.offset;
        self.expect(b'"')?;
        let mut escaped = false;
        while let Some(byte) = self.peek() {
            self.offset += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let slice = std::str::from_utf8(&self.bytes[start..self.offset])
                    .map_err(|_| wire_error("resolution evidence is not UTF-8"))?;
                let decoded: String = serde_json::from_str(slice)
                    .map_err(|_| wire_error("resolution evidence string is invalid"))?;
                if quote_json(&decoded) != slice {
                    return Err(wire_error(
                        "resolution evidence string is not canonically quoted",
                    ));
                }
                return Ok(decoded);
            } else if byte < 0x20 {
                return Err(wire_error("resolution evidence string has control byte"));
            }
        }
        Err(wire_error("resolution evidence string is unterminated"))
    }

    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        let slice = std::str::from_utf8(&self.bytes[start..self.offset])
            .map_err(|_| wire_error("resolution evidence number is invalid"))?;
        if slice.is_empty() || (slice.len() > 1 && slice.starts_with('0')) {
            return Err(wire_error(
                "resolution evidence integer is not canonical nonnegative decimal",
            ));
        }
        slice
            .parse::<u64>()
            .map(|_| ())
            .map_err(|_| wire_error("resolution evidence integer overflows u64"))
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), Diagnostic> {
        if self.bytes.get(self.offset..self.offset + literal.len()) == Some(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            Err(wire_error("resolution evidence literal is invalid"))
        }
    }

    fn enter(&mut self, byte: u8) -> Result<(), Diagnostic> {
        self.expect(byte)?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| limit_error("resolution JSON depth overflow"))?;
        if self.depth > MAX_JSON_DEPTH {
            return Err(limit_error("resolution JSON depth exceeds limit"));
        }
        Ok(())
    }

    fn leave(&mut self) -> Result<(), Diagnostic> {
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or_else(|| wire_error("resolution JSON depth underflow"))?;
        Ok(())
    }

    fn expect(&mut self, byte: u8) -> Result<(), Diagnostic> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(wire_error("resolution evidence punctuation is invalid"))
        }
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }
}

pub(super) fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, Diagnostic> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| wire_error(format!("{key} must be string")))
}

pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    bf!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn map_subject_error(error: &Diagnostic) -> Diagnostic {
    match error.code {
        "SPX-PL606" => limit_error("nested Subject-v3 bound failed"),
        _ => authentication_error("Subject-v3 or Report-v2 replay failed"),
    }
}

pub(super) fn map_lock_errors(errors: &[Diagnostic], message: &str) -> Diagnostic {
    errors.first().map_or_else(
        || resolution_error(message),
        |error| map_lock_error(error, message),
    )
}

pub(super) fn map_lock_error(error: &Diagnostic, message: &str) -> Diagnostic {
    match error.code {
        "SPX-PL606" => limit_error("nested Lock-v3 bound failed"),
        "SPX-PL607" => replay_error(message),
        "SPX-PL603" => authentication_error("nested Lock-v3 authentication failed"),
        _ => resolution_error(message),
    }
}

pub(super) fn option_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR601", message.into())
}
pub(super) fn input_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR601", message.into())
}
pub(super) fn authentication_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR602", message.into())
}
pub(super) fn resolution_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR603", message.into())
}
pub(super) fn policy_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR604", message.into())
}
pub(super) fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR605", message.into())
}
pub(super) fn wire_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR606", message.into())
}
pub(super) fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PR607", message.into())
}
