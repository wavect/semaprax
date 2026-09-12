# project/flat_owned_record/projections.rs

- render_flat_owned_record_c_header · function · L11-L65 — pub fn render_flat_owned_record_c_header(descriptor: &FlatOwnedRecordApiDescriptor) -> String
- render_flat_owned_record_cpp_header · function · L69-L90 — pub fn render_flat_owned_record_cpp_header(descriptor: &FlatOwnedRecordApiDescriptor) -> String
- emit_cpp_method · function · L92-L172 — fn emit_cpp_method(output: &mut String, export: &FlatOwnedRecordExport)
- cpp_field_type · function · L174-L181 — fn cpp_field_type(ty: super::FlatOwnedRecordFieldType) -> &'static str
- cpp_parameter_type · function · L183-L190 — fn cpp_parameter_type(ty: PublicApiParameterType) -> &'static str
- render_flat_owned_record_typescript · function · L192-L231 — pub fn render_flat_owned_record_typescript(descriptor: &FlatOwnedRecordApiDescriptor) -> String
- render_flat_owned_record_rust · function · L233-L269 — pub fn render_flat_owned_record_rust(descriptor: &FlatOwnedRecordApiDescriptor) -> String
- parameter_typescript · function · L271-L278 — fn parameter_typescript(ty: PublicApiParameterType) -> &'static str
- parameter_rust · function · L280-L287 — fn parameter_rust(ty: PublicApiParameterType) -> &'static str
