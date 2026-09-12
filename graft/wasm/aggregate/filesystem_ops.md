# wasm/aggregate/filesystem_ops.rs

- INVALID_PATH · constant · L6-L6 — const INVALID_PATH: i32 = crate::filesystem_ops::INVALID_PATH as i32;
- CAPACITY_EXCEEDED · constant · L7-L7 — const CAPACITY_EXCEEDED: i32 = crate::filesystem_ops::CAPACITY_EXCEEDED as i32;
- LAST_STATUS · constant · L8-L8 — const LAST_STATUS: i32 = crate::filesystem_ops::INVALID_FILE_TYPE as i32;
- emit_filesystem_command_call · function · L11-L242 — pub(super) fn emit_filesystem_command_call(
- emit_filesystem_failure_if_code · function · L244-L256 — fn emit_filesystem_failure_if_code(
- emit_filesystem_path_validation · function · L261-L386 — fn emit_filesystem_path_validation(
- emit_filesystem_failure_if · function · L388-L393 — fn emit_filesystem_failure_if(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic>
- reserve_filesystem_work · function · L398-L437 — fn reserve_filesystem_work(
- emit_filesystem_exit · function · L439-L454 — pub(super) fn emit_filesystem_exit(
