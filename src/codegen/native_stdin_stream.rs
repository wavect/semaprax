//! Standalone native provider foundation for bounded streaming command stdin.
//!
//! The emitted runtime has no process descriptor or filesystem access. An
//! invocation supplies a synchronous provider table through its target-state
//! carrier; the separately emitted process adapter binds permitted stdin.
//! Profile routing and the compiler's `StdinReader` value lowering are
//! integrated separately.

use super::{backend_error, COutput, NativeOutputProfile};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram, ResolvedType};
use std::collections::HashMap;
pub(super) mod exit_status;

pub(super) fn emit_runtime(output: &mut impl COutput) {
    output.push_str(STDIN_STREAM_RUNTIME_C);
}

/// Keep the command helpers warning-clean when a stream root uses only input.
pub(super) fn emit_command_helper_table(output: &mut impl COutput) {
    output.push_str(
        r#"static __attribute__((unused)) void spx_stream_command_table_v1(void) {
    (void)&spx_host_command_stdout_write_v1;
    (void)&spx_host_command_stderr_write_v1;
    (void)&spx_host_args_len_v1;
    (void)&spx_host_arg_utf8_v1;
    (void)&spx_host_stdin_read_v1;
    (void)&spx_host_stdin_stream_open_v1;
    (void)&spx_host_stdin_stream_next_v1;
    (void)&spx_stdin_stream_move_v1;
    (void)&spx_stdin_stream_chunk_v1;
    (void)&spx_stdin_stream_eof_v1;
    (void)&spx_stdin_stream_drop_v1;
}

"#,
    );
}

/// Emit the explicitly selected native command profile for bounded stdin
/// streaming. The public command-I/O runtime remains the source of argv and
/// output declarations; this profile adds only the streaming reader route.
pub fn emit_hir_c_with_stdin_stream(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    emit_profile(
        program,
        command_id,
        NativeOutputProfile::StdinStreamCommandIo,
    )
}

pub fn emit_hir_c_with_stdin_stream_exit_status(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    emit_profile(
        program,
        command_id,
        NativeOutputProfile::StdinStreamExitCommandIo,
    )
}

/// Project v25: private owned String helpers and length-delimited text operations.
pub fn emit_hir_c_with_stdin_stream_text(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    emit_profile(
        program,
        command_id,
        NativeOutputProfile::StdinStreamTextCommandIo,
    )
}

/// Project v27: v25 runtime plus private borrowed `Vec<Copy scalar>` helpers.
pub fn emit_hir_c_with_stdin_stream_data(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    emit_profile(
        program,
        command_id,
        NativeOutputProfile::StdinStreamDataCommandIo,
    )
}

/// Project v29 authenticates Copy-record closure before using the unchanged stream runtime.
pub fn emit_hir_c_with_stdin_stream_records(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    crate::hir::validate_stream_record_program(
        program,
        Some(&hir::DeclarationId::new(command_id)),
    )?;
    emit_profile(
        program,
        command_id,
        NativeOutputProfile::StdinStreamDataCommandIo,
    )
}

/// Project v30 authenticates the complete owned-leaf runtime closure.
pub fn emit_hir_c_with_stdin_stream_owned_data(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    crate::hir::validate_stream_owned_program(program, Some(&hir::DeclarationId::new(command_id)))?;
    emit_profile(
        program,
        command_id,
        NativeOutputProfile::StdinStreamDataCommandIo,
    )
}

fn emit_profile(
    program: &ResolvedProgram,
    command_id: &str,
    output_profile: NativeOutputProfile,
) -> Result<String, Diagnostic> {
    let exit_status = output_profile != NativeOutputProfile::StdinStreamCommandIo;
    hir::validate(program)?;
    super::reject_native_rust_for_native(program)?;
    let required_permits = [
        crate::command_io_ops::ARGS_READ_EFFECT,
        crate::command_io_ops::STDERR_WRITE_EFFECT,
        crate::command_io_ops::STDIN_READ_EFFECT,
        crate::host_io_ops::STDOUT_WRITE_EFFECT,
    ];
    if program.permits.as_slice() != required_permits {
        return Err(backend_error(
            "stdin stream command requires the exact canonical command-I/O permit inventory",
        ));
    }
    let command = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| {
            backend_error(format!(
                "selected stdin stream command `{command_id}` is absent"
            ))
        })?;
    if program
        .declarations
        .declaration(&command.id)
        .is_none_or(|declaration| declaration.identity_origin != hir::IdentityOrigin::Explicit)
        || !command.params.is_empty()
        || command.return_type
            != if exit_status {
                ResolvedType::I64
            } else {
                ResolvedType::Bool
            }
    {
        return Err(backend_error(if exit_status {
            "selected stdin stream exit command must be an explicit stable-ID `fn () -> i64`"
        } else {
            "selected stdin stream command must be an explicit stable-ID `fn () -> bool`"
        }));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
    )?;
    super::emit_hir_c_with_labels(program, &HashMap::new(), output_profile, Some(&command.id))
}

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"int spx_language_command_stream_run_v1(
    const struct spx_language_command_input_v1 *input,
    const struct spx_stdin_stream_provider_v1 *provider,
    struct spx_language_command_result_v1 *result_out
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

    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, &matched);
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
    .expect("writing native streaming command runner cannot fail");
}

pub(super) fn emit_process_adapter(output: &mut impl COutput) {
    process_adapter::emit_process_adapter(output);
}

const STDIN_STREAM_RUNTIME_C: &str = r#"#define SPX_STDIN_STREAM_CHUNK_CAPACITY_V1 UINT32_C(4096)

/* Provider tokens are opaque values. The runtime authenticates its own reader
   identity against target_state and never dereferences either token. */
struct spx_stdin_stream_provider_v1 {
    void *context;
    /* open returns only 0 (with a nonzero token) or 3 (with token still zero). */
    uint32_t (*open)(void *context, uintptr_t *provider_token_out);
    /* read returns only 0 with one of the two closed length/EOF tuples, or 3
       with both output slots untouched. A positive short read is non-EOF. */
    uint32_t (*read)(void *context, uintptr_t provider_token,
                     uint8_t *buffer, uint32_t capacity,
                     uint32_t *length_out, uint32_t *eof_out);
    /* Cleanup cannot replace the selected command status. */
    void (*drop)(void *context, uintptr_t provider_token);
    void (*settle)(void *context);
};

/* The command output prefix is intentional. Existing output helpers
   authenticate target_state through that first member. */
struct spx_stdin_stream_state_v1 {
    struct spx_language_command_state_v1 command;
    struct spx_stdin_stream_provider_v1 provider;
    uintptr_t provider_token;
    uint64_t chunk_generation;
    uint32_t chunk_length;
    uint8_t chunk_bytes[SPX_STDIN_STREAM_CHUNK_CAPACITY_V1];
    bool open_attempted;
    bool provider_token_live;
    bool reader_live;
    bool eof;
    bool in_callback;
    bool poisoned;
    bool read_failed;
    bool settled;
};

_Static_assert(
    offsetof(struct spx_stdin_stream_state_v1, command) == 0,
    "stdin stream command state must remain the target-state prefix"
);

_Static_assert(
    offsetof(struct spx_language_command_state_v1, output) == 0,
    "command output staging must remain the target-state prefix"
);

static struct spx_stdin_stream_state_v1 *spx_stdin_stream_state_v1(
    struct spx_context *spx_ctx
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("stdin stream invocation state is unavailable");
    }
    return (struct spx_stdin_stream_state_v1 *)spx_ctx->target_state;
}

static void spx_stdin_stream_require_operable_v1(
    struct spx_stdin_stream_state_v1 *state
) {
    if (state->poisoned || state->in_callback || state->settled) {
        spx_runtime_invariant_failure("stdin stream invocation is poisoned or settled");
    }
}

static bool spx_stdin_stream_provider_valid_v1(
    const struct spx_stdin_stream_provider_v1 *provider
) {
    return provider->open != NULL && provider->read != NULL &&
        provider->drop != NULL && provider->settle != NULL;
}

static void spx_stdin_stream_advance_generation_v1(
    struct spx_stdin_stream_state_v1 *state
) {
    state->chunk_generation += UINT64_C(1);
    if (state->chunk_generation == UINT64_C(0)) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream chunk generation overflow");
    }
}

static uint32_t spx_stdin_stream_read_v1(
    struct spx_stdin_stream_state_v1 *state
) {
    uint32_t length = UINT32_MAX;
    uint32_t eof = UINT32_MAX;
    spx_stdin_stream_advance_generation_v1(state);
    memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
    state->chunk_length = UINT32_C(0);
    state->eof = false;
    state->in_callback = true;
    uint32_t outcome = state->provider.read(
        state->provider.context,
        state->provider_token,
        state->chunk_bytes,
        SPX_STDIN_STREAM_CHUNK_CAPACITY_V1,
        &length,
        &eof
    );
    state->in_callback = false;
    if (outcome == UINT32_C(3)) {
        if (length != UINT32_MAX || eof != UINT32_MAX) {
            state->poisoned = true;
            spx_runtime_invariant_failure("stdin stream failed read published a result");
        }
        state->read_failed = true;
        memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
        return UINT32_C(3);
    }
    if (outcome != UINT32_C(0) || length == UINT32_MAX ||
        eof > UINT32_C(1) || length > SPX_STDIN_STREAM_CHUNK_CAPACITY_V1 ||
        (length == UINT32_C(0) && eof != UINT32_C(1)) ||
        (length != UINT32_C(0) && eof != UINT32_C(0))) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream provider violated its read protocol");
    }
    if (length == UINT32_C(0)) {
        state->eof = true;
        return UINT32_C(0);
    }
    state->chunk_length = length;
    return UINT32_C(0);
}

static spx_status_token spx_host_stdin_stream_open_v1(
    struct spx_context *spx_ctx,
    uintptr_t *reader_out
) {
    if (reader_out == NULL) {
        spx_runtime_invariant_failure("stdin stream open result slot is unavailable");
    }
    *reader_out = (uintptr_t)0;
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (state->open_attempted || state->provider_token_live || state->reader_live ||
        !spx_stdin_stream_provider_valid_v1(&state->provider)) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream open is unavailable or repeated");
    }
    state->open_attempted = true;
    uintptr_t provider_token = (uintptr_t)0;
    state->in_callback = true;
    uint32_t outcome = state->provider.open(state->provider.context, &provider_token);
    state->in_callback = false;
    if (outcome == UINT32_C(3)) {
        if (provider_token != (uintptr_t)0) {
            state->poisoned = true;
            spx_runtime_invariant_failure("stdin stream failed open published a token");
        }
        return spx_command_input_status_v1(spx_ctx, UINT32_C(3));
    }
    if (outcome != UINT32_C(0) || provider_token == (uintptr_t)0) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream provider violated its open protocol");
    }
    state->provider_token = provider_token;
    state->provider_token_live = true;
    if (spx_stdin_stream_read_v1(state) != UINT32_C(0)) {
        return spx_command_input_status_v1(spx_ctx, UINT32_C(3));
    }
    state->reader_live = true;
    *reader_out = (uintptr_t)state;
    return SPX_STATUS_SUCCESS;
}

static spx_status_token spx_host_stdin_stream_next_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader,
    uintptr_t *reader_out
) {
    if (reader_out == NULL) {
        spx_runtime_invariant_failure("stdin stream next result slot is unavailable");
    }
    *reader_out = (uintptr_t)0;
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream reader is foreign or not live");
    }
    if (state->read_failed) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream reader is failed");
    }
    if (!state->eof && spx_stdin_stream_read_v1(state) != UINT32_C(0)) {
        return spx_command_input_status_v1(spx_ctx, UINT32_C(3));
    }
    *reader_out = reader;
    return SPX_STATUS_SUCCESS;
}

static uintptr_t spx_stdin_stream_move_v1(
    struct spx_context *spx_ctx,
    uintptr_t *source
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (source == NULL || *source == (uintptr_t)0 ||
        *source != (uintptr_t)state || !state->reader_live ||
        !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream owned move source is invalid");
    }
    if (state->read_failed) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream reader cannot move after read failure");
    }
    uintptr_t reader = *source;
    *source = (uintptr_t)0;
    return reader;
}

static spx_slice_u8_v1 spx_stdin_stream_chunk_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream chunk reader is foreign or not live");
    }
    if (state->read_failed) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream chunk is unavailable after read failure");
    }
    if (state->chunk_length == UINT32_C(0)) {
        return (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0),
            .epoch = &state->chunk_generation, .captured_epoch = state->chunk_generation };
    }
    return (spx_slice_u8_v1){
        .ptr = state->chunk_bytes,
        .len = (uint64_t)state->chunk_length,
        .epoch = &state->chunk_generation,
        .captured_epoch = state->chunk_generation
    };
}

static bool spx_stdin_stream_eof_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream EOF reader is foreign or not live");
    }
    if (state->read_failed) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream EOF is unavailable after read failure");
    }
    return state->eof;
}

static void spx_stdin_stream_drop_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream drop reader is foreign or not live");
    }
    uintptr_t provider_token = state->provider_token;
    spx_stdin_stream_advance_generation_v1(state);
    state->reader_live = false;
    state->provider_token_live = false;
    state->provider_token = (uintptr_t)0;
    state->chunk_length = UINT32_C(0);
    state->eof = false;
    state->read_failed = false;
    memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
    state->in_callback = true;
    state->provider.drop(state->provider.context, provider_token);
    state->in_callback = false;
}

static void spx_stdin_stream_settle_v1(struct spx_context *spx_ctx) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    if (state->settled || state->in_callback) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream settlement is repeated or reentrant");
    }
    if (!spx_stdin_stream_provider_valid_v1(&state->provider)) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream settlement provider is unavailable");
    }
    state->settled = true;
    spx_stdin_stream_advance_generation_v1(state);
    state->reader_live = false;
    state->chunk_length = UINT32_C(0);
    state->eof = false;
    state->read_failed = false;
    memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
    if (state->provider_token_live) {
        uintptr_t provider_token = state->provider_token;
        state->provider_token_live = false;
        state->provider_token = (uintptr_t)0;
        state->in_callback = true;
        state->provider.drop(state->provider.context, provider_token);
        state->in_callback = false;
    }
    state->in_callback = true;
    state->provider.settle(state->provider.context);
    state->in_callback = false;
}
"#;

#[cfg(test)]
mod tests;

mod process_adapter;
