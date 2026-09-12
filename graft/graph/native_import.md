# graph/native_import.rs

- NATIVE_RUST_IMPORT_SCHEMA · constant · L19-L19 — pub(crate) const NATIVE_RUST_IMPORT_SCHEMA: &str = "semaprax.graph.v25";
- CLOSED_PROJECTION · constant · L21-L22 — const CLOSED_PROJECTION: &str =
- declares_native_rust_import · function · L29-L34 — pub(crate) fn declares_native_rust_import(interfaces: &[ResolvedInterface]) -> bool
- result_text · function · L37-L43 — pub(crate) fn result_text(kind: &ResolvedImportResultKind) -> &'static str
- reject_native_rust_imports · function · L47-L53 — pub(crate) fn reject_native_rust_imports(program: &ResolvedProgram) -> Result<(), Diagnostic>
- reject_source_native_rust_imports · function · L57-L68 — pub(crate) fn reject_source_native_rust_imports(program: &Program) -> Result<(), Vec<Diagnostic>>
- append_import_tail · function · L73-L79 — pub(crate) fn append_import_tail(output: &mut CappedString, schema: &str, native_rust: bool)
