# image_transport/vnext/symbol_diagnostics.rs

- REPORT_SCHEMA · constant · L6-L6 — const REPORT_SCHEMA: &str = "semaprax.project-candidate-symbol-diagnostics.v1";
- CHUNK_SCHEMA · constant · L7-L7 — const CHUNK_SCHEMA: &str = "semaprax.image-symbol-diagnostics-chunk.v1";
- MAX_ATTEMPTS_CONSIDERED · constant · L8-L8 — const MAX_ATTEMPTS_CONSIDERED: usize = 16;
- MAX_REPAIR_DISCOVERIES · constant · L9-L9 — const MAX_REPAIR_DISCOVERIES: usize = 4;
- MAX_REPORT_BYTES · constant · L10-L10 — const MAX_REPORT_BYTES: usize = 1024 * 1024;
- REPORT_DOMAIN · constant · L11-L11 — const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-symbol-diagnostics.report.v1\0";
- prepare · function · L13-L27 — pub(super) fn prepare(
- validate_parameters_before_selection · function · L29-L50 — pub(in crate::image_transport) fn validate_parameters_before_selection(
- for_subjects · function · L52-L165 — pub(super) fn for_subjects(
- render · function · L167-L197 — fn render(mut value: Value) -> Result<String, Vec<Diagnostic>>
- Sink · struct · L168-L168 — struct Sink(Vec<u8>);
- write · function · L170-L176 — fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize>
- flush · function · L177-L179 — fn flush(&mut self) -> std::io::Result<()>
