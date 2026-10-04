//! Per-kind payload validators. Every shape is closed (unknown members are
//! refused) and each kind has distinct required members, so a payload valid
//! for one kind is refused by the others (`SPX-HPA040`).

pub mod command;
pub mod context;
pub mod decision;
pub mod model;
pub mod skill;

use super::kind::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{Map, Value};

/// Which side of the envelope a payload travels on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Request,
    Result,
}

/// Validate one payload for `(kind, operation, direction)`.
pub fn validate_payload(
    kind: CapabilityKind,
    operation: &str,
    direction: Direction,
    payload: &Value,
) -> HarnessResult<()> {
    if !kind.operations().contains(&operation) {
        return Err(e(
            "SPX-HPA046",
            format!("`{operation}` is not an operation of {}", kind.as_str()),
        ));
    }
    match kind {
        CapabilityKind::ContextRepository => context::validate(operation, direction, payload),
        CapabilityKind::CommandView => command::validate(operation, direction, payload),
        CapabilityKind::DecisionEvaluate => decision::validate(direction, payload),
        CapabilityKind::ModelGenerate => model::validate(direction, payload),
        CapabilityKind::SkillCatalog => skill::validate(operation, direction, payload),
    }
}

/// Cross-check a result payload against the request that produced it
/// (decision choice within options, skill digest equals the requested one).
pub fn check_against_request(
    kind: CapabilityKind,
    request: &Value,
    result: &Value,
) -> HarnessResult<()> {
    match kind {
        CapabilityKind::DecisionEvaluate => decision::check_against_request(request, result),
        CapabilityKind::SkillCatalog => skill::check_against_request(request, result),
        CapabilityKind::ModelGenerate => model::check_against_request(request, result),
        _ => Ok(()),
    }
}

pub(crate) fn e(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Require an object whose members are exactly `required` plus a subset of `optional`.
pub(crate) fn shape<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> HarnessResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| e("SPX-HPA040", format!("{what} must be an object")))?;
    for r in required {
        if !m.contains_key(*r) {
            return Err(e("SPX-HPA040", format!("{what} is missing `{r}`")));
        }
    }
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(e(
                "SPX-HPA040",
                format!("{what} has unexpected member `{k}`"),
            ));
        }
    }
    Ok(m)
}

pub(crate) fn str_of<'a>(
    m: &'a Map<String, Value>,
    key: &str,
    max: usize,
) -> HarnessResult<&'a str> {
    let s = m
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| e("SPX-HPA040", format!("`{key}` must be a string")))?;
    if s.len() > max {
        return Err(e("SPX-HPA040", format!("`{key}` exceeds {max} bytes")));
    }
    Ok(s)
}

pub(crate) fn uint_of(m: &Map<String, Value>, key: &str) -> HarnessResult<u64> {
    m.get(key).and_then(Value::as_u64).ok_or_else(|| {
        e(
            "SPX-HPA040",
            format!("`{key}` must be a non-negative integer"),
        )
    })
}

pub(crate) fn bool_of(m: &Map<String, Value>, key: &str) -> HarnessResult<bool> {
    m.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| e("SPX-HPA040", format!("`{key}` must be a boolean")))
}

pub(crate) fn array_of<'a>(
    m: &'a Map<String, Value>,
    key: &str,
    max: usize,
) -> HarnessResult<&'a Vec<Value>> {
    let a = m
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| e("SPX-HPA040", format!("`{key}` must be an array")))?;
    if a.len() > max {
        return Err(e(
            "SPX-HPA040",
            format!("`{key}` has more than {max} entries"),
        ));
    }
    Ok(a)
}

/// `sha256:` + 64 lowercase hex.
pub(crate) fn is_digest(s: &str) -> bool {
    s.strip_prefix("sha256:")
        .is_some_and(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

pub(crate) fn digest_of(m: &Map<String, Value>, key: &str) -> HarnessResult<()> {
    if is_digest(str_of(m, key, 80)?) {
        Ok(())
    } else {
        Err(e(
            "SPX-HPA040",
            format!("`{key}` must be `sha256:<64 hex>`"),
        ))
    }
}

/// Project-relative path: no root, drive, backslash, NUL, empty or `..` segment.
pub(crate) fn check_relative_path(p: &str) -> HarnessResult<()> {
    let bad = p.is_empty()
        || p.len() > 1024
        || p.starts_with('/')
        || p.contains('\\')
        || p.contains('\0')
        || p.as_bytes().get(1) == Some(&b':')
        || p.split('/').any(|s| s.is_empty() || s == "." || s == "..");
    if bad {
        Err(e(
            "SPX-HPA041",
            format!("path `{p}` is not a project-relative path"),
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn path_of(m: &Map<String, Value>, key: &str) -> HarnessResult<()> {
    check_relative_path(str_of(m, key, 1024)?)
}

pub(crate) fn opt_uint(m: &Map<String, Value>, key: &str) -> HarnessResult<()> {
    if m.contains_key(key) {
        uint_of(m, key)?;
    }
    Ok(())
}
