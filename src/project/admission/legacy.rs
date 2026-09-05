//! Frozen Project target admission for profiles without public descriptors.

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

pub(super) fn network_command(program: &ResolvedProgram, command: &str) -> Result<(), Diagnostic> {
    crate::wasm::emit_resolved_language_network_io_v1(program, command).map(drop)
}

pub(super) fn https_command(program: &ResolvedProgram, command: &str) -> Result<(), Diagnostic> {
    crate::command_io_ops::validate_operation_profile(
        program,
        &crate::hir::DeclarationId::new(command),
        crate::command_io_ops::CommandOperationProfile::HttpV1,
    )
}
