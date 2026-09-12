# wasm/aggregate/filesystem_v2.rs

- list_get_local · function · L5-L8 — fn list_get_local(&mut self, local: u32)
- list_set_local · function · L9-L12 — fn list_set_local(&mut self, local: u32)
- list_integer · function · L13-L16 — fn list_integer(&mut self, value: i64)
- list_byte · function · L17-L23 — fn list_byte(&mut self, carrier: u32, index: u32)
- list_failure_if · function · L24-L41 — fn list_failure_if(
- validate_filesystem_list · function · L42-L193 — pub(super) fn validate_filesystem_list(
- ScanLocals · type · L196-L196 — type ScanLocals = (Option<(u32, u32, u32)>, Option<[u32; 6]>);
- allocate_scan_locals · function · L197-L222 — pub(super) fn allocate_scan_locals(
- append_command_exports · function · L224-L251 — pub(super) fn append_command_exports(
