//! Additive Project-v28 native process adapter.
//!
//! The argv and file-text carrier is the frozen Project-v26 implementation.
//! This module replaces only String and staged-output resources.

use super::COutput;

pub(super) fn emit_runtime(output: &mut impl COutput, program: &crate::hir::ResolvedProgram) {
    output.push_str(OUTPUT_RUNTIME_C);
    super::native_command_io::emit_line_runtime(output);
    output.push_str(
        r#"static __attribute__((unused)) void spx_source_resource_output_table_v1(void) {
    (void)&spx_host_command_stdout_write_checked_v1;
    (void)&spx_host_command_stderr_write_checked_v1;
    (void)&spx_host_command_stdout_append_v1;
    (void)&spx_host_command_stderr_append_v1;
    (void)&spx_host_command_stdout_write_resource_str_checked_v1;
    (void)&spx_host_command_stderr_write_resource_str_checked_v1;
    (void)&spx_host_command_stdout_append_resource_str_v1;
    (void)&spx_host_command_stderr_append_resource_str_v1;
}

struct spx_source_command_state_v1 {
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
        output.push_str(super::native_source_command::FILE_TEXT_RUNTIME_C);
    }
}

const OUTPUT_RUNTIME_C: &str = r##"#define SPX_COMMAND_OUTPUT_CAPACITY_V1 UINT64_C(1048576)
#define SPX_SOURCE_RESOURCE_OUTPUT_ALLOCATION_V1 UINT64_C(2097152)
#define SPX_COMMAND_OUTPUT_STATUS_DOMAIN_V1 "semaprax.command-output.v1"
#define SPX_COMMAND_OUTPUT_CAPACITY_FAILURE_V1 UINT32_C(1)

static __attribute__((unused)) void spx_source_resource_slice_require_valid_v1(
    spx_slice_u8_v1 value
) {
    if (value.len > SPX_COMMAND_OUTPUT_CAPACITY_V1)
        spx_runtime_invariant_failure("borrowed text byte view exceeds the source resource bound");
    spx_slice_u8_require_shape(value);
}

static __attribute__((unused)) uint64_t spx_source_resource_byte_len_v1(
    spx_slice_u8_v1 value
) {
    spx_source_resource_slice_require_valid_v1(value);
    return value.len;
}

static __attribute__((unused)) spx_status_token spx_source_resource_byte_range_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t start, uint64_t end,
    spx_slice_u8_v1 *result_out
) {
    spx_source_resource_slice_require_valid_v1(value);
    if (result_out == NULL)
        spx_runtime_invariant_failure("byte range result carrier is unavailable");
    *result_out = (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
    uint32_t failure_code = start > end ? UINT32_C(1) :
        (end > value.len ? UINT32_C(2) : UINT32_C(0));
    if (failure_code != UINT32_C(0)) {
        spx_status_token token = SPX_STATUS_SUCCESS;
        if (!spx_status_record_adapter(
            spx_ctx, "semaprax.byte-range.v1", failure_code,
            SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token
        )) spx_runtime_invariant_failure("byte range status could not be recorded");
        return token;
    }
    uint64_t length = end - start;
    result_out->ptr = length == UINT64_C(0) ? NULL : value.ptr + (size_t)start;
    result_out->len = length;
    return SPX_STATUS_SUCCESS;
}

struct spx_command_output_staging_v1 {
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t *stdout_bytes;
    uint8_t *stderr_bytes;
};

static struct spx_command_output_staging_v1 *spx_source_resource_output_v1(
    struct spx_context *spx_ctx
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL)
        spx_runtime_invariant_failure("command output state is unavailable");
    struct spx_command_output_staging_v1 *staging =
        (struct spx_command_output_staging_v1 *)spx_ctx->target_state;
    if (staging->stdout_bytes == NULL || staging->stderr_bytes == NULL)
        spx_runtime_invariant_failure("source resource output allocation is unavailable");
    return staging;
}

static spx_status_token spx_source_resource_output_failure_v1(struct spx_context *spx_ctx);

static spx_status_token spx_host_command_output_write_checked_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, bool stderr_channel,
    bool resource_text, uint64_t *result_out
) {
    if (resource_text) spx_source_resource_slice_require_valid_v1(value);
    else spx_slice_u8_require_valid(value);
    if (result_out == NULL) spx_runtime_invariant_failure("command write result is unavailable");
    *result_out = UINT64_C(0);
    struct spx_command_output_staging_v1 *staging = spx_source_resource_output_v1(spx_ctx);
    uint64_t other = stderr_channel ? staging->stdout_length : staging->stderr_length;
    if (other > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        value.len > SPX_COMMAND_OUTPUT_CAPACITY_V1 - other)
        return spx_source_resource_output_failure_v1(spx_ctx);
    uint8_t *destination = stderr_channel ? staging->stderr_bytes : staging->stdout_bytes;
    if (value.len != UINT64_C(0)) memcpy(destination, value.ptr, (size_t)value.len);
    if (stderr_channel) staging->stderr_length = value.len;
    else staging->stdout_length = value.len;
    *result_out = value.len;
    return SPX_STATUS_SUCCESS;
}

static spx_status_token spx_host_command_stdout_write_checked_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_write_checked_v1(spx_ctx, value, false, false, result_out); }

static spx_status_token spx_host_command_stderr_write_checked_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_write_checked_v1(spx_ctx, value, true, false, result_out); }

static spx_status_token spx_host_command_stdout_write_resource_str_checked_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_write_checked_v1(spx_ctx, value, false, true, result_out); }

static spx_status_token spx_host_command_stderr_write_resource_str_checked_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_write_checked_v1(spx_ctx, value, true, true, result_out); }

static spx_status_token spx_source_resource_output_failure_v1(struct spx_context *spx_ctx) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx, SPX_COMMAND_OUTPUT_STATUS_DOMAIN_V1,
        SPX_COMMAND_OUTPUT_CAPACITY_FAILURE_V1, SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE, &token
    )) spx_runtime_invariant_failure("command output status could not be recorded");
    return token;
}

static spx_status_token spx_host_command_output_append_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, bool stderr_channel,
    bool resource_text, uint64_t *result_out
) {
    if (resource_text) spx_source_resource_slice_require_valid_v1(value);
    else spx_slice_u8_require_valid(value);
    if (result_out == NULL) spx_runtime_invariant_failure("command append result is unavailable");
    *result_out = UINT64_C(0);
    struct spx_command_output_staging_v1 *staging = spx_source_resource_output_v1(spx_ctx);
    if (staging->stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        staging->stderr_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 - staging->stdout_length ||
        value.len > SPX_COMMAND_OUTPUT_CAPACITY_V1 -
            staging->stdout_length - staging->stderr_length)
        return spx_source_resource_output_failure_v1(spx_ctx);
    uint64_t offset = stderr_channel ? staging->stderr_length : staging->stdout_length;
    uint8_t *destination = stderr_channel ? staging->stderr_bytes : staging->stdout_bytes;
    if (value.len != UINT64_C(0)) memcpy(destination + (size_t)offset, value.ptr, (size_t)value.len);
    if (stderr_channel) staging->stderr_length += value.len;
    else staging->stdout_length += value.len;
    *result_out = value.len;
    return SPX_STATUS_SUCCESS;
}

static spx_status_token spx_host_command_stdout_append_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_append_v1(spx_ctx, value, false, false, result_out); }

static spx_status_token spx_host_command_stderr_append_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_append_v1(spx_ctx, value, true, false, result_out); }

static spx_status_token spx_host_command_stdout_append_resource_str_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_append_v1(spx_ctx, value, false, true, result_out); }

static spx_status_token spx_host_command_stderr_append_resource_str_v1(
    struct spx_context *spx_ctx, spx_slice_u8_v1 value, uint64_t *result_out
) { return spx_host_command_output_append_v1(spx_ctx, value, true, true, result_out); }

"##;

pub(super) fn emit_process_adapter(
    output: &mut impl COutput,
    root_symbol: &str,
    program: &crate::hir::ResolvedProgram,
) {
    let uses_file_text =
        crate::string_ops::program_uses_op(program, crate::string_ops::StringOp::FileReadText);
    let file_root_setup = if uses_file_text {
        r#"#if defined(_WIN32)
    state.file_root = -1;
#else
    state.file_root = open(".", O_RDONLY | O_DIRECTORY | O_CLOEXEC);
#endif"#
    } else {
        "    state.file_root = -1;"
    };
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

static int spx_source_resource_usage_v1(const char *message) {{
    (void)fputs(message, stderr);
    (void)fflush(stderr);
    return 2;
}}

static bool spx_source_resource_flush_v1(FILE *stream, const uint8_t *bytes, uint64_t length) {{
    if (length != UINT64_C(0) &&
        fwrite(bytes, sizeof(uint8_t), (size_t)length, stream) != (size_t)length) return false;
    return fflush(stream) == 0;
}}

static void spx_source_resource_release_v1(uint8_t *allocation) {{
    if (allocation == NULL) return;
    volatile uint8_t *bytes = (volatile uint8_t *)allocation;
    for (uint64_t index = UINT64_C(0); index < SPX_SOURCE_RESOURCE_OUTPUT_ALLOCATION_V1; ++index)
        bytes[index] = UINT8_C(0);
    free(allocation);
}}

#ifndef SPX_NO_ENTRY_WRAPPER
int main(int argc, char **argv) {{
    static struct spx_source_command_state_v1 state;
    struct spx_language_command_input_v1 input = {{0}};
#if defined(_WIN32)
    if (_setmode(_fileno(stdout), _O_BINARY) == -1 || _setmode(_fileno(stderr), _O_BINARY) == -1)
        return spx_source_resource_usage_v1("SEMAPRAX source resource command: cannot select binary output\n");
#else
    (void)signal(SIGPIPE, SIG_IGN);
#endif
    if (argc < 1 || argv == NULL || argc - 1 > (int)SPX_COMMAND_ARGUMENT_LIMIT_V1)
        return spx_source_resource_usage_v1("SEMAPRAX source resource command accepts at most 16 arguments\n");
    uint64_t total = UINT64_C(0);
    for (int index = 1; index < argc; ++index) {{
        const char *argument = argv[index];
        uint64_t length = UINT64_C(0);
        while (argument != NULL && length <= SPX_COMMAND_INPUT_CAPACITY_V1 && argument[length] != '\0') ++length;
        if (argument == NULL || length > SPX_COMMAND_INPUT_CAPACITY_V1 - total ||
            !spx_command_utf8_v1(length == UINT64_C(0) ? NULL : (const uint8_t *)argument, length))
            return spx_source_resource_usage_v1("SEMAPRAX source resource command arguments must be UTF-8 and at most 65536 bytes\n");
        input.arguments[input.argument_count++] = (spx_str_v1){{
            .data = length == UINT64_C(0) ? NULL : (const uint8_t *)argument, .len = length
        }};
        total += length;
    }}
    if (!spx_language_command_input_is_valid_v1(&input))
        return spx_source_resource_usage_v1("SEMAPRAX source resource command arguments are invalid\n");
    uint8_t *output_block = (uint8_t *)malloc((size_t)SPX_SOURCE_RESOURCE_OUTPUT_ALLOCATION_V1);
    if (output_block == NULL) {{
        fputs("SEMAPRAX native runtime invariant failure: source resource output allocation\n", stderr);
        return 72;
    }}
    memset(output_block, 0, (size_t)SPX_SOURCE_RESOURCE_OUTPUT_ALLOCATION_V1);
    state.command.output.stdout_bytes = output_block;
    state.command.output.stderr_bytes = output_block + (size_t)SPX_COMMAND_OUTPUT_CAPACITY_V1;
    state.command.input = &input;
{file_root_setup}
    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, &state)) {{
#if !defined(_WIN32)
        if (state.file_root >= 0) (void)close(state.file_root);
#endif
        spx_source_resource_release_v1(output_block);
        fputs("SEMAPRAX native runtime invariant failure: context initialization\n", stderr);
        return 72;
    }}
    int64_t result = INT64_C(0);
    spx_status_token status = {root_symbol}(&spx_ctx, &result);
#if !defined(_WIN32)
    if (state.file_root >= 0) (void)close(state.file_root);
#endif
    int exit_status = 1;
    if (status != SPX_STATUS_SUCCESS) {{
        (void)spx_public_failure(&spx_ctx, status);
    }} else if (result < INT64_C(0) || result > INT64_C(255)) {{
        fprintf(stderr, "SEMAPRAX source resource command: main returned %lld, outside the exit status range 0..=255\n", (long long)result);
    }} else if (spx_source_resource_flush_v1(
            stderr, state.command.output.stderr_bytes, state.command.output.stderr_length) &&
        spx_source_resource_flush_v1(
            stdout, state.command.output.stdout_bytes, state.command.output.stdout_length)) {{
        exit_status = (int)result;
    }}
    spx_source_resource_release_v1(output_block);
    return exit_status;
}}
#endif"#
    )
    .expect("writing the native source resource command adapter cannot fail");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit(body: &str) -> String {
        let source = format!(
            "module resource.native;\npermit {{ process.args.read, process.stdout.write }}\n\n@id(\"resource.main\")\nfn main() -> i64 uses {{ process.stdout.write }}\n{{\n{body}\n    0\n}}\n"
        );
        let parsed = crate::parse(&source, "resource-native.spx").unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        crate::codegen::emit_hir_c_with_source_resource_command(&resolved).unwrap()
    }

    #[test]
    fn resource_output_is_one_heap_block_with_closed_partitions_and_cleanup() {
        assert!(
            OUTPUT_RUNTIME_C.contains("#define SPX_COMMAND_OUTPUT_CAPACITY_V1 UINT64_C(1048576)")
        );
        assert!(OUTPUT_RUNTIME_C.contains(
            "value.len > SPX_COMMAND_OUTPUT_CAPACITY_V1 -\n            staging->stdout_length - staging->stderr_length"
        ));
        assert!(OUTPUT_RUNTIME_C.contains(
            "spx_host_command_output_append_v1(spx_ctx, value, false, false, result_out)"
        ));
        assert!(OUTPUT_RUNTIME_C.contains(
            "spx_host_command_output_append_v1(spx_ctx, value, false, true, result_out)"
        ));
        assert!(OUTPUT_RUNTIME_C.contains("return spx_source_resource_output_failure_v1(spx_ctx);"));
        assert!(!OUTPUT_RUNTIME_C.contains("combined command output capacity exceeded"));
        assert!(OUTPUT_RUNTIME_C.contains("else spx_slice_u8_require_valid(value);"));
        assert!(!OUTPUT_RUNTIME_C.contains("#define SPX_SLICE_U8_MAX_BYTES"));
        assert!(!OUTPUT_RUNTIME_C.contains("#define SPX_OWNED_BYTES_MAX_BYTES"));
        let output = emit("");
        assert_eq!(
            output
                .matches("malloc((size_t)SPX_SOURCE_RESOURCE_OUTPUT_ALLOCATION_V1)")
                .count(),
            1
        );
        assert!(output.contains(
            "state.command.output.stderr_bytes = output_block + (size_t)SPX_COMMAND_OUTPUT_CAPACITY_V1"
        ));
        assert!(output.contains("spx_source_resource_release_v1(output_block);"));
        assert!(
            output.find("status = spx_root").unwrap()
                < output.rfind("spx_source_resource_flush_v1(").unwrap()
        );
    }

    #[test]
    fn retained_borrowed_str_selects_wide_helpers_but_fixed_bytes_do_not() {
        let borrowed = emit(
            "    let text = \"wide\";\n    let view = string_as_str(text);\n    let written = stdout_append(str_as_bytes(view));",
        );
        assert!(borrowed
            .contains("spx_status = spx_host_command_stdout_append_resource_str_v1(spx_ctx,"));

        let ordinary = emit(
            "    let data = [1u8, 2u8];\n    let view = array_as_slice(data);\n    let written = stdout_append(view);",
        );
        assert!(ordinary.contains("spx_status = spx_host_command_stdout_append_v1(spx_ctx,"));
        assert!(!ordinary
            .contains("spx_status = spx_host_command_stdout_append_resource_str_v1(spx_ctx,"));
    }

    #[test]
    fn file_root_authority_requires_reachable_file_text() {
        let without_file = emit(
            "    let text = \"plain\";\n    let view = string_as_str(text);\n    let written = stdout_append(str_as_bytes(view));",
        );
        assert!(without_file.contains("state.file_root = -1;"));
        assert!(!without_file.contains("state.file_root = open(\".\""));

        let source = r#"module resource.native;
permit { fs.read, process.args.read, process.stdout.write }

@id("resource.main")
fn main() -> i64 uses { fs.read, process.args.read, process.stdout.write }
{
    let path = arg_utf8(0usize);
    let text = file_read_text(path);
    let view = string_as_str(text);
    let written = stdout_append(str_as_bytes(view));
    0
}
"#;
        let parsed = crate::parse(source, "resource-native-file.spx").unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        let with_file = crate::codegen::emit_hir_c_with_source_resource_command(&resolved).unwrap();
        assert!(with_file.contains("state.file_root = open(\".\""));
    }

    #[test]
    fn legacy_direct_write_uses_checked_resource_status_path() {
        let generated = emit(
            "    let text = \"checked\";\n    let view = string_as_str(text);\n    let written = stdout_write(str_as_bytes(view));",
        );
        assert!(generated.contains(
            "spx_status = spx_host_command_stdout_write_resource_str_checked_v1(spx_ctx,"
        ));
        assert!(generated.contains("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;"));
    }
}
