//! Project-v11 private native carrier lowering.

use std::fmt::Write as _;

use crate::aggregate_layout::{AggregateFieldValueKind, AggregateLayout, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::project::{
    NestedOwnedRecordExport, NestedOwnedRecordLeafType, PublicApiParameterType, PublicApiSubject,
};

use super::{
    provider_call_symbol, provider_error, NativeOwnedDataProviderArtifact, STATUS_CAPACITY,
};
use crate::codegen::native_emit::{c_field_symbol, c_function_symbol, c_record_symbol};

pub fn emit_project_v11_native_nested_owned_record_provider(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
) -> Result<NativeOwnedDataProviderArtifact, Diagnostic> {
    crate::hir::validate(program)?;
    let descriptor = crate::project::replay_nested_owned_record_api_descriptor(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )?;
    for record in descriptor.records() {
        let layout =
            AggregateLayout::for_record(program, AggregateTarget::Native64, record.stable_id())?;
        layout.validate(program)?;
        if layout.fields.len() != record.fields().len()
            || layout
                .fields
                .iter()
                .zip(record.fields())
                .any(|(physical, described)| physical.field != *described.stable_id())
        {
            return Err(provider_error(
                "nested record native layout disagrees with descriptor",
            ));
        }
    }
    let mut source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    writeln!(
        source,
        "#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{descriptor_digest}\""
    )
    .unwrap();
    source.push_str(&super::super::emit_hir_c_for_owned_data_provider(program)?);
    source.push('\n');
    let mut runtime = String::new();
    super::emit_provider_runtime(&mut runtime);
    runtime = runtime.replace(
        "static bool spx_owned_data_overlap_v1(",
        "static __attribute__((unused)) bool spx_owned_data_overlap_v1(",
    );
    runtime = runtime.replace(
        "static spx_owned_data_status_v1 spx_owned_data_attach_v1(",
        "static __attribute__((unused)) spx_owned_data_status_v1 spx_owned_data_attach_v1(",
    );
    source.push_str(&runtime);
    source.push_str(BATCH_ATTACH_RUNTIME);
    for export in descriptor.exports() {
        emit_export(&mut source, program, export)?;
    }
    Ok(NativeOwnedDataProviderArtifact {
        source,
        descriptor: descriptor_bytes.to_vec(),
        descriptor_digest: descriptor_digest.to_owned(),
    })
}

fn emit_export(
    out: &mut String,
    program: &ResolvedProgram,
    export: &NestedOwnedRecordExport,
) -> Result<(), Diagnostic> {
    let function = program
        .functions
        .iter()
        .find(|function| function.id == *export.stable_id())
        .ok_or_else(|| provider_error("nested record provider export is absent"))?;
    let symbol = provider_call_symbol(export.rust_method_name());
    write!(
        out,
        "SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 {symbol}(spx_context_v1 *context"
    )
    .unwrap();
    for (index, (_, _, parameter)) in export.parameters().iter().enumerate() {
        match parameter {
            PublicApiParameterType::I64 => write!(out, ", int64_t arg_{index}"),
            PublicApiParameterType::Bool => write!(out, ", uint8_t arg_{index}"),
            PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => write!(
                out,
                ", const uint8_t *arg_{index}, uint64_t arg_{index}_len"
            ),
        }
        .unwrap();
    }
    writeln!(out, ", uint64_t *carrier_out) {{").unwrap();
    writeln!(out,"    if (context == NULL || carrier_out == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER || spx_owned_data_overlap_v1(context, sizeof(*context), carrier_out, sizeof(*carrier_out) * UINT64_C({}))) return SPX_OWNED_DATA_ADAPTER_FAILURE;",export.leaves().len()).unwrap();
    writeln!(out,"    for (uintptr_t index = (uintptr_t)0; index < sizeof(*carrier_out) * UINT64_C({}); ++index) if (((const uint8_t *)carrier_out)[index] != UINT8_MAX) return SPX_OWNED_DATA_ADAPTER_FAILURE;",export.leaves().len()).unwrap();
    out.push_str("    uint64_t borrowed = UINT64_C(0);\n");
    for (index, (_, _, parameter)) in export.parameters().iter().enumerate() {
        match parameter{PublicApiParameterType::Bool=>writeln!(out,"    if (arg_{index} > UINT8_C(1)) return SPX_OWNED_DATA_ADAPTER_FAILURE;"),PublicApiParameterType::BorrowStr=>writeln!(out,"    if (arg_{index}_len > UINT64_C(65536) - borrowed || (arg_{index}_len != UINT64_C(0) && arg_{index} == NULL) || !spx_owned_data_utf8_v1(arg_{index}, arg_{index}_len)) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n    borrowed += arg_{index}_len;\n    spx_str_v1 value_{index} = {{ .data = arg_{index}, .len = arg_{index}_len }};"),PublicApiParameterType::BorrowSliceU8=>writeln!(out,"    if (arg_{index}_len > UINT64_C(65536) - borrowed || (arg_{index}_len != UINT64_C(0) && arg_{index} == NULL)) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n    borrowed += arg_{index}_len;\n    spx_slice_u8_v1 value_{index} = {{ .ptr = arg_{index}_len == UINT64_C(0) ? NULL : arg_{index}, .len = arg_{index}_len }};"),PublicApiParameterType::I64=>Ok(())}.unwrap()
    }
    writeln!(
        out,
        "    struct spx_status_entry statuses[UINT32_C({STATUS_CAPACITY})];"
    )
    .unwrap();
    out.push_str("    struct spx_context semantic = {0};\n    if (context->invocation == UINT64_MAX) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n    ++context->invocation;\n");
    writeln!(out,"    if (!spx_context_init(&semantic, context->invocation, statuses, UINT32_C({STATUS_CAPACITY}), NULL, NULL, NULL)) return SPX_OWNED_DATA_ADAPTER_FAILURE;").unwrap();
    writeln!(
        out,
        "    struct {} result = {{0}};",
        c_record_symbol(&function.return_type)
    )
    .unwrap();
    let arguments = export
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, (_, _, parameter))| match parameter {
            PublicApiParameterType::I64 => format!("arg_{index}"),
            PublicApiParameterType::Bool => format!("arg_{index} != UINT8_C(0)"),
            PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => {
                format!("value_{index}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let comma = if arguments.is_empty() { "" } else { ", " };
    writeln!(out,"    if ({}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;",c_function_symbol(export.stable_id())).unwrap();
    writeln!(
        out,
        "    uint64_t carrier[{}] = {{0}};",
        export.leaves().len()
    )
    .unwrap();
    let mut owned = Vec::new();
    for leaf in export.leaves() {
        let access = leaf_access(
            program,
            export.result_record_id(),
            leaf.field_path(),
            leaf.ty(),
        )?;
        match leaf.ty() {
            NestedOwnedRecordLeafType::I64 => writeln!(
                out,
                "    memcpy(&carrier[{}], &{access}, sizeof({access}));",
                leaf.ordinal()
            )
            .unwrap(),
            NestedOwnedRecordLeafType::Bool => writeln!(
                out,
                "    carrier[{}] = {access} ? UINT64_C(1) : UINT64_C(0);",
                leaf.ordinal()
            )
            .unwrap(),
            NestedOwnedRecordLeafType::Usize => {
                writeln!(out, "    carrier[{}] = {access};", leaf.ordinal()).unwrap()
            }
            NestedOwnedRecordLeafType::OwnedBytes => owned.push((leaf.ordinal(), access)),
        }
    }
    writeln!(out, "    spx_bytes_v1 *owned[{}];", owned.len()).unwrap();
    writeln!(
        out,
        "    spx_owned_bytes_handle_v1 handles[{}] = {{0}};",
        owned.len()
    )
    .unwrap();
    for (index, (_, access)) in owned.iter().enumerate() {
        writeln!(out, "    owned[{index}] = &{access};").unwrap()
    }
    writeln!(out,"    spx_owned_data_status_v1 attached = spx_owned_data_attach_batch_v1(context, owned, handles, UINT32_C({}));",owned.len()).unwrap();
    out.push_str("    if (attached != SPX_OWNED_DATA_SUCCESS) {");
    for (_, access) in &owned {
        write!(out, " spx_bytes_drop(&{access});").unwrap()
    }
    out.push_str(" return attached; }\n");
    for (index, (ordinal, _)) in owned.iter().enumerate() {
        writeln!(out, "    carrier[{ordinal}] = handles[{index}];").unwrap()
    }
    out.push_str("    memcpy(carrier_out, carrier, sizeof(carrier));\n    return SPX_OWNED_DATA_SUCCESS;\n}\n");
    Ok(())
}

fn leaf_access(
    program: &ResolvedProgram,
    root: &DeclarationId,
    path: &[DeclarationId],
    expected: NestedOwnedRecordLeafType,
) -> Result<String, Diagnostic> {
    let mut record = root.clone();
    let mut access = String::from("result");
    for (index, id) in path.iter().enumerate() {
        let layout = AggregateLayout::for_record(program, AggregateTarget::Native64, &record)?;
        layout.validate(program)?;
        let field = layout.field(id).ok_or_else(|| {
            provider_error("nested record leaf path is absent from native layout")
        })?;
        access.push('.');
        access.push_str(&c_field_symbol(id));
        let terminal = index + 1 == path.len();
        match (&field.value_kind, terminal, expected, &field.ty) {
            (AggregateFieldValueKind::Aggregate, false, _, _) => {
                let crate::hir::ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &field.ty
                else {
                    return Err(provider_error("nested record layout child is not nominal"));
                };
                if !arguments.is_empty() {
                    return Err(provider_error("nested record layout child is generic"));
                }
                record = declaration.clone()
            }
            (
                AggregateFieldValueKind::OwnedBytes,
                true,
                NestedOwnedRecordLeafType::OwnedBytes,
                crate::hir::ResolvedType::Bytes,
            )
            | (
                AggregateFieldValueKind::Copy,
                true,
                NestedOwnedRecordLeafType::I64,
                crate::hir::ResolvedType::I64,
            )
            | (
                AggregateFieldValueKind::Copy,
                true,
                NestedOwnedRecordLeafType::Bool,
                crate::hir::ResolvedType::Bool,
            )
            | (
                AggregateFieldValueKind::Copy,
                true,
                NestedOwnedRecordLeafType::Usize,
                crate::hir::ResolvedType::Usize,
            ) => {}
            _ => {
                return Err(provider_error(
                    "nested record leaf path type disagrees with native layout",
                ))
            }
        }
    }
    Ok(access)
}

const BATCH_ATTACH_RUNTIME: &str = r#"
static spx_owned_data_status_v1 spx_owned_data_attach_batch_v1(spx_context_v1 *context, spx_bytes_v1 **bytes, spx_owned_bytes_handle_v1 *handles, uint32_t count) {
    if (context == NULL || bytes == NULL || handles == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER || context->live_slots > UINT32_C(4096) || context->next_slot > UINT32_C(4096) || count == UINT32_C(0) || count > UINT32_C(256) || count > UINT32_C(4096) - context->live_slots) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    uint64_t total = UINT64_C(0);
    for (uint32_t outer = UINT32_C(0); outer < count; ++outer) { if (bytes[outer] == NULL || handles[outer] != UINT64_C(0) || bytes[outer]->len > UINT64_C(65536) - total) return SPX_OWNED_DATA_ADAPTER_FAILURE; for (uint32_t inner = UINT32_C(0); inner < outer; ++inner) if (bytes[inner] == bytes[outer]) return SPX_OWNED_DATA_ADAPTER_FAILURE; spx_bytes_require_valid(*bytes[outer]); total += bytes[outer]->len; }
    uint32_t indices[UINT32_C(256)]; uint64_t serials[UINT32_C(256)]; uint32_t found = UINT32_C(0);
    for (uint32_t index = UINT32_C(0); index < UINT32_C(4096) && found < count; ++index) if (index >= context->next_slot || !context->slots[index].live) indices[found++] = index;
    if (found != count) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    uint64_t first_serial = atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed);
    if (first_serial == UINT64_C(0) || first_serial > SPX_OWNED_DATA_MAX_SERIAL_V1 || (uint64_t)count - UINT64_C(1) > SPX_OWNED_DATA_MAX_SERIAL_V1 - first_serial) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    uint64_t after_serials = first_serial + (uint64_t)count;
    if (!atomic_compare_exchange_strong_explicit(&spx_owned_data_next_serial_v1, &first_serial, after_serials, memory_order_relaxed, memory_order_relaxed)) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    for (uint32_t index = UINT32_C(0); index < count; ++index) serials[index] = first_serial + (uint64_t)index;
    for (uint32_t ordinal = UINT32_C(0); ordinal < count; ++ordinal) { uint32_t index = indices[ordinal]; struct spx_owned_data_slot_v1 *slot = &context->slots[index]; slot->issuance_serial = serials[ordinal]; slot->bytes = spx_bytes_move(bytes[ordinal]); slot->live = true; handles[ordinal] = (serials[ordinal] << UINT32_C(13)) | (uint64_t)(index + UINT32_C(1)); if (index >= context->next_slot) context->next_slot = index + UINT32_C(1); }
    context->live_slots += count; return SPX_OWNED_DATA_SUCCESS;
}
"#;
