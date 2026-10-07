//! Reachability-gated C11 support for borrowed byte slices.
//!
//! The carrier is length-aware and deliberately unrelated to the owned UTF-8
//! string runtime. C cannot authenticate an arbitrary non-null host pointer;
//! callers remain responsible for readable storage across the admitted range.

pub(super) fn emit_runtime(output: &mut impl super::COutput) {
    output.push_str(BYTE_DATA_PREFIX_C);
    output.push_str(BYTE_DATA_ALLOCATORS_C);
    output.push_str(BYTE_DATA_OPERATIONS_C);
    output.push_str(BYTE_DATA_DROP_C);
}

/// Emit the additive Slice carrier used only by the explicit stdin-stream
/// profile. The default carrier and its frozen projection remain untouched.
pub(super) fn emit_stream_epoch_runtime(output: &mut impl super::COutput) {
    stream_epoch::emit_runtime(output);
}

pub(super) fn emit_stream_epoch_additive_operations(output: &mut impl super::COutput) {
    output.push_str(BYTE_DATA_ADDITIVE_OPERATIONS_C);
}

#[path = "native_byte_data/stream_epoch.rs"]
mod stream_epoch;

/// Emit byte-buffer store helpers introduced after the frozen default runtime.
///
/// These functions depend only on the default byte runtime. Keeping them in a
/// separately selected fragment preserves the exact legacy projection used by
/// existing generated packages.
pub(super) fn emit_additive_operations(output: &mut impl super::COutput) {
    output.push_str(BYTE_DATA_ADDITIVE_OPERATIONS_C);
}

pub(super) fn emit_reserved_runtime(output: &mut impl super::COutput) {
    output.push_str(BYTE_DATA_PREFIX_C);
    output.push_str(include_str!("native_byte_data/reserved_allocators.c"));
    output.push_str("\n");
    output.push_str(BYTE_DATA_OPERATIONS_C);
    output.push_str(include_str!("native_byte_data/reserved_drop.c"));
}

const BYTE_DATA_PREFIX_C: &str = r#"#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define SPX_SLICE_U8_MAX_BYTES UINT64_C(65536)
#define SPX_OWNED_BYTES_MAX_BYTES UINT64_C(131072)

typedef struct {
    const uint8_t *ptr;
    uint64_t len;
} spx_slice_u8_v1;

typedef struct {
    uint8_t *ptr;
    uint64_t len;
} spx_bytes_v1;

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
   instead span the internal 128 KiB owned-value bound. */
static __attribute__((unused)) void spx_slice_u8_require_valid(spx_slice_u8_v1 value) {
    if (value.len > SPX_SLICE_U8_MAX_BYTES) {
        spx_runtime_invariant_failure("borrowed byte slice exceeds the exact length bound");
    }
    spx_slice_u8_require_shape(value);
}

static __attribute__((unused)) void spx_slice_u8_require_owned_view_valid(spx_slice_u8_v1 value) {
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
    *result_out = (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
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

const BYTE_DATA_ALLOCATORS_C: &str = r#"static __attribute__((unused)) spx_bytes_v1 spx_bytes_copy(spx_slice_u8_v1 value) {
    spx_slice_u8_require_owned_view_valid(value);
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

static __attribute__((unused)) spx_bytes_v1 spx_bytes_zeroed(uint64_t count) {
    if (count > SPX_OWNED_BYTES_MAX_BYTES) {
        spx_runtime_invariant_failure("owned byte buffer capacity exceeds the exact length bound");
    }
    if (count == UINT64_C(0)) {
        return (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    }
    uint8_t *payload = (uint8_t *)calloc((size_t)count, sizeof(uint8_t));
    if (payload == NULL) {
        spx_runtime_invariant_failure("owned byte buffer allocation failed");
    }
    return (spx_bytes_v1){ .ptr = payload, .len = count };
}

"#;

const BYTE_DATA_OPERATIONS_C: &str = r#"/* The element index is any admitted `usize` expression, so the bound is
   checked before the owner transfer commits. A failed store selects the single
   `semaprax.byte-buffer.v1` failure and writes nothing; the buffer is still
   held by its canonical cleanup-plan call-argument slot, which frees it on the
   epilogue exactly once. */
static __attribute__((unused)) spx_status_token spx_bytes_set_check_v1(
    struct spx_context *spx_ctx, spx_bytes_v1 buffer, uint64_t index
) {
    spx_bytes_require_valid(buffer);
    if (index < buffer.len) {
        return SPX_STATUS_SUCCESS;
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        "semaprax.byte-buffer.v1",
        UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("owned byte buffer status could not be recorded");
    }
    return token;
}

/* The buffer is transferred in and handed straight back: one fill mutates the
   single live owner in place and never allocates. The caller's carrier is
   emptied by the ordinary canonical cleanup-plan transfer, so the payload never
   has a second owner. `spx_bytes_set_check_v1` has already selected a failure
   for an out-of-range index, so reaching one here is a compiler defect and is
   a runtime invariant failure rather than a silent truncation. */
static __attribute__((unused)) spx_bytes_v1 spx_bytes_set(
    spx_bytes_v1 buffer, uint64_t index, uint8_t value
) {
    spx_bytes_require_valid(buffer);
    if (index >= buffer.len) {
        spx_runtime_invariant_failure("owned byte buffer element index is outside its capacity");
    }
    buffer.ptr[(size_t)index] = value;
    return buffer;
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

"#;

const BYTE_DATA_ADDITIVE_OPERATIONS_C: &str = r#"/* Five-byte stores use the same status but preflight the whole interval before
   ownership commits, so no failed call can publish a partial prefix. */
static __attribute__((unused)) spx_status_token spx_bytes_set5_check_v1(
    struct spx_context *spx_ctx, spx_bytes_v1 buffer, uint64_t index
) {
    spx_bytes_require_valid(buffer);
    if (index <= buffer.len && buffer.len - index >= UINT64_C(5)) {
        return SPX_STATUS_SUCCESS;
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        "semaprax.byte-buffer.v1",
        UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("owned byte buffer status could not be recorded");
    }
    return token;
}

/* The one-or-five path has the same destination status as the fixed stores.
   Its borrowed source is read only after the destination preflight; absent
   source bytes are deterministic zero rather than a second failure family. */
static __attribute__((unused)) spx_status_token spx_bytes_set1_or5_check_v1(
    struct spx_context *spx_ctx, spx_bytes_v1 buffer, uint64_t selector, uint64_t index
) {
    return (selector & (UINT64_C(1) << 63)) != 0
        ? spx_bytes_set5_check_v1(spx_ctx, buffer, index)
        : spx_bytes_set_check_v1(spx_ctx, buffer, index);
}

static __attribute__((unused)) spx_bytes_v1 spx_bytes_set1_or5(
    spx_bytes_v1 buffer, uint64_t index, uint8_t one,
    spx_slice_u8_v1 source, uint64_t selector
) {
    spx_bytes_require_valid(buffer);
    spx_slice_u8_require_owned_view_valid(source);
    bool wide = (selector & (UINT64_C(1) << 63)) != 0;
    uint64_t source_start = selector & ~(UINT64_C(1) << 63);
    uint64_t width = wide ? UINT64_C(5) : UINT64_C(1);
    if (index > buffer.len || buffer.len - index < width) {
        spx_runtime_invariant_failure("owned byte buffer one-or-five interval is outside its capacity");
    }
    size_t slot = (size_t)index;
    if (!wide) {
        buffer.ptr[slot] = one;
        return buffer;
    }
    for (size_t offset = 0U; offset < 5U; offset++) {
        uint64_t source_index = source_start + (uint64_t)offset;
        buffer.ptr[slot + offset] = source_index >= source_start && source_index < source.len
            ? source.ptr[(size_t)source_index] : UINT8_C(0);
    }
    return buffer;
}

/* Bit 63 selects a source copy; bit 62 selects its forty-eight-byte width
   instead of six. The low 62 bits are the source offset. */
static __attribute__((unused)) spx_status_token spx_bytes_set1_or6_or48_check_v1(
    struct spx_context *spx_ctx, spx_bytes_v1 buffer, uint64_t selector, uint64_t index
) {
    if ((selector & (UINT64_C(1) << 63)) == 0) {
        return spx_bytes_set_check_v1(spx_ctx, buffer, index);
    }
    spx_bytes_require_valid(buffer);
    uint64_t width = (selector & (UINT64_C(1) << 62)) != 0
        ? UINT64_C(48) : UINT64_C(6);
    if (index <= buffer.len && buffer.len - index >= width) {
        return SPX_STATUS_SUCCESS;
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        "semaprax.byte-buffer.v1",
        UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("owned byte buffer status could not be recorded");
    }
    return token;
}

static __attribute__((unused)) spx_bytes_v1 spx_bytes_set1_or6_or48(
    spx_bytes_v1 buffer, uint64_t index, uint8_t one,
    spx_slice_u8_v1 source, uint64_t selector
) {
    spx_bytes_require_valid(buffer);
    spx_slice_u8_require_owned_view_valid(source);
    bool copy = (selector & (UINT64_C(1) << 63)) != 0;
    bool wide48 = (selector & (UINT64_C(1) << 62)) != 0;
    uint64_t source_start = selector & ((UINT64_C(1) << 62) - UINT64_C(1));
    uint64_t width = copy ? (wide48 ? UINT64_C(48) : UINT64_C(6)) : UINT64_C(1);
    if (index > buffer.len || buffer.len - index < width) {
        spx_runtime_invariant_failure("owned byte buffer one-or-six-or-forty-eight interval is outside its capacity");
    }
    size_t slot = (size_t)index;
    if (!copy) {
        buffer.ptr[slot] = one;
        return buffer;
    }
    for (size_t offset = 0U; offset < (size_t)width; offset++) {
        uint64_t source_index = source_start + (uint64_t)offset;
        buffer.ptr[slot + offset] = source_index >= source_start && source_index < source.len
            ? source.ptr[(size_t)source_index] : UINT8_C(0);
    }
    return buffer;
}

static __attribute__((unused)) spx_bytes_v1 spx_bytes_set5(
    spx_bytes_v1 buffer, uint64_t index, uint8_t first, uint8_t second,
    uint8_t third, uint8_t fourth, uint8_t fifth
) {
    spx_bytes_require_valid(buffer);
    if (index > buffer.len || buffer.len - index < UINT64_C(5)) {
        spx_runtime_invariant_failure("owned byte buffer five-byte interval is outside its capacity");
    }
    size_t slot = (size_t)index;
    buffer.ptr[slot] = first;
    buffer.ptr[slot + 1U] = second;
    buffer.ptr[slot + 2U] = third;
    buffer.ptr[slot + 3U] = fourth;
    buffer.ptr[slot + 4U] = fifth;
    return buffer;
}

"#;

const BYTE_DATA_DROP_C: &str = r#"static __attribute__((unused)) void spx_bytes_drop(spx_bytes_v1 *value) {
    if (value == NULL) {
        spx_runtime_invariant_failure("owned byte drop has a null carrier");
    }
    spx_bytes_require_valid(*value);
    free(value->ptr);
    value->ptr = NULL;
    value->len = UINT64_C(0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest as _, Sha256};

    #[test]
    fn default_emission_matches_frozen_runtime_bytes() {
        // The pre-split literal at 69e0b65b, independently hashed before editing.
        const FROZEN: &str = "f5f05852a39e264dac30ce5d5c37809b35cf6faf18ef610aea84ae92641f334f";
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        let mut emitted = String::new();
        emit_runtime(&mut emitted);
        assert_eq!(emitted.len(), 7_489);
        assert_eq!(digest(&emitted), FROZEN);
        // The oracle must reject changed emitter order and an omitted fragment,
        // even though the individual source constants remain unchanged.
        let reordered = [
            BYTE_DATA_PREFIX_C,
            BYTE_DATA_OPERATIONS_C,
            BYTE_DATA_ALLOCATORS_C,
            BYTE_DATA_DROP_C,
        ]
        .concat();
        assert_eq!(reordered.len(), emitted.len());
        assert_ne!(digest(&reordered), FROZEN);
        let omitted = [BYTE_DATA_PREFIX_C, BYTE_DATA_ALLOCATORS_C, BYTE_DATA_DROP_C].concat();
        assert_ne!(digest(&omitted), FROZEN);
    }

    #[test]
    fn additive_store_helpers_are_emitted_only_for_reachable_operations() {
        let legacy = crate::check(
            "module test.byte_runtime_legacy; @id(\"app.main\") fn main() -> i64 { let buffer = bytes_set(bytes_zeroed(1usize), 0usize, 1u8); let view = bytes_as_slice(buffer); if byte_len(view) == 1usize { 0 } else { 1 } }",
            "byte-runtime-legacy.spx",
        )
        .unwrap();
        let legacy_c = crate::codegen::emit_c(&legacy).unwrap();
        assert!(!legacy_c.contains("spx_bytes_set5_check_v1"));
        assert!(!legacy_c.contains("spx_bytes_set1_or5_check_v1"));
        assert!(!legacy_c.contains("spx_bytes_set1_or6_or48_check_v1"));

        for (source, invoked) in [
            (
                "module test.byte_runtime_set5; @id(\"app.main\") fn main() -> i64 { let buffer = bytes_set5(bytes_zeroed(5usize), 0usize, 1u8, 2u8, 3u8, 4u8, 5u8); let view = bytes_as_slice(buffer); if byte_len(view) == 5usize { 0 } else { 1 } }",
                "spx_bytes_set5(spx_bytes_move",
            ),
            (
                "module test.byte_runtime_set1_or5; @id(\"app.main\") fn main() -> i64 { let raw = [1u8]; let source = array_as_slice(raw); let buffer = bytes_set1_or5_from_slice(bytes_zeroed(5usize), 0usize, 1u8, source, 9223372036854775808usize); let view = bytes_as_slice(buffer); if byte_len(view) == 5usize { 0 } else { 1 } }",
                "spx_bytes_set1_or5(spx_bytes_move",
            ),
            (
                "module test.byte_runtime_set1_or6_or48; @id(\"app.main\") fn main() -> i64 { let raw = [1u8]; let source = array_as_slice(raw); let buffer = bytes_set1_or6_or48_from_slice(bytes_zeroed(48usize), 0usize, 1u8, source, 13835058055282163712usize); let view = bytes_as_slice(buffer); if byte_len(view) == 48usize { 0 } else { 1 } }",
                "spx_bytes_set1_or6_or48(spx_bytes_move",
            ),
        ] {
            let program = crate::check(source, "byte-runtime-additive.spx").unwrap();
            let emitted = crate::codegen::emit_c(&program).unwrap();
            assert!(emitted.contains(invoked), "missing selected helper for {invoked}");
            assert!(emitted.contains("spx_bytes_set5_check_v1"));
            assert!(emitted.contains("spx_bytes_set1_or5_check_v1"));
            assert!(emitted.contains("spx_bytes_set1_or6_or48_check_v1"));
        }
    }
}
