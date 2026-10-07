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
        WorkspaceIoProfile::StdinStreamCommand(command, false),
    )
}
pub(super) fn validate_selected_command(
    profile: &WorkspaceIoProfile,
    functions: &[LinkedScalarFunction],
) -> Result<(), Diagnostic> {
    let exit_status = matches!(
        profile,
        WorkspaceIoProfile::StdinStreamCommand(_, true)
            | WorkspaceIoProfile::StdinStreamTextCommand(_)
    );
    if let WorkspaceIoProfile::LanguageCommand { command }
    | WorkspaceIoProfile::LineCommand { command }
    | WorkspaceIoProfile::StdinStreamCommand(command, _)
    | WorkspaceIoProfile::StdinStreamTextCommand(command) = profile
    {
        let selected = functions
            .iter()
            .find(|linked| &linked.function.id == command)
            .ok_or_else(|| link_error("workspace language-command identity is absent"))?;
        if selected.origin != IdentityOrigin::Explicit
            || !selected.function.params.is_empty()
            || selected.function.return_type
                != if exit_status {
                    ResolvedType::I64
                } else {
                    ResolvedType::Bool
                }
        {
            return Err(link_error(if exit_status {
                "workspace stream exit command must be an explicit stable-ID `fn () -> i64`"
            } else {
                "workspace language command must be an explicit stable-ID `fn () -> bool`"
            }));
        }
    }
    Ok(())
}

pub(crate) fn link_stdin_stream_exit_command_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::StdinStreamCommand(command, true),
    )
}

/// Private String boundaries are explicit; no public owned UTF-8 ABI is added.
pub(crate) fn stream_text_parameter_admitted(parameter: &ResolvedParam) -> bool {
    (parameter.ty == ResolvedType::String && parameter.ownership == OwnershipMode::Own)
        || useful_data_workspace_parameter_admitted(&parameter.ty, parameter.ownership)
        || (crate::stdin_stream_ops::is_reader(&parameter.ty)
            && matches!(
                parameter.ownership,
                OwnershipMode::Own | OwnershipMode::Borrow
            ))
}
pub(crate) fn stream_text_return_admitted(ty: &ResolvedType) -> bool {
    *ty == ResolvedType::String
        || crate::stdin_stream_ops::is_reader(ty)
        || useful_data_workspace_return_admitted(ty)
}
impl WorkspaceIoProfile {
    pub(super) fn is_stream(&self) -> bool {
        matches!(
            self,
            Self::StdinStreamCommand(..)
                | Self::StdinStreamTextCommand(_)
                | Self::StdinStreamTextEntry
        )
    }
    pub(super) fn signature_admitted(&self, function: &ResolvedFunction) -> bool {
        if matches!(
            self,
            Self::StdinStreamTextCommand(_) | Self::StdinStreamTextEntry
        ) {
            stream_text_return_admitted(&function.return_type)
                && (!crate::stdin_stream_ops::is_reader(&function.return_type)
                    || crate::stdin_stream_ops::resolved_forward_signature(function))
                && function.params.iter().all(stream_text_parameter_admitted)
        } else {
            (useful_data_workspace_return_admitted(&function.return_type)
                || (self.is_stream()
                    && crate::stdin_stream_ops::resolved_forward_signature(function)))
                && function.params.iter().all(|p| {
                    useful_data_workspace_parameter_admitted(&p.ty, p.ownership)
                        || (self.is_stream()
                            && crate::stdin_stream_ops::is_reader(&p.ty)
                            && matches!(p.ownership, OwnershipMode::Own | OwnershipMode::Borrow))
                })
        }
    }
}
pub(crate) fn link_stdin_stream_text_entry_workspace(
    module: String,
    entrypoint: DeclarationId,
    functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        functions,
        WorkspaceIoProfile::StdinStreamTextEntry,
    )
}
pub(crate) fn link_stdin_stream_text_command_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        functions,
        WorkspaceIoProfile::StdinStreamTextCommand(command),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_text_string_boundary_is_owned_and_does_not_widen_legacy_profiles() {
        let ast=crate::parse("module t; @id(\"helper\") fn helper(text:string)->string { text } @id(\"app.main\") fn main()->i64 { 0 }",std::path::Path::new("text.spx")).unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let helper = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "helper")
            .unwrap();
        let old = WorkspaceIoProfile::StdinStreamCommand(DeclarationId::new("cmd"), true);
        let selected = WorkspaceIoProfile::StdinStreamTextCommand(DeclarationId::new("cmd"));
        assert!(!old.signature_admitted(helper));
        assert!(selected.signature_admitted(helper));
        let mut forged = helper.clone();
        forged.params[0].ownership = OwnershipMode::Borrow;
        assert!(!selected.signature_admitted(&forged));
        forged = helper.clone();
        forged.return_type = ResolvedType::SliceU8;
        assert!(!selected.signature_admitted(&forged));
    }
}
