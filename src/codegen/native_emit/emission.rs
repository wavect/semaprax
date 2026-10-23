//! Ordinary native emission, plus a sealed selected closure view.
use super::*;
/// Ordinary prelude only, for the private selected fused-row unit. Full HIR
/// validation runs first; no function, entry wrapper or public carrier is emitted.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private fused-row receiving entry has no production caller"
    )
)]
pub(in crate::codegen) fn emit_selected_fused_row_prelude(
    program: &ResolvedProgram,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    let resource_abi = native_resource::build_resource_abi(program)?;
    if !resource_abi.resources.is_empty() {
        return Err(resource_lowering_gate());
    }
    let mut output = crate::bounded_output::CappedString::new();
    emit_native_prelude_profile(
        &mut output,
        &resource_abi,
        program,
        NativeOutputProfile::Legacy.string_runtime(),
    );
    Ok(output.into_string())
}
/// Concrete selected owner receiving; complete selected validation precedes
/// the same ordinary resource/prelude generation, without a Legacy downgrade.
pub(in crate::codegen) fn emit_validated_fused_row_prelude(
    owner: &hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
) -> Result<String, Diagnostic> {
    // The owner-accepting constructor runs the complete current selected
    // revalidation; ordinary validation would refuse the joined loop.
    let resource_abi = native_resource::build_selected_resource_abi(owner)?;
    let program = owner.program();
    if !resource_abi.resources.is_empty() {
        return Err(resource_lowering_gate());
    }
    let mut output = crate::bounded_output::CappedString::new();
    emit_native_prelude_profile(
        &mut output,
        &resource_abi,
        program,
        NativeOutputProfile::Legacy.string_runtime(),
    );
    Ok(output.into_string())
}
pub(super) fn emit_hir_c_with_options(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
    output_profile: NativeOutputProfile,
    selected_command: Option<&DeclarationId>,
    semantic: Option<&NativeSemanticMetering>,
    selected: Option<&super::super::selected_final_utf8::Selection>,
) -> Result<String, Diagnostic> {
    emit_hir_c_with_receiving(
        program,
        contract_labels,
        output_profile,
        selected_command,
        semantic,
        selected,
        None,
        None,
    )
}

pub(in crate::codegen) fn emit_partition_source(
    projection: &super::super::native_byte_data::native_shared_bytes::source::Projection<'_, '_>,
) -> Result<String, Diagnostic> {
    emit_hir_c_with_receiving(
        projection.owner().program(),
        &HashMap::new(),
        NativeOutputProfile::SelectedPartition,
        None,
        None,
        None,
        Some(projection),
        None,
    )
}

pub(super) fn emit_checked_allocation_source(
    projection: &checked_allocation::Projection<'_, '_>,
) -> Result<String, Diagnostic> {
    emit_hir_c_with_receiving(
        projection.owner().program(),
        &HashMap::new(),
        NativeOutputProfile::CheckedAllocation,
        None,
        None,
        None,
        None,
        Some(projection),
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_hir_c_with_receiving(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
    output_profile: NativeOutputProfile,
    selected_command: Option<&DeclarationId>,
    semantic: Option<&NativeSemanticMetering>,
    selected: Option<&super::super::selected_final_utf8::Selection>,
    partition: Option<
        &super::super::native_byte_data::native_shared_bytes::source::Projection<'_, '_>,
    >,
    allocation: Option<&checked_allocation::Projection<'_, '_>>,
) -> Result<String, Diagnostic> {
    emit_hir_c_with_validation(
        program,
        contract_labels,
        output_profile,
        selected_command,
        semantic,
        selected,
        partition,
        allocation,
        None,
    )
}

pub(in crate::codegen) fn emit_validated_owned_provider(
    validated: native_resource::OrdinaryValidatedProgram<'_>,
    owned_utf8: bool,
) -> Result<String, Diagnostic> {
    let program = validated.program();
    super::super::reject_native_rust_for_native(program)?;
    emit_hir_c_with_validation(
        program,
        &HashMap::new(),
        if owned_utf8 {
            NativeOutputProfile::OwnedUtf8Provider
        } else {
            NativeOutputProfile::OwnedDataProvider
        },
        None,
        None,
        None,
        None,
        None,
        Some(validated),
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_hir_c_with_validation(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
    output_profile: NativeOutputProfile,
    selected_command: Option<&DeclarationId>,
    semantic: Option<&NativeSemanticMetering>,
    selected: Option<&super::super::selected_final_utf8::Selection>,
    partition: Option<
        &super::super::native_byte_data::native_shared_bytes::source::Projection<'_, '_>,
    >,
    allocation: Option<&checked_allocation::Projection<'_, '_>>,
    inherited: Option<native_resource::OrdinaryValidatedProgram<'_>>,
) -> Result<String, Diagnostic> {
    if inherited.as_ref().is_some_and(|owner| {
        !std::ptr::eq(program, owner.program())
            || !matches!(
                output_profile,
                NativeOutputProfile::OwnedDataProvider | NativeOutputProfile::OwnedUtf8Provider
            )
            || selected_command.is_some()
            || semantic.is_some()
            || selected.is_some()
            || partition.is_some()
            || allocation.is_some()
    }) {
        return Err(backend_error(
            "ordinary provider validation subject or profile is not current",
        ));
    }
    let ordinary_validation;
    if let Some(projection) = allocation {
        if partition.is_some()
            || selected.is_some()
            || output_profile != NativeOutputProfile::CheckedAllocation
            || !std::ptr::eq(program, projection.owner().program())
        {
            return Err(backend_error(
                "checked allocation native subject is not current",
            ));
        }
        projection.revalidate()?;
        ordinary_validation = None;
    } else if let Some(projection) = partition {
        if output_profile != NativeOutputProfile::SelectedPartition
            || !std::ptr::eq(program, projection.owner().program())
        {
            return Err(backend_error(
                "partition native receiving subject is not current",
            ));
        }
        projection.owner().revalidate_compiler()?;
        ordinary_validation = None;
    } else {
        if matches!(
            output_profile,
            NativeOutputProfile::SelectedPartition | NativeOutputProfile::CheckedAllocation
        ) {
            return Err(backend_error(
                "native emission requires its exact sealed source projection",
            ));
        }
        ordinary_validation = Some(match inherited {
            Some(owner) => owner,
            None => native_resource::OrdinaryValidatedProgram::validate(program)?,
        });
    }
    // This remains the authoritative complete program. The selected view
    // intentionally omits ordinary main and cannot pass public HIR validation.
    if (output_profile == NativeOutputProfile::SelectedFinalUtf8) != selected.is_some() {
        return Err(backend_error(
            "selected native profile requires an independently derived certificate",
        ));
    }
    let view;
    let program = if let Some(selection) = selected {
        view = selection.emission_view(program);
        &view
    } else {
        program
    };
    if !output_profile.is_stdin_stream() && crate::stdin_stream_ops::resolved_program_uses(program)
    {
        return Err(backend_error(
            "stdin reader requires the explicit native streaming-command profile",
        ));
    }
    if program.types.iter().any(|declaration| {
        matches!(
            declaration.kind,
            ResolvedTypeDeclarationKind::Resource { .. }
        )
    }) {
        return Err(resource_lowering_gate());
    }
    let resource_abi = if let Some(projection) = allocation {
        native_resource::build_checked_allocation_resource_abi(projection.owner())?
    } else if let Some(projection) = partition {
        native_resource::build_partition_resource_abi(projection.owner())?
    } else {
        native_resource::build_ordinary_validated_resource_abi(
            ordinary_validation.expect("ordinary receiving validated the authoritative program"),
        )?
    };
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Native64)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Native64)?;
    let functions = function_index(program)?;
    debug_assert!(resource_abi.resources.is_empty());
    let mut output = crate::bounded_output::CappedString::new();
    // Feature-test macros must precede the first system include. The
    // `file_read_text` runtime needs `openat` and `O_NOFOLLOW`, and every
    // command entry opens its file root with `O_DIRECTORY | O_CLOEXEC`;
    // glibc declares these under strict C11 only with POSIX.1-2008 visibility.
    if output_profile.is_command()
        || output_profile == NativeOutputProfile::SourceCommand
        || output_profile == NativeOutputProfile::SourceSelectedFileCommand
        || output_profile == NativeOutputProfile::SourceSelectedFileResourceCommand
        || output_profile == NativeOutputProfile::SourceResourceCommand
        || crate::string_ops::program_uses_op(program, crate::string_ops::StringOp::FileReadText)
    {
        network_io::emit_feature_macros(&mut output);
    }
    if output_profile.is_command() {
        emit_native_prelude_without_public_failure(
            &mut output,
            &resource_abi,
            program,
            output_profile.is_language_command(),
            output_profile.string_runtime(),
        );
    } else {
        emit_native_prelude_profile(
            &mut output,
            &resource_abi,
            program,
            output_profile.string_runtime(),
        );
    }
    if allocation.is_some() {
        let joined = checked_allocation::join_runtime(output.into_string())?;
        output = crate::bounded_output::CappedString::new();
        output.push_str(&joined);
    }
    if let Some(projection) = partition {
        let extended = super::super::native_byte_data::native_shared_bytes::extend_context(
            output.into_string(),
        );
        output = crate::bounded_output::CappedString::new();
        output.push_str(&extended);
        projection.emit_bindings(&mut output);
    }
    if output_profile == NativeOutputProfile::NetworkCommandIo {
        network_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::FilesystemCommandIo {
        filesystem_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::FilesystemCommandIoV2 {
        filesystem_io_v2::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::FilesystemCommandIoV3 {
        filesystem_io_v3::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::HttpsCommandIo {
        http_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::ProcessCommandIo {
        process_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::EnvironmentCommandIo {
        environment_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::LineCommandIo {
        native_host_output::emit_line_command_runtime(&mut output);
        native_command_io::emit_line_runtime(&mut output);
    } else if output_profile.is_stdin_stream() {
        native_host_output::emit_language_command_runtime(&mut output);
        native_command_io::emit_runtime(&mut output);
        crate::codegen::native_stdin_stream::emit_runtime(&mut output);
        crate::codegen::native_stdin_stream::emit_command_helper_table(&mut output);
    } else if output_profile == NativeOutputProfile::LanguageCommandIo {
        native_host_output::emit_language_command_runtime(&mut output);
        native_command_io::emit_runtime(&mut output);
    } else if output_profile == NativeOutputProfile::SourceSelectedFileResourceCommand {
        crate::codegen::native_selected_file_command::emit_resource_runtime(&mut output);
    } else if output_profile == NativeOutputProfile::SourceSelectedFileCommand {
        crate::codegen::native_selected_file_command::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::SourceCommand {
        native_source_command::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::SourceResourceCommand {
        native_source_resource_command::emit_runtime(&mut output, program);
    } else if output_profile.supports_stdout_transcript() {
        native_host_output::emit_runtime(&mut output);
    }
    if let Some(metering) = semantic {
        semantic_work::emit_runtime(&mut output, metering);
    }
    emit_fixed_byte_array_declarations(&mut output, program)?;
    emit_aggregate_declarations(
        &mut output,
        program,
        &resource_abi,
        &record_layouts,
        &variant_layouts,
    )?;
    if hir::closure::requires_runtime_closures(program) {
        closure::emit_carrier_declarations(&mut output, program, &resource_abi)?;
    }
    function_value::emit_typedefs(&mut output, program, &resource_abi)?;
    emit_function_prototypes(&mut output, program, &functions, &resource_abi)?;
    if closure::enabled(program) {
        closure::emit_thunk_prototypes(&mut output, program, &resource_abi)?;
    }
    let emission = NativeEmissionContext {
        program,
        resource_abi: &resource_abi,
        functions: &functions,
        contract_labels,
        record_layouts: &record_layouts,
        variant_layouts: &variant_layouts,
        output_profile,
        semantic,
        owned_str_views: !output_profile.string_runtime().resource_strings
            && string_views::program_uses_string_as_str(
                program,
                output_profile.string_runtime().include_instances,
            ),
    };
    for function in &program.functions {
        let selected_root = selected.is_some_and(|s| s.entry == function.id);
        let function_emission = NativeEmissionContext {
            output_profile: if selected.is_some() && !selected_root {
                NativeOutputProfile::Legacy
            } else {
                output_profile
            },
            ..emission
        };
        if selected_root {
            output.push_str("#define spx_string_drop spx_final_utf8_owned_drop\n");
        }
        emit_function(
            &mut output,
            function,
            &FunctionExecutionId::Monomorphic(function.id.clone()),
            &function_emission,
        )?;
        if selected_root {
            output.push_str("#undef spx_string_drop\n");
        }
    }
    for instance in &program.function_instances {
        emit_function(
            &mut output,
            &instance.function,
            &FunctionExecutionId::Generic(instance.id.clone()),
            &emission,
        )?;
    }
    if closure::enabled(program) {
        for function in closure::closure_functions(program)? {
            emit_function(
                &mut output,
                &function,
                &FunctionExecutionId::Monomorphic(function.id.clone()),
                &emission,
            )?;
        }
        closure::emit_thunks(&mut output, program, &emission)?;
    }
    if let Some(projection) = allocation {
        projection.emit_entry(&mut output, &functions, &resource_abi)?;
        return Ok(output.into_string());
    }
    if let Some(selection) = selected {
        super::super::selected_final_utf8::emit_adapter(
            &mut output,
            program,
            selection,
            emission.record_layouts,
            emission.variant_layouts,
        )?;
        return Ok(output.into_string());
    }
    if output_profile.is_command() {
        let command = selected_command
            .ok_or_else(|| backend_error("native command selection is unavailable"))?;
        let symbol = &functions
            .get(&FunctionExecutionId::Monomorphic(command.clone()))
            .ok_or_else(|| backend_error("selected native command is not indexed"))?
            .symbol;
        if output_profile == NativeOutputProfile::NetworkCommandIo {
            network_io::emit_runner(&mut output, symbol);
            native_command_io::emit_process_adapter(&mut output);
        } else if output_profile == NativeOutputProfile::FilesystemCommandIo {
            filesystem_io::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::FilesystemCommandIoV2 {
            filesystem_io_v2::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::FilesystemCommandIoV3 {
            filesystem_io_v3::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::HttpsCommandIo {
            http_io::emit_runner(&mut output, symbol);
            native_command_io::emit_process_adapter(&mut output);
        } else if output_profile == NativeOutputProfile::ProcessCommandIo {
            process_io::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::EnvironmentCommandIo {
            environment_io::emit_runner(&mut output, symbol);
        } else if output_profile.is_stdin_stream() {
            if matches!(
                output_profile,
                NativeOutputProfile::StdinStreamExitCommandIo
                    | NativeOutputProfile::StdinStreamTextCommandIo
                    | NativeOutputProfile::StdinStreamDataCommandIo
            ) {
                crate::codegen::native_stdin_stream::exit_status::emit_runner(&mut output, symbol);
                crate::codegen::native_stdin_stream::exit_status::emit_process_adapter(&mut output);
            } else {
                crate::codegen::native_stdin_stream::emit_runner(&mut output, symbol);
                crate::codegen::native_stdin_stream::emit_process_adapter(&mut output);
            }
        } else if output_profile.is_language_command() {
            native_command_io::emit_runner(&mut output, symbol);
            native_command_io::emit_process_adapter(&mut output);
        } else {
            native_command::emit_runner(&mut output, symbol);
            native_command::emit_process_adapter(&mut output);
        }
    } else {
        let main = program
            .functions
            .iter()
            .find(|function| function.id == program.entrypoint)
            .ok_or_else(|| backend_error("resolved native entry point is not indexed"))?;
        if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
            return Err(backend_error(
                "resolved native entry point must have type `fn main() -> i64`",
            ));
        }
        let symbol = &functions
            .get(&FunctionExecutionId::Monomorphic(main.id.clone()))
            .ok_or_else(|| backend_error("native entry point is not indexed"))?
            .symbol;
        if let Some(projection) = partition {
            projection.emit_entry(&mut output, symbol);
            return Ok(output.into_string());
        }
        if output_profile == NativeOutputProfile::StdoutTranscript {
            native_host_output::emit_root_wrapper(&mut output, symbol);
            return Ok(output.into_string());
        }
        if output_profile == NativeOutputProfile::SourceSelectedFileResourceCommand {
            crate::codegen::native_selected_file_command::emit_resource_process_adapter(
                &mut output,
                symbol,
            );
            return Ok(output.into_string());
        }
        if output_profile == NativeOutputProfile::SourceSelectedFileCommand {
            crate::codegen::native_selected_file_command::emit_process_adapter(&mut output, symbol);
            return Ok(output.into_string());
        }
        if output_profile == NativeOutputProfile::SourceCommand {
            native_source_command::emit_process_adapter(&mut output, symbol);
            return Ok(output.into_string());
        }
        if output_profile == NativeOutputProfile::SourceResourceCommand {
            native_source_resource_command::emit_process_adapter(&mut output, symbol, program);
            return Ok(output.into_string());
        }
        write!(
            output,
        "#ifndef SPX_NO_ENTRY_WRAPPER\n\
         #if defined(_WIN32)\n\
         #include <fcntl.h>\n\
         #include <io.h>\n\
         #endif\n\
         int main(void) {{\n\
             struct spx_status_entry spx_status_entries[UINT32_C(1)];\n\
             struct spx_context spx_ctx = {{0}};\n\
             if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: context initialization\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             int64_t result;\n\
             spx_status_token status = {symbol}(&spx_ctx, &result);\n\
             if (status != SPX_STATUS_SUCCESS) return spx_public_failure(&spx_ctx, status);\n\
             #if defined(_WIN32)\n\
             if (_setmode(_fileno(stdout), _O_BINARY) == -1) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: stdout binary mode\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             #endif\n\
             printf(\"%lld\\n\", (long long)result);\n\
             return 0;\n\
         }}\n\
         #endif\n"
        )
        .expect("writing to a string cannot fail");
    }
    Ok(output.into_string())
}

#[cfg(test)]
#[path = "emission_before.rs"]
mod before;
#[cfg(test)]
#[path = "validation_reuse_tests.rs"]
mod validation_reuse_tests;
