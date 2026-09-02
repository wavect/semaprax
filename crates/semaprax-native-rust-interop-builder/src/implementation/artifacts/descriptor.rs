//! Deterministic descriptor rendering for the source and project subjects.

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

pub(in crate::implementation) fn render_descriptor_with_limit(
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

pub(in crate::implementation) fn render_descriptor_for_subject(
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
