//! Signature admission confined to the explicit streaming command profile.

use crate::hir::{self, OwnershipMode, ResolvedParam, ResolvedType};

pub(in crate::workspace_graph) fn stream_parameter_admitted(parameter: &ResolvedParam) -> bool {
    (crate::stdin_stream_ops::is_reader(&parameter.ty)
        && matches!(
            parameter.ownership,
            OwnershipMode::Own | OwnershipMode::Borrow
        ))
        || hir::useful_data_workspace_parameter_admitted(&parameter.ty, parameter.ownership)
}

pub(in crate::workspace_graph) fn stream_return_admitted(ty: &ResolvedType) -> bool {
    crate::stdin_stream_ops::is_reader(ty) || hir::useful_data_workspace_return_admitted(ty)
}

pub(in crate::workspace_graph) fn command_link(
    profile: crate::project::ProjectProfile,
    module: String,
    entrypoint: hir::DeclarationId,
    command: hir::DeclarationId,
    functions: Vec<hir::LinkedScalarFunction>,
) -> Result<hir::ResolvedProgram, crate::diagnostic::Diagnostic> {
    let link = match profile {
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1 => {
            hir::link_stdin_stream_text_command_workspace
        }
        crate::project::ProjectProfile::StdinStreamCommandIoV2 => {
            hir::link_stdin_stream_exit_command_workspace
        }
        crate::project::ProjectProfile::StdinStreamCommandIoV1 => {
            hir::link_stdin_stream_command_workspace
        }
        _ => hir::link_language_command_io_workspace,
    };
    link(module, entrypoint, command, functions)
}
pub(in crate::workspace_graph) fn entry_link(
    profile: crate::project::ProjectProfile,
    module: String,
    entrypoint: hir::DeclarationId,
    functions: Vec<hir::LinkedScalarFunction>,
) -> Result<hir::ResolvedProgram, crate::diagnostic::Diagnostic> {
    let link = if profile == crate::project::ProjectProfile::StdinStreamTextCommandIoV1 {
        hir::link_stdin_stream_text_entry_workspace
    } else {
        hir::link_useful_data_workspace
    };
    link(module, entrypoint, functions)
}

/// The additive private text profile admits only monomorphic record schemas.
/// Exact field ownership, shape bounds and invariants are rederived by the linked HIR profile.
pub(in crate::workspace_graph) fn text_project_shape(
    module: &super::WorkspaceResolvedModule,
) -> bool {
    module.interfaces.is_empty()
        && module.function_templates.is_empty()
        && module.function_instances.is_empty()
        && module.types.iter().all(|ty| {
            ty.type_parameters.is_empty()
                && matches!(ty.kind, hir::ResolvedTypeDeclarationKind::Record { .. })
        })
}
pub(in crate::workspace_graph) fn text_command_program(
    program: &hir::ResolvedProgram,
    command: &str,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let command = hir::DeclarationId::new(command);
    hir::validate_stream_text_program(program, Some(&command))?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )
}
