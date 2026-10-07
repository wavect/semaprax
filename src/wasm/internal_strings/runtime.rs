//! Private trusted host for the standalone profile; no earlier runtime is reused or changed.

use crate::diagnostic::quote_json;

#[cfg(test)]
mod tests;

pub(super) fn render(
    descriptor: &str,
    wasm_sha256: &str,
    wasm_byte_length: usize,
    integer_conversions: bool,
) -> String {
    let runtime = crate::bounded_output::budgeted_format(format_args!(
        "// semaprax.wasm-internal-strings.runtime.v1\nconst DESCRIPTOR={descriptor};\nconst EXPECTED_SHA256={};\nconst EXPECTED_BYTES={wasm_byte_length};\n{}\n{}\n{}",
        quote_json(wasm_sha256),
        include_str!("runtime/input.js"),
        include_str!("runtime/arena.js"),
        include_str!("runtime/facade.js"),
    ));
    if !integer_conversions {
        return runtime;
    }
    const STATUS_GUARD: &str = "status<0||status>11||memory.buffer";
    const CAPACITY_CASE: &str =
        "else if(status===11)result=Object.freeze({kind:\"capacity\",cause});";
    assert_eq!(runtime.matches(STATUS_GUARD).count(), 1);
    assert_eq!(runtime.matches(CAPACITY_CASE).count(), 1);
    runtime
        .replacen(
            STATUS_GUARD,
            "status<0||(status>11&&status!==21)||memory.buffer",
            1,
        )
        .replacen(
            CAPACITY_CASE,
            "else if(status===11)result=Object.freeze({kind:\"capacity\",cause});\n      else if(status===21)result=Object.freeze({kind:\"failure\",domain:\"semaprax.convert.v1\",code:1});",
            1,
        )
}
