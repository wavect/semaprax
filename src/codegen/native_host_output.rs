//! Bounded, invocation-local native stdout transcript support.
//!
//! This is deliberately a memory sink. It never calls libc I/O and publishes
//! bytes to caller-owned storage only after the root function succeeds.

use super::COutput;

pub(super) const RUN_SYMBOL: &str = "spx_stdout_transcript_run_v1";

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
