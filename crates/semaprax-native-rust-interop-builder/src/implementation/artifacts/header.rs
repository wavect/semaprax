//! Generated C header projection.

use super::*;

pub(super) fn c_parameters(parameters: &[ParameterFact]) -> String {
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

pub(in crate::implementation) fn generate_header_with_limit(
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

pub(in crate::implementation) fn generate_header(
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    generate_header_with_limit(exports, imports, MAX_GENERATED_HEADER_BYTES)
}
