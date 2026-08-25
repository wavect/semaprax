//! Reachability-gated C11 support for borrowed byte slices.
//!
//! The carrier is length-aware and deliberately unrelated to the owned UTF-8
//! string runtime. C cannot authenticate an arbitrary non-null host pointer;
//! callers remain responsible for readable storage across the admitted range.

pub(super) fn emit_runtime(output: &mut impl super::COutput) {
    output.push_str(NATIVE_BYTE_DATA_RUNTIME_C);
}

const NATIVE_BYTE_DATA_RUNTIME_C: &str = r#"#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define SPX_SLICE_U8_MAX_BYTES UINT64_C(65536)

typedef struct {
    const uint8_t *ptr;
    uint64_t len;
} spx_slice_u8_v1;

typedef struct {
    uint8_t *ptr;
    uint64_t len;
} spx_bytes_v1;

static __attribute__((unused)) void spx_slice_u8_require_valid(spx_slice_u8_v1 value) {
    if (value.len > SPX_SLICE_U8_MAX_BYTES) {
        spx_runtime_invariant_failure("borrowed byte slice exceeds the exact length bound");
    }
    if (value.len == UINT64_C(0)) {
        if (value.ptr != NULL) {
            spx_runtime_invariant_failure("empty borrowed byte slice is not normalized");
        }
    } else if (value.ptr == NULL) {
        spx_runtime_invariant_failure("non-empty borrowed byte slice has a null pointer");
    }
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
    spx_slice_u8_require_valid(value);
    return value.len;
}

static __attribute__((unused)) void spx_bytes_require_valid(spx_bytes_v1 value) {
    if (value.len > SPX_SLICE_U8_MAX_BYTES) {
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

static __attribute__((unused)) spx_bytes_v1 spx_bytes_copy(spx_slice_u8_v1 value) {
    spx_slice_u8_require_valid(value);
    if (value.len == UINT64_C(0)) {
        return (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    }
    uint8_t *payload = (uint8_t *)malloc((size_t)value.len);
    if (payload == NULL) {
        spx_runtime_invariant_failure("owned byte allocation failed");
    }
    memcpy(payload, value.ptr, (size_t)value.len);
    return (spx_bytes_v1){ .ptr = payload, .len = value.len };
}

static __attribute__((unused)) spx_slice_u8_v1 spx_bytes_as_slice(
    const spx_bytes_v1 *value
) {
    if (value == NULL) {
        spx_runtime_invariant_failure("owned byte borrow has a null carrier");
    }
    spx_bytes_require_valid(*value);
    return (spx_slice_u8_v1){ .ptr = value->ptr, .len = value->len };
}

static __attribute__((unused)) spx_bytes_v1 spx_bytes_move(spx_bytes_v1 *source) {
    if (source == NULL) {
        spx_runtime_invariant_failure("owned byte move has a null carrier");
    }
    spx_bytes_require_valid(*source);
    spx_bytes_v1 moved = *source;
    source->ptr = NULL;
    source->len = UINT64_C(0);
    return moved;
}

static __attribute__((unused)) void spx_bytes_drop(spx_bytes_v1 *value) {
    if (value == NULL) {
        spx_runtime_invariant_failure("owned byte drop has a null carrier");
    }
    spx_bytes_require_valid(*value);
    free(value->ptr);
    value->ptr = NULL;
    value->len = UINT64_C(0);
}
"#;
