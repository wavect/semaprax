//! Signature admission confined to the explicit streaming command profile.

use crate::hir::{self, OwnershipMode, ResolvedParam, ResolvedType};

pub(in crate::workspace_graph) fn has_separate_command_root(
    profile: crate::project::ProjectProfile,
) -> bool {
    profile.is_stdin_stream()
        || matches!(
            profile,
            crate::project::ProjectProfile::LanguageCommandIoV1
                | crate::project::ProjectProfile::LineCommandIoV1
                | crate::project::ProjectProfile::ProcessIoV1
        )
}

pub(in crate::workspace_graph) fn owned_stream_entry(
    build: &super::super::WorkspaceGraphBuild,
    profile: crate::project::ProjectProfile,
    entry_module: &str,
) -> Result<Option<hir::ResolvedProgram>, Vec<crate::diagnostic::Diagnostic>> {
    let mut linked = match profile {
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1
        | crate::project::ProjectProfile::StdinStreamDataCommandIoV1
        | crate::project::ProjectProfile::StdinStreamDataCommandIoV2
        | crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1
        | crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => {
            build.linked_owned_data_api_program_with_roots(entry_module, &[])?
        }
        _ => return Ok(None),
    };
    match profile {
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1 => {
            hir::validate_stream_text_program(&linked, None)
        }
        crate::project::ProjectProfile::StdinStreamDataCommandIoV1 => {
            hir::validate_stream_data_program(&linked, None)
        }
        crate::project::ProjectProfile::StdinStreamDataCommandIoV2 => {
            hir::validate_stream_record_program(&linked, None)
        }
        crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1 => {
            hir::validate_stream_owned_program(&linked, None)
        }
        crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => {
            hir::validate_stream_collection_record_program(&linked, None)
        }
        _ => unreachable!("non-stream profile returned above"),
    }
    .map_err(|error| vec![error])?;
    build.attach_project_agents(&mut linked)?;
    Ok(Some(linked))
}

pub(in crate::workspace_graph) fn owned_stream_command(
    build: &super::super::WorkspaceGraphBuild,
    profile: crate::project::ProjectProfile,
    entry_module: &str,
    additional_roots: &[String],
    dependency_anchors: bool,
) -> Result<Option<hir::ResolvedProgram>, Vec<crate::diagnostic::Diagnostic>> {
    let label = match profile {
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1 => "text",
        crate::project::ProjectProfile::StdinStreamDataCommandIoV1 => "data",
        crate::project::ProjectProfile::StdinStreamDataCommandIoV2 => "records",
        crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1 => "owned data",
        crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => "collection records",
        _ => return Ok(None),
    };
    if additional_roots.is_empty() {
        return build
            .linked_project_program(entry_module, profile, dependency_anchors)
            .map(Some);
    }
    let [command] = additional_roots else {
        return Err(vec![super::super::graph_error(
            "SPX-G172",
            format!("stream {label} command must select exactly one explicit command"),
        )]);
    };
    let mut linked =
        build.linked_owned_data_api_program_with_roots(entry_module, additional_roots)?;
    match profile {
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1 => {
            text_command_program(&mut linked, command)
        }
        crate::project::ProjectProfile::StdinStreamDataCommandIoV1 => {
            data_command_program(&mut linked, command)
        }
        crate::project::ProjectProfile::StdinStreamDataCommandIoV2 => {
            record_command_program(&mut linked, command)
        }
        crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1 => {
            owned_command_program(&mut linked, command)
        }
        crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => {
            collection_record_command_program(&mut linked, command)
        }
        _ => unreachable!("non-stream profile returned above"),
    }
    .map_err(|error| vec![error])?;
    Ok(Some(linked))
}

pub(in crate::workspace_graph) fn stream_test_program(
    build: &super::super::WorkspaceGraphBuild,
    profile: crate::project::ProjectProfile,
    test_module: &str,
    dependency_anchors: bool,
) -> Result<hir::ResolvedProgram, Vec<crate::diagnostic::Diagnostic>> {
    match profile {
        crate::project::ProjectProfile::SourceCommandV1
        | crate::project::ProjectProfile::SourceCommandResourceOutputV1 => {
            build.linked_source_command_test_program(test_module)
        }
        crate::project::ProjectProfile::StdinStreamDataCommandIoV2 => {
            build.linked_stream_record_test_program(test_module)
        }
        crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1 => {
            build.linked_stream_owned_test_program(test_module)
        }
        crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => {
            build.linked_stream_collection_record_test_program(test_module)
        }
        crate::project::ProjectProfile::StdinStreamDataCommandIoV1 => {
            build.linked_stream_data_test_program(test_module)
        }
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1 => {
            build.linked_stream_text_test_program(test_module)
        }
        _ => build.linked_project_program(test_module, profile, dependency_anchors),
    }
}

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
        crate::project::ProjectProfile::StdinStreamDataCommandIoV2
        | crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1
        | crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => {
            return Err(super::super::graph_error(
                "SPX-G172",
                "stream record linking requires authenticated declaration facts",
            ))
        }

        crate::project::ProjectProfile::StdinStreamDataCommandIoV1 => {
            hir::link_stdin_stream_data_command_workspace
        }
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
    let link = match profile {
        crate::project::ProjectProfile::StdinStreamDataCommandIoV2
        | crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1
        | crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => {
            return Err(super::super::graph_error(
                "SPX-G172",
                "stream record linking requires authenticated declaration facts",
            ))
        }

        crate::project::ProjectProfile::StdinStreamDataCommandIoV1 => {
            hir::link_stdin_stream_data_entry_workspace
        }
        crate::project::ProjectProfile::StdinStreamTextCommandIoV1 => {
            hir::link_stdin_stream_text_entry_workspace
        }
        _ => hir::link_useful_data_workspace,
    };
    link(module, entrypoint, functions)
}

pub(in crate::workspace_graph) fn data_project_shape(
    module: &super::WorkspaceResolvedModule,
) -> bool {
    module.types.is_empty()
        && module.interfaces.is_empty()
        && module.function_templates.is_empty()
        && module.function_instances.is_empty()
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
    program: &mut hir::ResolvedProgram,
    command: &str,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let command = hir::DeclarationId::new(command);
    hir::validate_stream_text_program(program, Some(&command))?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )?;
    // The authenticated Project command adapter owns the full declared
    // capability inventory, including adapter effects unused by its body.
    program.permits = crate::project::PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
        .iter()
        .map(|effect| (*effect).to_owned())
        .collect();
    hir::validate(program)
}

pub(in crate::workspace_graph) fn data_command_program(
    program: &mut hir::ResolvedProgram,
    command: &str,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let command = hir::DeclarationId::new(command);
    hir::validate_stream_data_program(program, Some(&command))?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )?;
    program.permits = crate::project::PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
        .iter()
        .map(|effect| (*effect).to_owned())
        .collect();
    hir::validate(program)
}

/// Reachable types receive exact independent admission after linking; all module types remain source-verified.
pub(in crate::workspace_graph) fn record_project_shape(
    module: &super::WorkspaceResolvedModule,
) -> bool {
    module.interfaces.is_empty()
        && module.function_templates.is_empty()
        && module.function_instances.is_empty()
        && module.types.iter().all(|ty| {
            ty.type_parameters.is_empty()
                && matches!(
                    ty.kind,
                    hir::ResolvedTypeDeclarationKind::Record { .. }
                        | hir::ResolvedTypeDeclarationKind::Variant { .. }
                )
        })
}
pub(in crate::workspace_graph) fn record_command_program(
    program: &mut hir::ResolvedProgram,
    command: &str,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let command = hir::DeclarationId::new(command);
    hir::validate_stream_record_program(program, Some(&command))?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )?;
    program.permits = crate::project::PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
        .iter()
        .map(|effect| (*effect).to_owned())
        .collect();
    hir::validate(program)
}

pub(in crate::workspace_graph) fn owned_command_program(
    program: &mut hir::ResolvedProgram,
    command: &str,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let command = hir::DeclarationId::new(command);
    hir::validate_stream_owned_program(program, Some(&command))?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )?;
    program.permits = crate::project::PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
        .iter()
        .map(|effect| (*effect).to_owned())
        .collect();
    hir::validate(program)
}

pub(in crate::workspace_graph) fn collection_record_command_program(
    program: &mut hir::ResolvedProgram,
    command: &str,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let command = hir::DeclarationId::new(command);
    hir::validate_stream_collection_record_program(program, Some(&command))?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &command,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )?;
    program.permits = crate::project::PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
        .iter()
        .map(|effect| (*effect).to_owned())
        .collect();
    hir::validate(program)
}

/// Frozen stream profiles refuse new runtime carriers even in unused helpers.
/// Logical schema declarations alone carry no runtime authority.
pub(in crate::workspace_graph) fn reject_unselected_owned_collections(
    profile: crate::project::ProjectProfile,
    modules: &[super::WorkspaceResolvedModule],
) -> Result<(), Vec<crate::diagnostic::Diagnostic>> {
    // Check every retained module before reachability crops unused helpers.
    if profile.is_stdin_stream()
        && profile != crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
    {
        for module in modules {
            for function in &module.functions {
                if hir::owned_collection_record::function_requires_profile_by(function, |id| {
                    modules.iter().flat_map(|provider| &provider.types).find(|ty| &ty.id == id)
                }) {
                    return Err(vec![super::super::project_function_error(
                        module,
                        "nested collection-record runtime requires the explicitly selected language-command-io.collection-record.v1 profile",
                        Some(function.span),
                    )]);
                }
            }
        }
    }
    if !profile.is_stdin_stream()
        || profile == crate::project::ProjectProfile::StdinStreamOwnedDataCommandIoV1
        || profile == crate::project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
    {
        return Ok(());
    }
    for module in modules {
        for function in &module.functions {
            if hir::owned_leaf_collection::function_requires_profile_by(function, |id| {
                modules
                    .iter()
                    .flat_map(|provider| &provider.types)
                    .find(|ty| &ty.id == id)
            }) {
                return Err(vec![super::super::project_function_error(
                    module,
                    "owned collection runtime requires the explicitly selected language-command-io.owned-data.v1 profile",
                    Some(function.span),
                )]);
            }
        }
    }
    Ok(())
}
