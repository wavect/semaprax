use std::fmt::Write as _;

use super::{NestedOwnedRecordApiDescriptor, NestedOwnedRecordLeafType, PublicApiParameterType};

/// Render the low-level C11 boundary for the authenticated Project-v11 native
/// provider. Leaves are in descriptor path order, never target layout order.
pub fn render_nested_owned_record_c_header(descriptor: &NestedOwnedRecordApiDescriptor) -> String {
    let mut output = String::from(
        "#ifndef SEMAPRAX_NESTED_OWNED_RECORD_V1_H\n#define SEMAPRAX_NESTED_OWNED_RECORD_V1_H\n#include <stdint.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint32_t spx_owned_data_status_v1;\ntypedef uint64_t spx_owned_bytes_handle_v1;\ntypedef struct spx_owned_data_context_v1 spx_context_v1;\nenum { SPX_OWNED_DATA_SUCCESS=0, SPX_OWNED_DATA_SEMANTIC_FAILURE=1, SPX_OWNED_DATA_ADAPTER_FAILURE=2, SPX_OWNED_DATA_INVALID_HANDLE=3, SPX_OWNED_DATA_COPY_FAILURE=4, SPX_OWNED_DATA_SETTLEMENT_FAILURE=5 };\nenum { SPX_NESTED_RECORD_I64=0, SPX_NESTED_RECORD_BOOL=1, SPX_NESTED_RECORD_USIZE=2, SPX_NESTED_RECORD_OWNED_BYTES=3 };\nuint64_t spx_owned_data_context_size_v1(void);\nuint64_t spx_owned_data_context_align_v1(void);\nspx_owned_data_status_v1 spx_owned_data_context_init_v1(void*,uint64_t);\nspx_owned_data_status_v1 spx_owned_data_context_drop_v1(spx_context_v1*);\nspx_owned_data_status_v1 spx_owned_bytes_len_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint64_t*);\nspx_owned_data_status_v1 spx_owned_bytes_copy_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint8_t*,uint64_t);\nspx_owned_data_status_v1 spx_owned_bytes_drop_v1(spx_context_v1*,spx_owned_bytes_handle_v1);\n",
    );
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        writeln!(
            output,
            "#define SPX_NESTED_RECORD_EXPORT_{export_index}_LEAF_COUNT UINT32_C({})",
            export.leaves.len()
        )
        .unwrap();
        for (leaf_index, leaf) in export.leaves.iter().enumerate() {
            debug_assert_eq!(leaf_index, leaf.ordinal as usize);
            let kind = match leaf.ty {
                NestedOwnedRecordLeafType::I64 => "SPX_NESTED_RECORD_I64",
                NestedOwnedRecordLeafType::Bool => "SPX_NESTED_RECORD_BOOL",
                NestedOwnedRecordLeafType::Usize => "SPX_NESTED_RECORD_USIZE",
                NestedOwnedRecordLeafType::OwnedBytes => "SPX_NESTED_RECORD_OWNED_BYTES",
            };
            writeln!(
                output,
                "#define SPX_NESTED_RECORD_EXPORT_{export_index}_LEAF_{leaf_index} UINT32_C({leaf_index})\n#define SPX_NESTED_RECORD_EXPORT_{export_index}_LEAF_{leaf_index}_KIND {kind}"
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
        writeln!(output, ",uint64_t[static {}]);", export.leaves.len()).unwrap();
    }
    output.push_str("#ifdef __cplusplus\n}\n#endif\n#endif\n");
    output
}
