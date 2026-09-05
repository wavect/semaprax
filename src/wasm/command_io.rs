//! Additive raw Wasm boundary for Bounded Language Command I/O v1.

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, IdentityOrigin, ResolvedProgram, ResolvedType};

use super::{write_u32, I32};

pub(super) const IMPORT_COUNT: u32 = 4;
pub(super) const ARGS_LEN_IMPORT: u32 = super::SCALAR_IMPORT_COUNT + 4;
pub(super) const ARG_UTF8_IMPORT: u32 = ARGS_LEN_IMPORT + 1;
pub(super) const STDIN_READ_IMPORT: u32 = ARGS_LEN_IMPORT + 2;
pub(super) const OWNED_BYTES_VALIDATE_IMPORT: u32 = ARGS_LEN_IMPORT + 3;
pub(super) const INPUT_STATUS_GLOBAL: u32 = 14;
pub(super) const INPUT_STATUS_EXPORT: &str = "__spx_command_input_status_v1";

#[derive(Clone, Debug)]
pub(super) struct CommandPlan {
    pub(super) function_id: DeclarationId,
    pub(super) wasm_export: String,
    operation_profile: crate::command_io_ops::CommandOperationProfile,
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    command_id: &str,
    operation_profile: crate::command_io_ops::CommandOperationProfile,
) -> Result<CommandPlan, Diagnostic> {
    crate::hir::validate(program)?;
    if operation_profile == crate::command_io_ops::CommandOperationProfile::NetworkV1 {
        super::network_io::check_permits(&program.permits)?;
    } else if program.permits
        != [
            crate::command_io_ops::ARGS_READ_EFFECT,
            crate::command_io_ops::STDERR_WRITE_EFFECT,
            crate::command_io_ops::STDIN_READ_EFFECT,
            crate::host_io_ops::STDOUT_WRITE_EFFECT,
        ]
    {
        return Err(admission(
            "Language Command I/O v1 requires its exact four permits",
        ));
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| admission("selected Language Command I/O function is absent"))?;
    if program
        .declarations
        .declaration(&function.id)
        .map(|fact| fact.identity_origin)
        != Some(IdentityOrigin::Explicit)
        || !function.params.is_empty()
        || function.return_type != ResolvedType::Bool
    {
        return Err(admission(
            "selected Language Command I/O function must be explicit fn() -> bool",
        ));
    }
    crate::command_io_ops::validate_operation_profile(program, &function.id, operation_profile)?;
    Ok(CommandPlan {
        function_id: function.id.clone(),
        wasm_export: super::data_exports::raw_symbol(command_id),
        operation_profile,
    })
}

impl CommandPlan {
    pub(super) fn is_line_command(&self) -> bool {
        self.operation_profile == crate::command_io_ops::CommandOperationProfile::LineV1
    }

    pub(super) fn is_network_command(&self) -> bool {
        self.operation_profile == crate::command_io_ops::CommandOperationProfile::NetworkV1
    }

    /// Command imports, plus the network imports appended after them for the
    /// network profile only.
    pub(super) fn import_count(&self) -> u32 {
        if self.is_network_command() {
            IMPORT_COUNT + super::network_io::IMPORT_COUNT
        } else {
            IMPORT_COUNT
        }
    }
}

/// Transcript appends and provider validation need one scratch i32 local.
/// Command modules get it from the args permit; network modules always append
/// through the same path, so any network permit reserves it too.
pub(super) fn needs_command_byte(permit: &str) -> bool {
    permit == crate::command_io_ops::ARGS_READ_EFFECT
        || crate::network_io_ops::NETWORK_EFFECTS.contains(&permit)
}

pub(super) fn emit_wrapper_body(target_index: u32, plan: &CommandPlan) -> Vec<u8> {
    let line_command_io = plan.is_line_command();
    const OLD_STACK: u32 = 0;
    const RESULT_OUT: u32 = 1;
    const STATUS: u32 = 2;
    const RESULT: u32 = 3;
    let mut body = Vec::new();
    write_u32(&mut body, 1);
    write_u32(&mut body, 4);
    body.push(I32);
    super::host_output::emit_reset(&mut body, super::host_output::COMMAND_STDOUT_GLOBALS);
    super::host_output::emit_reset(&mut body, super::host_output::COMMAND_STDERR_GLOBALS);
    body.extend([0x41, 0x00, 0x24, 0x01]); // public status = success
    body.extend([0x41, 0x00, 0x24]);
    write_u32(&mut body, INPUT_STATUS_GLOBAL);
    if line_command_io {
        super::line_command_io::emit_reset(&mut body);
    }
    if plan.is_network_command() {
        super::network_io::emit_reset(&mut body);
    }
    body.extend([0x23, 0x00, 0x22]);
    write_u32(&mut body, OLD_STACK);
    body.extend([0x41, 0x08, 0x49, 0x04, 0x40, 0x00, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, OLD_STACK);
    body.extend([0x41, 0x08, 0x6b, 0x22]);
    write_u32(&mut body, RESULT_OUT);
    body.extend([0x24, 0x00, 0x20]);
    write_u32(&mut body, RESULT_OUT);
    body.push(0x10);
    write_u32(&mut body, target_index);
    body.extend([0x21]);
    write_u32(&mut body, STATUS);
    if plan.is_network_command() {
        super::network_io::emit_settle(&mut body);
    }
    body.push(0x20);
    write_u32(&mut body, OLD_STACK);
    body.extend([0x24, 0x00, 0x20]);
    write_u32(&mut body, STATUS);
    body.extend([0x24, 0x01, 0x20]);
    write_u32(&mut body, STATUS);
    body.extend([0x04, 0x40]);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDOUT_GLOBALS);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDERR_GLOBALS);
    body.extend([0x41, 0x00, 0x0f, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, RESULT_OUT);
    body.extend([0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, RESULT);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40]);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDOUT_GLOBALS);
    super::host_output::emit_discard(&mut body, super::host_output::COMMAND_STDERR_GLOBALS);
    body.extend([0x00, 0x0b]);
    super::host_output::emit_publish_immediate(
        &mut body,
        super::host_output::COMMAND_STDOUT_GLOBALS,
    );
    super::host_output::emit_publish_immediate(
        &mut body,
        super::host_output::COMMAND_STDERR_GLOBALS,
    );
    body.push(0x20);
    write_u32(&mut body, RESULT);
    body.push(0x0b);
    body
}

fn admission(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W114", message)
}

#[cfg(test)]
#[path = "command_io/tests.rs"]
mod tests;
