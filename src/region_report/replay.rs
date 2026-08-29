use super::*;

pub(super) fn verify_envelope(envelope: &str) -> Result<VerifiedRegionReport, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(consistency_error(
            "envelope must be a JSON object".to_owned(),
        ));
    };
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    if keys != ["bytes", "digest", "payload", "schema"] {
        return Err(consistency_error(format!(
            "envelope keys must be exactly [bytes, digest, payload, schema], found {keys:?}"
        )));
    }
    if object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "envelope schema must be {SCHEMA}"
        )));
    }
    let Some(envelope_digest) = object["digest"].as_str() else {
        return Err(consistency_error(
            "envelope digest must be a string".to_owned(),
        ));
    };
    let Some(declared_bytes) = object["bytes"].as_u64() else {
        return Err(consistency_error(
            "envelope bytes must be an unsigned integer".to_owned(),
        ));
    };
    const PAYLOAD_KEY: &str = "\"payload\":";
    let Some(offset) = envelope.find(PAYLOAD_KEY) else {
        return Err(consistency_error(
            "envelope is missing its payload member".to_owned(),
        ));
    };
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
    let recomputed = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }
    let payload_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    if payload_value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }

    let functions_total = payload_value["module"]["functions_total"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(
                "payload module functions_total must be an unsigned integer".to_owned(),
            )
        })?;
    let admitted = payload_value["module"]["functions_admitted"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(
                "payload module functions_admitted must be an unsigned integer".to_owned(),
            )
        })?;
    let excluded_total = payload_value["module"]["functions_excluded"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(
                "payload module functions_excluded must be an unsigned integer".to_owned(),
            )
        })?;
    let functions_len = payload_value["functions"].as_array().map_or(0, Vec::len) as u64;
    let exclusions_len = payload_value["exclusions"].as_array().map_or(0, Vec::len) as u64;
    if functions_total != functions_len + exclusions_len
        || admitted != functions_len
        || excluded_total != exclusions_len
    {
        return Err(consistency_error(
            "module counts disagree with the listed functions and exclusions".to_owned(),
        ));
    }

    let Some(exclusions) = payload_value["exclusions"].as_array() else {
        return Err(consistency_error(
            "payload exclusions must be an array".to_owned(),
        ));
    };
    let mut previous_exclusion: Option<&str> = None;
    for exclusion in exclusions {
        let Some(stable_id) = exclusion["stable_id"].as_str() else {
            return Err(consistency_error(
                "exclusion stable_id must be a string".to_owned(),
            ));
        };
        if let Some(previous) = previous_exclusion {
            if previous.as_bytes() >= stable_id.as_bytes() {
                return Err(consistency_error(format!(
                    "exclusion `{stable_id}` breaks the strict stable-id ordering"
                )));
            }
        }
        previous_exclusion = Some(stable_id);
        let Some(reason) = exclusion["reason"].as_str() else {
            return Err(consistency_error(
                "exclusion reason must be a string".to_owned(),
            ));
        };
        if !EXCLUSION_REASONS.contains(&reason) {
            return Err(consistency_error(format!(
                "exclusion reason `{reason}` is outside the closed vocabulary"
            )));
        }
    }

    let Some(functions) = payload_value["functions"].as_array() else {
        return Err(consistency_error(
            "payload functions must be an array".to_owned(),
        ));
    };
    let mut verified = Vec::with_capacity(functions.len());
    let mut previous_id: Option<&str> = None;
    for function in functions {
        let Some(stable_id) = function["stable_id"].as_str() else {
            return Err(consistency_error(
                "function stable_id must be a string".to_owned(),
            ));
        };
        if let Some(previous) = previous_id {
            if previous.as_bytes() >= stable_id.as_bytes() {
                return Err(consistency_error(format!(
                    "function `{stable_id}` breaks the strict stable-id ordering"
                )));
            }
        }
        previous_id = Some(stable_id);
        verified.push(replay_function(function, stable_id)?);
    }
    Ok(VerifiedRegionReport {
        functions: verified,
    })
}

/// Verify one envelope and additionally bind the current bytes of
/// `source_path` to the embedded source digest, failing closed on drift.
pub(super) fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<VerifiedRegionReport, Diagnostic> {
    let verified = verify_envelope(envelope)?;
    let current = std::fs::read(source_path).map_err(|error| {
        consistency_error(format!("cannot read {}: {error}", source_path.display()))
    })?;
    let bound = bound_source_digest(envelope)?;
    if bound != domain_digest(SOURCE_DIGEST_DOMAIN, &current) {
        return Err(consistency_error(
            "region report source digest does not match the current source bytes; \
             the source drifted after the report was generated"
                .to_owned(),
        ));
    }
    Ok(verified)
}

fn bound_source_digest(envelope: &str) -> Result<String, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(digest) = value["payload"]["source"]["sha256"].as_str() else {
        return Err(consistency_error(
            "payload source sha256 must be a string".to_owned(),
        ));
    };
    Ok(digest.to_owned())
}

#[expect(clippy::too_many_lines)]
fn replay_function(
    function: &serde_json::Value,
    stable_id: &str,
) -> Result<VerifiedFunctionReport, Diagnostic> {
    let Some(name) = function["name"].as_str() else {
        return Err(consistency_error(format!(
            "function `{stable_id}` name must be a string"
        )));
    };
    let Some(bindings) = function["bindings"].as_array() else {
        return Err(consistency_error(format!(
            "function `{stable_id}` bindings must be an array"
        )));
    };
    let mut facts: Vec<BindingFact> = Vec::with_capacity(bindings.len());
    let mut previous_binding: Option<&str> = None;
    for binding in bindings {
        let Some(id) = binding["id"].as_str() else {
            return Err(consistency_error(format!(
                "binding id in `{stable_id}` must be a string"
            )));
        };
        if let Some(previous) = previous_binding {
            if previous.as_bytes() >= id.as_bytes() {
                return Err(consistency_error(format!(
                    "binding `{id}` in `{stable_id}` breaks the strict binding-id ordering"
                )));
            }
        }
        previous_binding = Some(id);
        let kind = match binding["kind"].as_str() {
            Some(KIND_PARAM) => KIND_PARAM,
            Some(KIND_LOCAL) => KIND_LOCAL,
            Some(KIND_PATTERN) => KIND_PATTERN,
            _ => {
                return Err(consistency_error(format!(
                    "binding `{id}` carries an unknown or missing kind"
                )))
            }
        };
        let fact = BindingFact {
            id: id.to_owned(),
            name: binding["name"]
                .as_str()
                .ok_or_else(|| consistency_error(format!("binding `{id}` name must be a string")))?
                .to_owned(),
            kind,
            mutable: binding["mutable"].as_bool().ok_or_else(|| {
                consistency_error(format!("binding `{id}` mutable must be a boolean"))
            })?,
            ownership: binding["ownership"]
                .as_str()
                .ok_or_else(|| {
                    consistency_error(format!("binding `{id}` ownership must be a string"))
                })?
                .to_owned(),
            type_key: binding["type"]
                .as_str()
                .ok_or_else(|| consistency_error(format!("binding `{id}` type must be a string")))?
                .to_owned(),
            def_offset: binding["def_offset"].as_u64().ok_or_else(|| {
                consistency_error(format!(
                    "binding `{id}` def_offset must be an unsigned integer"
                ))
            })? as usize,
            last_use_offset: binding["last_use_offset"].as_u64().ok_or_else(|| {
                consistency_error(format!(
                    "binding `{id}` last_use_offset must be an unsigned integer"
                ))
            })? as usize,
            use_count: binding["use_count"].as_u64().ok_or_else(|| {
                consistency_error(format!(
                    "binding `{id}` use_count must be an unsigned integer"
                ))
            })? as usize,
        };
        if fact.last_use_offset < fact.def_offset {
            return Err(consistency_error(format!(
                "binding `{id}` claims a live-range end before its definition"
            )));
        }
        // A used binding's boundary is the end of its innermost statement or
        // tail, which always lies strictly after the definition token.
        if (fact.use_count == 0) != (fact.range_end() == fact.def_offset) {
            return Err(consistency_error(format!(
                "binding `{id}` use count and live-range end disagree"
            )));
        }
        facts.push(fact);
    }
    let bindings_total = function["bindings_total"].as_u64().ok_or_else(|| {
        consistency_error(format!(
            "function `{stable_id}` bindings_total must be an unsigned integer"
        ))
    })?;
    if bindings_total != facts.len() as u64 {
        return Err(consistency_error(format!(
            "function `{stable_id}` bindings_total disagrees with the listed bindings"
        )));
    }

    let Some(regions) = function["regions"].as_array() else {
        return Err(consistency_error(format!(
            "function `{stable_id}` regions must be an array"
        )));
    };
    let regions_total = function["regions_total"].as_u64().ok_or_else(|| {
        consistency_error(format!(
            "function `{stable_id}` regions_total must be an unsigned integer"
        ))
    })?;
    if regions_total != regions.len() as u64 {
        return Err(consistency_error(format!(
            "function `{stable_id}` regions_total disagrees with the listed regions"
        )));
    }
    let mut covered: Vec<&str> = Vec::new();
    for (index, region) in regions.iter().enumerate() {
        if region["index"].as_u64() != Some(index as u64) {
            return Err(consistency_error(format!(
                "function `{stable_id}` region indexes must enumerate 0..{regions_total} in order"
            )));
        }
        let Some(members) = region["binding_ids"].as_array() else {
            return Err(consistency_error(format!(
                "function `{stable_id}` region {index} binding_ids must be an array"
            )));
        };
        if members.is_empty() {
            return Err(consistency_error(format!(
                "function `{stable_id}` region {index} would be empty"
            )));
        }
        let mut member_facts: Vec<&BindingFact> = Vec::with_capacity(members.len());
        for member in members {
            let Some(member_id) = member.as_str() else {
                return Err(consistency_error(format!(
                    "function `{stable_id}` region {index} binding_ids must contain strings"
                )));
            };
            if covered.contains(&member_id) {
                return Err(consistency_error(format!(
                    "binding `{member_id}` appears in more than one region of `{stable_id}`"
                )));
            }
            let Some(fact) = facts.iter().find(|fact| fact.id == member_id) else {
                return Err(consistency_error(format!(
                    "region {index} of `{stable_id}` lists unknown binding `{member_id}`"
                )));
            };
            member_facts.push(fact);
            covered.push(member_id);
        }
        for pair_index in 0..member_facts.len() {
            for other_index in pair_index + 1..member_facts.len() {
                if member_facts[pair_index].overlaps(member_facts[other_index]) {
                    return Err(consistency_error(format!(
                        "function `{stable_id}` region {index} would hold overlapping \
                         live ranges `{}` and `{}`",
                        member_facts[pair_index].id, member_facts[other_index].id
                    )));
                }
            }
        }
    }
    if covered.len() != facts.len() {
        return Err(consistency_error(format!(
            "function `{stable_id}` regions do not cover every binding exactly once"
        )));
    }
    // Re-derive the canonical greedy clustering and require an exact match so
    // any reassignment - even a conflict-free one - fails replay.
    let expected_regions = derive_regions(&facts);
    let rendered_regions: Vec<Vec<&str>> = regions
        .iter()
        .map(|region| {
            region["binding_ids"]
                .as_array()
                .expect("checked above")
                .iter()
                .map(|value| value.as_str().expect("checked above"))
                .collect()
        })
        .collect();
    let expected_rendered: Vec<Vec<&str>> = expected_regions
        .iter()
        .map(|members| members.iter().map(String::as_str).collect())
        .collect();
    if rendered_regions != expected_rendered {
        return Err(consistency_error(format!(
            "function `{stable_id}` region assignment disagrees with the canonical \
             clustering re-derived from the reported live ranges"
        )));
    }

    // Escape facts: fully re-derived from the reported parameter ownership.
    let borrowed = function["escape"]["borrowed_parameters"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(format!(
                "function `{stable_id}` escape borrowed_parameters must be an unsigned integer"
            ))
        })?;
    let shared = function["escape"]["shared_parameters"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(format!(
                "function `{stable_id}` escape shared_parameters must be an unsigned integer"
            ))
        })?;
    let borrows_total = function["escape"]["borrows_total"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(format!(
                "function `{stable_id}` escape borrows_total must be an unsigned integer"
            ))
        })?;
    let non_escaping = function["escape"]["non_escaping_borrows_total"]
        .as_u64()
        .ok_or_else(|| {
            consistency_error(format!(
                "function `{stable_id}` escape non_escaping_borrows_total must be an unsigned integer"
            ))
        })?;
    if borrowed + shared != borrows_total || borrows_total != non_escaping {
        return Err(consistency_error(format!(
            "function `{stable_id}` escape totals disagree with their derivation"
        )));
    }
    if function["escape"]["all_borrows_provably_non_escaping"] != serde_json::Value::Bool(true) {
        return Err(consistency_error(format!(
            "function `{stable_id}` must assert every borrow provably non-escaping"
        )));
    }
    if function["escape"]["enforcing_check"].as_str() != Some(ESCAPE_ENFORCING_CHECK) {
        return Err(consistency_error(format!(
            "function `{stable_id}` escape enforcing_check must be {ESCAPE_ENFORCING_CHECK}"
        )));
    }
    if function["escape"]["enforcing_check_summary"].as_str()
        != Some(ESCAPE_ENFORCING_CHECK_SUMMARY)
    {
        return Err(consistency_error(format!(
            "function `{stable_id}` escape enforcing_check_summary must be verbatim"
        )));
    }
    let param_views_total = facts
        .iter()
        .filter(|fact| fact.kind == KIND_PARAM)
        .filter(|fact| matches!(fact.ownership.as_str(), "borrow" | "shared"))
        .count() as u64;
    if param_views_total != borrows_total {
        return Err(consistency_error(format!(
            "function `{stable_id}` escape totals disagree with the reported parameter ownership"
        )));
    }

    // Move facts: sites ordered/unique, moved_bindings exactly their distinct
    // roots in order, all inside the function's binding inventory.
    let Some(sites) = function["moves"]["consumption_sites"].as_array() else {
        return Err(consistency_error(format!(
            "function `{stable_id}` moves consumption_sites must be an array"
        )));
    };
    let mut previous_site: Option<(&str, u64)> = None;
    let mut derived_moved: Vec<&str> = Vec::new();
    for site in sites {
        let Some(binding) = site["binding"].as_str() else {
            return Err(consistency_error(format!(
                "function `{stable_id}` consumption site binding must be a string"
            )));
        };
        let Some(offset) = site["offset"].as_u64() else {
            return Err(consistency_error(format!(
                "function `{stable_id}` consumption site offset must be an unsigned integer"
            )));
        };
        if let Some((previous_binding, previous_offset)) = previous_site {
            if (previous_binding.as_bytes(), previous_offset) > (binding.as_bytes(), offset) {
                return Err(consistency_error(format!(
                    "function `{stable_id}` consumption sites break the canonical ordering"
                )));
            }
            if previous_binding == binding && previous_offset == offset {
                return Err(consistency_error(format!(
                    "function `{stable_id}` repeats consumption site `{binding}` at `{offset}`"
                )));
            }
        }
        previous_site = Some((binding, offset));
        if !facts.iter().any(|fact| fact.id == binding) {
            return Err(consistency_error(format!(
                "function `{stable_id}` consumption site names unknown binding `{binding}`"
            )));
        }
        if derived_moved.last() != Some(&binding) {
            derived_moved.push(binding);
        }
    }
    let Some(moved_bindings) = function["moves"]["moved_bindings"].as_array() else {
        return Err(consistency_error(format!(
            "function `{stable_id}` moves moved_bindings must be an array"
        )));
    };
    let moved_listed: Vec<&str> = moved_bindings
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                consistency_error(format!(
                    "function `{stable_id}` moved_bindings must contain strings"
                ))
            })
        })
        .collect::<Result<_, _>>()?;
    if moved_listed != derived_moved {
        return Err(consistency_error(format!(
            "function `{stable_id}` moved_bindings disagree with the listed consumption sites"
        )));
    }

    // Bulk-release grouping candidates: maximal same-end sets of size >= 2,
    // re-derived exactly, canonically ordered by end offset.
    let Some(release_groups) = function["release_groups"].as_array() else {
        return Err(consistency_error(format!(
            "function `{stable_id}` release_groups must be an array"
        )));
    };
    let rendered_groups: Vec<(u64, Vec<&str>)> = release_groups
        .iter()
        .map(|group| {
            let end = group["end_offset"].as_u64().ok_or_else(|| {
                consistency_error(format!(
                    "function `{stable_id}` release group end_offset must be an unsigned integer"
                ))
            })?;
            let Some(members) = group["binding_ids"].as_array() else {
                return Err(consistency_error(format!(
                    "function `{stable_id}` release group binding_ids must be an array"
                )));
            };
            let ids = members
                .iter()
                .map(|value| {
                    value.as_str().ok_or_else(|| {
                        consistency_error(format!(
                            "function `{stable_id}` release group binding_ids must contain strings"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((end, ids))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_groups = derive_release_groups(&facts);
    let expected_rendered_groups: Vec<(u64, Vec<&str>)> = expected_groups
        .iter()
        .map(|(end, ids)| (*end as u64, ids.iter().map(String::as_str).collect()))
        .collect();
    if rendered_groups != expected_rendered_groups {
        return Err(consistency_error(format!(
            "function `{stable_id}` release groups disagree with the maximal same-end \
             candidates re-derived from the reported live ranges"
        )));
    }

    Ok(VerifiedFunctionReport {
        stable_id: stable_id.to_owned(),
        name: name.to_owned(),
        bindings_total: facts.len(),
        regions_total: regions.len(),
    })
}
