use sha2::{Digest as _, Sha256};

use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::diagnostic::Diagnostic;
use crate::runtime_status;

pub const PROJECT_SOURCE_TRACE_SCHEMA: &str = "semaprax.project-source-trace.v1";
pub(super) const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-source-trace.payload.v1\0";
pub(super) const NONCLAIMS: [&str; 8] = [
    "in_process_reference_interpreter_only",
    "no_target_execution_or_debugger_control",
    "no_wall_time_or_schedule_determinism",
    "no_filesystem_process_backend_or_publication_authority",
    "no_source_content",
    "expression_identities_are_revision_scoped",
    "trace_is_not_provenance_approval_or_compatibility_evidence",
    "cancellation_is_one_observed_cooperative_step_boundary",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectPreparedExecutionOutcome {
    Returned(i64),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    Cancelled { before_step: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSourceTraceEvent {
    pub index: usize,
    pub step: usize,
    pub depth: usize,
    pub phase: &'static str,
    pub function_id: String,
    pub expression_id: String,
    pub path: String,
    pub source_revision: String,
    pub source_digest: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSourceTrace {
    pub(super) envelope: String,
    pub(super) digest: String,
    pub(super) steps_used: usize,
    pub(super) recorded_events: usize,
    pub(super) dropped_events: usize,
}

impl ProjectSourceTrace {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn steps_used(&self) -> usize {
        self.steps_used
    }

    pub const fn recorded_events(&self) -> usize {
        self.recorded_events
    }

    pub const fn dropped_events(&self) -> usize {
        self.dropped_events
    }

    pub const fn truncated(&self) -> bool {
        self.dropped_events != 0
    }
}

pub(crate) fn parse_status(value: &serde_json::Value) -> Result<NormalizedStatus, Diagnostic> {
    let status = object(value, "status")?;
    keys(
        status,
        &["class", "code", "domain_id", "retryable", "schema"],
        "status",
    )?;
    text_eq(
        status,
        "schema",
        crate::conformance::NORMALIZED_STATUS_SCHEMA_V1,
    )?;
    if status["retryable"].as_bool() != Some(false) {
        return Err(verification_error("language status must be non-retryable"));
    }
    let code = status["code"]
        .as_u64()
        .ok_or_else(|| verification_error("status code must be unsigned"))?;
    match (text(status, "domain_id")?, text(status, "class")?) {
        (crate::conformance::ARITHMETIC_STATUS_DOMAIN_V1, "arithmetic") => {
            let case = match code {
                1 => StatusCase::AddOverflow,
                2 => StatusCase::SubOverflow,
                3 => StatusCase::MulOverflow,
                4 => StatusCase::DivisionByZero,
                5 => StatusCase::DivisionOverflow,
                6 => StatusCase::RemainderByZero,
                7 => StatusCase::RemainderOverflow,
                8 => StatusCase::NegationOverflow,
                _ => return Err(verification_error("unknown arithmetic status code")),
            };
            Ok(runtime_status::normalize_arithmetic(case))
        }
        (crate::conformance::CONTRACT_STATUS_DOMAIN_V1, "contract") => match code {
            1 => Ok(runtime_status::normalize_contract(ContractPhase::Requires)),
            2 => Ok(runtime_status::normalize_contract(ContractPhase::Ensures)),
            _ => Err(verification_error("unknown contract status code")),
        },
        (crate::byte_ops::RANGE_STATUS_DOMAIN, "adapter") => {
            let code = match code {
                1 => crate::byte_ops::RANGE_START_AFTER_END_CODE,
                2 => crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                _ => return Err(verification_error("unknown byte-range status code")),
            };
            NormalizedStatus::try_new(
                crate::byte_ops::RANGE_STATUS_DOMAIN,
                code,
                StatusClass::Adapter,
                Retryability::Known(false),
            )
            .map_err(|_| verification_error("byte-range status is not canonical"))
        }
        _ => Err(verification_error(
            "status domain/class is not compiler-owned",
        )),
    }
}

pub(super) fn object<'a>(
    value: &'a serde_json::Value,
    section: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Diagnostic> {
    value
        .as_object()
        .ok_or_else(|| verification_error(&format!("{section} must be an object")))
}

pub(super) fn keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
    section: &str,
) -> Result<(), Diagnostic> {
    if object.keys().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err(verification_error(&format!(
            "{section} keys are not the exact closed set"
        )));
    }
    Ok(())
}

pub(super) fn text<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    object[key]
        .as_str()
        .filter(|value| !value.is_empty() && !value.contains('\0'))
        .ok_or_else(|| verification_error(&format!("{key} must be nonempty NUL-free text")))
}

pub(super) fn text_eq(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) -> Result<(), Diagnostic> {
    if text(object, key)? != expected {
        return Err(verification_error(&format!(
            "{key} must equal `{expected}`"
        )));
    }
    Ok(())
}

pub(super) fn usize_value(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<usize, Diagnostic> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| verification_error(&format!("{key} must fit usize")))
}

pub(super) fn digest<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    let value = text(object, key)?;
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(verification_error(&format!(
            "{key} is not a canonical digest"
        )));
    }
    Ok(value)
}

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn verification_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F110", message)
}
