//! Explicit additive streaming-command linking; old useful-data routes stay closed.
use super::*;
pub(crate) fn link_stdin_stream_command_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::StdinStreamCommand { command },
    )
}
pub(super) fn validate_selected_command(
    profile: &WorkspaceIoProfile,
    functions: &[LinkedScalarFunction],
) -> Result<(), Diagnostic> {
    if let WorkspaceIoProfile::LanguageCommand { command }
    | WorkspaceIoProfile::LineCommand { command }
    | WorkspaceIoProfile::StdinStreamCommand { command } = profile
    {
        let selected = functions
            .iter()
            .find(|linked| &linked.function.id == command)
            .ok_or_else(|| link_error("workspace language-command identity is absent"))?;
        if selected.origin != IdentityOrigin::Explicit
            || !selected.function.params.is_empty()
            || selected.function.return_type != ResolvedType::Bool
        {
            return Err(link_error(
                "workspace language command must be an explicit stable-ID `fn () -> bool`",
            ));
        }
    }
    Ok(())
}
