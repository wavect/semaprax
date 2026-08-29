//! Frozen Project-v1-through-v7 target admission.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

pub(super) fn scalar(program: &ResolvedProgram, exports: &[String]) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_module_with_scalar_exports(program, exports).map(drop)
}

pub(super) fn useful_text(program: &ResolvedProgram, exports: &[String]) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_module_with_text_exports(program, exports).map(drop)
}

pub(super) fn useful_data(program: &ResolvedProgram, exports: &[String]) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_module_with_byte_exports(program, exports).map(drop)
}

pub(super) fn useful_data_command_v1(
    program: &ResolvedProgram,
    exports: &[String],
) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_module_with_byte_exports_and_stdout_transcript(program, exports)
        .map(drop)
}

pub(super) fn useful_data_command_v2(
    program: &ResolvedProgram,
    command: &str,
) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_useful_data_command_v2(program, command).map(drop)
}

pub(super) fn language_command(program: &ResolvedProgram, command: &str) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_language_command_io_v1(program, command).map(drop)
}

pub(super) fn line_command(program: &ResolvedProgram, command: &str) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_line_command_io_v1(program, command).map(drop)
}
