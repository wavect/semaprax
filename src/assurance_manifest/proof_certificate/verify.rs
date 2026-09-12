//! Independent, filesystem-free structural replay of one certificate
//! ([`verify_certificate`]), plus two filesystem-touching checks that rebind
//! it to current source bytes: a pure re-derivation
//! ([`verify_certificate_against_source`]) and, strictly opt-in, a real
//! solver rerun of the exact embedded script
//! ([`verify_certificate_with_solver`]).
//!
//! See the parent module doc for why re-deriving the script from source
//! (rather than trusting the certificate's own recorded verdict) is the
//! specific defense against a translator regression or a hand-edited script
//! smuggling in an unsound claim.

use std::path::Path;

use serde_json::Value;

use crate::diagnostic::Diagnostic;
use crate::graph;

use super::super::smt_discharge::{
    self, render_postcondition_script, replay_function, run, translate_function, Model, ModelValue,
    Provisioning, ReplayOutcome, RunLimits, Verdict,
};
use super::render::{payload_digest, script_digest, source_digest, SCHEMA};

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z106", message)
}

fn drift_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z107", message)
}

fn is_sha256_wire_form(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn object_keys(value: &Value, context: &str) -> Result<Vec<String>, Diagnostic> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .ok_or_else(|| consistency_error(format!("{context} must be a JSON object")))
}

fn require_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic> {
    value
        .as_str()
        .ok_or_else(|| consistency_error(format!("`{field}` must be a string")))
}

fn require_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| consistency_error(format!("`{field}` must be an array")))
}

fn check_exact_keys(
    mut found: Vec<String>,
    expected: &[&str],
    context: &str,
) -> Result<(), Diagnostic> {
    found.sort();
    let mut expected_sorted: Vec<&str> = expected.to_vec();
    expected_sorted.sort_unstable();
    if found
        .iter()
        .map(String::as_str)
        .ne(expected_sorted.iter().copied())
    {
        return Err(consistency_error(format!(
            "{context} keys must be exactly {expected_sorted:?}, found {found:?}"
        )));
    }
    Ok(())
}

const PAYLOAD_KEYS: [&str; 14] = [
    "bounds",
    "compiler_version",
    "counterexample",
    "declaration_id",
    "ensures_index",
    "limits",
    "nonclaims",
    "obligation_id",
    "schema",
    "script",
    "script_sha256",
    "solver",
    "source",
    "verdict",
];

pub(super) enum CheckedBody {
    Proved,
    Refuted {
        model: Model,
        outcome: ReplayOutcome,
    },
}

pub(super) struct CheckedCertificate {
    pub declaration_id: String,
    pub ensures_index: usize,
    pub compiler_version: String,
    pub timeout_ms: u64,
    pub revision: String,
    pub source_sha256: String,
    pub script: String,
    pub body: CheckedBody,
}

fn check_model_entries(entries: &[Value]) -> Result<Model, Diagnostic> {
    let mut model = Model::new();
    let mut previous_name: Option<String> = None;
    for entry in entries {
        check_exact_keys(
            object_keys(entry, "counterexample.model[]")?,
            &["name", "sort", "value"],
            "counterexample.model[]",
        )?;
        let name = require_string(&entry["name"], "model.name")?.to_owned();
        if let Some(previous) = &previous_name {
            if name.as_str() <= previous.as_str() {
                return Err(consistency_error(
                    "counterexample.model entries must be in strict ascending `name` order"
                        .to_owned(),
                ));
            }
        }
        let sort = require_string(&entry["sort"], "model.sort")?;
        let value_text = require_string(&entry["value"], "model.value")?;
        let value = match sort {
            "int" => value_text
                .parse::<i128>()
                .map(ModelValue::Int)
                .map_err(|_| {
                    consistency_error(format!(
                    "model entry `{name}` has a non-integer value `{value_text}` for sort `int`"
                ))
                })?,
            "bool" => match value_text {
                "true" => ModelValue::Bool(true),
                "false" => ModelValue::Bool(false),
                other => {
                    return Err(consistency_error(format!(
                        "model entry `{name}` has a non-boolean value `{other}` for sort `bool`"
                    )))
                }
            },
            other => {
                return Err(consistency_error(format!(
                    "model entry `{name}` has an unrecognized sort `{other}`"
                )))
            }
        };
        model.insert(name.clone(), value);
        previous_name = Some(name);
    }
    Ok(model)
}

fn check_counterexample(
    verdict_token: &str,
    counterexample: &Value,
) -> Result<CheckedBody, Diagnostic> {
    match verdict_token {
        "proved" => {
            if !counterexample.is_null() {
                return Err(consistency_error(
                    "verdict `proved` must carry a null counterexample".to_owned(),
                ));
            }
            Ok(CheckedBody::Proved)
        }
        "refuted" => {
            if counterexample.is_null() {
                return Err(consistency_error(
                    "verdict `refuted` must carry a non-null counterexample".to_owned(),
                ));
            }
            check_exact_keys(
                object_keys(counterexample, "payload.counterexample")?,
                &["detail", "ensures_index", "kind", "model"],
                "payload.counterexample",
            )?;
            let kind = require_string(&counterexample["kind"], "counterexample.kind")?;
            let outcome = match kind {
                "trapped" => {
                    let detail =
                        require_string(&counterexample["detail"], "counterexample.detail")?
                            .to_owned();
                    if !counterexample["ensures_index"].is_null() {
                        return Err(consistency_error(
                            "counterexample.ensures_index must be null for kind `trapped`"
                                .to_owned(),
                        ));
                    }
                    ReplayOutcome::Trapped { detail }
                }
                "ensures_violated" => {
                    if !counterexample["detail"].is_null() {
                        return Err(consistency_error(
                            "counterexample.detail must be null for kind `ensures_violated`"
                                .to_owned(),
                        ));
                    }
                    let recorded_index =
                        counterexample["ensures_index"].as_u64().ok_or_else(|| {
                            consistency_error(
                            "counterexample.ensures_index must be an unsigned integer for kind \
                             `ensures_violated`"
                                .to_owned(),
                        )
                        })? as usize;
                    ReplayOutcome::EnsuresViolated {
                        ensures_index: recorded_index,
                    }
                }
                other => {
                    return Err(consistency_error(format!(
                        "counterexample kind `{other}` is outside the closed vocabulary"
                    )))
                }
            };
            let entries = require_array(&counterexample["model"], "counterexample.model")?;
            let model = check_model_entries(entries)?;
            Ok(CheckedBody::Refuted { model, outcome })
        }
        other => Err(consistency_error(format!(
            "verdict `{other}` is outside the closed vocabulary"
        ))),
    }
}

/// Independently verify one certificate produced by
/// [`super::export_postcondition_certificate`]. Touches no filesystem: this
/// only replays the certificate's own internal consistency (digest, byte
/// counts, closed vocabularies, and the `proved`/`refuted` verdict's
/// coherence with its `counterexample` field), and cannot by itself confirm
/// the certificate was produced from any particular source — see
/// [`verify_certificate_against_source`] for that.
pub fn verify_certificate(certificate: &str) -> Result<(), Diagnostic> {
    check_certificate(certificate)?;
    Ok(())
}

fn check_certificate(certificate: &str) -> Result<CheckedCertificate, Diagnostic> {
    let value: Value = serde_json::from_str(certificate)
        .map_err(|error| consistency_error(format!("certificate is not valid JSON: {error}")))?;
    check_exact_keys(
        object_keys(&value, "certificate")?,
        &["bytes", "digest", "payload", "schema"],
        "certificate",
    )?;
    if value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "certificate schema must be {SCHEMA}"
        )));
    }
    let envelope_digest = require_string(&value["digest"], "digest")?;
    if !is_sha256_wire_form(envelope_digest) {
        return Err(consistency_error(
            "certificate digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    let declared_bytes = value["bytes"].as_u64().ok_or_else(|| {
        consistency_error("certificate `bytes` must be an unsigned integer".to_owned())
    })?;

    const PAYLOAD_KEY: &str = "\"payload\":";
    let offset = certificate
        .find(PAYLOAD_KEY)
        .ok_or_else(|| consistency_error("certificate is missing its payload member".to_owned()))?;
    if !certificate.ends_with('}') {
        return Err(consistency_error(
            "certificate must end with `}`".to_owned(),
        ));
    }
    let payload = &certificate[offset + PAYLOAD_KEY.len()..certificate.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "certificate payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "certificate declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    let recomputed = payload_digest(payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "certificate digest does not match the exact payload bytes".to_owned(),
        ));
    }

    let payload_value: Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    check_exact_keys(
        object_keys(&payload_value, "payload")?,
        &PAYLOAD_KEYS,
        "payload",
    )?;
    if payload_value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }

    check_exact_keys(
        object_keys(&payload_value["source"], "payload.source")?,
        &["path", "revision", "sha256"],
        "payload.source",
    )?;
    require_string(&payload_value["source"]["path"], "source.path")?;
    let revision =
        require_string(&payload_value["source"]["revision"], "source.revision")?.to_owned();
    let source_sha256 =
        require_string(&payload_value["source"]["sha256"], "source.sha256")?.to_owned();
    if !is_sha256_wire_form(&source_sha256) {
        return Err(consistency_error(
            "source.sha256 must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }

    let declaration_id =
        require_string(&payload_value["declaration_id"], "declaration_id")?.to_owned();
    if declaration_id.is_empty() {
        return Err(consistency_error(
            "declaration_id must not be empty".to_owned(),
        ));
    }

    let ensures_index = payload_value["ensures_index"]
        .as_u64()
        .ok_or_else(|| consistency_error("ensures_index must be an unsigned integer".to_owned()))?
        as usize;

    let obligation_id = require_string(&payload_value["obligation_id"], "obligation_id")?;
    let expected_obligation_id =
        smt_discharge::postcondition_obligation_id(&declaration_id, ensures_index);
    if obligation_id != expected_obligation_id {
        return Err(consistency_error(
            "obligation_id does not match declaration_id/ensures_index under the shared \
             obligation-id scheme"
                .to_owned(),
        ));
    }

    let compiler_version =
        require_string(&payload_value["compiler_version"], "compiler_version")?.to_owned();
    if compiler_version.is_empty() {
        return Err(consistency_error(
            "compiler_version must not be empty".to_owned(),
        ));
    }

    if payload_value["bounds"].as_str() != Some(smt_discharge::BOUNDS_V1) {
        return Err(consistency_error(
            "bounds does not match the installed bounded SMT-discharge subset description"
                .to_owned(),
        ));
    }

    check_exact_keys(
        object_keys(&payload_value["solver"], "payload.solver")?,
        &["identity", "version"],
        "payload.solver",
    )?;
    require_string(&payload_value["solver"]["identity"], "solver.identity")?;
    require_string(&payload_value["solver"]["version"], "solver.version")?;

    check_exact_keys(
        object_keys(&payload_value["limits"], "payload.limits")?,
        &["max_output_bytes", "timeout_ms"],
        "payload.limits",
    )?;
    let timeout_ms = payload_value["limits"]["timeout_ms"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error("limits.timeout_ms must be an unsigned integer".to_owned())
        })?;
    payload_value["limits"]["max_output_bytes"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error("limits.max_output_bytes must be an unsigned integer".to_owned())
        })?;

    let script = require_string(&payload_value["script"], "script")?.to_owned();
    let recorded_script_sha256 = require_string(&payload_value["script_sha256"], "script_sha256")?;
    if script_digest(&script) != recorded_script_sha256 {
        return Err(consistency_error(
            "script_sha256 does not match the exact embedded script bytes".to_owned(),
        ));
    }

    let verdict_token = require_string(&payload_value["verdict"], "verdict")?;
    let body = check_counterexample(verdict_token, &payload_value["counterexample"])?;

    require_array(&payload_value["nonclaims"], "nonclaims")?;

    Ok(CheckedCertificate {
        declaration_id,
        ensures_index,
        compiler_version,
        timeout_ms,
        revision,
        source_sha256,
        script,
        body,
    })
}

/// Verify one certificate, then additionally re-derive its claim from
/// `source_path`'s **current** bytes rather than trusting anything the
/// certificate itself recorded beyond its own internal consistency:
///
/// - the current source's digest and semantic revision must match what the
///   certificate was bound to (fails closed on drift, `SPX-Z107`);
/// - the named declaration must still exist, still be inside the bounded
///   SMT-discharge subset, and still have an `ensures` clause at the
///   recorded index;
/// - re-rendering that declaration's postcondition script with the exact
///   recorded timeout must produce **byte-identical** text to the
///   certificate's embedded script (`SPX-Z106` otherwise) — this is the
///   check that catches a hand-edited or stale script, including one
///   reproducing the exact "give `result` an unconditional range axiom"
///   bug class `docs/SMT-DISCHARGE-V1.md` documents;
/// - for a `refuted` certificate, independently replaying the recorded
///   model against the current declaration via
///   [`smt_discharge::replay_function`] (which shares no code with the
///   translator) must reproduce the exact recorded counterexample outcome.
///
/// A `proved` verdict's `unsat` claim itself is *not* re-checked here — that
/// requires a real solver, which this function never spawns; see
/// [`verify_certificate_with_solver`].
pub fn verify_certificate_against_source(
    certificate: &str,
    source_path: &Path,
) -> Result<(), Diagnostic> {
    let checked = check_certificate(certificate)?;
    if checked.compiler_version != env!("CARGO_PKG_VERSION") {
        return Err(drift_error(format!(
            "certificate was produced by compiler version `{}`, but this replay is running \
             compiler version `{}`; toolchain version drift can change accepted proofs, so this \
             certificate must be regenerated rather than trusted across a version change",
            checked.compiler_version,
            env!("CARGO_PKG_VERSION"),
        )));
    }
    let current_bytes = std::fs::read(source_path)
        .map_err(|error| drift_error(format!("cannot read {}: {error}", source_path.display())))?;
    let current_source = String::from_utf8_lossy(&current_bytes).into_owned();
    if source_digest(&current_source) != checked.source_sha256 {
        return Err(drift_error(
            "proof certificate source digest does not match the current source bytes; the \
             source drifted after the certificate was generated"
                .to_owned(),
        ));
    }

    let program = crate::parse(&current_source, source_path).map_err(|_| {
        drift_error(
            "current source no longer parses; the certificate cannot be replayed".to_owned(),
        )
    })?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(drift_error(
            "current source no longer passes verification; the certificate cannot be replayed"
                .to_owned(),
        ));
    }
    if graph::revision(&program) != checked.revision {
        return Err(drift_error(
            "current source's semantic revision no longer matches the certificate; the source \
             drifted after the certificate was generated"
                .to_owned(),
        ));
    }

    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == checked.declaration_id)
        .ok_or_else(|| {
            drift_error(format!(
                "declaration `{}` is no longer present in the current source",
                checked.declaration_id
            ))
        })?;
    let encoding = translate_function(function).map_err(|reason| {
        drift_error(format!(
            "declaration `{}` is no longer inside the bounded SMT-discharge subset ({}): {}",
            checked.declaration_id,
            reason.code(),
            reason.detail()
        ))
    })?;
    if checked.ensures_index >= encoding.ensures.len() {
        return Err(drift_error(
            "ensures_index is out of range for the current declaration".to_owned(),
        ));
    }

    let recomputed_script =
        render_postcondition_script(&encoding, checked.ensures_index, checked.timeout_ms);
    if recomputed_script != checked.script {
        return Err(consistency_error(
            "the embedded SMT-LIB2 script is not the deterministic translation of the exact \
             current source and ensures clause; independent replay refuses to accept it (this \
             is exactly the check that would catch a tampered or stale script, including one \
             smuggling in an unconditional range axiom for `result` and vacuously proving a \
             false postcondition)"
                .to_owned(),
        ));
    }

    match checked.body {
        CheckedBody::Proved => Ok(()),
        CheckedBody::Refuted { model, outcome } => {
            let replayed = replay_function(function, &model).map_err(|error| {
                consistency_error(format!("independent replay failed to evaluate: {error}"))
            })?;
            if replayed != outcome {
                return Err(consistency_error(
                    "independent checked-arithmetic replay does not reproduce the certificate's \
                     recorded counterexample outcome for the current source"
                        .to_owned(),
                ));
            }
            Ok(())
        }
    }
}

/// Everything [`verify_certificate_against_source`] checks, plus — only for
/// a `proved` verdict, and only because the caller explicitly supplied a
/// solver — re-running the certificate's exact embedded script through
/// `provisioning` and requiring the fresh result to also be `unsat`.
///
/// This never spawns a process for a `refuted` certificate: its
/// counterexample is already independently validated by checked-arithmetic
/// replay above, which is strictly stronger evidence than a second `sat`
/// from any solver. Spawns a process only when this function is called with
/// an explicit `provisioning`; never consults the environment itself.
pub fn verify_certificate_with_solver(
    certificate: &str,
    source_path: &Path,
    provisioning: &Provisioning,
    limits: &RunLimits,
) -> Result<(), Diagnostic> {
    verify_certificate_against_source(certificate, source_path)?;
    let checked = check_certificate(certificate)?;
    if let CheckedBody::Proved = checked.body {
        match run(provisioning, &checked.script, limits) {
            Verdict::Unsat => Ok(()),
            other => Err(consistency_error(format!(
                "re-running the certificate's exact embedded script through the provisioned \
                 solver did not reproduce `unsat`: {other:?}"
            ))),
        }
    } else {
        Ok(())
    }
}
