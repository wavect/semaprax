//! Private trusted host for the standalone profile; no earlier runtime is reused or changed.

use crate::diagnostic::quote_json;

#[cfg(test)]
mod tests;

pub(super) fn render(descriptor: &str, wasm_sha256: &str, wasm_byte_length: usize) -> String {
    crate::bounded_output::budgeted_format(format_args!(
        "// semaprax.wasm-internal-strings.runtime.v1\nconst DESCRIPTOR={descriptor};\nconst EXPECTED_SHA256={};\nconst EXPECTED_BYTES={wasm_byte_length};\n{}\n{}\n{}",
        quote_json(wasm_sha256),
        include_str!("runtime/input.js"),
        include_str!("runtime/arena.js"),
        include_str!("runtime/facade.js"),
    ))
}
