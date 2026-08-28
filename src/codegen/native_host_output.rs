//! Bounded, invocation-local native stdout transcript support.
//!
//! This is deliberately a memory sink. It never calls libc I/O and publishes
//! bytes to caller-owned storage only after the root function succeeds.

use super::COutput;

pub(super) const RUN_SYMBOL: &str = "spx_stdout_transcript_run_v1";

/// Emit the additive two-channel staging runtime used only by Bounded
/// Language Command I/O v1. The legacy stdout-only runtime above is left
/// byte-for-byte unchanged: this profile needs a combined output budget and
/// therefore cannot safely reinterpret its `target_state` carrier.
pub(super) fn emit_language_command_runtime(output: &mut impl COutput) {
    output.push_str(
        r#"#define SPX_COMMAND_OUTPUT_CAPACITY_V1 UINT64_C(65536)

struct spx_command_output_staging_v1 {
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
    uint8_t stderr_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
};

static uint64_t spx_host_command_output_write_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value,
    bool stderr_channel
) {
    spx_slice_u8_require_valid(value);
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("command output state is unavailable");
    }
    struct spx_command_output_staging_v1 *staging =
        (struct spx_command_output_staging_v1 *)spx_ctx->target_state;
    uint64_t other = stderr_channel ? staging->stdout_length : staging->stderr_length;
    if (other > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        value.len > SPX_COMMAND_OUTPUT_CAPACITY_V1 - other) {
        spx_runtime_invariant_failure("combined command output capacity exceeded");
    }
    uint8_t *destination = stderr_channel ? staging->stderr_bytes : staging->stdout_bytes;
    if (value.len != UINT64_C(0)) {
        memcpy(destination, value.ptr, (size_t)value.len);
    }
    if (stderr_channel) {
        staging->stderr_length = value.len;
    } else {
        staging->stdout_length = value.len;
    }
    return value.len;
}

static uint64_t spx_host_command_stdout_write_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value
) {
    return spx_host_command_output_write_v1(spx_ctx, value, false);
}

static uint64_t spx_host_command_stderr_write_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value
) {
    return spx_host_command_output_write_v1(spx_ctx, value, true);
}

"#,
    );
}

/// Emit the additive line-command output runtime. Legacy command generation
/// continues to call `emit_language_command_runtime`, keeping its C projection
/// unchanged while this profile gains fallible cumulative append semantics.
pub(super) fn emit_line_command_runtime(output: &mut impl COutput) {
    output.push_str(
        r##"#define SPX_COMMAND_OUTPUT_CAPACITY_V1 UINT64_C(65536)
#define SPX_COMMAND_OUTPUT_STATUS_DOMAIN_V1 "semaprax.command-output.v1"
#define SPX_COMMAND_OUTPUT_CAPACITY_FAILURE_V1 UINT32_C(1)

struct spx_command_output_staging_v1 {
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
    uint8_t stderr_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
};

static __attribute__((unused)) spx_status_token spx_host_command_output_append_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value,
    bool stderr_channel,
    uint64_t *result_out
) {
    spx_slice_u8_require_valid(value);
    if (spx_ctx == NULL || spx_ctx->target_state == NULL || result_out == NULL) {
        spx_runtime_invariant_failure("command append state is unavailable");
    }
    *result_out = UINT64_C(0);
    struct spx_command_output_staging_v1 *staging =
        (struct spx_command_output_staging_v1 *)spx_ctx->target_state;
    if (staging->stdout_length > SPX_COMMAND_OUTPUT_CAPACITY_V1 ||
        staging->stderr_length >
            SPX_COMMAND_OUTPUT_CAPACITY_V1 - staging->stdout_length ||
        value.len > SPX_COMMAND_OUTPUT_CAPACITY_V1 -
            staging->stdout_length - staging->stderr_length) {
        spx_status_token token = SPX_STATUS_SUCCESS;
        if (!spx_status_record_adapter(
            spx_ctx,
            SPX_COMMAND_OUTPUT_STATUS_DOMAIN_V1,
            SPX_COMMAND_OUTPUT_CAPACITY_FAILURE_V1,
            SPX_STATUS_CLASS_ADAPTER,
            SPX_RETRYABILITY_FALSE,
            &token
        )) {
            spx_runtime_invariant_failure("command output status could not be recorded");
        }
        return token;
    }
    uint64_t offset = stderr_channel ? staging->stderr_length : staging->stdout_length;
    uint8_t *destination = stderr_channel ? staging->stderr_bytes : staging->stdout_bytes;
    if (value.len != UINT64_C(0)) {
        memcpy(destination + (size_t)offset, value.ptr, (size_t)value.len);
    }
    if (stderr_channel) {
        staging->stderr_length += value.len;
    } else {
        staging->stdout_length += value.len;
    }
    *result_out = value.len;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_command_stdout_append_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value,
    uint64_t *result_out
) {
    return spx_host_command_output_append_v1(spx_ctx, value, false, result_out);
}

static __attribute__((unused)) spx_status_token spx_host_command_stderr_append_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value,
    uint64_t *result_out
) {
    return spx_host_command_output_append_v1(spx_ctx, value, true, result_out);
}

"##,
    );
}

pub(super) fn emit_runtime(output: &mut impl COutput) {
    output.push_str(
        r#"#define SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 UINT64_C(65536)

struct spx_stdout_staging_v1 {
    uint64_t length;
    uint8_t bytes[SPX_STDOUT_TRANSCRIPT_CAPACITY_V1];
};

struct spx_stdout_transcript_result_v1 {
    int64_t value;
    uint64_t transcript_length;
    uint8_t transcript[SPX_STDOUT_TRANSCRIPT_CAPACITY_V1];
};

static uint64_t spx_host_stdout_write_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value
) {
    spx_slice_u8_require_valid(value);
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("stdout transcript state is unavailable");
    }
    struct spx_stdout_staging_v1 *staging =
        (struct spx_stdout_staging_v1 *)spx_ctx->target_state;
    if (value.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1) {
        spx_runtime_invariant_failure("stdout transcript capacity exceeded");
    }
    if (value.len != UINT64_C(0)) {
        memcpy(staging->bytes, value.ptr, (size_t)value.len);
    }
    staging->length = value.len;
    return value.len;
}

"#,
    );
}

pub(super) fn emit_root_wrapper(output: &mut impl COutput, root_symbol: &str) {
    writeln!(
        output,
        r#"int {RUN_SYMBOL}(struct spx_stdout_transcript_result_v1 *result_out) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
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
    int64_t value = INT64_C(0);
    spx_status_token status = {root_symbol}(&spx_ctx, &value);
    if (status != SPX_STATUS_SUCCESS) {{
        memset(&staging, 0, sizeof(staging));
        return 0;
    }}
    result_out->value = value;
    if (staging.length != UINT64_C(0)) {{
        memcpy(result_out->transcript, staging.bytes, (size_t)staging.length);
    }}
    result_out->transcript_length = staging.length;
    memset(&staging, 0, sizeof(staging));
    return 1;
}}
"#
    )
    .expect("writing native stdout wrapper cannot fail");
}
