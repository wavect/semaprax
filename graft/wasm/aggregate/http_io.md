# wasm/aggregate/http_io.rs

- MAX_URL_BYTES · constant · L6-L6 — const MAX_URL_BYTES: i64 = 2_048;
- MAX_RESPONSE_BYTES · constant · L7-L7 — const MAX_RESPONSE_BYTES: i64 = crate::network_io_ops::MAX_CHUNK_BYTES as i64;
- INVALID_URL · constant · L8-L8 — const INVALID_URL: i32 = crate::network_io_ops::HTTP_INVALID_URL as i32;
- RESPONSE_TOO_LARGE · constant · L9-L9 — const RESPONSE_TOO_LARGE: i32 = crate::network_io_ops::HTTP_RESPONSE_TOO_LARGE as i32;
- LAST_STATUS · constant · L10-L10 — const LAST_STATUS: i32 = crate::network_io_ops::HTTP_AUTHORITY_DENIED as i32;
- emit_http_command_call · function · L13-L90 — pub(super) fn emit_http_command_call(
- emit_http_failure_if_code · function · L92-L104 — fn emit_http_failure_if_code(
- emit_http_failure_if · function · L106-L111 — fn emit_http_failure_if(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic>
- emit_http_exit · function · L113-L125 — fn emit_http_exit(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic>
