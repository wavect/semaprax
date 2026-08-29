use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::bounded_output;
use crate::{package_lock_v2, package_report_v2};

use super::model::{Authenticated, Report};
use super::wire::{authentication_error, charge, digest, limit_error, required_str};
use super::{CompatibilityInput, INPUT_DOMAIN, MAX_INPUT_BYTES};

macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }

pub(super) fn authenticate(
    input: &CompatibilityInput,
    work: &mut usize,
    input_bytes: &mut usize,
) -> Result<Authenticated, crate::diagnostic::Diagnostic> {
    let added = input
        .report
        .len()
        .checked_add(input.lock.len())
        .and_then(|n| {
            input
                .lock_subjects
                .iter()
                .try_fold(n, |n, s| n.checked_add(s.len()))
        })
        .ok_or_else(|| limit_error("input byte overflow"))?;
    *input_bytes = input_bytes
        .checked_add(added)
        .ok_or_else(|| limit_error("input byte overflow"))?;
    if *input_bytes > MAX_INPUT_BYTES {
        return Err(limit_error("cumulative input bytes exceed limit"));
    }
    charge(work, input.lock_subjects.len().saturating_add(2))?;
    package_report_v2::verify_envelope(&input.report)
        .map_err(|_| authentication_error("loose v2 report replay failed"))?;
    let lock_value: Value =
        serde_json::from_str(&input.lock).map_err(|_| authentication_error("lock v2 not JSON"))?;
    let requested = lock_value["payload"]["limits"]["requested_max_bytes"]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| authentication_error("lock v2 options missing"))?;
    let lock_options = package_lock_v2::LockOptions::new(requested)
        .map_err(|_| authentication_error("lock v2 options invalid"))?;
    package_lock_v2::verify(&input.lock, &input.lock_subjects, &lock_options)
        .map_err(|_| authentication_error("lock v2 replay failed"))?;
    let mut selected = Vec::new();
    let mut hasher = Sha256::new();
    hasher.update(INPUT_DOMAIN);
    hasher.update((input.lock_subjects.len() as u64).to_le_bytes());
    let mut subject_bytes = 0usize;
    let mut ordered_subjects = input
        .lock_subjects
        .iter()
        .map(|subject| {
            let value: Value = serde_json::from_str(subject)
                .map_err(|_| authentication_error("authenticated subject not JSON"))?;
            Ok((
                value["payload"]["package"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned(),
                value["payload"]["version"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned(),
                subject,
            ))
        })
        .collect::<Result<Vec<_>, crate::diagnostic::Diagnostic>>()?;
    ordered_subjects.sort_by(|left, right| {
        (&left.0, &left.1, left.2.as_bytes()).cmp(&(&right.0, &right.1, right.2.as_bytes()))
    });
    for (_, _, subject) in ordered_subjects {
        subject_bytes = subject_bytes
            .checked_add(subject.len())
            .ok_or_else(|| limit_error("subject bytes overflow"))?;
        hasher.update((subject.len() as u64).to_le_bytes());
        hasher.update(subject.as_bytes());
        let value: Value = serde_json::from_str(subject)
            .map_err(|_| authentication_error("authenticated subject not JSON"))?;
        let payload = &value["payload"];
        let exact_report = exact_subject_report(subject)?;
        let replayed: Value = serde_json::from_str(exact_report)
            .map_err(|_| authentication_error("authenticated dependency report not JSON"))?;
        let report_payload = &replayed["payload"];
        let source_bytes = report_payload["source"]["bytes"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| authentication_error("dependency source byte fact missing"))?;
        let mut facts = report_payload["types"].as_array().map_or(0, Vec::len)
            + report_payload["targets"].as_array().map_or(0, Vec::len);
        for export in report_payload["exports"].as_array().into_iter().flatten() {
            facts = facts
                .saturating_add(1)
                .saturating_add(export["requires"].as_array().map_or(0, Vec::len))
                .saturating_add(export["ensures"].as_array().map_or(0, Vec::len));
        }
        charge(work, source_bytes.saturating_add(facts))?;
        if payload["package"].as_str() == Some(input.coordinate.package.as_str())
            && payload["version"].as_str() == Some(input.coordinate.version.as_str())
        {
            selected.push(exact_report.to_owned());
        }
    }
    if selected.len() != 1 || selected[0] != input.report {
        return Err(authentication_error(
            "loose report is not exact selected authenticated subject report",
        ));
    }
    let report_value: Value = serde_json::from_str(&input.report)
        .map_err(|_| authentication_error("verified report not JSON"))?;
    let report = parse_report(&report_value, work)?;
    let context = lock_context(&lock_value, &input.coordinate)?;
    let lock_targets = parse_target_rows(&lock_value["payload"]["target_matrix"])?;
    Ok(Authenticated {
        coordinate: input.coordinate.clone(),
        report_digest: digest(INPUT_DOMAIN, input.report.as_bytes()),
        report_bytes: input.report.len(),
        lock_digest: digest(INPUT_DOMAIN, input.lock.as_bytes()),
        lock_bytes: input.lock.len(),
        subjects_digest: bf!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(hasher.finalize())
        ),
        subjects_bytes: subject_bytes,
        report,
        context,
        lock_targets,
    })
}

fn parse_report(value: &Value, work: &mut usize) -> Result<Report, crate::diagnostic::Diagnostic> {
    let payload = &value["payload"];
    let mut exports = BTreeMap::new();
    let rows = payload["exports"]
        .as_array()
        .ok_or_else(|| authentication_error("v2 exports missing"))?;
    for row in rows {
        charge(work, 1)?;
        let id = required_str(row, "stable_id")?.to_owned();
        if exports.insert(id, row.clone()).is_some() {
            return Err(authentication_error("duplicate v2 export"));
        }
    }
    let mut types = BTreeMap::new();
    for row in payload["types"]
        .as_array()
        .ok_or_else(|| authentication_error("v2 types missing"))?
    {
        charge(work, 1)?;
        let id = required_str(row, "stable_id")?.to_owned();
        if types.insert(id, row.clone()).is_some() {
            return Err(authentication_error("duplicate v2 type"));
        }
    }
    let mut targets = BTreeMap::new();
    for row in payload["targets"]
        .as_array()
        .ok_or_else(|| authentication_error("v2 targets missing"))?
    {
        charge(work, 1)?;
        targets.insert(
            required_str(row, "target")?.to_owned(),
            required_str(row, "status")?.to_owned(),
        );
    }
    let unproven = payload["unproven_exports"]
        .as_array()
        .is_none_or(|v| !v.is_empty())
        || payload["unproven_types"]
            .as_array()
            .is_none_or(|v| !v.is_empty());
    let call_contract = rows.iter().any(|row| {
        row["requires"].to_string().contains("\"kind\":\"call\"")
            || row["ensures"].to_string().contains("\"kind\":\"call\"")
    });
    let imported_resource = types.values().any(|row| {
        row["definition"]["kind"] == "resource"
            && row["definition"]["lifecycle"]["kind"] == "imported"
    });
    Ok(Report {
        exports,
        types,
        targets,
        unproven,
        call_contract,
        imported_resource,
    })
}

fn exact_subject_report(subject: &str) -> Result<&str, crate::diagnostic::Diagnostic> {
    const PAYLOAD: &str = "\"payload\":";
    const REPORT: &str = "\"report\":";
    const END: &str = ",\"dependencies\":";
    let payload = subject
        .find(PAYLOAD)
        .map(|offset| offset + PAYLOAD.len())
        .ok_or_else(|| authentication_error("subject payload missing"))?;
    let start = subject[payload..]
        .find(REPORT)
        .map(|offset| payload + offset + REPORT.len())
        .ok_or_else(|| authentication_error("subject exact report missing"))?;
    let end = subject[start..]
        .find(END)
        .map(|offset| start + offset)
        .ok_or_else(|| authentication_error("subject exact report terminator missing"))?;
    Ok(&subject[start..end])
}

fn parse_target_rows(
    value: &Value,
) -> Result<BTreeMap<String, String>, crate::diagnostic::Diagnostic> {
    let mut targets = BTreeMap::new();
    for row in value
        .as_array()
        .ok_or_else(|| authentication_error("lock target matrix missing"))?
    {
        targets.insert(
            required_str(row, "target")?.to_owned(),
            required_str(row, "status")?.to_owned(),
        );
    }
    Ok(targets)
}

fn lock_context(
    lock: &Value,
    selected: &package_lock_v2::Coordinate,
) -> Result<Value, crate::diagnostic::Diagnostic> {
    let mut payload = lock["payload"].clone();
    let object = payload
        .as_object_mut()
        .ok_or_else(|| authentication_error("lock payload missing"))?;
    object.remove("limits");
    object.remove("budget");
    object.remove("nonclaims");
    object.remove("target_matrix");
    if let Some(packages) = object.get_mut("packages").and_then(Value::as_array_mut) {
        for row in packages {
            if row["package"].as_str() == Some(selected.package.as_str())
                && row["version"].as_str() == Some(selected.version.as_str())
            {
                let map = row
                    .as_object_mut()
                    .ok_or_else(|| authentication_error("lock package invalid"))?;
                for key in [
                    "subject_digest",
                    "subject_bytes",
                    "report_digest",
                    "report_bytes",
                    "revision",
                    "targets",
                ] {
                    map.remove(key);
                }
                map.insert("version".to_owned(), Value::String("<selected>".to_owned()));
            }
        }
    }
    normalize_selected_coordinates(&mut payload, selected);
    Ok(payload)
}

fn normalize_selected_coordinates(value: &mut Value, selected: &package_lock_v2::Coordinate) {
    match value {
        Value::Object(map) => {
            if map.get("package").and_then(Value::as_str) == Some(selected.package.as_str())
                && map.get("version").and_then(Value::as_str) == Some(selected.version.as_str())
            {
                map.insert("version".to_owned(), Value::String("<selected>".to_owned()));
            }
            for value in map.values_mut() {
                normalize_selected_coordinates(value, selected);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_selected_coordinates(value, selected)
            }
        }
        _ => {}
    }
}
