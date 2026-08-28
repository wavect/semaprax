// Pure deterministic artifact projection and structurally independent replay.
// This module has no filesystem, process, platform, settlement, or publication authority.
use super::*;

fn parameter_json(parameter: &ParameterFact) -> String {
    format!(
        "{{\"name\":{},\"type\":{},\"mode\":\"value\"}}",
        quote_json(&parameter.name),
        quote_json(scalar_text(parameter.ty))
    )
}

fn result_json(result: ScalarType) -> String {
    format!(
        "{{\"type\":{},\"out_slot\":{}}}",
        quote_json(scalar_text(result)),
        result != ScalarType::Unit
    )
}

pub(super) fn replay_limits_exact(replay: &mut ExactReplay<'_>) {
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

pub(super) fn replay_spec_bytes_exact(source: &str, spec: &Spec) -> bool {
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

pub(super) fn render_descriptor_with_limit(
    spec: &Spec,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<String, Diagnostic> {
    let source_revision = spec.source_revision().ok_or_else(b108)?;
    render_descriptor_for_subject_with_limit(
        spec,
        DescriptorSubject::SourceRevision(source_revision),
        hir_digest,
        status_domains,
        exports,
        imports,
        maximum,
    )
}

fn render_descriptor_for_subject_with_limit(
    spec: &Spec,
    subject: DescriptorSubject<'_>,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<String, Diagnostic> {
    let mut statuses = vec!["{\"ordinal\":0,\"domain_id\":\"success\"}".to_owned()];
    statuses.extend(status_domains.iter().enumerate().map(|(index, domain)| {
        format!(
            "{{\"ordinal\":{},\"domain_id\":{}}}",
            index + 1,
            quote_json(domain)
        )
    }));
    statuses
        .push("{\"ordinal\":65533,\"domain_id\":\"semaprax.native-rust-semantics.v1\"}".to_owned());
    statuses.push("{\"ordinal\":65534,\"domain_id\":\"semaprax.native-rust-host.v1\"}".to_owned());
    statuses
        .push("{\"ordinal\":65535,\"domain_id\":\"semaprax.native-rust-adapter.v1\"}".to_owned());
    #[cfg(test)]
    let status_scratch = checked_owned_string_vec(&statuses, statuses.capacity())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut export_row_values = Vec::with_capacity(exports.len());
    for export in exports {
        let id = quote_json(&export.id);
        let rust_method = quote_json(&export.rust_method);
        let c_symbol = quote_json(&export.c_symbol);
        let parameter_values = export
            .parameters
            .iter()
            .map(parameter_json)
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let effects = render_string_array(&export.effects);
        let capabilities = render_string_array(&export.capabilities);
        let required_imports = render_string_array(&export.required_imports);
        let ordinal_values = export
            .status_domain_ordinals
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>();
        let ordinals = ordinal_values.join(",");
        let result = result_json(export.result);
        let call_contract_digest = quote_json(&export.call_contract_digest);
        let row = format!(
            "{{\"id\":{},\"rust_method\":{},\"c_symbol\":{},\"parameters\":[{}],\"result\":{},\"effects\":[{}],\"capabilities\":[{}],\"required_imports\":[{}],\"status_domain_ordinals\":[{}],\"call_contract_digest\":{}}}",
            id,
            rust_method,
            c_symbol,
            parameters,
            result,
            effects,
            capabilities,
            required_imports,
            ordinals,
            call_contract_digest
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            status_scratch
                .saturating_add(
                    checked_owned_string_vec(&export_row_values, export_row_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(
                    checked_owned_string_vec(&parameter_values, parameter_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(
                    checked_owned_string_vec(&ordinal_values, ordinal_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(id.capacity())
                .saturating_add(rust_method.capacity())
                .saturating_add(c_symbol.capacity())
                .saturating_add(parameters.capacity())
                .saturating_add(effects.capacity())
                .saturating_add(capabilities.capacity())
                .saturating_add(required_imports.capacity())
                .saturating_add(ordinals.capacity())
                .saturating_add(result.capacity())
                .saturating_add(call_contract_digest.capacity())
                .saturating_add(row.capacity()),
        );
        export_row_values.push(row);
    }
    let export_rows = export_row_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        status_scratch
            .saturating_add(
                checked_owned_string_vec(&export_row_values, export_row_values.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(export_rows.capacity()),
    );
    drop(export_row_values);
    let mut import_row_values = Vec::with_capacity(imports.len());
    for import in imports {
        let id = quote_json(&import.id);
        let interface = quote_json(&import.interface);
        let import_key = quote_json(&import.import_key);
        let rust_method = quote_json(&import.rust_method);
        let c_field = quote_json(&import.c_field);
        let parameter_values = import
            .parameters
            .iter()
            .map(parameter_json)
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let effects = render_string_array(&import.effects);
        let capabilities = render_string_array(&import.capabilities);
        let failure = import.failure.as_ref().map_or_else(
            || "{\"kind\":\"infallible\"}".to_owned(),
            |domain| {
                format!(
                    "{{\"kind\":\"status\",\"domain_id\":{}}}",
                    quote_json(domain)
                )
            },
        );
        let result = result_json(import.result);
        let call_contract_digest = quote_json(&import.call_contract_digest);
        let row = format!(
            "{{\"id\":{},\"interface\":{},\"import_key\":{},\"rust_method\":{},\"c_field\":{},\"parameters\":[{}],\"result\":{},\"effects\":[{}],\"capabilities\":[{}],\"failure\":{},\"call_contract_digest\":{}}}",
            id,
            interface,
            import_key,
            rust_method,
            c_field,
            parameters,
            result,
            effects,
            capabilities,
            failure,
            call_contract_digest
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            status_scratch
                .saturating_add(export_rows.capacity())
                .saturating_add(
                    checked_owned_string_vec(&import_row_values, import_row_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(
                    checked_owned_string_vec(&parameter_values, parameter_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(id.capacity())
                .saturating_add(interface.capacity())
                .saturating_add(import_key.capacity())
                .saturating_add(rust_method.capacity())
                .saturating_add(c_field.capacity())
                .saturating_add(parameters.capacity())
                .saturating_add(effects.capacity())
                .saturating_add(capabilities.capacity())
                .saturating_add(failure.capacity())
                .saturating_add(result.capacity())
                .saturating_add(call_contract_digest.capacity())
                .saturating_add(row.capacity()),
        );
        import_row_values.push(row);
    }
    let import_rows = import_row_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        status_scratch
            .saturating_add(export_rows.capacity())
            .saturating_add(
                checked_owned_string_vec(&import_row_values, import_row_values.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(import_rows.capacity()),
    );
    drop(import_row_values);
    let schema = quote_json(subject.schema());
    let module = quote_json(&spec.module);
    let subject_value = quote_json(subject.value());
    let hir = quote_json(hir_digest);
    let target = target_json(&spec.target);
    let status_rows = statuses.join(",");
    let limits = limits_json();
    let nonclaims = nonclaims_json();
    #[cfg(test)]
    note_post_hir_render_capacity(
        checked_owned_string_vec(&statuses, statuses.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            .saturating_add(export_rows.capacity())
            .saturating_add(import_rows.capacity())
            .saturating_add(schema.capacity())
            .saturating_add(module.capacity())
            .saturating_add(subject_value.capacity())
            .saturating_add(hir.capacity())
            .saturating_add(target.capacity())
            .saturating_add(status_rows.capacity())
            .saturating_add(limits.capacity())
            .saturating_add(nonclaims.capacity()),
    );
    render_exact_artifact("max_descriptor_bytes", maximum, |sink| {
        write!(
            sink,
            "{{\"schema\":{},\"module\":{},\"{}\":{},\"hir_digest\":{},\"target\":{},\"status_domains\":[{}],\"abi\":{{\"version\":1,\"calling_convention\":\"C\",\"status_word\":\"u64-domain16-code32-class8-retry1-reserved7\",\"bool\":\"u8-0-or-1\",\"i64\":\"signed-two-complement-i64\",\"context\":\"SPXNRCTX1\",\"imports_table\":\"SPXNRIMP1\",\"result\":\"caller-owned-uninitialized-success-only\",\"allocator\":\"none-across-boundary\",\"unwind\":\"caught-before-ffi-return\",\"threading\":\"same-thread\",\"reentrancy\":\"rejected\"}},\"exports\":[{}],\"imports\":[{}],\"limits\":{},\"nonclaims\":[{}]}}\n",
            schema,
            module,
            subject.key(),
            subject_value,
            hir,
            target,
            status_rows,
            export_rows,
            import_rows,
            limits,
            nonclaims
        )
        .map_err(|_| b109("max_descriptor_bytes", MAX_DESCRIPTOR_BYTES))
    })
}

pub(super) fn render_descriptor(
    spec: &Spec,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_descriptor_with_limit(
        spec,
        hir_digest,
        status_domains,
        exports,
        imports,
        MAX_DESCRIPTOR_BYTES,
    )
}

pub(super) fn render_descriptor_for_subject(
    spec: &Spec,
    subject: DescriptorSubject<'_>,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_descriptor_for_subject_with_limit(
        spec,
        subject,
        hir_digest,
        status_domains,
        exports,
        imports,
        MAX_DESCRIPTOR_BYTES,
    )
}

pub(super) fn replay_descriptor(
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

pub(super) fn replay_descriptor_for_subject(
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

fn c_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("{} arg_{index}", c_type(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

pub(super) fn generate_header_with_limit(
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<String, Diagnostic> {
    let mut import_rows = Vec::with_capacity(imports.len());
    for import in imports {
        let params = c_parameters(&import.parameters);
        let out = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", {} *result_out", c_type(import.result))
        };
        let row = format!(
            " spxnr_status_v1 (*{})(void *userdata{}{}{});",
            import.c_field,
            if params.is_empty() { "" } else { ", " },
            params,
            out
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&import_rows)
                .saturating_add(params.capacity())
                .saturating_add(out.capacity())
                .saturating_add(row.capacity()),
        );
        import_rows.push(row);
    }
    let mut export_rows = Vec::with_capacity(exports.len());
    for export in exports {
        let params = c_parameters(&export.parameters);
        let out = if export.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", {} *result_out", c_type(export.result))
        };
        let row = format!(
            "spxnr_status_v1 {}(const spxnr_context_v1 *ctx{}{}{});\n",
            export.c_symbol,
            if params.is_empty() { "" } else { ", " },
            params,
            out
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&import_rows)
                .saturating_add(string_slice_owned_capacity(&export_rows))
                .saturating_add(params.capacity())
                .saturating_add(out.capacity())
                .saturating_add(row.capacity()),
        );
        export_rows.push(row);
    }
    render_exact_artifact("max_generated_header_bytes", maximum, |sink| {
        sink.write_str(
                "#ifndef SEMAPRAX_NATIVE_RUST_INTEROP_H\n#define SEMAPRAX_NATIVE_RUST_INTEROP_H\n#include <stdint.h>\n#include <stddef.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint64_t spxnr_status_v1;\ntypedef struct spxnr_imports_v1 spxnr_imports_v1;\ntypedef struct { uint32_t abi_version; uint32_t size; void *userdata; const spxnr_imports_v1 *imports; uint8_t capabilities_digest[32]; uint32_t call_depth; uint32_t reserved; } spxnr_context_v1;\nstruct spxnr_imports_v1 { uint32_t abi_version; uint32_t size;",
            )
            .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        for row in &import_rows {
            sink.write_str(row)
                .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        }
        sink.write_str(" };\n")
            .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        for row in &export_rows {
            sink.write_str(row)
                .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        }
        sink.write_str("#ifdef __cplusplus\n}\n#endif\n#endif\n")
            .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))
    })
}

pub(super) fn generate_header(
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    generate_header_with_limit(exports, imports, MAX_GENERATED_HEADER_BYTES)
}

#[derive(Clone, Copy)]
enum CExpressionMode {
    Generate,
    Replay,
}

enum CExpressionFrame<'a> {
    Enter(&'a ResolvedExpr),
    Unary(crate::ast::UnaryOp),
    BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    BinaryRight(crate::ast::BinaryOp, String),
    LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    LazyRight(String),
    Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    BlockAssign(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType),
    IfThen(&'a ResolvedExpr, Option<String>),
    IfElse(Option<String>),
    NativeArgs(&'a crate::hir::ResolvedNativeRustImportCall, usize, usize),
    CallArgs(&'a str, &'a [ResolvedExpr], &'a ResolvedType, usize, usize),
}

// Intentionally separate from `CExpressionFrame`: exact replay must not share
// the generator's scheduling state or traversal implementation.
enum ReplayCExpressionFrame<'a> {
    Evaluate(&'a ResolvedExpr),
    FinishUnary(crate::ast::UnaryOp),
    FinishBinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    FinishBinary(crate::ast::BinaryOp, String),
    FinishLazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    FinishLazy(String),
    ContinueBlock(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishBinding(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishAssignment(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType),
    FinishThen(&'a ResolvedExpr, Option<String>),
    FinishElse(Option<String>),
    ContinueNative(&'a crate::hir::ResolvedNativeRustImportCall, usize, usize),
    ContinueCall(&'a str, &'a [ResolvedExpr], &'a ResolvedType, usize, usize),
}

// Pinned by module-local assertions beside the private iterative enums.
pub(super) const C_EXPRESSION_FRAME_BYTES: usize = std::mem::size_of::<CExpressionFrame<'static>>();
pub(super) const REPLAY_C_EXPRESSION_FRAME_BYTES: usize =
    std::mem::size_of::<ReplayCExpressionFrame<'static>>();

/// One fixed backing allocation owns every generated statement byte for one C
/// expression. The final C artifact has a separate reservation; this arena is
/// transient scratch and cannot grow geometrically past the admitted artifact
/// ceiling before the final-size gate observes it.
struct CExpressionLineArena {
    bytes: Box<[u8]>,
    len: usize,
}

impl CExpressionLineArena {
    fn new() -> Self {
        Self {
            bytes: vec![0; MAX_GENERATED_C_BYTES].into_boxed_slice(),
            len: 0,
        }
    }

    fn as_str(&self) -> Result<&str, Diagnostic> {
        std::str::from_utf8(&self.bytes[..self.len]).map_err(|_| b111())
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    #[cfg(test)]
    fn retained_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl std::fmt::Write for CExpressionLineArena {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(std::fmt::Error)?;
        let destination = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        destination.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn c_expression_hash(mode: CExpressionMode, value: &str) -> String {
    match mode {
        CExpressionMode::Generate => full_hash(value),
        CExpressionMode::Replay => replay_symbol_hash(value),
    }
}

fn c_expression_scalar(mode: CExpressionMode, value: ScalarType) -> &'static str {
    match mode {
        CExpressionMode::Generate => c_type(value),
        CExpressionMode::Replay => replay_c_scalar(value),
    }
}

fn c_expression_resolved_scalar(mode: CExpressionMode, value: &ResolvedType) -> Option<ScalarType> {
    match mode {
        CExpressionMode::Generate => scalar_type(value),
        CExpressionMode::Replay => replay_resolved_scalar(value),
    }
}

#[cfg(any())]
fn take_c_lines(lines: &mut Vec<String>) -> String {
    let bytes = lines.iter().map(String::len).sum();
    let mut joined = String::with_capacity(bytes);
    for line in lines.drain(..) {
        joined.push_str(&line);
    }
    joined
}

#[cfg(any())]
fn append_c_lines(output: &mut String, lines: &mut Vec<String>) {
    for line in lines.drain(..) {
        output.push_str(&line);
    }
}

#[cfg(any())]
fn move_root_c_lines(lines: &mut Vec<String>, contexts: &mut [Vec<String>]) {
    let mut root = std::mem::take(&mut contexts[0]);
    if lines.is_empty() {
        std::mem::swap(lines, &mut root);
    } else {
        lines.append(&mut root);
    }
}

pub(super) fn c_expression_shape(expression: &ResolvedExpr) -> Result<(usize, usize), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    stack[0] = Some((expression, 0usize, 1usize));
    let mut stack_len = 1usize;
    let mut nodes = 0usize;
    let mut depth = 1usize;
    while stack_len > 0 {
        let (node, next_child, node_depth) = stack[stack_len - 1].take().ok_or_else(b111)?;
        stack_len -= 1;
        if next_child == 0 {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            depth = depth.max(node_depth);
        }
        let mut child_cursor = next_child;
        if let Some((_, child)) = super::resolved_expression_child(node, &mut child_cursor) {
            if stack_len + 2 > stack.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            stack[stack_len] = Some((node, child_cursor, node_depth));
            stack[stack_len + 1] = Some((child, 0, node_depth + 1));
            stack_len += 2;
        }
    }
    Ok((nodes, depth))
}

fn c_expression_frame_payload(frame: &CExpressionFrame<'_>) -> usize {
    match frame {
        CExpressionFrame::BinaryRight(_, value) | CExpressionFrame::LazyRight(value) => {
            value.capacity()
        }
        CExpressionFrame::IfThen(_, value) | CExpressionFrame::IfElse(value) => {
            value.as_ref().map_or(0, String::capacity)
        }
        _ => 0,
    }
}

fn c_expression_live_string_payload(
    current: &CExpressionFrame<'_>,
    frames: &[CExpressionFrame<'_>],
    values: &[String],
    arguments: &[String],
) -> Option<usize> {
    frames
        .iter()
        .try_fold(c_expression_frame_payload(current), |bytes, frame| {
            bytes.checked_add(c_expression_frame_payload(frame))
        })?
        .checked_add(
            values
                .iter()
                .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))?,
        )?
        .checked_add(
            arguments
                .iter()
                .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))?,
        )
}

#[allow(clippy::ptr_arg)] // Exact Vec capacities are part of the scratch proof.
fn note_c_expression_scratch(
    mode: CExpressionMode,
    current: &CExpressionFrame<'_>,
    frames: &Vec<CExpressionFrame<'_>>,
    values: &Vec<String>,
    arguments: &Vec<String>,
    lines: &CExpressionLineArena,
) -> Result<(), Diagnostic> {
    #[cfg(not(test))]
    let _ = mode;
    #[cfg(not(test))]
    let _ = lines;
    let string_payload = c_expression_live_string_payload(current, frames, values, arguments)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if string_payload > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    #[cfg(test)]
    {
        let working = frames
            .capacity()
            .saturating_mul(C_EXPRESSION_FRAME_BYTES)
            .saturating_add(
                values
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                arguments
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(lines.retained_bytes())
            .saturating_add(string_payload);
        match mode {
            CExpressionMode::Generate => note_post_hir_render_capacity(working),
            CExpressionMode::Replay => note_post_hir_replay_capacity(working),
        }
    }
    Ok(())
}

fn write_c_expression_arguments(
    lines: &mut CExpressionLineArena,
    arguments: &[String],
    separator: &str,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
            lines
                .write_str(separator)
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
        }
        lines
            .write_str(argument)
            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
    }
    Ok(())
}

fn c_expression_linear(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    let mode = CExpressionMode::Generate;
    let (node_count, depth) = c_expression_shape(expression)?;
    let frame_capacity = depth
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::<String>::with_capacity(frame_capacity);
    let mut arguments = Vec::<String>::with_capacity(node_count);
    frames.push(CExpressionFrame::Enter(expression));
    while let Some(frame) = frames.pop() {
        note_c_expression_scratch(mode, &frame, &frames, &values, &arguments, lines)?;
        match frame {
            CExpressionFrame::Enter(expression) => match &expression.kind {
                ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::BorrowPlace { .. }
                | ResolvedExprKind::ByteRange { .. } => {
                    // Non-i64 scalar signatures are outside the scalar
                    // native boundary; admission rejects them first.
                    return Err(b107("scalar value signature required"));
                }
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => values.push(
                    format!("v_{}", c_expression_hash(mode, place.root.as_str())),
                ),
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(CExpressionFrame::NativeArgs(call, 0, arguments.len()));
                }
                ResolvedExprKind::HostCommandCall(_) => {
                    // Command I/O is not part of the public Native Rust SDK
                    // boundary. Closure admission rejects it before emission.
                    return Err(b107("scalar value signature required"));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(CExpressionFrame::Unary(*op));
                    frames.push(CExpressionFrame::Enter(value));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(CExpressionFrame::LazyLeft(*op, right));
                    frames.push(CExpressionFrame::Enter(left));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(CExpressionFrame::BinaryLeft(*op, right));
                    frames.push(CExpressionFrame::Enter(left));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(CExpressionFrame::Block(statements, 0, tail));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = c_expression_resolved_scalar(mode, &expression.ty).ok_or_else(b111)?;
                    frames.push(CExpressionFrame::IfCondition(then_branch, else_branch, ty));
                    frames.push(CExpressionFrame::Enter(condition));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(CExpressionFrame::CallArgs(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        arguments.len(),
                    ));
                }
                ResolvedExprKind::ConstructRecord { .. }
                | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::UpdateRecord { .. }
                | ResolvedExprKind::Project { .. }
                | ResolvedExprKind::Upcast { .. }
                | ResolvedExprKind::Place(_) => {
                    return Err(b107("scalar value signature required"));
                }
            },
            CExpressionFrame::Unary(op) => {
                let value = values.pop().ok_or_else(b111)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(lines, "if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    crate::ast::UnaryOp::Not => values.push(format!("(!({value}))")),
                }
            }
            CExpressionFrame::BinaryLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(CExpressionFrame::BinaryRight(op, left));
                frames.push(CExpressionFrame::Enter(right));
            }
            CExpressionFrame::BinaryRight(op, left) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "int64_t {name};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    match op {
                        crate::ast::BinaryOp::Add => write!(lines, "if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => write!(lines, "if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => write!(lines, "if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    }
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            CExpressionFrame::LazyLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                write!(
                    lines,
                    "uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);if({}){{",
                    if op == crate::ast::BinaryOp::And {
                        name.clone()
                    } else {
                        format!("!{name}")
                    }
                )
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::LazyRight(name));
                frames.push(CExpressionFrame::Enter(right));
            }
            CExpressionFrame::LazyRight(name) => {
                let right = values.pop().ok_or_else(b111)?;
                write!(lines, " {name}=({right})?UINT8_C(1):UINT8_C(0);}}")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                values.push(name);
            }
            CExpressionFrame::Block(statements, index, tail) => match statements.get(index) {
                Some(ResolvedStatement::Let { value, .. }) => {
                    frames.push(CExpressionFrame::BlockLet(statements, index, tail));
                    frames.push(CExpressionFrame::Enter(value));
                }
                Some(statement @ ResolvedStatement::Assign { value, .. }) => {
                    frames.push(CExpressionFrame::BlockAssign(statements, index, tail));
                    frames.push(CExpressionFrame::Enter(value));
                    let _ = statement;
                }
                _ => frames.push(CExpressionFrame::Enter(tail)),
            },
            CExpressionFrame::BlockLet(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "{} v_{} = {value};",
                        c_expression_scalar(mode, ty),
                        c_expression_hash(mode, binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(CExpressionFrame::Block(statements, index + 1, tail));
            }
            CExpressionFrame::BlockAssign(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "v_{} = {value};",
                        c_expression_hash(mode, binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(CExpressionFrame::Block(statements, index + 1, tail));
            }
            CExpressionFrame::IfCondition(then_branch, else_branch, ty) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "{} {name};", c_expression_scalar(mode, ty))
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    Some(name)
                };
                write!(lines, "if({condition}){{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::IfThen(else_branch, name));
                frames.push(CExpressionFrame::Enter(then_branch));
            }
            CExpressionFrame::IfThen(else_branch, name) => {
                let then_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = &name {
                    write!(lines, "{name}={then_value};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                lines
                    .write_str("}else{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::IfElse(name));
                frames.push(CExpressionFrame::Enter(else_branch));
            }
            CExpressionFrame::IfElse(name) => {
                let else_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = name {
                    write!(lines, "{name}={else_value};}}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    lines
                        .write_str("}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push("INT64_C(0)".to_owned());
                }
            }
            CExpressionFrame::NativeArgs(call, index, start) => {
                if index < call.args.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(CExpressionFrame::NativeArgs(call, index + 1, start));
                    frames.push(CExpressionFrame::Enter(&call.args[index]));
                } else {
                    if !call.args.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = format!("tmp_{}", *temporary_count);
                    if import.result != ScalarType::Unit {
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};",
                            c_expression_scalar(mode, import.result)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(
                        lines,
                        "status = ctx->imports->{}(ctx->userdata",
                        import.c_field
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if start != arguments.len() {
                        lines
                            .write_str(", ")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        write_c_expression_arguments(lines, &arguments[start..], ", ")?;
                    }
                    if import.result != ScalarType::Unit {
                        write!(lines, ", &{name}")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(lines, "); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.rust_method)
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if import.result == ScalarType::Bool {
                        write!(lines, "if ({name} > UINT8_C(1)) return spxnr_adapter(4);")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    arguments.truncate(start);
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            CExpressionFrame::CallArgs(callee, source, ty, index, start) => {
                if index < source.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(CExpressionFrame::CallArgs(
                        callee,
                        source,
                        ty,
                        index + 1,
                        start,
                    ));
                    frames.push(CExpressionFrame::Enter(&source[index]));
                } else {
                    if !source.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        write!(
                            lines,
                            "status=spxnr1_f_{}(ctx",
                            c_expression_hash(mode, callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start != arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            write_c_expression_arguments(lines, &arguments[start..], ",")?;
                        }
                        lines
                            .write_str(");if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};status=spxnr1_f_{}(ctx",
                            c_expression_scalar(
                                mode,
                                c_expression_resolved_scalar(mode, ty).ok_or_else(b111)?
                            ),
                            c_expression_hash(mode, callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start != arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            write_c_expression_arguments(lines, &arguments[start..], ",")?;
                        }
                        write!(lines, ",&{name});if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    arguments.truncate(start);
                }
            }
        }
    }
    let terminal = CExpressionFrame::Enter(expression);
    note_c_expression_scratch(mode, &terminal, &frames, &values, &arguments, lines)?;
    if values.len() != 1 || !arguments.is_empty() {
        return Err(b111());
    }
    let result = values.pop().ok_or_else(b111)?;
    if result.capacity() > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    Ok(result)
}

#[cfg(any())]
fn c_context_line_slots(expression: &ResolvedExpr) -> Result<usize, Diagnostic> {
    // A line is owned by exactly one active context. Branch results are
    // collapsed to one String before being appended to their parent, and the
    // drained child Vec is released immediately. Child contexts therefore do
    // not reserve their whole subtree: across all live contexts their logical
    // line count is at most 3N. Vec geometric growth is below twice logical
    // length, so 6N String slots bounds all context backings simultaneously.
    c_expression_shape(expression)?
        .0
        .checked_mul(6)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

#[cfg(any())]
fn c_expr_iterative(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    mut temporary_names: Option<&mut Vec<String>>,
    lines: &mut Vec<String>,
    mode: CExpressionMode,
) -> Result<String, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr, usize),
        Unary(crate::ast::UnaryOp, usize),
        BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        BinaryRight(crate::ast::BinaryOp, String, usize),
        LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        LazyRight(crate::ast::BinaryOp, String, String, usize, usize),
        Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockAssign(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType, usize),
        IfThen(String, &'a ResolvedExpr, Option<String>, usize, usize),
        IfElse(String, Option<String>, String, usize, usize, usize),
        NativeArgs(
            &'a crate::hir::ResolvedNativeRustImportCall,
            usize,
            Vec<String>,
            usize,
        ),
        CallArgs(
            &'a str,
            &'a [ResolvedExpr],
            &'a ResolvedType,
            usize,
            Vec<String>,
            usize,
        ),
    }
    const _: () = assert!(std::mem::size_of::<Frame<'static>>() == C_EXPRESSION_FRAME_BYTES);

    let allocate_temporary =
        |temporary_count: &mut usize, temporary_names: &mut Option<&mut Vec<String>>| {
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            if let Some(names) = temporary_names.as_deref_mut() {
                names.push(name.clone());
            }
            name
        };
    let (node_count, depth) = c_expression_shape(expression)?;
    let line_capacity = node_count
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if lines.capacity() < line_capacity {
        lines
            .try_reserve_exact(line_capacity - lines.capacity())
            .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let frame_capacity = node_count
        .checked_mul(2)
        .and_then(|slots| slots.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    frames.push(Frame::Enter(expression, 0));
    let mut results = Vec::<String>::with_capacity(depth + 1);
    let mut contexts = Vec::with_capacity(node_count + 1);
    contexts.push(Vec::<String>::with_capacity(node_count.saturating_mul(3)));
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let frame_owned = frames
                .iter()
                .map(|frame| match frame {
                    Frame::BinaryRight(_, value, _)
                    | Frame::LazyRight(_, value, _, _, _)
                    | Frame::IfThen(value, _, _, _, _)
                    | Frame::IfElse(value, _, _, _, _, _) => value.capacity(),
                    Frame::NativeArgs(_, _, values, _) | Frame::CallArgs(_, _, _, _, values, _) => {
                        values.capacity() * std::mem::size_of::<String>()
                            + values.iter().map(String::capacity).sum::<usize>()
                    }
                    _ => 0,
                })
                .sum::<usize>();
            let result_owned = results.capacity() * std::mem::size_of::<String>()
                + results.iter().map(String::capacity).sum::<usize>();
            let context_owned = contexts.capacity() * std::mem::size_of::<Vec<String>>()
                + contexts
                    .iter()
                    .map(|context| {
                        context.capacity() * std::mem::size_of::<String>()
                            + context.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>();
            let caller_lines = lines.capacity() * std::mem::size_of::<String>()
                + lines.iter().map(String::capacity).sum::<usize>();
            let persistent_temporaries = temporary_names.as_deref().map_or(0, |names| {
                names.capacity() * std::mem::size_of::<String>()
                    + names.iter().map(String::capacity).sum::<usize>()
            });
            let working = frames.capacity() * std::mem::size_of::<Frame<'_>>()
                + frame_owned
                + result_owned
                + context_owned
                + caller_lines
                + persistent_temporaries;
            match mode {
                CExpressionMode::Generate => note_post_hir_render_capacity(working),
                CExpressionMode::Replay => note_post_hir_replay_capacity(working),
            }
        }
        match frame {
            Frame::Enter(expression, context) => match &expression.kind {
                ResolvedExprKind::Int(value) => results.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    results.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => results.push(
                    format!("v_{}", c_expression_hash(mode, place.root.as_str())),
                ),
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(Frame::NativeArgs(
                        call,
                        0,
                        Vec::with_capacity(call.args.len()),
                        context,
                    ));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(Frame::Unary(*op, context));
                    frames.push(Frame::Enter(value, context));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(Frame::LazyLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(Frame::BinaryLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(Frame::Block(statements, 0, tail, context));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = c_expression_resolved_scalar(mode, &expression.ty).ok_or_else(b111)?;
                    frames.push(Frame::IfCondition(then_branch, else_branch, ty, context));
                    frames.push(Frame::Enter(condition, context));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(Frame::CallArgs(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        Vec::with_capacity(args.len()),
                        context,
                    ));
                }
                ResolvedExprKind::ConstructRecord { .. }
                | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::UpdateRecord { .. }
                | ResolvedExprKind::Project { .. }
                | ResolvedExprKind::Place(_) => {
                    return Err(b107("scalar value signature required"));
                }
            },
            Frame::Unary(op, context) => {
                let value = results.pop().ok_or_else(b111)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let name = allocate_temporary(temporary_count, &mut temporary_names);
                        contexts[context].push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                        results.push(name);
                    }
                    crate::ast::UnaryOp::Not => results.push(format!("(!({value}))")),
                }
            }
            Frame::BinaryLeft(op, right, context) => {
                let left = results.pop().ok_or_else(b111)?;
                frames.push(Frame::BinaryRight(op, left, context));
                frames.push(Frame::Enter(right, context));
            }
            Frame::BinaryRight(op, left, context) => {
                let right = results.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = allocate_temporary(temporary_count, &mut temporary_names);
                    contexts[context].push(format!("int64_t {name};"));
                    contexts[context].push(match op {
                        crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    });
                    results.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    results.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            Frame::LazyLeft(op, right, context) => {
                let left = results.pop().ok_or_else(b111)?;
                let name = allocate_temporary(temporary_count, &mut temporary_names);
                contexts[context].push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
                let branch = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::LazyRight(op, name, left, context, branch));
                frames.push(Frame::Enter(right, branch));
            }
            Frame::LazyRight(op, name, _left, context, branch) => {
                let right = results.pop().ok_or_else(b111)?;
                let condition = if op == crate::ast::BinaryOp::And {
                    name.clone()
                } else {
                    format!("!{name}")
                };
                let branch_lines = take_c_lines(&mut contexts[branch]);
                contexts[branch] = Vec::new();
                contexts[context].push(format!(
                    "if({condition}){{{branch_lines} {name}=({right})?UINT8_C(1):UINT8_C(0);}}"
                ));
                results.push(name);
            }
            Frame::Block(statements, index, tail, context) => {
                if index == statements.len() {
                    frames.push(Frame::Enter(tail, context));
                } else {
                    match &statements[index] {
                        ResolvedStatement::Let { value, .. } => {
                            frames.push(Frame::BlockLet(statements, index, tail, context));
                            frames.push(Frame::Enter(value, context));
                        }
                        ResolvedStatement::Assign { value, .. } => {
                            frames.push(Frame::BlockAssign(statements, index, tail, context));
                            frames.push(Frame::Enter(value, context));
                        }
                    }
                }
            }
            Frame::BlockLet(statements, index, tail, context) => {
                let value = results.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "{} v_{} = {value};",
                        c_expression_scalar(mode, ty),
                        c_expression_hash(mode, binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::BlockAssign(statements, index, tail, context) => {
                let value = results.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "v_{} = {value};",
                        c_expression_hash(mode, binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::IfCondition(then_branch, else_branch, ty, context) => {
                let condition = results.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = allocate_temporary(temporary_count, &mut temporary_names);
                    contexts[context].push(format!("{} {name};", c_expression_scalar(mode, ty)));
                    Some(name)
                };
                let then_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfThen(
                    condition,
                    else_branch,
                    name,
                    context,
                    then_context,
                ));
                frames.push(Frame::Enter(then_branch, then_context));
            }
            Frame::IfThen(condition, else_branch, name, context, then_context) => {
                let then_value = results.pop().ok_or_else(b111)?;
                let else_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfElse(
                    condition,
                    name,
                    then_value,
                    context,
                    then_context,
                    else_context,
                ));
                frames.push(Frame::Enter(else_branch, else_context));
            }
            Frame::IfElse(condition, name, then_value, context, then_context, else_context) => {
                let else_value = results.pop().ok_or_else(b111)?;
                let then_lines = take_c_lines(&mut contexts[then_context]);
                let else_lines = take_c_lines(&mut contexts[else_context]);
                contexts[then_context] = Vec::new();
                contexts[else_context] = Vec::new();
                if let Some(name) = name {
                    contexts[context].push(format!("if({condition}){{{then_lines}{name}={then_value};}}else{{{else_lines}{name}={else_value};}}"));
                    results.push(name);
                } else {
                    contexts[context].push(format!(
                        "if({condition}){{{then_lines}}}else{{{else_lines}}}"
                    ));
                    results.push("INT64_C(0)".to_owned());
                }
            }
            Frame::NativeArgs(call, index, mut args, context) => {
                if index < call.args.len() {
                    if index > 0 {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::NativeArgs(call, index + 1, args, context));
                    frames.push(Frame::Enter(&call.args[index], context));
                } else {
                    if !call.args.is_empty() {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = if import.result == ScalarType::Unit {
                        format!("tmp_{}", *temporary_count)
                    } else {
                        allocate_temporary(temporary_count, &mut temporary_names)
                    };
                    if import.result != ScalarType::Unit {
                        contexts[context].push(format!(
                            "{} {name};",
                            c_expression_scalar(mode, import.result)
                        ));
                    }
                    contexts[context].push(format!("status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.c_field, if args.is_empty() { "" } else { ", " }, args.join(", "), if import.result == ScalarType::Unit { String::new() } else { format!(", &{name}") }, import.rust_method));
                    if import.result == ScalarType::Bool {
                        contexts[context]
                            .push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
                    }
                    results.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            Frame::CallArgs(callee, call_args, ty, index, mut args, context) => {
                if index < call_args.len() {
                    if index > 0 {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::CallArgs(
                        callee,
                        call_args,
                        ty,
                        index + 1,
                        args,
                        context,
                    ));
                    frames.push(Frame::Enter(&call_args[index], context));
                } else {
                    if !call_args.is_empty() {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        contexts[context].push(format!(
                            "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                            c_expression_hash(mode, callee),
                            if args.is_empty() { "" } else { ", " },
                            args.join(",")
                        ));
                        results.push("INT64_C(0)".to_owned());
                    } else {
                        let name = allocate_temporary(temporary_count, &mut temporary_names);
                        let scalar = c_expression_resolved_scalar(mode, ty).ok_or_else(b111)?;
                        contexts[context].push(format!("{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;", c_expression_scalar(mode, scalar), c_expression_hash(mode, callee), if args.is_empty() { "" } else { ", " }, args.join(",")));
                        results.push(name);
                    }
                }
            }
        }
    }
    if results.len() != 1 {
        return Err(b111());
    }
    move_root_c_lines(lines, &mut contexts);
    results.pop().ok_or_else(b111)
}

#[cfg(any())]
fn c_expr(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporaries: &mut Vec<String>,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    let mut count = temporaries.len();
    c_expr_iterative(
        expression,
        imports,
        &mut count,
        Some(temporaries),
        lines,
        CExpressionMode::Generate,
    )
}

fn c_expr(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    c_expression_linear(expression, imports, temporary_count, lines)
}

pub(super) fn generate_c_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let capability_digest = capability_digest(&spec.capabilities);
    let capability_hex = capability_digest.strip_prefix("sha256:").ok_or_else(b111)?;
    let bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "#include \"semaprax_native_rust_interop.h\"\n#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n#include <limits.h>\nstatic const uint8_t spxnr_capabilities[32] = {{{bytes}}};\nstatic spxnr_status_v1 spxnr_adapter(uint32_t code){{return (((uint64_t)65535)<<48)|(((uint64_t)4)<<32)|code;}}\nstatic spxnr_status_v1 spxnr_validate(const spxnr_context_v1 *ctx){{if(!ctx||((uintptr_t)ctx%_Alignof(spxnr_context_v1))!=0)return spxnr_adapter(1);if(ctx->abi_version!=1||ctx->size!=sizeof(*ctx)||ctx->reserved!=0)return spxnr_adapter(1);if(!ctx->imports||((uintptr_t)ctx->imports%_Alignof(spxnr_imports_v1))!=0)return spxnr_adapter(2);if(ctx->imports->abi_version!=1||ctx->imports->size!=sizeof(*ctx->imports))return spxnr_adapter(2);if(memcmp(ctx->capabilities_digest,spxnr_capabilities,32)!=0)return spxnr_adapter(3);if(ctx->call_depth>=32)return spxnr_adapter(7);return 0;}}\n"
    )
    .unwrap();
    if !imports.is_empty() {
        output.write_str("static int spxnr_status_canonical(spxnr_status_v1 status){if(status==0)return 1;uint32_t code=(uint32_t)status;uint8_t class_=(uint8_t)(status>>32);uint8_t retry=(uint8_t)((status>>40)&1);uint8_t reserved=(uint8_t)((status>>41)&0x7f);uint16_t domain=(uint16_t)(status>>48);if(code==0||reserved!=0||domain==0)return 0;if(domain==65533)return retry==0&&((class_==1&&code>=1&&code<=6)||(class_==2&&code>=1&&code<=2));").unwrap();
    }
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .collect::<BTreeSet<_>>();
    for (index, _) in domains.iter().enumerate() {
        write!(output, "if(domain=={})return class_==3;", index + 1).unwrap();
    }
    if !imports.is_empty() {
        output.write_str("if(domain==65534)return class_==4&&retry==0&&code>=1&&code<=2;if(domain==65535)return class_==4&&retry==0&&code>=1&&code<=8;return 0;}\n").unwrap();
    }
    let domain_ordinals = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    for import in imports {
        let custom = import
            .failure
            .as_deref()
            .and_then(|domain| domain_ordinals.get(domain).copied());
        write!(
            output,
            "static int spxnr_status_for_{}(spxnr_status_v1 status){{if(!spxnr_status_canonical(status))return 0;uint16_t domain=(uint16_t)(status>>48);return domain==65534||domain==65535{};}}\n",
            import.rust_method,
            custom.map_or_else(String::new, |ordinal| format!("||domain=={ordinal}"))
        )
        .unwrap();
        write!(output,"static spxnr_status_v1 spxnr_validate_{}(const spxnr_context_v1 *ctx){{return ctx->imports->{}?0:spxnr_adapter(2);}}\n",import.rust_method,import.c_field).unwrap();
    }
    for function in closure {
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type).ok_or_else(b111)?;
        let params = c_parameters(&parameters);
        write!(
            output,
            "static spxnr_status_v1 spxnr1_f_{}(const spxnr_context_v1 *ctx{}{}{});\n",
            full_hash(function.id.as_str()),
            if params.is_empty() { "" } else { ", " },
            params,
            if result == ScalarType::Unit {
                String::new()
            } else {
                format!(", {} *result_out", c_type(result))
            }
        )
        .unwrap();
    }
    for function in closure {
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type).ok_or_else(b111)?;
        let params = c_parameters(&parameters);
        write!(output,"static spxnr_status_v1 spxnr1_f_{}(const spxnr_context_v1 *ctx{}{}{} ){{spxnr_status_v1 status=0;(void)ctx;",full_hash(function.id.as_str()),if params.is_empty(){""}else{", "},params,if result==ScalarType::Unit{String::new()}else{format!(", {} *result_out",c_type(result))}).unwrap();
        for index in 0..parameters.len() {
            write!(output, "(void)arg_{index};").unwrap();
        }
        for (index, (parameter, resolved)) in parameters.iter().zip(&function.params).enumerate() {
            write!(
                output,
                "{} v_{}=arg_{};",
                c_type(parameter.ty),
                full_hash(resolved.id.as_str()),
                index
            )
            .unwrap();
        }
        let mut temporary_count = 0usize;
        let mut lines = CExpressionLineArena::new();
        for requirement in &function.requires {
            lines.clear();
            let value = c_expr(requirement, imports, &mut temporary_count, &mut lines)?;
            output.write_str(lines.as_str()?).unwrap();
            write!(
                output,
                "if(!({value}))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(1);"
            )
            .unwrap();
        }
        lines.clear();
        let value = c_expr(&function.body, imports, &mut temporary_count, &mut lines)?;
        output.write_str(lines.as_str()?).unwrap();
        if result != ScalarType::Unit {
            write!(
                output,
                "{} v_{}={value};",
                c_type(result),
                full_hash(function.result_id.as_str())
            )
            .unwrap();
        }
        for guarantee in &function.ensures {
            lines.clear();
            let value = c_expr(guarantee, imports, &mut temporary_count, &mut lines)?;
            output.write_str(lines.as_str()?).unwrap();
            write!(
                output,
                "if(!({value}))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(2);"
            )
            .unwrap();
        }
        if result != ScalarType::Unit {
            write!(
                output,
                "*result_out=v_{};",
                full_hash(function.result_id.as_str())
            )
            .unwrap();
        }
        output.write_str("return status;}\n").unwrap();
    }
    for export in exports {
        let params = c_parameters(&export.parameters);
        write!(output, "spxnr_status_v1 {}(const spxnr_context_v1 *ctx{}{}{} ){{spxnr_status_v1 status=spxnr_validate(ctx);if(status!=0)return status;", export.c_symbol, if params.is_empty(){""}else{", "}, params, if export.result==ScalarType::Unit{String::new()}else{format!(", {} *result_out",c_type(export.result))}).unwrap();
        for import in imports {
            write!(
                output,
                "status=spxnr_validate_{}(ctx);if(status!=0)return status;",
                import.rust_method
            )
            .unwrap();
        }
        if export.result != ScalarType::Unit {
            write!(
                output,
                "if(!result_out||((uintptr_t)result_out%_Alignof({}))!=0)return spxnr_adapter(5);",
                c_type(export.result)
            )
            .unwrap();
        }
        for (index, parameter) in export.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                write!(output, "if(arg_{index}>1)return spxnr_adapter(4);").unwrap();
            }
        }
        output
            .write_str("spxnr_context_v1 local=*ctx;local.call_depth=ctx->call_depth+1;")
            .unwrap();
        write!(
            output,
            "status=spxnr1_f_{}(&local{}{}{});",
            full_hash(&export.id),
            if export.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            (0..export.parameters.len())
                .map(|index| format!("arg_{index}"))
                .collect::<Vec<_>>()
                .join(","),
            if export.result == ScalarType::Unit {
                String::new()
            } else {
                ", result_out".to_owned()
            }
        )
        .unwrap();
        output.write_str("return status;}\n").unwrap();
    }
    Ok(())
}

pub(super) fn generate_c(
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_exact_artifact("max_generated_c_bytes", MAX_GENERATED_C_BYTES, |sink| {
        generate_c_into(sink, spec, closure, exports, imports)
    })
}

pub(super) fn capability_digest(capabilities: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITIES_DOMAIN);
    for capability in capabilities {
        frame(&mut hasher, capability.as_bytes());
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn rust_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("arg_{index}: {}", rust_type(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

fn generate_safe_rust_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    output.write_str("mod api{#![forbid(unsafe_code)]\nuse core::num::NonZeroU32;\n#[repr(u8)] #[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum NativeRustStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n").unwrap();
    if !imports.is_empty() {
        output.write_str("pub enum NativeRustImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailure}\n").unwrap();
    }
    output.write_str("pub enum NativeRustCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\npub struct NativeRustAdmissionError;\n").unwrap();
    output.write_str("pub trait NativeRustImports{").unwrap();
    for import in imports {
        write!(
            output,
            "fn {}(&mut self{}{})->NativeRustImportResult<{}>;",
            import.rust_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            rust_parameters(&import.parameters),
            rust_type(import.result)
        )
        .unwrap();
    }
    output.write_str("}\n").unwrap();
    let capability_values = spec
        .capabilities
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>();
    let capabilities = capability_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&capability_values).saturating_add(capabilities.capacity()),
    );
    write!(
        output,
        "const EXPECTED_CAPABILITIES:&[&str]=&[{}];\n",
        capabilities
    )
    .unwrap();
    output.write_str("pub struct NativeRustCapabilities{digest:[u8;32]} impl NativeRustCapabilities{pub fn new(values:&[&str])->Result<Self,NativeRustAdmissionError>{if values!=EXPECTED_CAPABILITIES{return Err(NativeRustAdmissionError)}Ok(Self{digest:super::ffi::capabilities_digest()})}}\n").unwrap();
    output.write_str("struct ActiveGuard<'a>{active:&'a mut bool}impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}\npub struct NativeRustBridge<H:NativeRustImports>{host:H,capabilities:NativeRustCapabilities,owner:std::thread::ThreadId,active:bool,calls:u32,_not_send_sync:core::marker::PhantomData<*mut ()>} impl<H:NativeRustImports> NativeRustBridge<H>{pub fn new(host:H,capabilities:NativeRustCapabilities)->Self{Self{host,capabilities,owner:std::thread::current().id(),active:false,calls:0,_not_send_sync:core::marker::PhantomData}}\n").unwrap();
    for export in exports {
        let parameters = rust_parameters(&export.parameters);
        let argument_values = (0..export.parameters.len())
            .map(|index| format!("arg_{index}"))
            .collect::<Vec<_>>();
        let arguments = argument_values.join(", ");
        #[cfg(test)]
        note_post_hir_render_capacity(
            parameters
                .capacity()
                .saturating_add(string_slice_owned_capacity(&argument_values))
                .saturating_add(arguments.capacity()),
        );
        write!(output,"pub fn {}(&mut self{}{})->Result<{},NativeRustCallError>{{if self.owner!=std::thread::current().id()||core::mem::replace(&mut self.active,true){{return Err(NativeRustCallError::AdapterRejected)}}let _active_guard=ActiveGuard{{active:&mut self.active}};super::ffi::{}(&mut self.host,&mut self.calls,self.capabilities.digest{}{})}}\n",export.rust_method,if export.parameters.is_empty(){""}else{", "},parameters,rust_type(export.result),export.rust_method,if export.parameters.is_empty(){""}else{", "},arguments).unwrap();
    }
    output
        .write_str(
            "}\n}\n#[path=\"semaprax_native_rust_interop_ffi.rs\"]mod ffi;\npub use api::*;\n",
        )
        .unwrap();
    Ok(())
}

fn generate_private_ffi_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let digest = capability_digest(&spec.capabilities);
    let hex = digest.strip_prefix("sha256:").unwrap_or("");
    let byte_values = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &hex[index..index + 2]))
        .collect::<Vec<_>>();
    let bytes = byte_values.join(",");
    let mut import_table_values = Vec::with_capacity(imports.len());
    for import in imports {
        let parameter_values = import
            .parameters
            .iter()
            .map(|parameter| match parameter.ty {
                ScalarType::I64 => "i64".to_owned(),
                ScalarType::Bool => "u8".to_owned(),
                ScalarType::Unit => "()".to_owned(),
            })
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let result = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", *mut {}", rust_ffi_wire_type(import.result))
        };
        let row = format!(
            "{}:unsafe extern \"C\" fn(*mut c_void{}{}{})->u64,",
            import.c_field,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            parameters,
            result
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&byte_values)
                .saturating_add(bytes.capacity())
                .saturating_add(string_slice_owned_capacity(&import_table_values))
                .saturating_add(string_slice_owned_capacity(&parameter_values))
                .saturating_add(parameters.capacity())
                .saturating_add(result.capacity())
                .saturating_add(row.capacity()),
        );
        import_table_values.push(row);
    }
    let import_table = import_table_values.join("");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&byte_values)
            .saturating_add(bytes.capacity())
            .saturating_add(string_slice_owned_capacity(&import_table_values))
            .saturating_add(import_table.capacity()),
    );
    write!(output, "#![allow(unsafe_code)]\nuse super::api::*;\nuse core::ffi::c_void;\n#[repr(C)]struct Imports{{abi_version:u32,size:u32,{import_table} }}\n#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}\n").unwrap();
    if !imports.is_empty() {
        output
            .write_str("struct Frame<H>{host:*mut H,calls:*mut u32}\n")
            .unwrap();
    }
    write!(
        output,
        "pub(super) fn capabilities_digest()->[u8;32]{{[{bytes}]}}\n"
    )
    .unwrap();
    #[cfg(test)]
    let ffi_prefix_scratch = digest
        .capacity()
        .saturating_add(string_slice_owned_capacity(&byte_values))
        .saturating_add(bytes.capacity())
        .saturating_add(string_slice_owned_capacity(&import_table_values))
        .saturating_add(import_table.capacity());
    if !imports.is_empty() {
        output.write_str("fn adapter(code:u32)->u64{((65535u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|u64::from(code)}\n").unwrap();
    }
    output.write_str("fn decode_status(status:u64)->NativeRustCallError{let code=(status&0xffff_ffff)as u32;let class=((status>>32)&0xff)as u8;let retryable=((status>>40)&1)!=0;let reserved=(status>>41)&0x7f;let domain=(status>>48)as u16;let Some(code)=core::num::NonZeroU32::new(code)else{return NativeRustCallError::AdapterRejected};let class=match class{1=>NativeRustStatusClass::Semantic,2=>NativeRustStatusClass::Contract,3=>NativeRustStatusClass::Import,4=>NativeRustStatusClass::Adapter,_=>return NativeRustCallError::AdapterRejected};if reserved!=0||domain==0{return NativeRustCallError::AdapterRejected}match domain{65533=>{let valid=!retryable&&match class{NativeRustStatusClass::Semantic=>(1..=6).contains(&code.get()),NativeRustStatusClass::Contract=>(1..=2).contains(&code.get()),_=>false};if !valid{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class,retryable}},").unwrap();
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    for (index, domain) in domains.iter().enumerate() {
        write!(output,"{}=>{{if class!=NativeRustStatusClass::Import{{return NativeRustCallError::AdapterRejected}}NativeRustCallError::Semantic{{domain_id:{},code,class,retryable}}}},",index+1,quote_json(domain)).unwrap();
    }
    output.write_str("65534=>if class==NativeRustStatusClass::Adapter&&!retryable{match code.get(){1=>NativeRustCallError::HostPanicked,2=>NativeRustCallError::HostFailed,_=>NativeRustCallError::AdapterRejected}}else{NativeRustCallError::AdapterRejected},65535=>if class==NativeRustStatusClass::Adapter&&!retryable&&(1..=8).contains(&code.get()){NativeRustCallError::AdapterRejected}else{NativeRustCallError::AdapterRejected},_=>NativeRustCallError::AdapterRejected}}\n").unwrap();
    for import in imports {
        let parameter_declaration_values = import
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                format!(
                    "arg_{index}:{}",
                    match p.ty {
                        ScalarType::I64 => "i64",
                        ScalarType::Bool => "u8",
                        ScalarType::Unit => "()",
                    }
                )
            })
            .collect::<Vec<_>>();
        let parameter_declarations = parameter_declaration_values.join(",");
        let result_declaration = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(
                ", result_out:*mut {}",
                match import.result {
                    ScalarType::I64 => "i64",
                    ScalarType::Bool => "u8",
                    ScalarType::Unit => "()",
                }
            )
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&parameter_declaration_values))
                .saturating_add(parameter_declarations.capacity())
                .saturating_add(result_declaration.capacity()),
        );
        write!(output,"unsafe extern \"C\" fn cb_{}<H:NativeRustImports>(userdata:*mut c_void{}{}{}) -> u64{{if userdata.is_null(){{return adapter(1);}}",import.rust_method,if import.parameters.is_empty(){""}else{", "},parameter_declarations,result_declaration).unwrap();
        for (index, parameter) in import.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                write!(output, "if arg_{index}>1{{return adapter(4);}}").unwrap();
            }
        }
        if import.result != ScalarType::Unit {
            write!(output,"if result_out.is_null()||(result_out as usize)%core::mem::align_of::<{}>()!=0{{return adapter(5);}}",rust_type(import.result)).unwrap();
        }
        let call_argument_values = import
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if p.ty == ScalarType::Bool {
                    format!("arg_{index}!=0")
                } else {
                    format!("arg_{index}")
                }
            })
            .collect::<Vec<_>>();
        let call_arguments = call_argument_values.join(",");
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&call_argument_values))
                .saturating_add(call_arguments.capacity()),
        );
        write!(output,"if (userdata as usize)%core::mem::align_of::<Frame<H>>()!=0{{return adapter(1);}}let frame=&mut*(userdata as *mut Frame<H>);if frame.host.is_null()||frame.calls.is_null()||*frame.calls>=4096{{return adapter(7);}}*frame.calls+=1;let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{{let host=&mut *frame.host;host.{}({})}}));match run{{Err(payload)=>{{core::mem::forget(payload);((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|1}},Ok(NativeRustImportResult::HostFailure)=>((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|2,",import.rust_method,call_arguments).unwrap();
        let ordinal = import
            .failure
            .as_ref()
            .and_then(|domain| domains.iter().position(|value| value == domain))
            .map(|index| index + 1);
        if let Some(ordinal) = ordinal {
            write!(output,"Ok(NativeRustImportResult::Status{{code,class,retryable}})=>if class==NativeRustStatusClass::Import{{(({}u64)<<48)|((class as u64)<<32)|((retryable as u64)<<40)|u64::from(code.get())}}else{{adapter(3)}},",ordinal).unwrap();
        } else {
            output.write_str("Ok(NativeRustImportResult::Status{code,class,retryable})=>{let _=(code,class,retryable);adapter(3)},").unwrap();
        }
        if import.result == ScalarType::Unit {
            output
                .write_str("Ok(NativeRustImportResult::Success(()))=>0}}}\n")
                .unwrap();
        } else {
            write!(
                output,
                "Ok(NativeRustImportResult::Success(value))=>{{*result_out={};0}}",
                if import.result == ScalarType::Bool {
                    "u8::from(value)"
                } else {
                    "value"
                }
            )
            .unwrap();
            output.write_str("}}\n").unwrap();
        }
    }
    for export in exports {
        let parameter_values = export
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| format!("arg_{index}:{}", rust_ffi_wire_type(parameter.ty)))
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let result = if export.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", result_out:*mut {}", rust_ffi_wire_type(export.result))
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&parameter_values))
                .saturating_add(parameters.capacity())
                .saturating_add(result.capacity()),
        );
        write!(
            output,
            "extern \"C\"{{fn {}(ctx:*const Context{}{}{})->u64;}}\n",
            export.c_symbol,
            if export.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            parameters,
            result
        )
        .unwrap();
    }
    for export in exports {
        let result_slot = match export.result {
            ScalarType::Unit => String::new(),
            ScalarType::I64 => "let mut result=core::mem::MaybeUninit::<i64>::uninit();".to_owned(),
            ScalarType::Bool => "let mut result=core::mem::MaybeUninit::<u8>::uninit();".to_owned(),
        };
        let publish = match export.result {
            ScalarType::Unit => "Ok(())",
            ScalarType::I64 => "Ok(result.assume_init())",
            ScalarType::Bool => {
                "let value=result.assume_init();if value>1{return Err(NativeRustCallError::AdapterRejected)}Ok(value!=0)"
            }
        };
        let parameters = rust_parameters(&export.parameters);
        let callback_values = imports
            .iter()
            .map(|import| format!("{}:cb_{}::<H>,", import.c_field, import.rust_method))
            .collect::<Vec<_>>();
        let callbacks = callback_values.join("");
        let argument_values = export
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if p.ty == ScalarType::Bool {
                    format!("u8::from(arg_{index})")
                } else {
                    format!("arg_{index}")
                }
            })
            .collect::<Vec<_>>();
        let arguments = argument_values.join(",");
        let result_argument = if export.result == ScalarType::Unit {
            String::new()
        } else {
            ", result.as_mut_ptr()".to_owned()
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(
                    parameters
                        .capacity()
                        .saturating_add(string_slice_owned_capacity(&callback_values))
                        .saturating_add(callbacks.capacity())
                        .saturating_add(result_slot.capacity())
                        .saturating_add(string_slice_owned_capacity(&argument_values))
                        .saturating_add(arguments.capacity())
                        .saturating_add(result_argument.capacity()),
                ),
        );
        let frame = if imports.is_empty() {
            "let _=host;let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};"
        } else {
            "let mut frame=Frame{host:host as *mut H,calls:calls as *mut u32};let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:&mut frame as *mut Frame<H> as *mut c_void,imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};"
        };
        write!(output,"pub(super) fn {}<H:NativeRustImports>(host:&mut H,calls:&mut u32,digest:[u8;32]{}{})->Result<{},NativeRustCallError>{{unsafe{{if *calls>=4096{{return Err(NativeRustCallError::AdapterRejected)}}*calls+=1;let table=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,{}}};{}{}let status={}(&ctx{}{}{});if status!=0{{return Err(decode_status(status))}}{} }}}}\n",export.rust_method,if export.parameters.is_empty(){""}else{", "},parameters,rust_type(export.result),callbacks,frame,result_slot,export.c_symbol,if export.parameters.is_empty(){""}else{", "},arguments,result_argument,publish).unwrap();
    }
    Ok(())
}

pub(super) fn generate_rust_artifacts_with_limit(
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<(String, String), Diagnostic> {
    let mut render_safe =
        |sink: &mut dyn std::fmt::Write| generate_safe_rust_into(sink, spec, exports, imports);
    let mut render_ffi =
        |sink: &mut dyn std::fmt::Write| generate_private_ffi_into(sink, spec, exports, imports);
    let safe_bytes = count_exact_artifact("max_generated_rust_bytes", maximum, &mut render_safe)?;
    let ffi_bytes = count_exact_artifact("max_generated_rust_bytes", maximum, &mut render_ffi)?;
    let combined_bytes = safe_bytes
        .checked_add(ffi_bytes)
        .ok_or_else(|| b109("max_generated_rust_bytes", maximum))?;
    if combined_bytes > maximum {
        return Err(b109("max_generated_rust_bytes", maximum));
    }
    let safe = render_counted_artifact(
        "max_generated_rust_bytes",
        maximum,
        safe_bytes,
        &mut render_safe,
    )?;
    let ffi = render_counted_artifact(
        "max_generated_rust_bytes",
        maximum,
        ffi_bytes,
        &mut render_ffi,
    )?;
    Ok((safe, ffi))
}

pub(super) fn generate_rust_artifacts(
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(String, String), Diagnostic> {
    generate_rust_artifacts_with_limit(spec, exports, imports, MAX_GENERATED_RUST_BYTES)
}

pub(super) fn replay_generated(
    header: &str,
    c: &str,
    rust: &str,
    ffi: &str,
) -> Result<(), Diagnostic> {
    if !header.starts_with("#ifndef ")
        || !header.ends_with("#endif\n")
        || !c.starts_with("#include \"semaprax_native_rust_interop.h\"")
        || !rust.starts_with("mod api{#![forbid(unsafe_code)]\n")
        || rust.contains("unsafe {")
        || !ffi.starts_with("#![allow(unsafe_code)]\n")
    {
        return Err(b111());
    }
    Ok(())
}

fn replay_header_exact(source: &str, exports: &[ExportFact], imports: &[ImportFact]) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("#ifndef SEMAPRAX_NATIVE_RUST_INTEROP_H\n#define SEMAPRAX_NATIVE_RUST_INTEROP_H\n#include <stdint.h>\n#include <stddef.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint64_t spxnr_status_v1;\ntypedef struct spxnr_imports_v1 spxnr_imports_v1;\ntypedef struct { uint32_t abi_version; uint32_t size; void *userdata; const spxnr_imports_v1 *imports; uint8_t capabilities_digest[32]; uint32_t call_depth; uint32_t reserved; } spxnr_context_v1;\nstruct spxnr_imports_v1 { uint32_t abi_version; uint32_t size;");
    for import in imports {
        replay.text(" spxnr_status_v1 (*");
        replay.text(&import.c_field);
        replay.text(")(void *userdata");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(", ");
            replay.text(c_type(parameter.ty));
            replay.text(" arg_");
            replay.number(index);
        }
        if import.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(c_type(import.result));
            replay.text(" *result_out");
        }
        replay.text(");");
    }
    replay.text(" };\n");
    for export in exports {
        replay.text("spxnr_status_v1 ");
        replay.text(&export.c_symbol);
        replay.text("(const spxnr_context_v1 *ctx");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(", ");
            replay.text(c_type(parameter.ty));
            replay.text(" arg_");
            replay.number(index);
        }
        if export.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(c_type(export.result));
            replay.text(" *result_out");
        }
        replay.text(");\n");
    }
    replay.text("#ifdef __cplusplus\n}\n#endif\n#endif\n");
    replay.finish()
}

fn replay_rust_scalar(replay: &mut ExactReplay<'_>, ty: ScalarType) {
    replay.text(match ty {
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
        ScalarType::Unit => "()",
    });
}

fn replay_rust_parameters(replay: &mut ExactReplay<'_>, parameters: &[ParameterFact]) {
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            replay.text(", ");
        }
        replay.text("arg_");
        replay.number(index);
        replay.text(": ");
        replay_rust_scalar(replay, parameter.ty);
    }
}

fn replay_safe_rust_exact(
    source: &str,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("mod api{#![forbid(unsafe_code)]\nuse core::num::NonZeroU32;\n#[repr(u8)] #[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum NativeRustStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n");
    if !imports.is_empty() {
        replay.text("pub enum NativeRustImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailure}\n");
    }
    replay.text("pub enum NativeRustCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\npub struct NativeRustAdmissionError;\n");
    replay.text("pub trait NativeRustImports{");
    for import in imports {
        replay.text("fn ");
        replay.text(&import.rust_method);
        replay.text("(&mut self");
        if !import.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &import.parameters);
        }
        replay.text(")->NativeRustImportResult<");
        replay_rust_scalar(&mut replay, import.result);
        replay.text(">;");
    }
    replay.text("}\nconst EXPECTED_CAPABILITIES:&[&str]=&[");
    for (index, capability) in spec.capabilities.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(capability);
    }
    replay.text("];\n");
    replay.text("pub struct NativeRustCapabilities{digest:[u8;32]} impl NativeRustCapabilities{pub fn new(values:&[&str])->Result<Self,NativeRustAdmissionError>{if values!=EXPECTED_CAPABILITIES{return Err(NativeRustAdmissionError)}Ok(Self{digest:super::ffi::capabilities_digest()})}}\n");
    replay.text("struct ActiveGuard<'a>{active:&'a mut bool}impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}\npub struct NativeRustBridge<H:NativeRustImports>{host:H,capabilities:NativeRustCapabilities,owner:std::thread::ThreadId,active:bool,calls:u32,_not_send_sync:core::marker::PhantomData<*mut ()>} impl<H:NativeRustImports> NativeRustBridge<H>{pub fn new(host:H,capabilities:NativeRustCapabilities)->Self{Self{host,capabilities,owner:std::thread::current().id(),active:false,calls:0,_not_send_sync:core::marker::PhantomData}}\n");
    for export in exports {
        replay.text("pub fn ");
        replay.text(&export.rust_method);
        replay.text("(&mut self");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &export.parameters);
        }
        replay.text(")->Result<");
        replay_rust_scalar(&mut replay, export.result);
        replay.text(",NativeRustCallError>{if self.owner!=std::thread::current().id()||core::mem::replace(&mut self.active,true){return Err(NativeRustCallError::AdapterRejected)}let _active_guard=ActiveGuard{active:&mut self.active};super::ffi::");
        replay.text(&export.rust_method);
        replay.text("(&mut self.host,&mut self.calls,self.capabilities.digest");
        for index in 0..export.parameters.len() {
            replay.text(", arg_");
            replay.number(index);
        }
        replay.text(")}\n");
    }
    replay.text("}\n}\n#[path=\"semaprax_native_rust_interop_ffi.rs\"]mod ffi;\npub use api::*;\n");
    replay.finish()
}

fn replay_ffi_wire_scalar(replay: &mut ExactReplay<'_>, ty: ScalarType) {
    replay.text(match ty {
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
        ScalarType::Unit => "()",
    });
}

fn replay_private_ffi_exact(
    source: &str,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("#![allow(unsafe_code)]\nuse super::api::*;\nuse core::ffi::c_void;\n#[repr(C)]struct Imports{abi_version:u32,size:u32,");
    for import in imports {
        replay.text(&import.c_field);
        replay.text(":unsafe extern \"C\" fn(*mut c_void");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", " } else { "," });
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if import.result != ScalarType::Unit {
            replay.text(", *mut ");
            replay_ffi_wire_scalar(&mut replay, import.result);
        }
        replay.text(")->u64,");
    }
    replay.text(" }\n#[repr(C)]struct Context{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}\n");
    if !imports.is_empty() {
        replay.text("struct Frame<H>{host:*mut H,calls:*mut u32}\n");
    }
    replay.text("pub(super) fn capabilities_digest()->[u8;32]{[");
    let digest = replay_capabilities_digest(&spec.capabilities);
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    for index in (0..64).step_by(2) {
        if index != 0 {
            replay.text(",");
        }
        replay.text("0x");
        replay.text(&hex[index..index + 2]);
    }
    replay.text("]}\n");
    if !imports.is_empty() {
        replay.text("fn adapter(code:u32)->u64{((65535u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|u64::from(code)}\n");
    }
    replay.text("fn decode_status(status:u64)->NativeRustCallError{let code=(status&0xffff_ffff)as u32;let class=((status>>32)&0xff)as u8;let retryable=((status>>40)&1)!=0;let reserved=(status>>41)&0x7f;let domain=(status>>48)as u16;let Some(code)=core::num::NonZeroU32::new(code)else{return NativeRustCallError::AdapterRejected};let class=match class{1=>NativeRustStatusClass::Semantic,2=>NativeRustStatusClass::Contract,3=>NativeRustStatusClass::Import,4=>NativeRustStatusClass::Adapter,_=>return NativeRustCallError::AdapterRejected};if reserved!=0||domain==0{return NativeRustCallError::AdapterRejected}match domain{65533=>{let valid=!retryable&&match class{NativeRustStatusClass::Semantic=>(1..=6).contains(&code.get()),NativeRustStatusClass::Contract=>(1..=2).contains(&code.get()),_=>false};if !valid{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class,retryable}},");
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    for (index, domain) in domains.iter().enumerate() {
        replay.number(index + 1);
        replay.text("=>{if class!=NativeRustStatusClass::Import{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:");
        replay.json(domain);
        replay.text(",code,class,retryable}},");
    }
    replay.text("65534=>if class==NativeRustStatusClass::Adapter&&!retryable{match code.get(){1=>NativeRustCallError::HostPanicked,2=>NativeRustCallError::HostFailed,_=>NativeRustCallError::AdapterRejected}}else{NativeRustCallError::AdapterRejected},65535=>if class==NativeRustStatusClass::Adapter&&!retryable&&(1..=8).contains(&code.get()){NativeRustCallError::AdapterRejected}else{NativeRustCallError::AdapterRejected},_=>NativeRustCallError::AdapterRejected}}\n");
    for import in imports {
        replay.text("unsafe extern \"C\" fn cb_");
        replay.text(&import.rust_method);
        replay.text("<H:NativeRustImports>(userdata:*mut c_void");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", arg_" } else { ",arg_" });
            replay.number(index);
            replay.text(":");
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if import.result != ScalarType::Unit {
            replay.text(", result_out:*mut ");
            replay_ffi_wire_scalar(&mut replay, import.result);
        }
        replay.text(") -> u64{if userdata.is_null(){return adapter(1);}");
        for (index, parameter) in import.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                replay.text("if arg_");
                replay.number(index);
                replay.text(">1{return adapter(4);}");
            }
        }
        if import.result != ScalarType::Unit {
            replay.text("if result_out.is_null()||(result_out as usize)%core::mem::align_of::<");
            replay_rust_scalar(&mut replay, import.result);
            replay.text(">()!=0{return adapter(5);}");
        }
        replay.text("if (userdata as usize)%core::mem::align_of::<Frame<H>>()!=0{return adapter(1);}let frame=&mut*(userdata as *mut Frame<H>);if frame.host.is_null()||frame.calls.is_null()||*frame.calls>=4096{return adapter(7);}*frame.calls+=1;let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let host=&mut *frame.host;host.");
        replay.text(&import.rust_method);
        replay.text("(");
        for (index, parameter) in import.parameters.iter().enumerate() {
            if index != 0 {
                replay.text(",");
            }
            replay.text("arg_");
            replay.number(index);
            if parameter.ty == ScalarType::Bool {
                replay.text("!=0");
            }
        }
        replay.text(")}));match run{Err(payload)=>{core::mem::forget(payload);((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|1},Ok(NativeRustImportResult::HostFailure)=>((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|2,");
        if let Some(domain) = &import.failure {
            let Some(ordinal) = domains.iter().position(|value| value == domain) else {
                return false;
            };
            replay.text("Ok(NativeRustImportResult::Status{code,class,retryable})=>if class==NativeRustStatusClass::Import{((");
            replay.number(ordinal + 1);
            replay.text("u64)<<48)|((class as u64)<<32)|((retryable as u64)<<40)|u64::from(code.get())}else{adapter(3)},");
        } else {
            replay.text("Ok(NativeRustImportResult::Status{code,class,retryable})=>{let _=(code,class,retryable);adapter(3)},");
        }
        if import.result == ScalarType::Unit {
            replay.text("Ok(NativeRustImportResult::Success(()))=>0}}}\n");
        } else {
            replay.text("Ok(NativeRustImportResult::Success(value))=>{*result_out=");
            if import.result == ScalarType::Bool {
                replay.text("u8::from(value)");
            } else {
                replay.text("value");
            }
            replay.text(";0}}}\n");
        }
    }
    for export in exports {
        replay.text("extern \"C\"{fn ");
        replay.text(&export.c_symbol);
        replay.text("(ctx:*const Context");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", arg_" } else { ",arg_" });
            replay.number(index);
            replay.text(":");
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if export.result != ScalarType::Unit {
            replay.text(", result_out:*mut ");
            replay_ffi_wire_scalar(&mut replay, export.result);
        }
        replay.text(")->u64;}\n");
    }
    for export in exports {
        replay.text("pub(super) fn ");
        replay.text(&export.rust_method);
        replay.text("<H:NativeRustImports>(host:&mut H,calls:&mut u32,digest:[u8;32]");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &export.parameters);
        }
        replay.text(")->Result<");
        replay_rust_scalar(&mut replay, export.result);
        replay.text(",NativeRustCallError>{unsafe{if *calls>=4096{return Err(NativeRustCallError::AdapterRejected)}*calls+=1;let table=Imports{abi_version:1,size:core::mem::size_of::<Imports>() as u32,");
        for import in imports {
            replay.text(&import.c_field);
            replay.text(":cb_");
            replay.text(&import.rust_method);
            replay.text("::<H>,");
        }
        replay.text("};");
        if imports.is_empty() {
            replay.text("let _=host;let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};");
        } else {
            replay.text("let mut frame=Frame{host:host as *mut H,calls:calls as *mut u32};let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:&mut frame as *mut Frame<H> as *mut c_void,imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};");
        }
        match export.result {
            ScalarType::Unit => {}
            ScalarType::I64 => {
                replay.text("let mut result=core::mem::MaybeUninit::<i64>::uninit();")
            }
            ScalarType::Bool => {
                replay.text("let mut result=core::mem::MaybeUninit::<u8>::uninit();")
            }
        }
        replay.text("let status=");
        replay.text(&export.c_symbol);
        replay.text("(&ctx");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", " } else { "," });
            if parameter.ty == ScalarType::Bool {
                replay.text("u8::from(");
            }
            replay.text("arg_");
            replay.number(index);
            if parameter.ty == ScalarType::Bool {
                replay.text(")");
            }
        }
        if export.result != ScalarType::Unit {
            replay.text(", result.as_mut_ptr()");
        }
        replay.text(");if status!=0{return Err(decode_status(status))}");
        match export.result {
            ScalarType::Unit => replay.text("Ok(())"),
            ScalarType::I64 => replay.text("Ok(result.assume_init())"),
            ScalarType::Bool => replay.text("let value=result.assume_init();if value>1{return Err(NativeRustCallError::AdapterRejected)}Ok(value!=0)"),
        }
        replay.text(" }}\n");
    }
    replay.finish()
}

fn replay_c_scalar(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
        ScalarType::Unit => "void",
    }
}

pub(super) fn replay_symbol_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(encoded, "{byte:02x}").unwrap();
    }
    #[cfg(test)]
    note_post_hir_replay_capacity(encoded.capacity());
    encoded
}

pub(super) fn replay_capabilities_digest(capabilities: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITIES_DOMAIN);
    for capability in capabilities {
        hasher.update(
            u64::try_from(capability.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(capability.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    note_post_hir_replay_capacity(digest.capacity());
    digest
}

fn replay_resolved_scalar(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

fn replay_parameter_facts(function: &ResolvedFunction) -> Result<Vec<ParameterFact>, Diagnostic> {
    if function.params.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    function
        .params
        .iter()
        .map(|parameter| {
            if parameter.ownership != OwnershipMode::Value
                || parameter.name.len() > MAX_IDENTIFIER_BYTES
            {
                return Err(b107("scalar value signature required"));
            }
            Ok(ParameterFact {
                name: parameter.name.clone(),
                ty: replay_resolved_scalar(&parameter.ty)
                    .filter(|ty| *ty != ScalarType::Unit)
                    .ok_or_else(|| b107("scalar value signature required"))?,
            })
        })
        .collect()
}

fn replay_c_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("{} arg_{index}", replay_c_scalar(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_replay_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

#[cfg(any())]
fn replay_c_expression(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr, usize),
        Unary(crate::ast::UnaryOp, usize),
        BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        BinaryRight(crate::ast::BinaryOp, String, usize),
        LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        LazyRight(crate::ast::BinaryOp, String, usize, usize),
        Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockAssign(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType, usize),
        IfThen(String, &'a ResolvedExpr, Option<String>, usize, usize),
        IfElse(String, Option<String>, String, usize, usize, usize),
        NativeArgs(
            &'a crate::hir::ResolvedNativeRustImportCall,
            usize,
            Vec<String>,
            usize,
        ),
        CallArgs(
            &'a str,
            &'a [ResolvedExpr],
            &'a ResolvedType,
            usize,
            Vec<String>,
            usize,
        ),
    }
    const _: () = assert!(std::mem::size_of::<Frame<'static>>() == C_EXPRESSION_FRAME_BYTES);
    let next_temporary = |count: &mut usize| {
        let value = format!("tmp_{}", *count);
        *count += 1;
        value
    };
    let (node_count, depth) = c_expression_shape(expression)?;
    let line_capacity = node_count
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if lines.capacity() < line_capacity {
        lines
            .try_reserve_exact(line_capacity - lines.capacity())
            .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let frame_capacity = node_count
        .checked_mul(2)
        .and_then(|slots| slots.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::<String>::with_capacity(depth + 1);
    let mut contexts = Vec::<Vec<String>>::with_capacity(node_count + 1);
    contexts.push(Vec::with_capacity(line_capacity));
    frames.push(Frame::Enter(expression, 0));
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let frame_payload = |frame: &Frame<'_>| match frame {
                Frame::BinaryRight(_, value, _)
                | Frame::LazyRight(_, value, _, _)
                | Frame::IfThen(value, _, _, _, _)
                | Frame::IfElse(value, _, _, _, _, _) => value.capacity(),
                Frame::NativeArgs(_, _, values, _) | Frame::CallArgs(_, _, _, _, values, _) => {
                    values.capacity() * std::mem::size_of::<String>()
                        + values.iter().map(String::capacity).sum::<usize>()
                }
                _ => 0,
            };
            let frame_owned = frames.iter().map(&frame_payload).sum::<usize>();
            let owned = frames.capacity() * std::mem::size_of::<Frame<'_>>()
                + frame_owned
                + frame_payload(&frame)
                + values.capacity() * std::mem::size_of::<String>()
                + values.iter().map(String::capacity).sum::<usize>()
                + contexts.capacity() * std::mem::size_of::<Vec<String>>()
                + contexts
                    .iter()
                    .map(|context| {
                        context.capacity() * std::mem::size_of::<String>()
                            + context.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>()
                + lines.capacity() * std::mem::size_of::<String>()
                + lines.iter().map(String::capacity).sum::<usize>();
            note_post_hir_replay_capacity(owned);
        }
        match frame {
            Frame::Enter(expression, context) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                    values.push(format!("v_{}", replay_symbol_hash(place.root.as_str())))
                }
                ResolvedExprKind::NativeRustImportCall(call) => frames.push(Frame::NativeArgs(
                    call,
                    0,
                    Vec::with_capacity(call.args.len()),
                    context,
                )),
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(Frame::Unary(*op, context));
                    frames.push(Frame::Enter(value, context));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(Frame::LazyLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(Frame::BinaryLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(Frame::Block(statements, 0, tail, context));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = replay_resolved_scalar(&expression.ty).ok_or_else(b111)?;
                    frames.push(Frame::IfCondition(then_branch, else_branch, ty, context));
                    frames.push(Frame::Enter(condition, context));
                }
                ResolvedExprKind::Call { callee, args, .. } => frames.push(Frame::CallArgs(
                    callee.as_str(),
                    args,
                    &expression.ty,
                    0,
                    Vec::with_capacity(args.len()),
                    context,
                )),
                _ => return Err(b107("scalar value signature required")),
            },
            Frame::Unary(op, context) => {
                let value = values.pop().ok_or_else(b111)?;
                if op == crate::ast::UnaryOp::Not {
                    values.push(format!("(!({value}))"));
                } else {
                    let name = next_temporary(temporary_count);
                    contexts[context].push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                    values.push(name);
                }
            }
            Frame::BinaryLeft(op, right, context) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(Frame::BinaryRight(op, left, context));
                frames.push(Frame::Enter(right, context));
            }
            Frame::BinaryRight(op, left, context) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = next_temporary(temporary_count);
                    contexts[context].push(format!("int64_t {name};"));
                    contexts[context].push(match op {
                        crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    });
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            Frame::LazyLeft(op, right, context) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = next_temporary(temporary_count);
                contexts[context].push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
                let branch = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::LazyRight(op, name, context, branch));
                frames.push(Frame::Enter(right, branch));
            }
            Frame::LazyRight(op, name, context, branch) => {
                let right = values.pop().ok_or_else(b111)?;
                let branch_lines = take_c_lines(&mut contexts[branch]);
                contexts[branch] = Vec::new();
                let condition = if op == crate::ast::BinaryOp::And {
                    name.clone()
                } else {
                    format!("!{name}")
                };
                contexts[context].push(format!(
                    "if({condition}){{{branch_lines} {name}=({right})?UINT8_C(1):UINT8_C(0);}}"
                ));
                values.push(name);
            }
            Frame::Block(statements, index, tail, context) => match statements.get(index) {
                Some(ResolvedStatement::Let { value, .. }) => {
                    frames.push(Frame::BlockLet(statements, index, tail, context));
                    frames.push(Frame::Enter(value, context));
                }
                Some(ResolvedStatement::Assign { value, .. }) => {
                    frames.push(Frame::BlockAssign(statements, index, tail, context));
                    frames.push(Frame::Enter(value, context));
                }
                _ => frames.push(Frame::Enter(tail, context)),
            },
            Frame::BlockLet(statements, index, tail, context) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::BlockAssign(statements, index, tail, context) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "v_{} = {value};",
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::IfCondition(then_branch, else_branch, ty, context) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = (ty != ScalarType::Unit).then(|| next_temporary(temporary_count));
                if let Some(name) = &name {
                    contexts[context].push(format!("{} {name};", replay_c_scalar(ty)));
                }
                let then_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfThen(
                    condition,
                    else_branch,
                    name,
                    context,
                    then_context,
                ));
                frames.push(Frame::Enter(then_branch, then_context));
            }
            Frame::IfThen(condition, else_branch, name, context, then_context) => {
                let then_value = values.pop().ok_or_else(b111)?;
                let else_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfElse(
                    condition,
                    name,
                    then_value,
                    context,
                    then_context,
                    else_context,
                ));
                frames.push(Frame::Enter(else_branch, else_context));
            }
            Frame::IfElse(condition, name, then_value, context, then_context, else_context) => {
                let else_value = values.pop().ok_or_else(b111)?;
                let then_lines = take_c_lines(&mut contexts[then_context]);
                let else_lines = take_c_lines(&mut contexts[else_context]);
                contexts[then_context] = Vec::new();
                contexts[else_context] = Vec::new();
                if let Some(name) = name {
                    contexts[context].push(format!("if({condition}){{{then_lines}{name}={then_value};}}else{{{else_lines}{name}={else_value};}}"));
                    values.push(name);
                } else {
                    contexts[context].push(format!(
                        "if({condition}){{{then_lines}}}else{{{else_lines}}}"
                    ));
                    values.push("INT64_C(0)".to_owned());
                }
            }
            Frame::NativeArgs(call, index, mut args, context) => {
                if index < call.args.len() {
                    if index > 0 {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::NativeArgs(call, index + 1, args, context));
                    frames.push(Frame::Enter(&call.args[index], context));
                } else {
                    if !call.args.is_empty() {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = if import.result == ScalarType::Unit {
                        format!("tmp_{}", *temporary_count)
                    } else {
                        next_temporary(temporary_count)
                    };
                    if import.result != ScalarType::Unit {
                        contexts[context]
                            .push(format!("{} {name};", replay_c_scalar(import.result)));
                    }
                    contexts[context].push(format!("status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.c_field, if args.is_empty() { "" } else { ", " }, args.join(", "), if import.result == ScalarType::Unit { String::new() } else { format!(", &{name}") }, import.rust_method));
                    if import.result == ScalarType::Bool {
                        contexts[context]
                            .push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
                    }
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            Frame::CallArgs(callee, args_source, ty, index, mut args, context) => {
                if index < args_source.len() {
                    if index > 0 {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::CallArgs(
                        callee,
                        args_source,
                        ty,
                        index + 1,
                        args,
                        context,
                    ));
                    frames.push(Frame::Enter(&args_source[index], context));
                } else {
                    if !args_source.is_empty() {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        contexts[context].push(format!(
                            "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                            replay_symbol_hash(callee),
                            if args.is_empty() { "" } else { ", " },
                            args.join(",")
                        ));
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = next_temporary(temporary_count);
                        contexts[context].push(format!("{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;", replay_c_scalar(replay_resolved_scalar(ty).ok_or_else(b111)?), replay_symbol_hash(callee), if args.is_empty() { "" } else { ", " }, args.join(",")));
                        values.push(name);
                    }
                }
            }
        }
    }
    if values.len() != 1 {
        return Err(b111());
    }
    move_root_c_lines(lines, &mut contexts);
    values.pop().ok_or_else(b111)
}

fn replay_c_expression_shape(expression: &ResolvedExpr) -> Result<(usize, usize), Diagnostic> {
    let mut pending = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    pending[0] = Some((expression, 0usize, 1usize));
    let mut pending_len = 1usize;
    let mut nodes = 0usize;
    let mut maximum_depth = 1usize;
    while pending_len != 0 {
        let (node, child_index, node_depth) = pending[pending_len - 1].take().ok_or_else(b111)?;
        pending_len -= 1;
        if child_index == 0 {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            maximum_depth = maximum_depth.max(node_depth);
        }
        let mut child_cursor = child_index;
        if let Some((_, child)) = super::resolved_expression_child(node, &mut child_cursor) {
            if pending_len + 2 > pending.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            pending[pending_len] = Some((node, child_cursor, node_depth));
            pending[pending_len + 1] = Some((child, 0, node_depth + 1));
            pending_len += 2;
        }
    }
    Ok((nodes, maximum_depth))
}

fn replay_c_frame_payload(frame: &ReplayCExpressionFrame<'_>) -> usize {
    match frame {
        ReplayCExpressionFrame::FinishBinary(_, value)
        | ReplayCExpressionFrame::FinishLazy(value) => value.capacity(),
        ReplayCExpressionFrame::FinishThen(_, value)
        | ReplayCExpressionFrame::FinishElse(value) => value.as_ref().map_or(0, String::capacity),
        _ => 0,
    }
}

#[allow(clippy::ptr_arg)] // Exact Vec capacities are part of the scratch proof.
fn note_replay_c_expression_scratch(
    current: &ReplayCExpressionFrame<'_>,
    frames: &Vec<ReplayCExpressionFrame<'_>>,
    values: &Vec<String>,
    arguments: &Vec<String>,
    lines: &CExpressionLineArena,
) -> Result<(), Diagnostic> {
    #[cfg(not(test))]
    let _ = lines;
    let mut string_payload = replay_c_frame_payload(current);
    for frame in frames {
        string_payload = string_payload
            .checked_add(replay_c_frame_payload(frame))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for value in values.iter().chain(arguments) {
        string_payload = string_payload
            .checked_add(value.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    if string_payload > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    #[cfg(test)]
    note_post_hir_replay_capacity(
        frames
            .capacity()
            .saturating_mul(REPLAY_C_EXPRESSION_FRAME_BYTES)
            .saturating_add(
                values
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                arguments
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(lines.retained_bytes())
            .saturating_add(string_payload),
    );
    Ok(())
}

fn replay_write_c_arguments(
    lines: &mut CExpressionLineArena,
    arguments: &[String],
    separator: &str,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            lines
                .write_str(separator)
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
        }
        lines
            .write_str(argument)
            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
    }
    Ok(())
}

fn replay_c_expression_linear_independent(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    let (node_count, depth) = replay_c_expression_shape(expression)?;
    let capacity = depth
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(capacity);
    let mut values = Vec::<String>::with_capacity(capacity);
    let mut arguments = Vec::<String>::with_capacity(node_count);
    frames.push(ReplayCExpressionFrame::Evaluate(expression));
    while let Some(frame) = frames.pop() {
        note_replay_c_expression_scratch(&frame, &frames, &values, &arguments, lines)?;
        match frame {
            ReplayCExpressionFrame::Evaluate(expression) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                    values.push(format!("v_{}", replay_symbol_hash(place.root.as_str())))
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(ReplayCExpressionFrame::ContinueNative(
                        call,
                        0,
                        arguments.len(),
                    ));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(ReplayCExpressionFrame::FinishUnary(*op));
                    frames.push(ReplayCExpressionFrame::Evaluate(value));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(ReplayCExpressionFrame::FinishLazyLeft(*op, right));
                    frames.push(ReplayCExpressionFrame::Evaluate(left));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(ReplayCExpressionFrame::FinishBinaryLeft(*op, right));
                    frames.push(ReplayCExpressionFrame::Evaluate(left));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(ReplayCExpressionFrame::ContinueBlock(statements, 0, tail));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = replay_resolved_scalar(&expression.ty).ok_or_else(b111)?;
                    frames.push(ReplayCExpressionFrame::FinishCondition(
                        then_branch,
                        else_branch,
                        ty,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(condition));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(ReplayCExpressionFrame::ContinueCall(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        arguments.len(),
                    ));
                }
                _ => return Err(b107("scalar value signature required")),
            },
            ReplayCExpressionFrame::FinishUnary(op) => {
                let value = values.pop().ok_or_else(b111)?;
                if op == crate::ast::UnaryOp::Not {
                    values.push(format!("(!({value}))"));
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                }
            }
            ReplayCExpressionFrame::FinishBinaryLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(ReplayCExpressionFrame::FinishBinary(op, left));
                frames.push(ReplayCExpressionFrame::Evaluate(right));
            }
            ReplayCExpressionFrame::FinishBinary(op, left) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "int64_t {name};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    match op {
                        crate::ast::BinaryOp::Add => write!(lines, "if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => write!(lines, "if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => write!(lines, "if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    }
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            ReplayCExpressionFrame::FinishLazyLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                write!(
                    lines,
                    "uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);if({}){{",
                    if op == crate::ast::BinaryOp::And {
                        name.clone()
                    } else {
                        format!("!{name}")
                    }
                )
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishLazy(name));
                frames.push(ReplayCExpressionFrame::Evaluate(right));
            }
            ReplayCExpressionFrame::FinishLazy(name) => {
                let right = values.pop().ok_or_else(b111)?;
                write!(lines, " {name}=({right})?UINT8_C(1):UINT8_C(0);}}")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                values.push(name);
            }
            ReplayCExpressionFrame::ContinueBlock(statements, index, tail) => {
                match statements.get(index) {
                    Some(ResolvedStatement::Let { value, .. }) => {
                        frames.push(ReplayCExpressionFrame::FinishBinding(
                            statements, index, tail,
                        ));
                        frames.push(ReplayCExpressionFrame::Evaluate(value));
                    }
                    Some(ResolvedStatement::Assign { value, .. }) => {
                        frames.push(ReplayCExpressionFrame::FinishAssignment(
                            statements, index, tail,
                        ));
                        frames.push(ReplayCExpressionFrame::Evaluate(value));
                    }
                    _ => frames.push(ReplayCExpressionFrame::Evaluate(tail)),
                }
            }
            ReplayCExpressionFrame::FinishBinding(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(ReplayCExpressionFrame::ContinueBlock(
                    statements,
                    index + 1,
                    tail,
                ));
            }
            ReplayCExpressionFrame::FinishAssignment(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "v_{} = {value};",
                        replay_symbol_hash(binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(ReplayCExpressionFrame::ContinueBlock(
                    statements,
                    index + 1,
                    tail,
                ));
            }
            ReplayCExpressionFrame::FinishCondition(then_branch, else_branch, ty) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "{} {name};", replay_c_scalar(ty))
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    Some(name)
                };
                write!(lines, "if({condition}){{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishThen(else_branch, name));
                frames.push(ReplayCExpressionFrame::Evaluate(then_branch));
            }
            ReplayCExpressionFrame::FinishThen(else_branch, name) => {
                let then_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = &name {
                    write!(lines, "{name}={then_value};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                lines
                    .write_str("}else{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishElse(name));
                frames.push(ReplayCExpressionFrame::Evaluate(else_branch));
            }
            ReplayCExpressionFrame::FinishElse(name) => {
                let else_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = name {
                    write!(lines, "{name}={else_value};}}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    lines
                        .write_str("}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push("INT64_C(0)".to_owned());
                }
            }
            ReplayCExpressionFrame::ContinueNative(call, index, start) => {
                if index < call.args.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(ReplayCExpressionFrame::ContinueNative(
                        call,
                        index + 1,
                        start,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(&call.args[index]));
                } else {
                    if !call.args.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = format!("tmp_{}", *temporary_count);
                    if import.result != ScalarType::Unit {
                        *temporary_count += 1;
                        write!(lines, "{} {name};", replay_c_scalar(import.result))
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(
                        lines,
                        "status = ctx->imports->{}(ctx->userdata",
                        import.c_field
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if start < arguments.len() {
                        lines
                            .write_str(", ")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        replay_write_c_arguments(lines, &arguments[start..], ", ")?;
                    }
                    if import.result != ScalarType::Unit {
                        write!(lines, ", &{name}")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(lines, "); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.rust_method)
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if import.result == ScalarType::Bool {
                        write!(lines, "if ({name} > UINT8_C(1)) return spxnr_adapter(4);")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    arguments.truncate(start);
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            ReplayCExpressionFrame::ContinueCall(callee, source, ty, index, start) => {
                if index < source.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(ReplayCExpressionFrame::ContinueCall(
                        callee,
                        source,
                        ty,
                        index + 1,
                        start,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(&source[index]));
                } else {
                    if !source.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        write!(lines, "status=spxnr1_f_{}(ctx", replay_symbol_hash(callee))
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start < arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            replay_write_c_arguments(lines, &arguments[start..], ",")?;
                        }
                        lines
                            .write_str(");if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};status=spxnr1_f_{}(ctx",
                            replay_c_scalar(replay_resolved_scalar(ty).ok_or_else(b111)?),
                            replay_symbol_hash(callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start < arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            replay_write_c_arguments(lines, &arguments[start..], ",")?;
                        }
                        write!(lines, ",&{name});if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    arguments.truncate(start);
                }
            }
        }
    }
    let terminal = ReplayCExpressionFrame::Evaluate(expression);
    note_replay_c_expression_scratch(&terminal, &frames, &values, &arguments, lines)?;
    if values.len() != 1 || !arguments.is_empty() {
        return Err(b111());
    }
    values.pop().ok_or_else(b111)
}

fn replay_c_expression(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    replay_c_expression_linear_independent(expression, imports, temporary_count, lines)
}

// Kept out of every build: the iterative generator above is the sole replay
// evaluator. This source reference makes authored formatting changes easy to
// audit while preventing a recursive production route from reappearing.
#[cfg(any())]
fn replay_c_expression_recursive_reference(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(if *value == i64::MIN {
            "INT64_MIN".to_owned()
        } else {
            format!("INT64_C({value})")
        }),
        ResolvedExprKind::Bool(value) => Ok(if *value {
            "UINT8_C(1)".to_owned()
        } else {
            "UINT8_C(0)".to_owned()
        }),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => {
            Ok(format!("v_{}", replay_symbol_hash(place.root.as_str())))
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let import = imports
                .iter()
                .find(|item| item.id == call.import.as_str())
                .ok_or_else(b111)?;
            let args = call
                .args
                .iter()
                .map(|arg| replay_c_expression(arg, imports, temporary_count, lines))
                .collect::<Result<Vec<_>, _>>()?;
            let name = format!("tmp_{}", *temporary_count);
            if import.result != ScalarType::Unit {
                lines.push(format!("{} {name};", replay_c_scalar(import.result)));
                *temporary_count += 1;
            }
            lines.push(format!(
                "status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}",
                import.c_field,
                if args.is_empty() { "" } else { ", " },
                args.join(", "),
                if import.result == ScalarType::Unit {
                    String::new()
                } else {
                    format!(", &{name}")
                },
                import.rust_method,
            ));
            if import.result == ScalarType::Bool {
                lines.push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
            }
            Ok(if import.result == ScalarType::Unit {
                "INT64_C(0)".to_owned()
            } else {
                name
            })
        }
        ResolvedExprKind::Unary { op, value } => {
            let value = replay_c_expression(value, imports, temporary_count, lines)?;
            match op {
                crate::ast::UnaryOp::Neg => {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    lines.push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                    Ok(name)
                }
                crate::ast::UnaryOp::Not => Ok(format!("(!({value}))")),
            }
        }
        ResolvedExprKind::Binary {
            op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
            left,
            right,
        } => {
            let left = replay_c_expression(left, imports, temporary_count, lines)?;
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
            let mut branch_lines = Vec::new();
            let right = replay_c_expression(right, imports, temporary_count, &mut branch_lines)?;
            let condition = if matches!(
                expression.kind,
                ResolvedExprKind::Binary {
                    op: crate::ast::BinaryOp::And,
                    ..
                }
            ) {
                name.clone()
            } else {
                format!("!{name}")
            };
            lines.push(format!(
                "if({condition}){{{} {name}=({right})?UINT8_C(1):UINT8_C(0);}}",
                branch_lines.join("")
            ));
            Ok(name)
        }
        ResolvedExprKind::Binary { op, left, right } => {
            let left = replay_c_expression(left, imports, temporary_count, lines)?;
            let right = replay_c_expression(right, imports, temporary_count, lines)?;
            if matches!(
                op,
                crate::ast::BinaryOp::Add
                    | crate::ast::BinaryOp::Sub
                    | crate::ast::BinaryOp::Mul
                    | crate::ast::BinaryOp::Div
                    | crate::ast::BinaryOp::Rem
            ) {
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                lines.push(format!("int64_t {name};"));
                lines.push(match op {
                    crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                    crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                    crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                    crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                    crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                    _ => unreachable!(),
                });
                return Ok(name);
            }
            let operator = match op {
                crate::ast::BinaryOp::Add => "+",
                crate::ast::BinaryOp::Sub => "-",
                crate::ast::BinaryOp::Mul => "*",
                crate::ast::BinaryOp::Div => "/",
                crate::ast::BinaryOp::Rem => "%",
                crate::ast::BinaryOp::Eq => "==",
                crate::ast::BinaryOp::Ne => "!=",
                crate::ast::BinaryOp::Lt => "<",
                crate::ast::BinaryOp::Le => "<=",
                crate::ast::BinaryOp::Gt => ">",
                crate::ast::BinaryOp::Ge => ">=",
                crate::ast::BinaryOp::And => "&&",
                crate::ast::BinaryOp::Or => "||",
            };
            Ok(format!("(({left}) {operator} ({right}))"))
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                let value = replay_c_expression(value, imports, temporary_count, lines)?;
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    lines.push(format!(
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
            }
            replay_c_expression(tail, imports, temporary_count, lines)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = replay_c_expression(condition, imports, temporary_count, lines)?;
            if replay_resolved_scalar(&expression.ty) == Some(ScalarType::Unit) {
                let mut then_lines = Vec::new();
                let _ =
                    replay_c_expression(then_branch, imports, temporary_count, &mut then_lines)?;
                let mut else_lines = Vec::new();
                let _ =
                    replay_c_expression(else_branch, imports, temporary_count, &mut else_lines)?;
                lines.push(format!(
                    "if({condition}){{{}}}else{{{}}}",
                    then_lines.join(""),
                    else_lines.join("")
                ));
                return Ok("INT64_C(0)".to_owned());
            }
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!(
                "{} {name};",
                replay_c_scalar(replay_resolved_scalar(&expression.ty).ok_or_else(b111)?)
            ));
            let mut then_lines = Vec::new();
            let then_value =
                replay_c_expression(then_branch, imports, temporary_count, &mut then_lines)?;
            let mut else_lines = Vec::new();
            let else_value =
                replay_c_expression(else_branch, imports, temporary_count, &mut else_lines)?;
            lines.push(format!(
                "if({condition}){{{}{name}={then_value};}}else{{{}{name}={else_value};}}",
                then_lines.join(""),
                else_lines.join("")
            ));
            Ok(name)
        }
        ResolvedExprKind::Call { callee, args, .. } => {
            let args = args
                .iter()
                .map(|arg| replay_c_expression(arg, imports, temporary_count, lines))
                .collect::<Result<Vec<_>, _>>()?;
            if expression.ty == ResolvedType::Unit {
                lines.push(format!(
                    "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                    replay_symbol_hash(callee.as_str()),
                    if args.is_empty() { "" } else { ", " },
                    args.join(",")
                ));
                return Ok("INT64_C(0)".to_owned());
            }
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!(
                "{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;",
                replay_c_scalar(replay_resolved_scalar(&expression.ty).ok_or_else(b111)?),
                replay_symbol_hash(callee.as_str()),
                if args.is_empty() { "" } else { ", " },
                args.join(",")
            ));
            Ok(name)
        }
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Place(_) => Err(b107("scalar value signature required")),
    }
}

fn replay_c_exact(
    source: &str,
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<bool, Diagnostic> {
    let mut replay = ExactReplay::new(source);
    replay.text("#include \"semaprax_native_rust_interop.h\"\n#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n#include <limits.h>\nstatic const uint8_t spxnr_capabilities[32] = {");
    let digest = replay_capabilities_digest(&spec.capabilities);
    let hex = digest.strip_prefix("sha256:").ok_or_else(b111)?;
    if hex.len() != 64 {
        return Err(b111());
    }
    for index in (0..64).step_by(2) {
        if index != 0 {
            replay.text(",");
        }
        replay.text("0x");
        replay.text(&hex[index..index + 2]);
    }
    replay.text("};\nstatic spxnr_status_v1 spxnr_adapter(uint32_t code){return (((uint64_t)65535)<<48)|(((uint64_t)4)<<32)|code;}\nstatic spxnr_status_v1 spxnr_validate(const spxnr_context_v1 *ctx){if(!ctx||((uintptr_t)ctx%_Alignof(spxnr_context_v1))!=0)return spxnr_adapter(1);if(ctx->abi_version!=1||ctx->size!=sizeof(*ctx)||ctx->reserved!=0)return spxnr_adapter(1);if(!ctx->imports||((uintptr_t)ctx->imports%_Alignof(spxnr_imports_v1))!=0)return spxnr_adapter(2);if(ctx->imports->abi_version!=1||ctx->imports->size!=sizeof(*ctx->imports))return spxnr_adapter(2);if(memcmp(ctx->capabilities_digest,spxnr_capabilities,32)!=0)return spxnr_adapter(3);if(ctx->call_depth>=32)return spxnr_adapter(7);return 0;}\n");
    if !imports.is_empty() {
        replay.text("static int spxnr_status_canonical(spxnr_status_v1 status){if(status==0)return 1;uint32_t code=(uint32_t)status;uint8_t class_=(uint8_t)(status>>32);uint8_t retry=(uint8_t)((status>>40)&1);uint8_t reserved=(uint8_t)((status>>41)&0x7f);uint16_t domain=(uint16_t)(status>>48);if(code==0||reserved!=0||domain==0)return 0;if(domain==65533)return retry==0&&((class_==1&&code>=1&&code<=6)||(class_==2&&code>=1&&code<=2));");
    }
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .collect::<BTreeSet<_>>();
    for (index, _) in domains.iter().enumerate() {
        replay.text("if(domain==");
        replay.number(index + 1);
        replay.text(")return class_==3;");
    }
    if !imports.is_empty() {
        replay.text("if(domain==65534)return class_==4&&retry==0&&code>=1&&code<=2;if(domain==65535)return class_==4&&retry==0&&code>=1&&code<=8;return 0;}\n");
    }
    let ordinals = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    for import in imports {
        replay.text("static int spxnr_status_for_");
        replay.text(&import.rust_method);
        replay.text("(spxnr_status_v1 status){if(!spxnr_status_canonical(status))return 0;uint16_t domain=(uint16_t)(status>>48);return domain==65534||domain==65535");
        if let Some(ordinal) = import
            .failure
            .as_deref()
            .and_then(|domain| ordinals.get(domain).copied())
        {
            replay.text("||domain==");
            replay.number(ordinal);
        }
        replay.text(";}\nstatic spxnr_status_v1 spxnr_validate_");
        replay.text(&import.rust_method);
        replay.text("(const spxnr_context_v1 *ctx){return ctx->imports->");
        replay.text(&import.c_field);
        replay.text("?0:spxnr_adapter(2);}\n");
    }
    for function in closure {
        let parameters = replay_parameter_facts(function)?;
        let result = replay_resolved_scalar(&function.return_type).ok_or_else(b111)?;
        replay.text("static spxnr_status_v1 spxnr1_f_");
        replay.text(&replay_symbol_hash(function.id.as_str()));
        replay.text("(const spxnr_context_v1 *ctx");
        if !parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&parameters));
        }
        if result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(result));
            replay.text(" *result_out");
        }
        replay.text(");\n");
    }
    for function in closure {
        let parameters = replay_parameter_facts(function)?;
        let result = replay_resolved_scalar(&function.return_type).ok_or_else(b111)?;
        replay.text("static spxnr_status_v1 spxnr1_f_");
        replay.text(&replay_symbol_hash(function.id.as_str()));
        replay.text("(const spxnr_context_v1 *ctx");
        if !parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&parameters));
        }
        if result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(result));
            replay.text(" *result_out");
        }
        replay.text(" ){spxnr_status_v1 status=0;(void)ctx;");
        for index in 0..parameters.len() {
            replay.text("(void)arg_");
            replay.number(index);
            replay.text(";");
        }
        for (index, (parameter, resolved)) in parameters.iter().zip(&function.params).enumerate() {
            replay.text(replay_c_scalar(parameter.ty));
            replay.text(" v_");
            replay.text(&replay_symbol_hash(resolved.id.as_str()));
            replay.text("=arg_");
            replay.number(index);
            replay.text(";");
        }
        let mut temporary_count = 0;
        let mut lines = CExpressionLineArena::new();
        for requirement in &function.requires {
            lines.clear();
            let value =
                replay_c_expression(requirement, imports, &mut temporary_count, &mut lines)?;
            replay.text(lines.as_str()?);
            replay.text("if(!(");
            replay.text(&value);
            replay.text("))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(1);");
        }
        lines.clear();
        let value = replay_c_expression(&function.body, imports, &mut temporary_count, &mut lines)?;
        replay.text(lines.as_str()?);
        if result != ScalarType::Unit {
            replay.text(replay_c_scalar(result));
            replay.text(" v_");
            replay.text(&replay_symbol_hash(function.result_id.as_str()));
            replay.text("=");
            replay.text(&value);
            replay.text(";");
        }
        for guarantee in &function.ensures {
            lines.clear();
            let value = replay_c_expression(guarantee, imports, &mut temporary_count, &mut lines)?;
            replay.text(lines.as_str()?);
            replay.text("if(!(");
            replay.text(&value);
            replay.text("))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(2);");
        }
        if result != ScalarType::Unit {
            replay.text("*result_out=v_");
            replay.text(&replay_symbol_hash(function.result_id.as_str()));
            replay.text(";");
        }
        replay.text("return status;}\n");
    }
    for export in exports {
        replay.text("spxnr_status_v1 ");
        replay.text(&export.c_symbol);
        replay.text("(const spxnr_context_v1 *ctx");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&export.parameters));
        }
        if export.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(export.result));
            replay.text(" *result_out");
        }
        replay.text(" ){spxnr_status_v1 status=spxnr_validate(ctx);if(status!=0)return status;");
        for import in imports {
            replay.text("status=spxnr_validate_");
            replay.text(&import.rust_method);
            replay.text("(ctx);if(status!=0)return status;");
        }
        if export.result != ScalarType::Unit {
            replay.text("if(!result_out||((uintptr_t)result_out%_Alignof(");
            replay.text(replay_c_scalar(export.result));
            replay.text("))!=0)return spxnr_adapter(5);");
        }
        for (index, parameter) in export.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                replay.text("if(arg_");
                replay.number(index);
                replay.text(">1)return spxnr_adapter(4);");
            }
        }
        replay.text(
            "spxnr_context_v1 local=*ctx;local.call_depth=ctx->call_depth+1;status=spxnr1_f_",
        );
        replay.text(&replay_symbol_hash(&export.id));
        replay.text("(&local");
        for index in 0..export.parameters.len() {
            replay.text(if index == 0 { ", " } else { "," });
            replay.text("arg_");
            replay.number(index);
        }
        if export.result != ScalarType::Unit {
            replay.text(", result_out");
        }
        replay.text(");return status;}\n");
    }
    Ok(replay.finish())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_generated_exact(
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
    header: &str,
    c: &str,
    rust: &str,
    ffi: &str,
) -> Result<(), Diagnostic> {
    if !replay_header_exact(header, exports, imports) {
        return Err(b111());
    }
    if !replay_safe_rust_exact(rust, spec, exports, imports)
        || !replay_private_ffi_exact(ffi, spec, exports, imports)
        || !replay_c_exact(c, spec, closure, exports, imports)?
    {
        return Err(b111());
    }
    replay_generated(header, c, rust, ffi)
}
