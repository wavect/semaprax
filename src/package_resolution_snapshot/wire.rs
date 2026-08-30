use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::package_resolver::{Requirement, ResolutionInput, ResolutionOptions};

use super::{input_error, limit_error, replay_error, wire_error, INPUT_DOMAIN, INPUT_SCHEMA};

const NONCLAIMS: [&str; 9] = [
    "no_registry_network_discovery_or_fetch",
    "fresh_output_directory_only_no_mutable_update_or_active_pivot",
    "no_cache_index_or_garbage_collection",
    "no_signature_publisher_identity_trusted_provenance_license_or_sbom_trust",
    "no_build_scripts_external_tools_or_target_execution",
    "no_capability_enforcement_or_hermetic_sandbox",
    "evidence_and_digest_are_not_authority",
    "no_source_git_editor_or_commit_mutation",
    "resolver_lock_report_capsule_build_project_graph_and_cleanup_contracts_unchanged",
];

pub(super) struct ParsedInput {
    pub(super) input: ResolutionInput,
    pub(super) options: ResolutionOptions,
}

pub(super) fn render_input(
    input: &ResolutionInput,
    options: &ResolutionOptions,
) -> Result<String, Diagnostic> {
    super::model::preflight_requirements(&input.requirements)?;
    ResolutionOptions::new(options.max_bytes)
        .map_err(|_| input_error("snapshot Resolver-v1 options are invalid"))?;
    if input.subjects.is_empty() || input.subjects.len() > crate::package_resolver::MAX_SUBJECTS {
        return Err(input_error("snapshot subject count is outside bounds"));
    }
    let total_subject_bytes = input.subjects.iter().try_fold(0usize, |sum, subject| {
        if subject.len() > crate::package_resolver::MAX_SUBJECT_BYTES {
            return Err(limit_error("snapshot subject exceeds Resolver-v1 bound"));
        }
        sum.checked_add(subject.len())
            .ok_or_else(|| limit_error("snapshot subject byte accounting overflowed"))
    })?;
    if total_subject_bytes > crate::package_resolver::MAX_TOTAL_SUBJECT_BYTES {
        return Err(limit_error(
            "snapshot catalog exceeds Resolver-v1 cumulative bound",
        ));
    }

    // Resolver replay above authenticates this exact catalog. This second
    // narrow pass ensures this renderer is safe when used independently by
    // verification and never re-renders a Subject-v2 envelope.
    let mut work = 0usize;
    for subject in &input.subjects {
        crate::package_lock_v2::authenticate_subject_for_resolution(subject, &mut work).map_err(
            |error| {
                if error.code == "SPX-PL506" {
                    limit_error("snapshot Subject-v2 authentication exceeded bounds")
                } else {
                    super::authentication_error("snapshot contains an invalid Subject-v2 envelope")
                }
            },
        )?;
    }

    let mut subjects = input
        .subjects
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    subjects.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if subjects
        .windows(2)
        .any(|pair| pair[0].as_bytes() >= pair[1].as_bytes())
    {
        return Err(input_error("snapshot subjects must be byte-distinct"));
    }
    let (output, overflowed) = bounded_output::with_limit(super::MAX_INPUT_RENDER_BYTES, || {
        render_canonical(input, options, &subjects)
    });
    if overflowed {
        return Err(limit_error(
            "snapshot input cumulative render bound exceeded",
        ));
    }
    let framing_bytes = output
        .len()
        .checked_sub(total_subject_bytes)
        .ok_or_else(|| limit_error("snapshot input framing accounting underflowed"))?;
    if framing_bytes > super::MAX_INPUT_FRAMING_BYTES {
        return Err(limit_error("snapshot input framing bound exceeded"));
    }
    if output.len() > super::MAX_INPUT_BYTES {
        return Err(limit_error("snapshot input capsule exceeds derived bound"));
    }
    Ok(output)
}

pub(super) fn parse_input(wire: &str) -> Result<ParsedInput, Diagnostic> {
    if wire.is_empty()
        || wire.len() > super::MAX_INPUT_BYTES
        || wire.starts_with('\u{feff}')
        || wire.ends_with('\n')
        || wire.contains('\r')
    {
        return Err(wire_error(
            "snapshot input capsule is outside its wire bound",
        ));
    }
    let marker = "\"payload\":";
    let start = wire
        .find(marker)
        .map(|offset| offset + marker.len())
        .ok_or_else(|| wire_error("snapshot input payload is missing"))?;
    let payload = wire
        .get(start..wire.len().saturating_sub(1))
        .ok_or_else(|| wire_error("snapshot input payload boundary is invalid"))?;
    let (subjects, subject_start, subject_end) = exact_subjects(payload)?;
    let global_subject_start = start
        .checked_add(subject_start)
        .ok_or_else(|| wire_error("snapshot subject boundary overflowed"))?;
    let global_subject_end = start
        .checked_add(subject_end)
        .ok_or_else(|| wire_error("snapshot subject boundary overflowed"))?;
    let raw_subject_bytes = global_subject_end
        .checked_sub(global_subject_start)
        .ok_or_else(|| wire_error("snapshot subject boundary underflowed"))?;
    let sanitized_bytes = wire
        .len()
        .checked_sub(raw_subject_bytes)
        .ok_or_else(|| wire_error("snapshot subject boundary underflowed"))?;
    if sanitized_bytes > super::MAX_INPUT_FRAMING_BYTES {
        return Err(limit_error(
            "snapshot input framing exceeds its closed bound",
        ));
    }
    let mut sanitized = String::with_capacity(sanitized_bytes);
    sanitized.push_str(&wire[..global_subject_start]);
    sanitized.push_str(&wire[global_subject_end..]);
    let value: Value = serde_json::from_str(&sanitized)
        .map_err(|_| wire_error("snapshot input capsule is not JSON"))?;
    require_keys(&value, &["schema", "digest", "bytes", "payload"], "wrapper")?;
    if value["schema"].as_str() != Some(INPUT_SCHEMA) {
        return Err(wire_error("snapshot input schema is invalid"));
    }
    if value["bytes"].as_u64() != Some(payload.len() as u64)
        || value["digest"].as_str() != Some(digest(payload.as_bytes()).as_str())
    {
        return Err(replay_error("snapshot input digest/byte binding failed"));
    }
    let payload_value = &value["payload"];
    require_keys(
        payload_value,
        &[
            "schema",
            "requirements",
            "target",
            "allowed_capabilities",
            "subjects",
            "resolution_max_bytes",
            "limits",
            "nonclaims",
        ],
        "payload",
    )?;
    require_keys(
        &payload_value["limits"],
        &[
            "max_subjects",
            "max_subject_bytes",
            "max_total_subject_bytes",
            "max_resolution_bytes",
            "max_lock_bytes",
            "max_input_framing_bytes",
            "max_input_bytes",
            "max_input_render_bytes",
            "max_snapshot_bytes",
        ],
        "limits",
    )?;
    let requirement_rows = payload_value["requirements"]
        .as_array()
        .ok_or_else(|| wire_error("snapshot requirements must be an array"))?;
    super::model::validate_requirement_count(requirement_rows.len())?;
    let requirements = requirement_rows
        .iter()
        .map(|row| {
            require_keys(row, &["package", "range"], "requirement")?;
            let range = row["range"]
                .as_str()
                .ok_or_else(|| wire_error("snapshot range must be a string"))?;
            super::model::validate_range_length(range.len())?;
            Ok(Requirement {
                package: required_string(row, "package")?,
                range: range.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let allowed_capabilities = payload_value["allowed_capabilities"]
        .as_array()
        .ok_or_else(|| wire_error("snapshot capabilities must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| wire_error("snapshot capability must be a string"))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let max_bytes = payload_value["resolution_max_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| wire_error("snapshot resolution bound is invalid"))?;
    let options = ResolutionOptions::new(max_bytes)
        .map_err(|_| input_error("snapshot Resolver-v1 options are invalid"))?;
    let input = ResolutionInput {
        requirements,
        subjects,
        target: required_string(payload_value, "target")?,
        allowed_capabilities,
    };
    let canonical = render_input(&input, &options)?;
    if canonical != wire {
        return Err(replay_error("snapshot input capsule is not canonical"));
    }
    Ok(ParsedInput { input, options })
}

fn exact_subjects(payload: &str) -> Result<(Vec<String>, usize, usize), Diagnostic> {
    const START: &str = ",\"subjects\":[";
    const FOLLOW: &str = "],\"resolution_max_bytes\":";
    let start = payload
        .find(START)
        .map(|offset| offset + START.len())
        .ok_or_else(|| wire_error("snapshot raw subjects member is missing"))?;
    let bytes = payload.as_bytes();
    let mut offset = start;
    let mut values = Vec::new();
    let mut subject_bytes = 0usize;
    loop {
        if bytes.get(offset) == Some(&b']') {
            break;
        }
        if bytes.get(offset) != Some(&b'{') {
            return Err(wire_error("snapshot subject must be a raw JSON object"));
        }
        super::model::admit_subject_slot(values.len())?;
        let value_end = object_end(bytes, offset, payload.len())?;
        let value_bytes = value_end
            .checked_sub(offset)
            .ok_or_else(|| wire_error("snapshot subject boundary underflowed"))?;
        subject_bytes = super::model::add_subject_bytes(subject_bytes, value_bytes)?;
        values.push(payload[offset..value_end].to_owned());
        offset = value_end;
        if bytes.get(offset) == Some(&b',') {
            offset += 1;
        } else if bytes.get(offset) != Some(&b']') {
            return Err(wire_error("snapshot subject delimiter is invalid"));
        }
    }
    let end = offset;
    let follow_end = end
        .checked_add(FOLLOW.len())
        .ok_or_else(|| wire_error("snapshot subject terminator overflowed"))?;
    if payload.get(end..follow_end) != Some(FOLLOW) {
        return Err(wire_error("snapshot raw subjects terminator is invalid"));
    }
    if values.is_empty() || values.len() > crate::package_resolver::MAX_SUBJECTS {
        return Err(input_error("snapshot subject count is outside bounds"));
    }
    if values
        .windows(2)
        .any(|pair| pair[0].as_bytes() >= pair[1].as_bytes())
    {
        return Err(wire_error("snapshot subjects are not strictly byte-sorted"));
    }
    Ok((values, start, end))
}

fn render_canonical(
    input: &ResolutionInput,
    options: &ResolutionOptions,
    subjects: &[&str],
) -> String {
    let requirements = input
        .requirements
        .iter()
        .map(|row| {
            bounded_output::budgeted_format(format_args!(
                "{{\"package\":{},\"range\":{}}}",
                quote_json(&row.package),
                quote_json(&row.range)
            ))
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let capabilities = input
        .allowed_capabilities
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let subjects = subjects.budgeted_join(",");
    let nonclaims = NONCLAIMS
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let payload = bounded_output::budgeted_format(format_args!(
        "{{\"schema\":{},\"requirements\":[{}],\"target\":{},\"allowed_capabilities\":[{}],\"subjects\":[{}],\"resolution_max_bytes\":{},\"limits\":{{\"max_subjects\":{},\"max_subject_bytes\":{},\"max_total_subject_bytes\":{},\"max_resolution_bytes\":{},\"max_lock_bytes\":{},\"max_input_framing_bytes\":{},\"max_input_bytes\":{},\"max_input_render_bytes\":{},\"max_snapshot_bytes\":{}}},\"nonclaims\":[{}]}}",
        quote_json(INPUT_SCHEMA), requirements, quote_json(&input.target), capabilities,
        subjects, options.max_bytes, crate::package_resolver::MAX_SUBJECTS,
        crate::package_resolver::MAX_SUBJECT_BYTES,
        crate::package_resolver::MAX_TOTAL_SUBJECT_BYTES,
        crate::package_resolver::MAX_OUTPUT_BYTES, crate::package_lock_v2::MAX_OUTPUT_BYTES,
        super::MAX_INPUT_FRAMING_BYTES, super::MAX_INPUT_BYTES,
        super::MAX_INPUT_RENDER_BYTES, super::MAX_SNAPSHOT_BYTES, nonclaims
    ));
    render_wrapper(&payload)
}

#[cfg(test)]
pub(super) fn fixed_framing_fixture_bytes() -> usize {
    let input = ResolutionInput {
        requirements: Vec::new(),
        subjects: Vec::new(),
        target: "native64".to_owned(),
        allowed_capabilities: Vec::new(),
    };
    let wire = render_canonical(&input, &ResolutionOptions::default(), &[]);
    let value: Value = serde_json::from_str(&wire).expect("production framing fixture");
    let actual_length_digits = value["bytes"]
        .as_u64()
        .expect("production payload length")
        .to_string()
        .len();
    // The empty arrays deliberately isolate fixed bytes. Replace only the
    // short fixture payload-length integer width with the maximum admitted
    // payload width; every other production field already has its maximum.
    wire.len() + super::MAX_INPUT_BYTES.to_string().len() - actual_length_digits
}

fn object_end(bytes: &[u8], start: usize, limit: usize) -> Result<usize, Diagnostic> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate().take(limit).skip(start) {
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'\"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'\"' => string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| wire_error("snapshot JSON depth overflowed"))?;
                if depth > crate::package_lock_v2::MAX_JSON_DEPTH + 8 {
                    return Err(wire_error("snapshot subject JSON depth exceeds bound"));
                }
            }
            b'}' | b']' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| wire_error("snapshot subject JSON nesting is invalid"))?;
                if depth == 0 {
                    return Ok(index + 1);
                }
            }
            _ => {}
        }
    }
    Err(wire_error("snapshot subject JSON object is unterminated"))
}

fn render_wrapper(payload: &str) -> String {
    bounded_output::budgeted_format(format_args!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(INPUT_SCHEMA),
        quote_json(&digest(payload.as_bytes())),
        payload.len(),
        payload
    ))
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(INPUT_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ))
}

fn require_keys(value: &Value, keys: &[&str], label: &str) -> Result<(), Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| wire_error(format!("snapshot {label} must be an object")))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(wire_error(format!(
            "snapshot {label} keys are not the closed schema"
        )));
    }
    Ok(())
}

fn required_string(value: &Value, key: &str) -> Result<String, Diagnostic> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| wire_error(format!("snapshot {key} must be a string")))
}
