//! [`REVERSE_PROBE_MJS`]: a small, real, hand-authored Node script that
//! proves the byte-reversal fixture endpoint and a genuine
//! `WebAssembly.Memory` allocate/write/release for real under a real
//! WebAssembly host. See the script's own header comment and [Public
//! Generic Carrier
//! v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155)
//! for exactly what this proves and what it deliberately does not (the full
//! handle/registry/sticky-settlement protocol, which
//! [`super::provider::WasmProvider`] exercises in Rust against the shared
//! `CarrierCallMachine` instead).

/// The versioned probe script text, exposed so
/// `tests/public_generic_wasm_adapter_v1` can write it out and run it under
/// Node without embedding a second copy.
pub const REVERSE_PROBE_MJS: &str = include_str!("reverse_probe.mjs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_script_is_nonempty_and_versioned_for_a_real_webassembly_memory() {
        assert!(REVERSE_PROBE_MJS.contains("WebAssembly.Memory"));
        assert!(REVERSE_PROBE_MJS.contains("reverse()"));
    }
}
