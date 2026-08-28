//! Explicit injected native command I/O for Bounded Language Command I/O v1.
//!
//! Generated SEMAPRAX functions see only the invocation context. The process
//! adapter snapshots argv and stdin completely before starting the selected
//! root and performs physical writes only after semantic settlement.

use super::COutput;

pub(super) const RUN_SYMBOL: &str = "spx_language_command_run_v1";

pub(super) fn emit_runtime(output: &mut impl COutput) {
    output.push_str(
        r#"#define SPX_COMMAND_ARGUMENT_LIMIT_V1 UINT32_C(16)
#define SPX_COMMAND_INPUT_CAPACITY_V1 UINT64_C(65536)
#define SPX_COMMAND_INPUT_STATUS_DOMAIN_V1 "semaprax.command-input.v1"
#define SPX_COMMAND_INPUT_ARGUMENT_RANGE_V1 UINT32_C(1)
#define SPX_COMMAND_INPUT_INVALID_UTF8_V1 UINT32_C(2)
#define SPX_COMMAND_INPUT_STDIN_FAILURE_V1 UINT32_C(3)
#define SPX_COMMAND_INPUT_CAPACITY_FAILURE_V1 UINT32_C(4)

struct spx_language_command_input_v1 {
    uint32_t argument_count;
    spx_str_v1 arguments[SPX_COMMAND_ARGUMENT_LIMIT_V1];
    spx_slice_u8_v1 stdin_snapshot;
};

struct spx_language_command_state_v1 {
    /* First-member layout is intentional: the two-channel output helpers
       authenticate target_state through this exact prefix. */
    struct spx_command_output_staging_v1 output;
    const struct spx_language_command_input_v1 *input;
    bool stdin_consumed;
};

_Static_assert(
    offsetof(struct spx_language_command_state_v1, output) == 0,
    "command output staging must be the target-state prefix"
);

struct spx_language_command_result_v1 {
    bool semantic_success;
    bool matched;
    uint32_t status_code;
    spx_status_class status_class;
    spx_retryability status_retryability;
    char status_domain[SPX_STATUS_DOMAIN_MAX_BYTES];
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
    uint8_t stderr_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
};

static bool spx_command_utf8_v1(const uint8_t *bytes, uint64_t length) {
    if (length > SPX_COMMAND_INPUT_CAPACITY_V1 ||
        (length == UINT64_C(0) ? bytes != NULL : bytes == NULL)) {
        return false;
    }
    uint64_t offset = UINT64_C(0);
    while (offset < length) {
        uint8_t first = bytes[offset];
        uint64_t width;
        if (first <= UINT8_C(0x7f)) {
            width = UINT64_C(1);
        } else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
            width = UINT64_C(2);
        } else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            width = UINT64_C(3);
        } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            width = UINT64_C(4);
        } else {
            return false;
        }
        if (width > length - offset) return false;
        if (width >= UINT64_C(2)) {
            uint8_t second = bytes[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80) ||
                (first == UINT8_C(0xe0) && second < UINT8_C(0xa0)) ||
                (first == UINT8_C(0xed) && second > UINT8_C(0x9f)) ||
                (first == UINT8_C(0xf0) && second < UINT8_C(0x90)) ||
                (first == UINT8_C(0xf4) && second > UINT8_C(0x8f))) {
                return false;
            }
        }
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail) {
            if ((bytes[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80)) {
                return false;
            }
        }
        offset += width;
    }
    return true;
}

static bool spx_command_argument_is_valid_v1(spx_str_v1 value) {
    if (!spx_command_utf8_v1(value.data, value.len)) return false;
    for (uint64_t index = UINT64_C(0); index < value.len; ++index) {
        if (value.data[index] == UINT8_C(0)) return false;
    }
    return true;
}

static bool spx_language_command_input_is_valid_v1(
    const struct spx_language_command_input_v1 *input
) {
    if (input == NULL || input->argument_count > SPX_COMMAND_ARGUMENT_LIMIT_V1) {
        return false;
    }
    uint64_t total = UINT64_C(0);
    for (uint32_t index = UINT32_C(0); index < input->argument_count; ++index) {
        spx_str_v1 value = input->arguments[index];
        if (!spx_command_argument_is_valid_v1(value) ||
            value.len > SPX_COMMAND_INPUT_CAPACITY_V1 - total) {
            return false;
        }
        total += value.len;
    }
    spx_slice_u8_v1 bytes = input->stdin_snapshot;
    if (bytes.len > SPX_COMMAND_INPUT_CAPACITY_V1 - total ||
        (bytes.len == UINT64_C(0) ? bytes.ptr != NULL : bytes.ptr == NULL)) {
        return false;
    }
    return true;
}

static spx_status_token spx_command_input_status_v1(
    struct spx_context *spx_ctx,
    uint32_t code
) {
    if (code < SPX_COMMAND_INPUT_ARGUMENT_RANGE_V1 ||
        code > SPX_COMMAND_INPUT_CAPACITY_FAILURE_V1) {
        spx_runtime_invariant_failure("command input status is outside the closed table");
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        SPX_COMMAND_INPUT_STATUS_DOMAIN_V1,
        code,
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("command input status could not be recorded");
    }
    return token;
}

static struct spx_language_command_state_v1 *spx_command_state_v1(
    struct spx_context *spx_ctx
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("command input state is unavailable");
    }
    struct spx_language_command_state_v1 *state =
        (struct spx_language_command_state_v1 *)spx_ctx->target_state;
    if (state->input == NULL) {
        spx_runtime_invariant_failure("command input snapshot is unavailable");
    }
    return state;
}

static spx_status_token spx_host_args_len_v1(
    struct spx_context *spx_ctx,
    uint64_t *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("args_len result slot is unavailable");
    }
    struct spx_language_command_state_v1 *state = spx_command_state_v1(spx_ctx);
    *result_out = (uint64_t)state->input->argument_count;
    return SPX_STATUS_SUCCESS;
}

static spx_status_token spx_host_arg_utf8_v1(
    struct spx_context *spx_ctx,
    uint64_t index,
    spx_str_v1 *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("arg_utf8 result slot is unavailable");
    }
    *result_out = (spx_str_v1){ .data = NULL, .len = UINT64_C(0) };
    struct spx_language_command_state_v1 *state = spx_command_state_v1(spx_ctx);
    if (index >= (uint64_t)state->input->argument_count) {
        return spx_command_input_status_v1(
            spx_ctx,
            SPX_COMMAND_INPUT_ARGUMENT_RANGE_V1
        );
    }
    spx_str_v1 value = state->input->arguments[index];
    if (!spx_command_utf8_v1(value.data, value.len)) {
        return spx_command_input_status_v1(
            spx_ctx,
            SPX_COMMAND_INPUT_INVALID_UTF8_V1
        );
    }
    *result_out = value;
    return SPX_STATUS_SUCCESS;
}

static spx_status_token spx_host_stdin_read_v1(
    struct spx_context *spx_ctx,
    spx_bytes_v1 *result_out
) {
    if (result_out == NULL) {
        spx_runtime_invariant_failure("stdin_read result slot is unavailable");
    }
    *result_out = (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    struct spx_language_command_state_v1 *state = spx_command_state_v1(spx_ctx);
    if (state->stdin_consumed) {
        return spx_command_input_status_v1(
            spx_ctx,
            SPX_COMMAND_INPUT_STDIN_FAILURE_V1
        );
    }
    spx_slice_u8_v1 value = state->input->stdin_snapshot;
    if (value.len > SPX_COMMAND_INPUT_CAPACITY_V1 ||
        (value.len == UINT64_C(0) ? value.ptr != NULL : value.ptr == NULL)) {
        return spx_command_input_status_v1(
            spx_ctx,
            SPX_COMMAND_INPUT_CAPACITY_FAILURE_V1
        );
    }
    if (value.len != UINT64_C(0)) {
        uint8_t *copy = (uint8_t *)malloc((size_t)value.len);
        if (copy == NULL) {
            return spx_command_input_status_v1(
                spx_ctx,
                SPX_COMMAND_INPUT_STDIN_FAILURE_V1
            );
        }
        memcpy(copy, value.ptr, (size_t)value.len);
        result_out->ptr = copy;
        result_out->len = value.len;
    }
    state->stdin_consumed = true;
    return SPX_STATUS_SUCCESS;
}

"#,
    );
}

/// Emit the same authenticated input runtime for the additive line profile and
/// retain its closed operation table even when one valid command does not call
/// every input operation. The legacy v6 projection remains byte-for-byte
/// unchanged because only v7 calls this wrapper.
pub(super) fn emit_line_runtime(output: &mut impl COutput) {
    emit_runtime(output);
    output.push_str(
        r#"static __attribute__((unused)) void spx_line_command_input_table_v1(void) {
    (void)&spx_host_args_len_v1;
    (void)&spx_host_arg_utf8_v1;
    (void)&spx_host_stdin_read_v1;
}

"#,
    );
}

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"int {RUN_SYMBOL}(
    const struct spx_language_command_input_v1 *input,
    struct spx_language_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (!spx_language_command_input_is_valid_v1(input)) return 0;

    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_language_command_state_v1 state = {{0}};
    state.input = input;
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

    if (state.output.stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        state.output.stderr_length >
            SPX_COMMAND_OUTPUT_CAPACITY_V1 - state.output.stdout_length) {{
        memset(&state, 0, sizeof(state));
        memset(result_out, 0, sizeof(*result_out));
        return 0;
    }}
    result_out->semantic_success = true;
    result_out->matched = matched;
    if (state.output.stdout_length != UINT64_C(0)) {{
        memcpy(
            result_out->stdout_bytes,
            state.output.stdout_bytes,
            (size_t)state.output.stdout_length
        );
    }}
    if (state.output.stderr_length != UINT64_C(0)) {{
        memcpy(
            result_out->stderr_bytes,
            state.output.stderr_bytes,
            (size_t)state.output.stderr_length
        );
    }}
    result_out->stdout_length = state.output.stdout_length;
    result_out->stderr_length = state.output.stderr_length;
    memset(&state, 0, sizeof(state));
    return 1;
}}
"#
    )
    .expect("writing native language command runner cannot fail");
}

pub(super) fn emit_process_adapter(output: &mut impl COutput) {
    output.push_str(
        r#"#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#include <limits.h>
#include <windows.h>
#include <wchar.h>
#else
#include <signal.h>
#endif

static int spx_language_command_fail_v1(void) {
    static const char message[] = "SEMAPRAX language command failed\n";
    (void)fwrite(message, sizeof(char), sizeof(message) - 1u, stderr);
    (void)fflush(stderr);
    return 2;
}

static bool spx_language_command_read_stdin_v1(
    uint8_t *bytes,
    uint64_t capacity,
    uint64_t *length_out
) {
    if (bytes == NULL || length_out == NULL ||
        capacity > SPX_COMMAND_INPUT_CAPACITY_V1) return false;
    uint64_t length = UINT64_C(0);
    while (length < capacity) {
        size_t count = fread(
            bytes + (size_t)length,
            sizeof(uint8_t),
            (size_t)(capacity - length),
            stdin
        );
        if (count != 0u) {
            length += (uint64_t)count;
            continue;
        }
        if (feof(stdin)) {
            *length_out = length;
            return true;
        }
        return false;
    }
    int probe = fgetc(stdin);
    if (probe != EOF || ferror(stdin)) return false;
    *length_out = length;
    return true;
}

static bool spx_language_command_flush_v1(
    FILE *stream,
    const uint8_t *bytes,
    uint64_t length
) {
    if (stream == NULL || length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        (length != UINT64_C(0) && bytes == NULL)) return false;
    if (length != UINT64_C(0) &&
        fwrite(bytes, sizeof(uint8_t), (size_t)length, stream) != (size_t)length) {
        return false;
    }
    return fflush(stream) == 0;
}

static int spx_language_command_finish_v1(
    uint8_t *arena,
    struct spx_language_command_result_v1 *result,
    const struct spx_language_command_input_v1 *input
) {
    int exit_code = 2;
    if (arena != NULL && result != NULL && input != NULL &&
        spx_language_command_run_v1(input, result) &&
        result->semantic_success &&
        spx_language_command_flush_v1(
            stderr,
            result->stderr_bytes,
            result->stderr_length
        ) &&
        spx_language_command_flush_v1(
            stdout,
            result->stdout_bytes,
            result->stdout_length
        )) {
        exit_code = result->matched ? 0 : 1;
    }
    if (result != NULL) {
        memset(result, 0, sizeof(*result));
        free(result);
    }
    if (arena != NULL) {
        memset(arena, 0, (size_t)SPX_COMMAND_INPUT_CAPACITY_V1);
        free(arena);
    }
    return exit_code == 2 ? spx_language_command_fail_v1() : exit_code;
}

#if !defined(SPX_NO_LANGUAGE_COMMAND_PROCESS_ADAPTER)
#if defined(_WIN32)
int wmain(int argc, wchar_t **argv) {
    if (_setmode(_fileno(stderr), _O_BINARY) == -1 ||
        argc < 1 || argc > (int)SPX_COMMAND_ARGUMENT_LIMIT_V1 + 1 ||
        argv == NULL ||
        _setmode(_fileno(stdin), _O_BINARY) == -1 ||
        _setmode(_fileno(stdout), _O_BINARY) == -1) {
        return spx_language_command_fail_v1();
    }
    uint8_t *arena = (uint8_t *)malloc((size_t)SPX_COMMAND_INPUT_CAPACITY_V1);
    struct spx_language_command_result_v1 *result =
        (struct spx_language_command_result_v1 *)malloc(sizeof(*result));
    if (arena == NULL || result == NULL) {
        free(arena);
        free(result);
        return spx_language_command_fail_v1();
    }
    struct spx_language_command_input_v1 input = {0};
    input.argument_count = (uint32_t)(argc - 1);
    uint64_t used = UINT64_C(0);
    for (uint32_t index = UINT32_C(0); index < input.argument_count; ++index) {
        const wchar_t *argument = argv[index + UINT32_C(1)];
        if (argument == NULL) return spx_language_command_finish_v1(arena, result, NULL);
        size_t wide_length = 0u;
        while (wide_length <= (size_t)SPX_COMMAND_INPUT_CAPACITY_V1 &&
            argument[wide_length] != L'\0') ++wide_length;
        if (wide_length > (size_t)SPX_COMMAND_INPUT_CAPACITY_V1 ||
            wide_length > (size_t)INT_MAX) {
            return spx_language_command_finish_v1(arena, result, NULL);
        }
        int required = wide_length == 0u ? 0 : WideCharToMultiByte(
            CP_UTF8, WC_ERR_INVALID_CHARS, argument, (int)wide_length,
            NULL, 0, NULL, NULL
        );
        if (required < 0 || (wide_length != 0u && required == 0) ||
            (uint64_t)required > SPX_COMMAND_INPUT_CAPACITY_V1 - used) {
            return spx_language_command_finish_v1(arena, result, NULL);
        }
        if (required != 0 && WideCharToMultiByte(
            CP_UTF8, WC_ERR_INVALID_CHARS, argument, (int)wide_length,
            (char *)(arena + (size_t)used), required, NULL, NULL
        ) != required) {
            return spx_language_command_finish_v1(arena, result, NULL);
        }
        input.arguments[index] = (spx_str_v1){
            .data = required == 0 ? NULL : arena + (size_t)used,
            .len = (uint64_t)required
        };
        used += (uint64_t)required;
    }
#else
int main(int argc, char **argv) {
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR ||
        argc < 1 || argc > (int)SPX_COMMAND_ARGUMENT_LIMIT_V1 + 1 || argv == NULL) {
        return spx_language_command_fail_v1();
    }
    uint8_t *arena = (uint8_t *)malloc((size_t)SPX_COMMAND_INPUT_CAPACITY_V1);
    struct spx_language_command_result_v1 *result =
        (struct spx_language_command_result_v1 *)malloc(sizeof(*result));
    if (arena == NULL || result == NULL) {
        free(arena);
        free(result);
        return spx_language_command_fail_v1();
    }
    struct spx_language_command_input_v1 input = {0};
    input.argument_count = (uint32_t)(argc - 1);
    uint64_t used = UINT64_C(0);
    for (uint32_t index = UINT32_C(0); index < input.argument_count; ++index) {
        const char *argument = argv[index + UINT32_C(1)];
        if (argument == NULL) return spx_language_command_finish_v1(arena, result, NULL);
        uint64_t length = UINT64_C(0);
        while (length <= SPX_COMMAND_INPUT_CAPACITY_V1 && argument[length] != '\0') ++length;
        if (length > SPX_COMMAND_INPUT_CAPACITY_V1 - used ||
            !spx_command_utf8_v1((const uint8_t *)argument, length)) {
            return spx_language_command_finish_v1(arena, result, NULL);
        }
        if (length != UINT64_C(0)) memcpy(arena + (size_t)used, argument, (size_t)length);
        input.arguments[index] = (spx_str_v1){
            .data = length == UINT64_C(0) ? NULL : arena + (size_t)used,
            .len = length
        };
        used += length;
    }
#endif
    uint64_t stdin_length = UINT64_C(0);
    if (!spx_language_command_read_stdin_v1(
        arena + (size_t)used,
        SPX_COMMAND_INPUT_CAPACITY_V1 - used,
        &stdin_length
    )) return spx_language_command_finish_v1(arena, result, NULL);
    input.stdin_snapshot = (spx_slice_u8_v1){
        .ptr = stdin_length == UINT64_C(0) ? NULL : arena + (size_t)used,
        .len = stdin_length
    };
    return spx_language_command_finish_v1(arena, result, &input);
}
#endif
"#,
    );
}
