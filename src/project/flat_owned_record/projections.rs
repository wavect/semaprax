use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::diagnostic::quote_json;

use super::{FlatOwnedRecordApiDescriptor, FlatOwnedRecordExport, PublicApiParameterType};

/// Render the low-level C11 boundary implemented by the authenticated native
/// provider. Record results use descriptor-order `uint64_t` carrier slots;
/// owned byte slots are opaque handles and remain provider-owned until dropped.
pub fn render_flat_owned_record_c_header(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut output = String::from(
        "#ifndef SEMAPRAX_FLAT_OWNED_RECORD_V1_H\n#define SEMAPRAX_FLAT_OWNED_RECORD_V1_H\n#include <stdint.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint32_t spx_owned_data_status_v1;\ntypedef uint64_t spx_owned_bytes_handle_v1;\ntypedef struct spx_owned_data_context_v1 spx_context_v1;\nenum { SPX_OWNED_DATA_SUCCESS=0, SPX_OWNED_DATA_SEMANTIC_FAILURE=1, SPX_OWNED_DATA_ADAPTER_FAILURE=2, SPX_OWNED_DATA_INVALID_HANDLE=3, SPX_OWNED_DATA_COPY_FAILURE=4, SPX_OWNED_DATA_SETTLEMENT_FAILURE=5 };\nenum { SPX_FLAT_RECORD_I64=0, SPX_FLAT_RECORD_BOOL=1, SPX_FLAT_RECORD_USIZE=2, SPX_FLAT_RECORD_OWNED_BYTES=3 };\nuint64_t spx_owned_data_context_size_v1(void);\nuint64_t spx_owned_data_context_align_v1(void);\nspx_owned_data_status_v1 spx_owned_data_context_init_v1(void*,uint64_t);\nspx_owned_data_status_v1 spx_owned_data_context_drop_v1(spx_context_v1*);\nspx_owned_data_status_v1 spx_owned_bytes_len_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint64_t*);\nspx_owned_data_status_v1 spx_owned_bytes_copy_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint8_t*,uint64_t);\nspx_owned_data_status_v1 spx_owned_bytes_drop_v1(spx_context_v1*,spx_owned_bytes_handle_v1);\n",
    );
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        writeln!(
            output,
            "#define SPX_FLAT_RECORD_EXPORT_{export_index}_FIELD_COUNT UINT32_C({})",
            export.fields.len()
        )
        .unwrap();
        for (field_index, field) in export.fields.iter().enumerate() {
            debug_assert_eq!(field_index, field.ordinal as usize);
            writeln!(
                output,
                "#define SPX_FLAT_RECORD_EXPORT_{export_index}_FIELD_{field_index} UINT32_C({field_index})"
            )
            .unwrap();
            let kind = match field.ty {
                super::FlatOwnedRecordFieldType::I64 => "SPX_FLAT_RECORD_I64",
                super::FlatOwnedRecordFieldType::Bool => "SPX_FLAT_RECORD_BOOL",
                super::FlatOwnedRecordFieldType::Usize => "SPX_FLAT_RECORD_USIZE",
                super::FlatOwnedRecordFieldType::OwnedBytes => "SPX_FLAT_RECORD_OWNED_BYTES",
            };
            writeln!(
                output,
                "#define SPX_FLAT_RECORD_EXPORT_{export_index}_FIELD_{field_index}_KIND {kind}"
            )
            .unwrap();
        }
        write!(
            output,
            "spx_owned_data_status_v1 spx_owned_data_call_{}_v1(spx_context_v1*",
            export.rust_method_name
        )
        .unwrap();
        for (_, _, parameter) in &export.parameters {
            match parameter {
                PublicApiParameterType::I64 => output.push_str(",int64_t"),
                PublicApiParameterType::Bool => output.push_str(",uint8_t"),
                PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => {
                    output.push_str(",const uint8_t*,uint64_t")
                }
            }
        }
        writeln!(output, ",uint64_t[static {}]);", export.fields.len()).unwrap();
    }
    output.push_str("#ifdef __cplusplus\n}\n#endif\n#endif\n");
    output
}

pub fn render_flat_owned_record_typescript(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::new();
    for (name, export) in records {
        output.push_str("export interface ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("  readonly ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.typescript());
            output.push_str(";\n");
        }
        output.push_str("}\n");
    }
    output.push_str("export interface SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("  readonly ");
        output.push_str(&quote_json(export.typescript_name()));
        output.push_str(": (");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str("arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_typescript(*ty));
        }
        output.push_str(") => ");
        output.push_str(&export.record_host_name);
        output.push_str(";\n");
    }
    output.push_str("}\n");
    output
}

pub fn render_flat_owned_record_rust(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::from("#![forbid(unsafe_code)]\n#[derive(Clone, Debug, Eq, PartialEq)]\npub struct CallError { message: &'static str }\nimpl CallError { pub fn message(&self) -> &str { self.message } }\n");
    for (name, export) in records {
        output.push_str("#[derive(Clone, Debug, Eq, PartialEq)]\npub struct ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("    pub ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.rust());
            output.push_str(",\n");
        }
        output.push_str("}\n");
    }
    output.push_str("pub trait SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("    fn ");
        output.push_str(export.rust_method_name());
        output.push_str("(&self");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            output.push_str(", arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_rust(*ty));
        }
        output.push_str(") -> Result<");
        output.push_str(&export.record_host_name);
        output.push_str(", CallError>;\n");
    }
    output.push_str("}\n");
    output
}

fn parameter_typescript(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "bigint",
        PublicApiParameterType::Bool => "boolean",
        PublicApiParameterType::BorrowStr => "string",
        PublicApiParameterType::BorrowSliceU8 => "Uint8Array",
    }
}

fn parameter_rust(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "i64",
        PublicApiParameterType::Bool => "bool",
        PublicApiParameterType::BorrowStr => "&str",
        PublicApiParameterType::BorrowSliceU8 => "&[u8]",
    }
}
