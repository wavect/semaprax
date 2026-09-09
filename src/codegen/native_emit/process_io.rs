//! Explicit registered process callbacks over the frozen environment/command context.
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
        .ok_or_else(|| backend_error("selected process command is absent"))?;
    if !command.params.is_empty()
        || command.return_type != ResolvedType::Bool
        || program
            .declarations
            .declaration(&command.id)
            .is_none_or(|d| d.identity_origin != hir::IdentityOrigin::Explicit)
    {
        return Err(backend_error(
            "process command must be explicit fn() -> bool",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::ProcessV1,
    )?;
    super::super::emit_hir_c_with_labels(
        program,
        &std::collections::HashMap::new(),
        NativeOutputProfile::ProcessCommandIo,
        Some(&command.id),
    )
}

pub(crate) fn check_permits(permits: &[String]) -> Result<(), Diagnostic> {
    const ADMITTED: &[&str] = &[
        "process.args.read",
        "process.environment.read",
        "process.execute",
        "process.stderr.write",
        "process.stdin.read",
        "process.stdout.write",
    ];
    if !permits.iter().any(|p| p == crate::process_ops::EFFECT)
        || permits.iter().any(|p| !ADMITTED.contains(&p.as_str()))
    {
        return Err(backend_error("process command requires process.execute and admits only environment, command input and append output authority"));
    }
    Ok(())
}

pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    super::environment_io::emit_runtime(output, program);
    use crate::process_ops as ops;
    for (name, value) in [
        ("MAX_ARGUMENTS", ops::MAX_ARGUMENTS),
        ("MAX_INPUT_BYTES", ops::MAX_INPUT_BYTES),
        ("MAX_OUTPUT_BYTES", ops::MAX_OUTPUT_BYTES),
        ("HEADER_BYTES", ops::HEADER_BYTES),
        ("MAX_WAIT_MILLIS", ops::MAX_WAIT_MILLIS),
        ("MAX_OPERATIONS", ops::MAX_OPERATIONS),
        ("MAX_TOTAL_BYTES", ops::MAX_TOTAL_BYTES),
    ] {
        writeln!(output, "#define SPX_PROCESS_{name}_V1 UINT64_C({value})")
            .expect("write process constants");
    }
    output.push_str(include_str!("process_runtime.c"));
}

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"int spx_process_command_run_v1(
    const struct spx_language_command_input_v1 *input,
    const struct spx_environment_snapshot_v1 *environment,
    const struct spx_process_callbacks_v1 *callbacks,
    struct spx_language_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (!spx_environment_input_is_valid_v1(input, environment)) return 0;

    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_process_command_state_v1 state = {{0}};
    state.environment.command.input = input;
    state.environment.environment = environment;
    state.callbacks = callbacks;
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx,
        UINT64_C(1),
        spx_status_entries,
        UINT32_C(1),
        NULL,
        NULL,
        &state
    )) {{
        if (callbacks != NULL && callbacks->settle != NULL) {{
            uint32_t code = callbacks->settle(callbacks->context);
            if (code != 0 && code != 7) spx_runtime_invariant_failure("process settlement status outside closed table");
        }}
        return 0;
    }}

    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, &matched);
    if (callbacks != NULL && callbacks->settle != NULL) {{
        uint32_t code = callbacks->settle(callbacks->context);
        if (code != 0 && code != 7) spx_runtime_invariant_failure("process settlement status outside closed table");
        if (code == 7 && status == SPX_STATUS_SUCCESS) status = spx_process_status_v1(&spx_ctx, code);
    }}
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

    if (state.environment.command.output.stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        state.environment.command.output.stderr_length >
            SPX_COMMAND_OUTPUT_CAPACITY_V1 - state.environment.command.output.stdout_length) {{
        memset(&state, 0, sizeof(state));
        memset(result_out, 0, sizeof(*result_out));
        return 0;
    }}
    result_out->semantic_success = true;
    result_out->matched = matched;
    if (state.environment.command.output.stdout_length != UINT64_C(0)) {{
        memcpy(
            result_out->stdout_bytes,
            state.environment.command.output.stdout_bytes,
            (size_t)state.environment.command.output.stdout_length
        );
    }}
    if (state.environment.command.output.stderr_length != UINT64_C(0)) {{
        memcpy(
            result_out->stderr_bytes,
            state.environment.command.output.stderr_bytes,
            (size_t)state.environment.command.output.stderr_length
        );
    }}
    result_out->stdout_length = state.environment.command.output.stdout_length;
    result_out->stderr_length = state.environment.command.output.stderr_length;
    memset(&state, 0, sizeof(state));
    return 1;
}}
"#
    )
    .expect("writing native language command runner cannot fail");
}
