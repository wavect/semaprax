//! Structurally independent replay of the specification and descriptor
//! bytes, plus the decoded-value validators the replay binds.

use super::*;

pub(in crate::implementation) fn replay_limits_exact(replay: &mut ExactReplay<'_>) {
    replay.text("{");
    for (index, (name, value)) in LIMIT_ROWS.into_iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(name);
        replay.text(":");
        replay.usize_noalloc(value);
    }
    replay.text("}");
}

pub(in crate::implementation) fn replay_spec_bytes_exact(source: &str, spec: &Spec) -> bool {
    let Some(source_revision) = spec.source_revision() else {
        return false;
    };
    let mut replay = ExactReplay::new(source);
    replay.text("{\"schema\":");
    replay.json(SPEC_SCHEMA);
    replay.text(",\"module\":");
    replay.json(&spec.module);
    replay.text(",\"source_revision\":");
    replay.json(source_revision);
    replay.text(",\"target\":{\"triple\":");
    replay.json(&spec.target.triple);
    replay.text(",\"pointer_width\":");
    replay.number(spec.target.pointer_width);
    replay.text(",\"endian\":");
    replay.json(&spec.target.endian);
    replay.text(",\"panic_strategy\":");
    replay.json(&spec.target.panic_strategy);
    replay.text(",\"thread_policy\":");
    replay.json(&spec.target.thread_policy);
    replay.text("},\"exports\":[");
    replay_strings_exact(&mut replay, &spec.exports);
    replay.text("],\"imports\":[");
    replay_strings_exact(&mut replay, &spec.imports);
    replay.text("],\"capabilities\":[");
    replay_strings_exact(&mut replay, &spec.capabilities);
    replay.text("],\"limits\":");
    replay_limits_exact(&mut replay);
    replay.text(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(nonclaim);
    }
    replay.text("]}\n");
    replay.finish()
}

fn replay_parameter_exact(replay: &mut ExactReplay<'_>, parameter: &ParameterFact) {
    replay.text("{\"name\":");
    replay.json(&parameter.name);
    replay.text(",\"type\":");
    replay.json(scalar_text(parameter.ty));
    replay.text(",\"mode\":\"value\"}");
}

fn replay_result_exact(replay: &mut ExactReplay<'_>, result: ScalarType) {
    replay.text("{\"type\":");
    replay.json(scalar_text(result));
    replay.text(",\"out_slot\":");
    replay.text(if result == ScalarType::Unit {
        "false"
    } else {
        "true"
    });
    replay.text("}");
}

fn replay_strings_exact(replay: &mut ExactReplay<'_>, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(value);
    }
}

fn replay_descriptor_bytes_exact(
    source: &str,
    spec: &Spec,
    subject: DescriptorSubject<'_>,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("{\"schema\":");
    replay.json(subject.schema());
    replay.text(",\"module\":");
    replay.json(&spec.module);
    replay.text(",\"");
    replay.text(subject.key());
    replay.text("\":");
    replay.json(subject.value());
    replay.text(",\"hir_digest\":");
    replay.json(hir_digest);
    replay.text(",\"target\":{\"triple\":");
    replay.json(&spec.target.triple);
    replay.text(",\"pointer_width\":");
    replay.number(spec.target.pointer_width);
    replay.text(",\"endian\":");
    replay.json(&spec.target.endian);
    replay.text(",\"panic_strategy\":");
    replay.json(&spec.target.panic_strategy);
    replay.text(",\"thread_policy\":");
    replay.json(&spec.target.thread_policy);
    replay.text("},\"status_domains\":[{\"ordinal\":0,\"domain_id\":\"success\"}");
    for (index, domain) in status_domains.iter().enumerate() {
        replay.text(",{\"ordinal\":");
        replay.number(index + 1);
        replay.text(",\"domain_id\":");
        replay.json(domain);
        replay.text("}");
    }
    replay.text(r#",{"ordinal":65533,"domain_id":"semaprax.native-rust-semantics.v1"},{"ordinal":65534,"domain_id":"semaprax.native-rust-host.v1"},{"ordinal":65535,"domain_id":"semaprax.native-rust-adapter.v1"}],"abi":{"version":1,"calling_convention":"C","status_word":"u64-domain16-code32-class8-retry1-reserved7","bool":"u8-0-or-1","i64":"signed-two-complement-i64","context":"SPXNRCTX1","imports_table":"SPXNRIMP1","result":"caller-owned-uninitialized-success-only","allocator":"none-across-boundary","unwind":"caught-before-ffi-return","threading":"same-thread","reentrancy":"rejected"},"exports":["#);
    for (index, export) in exports.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.text("{\"id\":");
        replay.json(&export.id);
        replay.text(",\"rust_method\":");
        replay.json(&export.rust_method);
        replay.text(",\"c_symbol\":");
        replay.json(&export.c_symbol);
        replay.text(",\"parameters\":[");
        for (parameter_index, parameter) in export.parameters.iter().enumerate() {
            if parameter_index != 0 {
                replay.text(",");
            }
            replay_parameter_exact(&mut replay, parameter);
        }
        replay.text("],\"result\":");
        replay_result_exact(&mut replay, export.result);
        replay.text(",\"effects\":[");
        replay_strings_exact(&mut replay, &export.effects);
        replay.text("],\"capabilities\":[");
        replay_strings_exact(&mut replay, &export.capabilities);
        replay.text("],\"required_imports\":[");
        replay_strings_exact(&mut replay, &export.required_imports);
        replay.text("],\"status_domain_ordinals\":[");
        for (ordinal_index, ordinal) in export.status_domain_ordinals.iter().enumerate() {
            if ordinal_index != 0 {
                replay.text(",");
            }
            replay.number(ordinal);
        }
        replay.text("],\"call_contract_digest\":");
        replay.json(&export.call_contract_digest);
        replay.text("}");
    }
    replay.text("],\"imports\":[");
    for (index, import) in imports.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.text("{\"id\":");
        replay.json(&import.id);
        replay.text(",\"interface\":");
        replay.json(&import.interface);
        replay.text(",\"import_key\":");
        replay.json(&import.import_key);
        replay.text(",\"rust_method\":");
        replay.json(&import.rust_method);
        replay.text(",\"c_field\":");
        replay.json(&import.c_field);
        replay.text(",\"parameters\":[");
        for (parameter_index, parameter) in import.parameters.iter().enumerate() {
            if parameter_index != 0 {
                replay.text(",");
            }
            replay_parameter_exact(&mut replay, parameter);
        }
        replay.text("],\"result\":");
        replay_result_exact(&mut replay, import.result);
        replay.text(",\"effects\":[");
        replay_strings_exact(&mut replay, &import.effects);
        replay.text("],\"capabilities\":[");
        replay_strings_exact(&mut replay, &import.capabilities);
        replay.text("],\"failure\":{\"kind\":");
        if let Some(domain) = &import.failure {
            replay.text("\"status\",\"domain_id\":");
            replay.json(domain);
        } else {
            replay.text("\"infallible\"");
        }
        replay.text("},\"call_contract_digest\":");
        replay.json(&import.call_contract_digest);
        replay.text("}");
    }
    replay.text("],\"limits\":");
    replay_limits_exact(&mut replay);
    replay.text(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(nonclaim);
    }
    replay.text("]}\n");
    replay.finish()
}

pub(in crate::implementation) fn replay_descriptor(
    source: &str,
    spec: &Spec,
    hir_digest: &str,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let source_revision = spec.source_revision().ok_or_else(b108)?;
    replay_descriptor_for_subject(
        source,
        spec,
        DescriptorSubject::SourceRevision(source_revision),
        hir_digest,
        exports,
        imports,
    )
}

pub(in crate::implementation) fn replay_descriptor_for_subject(
    source: &str,
    spec: &Spec,
    subject: DescriptorSubject<'_>,
    hir_digest: &str,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let status_domain_set = imports
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>();
    #[cfg(test)]
    let status_domain_set_owned = checked_owned_string_set(&status_domain_set)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_replay_capacity(status_domain_set_owned);
    let mut status_domains = Vec::with_capacity(status_domain_set.len());
    for domain in status_domain_set {
        status_domains.push(domain);
        #[cfg(test)]
        note_post_hir_replay_capacity(
            status_domain_set_owned
                .checked_add(
                    checked_owned_string_vec(&status_domains, status_domains.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
    }
    if !replay_descriptor_bytes_exact(
        source,
        spec,
        subject,
        hir_digest,
        &status_domains,
        exports,
        imports,
    ) {
        return Err(b108());
    }
    if !source.ends_with('\n') {
        return Err(b108());
    }
    let value: Value = serde_json::from_str(source).map_err(|_| b108())?;
    #[cfg(test)]
    let descriptor_dom_owned = checked_json_value_owned(&value)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let status_domains_owned = checked_owned_string_vec(&status_domains, status_domains.capacity())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_replay_capacity(
        descriptor_dom_owned
            .checked_add(status_domains_owned)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    let row = value.as_object().ok_or_else(b108)?;
    if row.len() != 11
        || row.get("schema").and_then(Value::as_str) != Some(subject.schema())
        || row.get("module").and_then(Value::as_str) != Some(&spec.module)
        || row.get(subject.key()).and_then(Value::as_str) != Some(subject.value())
        || row.contains_key(match subject {
            DescriptorSubject::SourceRevision(_) => "project_subject_digest",
            DescriptorSubject::ProjectSubjectDigest(_) => "source_revision",
        })
        || row.get("hir_digest").and_then(Value::as_str) != Some(hir_digest)
        || row.get("exports").and_then(Value::as_array).map(Vec::len) != Some(exports.len())
        || row.get("imports").and_then(Value::as_array).map(Vec::len) != Some(imports.len())
    {
        return Err(b108());
    }
    let target = row
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(b108)?;
    if target.len() != 5
        || target.get("triple").and_then(Value::as_str) != Some(&spec.target.triple)
        || target.get("pointer_width").and_then(Value::as_u64)
            != Some(u64::from(spec.target.pointer_width))
        || target.get("endian").and_then(Value::as_str) != Some(&spec.target.endian)
        || target.get("panic_strategy").and_then(Value::as_str) != Some(&spec.target.panic_strategy)
        || target.get("thread_policy").and_then(Value::as_str) != Some(&spec.target.thread_policy)
    {
        return Err(b108());
    }
    let expected_statuses = std::iter::once((0_u64, "success"))
        .chain(status_domains.iter().enumerate().map(|(index, domain)| {
            (
                u64::try_from(index + 1).unwrap_or(u64::MAX),
                domain.as_str(),
            )
        }))
        .chain([
            (65_533, "semaprax.native-rust-semantics.v1"),
            (65_534, "semaprax.native-rust-host.v1"),
            (65_535, "semaprax.native-rust-adapter.v1"),
        ])
        .collect::<Vec<_>>();
    #[cfg(test)]
    note_post_hir_replay_capacity(
        descriptor_dom_owned
            .checked_add(status_domains_owned)
            .and_then(|bytes| {
                bytes.checked_add(
                    expected_statuses
                        .capacity()
                        .checked_mul(std::mem::size_of::<(u64, &str)>())?,
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    let statuses = row
        .get("status_domains")
        .and_then(Value::as_array)
        .ok_or_else(b108)?;
    if statuses.len() != expected_statuses.len()
        || statuses
            .iter()
            .zip(&expected_statuses)
            .any(|(value, expected)| {
                value.as_object().is_none_or(|object| {
                    object.len() != 2
                        || object.get("ordinal").and_then(Value::as_u64) != Some(expected.0)
                        || object.get("domain_id").and_then(Value::as_str) != Some(expected.1)
                })
            })
    {
        return Err(b108());
    }
    let abi = row.get("abi").and_then(Value::as_object).ok_or_else(b108)?;
    for (key, expected) in [
        ("calling_convention", "C"),
        ("status_word", "u64-domain16-code32-class8-retry1-reserved7"),
        ("bool", "u8-0-or-1"),
        ("i64", "signed-two-complement-i64"),
        ("context", "SPXNRCTX1"),
        ("imports_table", "SPXNRIMP1"),
        ("result", "caller-owned-uninitialized-success-only"),
        ("allocator", "none-across-boundary"),
        ("unwind", "caught-before-ffi-return"),
        ("threading", "same-thread"),
        ("reentrancy", "rejected"),
    ] {
        if abi.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(b108());
        }
    }
    if abi.len() != 12 || abi.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(b108());
    }
    validate_descriptor_exports(row.get("exports").ok_or_else(b108)?, exports)?;
    validate_descriptor_imports(row.get("imports").ok_or_else(b108)?, imports)?;
    let limits = row
        .get("limits")
        .and_then(Value::as_object)
        .ok_or_else(b108)?;
    if limits.len() != LIMIT_ROWS.len()
        || LIMIT_ROWS.iter().any(|(name, expected)| {
            limits.get(*name).and_then(Value::as_u64) != u64::try_from(*expected).ok()
        })
        || row
            .get("nonclaims")
            .and_then(Value::as_array)
            .is_none_or(|values| {
                values.len() != NONCLAIMS.len()
                    || values
                        .iter()
                        .zip(NONCLAIMS)
                        .any(|(value, expected)| value.as_str() != Some(*expected))
            })
    {
        return Err(b108());
    }
    Ok(())
}

fn validate_parameter_values(value: &Value, expected: &[ParameterFact]) -> Result<(), Diagnostic> {
    let values = value.as_array().ok_or_else(b108)?;
    if values.len() != expected.len() {
        return Err(b108());
    }
    for (value, expected) in values.iter().zip(expected) {
        let row = value.as_object().ok_or_else(b108)?;
        if row.len() != 3
            || row.get("name").and_then(Value::as_str) != Some(&expected.name)
            || row.get("type").and_then(Value::as_str) != Some(scalar_text(expected.ty))
            || row.get("mode").and_then(Value::as_str) != Some("value")
        {
            return Err(b108());
        }
    }
    Ok(())
}

fn validate_result_value(value: &Value, expected: ScalarType) -> Result<(), Diagnostic> {
    let row = value.as_object().ok_or_else(b108)?;
    if row.len() != 2
        || row.get("type").and_then(Value::as_str) != Some(scalar_text(expected))
        || row.get("out_slot").and_then(Value::as_bool) != Some(expected != ScalarType::Unit)
    {
        return Err(b108());
    }
    Ok(())
}

fn strings_equal(value: Option<&Value>, expected: &[String]) -> bool {
    value.and_then(Value::as_array).is_some_and(|values| {
        values.len() == expected.len()
            && values
                .iter()
                .zip(expected)
                .all(|(value, expected)| value.as_str() == Some(expected))
    })
}

fn validate_descriptor_exports(value: &Value, expected: &[ExportFact]) -> Result<(), Diagnostic> {
    let rows = value.as_array().ok_or_else(b108)?;
    if rows.len() != expected.len() {
        return Err(b108());
    }
    for (value, expected) in rows.iter().zip(expected) {
        let row = value.as_object().ok_or_else(b108)?;
        validate_parameter_values(
            row.get("parameters").ok_or_else(b108)?,
            &expected.parameters,
        )?;
        validate_result_value(row.get("result").ok_or_else(b108)?, expected.result)?;
        if row.len() != 10
            || row.get("id").and_then(Value::as_str) != Some(&expected.id)
            || row.get("rust_method").and_then(Value::as_str) != Some(&expected.rust_method)
            || row.get("c_symbol").and_then(Value::as_str) != Some(&expected.c_symbol)
            || !strings_equal(row.get("effects"), &expected.effects)
            || !strings_equal(row.get("capabilities"), &expected.capabilities)
            || !strings_equal(row.get("required_imports"), &expected.required_imports)
            || row
                .get("status_domain_ordinals")
                .and_then(Value::as_array)
                .is_none_or(|values| {
                    values.len() != expected.status_domain_ordinals.len()
                        || values
                            .iter()
                            .zip(&expected.status_domain_ordinals)
                            .any(|(value, expected)| value.as_u64() != Some(u64::from(*expected)))
                })
            || row.get("call_contract_digest").and_then(Value::as_str)
                != Some(&expected.call_contract_digest)
        {
            return Err(b108());
        }
    }
    Ok(())
}

fn validate_descriptor_imports(value: &Value, expected: &[ImportFact]) -> Result<(), Diagnostic> {
    let rows = value.as_array().ok_or_else(b108)?;
    if rows.len() != expected.len() {
        return Err(b108());
    }
    for (value, expected) in rows.iter().zip(expected) {
        let row = value.as_object().ok_or_else(b108)?;
        validate_parameter_values(
            row.get("parameters").ok_or_else(b108)?,
            &expected.parameters,
        )?;
        validate_result_value(row.get("result").ok_or_else(b108)?, expected.result)?;
        let failure = row
            .get("failure")
            .and_then(Value::as_object)
            .ok_or_else(b108)?;
        let valid_failure = expected.failure.as_ref().map_or_else(
            || {
                failure.len() == 1
                    && failure.get("kind").and_then(Value::as_str) == Some("infallible")
            },
            |domain| {
                failure.len() == 2
                    && failure.get("kind").and_then(Value::as_str) == Some("status")
                    && failure.get("domain_id").and_then(Value::as_str) == Some(domain)
            },
        );
        if row.len() != 11
            || row.get("id").and_then(Value::as_str) != Some(&expected.id)
            || row.get("interface").and_then(Value::as_str) != Some(&expected.interface)
            || row.get("import_key").and_then(Value::as_str) != Some(&expected.import_key)
            || row.get("rust_method").and_then(Value::as_str) != Some(&expected.rust_method)
            || row.get("c_field").and_then(Value::as_str) != Some(&expected.c_field)
            || !strings_equal(row.get("effects"), &expected.effects)
            || !strings_equal(row.get("capabilities"), &expected.capabilities)
            || !valid_failure
            || row.get("call_contract_digest").and_then(Value::as_str)
                != Some(&expected.call_contract_digest)
        {
            return Err(b108());
        }
    }
    Ok(())
}
