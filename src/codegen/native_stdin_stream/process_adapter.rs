use super::super::COutput;

pub(super) fn emit_process_adapter(output: &mut impl COutput) {
    output.push_str(
        r#"#ifndef SPX_LANGUAGE_COMMAND_RAW_ARGUMENT_LIMIT_V1
#define SPX_LANGUAGE_COMMAND_RAW_ARGUMENT_LIMIT_V1 SPX_COMMAND_ARGUMENT_LIMIT_V1
#endif

#ifndef SPX_LANGUAGE_COMMAND_STREAM_RUN_V1
#define SPX_LANGUAGE_COMMAND_STREAM_RUN_V1(input, provider, result) \
    spx_language_command_stream_run_v1((input), (provider), (result))
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#include <limits.h>
#include <windows.h>
#include <wchar.h>
#else
#include <signal.h>
#endif

struct spx_process_stdin_provider_context_v1 {
    FILE *stream;
    bool opened;
    bool settled;
};

static uint32_t spx_process_stdin_open_v1(
    void *opaque,
    uintptr_t *provider_token_out
) {
    struct spx_process_stdin_provider_context_v1 *context =
        (struct spx_process_stdin_provider_context_v1 *)opaque;
    if (context == NULL || provider_token_out == NULL || context->stream == NULL ||
        context->opened || context->settled) {
        spx_runtime_invariant_failure("process stdin provider open state is invalid");
    }
    context->opened = true;
    *provider_token_out = (uintptr_t)1;
    return UINT32_C(0);
}

static uint32_t spx_process_stdin_read_v1(
    void *opaque,
    uintptr_t provider_token,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *length_out,
    uint32_t *eof_out
) {
    struct spx_process_stdin_provider_context_v1 *context =
        (struct spx_process_stdin_provider_context_v1 *)opaque;
    if (context == NULL || provider_token != (uintptr_t)1 || !context->opened ||
        context->settled || context->stream == NULL || buffer == NULL ||
        capacity != SPX_STDIN_STREAM_CHUNK_CAPACITY_V1 ||
        length_out == NULL || eof_out == NULL) {
        spx_runtime_invariant_failure("process stdin provider read state is invalid");
    }
    size_t count = fread(buffer, sizeof(uint8_t), (size_t)capacity, context->stream);
    if (ferror(context->stream)) return UINT32_C(3);
    if (count != (size_t)0) {
        *length_out = (uint32_t)count;
        *eof_out = UINT32_C(0);
        return UINT32_C(0);
    }
    if (feof(context->stream)) {
        *length_out = UINT32_C(0);
        *eof_out = UINT32_C(1);
        return UINT32_C(0);
    }
    return UINT32_C(3);
}

static void spx_process_stdin_drop_v1(void *opaque, uintptr_t provider_token) {
    struct spx_process_stdin_provider_context_v1 *context =
        (struct spx_process_stdin_provider_context_v1 *)opaque;
    if (context == NULL || provider_token != (uintptr_t)1 || !context->opened ||
        context->settled) {
        spx_runtime_invariant_failure("process stdin provider drop state is invalid");
    }
}

static void spx_process_stdin_settle_v1(void *opaque) {
    struct spx_process_stdin_provider_context_v1 *context =
        (struct spx_process_stdin_provider_context_v1 *)opaque;
    if (context == NULL || context->settled) {
        spx_runtime_invariant_failure("process stdin provider settlement is invalid");
    }
    context->settled = true;
}

static int spx_language_command_stream_fail_v1(void) {
    static const char message[] = "SEMAPRAX language command failed\n";
    (void)fwrite(message, sizeof(char), sizeof(message) - 1u, stderr);
    (void)fflush(stderr);
    return 2;
}

static bool spx_language_command_stream_flush_v1(
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

static int spx_language_command_stream_finish_v1(
    uint8_t *argument_arena,
    struct spx_language_command_result_v1 *result,
    const struct spx_language_command_input_v1 *input
) {
    int exit_code = 2;
    if (argument_arena != NULL && result != NULL && input != NULL) {
        struct spx_process_stdin_provider_context_v1 stream_context = {
            .stream = stdin,
            .opened = false,
            .settled = false
        };
        const struct spx_stdin_stream_provider_v1 provider = {
            .context = &stream_context,
            .open = spx_process_stdin_open_v1,
            .read = spx_process_stdin_read_v1,
            .drop = spx_process_stdin_drop_v1,
            .settle = spx_process_stdin_settle_v1
        };
        if (SPX_LANGUAGE_COMMAND_STREAM_RUN_V1(input, &provider, result) &&
            result->semantic_success &&
            spx_language_command_stream_flush_v1(
                stderr,
                result->stderr_bytes,
                result->stderr_length
            ) &&
            spx_language_command_stream_flush_v1(
                stdout,
                result->stdout_bytes,
                result->stdout_length
            )) {
            exit_code = result->matched ? 0 : 1;
        }
    }
    if (result != NULL) {
        memset(result, 0, sizeof(*result));
        free(result);
    }
    if (argument_arena != NULL) {
        memset(argument_arena, 0, (size_t)SPX_COMMAND_INPUT_CAPACITY_V1);
        free(argument_arena);
    }
    return exit_code == 2 ? spx_language_command_stream_fail_v1() : exit_code;
}

#if !defined(SPX_NO_LANGUAGE_COMMAND_PROCESS_ADAPTER)
#if defined(_WIN32)
int wmain(int argc, wchar_t **argv) {
    if (_setmode(_fileno(stderr), _O_BINARY) == -1 ||
        argc < 1 || argc > (int)SPX_LANGUAGE_COMMAND_RAW_ARGUMENT_LIMIT_V1 + 1 ||
        argv == NULL ||
        _setmode(_fileno(stdin), _O_BINARY) == -1 ||
        _setmode(_fileno(stdout), _O_BINARY) == -1) {
        return spx_language_command_stream_fail_v1();
    }
    uint8_t *arena = (uint8_t *)malloc((size_t)SPX_COMMAND_INPUT_CAPACITY_V1);
    struct spx_language_command_result_v1 *result =
        (struct spx_language_command_result_v1 *)malloc(sizeof(*result));
    if (arena == NULL || result == NULL) {
        free(arena);
        free(result);
        return spx_language_command_stream_fail_v1();
    }
    struct spx_language_command_input_v1 input = {0};
    uint64_t used = UINT64_C(0);
    for (uint32_t index = UINT32_C(0); index < (uint32_t)(argc - 1); ++index) {
        const wchar_t *argument = argv[index + UINT32_C(1)];
        if (argument == NULL) return spx_language_command_stream_finish_v1(arena, result, NULL);
        size_t wide_length = 0u;
        while (wide_length <= (size_t)SPX_COMMAND_INPUT_CAPACITY_V1 &&
            argument[wide_length] != L'\0') ++wide_length;
        if (wide_length > (size_t)SPX_COMMAND_INPUT_CAPACITY_V1 ||
            wide_length > (size_t)INT_MAX) {
            return spx_language_command_stream_finish_v1(arena, result, NULL);
        }
        int required = wide_length == 0u ? 0 : WideCharToMultiByte(
            CP_UTF8, WC_ERR_INVALID_CHARS, argument, (int)wide_length,
            NULL, 0, NULL, NULL
        );
        if (required < 0 || (wide_length != 0u && required == 0) ||
            (uint64_t)required > SPX_COMMAND_INPUT_CAPACITY_V1 - used) {
            return spx_language_command_stream_finish_v1(arena, result, NULL);
        }
        if (required != 0 && WideCharToMultiByte(
            CP_UTF8, WC_ERR_INVALID_CHARS, argument, (int)wide_length,
            (char *)(arena + (size_t)used), required, NULL, NULL
        ) != required) {
            return spx_language_command_stream_finish_v1(arena, result, NULL);
        }
        if (input.argument_count >= SPX_COMMAND_ARGUMENT_LIMIT_V1) {
            return spx_language_command_stream_finish_v1(arena, result, NULL);
        }
        input.arguments[input.argument_count++] = (spx_str_v1){
            .data = required == 0 ? NULL : arena + (size_t)used,
            .len = (uint64_t)required
        };
        used += (uint64_t)required;
    }
    input.stdin_snapshot = (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
    return spx_language_command_stream_finish_v1(arena, result, &input);
}
#else
int main(int argc, char **argv) {
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR ||
        argc < 1 || argc > (int)SPX_LANGUAGE_COMMAND_RAW_ARGUMENT_LIMIT_V1 + 1 ||
        argv == NULL) {
        return spx_language_command_stream_fail_v1();
    }
    uint8_t *arena = (uint8_t *)malloc((size_t)SPX_COMMAND_INPUT_CAPACITY_V1);
    struct spx_language_command_result_v1 *result =
        (struct spx_language_command_result_v1 *)malloc(sizeof(*result));
    if (arena == NULL || result == NULL) {
        free(arena);
        free(result);
        return spx_language_command_stream_fail_v1();
    }
    struct spx_language_command_input_v1 input = {0};
    uint64_t used = UINT64_C(0);
    for (uint32_t index = UINT32_C(0); index < (uint32_t)(argc - 1); ++index) {
        const char *argument = argv[index + UINT32_C(1)];
        if (argument == NULL) return spx_language_command_stream_finish_v1(arena, result, NULL);
        uint64_t length = UINT64_C(0);
        while (length <= SPX_COMMAND_INPUT_CAPACITY_V1 && argument[length] != '\0') ++length;
        if (length > SPX_COMMAND_INPUT_CAPACITY_V1 - used ||
            !spx_command_utf8_v1((const uint8_t *)argument, length)) {
            return spx_language_command_stream_finish_v1(arena, result, NULL);
        }
        if (length != UINT64_C(0)) memcpy(arena + (size_t)used, argument, (size_t)length);
        if (input.argument_count >= SPX_COMMAND_ARGUMENT_LIMIT_V1) {
            return spx_language_command_stream_finish_v1(arena, result, NULL);
        }
        input.arguments[input.argument_count++] = (spx_str_v1){
            .data = length == UINT64_C(0) ? NULL : arena + (size_t)used,
            .len = length
        };
        used += length;
    }
    input.stdin_snapshot = (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
    return spx_language_command_stream_finish_v1(arena, result, &input);
}
#endif
#endif
"#,
    );
}
