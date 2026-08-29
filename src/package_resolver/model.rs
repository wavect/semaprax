use std::collections::BTreeSet;

use serde_json::Value;

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};

use super::catalog::Catalog;
use super::semver::{self, Range};
use super::solver::Solved;
use super::wire::{self, render_wrapper};
use super::{
    ResolutionInput, ResolutionOptions, CATALOG_DOMAIN, MAX_ALLOWED_CAPABILITIES,
    MAX_DECISIONS, MAX_DEPTH, MAX_EDGES, MAX_JSON_DEPTH, MAX_OUTPUT_BYTES, MAX_RENDER_BYTES,
    MAX_REQUIREMENTS, MAX_SELECTED_PACKAGES, MAX_SUBJECT_BYTES, MAX_SUBJECTS,
    MAX_TOTAL_SUBJECT_BYTES, MAX_VERSIONS_PER_PACKAGE, MAX_WORK_UNITS, SCHEMA,
};

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

#[derive(Clone)]
pub(super) struct ParsedRequirement {
    pub(super) package: String,
    pub(super) text: String,
    pub(super) range: Range,
    pub(super) row: usize,
}

pub(super) fn validate_input(
    input: &ResolutionInput,
    work: &mut usize,
) -> Result<Vec<ParsedRequirement>, Diagnostic> {
    if input.requirements.is_empty() || input.requirements.len() > MAX_REQUIREMENTS {
        return Err(wire::input_error("requirement count is outside bounds"));
    }
    if !matches!(input.target.as_str(), "native64" | "wasm32") {
        return Err(wire::input_error("target is outside the closed set"));
    }
    validate_values(&input.allowed_capabilities, MAX_ALLOWED_CAPABILITIES, "capability")?;
    let mut previous = None;
    let mut parsed = Vec::with_capacity(input.requirements.len());
    for (row, requirement) in input.requirements.iter().enumerate() {
        validate_identity(&requirement.package, "package")?;
        if previous.is_some_and(|value: &String| value >= &requirement.package) {
            return Err(wire::input_error(
                "requirements must be strictly package-sorted and unique",
            ));
        }
        previous = Some(requirement.package.clone());
        parsed.push(ParsedRequirement {
            package: requirement.package.clone(),
            text: requirement.range.clone(),
            range: semver::parse_range(&requirement.range)?,
            row,
        });
        wire::charge(work, 1)?;
    }
    Ok(parsed)
}

pub(super) fn validate_identity(value: &str, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.len() > 255
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(wire::input_error(format!("invalid {label}")));
    }
    Ok(())
}

fn validate_values(values: &[String], maximum: usize, label: &str) -> Result<(), Diagnostic> {
    if values.len() > maximum {
        return Err(wire::input_error(format!("{label} count exceeds limit")));
    }
    let mut previous = None;
    for value in values {
        validate_identity(value, label)?;
        if previous.is_some_and(|item: &String| item >= value) {
            return Err(wire::input_error(format!(
                "{label} values must be strictly sorted and unique"
            )));
        }
        previous = Some(value.clone());
    }
    Ok(())
}

pub(super) fn render_evidence(
    input: &ResolutionInput,
    options: &ResolutionOptions,
    requirements: &[ParsedRequirement],
    catalog: &Catalog<'_>,
    solved: &Solved<'_, '_>,
    lock: &str,
    work: usize,
) -> Result<String, Diagnostic> {
    let requirement_rows = requirements
        .iter()
        .map(|row| {
            bf!(
                "{{\"package\":{},\"range\":{}}}",
                quote_json(&row.package),
                quote_json(&row.text)
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let capabilities = input
        .allowed_capabilities
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let selected = solved
        .selected
        .values()
        .map(|entry| {
            bf!(
                "{{\"package\":{},\"version\":{},\"subject_digest\":{},\"subject_bytes\":{}}}",
                quote_json(&entry.subject.coordinate.package),
                quote_json(&entry.subject.coordinate.version),
                quote_json(&entry.subject.subject_digest),
                entry.subject.subject_bytes
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let lock_value: Value = serde_json::from_str(lock)
        .map_err(|_| wire::authentication_error("verified Lock-v2 is not JSON"))?;
    let lock_digest = lock_value["digest"]
        .as_str()
        .ok_or_else(|| wire::authentication_error("verified Lock-v2 digest is missing"))?;
    let payload = bf!(
        "{{\"schema\":{},\"requirements\":[{}],\"target\":{},\"allowed_capabilities\":[{}],\"catalog\":{{\"subjects\":{},\"bytes\":{},\"digest\":{}}},\"selected\":[{}],\"lock_digest\":{},\"lock_bytes\":{},\"lock\":{},\"limits\":{{\"max_requirements\":{MAX_REQUIREMENTS},\"max_subjects\":{MAX_SUBJECTS},\"max_versions_per_package\":{MAX_VERSIONS_PER_PACKAGE},\"max_selected_packages\":{MAX_SELECTED_PACKAGES},\"max_allowed_capabilities\":{MAX_ALLOWED_CAPABILITIES},\"max_subject_bytes\":{MAX_SUBJECT_BYTES},\"max_total_subject_bytes\":{MAX_TOTAL_SUBJECT_BYTES},\"max_edges\":{MAX_EDGES},\"max_depth\":{MAX_DEPTH},\"max_decisions\":{MAX_DECISIONS},\"max_work_units\":{MAX_WORK_UNITS},\"max_json_depth\":{MAX_JSON_DEPTH},\"max_render_bytes\":{MAX_RENDER_BYTES},\"max_output_bytes\":{MAX_OUTPUT_BYTES},\"requested_max_bytes\":{}}},\"budget\":{{\"used_subjects\":{},\"used_subject_bytes\":{},\"used_selected_packages\":{},\"used_edges\":{},\"used_depth\":{},\"used_decisions\":{},\"used_allowed_capabilities\":{},\"used_work_units\":{}}},\"nonclaims\":[\"offline_deterministic_resolution_evidence\",\"no_registry_network_fetch_build_script_execution_cache_or_publication\",\"capability_allowlist_is_resolution_admission_not_runtime_enforcement\",\"target_availability_is_projection_not_execution\",\"evidence_is_not_authority\"]}}",
        quote_json(SCHEMA),
        requirement_rows,
        quote_json(&input.target),
        capabilities,
        catalog.entries.len(),
        catalog.total_bytes,
        quote_json(&catalog.digest),
        selected,
        quote_json(lock_digest),
        lock.len(),
        lock,
        options.max_bytes,
        catalog.entries.len(),
        catalog.total_bytes,
        solved.selected.len(),
        solved.edges,
        solved.depth,
        solved.decisions,
        input.allowed_capabilities.len(),
        work
    );
    Ok(render_wrapper(&payload))
}

pub(super) fn recheck_lock_policy(
    lock: &str,
    input: &ResolutionInput,
) -> Result<(), Diagnostic> {
    let value: Value = serde_json::from_str(lock)
        .map_err(|_| wire::authentication_error("verified Lock-v2 is not JSON"))?;
    let allowed = input
        .allowed_capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for package in value["payload"]["packages"]
        .as_array()
        .ok_or_else(|| wire::authentication_error("Lock-v2 package rows missing"))?
    {
        let target = package["targets"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|row| row["target"].as_str() == Some(input.target.as_str()))
            })
            .and_then(|row| row["status"].as_str());
        if target != Some("available") {
            return Err(wire::policy_error("Lock-v2 target policy recheck failed"));
        }
        for capability in package["capability_closure"]
            .as_array()
            .ok_or_else(|| wire::authentication_error("Lock-v2 capability closure missing"))?
        {
            if capability.as_str().is_none_or(|value| !allowed.contains(value)) {
                return Err(wire::policy_error(
                    "Lock-v2 capability policy recheck failed",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn exact_lock_bytes(evidence: &str) -> Result<&str, Diagnostic> {
    const START: &str = "\"lock\":";
    let start = evidence
        .find(START)
        .map(|offset| offset + START.len())
        .ok_or_else(|| wire::wire_error("exact embedded Lock-v2 missing"))?;
    let end = structural_json_value_end(evidence.as_bytes(), start)?;
    let suffix_end = end
        .checked_add(",\"limits\":".len())
        .ok_or_else(|| wire::wire_error("embedded Lock-v2 boundary overflows"))?;
    if evidence.get(end..suffix_end) != Some(",\"limits\":") {
        return Err(wire::wire_error(
            "embedded Lock-v2 is not followed by outer limits",
        ));
    }
    Ok(&evidence[start..end])
}

fn structural_json_value_end(bytes: &[u8], start: usize) -> Result<usize, Diagnostic> {
    if bytes.get(start) != Some(&b'{') {
        return Err(wire::wire_error("embedded Lock-v2 must be a JSON object"));
    }
    let mut stack = vec![b'}'];
    let mut string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start + 1..].iter().copied().enumerate() {
        let index = start + 1 + offset;
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => string = true,
            b'{' => stack.push(b'}'),
            b'[' => stack.push(b']'),
            b'}' | b']' => {
                if stack.pop() != Some(byte) {
                    return Err(wire::wire_error(
                        "embedded Lock-v2 JSON nesting is invalid",
                    ));
                }
                if stack.is_empty() {
                    return Ok(index + 1);
                }
            }
            _ => {}
        }
    }
    Err(wire::wire_error(
        "embedded Lock-v2 JSON value is unterminated",
    ))
}

pub(super) fn catalog_digest<'a>(entries: impl Iterator<Item = &'a str>, count: usize) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(CATALOG_DOMAIN);
    hasher.update((count as u64).to_le_bytes());
    for bytes in entries {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes.as_bytes());
    }
    bf!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}
