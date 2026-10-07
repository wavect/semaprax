//! Stream-profile-only Slice carrier with runtime-checked chunk epochs.
//!
//! This fragment is selected only by `language-command-io.stream.v1`. The
//! ordinary bytes profile keeps its original two-field Slice ABI and runtime
//! projection. The epoch pointer is created by the private stream provider;
//! host callbacks never receive or supply it.

use super::{BYTE_DATA_ALLOCATORS_C, BYTE_DATA_DROP_C, BYTE_DATA_OPERATIONS_C};
use crate::codegen::COutput;

pub(super) fn emit_runtime(output: &mut impl COutput) {
    output.push_str(STREAM_EPOCH_PREFIX_C);
    output.push_str(BYTE_DATA_ALLOCATORS_C);
    output.push_str(BYTE_DATA_OPERATIONS_C);
    output.push_str(BYTE_DATA_DROP_C);
}

const STREAM_EPOCH_PREFIX_C: &str = r#"#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define SPX_SLICE_U8_MAX_BYTES UINT64_C(65536)
#define SPX_OWNED_BYTES_MAX_BYTES UINT64_C(131072)

typedef struct {
    const uint8_t *ptr;
    uint64_t len;
    const uint64_t *epoch;
    uint64_t captured_epoch;
} spx_slice_u8_v1;

typedef struct {
    uint8_t *ptr;
    uint64_t len;
} spx_bytes_v1;

static __attribute__((unused)) void spx_slice_u8_require_current_epoch(
    spx_slice_u8_v1 value
) {
    if (value.epoch == NULL && value.captured_epoch != UINT64_C(0)) {
        spx_runtime_invariant_failure("unleased byte slice carries an epoch");
    }
    if (value.epoch != NULL && *value.epoch != value.captured_epoch) {
        spx_runtime_invariant_failure("borrowed byte slice epoch is stale");
    }
}

static __attribute__((unused)) void spx_slice_u8_require_shape(spx_slice_u8_v1 value) {
    if (value.len == UINT64_C(0)) {
        if (value.ptr != NULL) {
            spx_runtime_invariant_failure("empty borrowed byte slice is not normalized");
        }
    } else if (value.ptr == NULL) {
        spx_runtime_invariant_failure("non-empty borrowed byte slice has a null pointer");
    }
}

/* External roots stay at 64 KiB. A view derived from an owned Bytes value may
   instead span the internal 128 KiB owned-value bound. Epoch authentication
   runs before shape/length checks or any later access to the pointed bytes. */
static __attribute__((unused)) void spx_slice_u8_require_valid(spx_slice_u8_v1 value) {
    spx_slice_u8_require_current_epoch(value);
    if (value.len > SPX_SLICE_U8_MAX_BYTES) {
        spx_runtime_invariant_failure("borrowed byte slice exceeds the exact length bound");
    }
    spx_slice_u8_require_shape(value);
}

static __attribute__((unused)) void spx_slice_u8_require_owned_view_valid(spx_slice_u8_v1 value) {
    spx_slice_u8_require_current_epoch(value);
    if (value.len > SPX_OWNED_BYTES_MAX_BYTES) {
        spx_runtime_invariant_failure("owned byte view exceeds the exact length bound");
    }
    spx_slice_u8_require_shape(value);
}

static __attribute__((unused)) uint64_t spx_slice_u8_charge_root(
    uint64_t charged, spx_slice_u8_v1 value
) {
    spx_slice_u8_require_valid(value);
    if (value.len > SPX_SLICE_U8_MAX_BYTES - charged) {
        spx_runtime_invariant_failure("borrowed byte invocation exceeds the cumulative root bound");
    }
    return charged + value.len;
}

static __attribute__((unused)) uint64_t spx_byte_len(spx_slice_u8_v1 value) {
    spx_slice_u8_require_owned_view_valid(value);
    return value.len;
}

static __attribute__((unused)) spx_status_token spx_byte_range_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 value,
    uint64_t start,
    uint64_t end,
    spx_slice_u8_v1 *result_out
) {
    spx_slice_u8_require_owned_view_valid(value);
    if (result_out == NULL) {
        spx_runtime_invariant_failure("byte range result carrier is unavailable");
    }
    *result_out = (spx_slice_u8_v1){
        .ptr = NULL,
        .len = UINT64_C(0),
        .epoch = NULL,
        .captured_epoch = UINT64_C(0)
    };
    uint32_t failure_code = UINT32_C(0);
    if (start > end) {
        failure_code = UINT32_C(1);
    } else if (end > value.len) {
        failure_code = UINT32_C(2);
    }
    if (failure_code != UINT32_C(0)) {
        spx_status_token token = SPX_STATUS_SUCCESS;
        if (!spx_status_record_adapter(
            spx_ctx,
            "semaprax.byte-range.v1",
            failure_code,
            SPX_STATUS_CLASS_ADAPTER,
            SPX_RETRYABILITY_FALSE,
            &token
        )) {
            spx_runtime_invariant_failure("byte range status could not be recorded");
        }
        return token;
    }
    uint64_t length = end - start;
    result_out->ptr = length == UINT64_C(0) ? NULL : value.ptr + (size_t)start;
    result_out->len = length;
    result_out->epoch = value.epoch;
    result_out->captured_epoch = value.captured_epoch;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) void spx_bytes_require_valid(spx_bytes_v1 value) {
    if (value.len > SPX_OWNED_BYTES_MAX_BYTES) {
        spx_runtime_invariant_failure("owned bytes exceed the exact length bound");
    }
    if (value.len == UINT64_C(0)) {
        if (value.ptr != NULL) {
            spx_runtime_invariant_failure("empty owned bytes are not normalized");
        }
    } else if (value.ptr == NULL) {
        spx_runtime_invariant_failure("non-empty owned bytes have a null pointer");
    }
}

"#;

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn compile_and_run(body: &str, expect_success: bool) {
        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-native-slice-epoch-{}-{id}", std::process::id());
        let c_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        let mut source = C_STUBS.to_owned();
        source.push('\n');
        super::super::emit_stream_epoch_runtime(&mut source);
        source.push('\n');
        crate::codegen::native_host_output::emit_language_command_runtime(&mut source);
        source.push_str(body);
        std::fs::write(&c_path, source).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wno-unused-function",
                "-O2",
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "native Slice epoch fixture did not compile:\n{}\n{}",
            String::from_utf8_lossy(&compiled.stdout),
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&executable).output().unwrap();
        if expect_success {
            assert!(
                output.status.success(),
                "native Slice epoch fixture failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert_eq!(
                output.status.code(),
                Some(86),
                "stale native Slice did not take the invariant-failure path:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let _ = std::fs::remove_file(c_path);
        let _ = std::fs::remove_file(executable);
    }

    #[test]
    fn stream_epoch_is_checked_by_slice_consumers_and_derived_ranges() {
        for body in [
            r#"int main(void) {
    spx_slice_u8_v1 view = { .captured_epoch = UINT64_C(1) };
    (void)spx_byte_len(view);
    return 0;
}"#,
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(2);
    spx_slice_u8_v1 view = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(1) };
    (void)spx_byte_len(view);
    return 0;
}"#,
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(2);
    spx_slice_u8_v1 view = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(1) };
    spx_slice_u8_require_valid(view);
    (void)view.ptr[0];
    return 0;
}"#,
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(2);
    spx_slice_u8_v1 view = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(1) };
    struct spx_context context = {0};
    spx_slice_u8_v1 ranged = {0};
    (void)spx_byte_range_v1(&context, view, UINT64_C(0), UINT64_C(1), &ranged);
    return 0;
}"#,
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(2);
    spx_slice_u8_v1 view = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(1) };
    (void)spx_bytes_copy(view);
    return 0;
}"#,
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(2);
    spx_slice_u8_v1 view = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(1) };
    struct spx_command_output_staging_v1 staging = {0};
    struct spx_context context = { .target_state = &staging };
    (void)spx_host_command_stdout_write_v1(&context, view);
    return 0;
}"#,
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(4);
    spx_slice_u8_v1 view = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(4) };
    struct spx_context context = {0};
    spx_slice_u8_v1 ranged = {0};
    if (spx_byte_range_v1(&context, view, UINT64_C(0), UINT64_C(1), &ranged) != SPX_STATUS_SUCCESS) return 1;
    epoch = UINT64_C(5);
    (void)spx_byte_len(ranged);
    return 0;
}"#,
        ] {
            compile_and_run(body, false);
        }
    }

    #[test]
    fn stream_epoch_preserves_current_views_and_unleased_views() {
        compile_and_run(
            r#"int main(void) {
    uint8_t byte = UINT8_C(7);
    uint64_t epoch = UINT64_C(4);
    spx_slice_u8_v1 leased = { .ptr = &byte, .len = UINT64_C(1), .epoch = &epoch, .captured_epoch = UINT64_C(4) };
    spx_slice_u8_v1 ordinary = { .ptr = &byte, .len = UINT64_C(1), .epoch = NULL, .captured_epoch = UINT64_C(0) };
    if (spx_byte_len(leased) != UINT64_C(1) || spx_byte_len(ordinary) != UINT64_C(1)) return 1;
    return 0;
}"#,
            true,
        );
    }

    const C_STUBS: &str = r#"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint32_t spx_status_token;
#define SPX_STATUS_SUCCESS UINT32_C(0)
#define SPX_STATUS_CLASS_ADAPTER UINT32_C(1)
#define SPX_RETRYABILITY_FALSE UINT32_C(0)
struct spx_context { void *target_state; };

static void spx_runtime_invariant_failure(const char *message) {
    (void)message;
    exit(86);
}

static bool spx_status_record_adapter(
    struct spx_context *context, const char *domain, uint32_t code,
    uint32_t status_class, uint32_t retryability, spx_status_token *token_out
) {
    (void)context; (void)domain; (void)code; (void)status_class; (void)retryability;
    *token_out = SPX_STATUS_SUCCESS;
    return true;
}
"#;
}
