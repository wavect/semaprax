//! Reachability-gated C11 runtime for the internal Owned Bounded Vec v1 lane.

pub(super) fn emit_runtime(output: &mut impl super::COutput) {
    output.push_str(NATIVE_VEC_RUNTIME_C);
}

const NATIVE_VEC_RUNTIME_C: &str = r#"#include <stddef.h>
#include <stdlib.h>

#define SPX_VEC_MAX_CAPACITY UINT64_C(8192)

typedef struct {
    uint64_t *ptr;
    uint64_t len;
    uint64_t capacity;
    uint64_t generation;
    uint32_t type_tag;
    uint32_t authority;
} spx_vec_v1;

static __attribute__((unused)) uint64_t spx_vec_f32_bits(float value) {
    union { float value; uint32_t bits; } carrier = { .value = value };
    return (uint64_t)carrier.bits;
}
static __attribute__((unused)) uint64_t spx_vec_f64_bits(double value) {
    union { double value; uint64_t bits; } carrier = { .value = value };
    return carrier.bits;
}
static __attribute__((unused)) float spx_vec_bits_f32(uint64_t bits) {
    union { float value; uint32_t bits; } carrier = { .bits = (uint32_t)bits };
    return carrier.value;
}
static __attribute__((unused)) double spx_vec_bits_f64(uint64_t bits) {
    union { double value; uint64_t bits; } carrier = { .bits = bits };
    return carrier.value;
}

static __attribute__((unused)) struct spx_vec_authority_entry *spx_vec_require_valid(
    struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || value == NULL
        || tag < UINT32_C(1) || tag > UINT32_C(8)
        || value->type_tag != tag || value->generation == UINT64_C(0)
        || value->authority == UINT32_C(0)
        || value->authority > SPX_VEC_AUTHORITY_CAPACITY
        || value->capacity > SPX_VEC_MAX_CAPACITY || value->len > value->capacity
        || ((value->capacity == UINT64_C(0)) != (value->ptr == NULL))) {
        spx_runtime_invariant_failure("invalid owned bounded Vec carrier");
    }
    struct spx_vec_authority_entry *entry =
        &spx_ctx->vec_authority[value->authority - UINT32_C(1)];
    if (!entry->live || entry->ptr != value->ptr || entry->len != value->len
        || entry->capacity != value->capacity || entry->generation != value->generation
        || entry->type_tag != value->type_tag) {
        spx_runtime_invariant_failure("stale or forged owned bounded Vec carrier");
    }
    return entry;
}

static __attribute__((unused)) uint64_t spx_vec_next_generation(struct spx_context *spx_ctx) {
    if (spx_ctx->vec_next_generation == UINT64_C(0)
        || spx_ctx->vec_next_generation == UINT64_MAX) {
        spx_runtime_invariant_failure("bounded Vec generation authority exhausted");
    }
    return spx_ctx->vec_next_generation++;
}

static __attribute__((unused)) uint32_t spx_vec_register(
    struct spx_context *spx_ctx, uint64_t *ptr, uint64_t capacity, uint32_t tag,
    uint64_t generation
) {
    for (uint32_t index = UINT32_C(0); index < SPX_VEC_AUTHORITY_CAPACITY; ++index) {
        struct spx_vec_authority_entry *entry = &spx_ctx->vec_authority[index];
        if (!entry->live) {
            entry->ptr = ptr; entry->len = UINT64_C(0); entry->capacity = capacity;
            entry->generation = generation; entry->type_tag = tag; entry->live = true;
            return index + UINT32_C(1);
        }
    }
    return UINT32_C(0);
}

static __attribute__((unused)) spx_status_token spx_vec_failure(
    struct spx_context *spx_ctx, uint32_t code
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(spx_ctx, "semaprax.vec.v1", code,
        SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token)) {
        spx_runtime_invariant_failure("bounded Vec status could not be recorded");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_vec_with_capacity(
    struct spx_context *spx_ctx, uint32_t tag, uint64_t capacity, spx_vec_v1 *result
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED
        || result == NULL || tag < UINT32_C(1) || tag > UINT32_C(8)) {
        spx_runtime_invariant_failure("invalid bounded Vec allocation request");
    }
    *result = (spx_vec_v1){0};
    if (capacity > SPX_VEC_MAX_CAPACITY) return spx_vec_failure(spx_ctx, UINT32_C(3));
    uint64_t *payload = capacity == UINT64_C(0)
        ? NULL : (uint64_t *)calloc((size_t)capacity, sizeof(uint64_t));
    if (capacity != UINT64_C(0) && payload == NULL) return spx_vec_failure(spx_ctx, UINT32_C(3));
    uint64_t generation = spx_vec_next_generation(spx_ctx);
    uint32_t authority = spx_vec_register(spx_ctx, payload, capacity, tag, generation);
    if (authority == UINT32_C(0)) {
        free(payload); return spx_vec_failure(spx_ctx, UINT32_C(3));
    }
    result->ptr = payload; result->capacity = capacity; result->generation = generation;
    result->authority = authority; result->type_tag = tag;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_vec_push(
    struct spx_context *spx_ctx, uint32_t tag, spx_vec_v1 *source,
    uint64_t bits, spx_vec_v1 *result
) {
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, source, tag);
    if (result == NULL || result == source) spx_runtime_invariant_failure("invalid bounded Vec push result");
    if (source->len == source->capacity) return spx_vec_failure(spx_ctx, UINT32_C(1));
    source->ptr[source->len] = bits; source->len += UINT64_C(1);
    source->generation = spx_vec_next_generation(spx_ctx);
    entry->len = source->len; entry->generation = source->generation;
    result->ptr = source->ptr; result->len = source->len; result->capacity = source->capacity;
    result->generation = source->generation; result->authority = source->authority;
    result->type_tag = source->type_tag; *source = (spx_vec_v1){0};
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) uint64_t spx_vec_len(struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag) {
    (void)spx_vec_require_valid(spx_ctx, value, tag); return value->len;
}
static __attribute__((unused)) uint64_t spx_vec_capacity(struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag) {
    (void)spx_vec_require_valid(spx_ctx, value, tag); return value->capacity;
}
static __attribute__((unused)) spx_status_token spx_vec_get(
    struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag,
    uint64_t index, uint64_t *result
) {
    (void)spx_vec_require_valid(spx_ctx, value, tag);
    if (result == NULL) spx_runtime_invariant_failure("bounded Vec get result is unavailable");
    if (index >= value->len) return spx_vec_failure(spx_ctx, UINT32_C(2));
    *result = value->ptr[index]; return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_vec_v1 spx_vec_move(struct spx_context *spx_ctx, spx_vec_v1 *source) {
    if (source == NULL) spx_runtime_invariant_failure("invalid bounded Vec move source");
    (void)spx_vec_require_valid(spx_ctx, source, source->type_tag);
    spx_vec_v1 moved = *source; *source = (spx_vec_v1){0}; return moved;
}
static __attribute__((unused)) void spx_vec_drop(struct spx_context *spx_ctx, spx_vec_v1 *value) {
    if (value == NULL) spx_runtime_invariant_failure("invalid bounded Vec drop source");
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, value, value->type_tag);
    uint64_t *payload = value->ptr;
    *entry = (struct spx_vec_authority_entry){0}; *value = (spx_vec_v1){0};
    free(payload);
}
"#;
