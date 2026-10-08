//! Project-v28 native String and borrowed-text bounds.
//!
//! These fragments are selected only by the additive resource-output profile.
//! Frozen native profiles retain their existing runtime text and 64-KiB
//! borrowed-carrier ceiling.

pub(super) const LENGTH_DELIMITED_RUNTIME_C: &str = r#"#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define SPX_SOURCE_RESOURCE_STRING_MAX_V1 UINT64_C(1048576)

struct spx_string_v10 { uint64_t len; char data[]; };
static __attribute__((unused)) struct spx_string_v10 *spx_string_header_v10(const char *value) {
    return (struct spx_string_v10 *)((uint8_t *)value - offsetof(struct spx_string_v10, data));
}
static __attribute__((unused)) uint64_t spx_string_length_v10(const char *value) {
    return spx_string_header_v10(value)->len;
}
static __attribute__((unused)) char *spx_string_from_literal(
    const char *spx_data, uint64_t spx_len
) {
    if (spx_len > SPX_SOURCE_RESOURCE_STRING_MAX_V1)
        spx_runtime_invariant_failure("string length exceeds source resource profile");
    if (spx_len > (uint64_t)SIZE_MAX - (uint64_t)offsetof(struct spx_string_v10, data) - UINT64_C(1))
        spx_runtime_invariant_failure("string allocation length overflow");
    struct spx_string_v10 *spx_value = (struct spx_string_v10 *)malloc(
        offsetof(struct spx_string_v10, data) + (size_t)spx_len + 1u
    );
    if (spx_value == NULL) spx_runtime_invariant_failure("string allocation failed");
    spx_value->len = spx_len;
    if (spx_len != UINT64_C(0)) memcpy(spx_value->data, spx_data, (size_t)spx_len);
    spx_value->data[spx_len] = '\0';
    return spx_value->data;
}
static __attribute__((unused)) char *spx_string_clone(const char *spx_source) {
    return spx_string_from_literal(spx_source, spx_string_length_v10(spx_source));
}
static __attribute__((unused)) bool spx_string_eq(const char *a, const char *b) {
    uint64_t a_len = spx_string_length_v10(a), b_len = spx_string_length_v10(b);
    return a_len == b_len && (a_len == UINT64_C(0) || memcmp(a, b, (size_t)a_len) == 0);
}
static __attribute__((unused)) void spx_string_drop(char *spx_value) {
    free(spx_string_header_v10(spx_value));
}
"#;

pub(super) const LENGTH_DELIMITED_OPS_RUNTIME_C: &str = r#"static __attribute__((unused)) int64_t spx_string_len(const char *spx_value) {
    uint64_t length = spx_string_length_v10(spx_value);
    if (length > (uint64_t)INT64_MAX) spx_runtime_invariant_failure("string length overflow");
    return (int64_t)length;
}
static __attribute__((unused)) bool spx_string_is_empty(const char *spx_value) {
    return spx_string_length_v10(spx_value) == UINT64_C(0);
}
static __attribute__((unused)) char *spx_string_concat(char *left, char *right) {
    uint64_t left_len = spx_string_length_v10(left), right_len = spx_string_length_v10(right);
    if (left_len > SPX_SOURCE_RESOURCE_STRING_MAX_V1 ||
        right_len > SPX_SOURCE_RESOURCE_STRING_MAX_V1 - left_len)
        spx_runtime_invariant_failure("string length exceeds source resource profile");
    uint64_t joined_len = left_len + right_len;
    char *joined;
    if (joined_len == UINT64_C(0)) joined = spx_string_from_literal("", UINT64_C(0));
    else {
        char *temporary = (char *)malloc((size_t)joined_len);
        if (temporary == NULL) spx_runtime_invariant_failure("string allocation failed");
        if (left_len != UINT64_C(0)) memcpy(temporary, left, (size_t)left_len);
        if (right_len != UINT64_C(0)) memcpy(temporary + left_len, right, (size_t)right_len);
        joined = spx_string_from_literal(temporary, joined_len);
        free(temporary);
    }
    return joined;
}
"#;

pub(super) const BORROWED_STR_RUNTIME_C: &str = r#"#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    const uint8_t *data;
    uint64_t len;
} spx_str_v1;

#define SPX_BORROWED_STR_MAX_BYTES UINT64_C(1048576)

static __attribute__((unused)) void spx_str_require_valid(spx_str_v1 value) {
    if (value.len != UINT64_C(0) && value.data == NULL)
        spx_runtime_invariant_failure("borrowed str has null data with nonzero length");
    if (value.len > SPX_BORROWED_STR_MAX_BYTES || value.len > (uint64_t)SIZE_MAX ||
        value.len > (uint64_t)INT64_MAX)
        spx_runtime_invariant_failure("borrowed str length exceeds native profile");
    uint64_t offset = UINT64_C(0);
    while (offset < value.len) {
        const uint8_t first = value.data[offset];
        uint64_t width = UINT64_C(0);
        if (first <= UINT8_C(0x7f)) width = UINT64_C(1);
        else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) width = UINT64_C(2);
        else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) width = UINT64_C(3);
        else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) width = UINT64_C(4);
        else spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
        if (width > value.len - offset)
            spx_runtime_invariant_failure("borrowed str has truncated UTF-8");
        if (width >= UINT64_C(2)) {
            const uint8_t second = value.data[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80) ||
                (first == UINT8_C(0xe0) && second < UINT8_C(0xa0)) ||
                (first == UINT8_C(0xed) && second > UINT8_C(0x9f)) ||
                (first == UINT8_C(0xf0) && second < UINT8_C(0x90)) ||
                (first == UINT8_C(0xf4) && second > UINT8_C(0x8f)))
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
        }
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail)
            if ((value.data[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80))
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
        offset += width;
    }
}

static __attribute__((unused)) int64_t spx_str_len_bytes(spx_str_v1 value) {
    spx_str_require_valid(value);
    return (int64_t)value.len;
}
static __attribute__((unused)) bool spx_str_is_empty(spx_str_v1 value) {
    spx_str_require_valid(value);
    return value.len == UINT64_C(0);
}
static __attribute__((unused)) bool spx_str_starts_with(spx_str_v1 value, spx_str_v1 prefix) {
    spx_str_require_valid(value);
    spx_str_require_valid(prefix);
    return prefix.len <= value.len &&
        (prefix.len == UINT64_C(0) || memcmp(value.data, prefix.data, (size_t)prefix.len) == 0);
}
static __attribute__((unused)) bool spx_str_contains(spx_str_v1 value, spx_str_v1 needle) {
    spx_str_require_valid(value);
    spx_str_require_valid(needle);
    if (needle.len == UINT64_C(0)) return true;
    if (needle.len > value.len) return false;
    uint32_t *prefix = (uint32_t *)malloc((size_t)needle.len * sizeof(uint32_t));
    if (prefix == NULL) spx_runtime_invariant_failure("borrowed str search allocation failed");
    prefix[0] = UINT32_C(0);
    uint64_t matched = UINT64_C(0);
    for (uint64_t index = UINT64_C(1); index < needle.len; ++index) {
        while (matched != UINT64_C(0) && needle.data[matched] != needle.data[index])
            matched = (uint64_t)prefix[matched - UINT64_C(1)];
        if (needle.data[matched] == needle.data[index]) ++matched;
        prefix[index] = (uint32_t)matched;
    }
    matched = UINT64_C(0);
    for (uint64_t index = UINT64_C(0); index < value.len; ++index) {
        while (matched != UINT64_C(0) && needle.data[matched] != value.data[index])
            matched = (uint64_t)prefix[matched - UINT64_C(1)];
        if (needle.data[matched] == value.data[index] && ++matched == needle.len) {
            free(prefix);
            return true;
        }
    }
    free(prefix);
    return false;
}
"#;
