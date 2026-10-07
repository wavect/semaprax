//! Native process adapter for single-file command-line programs
//! (`docs/TEXT-TOOLKIT-V1.md`, "Command-line programs").
//!
//! Generated semantic functions see only the invocation context. The adapter
//! snapshots argv before the root runs, stages stdout and stderr in memory,
//! and writes them only after the root and its cleanup settle. `main`'s `i64`
//! result becomes the process exit status. `file_read_text` reads regular
//! files below the directory the process started in: the root descriptor is
//! opened once before the program runs, every component is opened relative to
//! its parent without following symbolic links, and the path grammar, per-file
//! bound, and aggregate budget are those of Filesystem I/O v1.

use super::COutput;

/// Emit the argument, two-channel output, and file-text runtime.
pub(super) fn emit_runtime(output: &mut impl COutput, program: &crate::hir::ResolvedProgram) {
    super::native_host_output::emit_language_command_runtime(output);
    super::native_command_io::emit_line_runtime(output);
    output.push_str(
        r#"static __attribute__((unused)) void spx_source_command_output_table_v1(void) {
    (void)&spx_host_command_stdout_write_v1;
    (void)&spx_host_command_stderr_write_v1;
}

struct spx_source_command_state_v1 {
    /* The language-command state is the prefix the argument and output
       helpers authenticate through `target_state`. */
    struct spx_language_command_state_v1 command;
    int file_root;
    uint64_t file_operations;
    uint64_t file_reserved;
};

_Static_assert(
    offsetof(struct spx_source_command_state_v1, command) == 0,
    "language command state must be the source command prefix"
);

"#,
    );
    if crate::string_ops::program_uses_op(program, crate::string_ops::StringOp::FileReadText) {
        output.push_str(FILE_TEXT_RUNTIME_C);
    }
}

const FILE_TEXT_RUNTIME_C: &str = r#"#if !defined(_WIN32)
#include <errno.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#endif
#define SPX_SOURCE_FILE_STATUS_DOMAIN_V1 "semaprax.filesystem.v1"
#define SPX_SOURCE_FILE_TEXT_MAX_V1 UINT64_C(65536)
#define SPX_SOURCE_FILE_OPERATIONS_MAX_V1 UINT64_C(64)
#define SPX_SOURCE_FILE_RESERVED_MAX_V1 UINT64_C(1048576)
#define SPX_SOURCE_FILE_PATH_MAX_V1 UINT64_C(4096)

static spx_status_token spx_source_file_status_v1(struct spx_context *spx_ctx, uint32_t code) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(spx_ctx, SPX_SOURCE_FILE_STATUS_DOMAIN_V1, code,
            SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("filesystem status could not be recorded");
    return token;
}

static bool spx_source_path_valid_v1(spx_str_v1 path) {
    if (path.len == UINT64_C(0) || path.len > SPX_SOURCE_FILE_PATH_MAX_V1) return false;
    uint64_t component = UINT64_C(0);
    for (uint64_t index = UINT64_C(0); index <= path.len; ++index) {
        if (index == path.len || path.data[index] == (uint8_t)'/') {
            uint64_t width = index - component;
            const uint8_t *part = path.data + component;
            if (width == UINT64_C(0) ||
                (width == UINT64_C(1) && part[0] == (uint8_t)'.') ||
                (width == UINT64_C(2) && part[0] == (uint8_t)'.' && part[1] == (uint8_t)'.'))
                return false;
            component = index + UINT64_C(1);
        } else if (path.data[index] == UINT8_C(0) || path.data[index] == (uint8_t)'\\' ||
            path.data[index] == (uint8_t)':') {
            return false;
        }
    }
    return true;
}

#if !defined(_WIN32)
static uint32_t spx_source_errno_status_v1(int error) {
    if (error == ENOENT) return UINT32_C(2);
    if (error == EACCES || error == EPERM) return UINT32_C(6);
    if (error == ELOOP || error == ENOTDIR) return UINT32_C(7);
    return UINT32_C(5);
}
#endif

static spx_status_token spx_host_file_read_text_v1(
    struct spx_context *spx_ctx, spx_str_v1 path, char **result_out
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL || result_out == NULL)
        spx_runtime_invariant_failure("file_read_text state is unavailable");
    struct spx_source_command_state_v1 *state =
        (struct spx_source_command_state_v1 *)spx_ctx->target_state;
    /* Reservation precedes validation and is never refunded. */
    if (state->file_operations >= SPX_SOURCE_FILE_OPERATIONS_MAX_V1 ||
        state->file_reserved > SPX_SOURCE_FILE_RESERVED_MAX_V1 - SPX_SOURCE_FILE_TEXT_MAX_V1) {
        state->file_operations += UINT64_C(1);
        return spx_source_file_status_v1(spx_ctx, UINT32_C(4));
    }
    state->file_operations += UINT64_C(1);
    state->file_reserved += SPX_SOURCE_FILE_TEXT_MAX_V1;
    if (!spx_source_path_valid_v1(path)) return spx_source_file_status_v1(spx_ctx, UINT32_C(1));
#if defined(_WIN32)
    return spx_source_file_status_v1(spx_ctx, UINT32_C(6));
#else
    if (state->file_root < 0) return spx_source_file_status_v1(spx_ctx, UINT32_C(6));
    char component[SPX_SOURCE_FILE_PATH_MAX_V1 + 1u];
    int parent = dup(state->file_root);
    if (parent < 0) return spx_source_file_status_v1(spx_ctx, spx_source_errno_status_v1(errno));
    uint64_t start = UINT64_C(0);
    int file = -1;
    for (uint64_t index = UINT64_C(0); index <= path.len; ++index) {
        if (index != path.len && path.data[index] != (uint8_t)'/') continue;
        uint64_t width = index - start;
        memcpy(component, path.data + start, (size_t)width);
        component[width] = '\0';
        if (index == path.len) {
            file = openat(parent, component, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK);
        } else {
            int next = openat(parent, component,
                O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK);
            if (next < 0) {
                uint32_t code = spx_source_errno_status_v1(errno);
                (void)close(parent);
                return spx_source_file_status_v1(spx_ctx, code);
            }
            (void)close(parent);
            parent = next;
        }
        start = index + UINT64_C(1);
    }
    uint32_t code = UINT32_C(0);
    if (file < 0) code = spx_source_errno_status_v1(errno);
    (void)close(parent);
    if (code != UINT32_C(0)) return spx_source_file_status_v1(spx_ctx, code);
    struct stat metadata;
    if (fstat(file, &metadata) != 0) code = spx_source_errno_status_v1(errno);
    else if (!S_ISREG(metadata.st_mode)) code = UINT32_C(7);
    uint8_t *buffer = NULL;
    uint64_t length = UINT64_C(0);
    if (code == UINT32_C(0)) {
        buffer = (uint8_t *)malloc((size_t)SPX_SOURCE_FILE_TEXT_MAX_V1 + 1u);
        if (buffer == NULL) code = UINT32_C(5);
    }
    while (code == UINT32_C(0)) {
        ssize_t count = read(file, buffer + (size_t)length,
            (size_t)(SPX_SOURCE_FILE_TEXT_MAX_V1 + UINT64_C(1) - length));
        if (count < 0) {
            if (errno == EINTR) continue;
            code = UINT32_C(5);
        } else if (count == 0) {
            break;
        } else {
            length += (uint64_t)count;
            if (length > SPX_SOURCE_FILE_TEXT_MAX_V1) code = UINT32_C(4);
        }
    }
    (void)close(file);
    if (code != UINT32_C(0)) {
        free(buffer);
        return spx_source_file_status_v1(spx_ctx, code);
    }
    if (!spx_command_utf8_v1(length == UINT64_C(0) ? NULL : buffer, length)) {
        free(buffer);
        return spx_text_failure_v1(spx_ctx, UINT32_C(3));
    }
    *result_out = spx_string_from_literal((const char *)buffer, length);
    free(buffer);
    return SPX_STATUS_SUCCESS;
#endif
}

"#;

/// Emit the process `main`: snapshot argv, run the root, settle, then publish
/// stderr and stdout and exit with the root's result.
pub(super) fn emit_process_adapter(output: &mut impl COutput, root_symbol: &str) {
    writeln!(
        output,
        r#"#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#else
#include <fcntl.h>
#include <signal.h>
#include <unistd.h>
#endif

static int spx_source_command_usage_v1(const char *message) {{
    (void)fputs(message, stderr);
    (void)fflush(stderr);
    return 2;
}}

static bool spx_source_command_flush_v1(FILE *stream, const uint8_t *bytes, uint64_t length) {{
    if (length != UINT64_C(0) &&
        fwrite(bytes, sizeof(uint8_t), (size_t)length, stream) != (size_t)length) return false;
    return fflush(stream) == 0;
}}

#ifndef SPX_NO_ENTRY_WRAPPER
int main(int argc, char **argv) {{
    static struct spx_source_command_state_v1 state;
    struct spx_language_command_input_v1 input = {{0}};
#if defined(_WIN32)
    if (_setmode(_fileno(stdout), _O_BINARY) == -1 || _setmode(_fileno(stderr), _O_BINARY) == -1)
        return spx_source_command_usage_v1("SEMAPRAX single-file command: cannot select binary output\n");
#else
    (void)signal(SIGPIPE, SIG_IGN);
#endif
    if (argc < 1 || argv == NULL || argc - 1 > (int)SPX_COMMAND_ARGUMENT_LIMIT_V1)
        return spx_source_command_usage_v1("SEMAPRAX single-file command accepts at most 16 arguments\n");
    uint64_t total = UINT64_C(0);
    for (int index = 1; index < argc; ++index) {{
        const char *argument = argv[index];
        uint64_t length = UINT64_C(0);
        while (argument != NULL && length <= SPX_COMMAND_INPUT_CAPACITY_V1 && argument[length] != '\0') ++length;
        if (argument == NULL || length > SPX_COMMAND_INPUT_CAPACITY_V1 - total ||
            !spx_command_utf8_v1(length == UINT64_C(0) ? NULL : (const uint8_t *)argument, length))
            return spx_source_command_usage_v1("SEMAPRAX single-file command arguments must be UTF-8 and at most 65536 bytes\n");
        input.arguments[input.argument_count++] = (spx_str_v1){{
            .data = length == UINT64_C(0) ? NULL : (const uint8_t *)argument, .len = length
        }};
        total += length;
    }}
    if (!spx_language_command_input_is_valid_v1(&input))
        return spx_source_command_usage_v1("SEMAPRAX single-file command arguments are invalid\n");
    state.command.input = &input;
#if defined(_WIN32)
    state.file_root = -1;
#else
    state.file_root = open(".", O_RDONLY | O_DIRECTORY | O_CLOEXEC);
#endif
    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, &state)) {{
        fputs("SEMAPRAX native runtime invariant failure: context initialization\n", stderr);
        return 72;
    }}
    int64_t result = INT64_C(0);
    spx_status_token status = {root_symbol}(&spx_ctx, &result);
#if !defined(_WIN32)
    if (state.file_root >= 0) (void)close(state.file_root);
#endif
    if (status != SPX_STATUS_SUCCESS) {{
        (void)spx_public_failure(&spx_ctx, status);
        return 1;
    }}
    if (result < INT64_C(0) || result > INT64_C(255)) {{
        fprintf(stderr, "SEMAPRAX single-file command: main returned %lld, outside the exit status range 0..=255\n", (long long)result);
        return 1;
    }}
    if (!spx_source_command_flush_v1(stderr, state.command.output.stderr_bytes, state.command.output.stderr_length) ||
        !spx_source_command_flush_v1(stdout, state.command.output.stdout_bytes, state.command.output.stdout_length))
        return 1;
    return (int)result;
}}
#endif"#
    )
    .expect("writing the native source command adapter cannot fail");
}
