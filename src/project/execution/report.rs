//! Canonical rendering and independent replay of Project execution reports.
//!
//! This module is authority-free: it transforms already-authenticated facts
//! into bounded bytes or verifies one closed envelope by exact reconstruction.

use crate::bounded_output::{budgeted_format, with_limit};
use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::NormalizedStatus;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::interpreter::{self, MAX_STEPS_LIMIT};
use crate::{graph, runtime_status};
use sha2::{Digest as _, Sha256};

use super::cases::{
    ProjectContractArgument, ProjectContractFailure, ProjectTestCase, MAX_CONTRACT_TEXT_BYTES,
};
use super::{ProjectExecutionOutcome, ProjectExecutionRole, TEST_CASE_PREFIX};
use crate::project::{
    MAX_MODULE_BYTES, MAX_NAME_BYTES, MAX_STABLE_ID_BYTES, PROJECT_SCHEMA, PROJECT_SCHEMA_V10,
    PROJECT_SCHEMA_V11, PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3, PROJECT_SCHEMA_V4, PROJECT_SCHEMA_V5,
    PROJECT_SCHEMA_V6, PROJECT_SCHEMA_V7, PROJECT_SCHEMA_V8, PROJECT_SCHEMA_V9,
};

pub const PROJECT_EXECUTION_SCHEMA: &str = "semaprax.project-execution.v1";
pub(super) const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-execution.payload.v1\0";
const NONCLAIMS_JSON: &str = "\"in_process_reference_interpreter_only\",\
\"no_target_execution\",\
\"no_filesystem_process_or_backend_authority\",\
\"no_test_discovery\",\
\"no_cache_or_persistence\"";
const NONCLAIMS: [&str; 5] = [
    "in_process_reference_interpreter_only",
    "no_target_execution",
    "no_filesystem_process_or_backend_authority",
    "no_test_discovery",
    "no_cache_or_persistence",
];

/// Render an envelope without contract-failure detail or named cases; the
/// entry role and every pre-case test envelope have exactly this shape.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn render(
    project_schema: &str,
    project_revision: &str,
    workspace_revision: &str,
    project: &str,
    role: ProjectExecutionRole,
    module: &str,
    entry_id: &str,
    steps_used: usize,
    max_steps: usize,
    max_bytes: usize,
    outcome: &ProjectExecutionOutcome,
) -> Result<String, Vec<Diagnostic>> {
    render_full(
        project_schema,
        project_revision,
        workspace_revision,
        project,
        role,
        module,
        entry_id,
        steps_used,
        max_steps,
        max_bytes,
        outcome,
        None,
        &[],
    )
}

fn outcome_json(
    outcome: &ProjectExecutionOutcome,
    failure: Option<&ProjectContractFailure>,
) -> String {
    match outcome {
        ProjectExecutionOutcome::Returned(value) => budgeted_format(format_args!(
            "{{\"kind\":\"returned\",\"type\":\"i64\",\"value\":{}}}",
            quote_json(&value.to_string())
        )),
        ProjectExecutionOutcome::LanguageFailure(status) => match failure {
            Some(failure) => budgeted_format(format_args!(
                "{{\"kind\":\"language_failure\",\"status\":{},\"failure\":{}}}",
                status.to_json(),
                failure_json(failure)
            )),
            None => budgeted_format(format_args!(
                "{{\"kind\":\"language_failure\",\"status\":{}}}",
                status.to_json()
            )),
        },
        ProjectExecutionOutcome::FuelExhausted => {
            budgeted_format(format_args!("{{\"kind\":\"fuel_exhausted\"}}"))
        }
        ProjectExecutionOutcome::CallDepthExceeded => {
            budgeted_format(format_args!("{{\"kind\":\"call_depth_exceeded\"}}"))
        }
    }
}

fn failure_json(failure: &ProjectContractFailure) -> String {
    let arguments = failure
        .arguments
        .iter()
        .map(|argument| {
            budgeted_format(format_args!(
                "{{\"name\":{},\"type\":{},\"value\":{}}}",
                quote_json(&argument.name),
                quote_json(&argument.ty),
                quote_json(&argument.value)
            ))
        })
        .collect::<Vec<_>>()
        .join(",");
    budgeted_format(format_args!(
        "{{\"function\":{},\"phase\":{},\"clause\":{},\"arguments\":[{}]}}",
        quote_json(&failure.function_id),
        quote_json(failure.phase),
        quote_json(&failure.clause),
        arguments
    ))
}

fn cases_json(cases: &[ProjectTestCase]) -> String {
    cases
        .iter()
        .map(|case| {
            budgeted_format(format_args!(
                "{{\"stable_id\":{},\"name\":{},\"fuel\":{{\"steps_used\":{},\"max_steps\":{}}},\"outcome\":{}}}",
                quote_json(&case.stable_id),
                quote_json(&case.name),
                case.steps_used,
                case.max_steps,
                outcome_json(&case.outcome, case.failure.as_ref())
            ))
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Render the complete envelope. `failure` is admitted only with a
/// `LanguageFailure` outcome; `cases` is rendered only for the test role, where
/// it is always present, possibly empty.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_full(
    project_schema: &str,
    project_revision: &str,
    workspace_revision: &str,
    project: &str,
    role: ProjectExecutionRole,
    module: &str,
    entry_id: &str,
    steps_used: usize,
    max_steps: usize,
    max_bytes: usize,
    outcome: &ProjectExecutionOutcome,
    failure: Option<&ProjectContractFailure>,
    cases: &[ProjectTestCase],
) -> Result<String, Vec<Diagnostic>> {
    let (envelope, overflowed) = with_limit(max_bytes, || {
        let outcome = outcome_json(outcome, failure);
        let cases = match role {
            ProjectExecutionRole::Entry => String::new(),
            ProjectExecutionRole::Test => {
                budgeted_format(format_args!(",\"cases\":[{}]", cases_json(cases)))
            }
        };
        let payload = budgeted_format(format_args!(
            "{{\"schema\":{},\"project_schema\":{},\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"role\":{},\"module\":{},\"stable_id\":{},\"limits\":{{\"max_bytes\":{},\"max_steps\":{}}},\"fuel\":{{\"steps_used\":{},\"max_steps\":{}}},\"outcome\":{}{},\"nonclaims\":[{}]}}",
            quote_json(PROJECT_EXECUTION_SCHEMA),
            quote_json(project_schema),
            quote_json(project),
            quote_json(project_revision),
            quote_json(workspace_revision),
            quote_json(role.text()),
            quote_json(module),
            quote_json(entry_id),
            max_bytes,
            max_steps,
            steps_used,
            max_steps,
            outcome,
            cases,
            NONCLAIMS_JSON,
        ));
        let digest = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
        let prefix = payload.strip_suffix('}').unwrap_or_default();
        budgeted_format(format_args!(
            "{},\"payload_digest\":{}}}",
            prefix,
            quote_json(&digest)
        ))
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-F104",
            "project execution output exceeds the max-bytes budget; refusing to truncate"
                .to_owned(),
        )]);
    }
    Ok(envelope)
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

/// Verifies one canonical Project Developer Loop v1 execution envelope.
///
/// Verification authenticates only the closed report bytes. It grants no
/// execution, filesystem, process, backend, cache, or publication authority.
/// Every admitted field is independently checked and the complete canonical
/// envelope is reconstructed before exact byte comparison.
pub fn verify_execution_envelope(envelope: &str) -> Result<(), Diagnostic> {
    if envelope.len() > graph::MAX_AGENT_CONTEXT_BYTES {
        return Err(verification_error(format!(
            "project execution envelope exceeds the {}-byte verification bound",
            graph::MAX_AGENT_CONTEXT_BYTES
        )));
    }
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| verification_error(format!("envelope is not valid JSON: {error}")))?;
    let object = require_object(&value, "envelope")?;
    let role = match object.get("role").and_then(serde_json::Value::as_str) {
        Some("entry") => ProjectExecutionRole::Entry,
        Some("test") => ProjectExecutionRole::Test,
        _ => {
            return Err(verification_error(
                "envelope role must be exactly `entry` or `test`".to_owned(),
            ))
        }
    };
    let common_keys = [
        "fuel",
        "limits",
        "module",
        "nonclaims",
        "outcome",
        "payload_digest",
        "project",
        "project_revision",
        "project_schema",
        "role",
        "schema",
        "stable_id",
        "workspace_revision",
    ];
    match role {
        ProjectExecutionRole::Entry => require_keys(object, &common_keys, "envelope")?,
        ProjectExecutionRole::Test => {
            let mut keys = vec!["cases"];
            keys.extend(common_keys);
            require_keys(object, &keys, "test envelope")?;
        }
    }
    require_text_eq(object, "schema", PROJECT_EXECUTION_SCHEMA)?;
    let project_schema = require_text(object, "project_schema")?;
    if !matches!(
        project_schema,
        PROJECT_SCHEMA
            | PROJECT_SCHEMA_V2
            | PROJECT_SCHEMA_V3
            | PROJECT_SCHEMA_V4
            | PROJECT_SCHEMA_V5
            | PROJECT_SCHEMA_V6
            | PROJECT_SCHEMA_V7
            | PROJECT_SCHEMA_V8
            | PROJECT_SCHEMA_V9
            | PROJECT_SCHEMA_V10
            | PROJECT_SCHEMA_V11
    ) {
        return Err(verification_error(
            "project_schema must name an admitted Project v1 through v11 schema".to_owned(),
        ));
    }
    let project = require_bounded_text(object, "project", MAX_NAME_BYTES)?;
    let module = require_bounded_text(object, "module", MAX_MODULE_BYTES)?;
    let stable_id = require_bounded_text(object, "stable_id", MAX_STABLE_ID_BYTES)?;
    if !valid_project_name(project) {
        return Err(verification_error(
            "project is not a canonical Project v1 name".to_owned(),
        ));
    }
    if !valid_module(module) {
        return Err(verification_error(
            "module is not a canonical Project v1 module name".to_owned(),
        ));
    }
    let project_revision = require_digest(object, "project_revision")?;
    let workspace_revision = require_digest(object, "workspace_revision")?;

    let limits = require_member_object(object, "limits")?;
    require_keys(limits, &["max_bytes", "max_steps"], "limits")?;
    let max_bytes = require_usize(limits, "max_bytes")?;
    let max_steps = require_usize(limits, "max_steps")?;
    interpreter::InterpreterOptions::new(max_bytes, max_steps).map_err(|_| {
        verification_error(format!(
            "limits must be within max_bytes {}..={} and max_steps 1..={MAX_STEPS_LIMIT}",
            graph::MIN_AGENT_CONTEXT_BYTES,
            graph::MAX_AGENT_CONTEXT_BYTES
        ))
    })?;
    if envelope.len() > max_bytes {
        return Err(verification_error(
            "envelope byte length exceeds its declared max_bytes".to_owned(),
        ));
    }

    let fuel = require_member_object(object, "fuel")?;
    require_keys(fuel, &["max_steps", "steps_used"], "fuel")?;
    let fuel_max_steps = require_usize(fuel, "max_steps")?;
    let steps_used = require_usize(fuel, "steps_used")?;
    if fuel_max_steps != max_steps {
        return Err(verification_error(
            "fuel max_steps must equal limits max_steps".to_owned(),
        ));
    }
    if steps_used > max_steps {
        return Err(verification_error(
            "fuel steps_used exceeds max_steps".to_owned(),
        ));
    }

    let outcome_object = require_member_object(object, "outcome")?;
    let (outcome, failure) = verify_outcome(outcome_object, steps_used, max_steps)?;
    let cases = match role {
        ProjectExecutionRole::Entry => Vec::new(),
        ProjectExecutionRole::Test => verify_cases(&object["cases"], max_steps)?,
    };

    let Some(nonclaims) = object["nonclaims"].as_array() else {
        return Err(verification_error(
            "envelope nonclaims must be an array".to_owned(),
        ));
    };
    if nonclaims.len() != NONCLAIMS.len()
        || nonclaims
            .iter()
            .zip(NONCLAIMS)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(verification_error(
            "envelope nonclaims must equal the fixed ordered v1 list".to_owned(),
        ));
    }
    let payload_digest = require_text(object, "payload_digest")?;
    if !is_sha256_digest(payload_digest) {
        return Err(verification_error(
            "payload_digest must be `sha256:` plus 64 lowercase hexadecimal digits".to_owned(),
        ));
    }

    let reconstructed = render_full(
        project_schema,
        project_revision,
        workspace_revision,
        project,
        role,
        module,
        stable_id,
        steps_used,
        max_steps,
        max_bytes,
        &outcome,
        failure.as_ref(),
        &cases,
    )
    .map_err(|_| {
        verification_error(
            "canonical reconstruction exceeds the envelope's declared max_bytes".to_owned(),
        )
    })?;
    if reconstructed != envelope {
        return Err(verification_error(
            "envelope bytes are not the exact canonical reconstruction".to_owned(),
        ));
    }
    Ok(())
}

fn verify_cases(
    value: &serde_json::Value,
    max_steps: usize,
) -> Result<Vec<ProjectTestCase>, Diagnostic> {
    let Some(entries) = value.as_array() else {
        return Err(verification_error(
            "test envelope cases must be an array".to_owned(),
        ));
    };
    let mut cases = Vec::with_capacity(entries.len());
    for entry in entries {
        let case = require_object(entry, "test case")?;
        require_keys(case, &["fuel", "name", "outcome", "stable_id"], "test case")?;
        let stable_id = require_bounded_text(case, "stable_id", MAX_STABLE_ID_BYTES)?;
        let name = require_bounded_text(case, "name", MAX_STABLE_ID_BYTES)?;
        if !name.starts_with(TEST_CASE_PREFIX) {
            return Err(verification_error(format!(
                "test case name must start with `{TEST_CASE_PREFIX}`"
            )));
        }
        let fuel = require_member_object(case, "fuel")?;
        require_keys(fuel, &["max_steps", "steps_used"], "test case fuel")?;
        let case_max_steps = require_usize(fuel, "max_steps")?;
        let steps_used = require_usize(fuel, "steps_used")?;
        if case_max_steps != max_steps || steps_used > max_steps {
            return Err(verification_error(
                "test case fuel must use the envelope max_steps and not exceed it".to_owned(),
            ));
        }
        let outcome_object = require_member_object(case, "outcome")?;
        let (outcome, failure) = verify_outcome(outcome_object, steps_used, max_steps)?;
        cases.push(ProjectTestCase {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            outcome,
            steps_used,
            max_steps,
            failure,
        });
    }
    Ok(cases)
}

fn verify_failure(value: &serde_json::Value) -> Result<ProjectContractFailure, Diagnostic> {
    let failure = require_object(value, "contract failure")?;
    require_keys(
        failure,
        &["arguments", "clause", "function", "phase"],
        "contract failure",
    )?;
    let function_id = require_bounded_text(failure, "function", MAX_STABLE_ID_BYTES)?;
    let phase = match require_text(failure, "phase")? {
        "requires" => "requires",
        "ensures" => "ensures",
        _ => {
            return Err(verification_error(
                "contract failure phase must be exactly `requires` or `ensures`".to_owned(),
            ))
        }
    };
    let clause = require_bounded_text(failure, "clause", MAX_CONTRACT_TEXT_BYTES)?;
    let Some(entries) = failure["arguments"].as_array() else {
        return Err(verification_error(
            "contract failure arguments must be an array".to_owned(),
        ));
    };
    let mut arguments = Vec::with_capacity(entries.len());
    for entry in entries {
        let argument = require_object(entry, "contract failure argument")?;
        require_keys(
            argument,
            &["name", "type", "value"],
            "contract failure argument",
        )?;
        arguments.push(ProjectContractArgument {
            name: require_bounded_text(argument, "name", MAX_CONTRACT_TEXT_BYTES)?.to_owned(),
            ty: require_bounded_text(argument, "type", MAX_CONTRACT_TEXT_BYTES)?.to_owned(),
            value: require_bounded_text(argument, "value", MAX_CONTRACT_TEXT_BYTES)?.to_owned(),
        });
    }
    Ok(ProjectContractFailure {
        function_id: function_id.to_owned(),
        phase,
        clause: clause.to_owned(),
        arguments,
    })
}

fn verify_outcome(
    outcome: &serde_json::Map<String, serde_json::Value>,
    steps_used: usize,
    max_steps: usize,
) -> Result<(ProjectExecutionOutcome, Option<ProjectContractFailure>), Diagnostic> {
    match require_text(outcome, "kind")? {
        "returned" => {
            require_keys(outcome, &["kind", "type", "value"], "returned outcome")?;
            require_text_eq(outcome, "type", "i64")?;
            let text = require_text(outcome, "value")?;
            let value = text.parse::<i64>().map_err(|_| {
                verification_error(
                    "returned i64 value must be a canonical decimal string".to_owned(),
                )
            })?;
            if value.to_string() != text {
                return Err(verification_error(
                    "returned i64 value must be a canonical decimal string".to_owned(),
                ));
            }
            Ok((ProjectExecutionOutcome::Returned(value), None))
        }
        "language_failure" => {
            let failure = if outcome.contains_key("failure") {
                require_keys(
                    outcome,
                    &["failure", "kind", "status"],
                    "language_failure outcome",
                )?;
                Some(verify_failure(&outcome["failure"])?)
            } else {
                require_keys(outcome, &["kind", "status"], "language_failure outcome")?;
                None
            };
            let status = verify_status(&outcome["status"])?;
            if let Some(failure) = &failure {
                let phase = match failure.phase {
                    "ensures" => ContractPhase::Ensures,
                    _ => ContractPhase::Requires,
                };
                if status != runtime_status::normalize_contract(phase) {
                    return Err(verification_error(
                        "contract failure detail phase disagrees with the language failure status"
                            .to_owned(),
                    ));
                }
            }
            Ok((ProjectExecutionOutcome::LanguageFailure(status), failure))
        }
        "fuel_exhausted" => {
            require_keys(outcome, &["kind"], "fuel_exhausted outcome")?;
            if steps_used != max_steps {
                return Err(verification_error(
                    "fuel_exhausted requires steps_used equal to max_steps".to_owned(),
                ));
            }
            Ok((ProjectExecutionOutcome::FuelExhausted, None))
        }
        "call_depth_exceeded" => {
            require_keys(outcome, &["kind"], "call_depth_exceeded outcome")?;
            Ok((ProjectExecutionOutcome::CallDepthExceeded, None))
        }
        _ => Err(verification_error(
            "outcome kind is outside the closed project-execution v1 vocabulary".to_owned(),
        )),
    }
}

fn verify_status(value: &serde_json::Value) -> Result<NormalizedStatus, Diagnostic> {
    let status = require_object(value, "language_failure status")?;
    require_keys(
        status,
        &["class", "code", "domain_id", "retryable", "schema"],
        "language_failure status",
    )?;
    require_text_eq(
        status,
        "schema",
        crate::conformance::NORMALIZED_STATUS_SCHEMA_V1,
    )?;
    if status["retryable"].as_bool() != Some(false) {
        return Err(verification_error(
            "compiler-owned language failure status must be non-retryable".to_owned(),
        ));
    }
    let code = status["code"].as_u64().ok_or_else(|| {
        verification_error("language failure status code must be an unsigned integer".to_owned())
    })?;
    let rebuilt = match (
        require_text(status, "domain_id")?,
        require_text(status, "class")?,
    ) {
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
                _ => {
                    return Err(verification_error(
                        "arithmetic status code is outside the closed v1 table".to_owned(),
                    ))
                }
            };
            runtime_status::normalize_arithmetic(case)
        }
        (crate::conformance::CONTRACT_STATUS_DOMAIN_V1, "contract") => match code {
            1 => runtime_status::normalize_contract(ContractPhase::Requires),
            2 => runtime_status::normalize_contract(ContractPhase::Ensures),
            _ => {
                return Err(verification_error(
                    "contract status code is outside the closed v1 table".to_owned(),
                ))
            }
        },
        _ => {
            return Err(verification_error(
                "language failure status domain/class is outside the compiler-owned v1 table"
                    .to_owned(),
            ))
        }
    };
    Ok(rebuilt)
}

fn require_object<'a>(
    value: &'a serde_json::Value,
    section: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Diagnostic> {
    value
        .as_object()
        .ok_or_else(|| verification_error(format!("{section} must be a JSON object")))
}

fn require_member_object<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Diagnostic> {
    require_object(&object[key], key)
}

fn require_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
    section: &str,
) -> Result<(), Diagnostic> {
    if object.keys().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err(verification_error(format!(
            "{section} keys are not the exact closed v1 set"
        )));
    }
    Ok(())
}

fn require_text<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    object[key]
        .as_str()
        .ok_or_else(|| verification_error(format!("{key} must be a string")))
}

fn require_text_eq(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) -> Result<(), Diagnostic> {
    if require_text(object, key)? != expected {
        return Err(verification_error(format!(
            "{key} must be exactly `{expected}`"
        )));
    }
    Ok(())
}

fn require_bounded_text<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    max_bytes: usize,
) -> Result<&'a str, Diagnostic> {
    let text = require_text(object, key)?;
    if text.is_empty() || text.len() > max_bytes || text.contains('\0') {
        return Err(verification_error(format!(
            "{key} must be a nonempty NUL-free string of at most {max_bytes} UTF-8 bytes"
        )));
    }
    Ok(text)
}

fn require_digest<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    let digest = require_text(object, key)?;
    if !is_sha256_digest(digest) {
        return Err(verification_error(format!(
            "{key} must be `sha256:` plus 64 lowercase hexadecimal digits"
        )));
    }
    Ok(digest)
}

fn valid_project_name(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_module(value: &str) -> bool {
    value.split('.').all(|segment| {
        segment
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn require_usize(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<usize, Diagnostic> {
    let value = object[key]
        .as_u64()
        .ok_or_else(|| verification_error(format!("{key} must be an unsigned integer")))?;
    usize::try_from(value)
        .map_err(|_| verification_error(format!("{key} does not fit the host usize")))
}

fn verification_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F106", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V9};

    #[test]
    fn additive_project_schemas_render_and_replay_without_widening_the_envelope() {
        for schema in [PROJECT_SCHEMA_V9, PROJECT_SCHEMA_V10] {
            let envelope = render(
                schema,
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "profile",
                ProjectExecutionRole::Test,
                "profile.tests",
                "profile.tests.main",
                1,
                100,
                65_536,
                &ProjectExecutionOutcome::Returned(0),
            )
            .unwrap();
            verify_execution_envelope(&envelope).unwrap();
            assert!(verify_execution_envelope(&envelope.replacen(
                schema,
                "semaprax.project.v99",
                1
            ))
            .is_err());
        }
    }
}
