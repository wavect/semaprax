# wasm/line_command_io.rs

- OUTPUT_STATUS_GLOBAL · constant · L10-L10 — pub(super) const OUTPUT_STATUS_GLOBAL: u32 = 15;
- OUTPUT_STATUS_EXPORT · constant · L11-L11 — pub(super) const OUTPUT_STATUS_EXPORT: &str = "__spx_command_output_status_v1";
- append_global · function · L13-L15 — pub(super) fn append_global(globals: &mut Vec<u8>)
- append_export · function · L17-L21 — pub(super) fn append_export(exports: &mut Vec<u8>)
- emit_reset · function · L23-L26 — pub(super) fn emit_reset(body: &mut Vec<u8>)
