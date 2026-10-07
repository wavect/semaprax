//! Project v23 admission for the native streaming stdin command contract.

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram, ResolvedType};

pub(super) fn admit(program: &ResolvedProgram, command: &str) -> Result<(), Diagnostic> {
    crate::hir::validate(program)?;
    let id = DeclarationId::new(command);
    let function = program
        .functions
        .iter()
        .find(|function| function.id == id)
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-J105",
                "Project v23 selected stream command is absent from the linked program",
            )
        })?;
    if !function.type_parameters.is_empty()
        || !function.params.is_empty()
        || function.return_type != ResolvedType::Bool
    {
        return Err(Diagnostic::io(
            "SPX-J105",
            "Project v23 stream command must have the exact signature fn() -> bool",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &id,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )
}
