//! Additive native stream command result/adapter; the v1 runner stays frozen.
use super::super::COutput;
mod process_adapter;

pub(in crate::codegen) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    output.push_str(
        r#"struct spx_language_command_stream_result_v2 {
    bool semantic_success;
    int64_t application_status;
    uint32_t status_code;
    spx_status_class status_class;
    spx_retryability status_retryability;
    char status_domain[SPX_STATUS_DOMAIN_MAX_BYTES];
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
    uint8_t stderr_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
};

"#,
    );
    writeln!(
        output,
        r#"int spx_language_command_stream_run_v2(
    const struct spx_language_command_input_v1 *input,
    const struct spx_stdin_stream_provider_v1 *provider,
    struct spx_language_command_stream_result_v2 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (provider == NULL || provider->open == NULL || provider->read == NULL ||
        provider->drop == NULL || provider->settle == NULL) return 0;
    if (input == NULL || input->stdin_snapshot.ptr != NULL ||
        input->stdin_snapshot.len != UINT64_C(0) ||
        input->stdin_snapshot.epoch != NULL ||
        input->stdin_snapshot.captured_epoch != UINT64_C(0) ||
        !spx_language_command_input_is_valid_v1(input)) {{
        provider->settle(provider->context);
        return 0;
    }}

    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_stdin_stream_state_v1 state = {{0}};
    state.command.input = input;
    state.provider = *provider;
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
        provider->settle(provider->context);
        memset(&state, 0, sizeof(state));
        return 0;
    }}

    int64_t application_status = INT64_C(0);
    spx_status_token status = {command_symbol}(&spx_ctx, &application_status);
    if (status != SPX_STATUS_SUCCESS) {{
        spx_stdin_stream_settle_v1(&spx_ctx);
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

    if (application_status < INT64_C(0) || application_status > INT64_C(255)) {{
        spx_stdin_stream_settle_v1(&spx_ctx);
        memset(&state, 0, sizeof(state));
        memset(result_out, 0, sizeof(*result_out));
        return 0;
    }}

    if (state.command.output.stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        state.command.output.stderr_length >
            SPX_COMMAND_OUTPUT_CAPACITY_V1 - state.command.output.stdout_length) {{
        spx_stdin_stream_settle_v1(&spx_ctx);
        memset(&state, 0, sizeof(state));
        memset(result_out, 0, sizeof(*result_out));
        return 0;
    }}
    spx_stdin_stream_settle_v1(&spx_ctx);
    result_out->semantic_success = true;
    result_out->application_status = application_status;
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
    .expect("writing native streaming command runner cannot fail");
}

pub(in crate::codegen) fn emit_process_adapter(output: &mut impl COutput) {
    process_adapter::emit_process_adapter(output);
}
