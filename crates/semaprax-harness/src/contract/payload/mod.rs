//! Per-kind payload validators. Every shape is closed (unknown members are
//! refused) and each kind has distinct required members, so a payload valid
//! for one kind is refused by the others (`SPX-HPA040`).

pub mod command;
pub mod context;
pub mod evolve;
pub mod model;
pub mod skill;

use super::kind::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{Map, Value};

/// Which side of the envelope a payload travels on.
pub use semaprax_decision_core::wire::Direction;

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
        CapabilityKind::DecisionEvaluate => {
            semaprax_decision_core::wire::validate(direction, payload)
        }
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
        CapabilityKind::DecisionEvaluate => {
            semaprax_decision_core::wire::check_against_request(request, result)
        }
        CapabilityKind::SkillCatalog => skill::check_against_request(request, result),
        CapabilityKind::ModelGenerate => model::check_against_request(request, result),
        _ => Ok(()),
    }
}

pub(crate) fn e(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

// The closed-shape accessors and digest syntax are the decision core's (one
// implementation shared with the runtime boundary).
pub(crate) use semaprax_decision_core::shape::{array_of, bool_of, shape, str_of, uint_of};
pub(crate) use semaprax_decision_core::text::is_digest;

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
