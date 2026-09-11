//! Independent, filesystem-free structural replay of one envelope, plus the
//! one filesystem-touching check that rebinds it to current source bytes.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "Drift and fail-closed replay" and "Diagnostics".

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::diagnostic::Diagnostic;

use super::lattice::{classification_of, AssuranceClass};
use super::obligation::ObligationKind;
use super::render::{payload_digest, source_digest, SCHEMA};

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z103", message)
}

fn drift_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z104", message)
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

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
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

fn require_bool(value: &Value, field: &str) -> Result<bool, Diagnostic> {
    value
        .as_bool()
        .ok_or_else(|| consistency_error(format!("`{field}` must be a boolean")))
}

fn require_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| consistency_error(format!("`{field}` must be an array")))
}

fn require_string_or_null<'a>(
    value: &'a Value,
    field: &str,
) -> Result<Option<&'a str>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(require_string(value, field)?))
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, Diagnostic> {
    require_array(value, field)?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| consistency_error(format!("`{field}` entries must be strings")))
        })
        .collect()
}

const OBLIGATION_KEYS: [&str; 6] = [
    "assumption_ids",
    "classification",
    "declaration_id",
    "id",
    "kind",
    "methods",
];

const METHOD_KEYS: [&str; 13] = [
    "artifact_digest",
    "assumption_ids",
    "bounds",
    "class",
    "counterexample_ref",
    "detail",
    "inputs",
    "proof_ref",
    "runtime_fallback",
    "target",
    "test_refs",
    "tool",
    "tool_version",
];

const ASSUMPTION_KEYS: [&str; 6] = [
    "dependents",
    "id",
    "owner",
    "rationale",
    "review_by",
    "scope",
];

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

struct CheckedMethod {
    class: AssuranceClass,
    assumption_ids: Vec<String>,
}

fn check_method(value: &Value) -> Result<CheckedMethod, Diagnostic> {
    check_exact_keys(object_keys(value, "method")?, &METHOD_KEYS, "method")?;
    let class_token = require_string(&value["class"], "class")?;
    let class = AssuranceClass::from_token(class_token).ok_or_else(|| {
        consistency_error(format!(
            "method class `{class_token}` is outside the closed vocabulary"
        ))
    })?;
    require_string(&value["tool"], "tool")?;
    require_string(&value["tool_version"], "tool_version")?;
    string_array(&value["inputs"], "inputs")?;
    require_string_or_null(&value["bounds"], "bounds")?;
    let assumption_ids = string_array(&value["assumption_ids"], "assumption_ids")?;
    let proof_ref = require_string_or_null(&value["proof_ref"], "proof_ref")?;
    require_string_or_null(&value["counterexample_ref"], "counterexample_ref")?;
    require_bool(&value["runtime_fallback"], "runtime_fallback")?;
    string_array(&value["test_refs"], "test_refs")?;
    require_string_or_null(&value["target"], "target")?;
    require_string_or_null(&value["artifact_digest"], "artifact_digest")?;
    require_string_or_null(&value["detail"], "detail")?;

    // "A failed solver run remains evidence about an attempt, not a
    // successful classification": a method that has not reached a
    // successful classification (open/assumed/attempt_inconclusive) can
    // never simultaneously carry a positive proof reference.
    if matches!(
        class,
        AssuranceClass::Open | AssuranceClass::Assumed | AssuranceClass::AttemptInconclusive
    ) && proof_ref.is_some()
    {
        return Err(consistency_error(format!(
            "method class `{class_token}` cannot carry a `proof_ref`; \
             an attempt that did not reach a positive verdict has no proof"
        )));
    }
    Ok(CheckedMethod {
        class,
        assumption_ids,
    })
}

struct CheckedObligation {
    id: String,
    assumption_ids: Vec<String>,
}

fn check_obligation(
    value: &Value,
    all_assumption_ids: &BTreeSet<String>,
) -> Result<CheckedObligation, Diagnostic> {
    check_exact_keys(
        object_keys(value, "obligation")?,
        &OBLIGATION_KEYS,
        "obligation",
    )?;
    let id = require_string(&value["id"], "id")?.to_owned();
    if id.is_empty() {
        return Err(consistency_error(
            "obligation `id` must not be empty".to_owned(),
        ));
    }
    let declaration_id = require_string(&value["declaration_id"], "declaration_id")?;
    if declaration_id.is_empty() {
        return Err(consistency_error(
            "obligation `declaration_id` must not be empty".to_owned(),
        ));
    }
    let kind_token = require_string(&value["kind"], "kind")?;
    ObligationKind::from_token(kind_token).ok_or_else(|| {
        consistency_error(format!(
            "obligation kind `{kind_token}` is outside the closed vocabulary"
        ))
    })?;
    let classification_token = require_string(&value["classification"], "classification")?;
    let declared_classification =
        AssuranceClass::from_token(classification_token).ok_or_else(|| {
            consistency_error(format!(
            "obligation classification `{classification_token}` is outside the closed vocabulary"
        ))
        })?;

    let methods = require_array(&value["methods"], "methods")?;
    let mut classes = Vec::with_capacity(methods.len());
    let mut method_assumption_ids: Vec<String> = Vec::new();
    for method in methods {
        let checked = check_method(method)?;
        classes.push(checked.class);
        method_assumption_ids.extend(checked.assumption_ids);
    }
    let expected_classification = classification_of(&classes);
    if expected_classification != declared_classification {
        return Err(consistency_error(format!(
            "obligation `{id}` declares classification `{classification_token}` but its own methods \
             re-derive `{}`; a classification can never outrun what its methods actually prove",
            expected_classification.token()
        )));
    }

    let assumption_ids = string_array(&value["assumption_ids"], "assumption_ids")?;
    let mut expected_assumption_ids: Vec<String> = method_assumption_ids;
    expected_assumption_ids.sort();
    expected_assumption_ids.dedup();
    if assumption_ids != expected_assumption_ids {
        return Err(consistency_error(format!(
            "obligation `{id}` `assumption_ids` must be exactly the sorted, de-duplicated union of \
             its methods' `assumption_ids`"
        )));
    }
    for referenced in &assumption_ids {
        if !all_assumption_ids.contains(referenced) {
            return Err(consistency_error(format!(
                "obligation `{id}` references assumption `{referenced}`, which is not present in `assumptions`"
            )));
        }
    }
    Ok(CheckedObligation { id, assumption_ids })
}

fn check_assumption(value: &Value) -> Result<String, Diagnostic> {
    check_exact_keys(
        object_keys(value, "assumption")?,
        &ASSUMPTION_KEYS,
        "assumption",
    )?;
    let id = require_string(&value["id"], "id")?.to_owned();
    if id.is_empty() {
        return Err(consistency_error(
            "assumption `id` must not be empty".to_owned(),
        ));
    }
    require_string(&value["owner"], "owner")?;
    require_string(&value["rationale"], "rationale")?;
    require_string(&value["scope"], "scope")?;
    if let Some(review_by) = require_string_or_null(&value["review_by"], "review_by")? {
        if !is_iso_date(review_by) {
            return Err(consistency_error(format!(
                "assumption `review_by` must be an ISO-8601 `YYYY-MM-DD` date, found `{review_by}`"
            )));
        }
    }
    string_array(&value["dependents"], "dependents")?;
    Ok(id)
}

fn check_strict_ascending(ids: &[String], context: &str) -> Result<(), Diagnostic> {
    for window in ids.windows(2) {
        if window[0] >= window[1] {
            return Err(consistency_error(format!(
                "{context} must be in strict ascending `id` order; canonical order is never repaired downstream"
            )));
        }
    }
    Ok(())
}

/// Independently verify one envelope produced by [`super::generate`]. Touches
/// no filesystem.
pub fn verify_envelope(envelope: &str) -> Result<(), Diagnostic> {
    let value: Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    check_exact_keys(
        object_keys(&value, "envelope")?,
        &["bytes", "digest", "payload", "schema"],
        "envelope",
    )?;
    if value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "envelope schema must be {SCHEMA}"
        )));
    }
    let envelope_digest = require_string(&value["digest"], "digest")?;
    if !is_sha256_wire_form(envelope_digest) {
        return Err(consistency_error(
            "envelope digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    let declared_bytes = value["bytes"].as_u64().ok_or_else(|| {
        consistency_error("envelope `bytes` must be an unsigned integer".to_owned())
    })?;

    const PAYLOAD_KEY: &str = "\"payload\":";
    let offset = envelope
        .find(PAYLOAD_KEY)
        .ok_or_else(|| consistency_error("envelope is missing its payload member".to_owned()))?;
    if !envelope.ends_with('}') {
        return Err(consistency_error("envelope must end with `}`".to_owned()));
    }
    let payload = &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "envelope payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "envelope declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    let recomputed = payload_digest(payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }

    let payload_value: Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    check_exact_keys(
        object_keys(&payload_value, "payload")?,
        &[
            "assumptions",
            "counts",
            "limits",
            "nonclaims",
            "obligations",
            "schema",
            "source",
        ],
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
    require_string(&payload_value["source"]["revision"], "source.revision")?;
    let source_sha256 = require_string(&payload_value["source"]["sha256"], "source.sha256")?;
    if !is_sha256_wire_form(source_sha256) {
        return Err(consistency_error(
            "source.sha256 must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }

    let assumptions = require_array(&payload_value["assumptions"], "assumptions")?;
    let mut assumption_ids = Vec::with_capacity(assumptions.len());
    for assumption in assumptions {
        assumption_ids.push(check_assumption(assumption)?);
    }
    check_strict_ascending(&assumption_ids, "assumptions")?;
    let assumption_id_set: BTreeSet<String> = assumption_ids.iter().cloned().collect();
    if assumption_id_set.len() != assumption_ids.len() {
        return Err(consistency_error(
            "assumption ids must be unique".to_owned(),
        ));
    }

    let obligations = require_array(&payload_value["obligations"], "obligations")?;
    let mut obligation_ids = Vec::with_capacity(obligations.len());
    let mut class_counts: Vec<(AssuranceClass, usize)> = AssuranceClass::ALL
        .into_iter()
        .map(|class| (class, 0usize))
        .collect();
    for obligation in obligations {
        let checked = check_obligation(obligation, &assumption_id_set)?;
        let classification_token = obligation["classification"].as_str().unwrap_or_default();
        if let Some(class) = AssuranceClass::from_token(classification_token) {
            if let Some(slot) = class_counts
                .iter_mut()
                .find(|(candidate, _)| *candidate == class)
            {
                slot.1 += 1;
            }
        }
        obligation_ids.push(checked.id);
        let _ = checked.assumption_ids;
    }
    check_strict_ascending(&obligation_ids, "obligations")?;
    let obligation_id_set: BTreeSet<String> = obligation_ids.iter().cloned().collect();
    if obligation_id_set.len() != obligation_ids.len() {
        return Err(consistency_error(
            "obligation ids must be unique".to_owned(),
        ));
    }

    check_exact_keys(
        object_keys(&payload_value["counts"], "payload.counts")?,
        &["assumptions_total", "by_class", "obligations_total"],
        "payload.counts",
    )?;
    if payload_value["counts"]["obligations_total"].as_u64() != Some(obligation_ids.len() as u64) {
        return Err(consistency_error(
            "counts.obligations_total does not match the listed obligations".to_owned(),
        ));
    }
    if payload_value["counts"]["assumptions_total"].as_u64() != Some(assumption_ids.len() as u64) {
        return Err(consistency_error(
            "counts.assumptions_total does not match the listed assumptions".to_owned(),
        ));
    }
    let by_class = payload_value["counts"]["by_class"]
        .as_object()
        .ok_or_else(|| consistency_error("counts.by_class must be a JSON object".to_owned()))?;
    if by_class.len() != AssuranceClass::ALL.len() {
        return Err(consistency_error(
            "counts.by_class must have exactly one member per assurance class".to_owned(),
        ));
    }
    for (class, expected_count) in &class_counts {
        let found = by_class
            .get(class.token())
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                consistency_error(format!("counts.by_class is missing `{}`", class.token()))
            })?;
        if found != *expected_count as u64 {
            return Err(consistency_error(format!(
                "counts.by_class[`{}`] does not match the derived classifications",
                class.token()
            )));
        }
    }

    Ok(())
}

/// Verify one envelope and additionally bind the current bytes of
/// `source_path` to the embedded source digest, failing closed on drift.
pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<(), Diagnostic> {
    verify_envelope(envelope)?;
    let current = std::fs::read(source_path)
        .map_err(|error| drift_error(format!("cannot read {}: {error}", source_path.display())))?;
    let bound = bound_source_digest(envelope)?;
    if bound != source_digest(&String::from_utf8_lossy(&current)) {
        return Err(drift_error(
            "assurance manifest source digest does not match the current source bytes; \
             the source drifted after the manifest was generated"
                .to_owned(),
        ));
    }
    Ok(())
}

fn bound_source_digest(envelope: &str) -> Result<String, Diagnostic> {
    let value: Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    value["payload"]["source"]["sha256"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| consistency_error("payload.source.sha256 must be a string".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_sha256_wire_form_rejects_uppercase_and_wrong_length() {
        assert!(is_sha256_wire_form(&format!("sha256:{}", "0".repeat(64))));
        assert!(!is_sha256_wire_form(&format!("sha256:{}", "A".repeat(64))));
        assert!(!is_sha256_wire_form(&format!("sha256:{}", "0".repeat(63))));
        assert!(!is_sha256_wire_form("not-a-digest"));
    }

    #[test]
    fn is_iso_date_accepts_only_the_fixed_shape() {
        assert!(is_iso_date("2026-09-11"));
        assert!(!is_iso_date("2026/09/11"));
        assert!(!is_iso_date("26-09-11"));
        assert!(!is_iso_date("2026-09-1"));
    }
}
