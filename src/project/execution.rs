//! Bounded, deterministic execution over one authenticated Project v1 snapshot.
//!
//! This module receives only the already-linked entry or test HIR retained by
//! [`super::ProjectSnapshot`]. It never parses, resolves, links, reads, writes,
//! spawns, or invokes a backend. The enclosing authenticated-project operation
//! retains ownership of the final held-input recheck.

use crate::bounded_output::{budgeted_format, with_limit};
use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::NormalizedStatus;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::interpreter::{
    self, ResolvedEvaluation, ResolvedEvaluationOutcome, DEFAULT_MAX_STEPS, MAX_STEPS_LIMIT,
};
use crate::{graph, runtime_status};
use sha2::{Digest as _, Sha256};

use super::{
    ProjectSnapshot, MAX_MODULE_BYTES, MAX_NAME_BYTES, MAX_STABLE_ID_BYTES, PROJECT_SCHEMA,
    PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3, PROJECT_SCHEMA_V4,
};

pub const PROJECT_EXECUTION_SCHEMA: &str = "semaprax.project-execution.v1";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-execution.payload.v1\0";
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectExecutionRole {
    Entry,
    Test,
}

impl ProjectExecutionRole {
    const fn text(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Test => "test",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectExecutionOptions {
    pub max_bytes: usize,
    pub max_steps: usize,
}

impl ProjectExecutionOptions {
    pub fn new(max_bytes: usize, max_steps: usize) -> Result<Self, Diagnostic> {
        interpreter::InterpreterOptions::new(max_bytes, max_steps)?;
        Ok(Self {
            max_bytes,
            max_steps,
        })
    }
}

impl Default for ProjectExecutionOptions {
    fn default() -> Self {
        let defaults = interpreter::InterpreterOptions::default();
        Self {
            max_bytes: defaults.max_bytes,
            max_steps: DEFAULT_MAX_STEPS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectExecutionOutcome {
    Returned(i64),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectExecution {
    role: ProjectExecutionRole,
    module: String,
    stable_id: String,
    outcome: ProjectExecutionOutcome,
    steps_used: usize,
    max_steps: usize,
    envelope: String,
}

impl ProjectExecution {
    pub const fn role(&self) -> ProjectExecutionRole {
        self.role
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    pub const fn outcome(&self) -> &ProjectExecutionOutcome {
        &self.outcome
    }

    pub const fn steps_used(&self) -> usize {
        self.steps_used
    }

    pub const fn max_steps(&self) -> usize {
        self.max_steps
    }

    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    /// Command-level success: any returned entry value is a successful run;
    /// the exact declared test closure passes only by returning zero.
    pub const fn command_succeeded(&self) -> bool {
        matches!(
            (&self.role, &self.outcome),
            (
                ProjectExecutionRole::Entry,
                ProjectExecutionOutcome::Returned(_)
            ) | (
                ProjectExecutionRole::Test,
                ProjectExecutionOutcome::Returned(0)
            )
        )
    }
}

pub(super) fn execute(
    snapshot: &ProjectSnapshot,
    role: ProjectExecutionRole,
    options: &ProjectExecutionOptions,
) -> Result<ProjectExecution, Vec<Diagnostic>> {
    // Revalidate public option construction even if a caller assembled the
    // public fields directly.
    interpreter::InterpreterOptions::new(options.max_bytes, options.max_steps)
        .map_err(|error| vec![error])?;

    let (program, module) = match role {
        ProjectExecutionRole::Entry => (&snapshot.entry_program, snapshot.manifest.entry()),
        ProjectExecutionRole::Test => (&snapshot.test_program, snapshot.manifest.test_module()),
    };
    if program.module != module {
        return Err(vec![guard_error(format!(
            "authenticated {role:?} closure module `{}` disagrees with manifest module `{module}`",
            program.module
        ))]);
    }
    let entry_id = program.entrypoint.as_str();
    let evaluated =
        interpreter::evaluate_resolved_zero_arg_i64(program, entry_id, options.max_steps)?;
    finish(snapshot, role, module, entry_id, evaluated, options)
}

fn finish(
    snapshot: &ProjectSnapshot,
    role: ProjectExecutionRole,
    module: &str,
    entry_id: &str,
    evaluated: ResolvedEvaluation,
    options: &ProjectExecutionOptions,
) -> Result<ProjectExecution, Vec<Diagnostic>> {
    let outcome = match evaluated.outcome {
        ResolvedEvaluationOutcome::ReturnedI64(value) => ProjectExecutionOutcome::Returned(value),
        ResolvedEvaluationOutcome::LanguageFailure(status) => {
            ProjectExecutionOutcome::LanguageFailure(status)
        }
        ResolvedEvaluationOutcome::FuelExhausted => ProjectExecutionOutcome::FuelExhausted,
        ResolvedEvaluationOutcome::CallDepthExceeded => ProjectExecutionOutcome::CallDepthExceeded,
        ResolvedEvaluationOutcome::GuardError(detail) => {
            return Err(vec![guard_error(format!(
                "authenticated project execution reached an impossible post-validation state: {detail}"
            ))]);
        }
    };
    let envelope = render(
        snapshot.manifest.schema(),
        snapshot.project_revision(),
        snapshot.workspace_revision(),
        snapshot.manifest.name(),
        role,
        module,
        entry_id,
        evaluated.steps_used,
        evaluated.max_steps,
        options.max_bytes,
        &outcome,
    )?;
    Ok(ProjectExecution {
        role,
        module: module.to_owned(),
        stable_id: entry_id.to_owned(),
        outcome,
        steps_used: evaluated.steps_used,
        max_steps: evaluated.max_steps,
        envelope,
    })
}

#[allow(clippy::too_many_arguments)]
fn render(
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
    let (envelope, overflowed) = with_limit(max_bytes, || {
        let outcome = match outcome {
            ProjectExecutionOutcome::Returned(value) => budgeted_format(format_args!(
                "{{\"kind\":\"returned\",\"type\":\"i64\",\"value\":{}}}",
                quote_json(&value.to_string())
            )),
            ProjectExecutionOutcome::LanguageFailure(status) => budgeted_format(format_args!(
                "{{\"kind\":\"language_failure\",\"status\":{}}}",
                status.to_json()
            )),
            ProjectExecutionOutcome::FuelExhausted => {
                budgeted_format(format_args!("{{\"kind\":\"fuel_exhausted\"}}"))
            }
            ProjectExecutionOutcome::CallDepthExceeded => {
                budgeted_format(format_args!("{{\"kind\":\"call_depth_exceeded\"}}"))
            }
        };
        let payload = budgeted_format(format_args!(
            "{{\"schema\":{},\"project_schema\":{},\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"role\":{},\"module\":{},\"stable_id\":{},\"limits\":{{\"max_bytes\":{},\"max_steps\":{}}},\"fuel\":{{\"steps_used\":{},\"max_steps\":{}}},\"outcome\":{},\"nonclaims\":[{}]}}",
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

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
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
    require_keys(
        object,
        &[
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
        ],
        "envelope",
    )?;
    require_text_eq(object, "schema", PROJECT_EXECUTION_SCHEMA)?;
    let project_schema = require_text(object, "project_schema")?;
    if !matches!(
        project_schema,
        PROJECT_SCHEMA | PROJECT_SCHEMA_V2 | PROJECT_SCHEMA_V3 | PROJECT_SCHEMA_V4
    ) {
        return Err(verification_error(
            "project_schema must name an admitted Project v1, v2, v3, or v4 schema".to_owned(),
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
    let role = match require_text(object, "role")? {
        "entry" => ProjectExecutionRole::Entry,
        "test" => ProjectExecutionRole::Test,
        _ => {
            return Err(verification_error(
                "envelope role must be exactly `entry` or `test`".to_owned(),
            ))
        }
    };

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
    let outcome = verify_outcome(outcome_object, steps_used, max_steps)?;

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

    let reconstructed = render(
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

fn verify_outcome(
    outcome: &serde_json::Map<String, serde_json::Value>,
    steps_used: usize,
    max_steps: usize,
) -> Result<ProjectExecutionOutcome, Diagnostic> {
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
            Ok(ProjectExecutionOutcome::Returned(value))
        }
        "language_failure" => {
            require_keys(outcome, &["kind", "status"], "language_failure outcome")?;
            Ok(ProjectExecutionOutcome::LanguageFailure(verify_status(
                &outcome["status"],
            )?))
        }
        "fuel_exhausted" => {
            require_keys(outcome, &["kind"], "fuel_exhausted outcome")?;
            if steps_used != max_steps {
                return Err(verification_error(
                    "fuel_exhausted requires steps_used equal to max_steps".to_owned(),
                ));
            }
            Ok(ProjectExecutionOutcome::FuelExhausted)
        }
        "call_depth_exceeded" => {
            require_keys(outcome, &["kind"], "call_depth_exceeded outcome")?;
            Ok(ProjectExecutionOutcome::CallDepthExceeded)
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

fn guard_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-F105", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT_REVISION: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    const WORKSPACE_REVISION: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";

    fn canonical_return(max_bytes: usize) -> String {
        render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            "calculator",
            ProjectExecutionRole::Entry,
            "calculator.app",
            "calculator.app.main",
            7,
            100,
            max_bytes,
            &ProjectExecutionOutcome::Returned(42),
        )
        .unwrap()
    }

    fn rendered_return(value: i64) -> serde_json::Value {
        let envelope = render(
            PROJECT_SCHEMA,
            "sha256:project",
            "sha256:workspace",
            "calculator",
            ProjectExecutionRole::Entry,
            "calculator.app",
            "calculator.app.main",
            7,
            100,
            65_536,
            &ProjectExecutionOutcome::Returned(value),
        )
        .unwrap();
        let marker = ",\"payload_digest\":";
        let offset = envelope.rfind(marker).unwrap();
        let payload = format!("{}}}", &envelope[..offset]);
        let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        assert_eq!(
            parsed["payload_digest"],
            domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())
        );
        parsed
    }

    #[test]
    fn returned_i64_extremes_are_lossless_decimal_strings() {
        assert_eq!(
            rendered_return(i64::MIN)["outcome"]["value"],
            i64::MIN.to_string()
        );
        assert_eq!(
            rendered_return(i64::MAX)["outcome"]["value"],
            i64::MAX.to_string()
        );
    }

    #[test]
    fn rendering_is_fail_closed_when_the_bound_cannot_hold_the_envelope() {
        let outcome = ProjectExecutionOutcome::Returned(42);
        assert_eq!(
            render(
                PROJECT_SCHEMA,
                "sha256:project",
                "sha256:workspace",
                "calculator",
                ProjectExecutionRole::Entry,
                "calculator.app",
                "calculator.app.main",
                7,
                100,
                1,
                &outcome,
            )
            .unwrap_err()[0]
                .code,
            "SPX-F104"
        );
    }

    #[test]
    fn complete_envelope_is_a_frozen_kat_and_independently_verifies() {
        let envelope = canonical_return(65_536);
        let expected = "{\"schema\":\"semaprax.project-execution.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":\"calculator\",\"project_revision\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"workspace_revision\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",\"role\":\"entry\",\"module\":\"calculator.app\",\"stable_id\":\"calculator.app.main\",\"limits\":{\"max_bytes\":65536,\"max_steps\":100},\"fuel\":{\"steps_used\":7,\"max_steps\":100},\"outcome\":{\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"42\"},\"nonclaims\":[\"in_process_reference_interpreter_only\",\"no_target_execution\",\"no_filesystem_process_or_backend_authority\",\"no_test_discovery\",\"no_cache_or_persistence\"],\"payload_digest\":\"sha256:b47dba4ff0d97550ee68f7879b0bcbf810d9e2ea60c50ac35f0f283a56d7ef61\"}";
        assert_eq!(envelope, expected);
        verify_execution_envelope(&envelope).unwrap();
    }

    #[test]
    fn verifier_rejects_noncanonical_confused_and_mutated_envelopes() {
        let envelope = canonical_return(65_536);
        let mutations = [
            format!(" {envelope}"),
            format!("{envelope}\n"),
            envelope.replacen(
                "{\"schema\":\"semaprax.project-execution.v1\",\"project_schema\":\"semaprax.project.v1\"",
                "{\"project_schema\":\"semaprax.project.v1\",\"schema\":\"semaprax.project-execution.v1\"",
                1,
            ),
            envelope.replacen(
                "{\"schema\":",
                "{\"unknown\":false,\"schema\":",
                1,
            ),
            envelope.replacen(
                "{\"schema\":",
                "{\"schema\":\"semaprax.project-execution.v1\",\"schema\":",
                1,
            ),
            envelope.replacen("\"role\":\"entry\"", "\"role\":\"test\"", 1),
            envelope.replacen(
                "semaprax.project-execution.v1",
                "semaprax.project.v1",
                1,
            ),
            envelope.replacen("\"steps_used\":7", "\"steps_used\":101", 1),
            envelope.replacen(
                "\"no_target_execution\"",
                "\"target_execution\"",
                1,
            ),
        ];
        for mutation in mutations {
            assert!(
                verify_execution_envelope(&mutation).is_err(),
                "mutation unexpectedly verified: {mutation}"
            );
        }
    }

    #[test]
    fn verifier_reconstructs_the_closed_status_table() {
        let status = runtime_status::normalize_arithmetic(StatusCase::DivisionByZero);
        let envelope = render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            "calculator",
            ProjectExecutionRole::Entry,
            "calculator.app",
            "calculator.app.main",
            9,
            100,
            65_536,
            &ProjectExecutionOutcome::LanguageFailure(status),
        )
        .unwrap();
        verify_execution_envelope(&envelope).unwrap();
        assert!(
            verify_execution_envelope(&envelope.replacen("\"code\":4", "\"code\":9", 1)).is_err()
        );
        assert!(verify_execution_envelope(&envelope.replacen(
            "\"class\":\"arithmetic\"",
            "\"class\":\"contract\"",
            1
        ))
        .is_err());

        let external = NormalizedStatus::try_new(
            "host.failure.v1",
            7,
            crate::conformance::StatusClass::Import,
            crate::conformance::Retryability::Known(false),
        )
        .unwrap();
        let confused = render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            "calculator",
            ProjectExecutionRole::Entry,
            "calculator.app",
            "calculator.app.main",
            9,
            100,
            65_536,
            &ProjectExecutionOutcome::LanguageFailure(external),
        )
        .unwrap();
        assert!(verify_execution_envelope(&confused).is_err());
    }

    #[test]
    fn verifier_rejects_self_consistent_but_impossible_semantic_facts() {
        let premature_exhaustion = render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            "calculator",
            ProjectExecutionRole::Entry,
            "calculator.app",
            "calculator.app.main",
            99,
            100,
            65_536,
            &ProjectExecutionOutcome::FuelExhausted,
        )
        .unwrap();
        assert!(verify_execution_envelope(&premature_exhaustion).is_err());

        let invalid_project = render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            "Calculator",
            ProjectExecutionRole::Entry,
            "calculator..app",
            "calculator.app.main",
            7,
            100,
            65_536,
            &ProjectExecutionOutcome::Returned(42),
        )
        .unwrap();
        assert!(verify_execution_envelope(&invalid_project).is_err());

        let oversized = " ".repeat(graph::MAX_AGENT_CONTEXT_BYTES + 1);
        assert!(verify_execution_envelope(&oversized).is_err());
    }

    #[test]
    fn rendering_and_verification_honor_the_exact_max_bytes_boundary() {
        let project = "p".repeat(MAX_NAME_BYTES);
        let module = "m".repeat(MAX_MODULE_BYTES);
        let stable_id = "s".repeat(MAX_STABLE_ID_BYTES);
        let outcome = ProjectExecutionOutcome::Returned(i64::MIN);
        let mut low = graph::MIN_AGENT_CONTEXT_BYTES;
        let mut high = graph::MAX_AGENT_CONTEXT_BYTES;
        while low < high {
            let midpoint = low + (high - low) / 2;
            if render(
                PROJECT_SCHEMA,
                PROJECT_REVISION,
                WORKSPACE_REVISION,
                &project,
                ProjectExecutionRole::Entry,
                &module,
                &stable_id,
                100,
                100,
                midpoint,
                &outcome,
            )
            .is_ok()
            {
                high = midpoint;
            } else {
                low = midpoint + 1;
            }
        }
        let exact_limit = low;
        let exact = render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            &project,
            ProjectExecutionRole::Entry,
            &module,
            &stable_id,
            100,
            100,
            exact_limit,
            &outcome,
        )
        .unwrap();
        verify_execution_envelope(&exact).unwrap();
        assert!(render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            &project,
            ProjectExecutionRole::Entry,
            &module,
            &stable_id,
            100,
            100,
            exact_limit - 1,
            &outcome,
        )
        .is_err());
    }
}
