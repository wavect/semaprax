//! Additive Wasm metadata for fallible line-command output operations.
//!
//! Command-input and command-output failures deliberately use distinct public
//! markers. The ordinary status global remains the control carrier; these
//! markers only authenticate which closed compiler-owned status domain
//! produced it.

use super::{write_u32, I32};

pub(super) const OUTPUT_STATUS_GLOBAL: u32 = 15;
pub(super) const OUTPUT_STATUS_EXPORT: &str = "__spx_command_output_status_v1";

pub(super) fn append_global(globals: &mut Vec<u8>) {
    globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
}

pub(super) fn append_export(exports: &mut Vec<u8>) {
    super::write_name(exports, OUTPUT_STATUS_EXPORT);
    exports.push(0x03);
    write_u32(exports, OUTPUT_STATUS_GLOBAL);
}

pub(super) fn emit_reset(body: &mut Vec<u8>) {
    body.extend([0x41, 0x00, 0x24]);
    write_u32(body, OUTPUT_STATUS_GLOBAL);
}
