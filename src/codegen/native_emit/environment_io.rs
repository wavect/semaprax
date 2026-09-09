//! Explicit immutable environment snapshots layered over the frozen command context.
use super::{backend_error, COutput, NativeOutputProfile};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram, ResolvedType};

pub fn emit(program: &ResolvedProgram, command_id: &str) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    super::super::reject_native_rust_for_native(program)?;
    check_permits(&program.permits)?;
    let command = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == command_id)
        .ok_or_else(|| backend_error("selected environment command is absent"))?;
    if !command.params.is_empty()
        || command.return_type != ResolvedType::Bool
        || program
            .declarations
            .declaration(&command.id)
            .is_none_or(|d| d.identity_origin != hir::IdentityOrigin::Explicit)
    {
        return Err(backend_error(
            "environment command must be explicit fn() -> bool",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::EnvironmentV1,
    )?;
    super::super::emit_hir_c_with_labels(
        program,
        &std::collections::HashMap::new(),
        NativeOutputProfile::EnvironmentCommandIo,
        Some(&command.id),
    )
}

pub(crate) fn check_permits(permits: &[String]) -> Result<(), Diagnostic> {
    const ADMITTED: &[&str] = &[
        "process.args.read",
        "process.environment.read",
        "process.stderr.write",
        "process.stdin.read",
        "process.stdout.write",
    ];
    if !permits.iter().any(|p| p == crate::environment_ops::EFFECT)
        || permits.iter().any(|p| !ADMITTED.contains(&p.as_str()))
    {
        return Err(backend_error("environment command requires process.environment.read and admits only command input and append output authority"));
    }
    Ok(())
}

pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    if !super::program_uses_byte_data(program) {
        super::native_byte_data::emit_runtime(output);
    }
    super::native_host_output::emit_line_command_runtime(output);
    super::native_command_io::emit_line_runtime(output);
    output.push_str(include_str!("environment_runtime.c"));
}

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"int spx_environment_command_run_v1(
    const struct spx_language_command_input_v1 *input,
    const struct spx_environment_snapshot_v1 *environment,
    struct spx_language_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (!spx_environment_input_is_valid_v1(input, environment)) return 0;

    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_environment_command_state_v1 state = {{0}};
    state.command.input = input;
    state.environment = environment;
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx,
        UINT64_C(1),
        spx_status_entries,
        UINT32_C(1),
        NULL,
        NULL,
        &state
    )) return 0;

    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, &matched);
    if (status != SPX_STATUS_SUCCESS) {{
        const struct spx_normalized_status *failure =
            spx_status_resolve(&spx_ctx, status);
        (void)spx_status_resolve_detail(&spx_ctx, status);
        if (failure == NULL || failure->domain_id == NULL) {{
            memset(&state, 0, sizeof(state));
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        size_t domain_size = 0;
        if (!spx_status_domain_size(failure->domain_id, &domain_size) ||
            domain_size > sizeof(result_out->status_domain)) {{
            memset(&state, 0, sizeof(state));
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        memcpy(result_out->status_domain, failure->domain_id, domain_size);
        result_out->status_code = failure->code;
        result_out->status_class = failure->status_class;
        result_out->status_retryability = failure->retryability;
        memset(&state, 0, sizeof(state));
        return 1;
    }}

    if (state.command.output.stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        state.command.output.stderr_length >
            SPX_COMMAND_OUTPUT_CAPACITY_V1 - state.command.output.stdout_length) {{
        memset(&state, 0, sizeof(state));
        memset(result_out, 0, sizeof(*result_out));
        return 0;
    }}
    result_out->semantic_success = true;
    result_out->matched = matched;
    if (state.command.output.stdout_length != UINT64_C(0)) {{
        memcpy(
            result_out->stdout_bytes,
            state.command.output.stdout_bytes,
            (size_t)state.command.output.stdout_length
        );
    }}
    if (state.command.output.stderr_length != UINT64_C(0)) {{
        memcpy(
            result_out->stderr_bytes,
            state.command.output.stderr_bytes,
            (size_t)state.command.output.stderr_length
        );
    }}
    result_out->stdout_length = state.command.output.stdout_length;
    result_out->stderr_length = state.command.output.stderr_length;
    memset(&state, 0, sizeof(state));
    return 1;
}}
"#
    )
    .expect("writing native language command runner cannot fail");
}
