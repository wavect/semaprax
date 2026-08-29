//! Compiler-owned native provider for Public Owned Data API v1.
//!
//! The provider is derived only from validated HIR and an independently
//! replayed canonical public descriptor. Compiler-owned aggregate layouts are
//! consumed here, before the private opaque-handle ABI is rendered.

use std::fmt::Write as _;

use crate::aggregate_layout::{AggregateFieldValueKind, AggregateLayout, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram, ResolvedType};
use crate::project::{
    replay_public_api_descriptor, PublicApiParameterType, PublicApiResultType, PublicApiSubject,
};
use crate::variant_layout::{VariantLayout, VariantTarget};

use super::native_emit::c_record_symbol;
use super::native_emit::{c_case_symbol, c_field_symbol, c_function_symbol, c_variant_symbol};

const STATUS_CAPACITY: usize = 64;
const HANDLE_CAPACITY: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOwnedDataProviderArtifact {
    source: String,
    descriptor: Vec<u8>,
    descriptor_digest: String,
}

impl NativeOwnedDataProviderArtifact {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
}

pub fn emit_native_owned_data_provider(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
) -> Result<NativeOwnedDataProviderArtifact, Diagnostic> {
    emit_provider(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
        false,
    )
}

pub fn emit_project_v8_native_owned_data_provider(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
) -> Result<NativeOwnedDataProviderArtifact, Diagnostic> {
    emit_provider(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
        true,
    )
}

pub fn emit_project_v9_native_flat_owned_record_provider(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
) -> Result<NativeOwnedDataProviderArtifact, Diagnostic> {
    crate::hir::validate(program)?;
    let descriptor = crate::project::replay_flat_owned_record_api_descriptor(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )?;
    let mut source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    writeln!(
        source,
        "#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{descriptor_digest}\""
    )
    .unwrap();
    source.push_str(&super::emit_hir_c_for_owned_data_provider(program)?);
    source.push('\n');
    let mut runtime = String::new();
    emit_provider_runtime(&mut runtime);
    runtime = runtime.replace(
        "static bool spx_owned_data_overlap_v1(",
        "static __attribute__((unused)) bool spx_owned_data_overlap_v1(",
    );
    source.push_str(&runtime);
    for export in descriptor.exports() {
        emit_flat_export(&mut source, program, export)?;
    }
    Ok(NativeOwnedDataProviderArtifact {
        source,
        descriptor: descriptor_bytes.to_vec(),
        descriptor_digest: descriptor_digest.to_owned(),
    })
}

fn emit_flat_export(
    output: &mut String,
    program: &ResolvedProgram,
    export: &crate::project::FlatOwnedRecordExport,
) -> Result<(), Diagnostic> {
    let function = program
        .functions
        .iter()
        .find(|function| function.id == *export.stable_id())
        .ok_or_else(|| provider_error("flat record provider export is absent"))?;
    let layout =
        AggregateLayout::for_type(program, AggregateTarget::Native64, &function.return_type)?;
    layout.validate(program)?;
    if layout.record != *export.record_id() || layout.fields.len() != export.fields().len() {
        return Err(provider_error(
            "flat record native layout disagrees with descriptor",
        ));
    }
    let symbol = provider_call_symbol(export.rust_method_name());
    write!(
        output,
        "SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 {symbol}(spx_context_v1 *context"
    )
    .unwrap();
    for (index, (_, _, parameter)) in export.parameters().iter().enumerate() {
        match parameter {
            PublicApiParameterType::I64 => write!(output, ", int64_t arg_{index}"),
            PublicApiParameterType::Bool => write!(output, ", uint8_t arg_{index}"),
            PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => write!(
                output,
                ", const uint8_t *arg_{index}, uint64_t arg_{index}_len"
            ),
        }
        .unwrap();
    }
    writeln!(output, ", uint64_t *carrier_out) {{").unwrap();
    writeln!(output, "    if (context == NULL || carrier_out == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER || spx_owned_data_overlap_v1(context, sizeof(*context), carrier_out, sizeof(*carrier_out) * UINT64_C({}))) return SPX_OWNED_DATA_ADAPTER_FAILURE;", export.fields().len()).unwrap();
    writeln!(output, "    for (uintptr_t index = (uintptr_t)0; index < sizeof(*carrier_out) * UINT64_C({}); ++index) if (((const uint8_t *)carrier_out)[index] != UINT8_MAX) return SPX_OWNED_DATA_ADAPTER_FAILURE;", export.fields().len()).unwrap();
    output.push_str("    uint64_t borrowed = UINT64_C(0);\n");
    for (index, (_, _, parameter)) in export.parameters().iter().enumerate() {
        match parameter {
            PublicApiParameterType::Bool => writeln!(output, "    if (arg_{index} > UINT8_C(1)) return SPX_OWNED_DATA_ADAPTER_FAILURE;"),
            PublicApiParameterType::BorrowStr => writeln!(output, "    if (arg_{index}_len > UINT64_C(65536) - borrowed || (arg_{index}_len != UINT64_C(0) && arg_{index} == NULL) || !spx_owned_data_utf8_v1(arg_{index}, arg_{index}_len)) return SPX_OWNED_DATA_ADAPTER_FAILURE; borrowed += arg_{index}_len; spx_str_v1 value_{index} = {{ .data = arg_{index}, .len = arg_{index}_len }};"),
            PublicApiParameterType::BorrowSliceU8 => writeln!(output, "    if (arg_{index}_len > UINT64_C(65536) - borrowed || (arg_{index}_len != UINT64_C(0) && arg_{index} == NULL)) return SPX_OWNED_DATA_ADAPTER_FAILURE; borrowed += arg_{index}_len; spx_slice_u8_v1 value_{index} = {{ .ptr = arg_{index}_len == UINT64_C(0) ? NULL : arg_{index}, .len = arg_{index}_len }};"),
            PublicApiParameterType::I64 => Ok(()),
        }.unwrap();
    }
    output.push_str("    (void)borrowed;\n");
    writeln!(
        output,
        "    struct spx_status_entry statuses[UINT32_C({STATUS_CAPACITY})];"
    )
    .unwrap();
    output.push_str("    struct spx_context semantic = {0};\n    if (context->invocation == UINT64_MAX) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n    ++context->invocation;\n");
    writeln!(output, "    if (!spx_context_init(&semantic, context->invocation, statuses, UINT32_C({STATUS_CAPACITY}), NULL, NULL, NULL)) return SPX_OWNED_DATA_ADAPTER_FAILURE;").unwrap();
    let record = c_record_symbol(&function.return_type);
    writeln!(output, "    struct {record} result = {{0}};").unwrap();
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
    writeln!(output, "    if ({}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;", c_function_symbol(export.stable_id())).unwrap();
    writeln!(
        output,
        "    uint64_t carrier[{}] = {{0}};",
        export.fields().len()
    )
    .unwrap();
    let mut owned_field = None;
    for (field, physical) in export.fields().iter().zip(&layout.fields) {
        if field.stable_id() != &physical.field {
            return Err(provider_error("flat record native field order disagrees"));
        }
        let member = c_field_symbol(field.stable_id());
        match field.ty() {
            crate::project::FlatOwnedRecordFieldType::I64 => writeln!(
                output,
                "    memcpy(&carrier[{}], &result.{member}, sizeof(result.{member}));",
                field.ordinal()
            )
            .unwrap(),
            crate::project::FlatOwnedRecordFieldType::Bool => writeln!(
                output,
                "    carrier[{}] = result.{member} ? UINT64_C(1) : UINT64_C(0);",
                field.ordinal()
            )
            .unwrap(),
            crate::project::FlatOwnedRecordFieldType::Usize => writeln!(
                output,
                "    carrier[{}] = result.{member};",
                field.ordinal()
            )
            .unwrap(),
            crate::project::FlatOwnedRecordFieldType::OwnedBytes => {
                if physical.value_kind != AggregateFieldValueKind::OwnedBytes
                    || owned_field.replace((field.ordinal(), member)).is_some()
                {
                    return Err(provider_error(
                        "flat record native owned field is not exact",
                    ));
                }
            }
        }
    }
    let (owned_ordinal, owned_member) =
        owned_field.ok_or_else(|| provider_error("flat record native owned field is absent"))?;
    output.push_str("    spx_owned_bytes_handle_v1 published = UINT64_C(0);\n");
    writeln!(output, "    spx_owned_data_status_v1 attached = spx_owned_data_attach_v1(context, &result.{owned_member}, &published);").unwrap();
    writeln!(output, "    if (attached != SPX_OWNED_DATA_SUCCESS) {{ spx_bytes_drop(&result.{owned_member}); return attached; }}").unwrap();
    writeln!(output, "    carrier[{owned_ordinal}] = published;\n    memcpy(carrier_out, carrier, sizeof(carrier));\n    return SPX_OWNED_DATA_SUCCESS;\n}}").unwrap();
    Ok(())
}

fn emit_provider(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
    project_v8: bool,
) -> Result<NativeOwnedDataProviderArtifact, Diagnostic> {
    crate::hir::validate(program)?;
    let descriptor = replay_public_api_descriptor(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )?;
    if !project_v8
        && descriptor.exports().iter().any(|export| {
            !matches!(
                export.result(),
                PublicApiResultType::OwnedBytes
                    | PublicApiResultType::OptionOwnedBytes
                    | PublicApiResultType::ResultOwnedBytesI64
            )
        })
    {
        return Err(provider_error(
            "native owned-data provider requires only owned Bytes result exports",
        ));
    }
    let mut source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    if project_v8 {
        writeln!(
            source,
            "#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{descriptor_digest}\""
        )
        .expect("writing provider descriptor binding cannot fail");
    }
    source.push_str(&super::emit_hir_c_for_owned_data_provider(program)?);
    source.push('\n');
    let mut runtime = String::new();
    emit_provider_runtime(&mut runtime);
    if project_v8 {
        runtime = runtime.replace(
            "static bool spx_owned_data_overlap_v1(",
            "static __attribute__((unused)) bool spx_owned_data_overlap_v1(",
        );
    }
    source.push_str(&runtime);
    for export in descriptor.exports() {
        let function = program
            .functions
            .iter()
            .find(|function| function.id == *export.stable_id())
            .ok_or_else(|| provider_error("replayed native owned-data export is absent"))?;
        emit_export(
            &mut source,
            program,
            export,
            &function.return_type,
            project_v8,
        )?;
    }
    Ok(NativeOwnedDataProviderArtifact {
        source,
        descriptor: descriptor_bytes.to_vec(),
        descriptor_digest: descriptor_digest.to_owned(),
    })
}

fn emit_provider_runtime(output: &mut String) {
    writeln!(
        output,
        r#"
#if defined(_WIN32)
#define SPX_OWNED_DATA_EXPORT __declspec(dllexport)
#else
#define SPX_OWNED_DATA_EXPORT __attribute__((visibility("default")))
#endif
typedef uint32_t spx_owned_data_status_v1;
typedef uint64_t spx_owned_bytes_handle_v1;
enum {{
    SPX_OWNED_DATA_SUCCESS = UINT32_C(0),
    SPX_OWNED_DATA_SEMANTIC_FAILURE = UINT32_C(1),
    SPX_OWNED_DATA_ADAPTER_FAILURE = UINT32_C(2),
    SPX_OWNED_DATA_INVALID_HANDLE = UINT32_C(3),
    SPX_OWNED_DATA_COPY_FAILURE = UINT32_C(4),
    SPX_OWNED_DATA_SETTLEMENT_FAILURE = UINT32_C(5)
}};
static __attribute__((unused)) bool spx_owned_data_utf8_v1(const uint8_t *bytes, uint64_t length) {{
    uint64_t offset = UINT64_C(0);
    while (offset < length) {{
        uint8_t first = bytes[offset]; uint64_t width;
        if (first <= UINT8_C(0x7f)) width = UINT64_C(1);
        else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) width = UINT64_C(2);
        else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) width = UINT64_C(3);
        else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) width = UINT64_C(4);
        else return false;
        if (width > length - offset) return false;
        if (width >= UINT64_C(2)) {{
            uint8_t second = bytes[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80)
                || (first == UINT8_C(0xe0) && second < UINT8_C(0xa0))
                || (first == UINT8_C(0xed) && second > UINT8_C(0x9f))
                || (first == UINT8_C(0xf0) && second < UINT8_C(0x90))
                || (first == UINT8_C(0xf4) && second > UINT8_C(0x8f))) return false;
        }}
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail)
            if ((bytes[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80)) return false;
        offset += width;
    }}
    return true;
}}
static bool spx_owned_data_overlap_v1(const void *left, uintptr_t left_size, const void *right, uintptr_t right_size) {{
    uintptr_t a = (uintptr_t)left; uintptr_t b = (uintptr_t)right;
    return a <= b ? b - a < left_size : a - b < right_size;
}}
struct spx_owned_data_slot_v1 {{ spx_bytes_v1 bytes; uint64_t generation; bool live; }};
typedef struct spx_owned_data_context_v1 {{
    uint64_t marker;
    uint64_t invocation;
    uint32_t next_slot;
    uint32_t live_slots;
#if defined(SPX_OWNED_DATA_TESTING)
    uint32_t fault;
#endif
    struct spx_owned_data_slot_v1 slots[{HANDLE_CAPACITY}];
}} spx_context_v1;
static const uint64_t SPX_OWNED_DATA_CONTEXT_MARKER = UINT64_C(0x5350584f44433131);
static struct spx_owned_data_slot_v1 *spx_owned_data_find_v1(
    spx_context_v1 *context, spx_owned_bytes_handle_v1 handle
) {{
    if (context == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER || handle == UINT64_C(0)) return NULL;
    uint32_t encoded = (uint32_t)(handle & UINT64_C(0xfff));
    uint64_t generation = handle >> UINT32_C(12);
    if (encoded == UINT32_C(0) || generation == UINT64_C(0)) return NULL;
    uint32_t wanted = encoded - UINT32_C(1);
    for (uint32_t index = UINT32_C(0); index < context->next_slot; ++index) {{
        struct spx_owned_data_slot_v1 *slot = &context->slots[index];
        if (index == wanted) return slot->live && slot->generation == generation ? slot : NULL;
    }}
    return NULL;
}}
static spx_owned_data_status_v1 spx_owned_data_attach_v1(
    spx_context_v1 *context, spx_bytes_v1 *bytes, spx_owned_bytes_handle_v1 *handle_out
) {{
    if (context == NULL || bytes == NULL || handle_out == NULL || *handle_out != UINT64_C(0)
        || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER
        || bytes->len > UINT64_C(65536)) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    spx_bytes_require_valid(*bytes);
    uint32_t index = UINT32_C(0);
    while (index < context->next_slot && context->slots[index].live) ++index;
    if (index == UINT32_C({HANDLE_CAPACITY})) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    if (index == context->next_slot) ++context->next_slot;
    struct spx_owned_data_slot_v1 *slot = &context->slots[index];
    if (slot->generation == (UINT64_MAX >> UINT32_C(12))) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    ++slot->generation;
    slot->bytes = spx_bytes_move(bytes);
    slot->live = true;
    ++context->live_slots;
    *handle_out = (slot->generation << UINT32_C(12)) | (uint64_t)(index + UINT32_C(1));
    return SPX_OWNED_DATA_SUCCESS;
}}
SPX_OWNED_DATA_EXPORT uint64_t spx_owned_data_context_size_v1(void) {{ return (uint64_t)sizeof(spx_context_v1); }}
SPX_OWNED_DATA_EXPORT uint64_t spx_owned_data_context_align_v1(void) {{ return (uint64_t)_Alignof(spx_context_v1); }}
SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 spx_owned_data_context_init_v1(void *storage, uint64_t length) {{
    if (storage == NULL || length != (uint64_t)sizeof(spx_context_v1)
        || ((uintptr_t)storage % (uintptr_t)_Alignof(spx_context_v1)) != (uintptr_t)0) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    spx_context_v1 *context = (spx_context_v1 *)storage;
    memset(context, 0, sizeof(*context));
    context->marker = SPX_OWNED_DATA_CONTEXT_MARKER;
    return SPX_OWNED_DATA_SUCCESS;
}}
SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 spx_owned_data_context_drop_v1(spx_context_v1 *context) {{
    if (context == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER || context->live_slots != UINT32_C(0)) return SPX_OWNED_DATA_SETTLEMENT_FAILURE;
    context->marker = UINT64_C(0);
    return SPX_OWNED_DATA_SUCCESS;
}}
SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 spx_owned_bytes_len_v1(
    spx_context_v1 *context, spx_owned_bytes_handle_v1 handle, uint64_t *length_out
) {{
    if (length_out == NULL) return SPX_OWNED_DATA_ADAPTER_FAILURE;
    struct spx_owned_data_slot_v1 *slot = spx_owned_data_find_v1(context, handle);
    if (slot == NULL) return SPX_OWNED_DATA_INVALID_HANDLE;
    *length_out = slot->bytes.len;
    return SPX_OWNED_DATA_SUCCESS;
}}
SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 spx_owned_bytes_copy_v1(
    spx_context_v1 *context, spx_owned_bytes_handle_v1 handle,
    uint8_t *destination, uint64_t destination_length
) {{
    struct spx_owned_data_slot_v1 *slot = spx_owned_data_find_v1(context, handle);
    if (slot == NULL) return SPX_OWNED_DATA_INVALID_HANDLE;
#if defined(SPX_OWNED_DATA_TESTING)
    if (context->fault == UINT32_C(1)) {{ context->fault = UINT32_C(0); return SPX_OWNED_DATA_COPY_FAILURE; }}
#endif
    if (destination_length != slot->bytes.len
        || (destination_length != UINT64_C(0) && destination == NULL)) return SPX_OWNED_DATA_COPY_FAILURE;
    if (destination_length != UINT64_C(0)) memcpy(destination, slot->bytes.ptr, (size_t)destination_length);
    return SPX_OWNED_DATA_SUCCESS;
}}
SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 spx_owned_bytes_drop_v1(
    spx_context_v1 *context, spx_owned_bytes_handle_v1 handle
) {{
    struct spx_owned_data_slot_v1 *slot = spx_owned_data_find_v1(context, handle);
    if (slot == NULL) return SPX_OWNED_DATA_INVALID_HANDLE;
#if defined(SPX_OWNED_DATA_TESTING)
    if (context->fault == UINT32_C(2)) {{ context->fault = UINT32_C(0); return SPX_OWNED_DATA_SETTLEMENT_FAILURE; }}
#endif
    spx_bytes_drop(&slot->bytes);
    slot->live = false;
    --context->live_slots;
    return SPX_OWNED_DATA_SUCCESS;
}}
#if defined(SPX_OWNED_DATA_TESTING)
SPX_OWNED_DATA_EXPORT void spx_owned_data_test_fault_v1(spx_context_v1 *context, uint32_t fault) {{ if (context != NULL) context->fault = fault; }}
#endif
"#
    )
    .expect("writing provider runtime cannot fail");
}

fn emit_export(
    output: &mut String,
    program: &ResolvedProgram,
    export: &crate::project::PublicApiExport,
    return_type: &ResolvedType,
    project_v8: bool,
) -> Result<(), Diagnostic> {
    let symbol = provider_call_symbol(export.rust_method_name());
    write!(
        output,
        "SPX_OWNED_DATA_EXPORT spx_owned_data_status_v1 {symbol}(spx_context_v1 *context"
    )
    .unwrap();
    for (index, parameter) in export.parameters().iter().enumerate() {
        match parameter.ty() {
            PublicApiParameterType::I64 => write!(output, ", int64_t arg_{index}"),
            PublicApiParameterType::Bool => write!(output, ", uint8_t arg_{index}"),
            PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => {
                write!(
                    output,
                    ", const uint8_t *arg_{index}, uint64_t arg_{index}_len"
                )
            }
        }
        .unwrap();
    }
    match export.result() {
        PublicApiResultType::I64 => output.push_str(", int64_t *value_out) {\n"),
        PublicApiResultType::Bool => output.push_str(", uint8_t *value_out) {\n"),
        PublicApiResultType::Usize => output.push_str(", uint64_t *value_out) {\n"),
        PublicApiResultType::OwnedBytes
        | PublicApiResultType::OptionOwnedBytes
        | PublicApiResultType::ResultOwnedBytesI64 => output.push_str(
            ", uint32_t *tag_out, spx_owned_bytes_handle_v1 *handle_out, int64_t *error_out) {\n",
        ),
    }
    match export.result() {
        PublicApiResultType::I64 | PublicApiResultType::Bool | PublicApiResultType::Usize => {
            output.push_str("    if (context == NULL || value_out == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n");
        }
        PublicApiResultType::OwnedBytes
        | PublicApiResultType::OptionOwnedBytes
        | PublicApiResultType::ResultOwnedBytesI64 => output.push_str("    if (context == NULL || tag_out == NULL || handle_out == NULL || error_out == NULL || context->marker != SPX_OWNED_DATA_CONTEXT_MARKER || spx_owned_data_overlap_v1(tag_out, sizeof(*tag_out), handle_out, sizeof(*handle_out)) || spx_owned_data_overlap_v1(tag_out, sizeof(*tag_out), error_out, sizeof(*error_out)) || spx_owned_data_overlap_v1(handle_out, sizeof(*handle_out), error_out, sizeof(*error_out)) || *handle_out != UINT64_C(0)) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n"),
    }
    output.push_str("    uint64_t borrowed = UINT64_C(0);\n");
    for (index, parameter) in export.parameters().iter().enumerate() {
        match parameter.ty() {
            PublicApiParameterType::Bool => writeln!(output, "    if (arg_{index} > UINT8_C(1)) return SPX_OWNED_DATA_ADAPTER_FAILURE;"),
            PublicApiParameterType::BorrowStr => writeln!(output, "    if (arg_{index}_len > UINT64_C(65536) - borrowed || (arg_{index}_len != UINT64_C(0) && arg_{index} == NULL) || !spx_owned_data_utf8_v1(arg_{index}, arg_{index}_len)) return SPX_OWNED_DATA_ADAPTER_FAILURE; borrowed += arg_{index}_len; spx_str_v1 value_{index} = {{ .data = arg_{index}, .len = arg_{index}_len }};"),
            PublicApiParameterType::BorrowSliceU8 => writeln!(output, "    if (arg_{index}_len > UINT64_C(65536) - borrowed || (arg_{index}_len != UINT64_C(0) && arg_{index} == NULL)) return SPX_OWNED_DATA_ADAPTER_FAILURE; borrowed += arg_{index}_len; spx_slice_u8_v1 value_{index} = {{ .ptr = arg_{index}_len == UINT64_C(0) ? NULL : arg_{index}, .len = arg_{index}_len }};"),
            PublicApiParameterType::I64 => Ok(()),
        }.unwrap();
    }
    if project_v8 {
        output.push_str("    (void)borrowed;\n");
    }
    writeln!(
        output,
        "    struct spx_status_entry statuses[UINT32_C({STATUS_CAPACITY})];"
    )
    .unwrap();
    output.push_str("    struct spx_context semantic = {0};\n    if (context->invocation == UINT64_MAX) return SPX_OWNED_DATA_ADAPTER_FAILURE;\n    ++context->invocation;\n");
    writeln!(output, "    if (!spx_context_init(&semantic, context->invocation, statuses, UINT32_C({STATUS_CAPACITY}), NULL, NULL, NULL)) return SPX_OWNED_DATA_ADAPTER_FAILURE;").unwrap();

    let call = c_function_symbol(export.stable_id());
    let arguments = export
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, parameter)| match parameter.ty() {
            PublicApiParameterType::I64 => format!("arg_{index}"),
            PublicApiParameterType::Bool => format!("arg_{index} != UINT8_C(0)"),
            PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => {
                format!("value_{index}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let comma = if arguments.is_empty() { "" } else { ", " };
    match export.result() {
        PublicApiResultType::I64 => {
            output.push_str("    int64_t result = INT64_C(0);\n");
            writeln!(output, "    if ({call}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;").unwrap();
            output.push_str("    *value_out = result;\n    return SPX_OWNED_DATA_SUCCESS;\n");
        }
        PublicApiResultType::Bool => {
            output.push_str("    bool result = false;\n");
            writeln!(output, "    if ({call}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;").unwrap();
            output.push_str("    *value_out = result ? UINT8_C(1) : UINT8_C(0);\n    return SPX_OWNED_DATA_SUCCESS;\n");
        }
        PublicApiResultType::Usize => {
            output.push_str("    uint64_t result = UINT64_C(0);\n");
            writeln!(output, "    if ({call}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;").unwrap();
            output.push_str("    *value_out = result;\n    return SPX_OWNED_DATA_SUCCESS;\n");
        }
        PublicApiResultType::OwnedBytes => {
            output.push_str("    spx_bytes_v1 result = {0};\n");
            writeln!(output, "    if ({call}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;").unwrap();
            output.push_str("    spx_owned_bytes_handle_v1 published = UINT64_C(0);\n    spx_owned_data_status_v1 attached = spx_owned_data_attach_v1(context, &result, &published);\n    if (attached != SPX_OWNED_DATA_SUCCESS) { spx_bytes_drop(&result); return attached; }\n    *error_out = INT64_C(0); *handle_out = published; *tag_out = UINT32_C(0);\n    return SPX_OWNED_DATA_SUCCESS;\n");
        }
        PublicApiResultType::OptionOwnedBytes => {
            emit_variant_result(output, program, return_type, &call, comma, &arguments, true)?
        }
        PublicApiResultType::ResultOwnedBytesI64 => emit_variant_result(
            output,
            program,
            return_type,
            &call,
            comma,
            &arguments,
            false,
        )?,
    }
    output.push_str("}\n");
    Ok(())
}

fn emit_variant_result(
    output: &mut String,
    program: &ResolvedProgram,
    ty: &ResolvedType,
    call: &str,
    comma: &str,
    arguments: &str,
    option: bool,
) -> Result<(), Diagnostic> {
    let layout = VariantLayout::for_type(program, VariantTarget::Native64, ty)?;
    layout.validate(program)?;
    let variant = c_variant_symbol(ty);
    writeln!(output, "    struct {variant} result = {{0}};").unwrap();
    writeln!(output, "    if ({call}(&semantic{comma}{arguments}, &result) != SPX_STATUS_SUCCESS) return SPX_OWNED_DATA_SEMANTIC_FAILURE;").unwrap();
    let owned_id = DeclarationId::new(if option {
        crate::prelude::OPTION_SOME_ID
    } else {
        crate::prelude::RESULT_OK_ID
    });
    let owned = layout
        .case(&owned_id)
        .ok_or_else(|| provider_error("owned result case layout is absent"))?;
    let owned_field = owned
        .fields
        .iter()
        .find(|field| field.ty == ResolvedType::Bytes)
        .ok_or_else(|| provider_error("owned result field layout is absent"))?;
    if option {
        let none = layout
            .case(&DeclarationId::new(crate::prelude::OPTION_NONE_ID))
            .ok_or_else(|| provider_error("Option None layout is absent"))?;
        writeln!(output, "    if (result.spx_tag == UINT32_C({})) {{ *error_out = INT64_C(0); *tag_out = UINT32_C(0); return SPX_OWNED_DATA_SUCCESS; }}", none.tag).unwrap();
    } else {
        let err = layout
            .case(&DeclarationId::new(crate::prelude::RESULT_ERR_ID))
            .ok_or_else(|| provider_error("Result Err layout is absent"))?;
        let error_field = err
            .fields
            .iter()
            .find(|field| field.ty == ResolvedType::I64)
            .ok_or_else(|| provider_error("Result error field layout is absent"))?;
        writeln!(output, "    if (result.spx_tag == UINT32_C({})) {{ *error_out = result.spx_payload.{}.{}; *tag_out = UINT32_C(1); return SPX_OWNED_DATA_SUCCESS; }}", err.tag, c_case_symbol(&err.case), c_field_symbol(&error_field.field)).unwrap();
    }
    writeln!(
        output,
        "    if (result.spx_tag != UINT32_C({})) abort();",
        owned.tag
    )
    .unwrap();
    writeln!(
        output,
        "    spx_bytes_v1 *owned = &result.spx_payload.{}.{};",
        c_case_symbol(&owned.case),
        c_field_symbol(&owned_field.field)
    )
    .unwrap();
    writeln!(output, "    spx_owned_bytes_handle_v1 published = UINT64_C(0);\n    spx_owned_data_status_v1 attached = spx_owned_data_attach_v1(context, owned, &published);\n    if (attached != SPX_OWNED_DATA_SUCCESS) {{ spx_bytes_drop(owned); return attached; }}\n    *error_out = INT64_C(0); *handle_out = published; *tag_out = UINT32_C({});\n    return SPX_OWNED_DATA_SUCCESS;", usize::from(option)).unwrap();
    Ok(())
}

pub(crate) fn provider_call_symbol(rust_method: &str) -> String {
    format!("spx_owned_data_call_{rust_method}_v1")
}

pub(super) fn public_provider_call_symbol(rust_method: &str) -> String {
    provider_call_symbol(rust_method)
}

fn provider_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B113", message)
}
