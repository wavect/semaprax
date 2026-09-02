//! Fixed native process adapter for Project Useful Data Command v2.
//!
//! The semantic runner is deliberately separate from the process adapter. It
//! accepts two already bounded borrowed slices, stages output in invocation-
//! local memory, and publishes the transcript only after semantic success.
//! The process adapter owns argv/stdin decoding and the one physical flush.

use super::COutput;

pub(super) const RUN_SYMBOL: &str = "spx_native_command_run_v1";

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"struct spx_native_command_result_v1 {{
    bool matched;
    uint64_t transcript_length;
    uint8_t transcript[SPX_STDOUT_TRANSCRIPT_CAPACITY_V1];
}};

int {RUN_SYMBOL}(
    spx_slice_u8_v1 input,
    spx_slice_u8_v1 needle,
    struct spx_native_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (input.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 ||
        needle.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 ||
        input.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 - needle.len ||
        (input.len == UINT64_C(0) ? input.ptr != NULL : input.ptr == NULL) ||
        (needle.len == UINT64_C(0) ? needle.ptr != NULL : needle.ptr == NULL)) {{
        return 0;
    }}

    struct spx_stdout_staging_v1 staging = {{0}};
    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx,
        UINT64_C(1),
        spx_status_entries,
        UINT32_C(1),
        NULL,
        NULL,
        &staging
    )) return 0;

    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, input, needle, &matched);
    if (status != SPX_STATUS_SUCCESS) {{
        (void)spx_status_resolve(&spx_ctx, status);
        (void)spx_status_resolve_detail(&spx_ctx, status);
        memset(&staging, 0, sizeof(staging));
        return 0;
    }}
    if (staging.length > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1) {{
        memset(&staging, 0, sizeof(staging));
        return 0;
    }}

    if (!matched) {{
        memset(&staging, 0, sizeof(staging));
        return 1;
    }}

    result_out->matched = true;
    if (staging.length != UINT64_C(0)) {{
        memcpy(result_out->transcript, staging.bytes, (size_t)staging.length);
    }}
    result_out->transcript_length = staging.length;
    memset(&staging, 0, sizeof(staging));
    return 1;
}}
"#
    )
    .expect("writing native command runner cannot fail");
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

static int spx_native_command_fail_v1(void) {
    static const char message[] = "SEMAPRAX native command failed\n";
    (void)fwrite(message, sizeof(char), sizeof(message) - 1u, stderr);
    return 2;
}

static __attribute__((unused)) bool spx_native_command_utf8_v1(
    const uint8_t *bytes,
    uint64_t length
) {
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

static bool spx_native_command_read_stdin_v1(
    uint8_t *bytes,
    uint64_t capacity,
    uint64_t *length_out
) {
    if (bytes == NULL || length_out == NULL || capacity > UINT64_C(65536)) {
        return false;
    }
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
    if (probe != EOF) return false;
    if (ferror(stdin)) return false;
    *length_out = length;
    return true;
}

static int spx_native_command_execute_v1(
    uint8_t *arena,
    uint64_t needle_length
) {
    if (arena == NULL || needle_length > UINT64_C(65536)) {
        return spx_native_command_fail_v1();
    }
    uint64_t input_length = UINT64_C(0);
    if (!spx_native_command_read_stdin_v1(
        arena + (size_t)needle_length,
        UINT64_C(65536) - needle_length,
        &input_length
    )) {
        return spx_native_command_fail_v1();
    }
    spx_slice_u8_v1 input = {
        .ptr = input_length == UINT64_C(0) ? NULL : arena + (size_t)needle_length,
        .len = input_length
    };
    spx_slice_u8_v1 needle = {
        .ptr = needle_length == UINT64_C(0) ? NULL : arena,
        .len = needle_length
    };
    struct spx_native_command_result_v1 result;
    if (!spx_native_command_run_v1(input, needle, &result)) {
        return spx_native_command_fail_v1();
    }
    if (result.transcript_length != UINT64_C(0) &&
        fwrite(
            result.transcript,
            sizeof(uint8_t),
            (size_t)result.transcript_length,
            stdout
        ) != (size_t)result.transcript_length) {
        memset(&result, 0, sizeof(result));
        return spx_native_command_fail_v1();
    }
    if (fflush(stdout) != 0) {
        memset(&result, 0, sizeof(result));
        return spx_native_command_fail_v1();
    }
    int exit_code = result.matched ? 0 : 1;
    memset(&result, 0, sizeof(result));
    return exit_code;
}

#if defined(_WIN32)
int wmain(int argc, wchar_t **argv) {
    uint8_t arena[UINT32_C(65536)];
    if (_setmode(_fileno(stderr), _O_BINARY) == -1) return 2;
    if (argc != 2 || argv == NULL || argv[1] == NULL ||
        _setmode(_fileno(stdin), _O_BINARY) == -1 ||
        _setmode(_fileno(stdout), _O_BINARY) == -1) {
        return spx_native_command_fail_v1();
    }
    size_t wide_length = UINT32_C(0);
    while (wide_length <= UINT32_C(65536) && argv[1][wide_length] != L'\0') {
        ++wide_length;
    }
    if (wide_length > UINT32_C(65536) || wide_length > (size_t)INT_MAX) {
        return spx_native_command_fail_v1();
    }
    int needle_length = 0;
    if (wide_length != 0u) {
        needle_length = WideCharToMultiByte(
            CP_UTF8,
            WC_ERR_INVALID_CHARS,
            argv[1],
            (int)wide_length,
            (char *)arena,
            65536,
            NULL,
            NULL
        );
        if (needle_length <= 0) return spx_native_command_fail_v1();
    }
    return spx_native_command_execute_v1(arena, (uint64_t)needle_length);
}
#else
int main(int argc, char **argv) {
    uint8_t arena[UINT32_C(65536)];
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR ||
        argc != 2 || argv == NULL || argv[1] == NULL) {
        return spx_native_command_fail_v1();
    }
    uint64_t needle_length = UINT64_C(0);
    while (needle_length <= UINT64_C(65536) && argv[1][needle_length] != '\0') {
        ++needle_length;
    }
    if (needle_length > UINT64_C(65536) ||
        !spx_native_command_utf8_v1((const uint8_t *)argv[1], needle_length)) {
        return spx_native_command_fail_v1();
    }
    if (needle_length != UINT64_C(0)) {
        memcpy(arena, argv[1], (size_t)needle_length);
    }
    return spx_native_command_execute_v1(arena, needle_length);
}
#endif
"#,
    );
}

#[cfg(test)]
#[path = "native_command/tests.rs"]
mod tests;
