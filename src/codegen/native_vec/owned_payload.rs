//! Native Vec v2: each initialized slot owns one `Bytes` carrier.

pub(super) fn emit_runtime(output: &mut impl super::super::COutput) {
    output.push_str(OWNED_PAYLOAD_RUNTIME_C);
}

const OWNED_PAYLOAD_RUNTIME_C: &str = r#"#include <stddef.h>
#include <stdlib.h>

#define SPX_VEC_MAX_CAPACITY UINT64_C(8192)

typedef struct {
    spx_bytes_v1 *ptr;
    uint64_t len;
    uint64_t capacity;
    uint64_t generation;
    uint32_t type_tag;
    uint32_t authority;
} spx_vec_v1;

static __attribute__((unused)) struct spx_vec_authority_entry *spx_vec_require_valid(
    struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || value == NULL
        || tag < UINT32_C(1) || tag > UINT32_C(9) || value->type_tag != tag
        || value->generation == UINT64_C(0) || value->authority == UINT32_C(0)
        || value->authority > SPX_VEC_AUTHORITY_CAPACITY
        || value->capacity > SPX_VEC_MAX_CAPACITY || value->len > value->capacity
        || ((value->capacity == UINT64_C(0)) != (value->ptr == NULL))) {
        spx_runtime_invariant_failure("invalid owned bounded Vec Bytes carrier");
    }
    struct spx_vec_authority_entry *entry =
        &spx_ctx->vec_authority[value->authority - UINT32_C(1)];
    if (!entry->live || entry->ptr != (void *)value->ptr || entry->len != value->len
        || entry->capacity != value->capacity || entry->generation != value->generation
        || entry->type_tag != tag) {
        spx_runtime_invariant_failure("stale or forged owned bounded Vec Bytes carrier");
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
    struct spx_context *spx_ctx, spx_bytes_v1 *ptr, uint64_t capacity, uint64_t generation
) {
    for (uint32_t index = UINT32_C(0); index < SPX_VEC_AUTHORITY_CAPACITY; ++index) {
        struct spx_vec_authority_entry *entry = &spx_ctx->vec_authority[index];
        if (!entry->live) {
            entry->ptr = (void *)ptr; entry->len = UINT64_C(0); entry->capacity = capacity;
            entry->generation = generation; entry->type_tag = UINT32_C(9); entry->live = true;
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

static __attribute__((unused)) void spx_vec_transfer_result(
    struct spx_context *spx_ctx, spx_vec_v1 *source,
    struct spx_vec_authority_entry *entry, spx_vec_v1 *result
) {
    if (result == NULL || result == source)
        spx_runtime_invariant_failure("invalid bounded Vec Bytes result");
    source->generation = spx_vec_next_generation(spx_ctx);
    entry->ptr = (void *)source->ptr; entry->len = source->len;
    entry->capacity = source->capacity; entry->generation = source->generation;
    *result = *source; *source = (spx_vec_v1){0};
}

static __attribute__((unused)) spx_status_token spx_vec_bytes_with_capacity(
    struct spx_context *spx_ctx, uint64_t capacity, spx_vec_v1 *result
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || result == NULL)
        spx_runtime_invariant_failure("invalid bounded Vec Bytes allocation request");
    *result = (spx_vec_v1){0};
    if (capacity > SPX_VEC_MAX_CAPACITY) return spx_vec_failure(spx_ctx, UINT32_C(3));
    spx_bytes_v1 *payload = capacity == UINT64_C(0) ? NULL :
        (spx_bytes_v1 *)calloc((size_t)capacity, sizeof(spx_bytes_v1));
    if (capacity != UINT64_C(0) && payload == NULL) return spx_vec_failure(spx_ctx, UINT32_C(3));
    uint64_t generation = spx_vec_next_generation(spx_ctx);
    uint32_t authority = spx_vec_register(spx_ctx, payload, capacity, generation);
    if (authority == UINT32_C(0)) { free(payload); return spx_vec_failure(spx_ctx, UINT32_C(3)); }
    result->ptr = payload; result->capacity = capacity; result->generation = generation;
    result->type_tag = UINT32_C(9); result->authority = authority;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_vec_bytes_push(
    struct spx_context *spx_ctx, spx_vec_v1 *source, spx_bytes_v1 *value, spx_vec_v1 *result
) {
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, source, UINT32_C(9));
    if (result == NULL || result == source) spx_runtime_invariant_failure("invalid owned Bytes Vec result");
    if (value == NULL) spx_runtime_invariant_failure("invalid owned Bytes Vec push value");
    spx_bytes_require_valid(*value);
    if (source->len == source->capacity) return spx_vec_failure(spx_ctx, UINT32_C(1));
    source->ptr[source->len] = spx_bytes_move(value); source->len += UINT64_C(1);
    spx_vec_transfer_result(spx_ctx, source, entry, result); return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_vec_bytes_reserve_exact(
    struct spx_context *spx_ctx, spx_vec_v1 *source, uint64_t additional, spx_vec_v1 *result
) {
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, source, UINT32_C(9));
    if (result == NULL || result == source) spx_runtime_invariant_failure("invalid bounded Vec Bytes reserve result");
    if (additional > UINT64_MAX - source->len) return spx_vec_failure(spx_ctx, UINT32_C(3));
    uint64_t required = source->len + additional;
    uint64_t target = source->capacity > required ? source->capacity : required;
    if (target > SPX_VEC_MAX_CAPACITY) return spx_vec_failure(spx_ctx, UINT32_C(3));
    if (target != source->capacity) {
        spx_bytes_v1 *payload = (spx_bytes_v1 *)calloc((size_t)target, sizeof(spx_bytes_v1));
        if (payload == NULL) return spx_vec_failure(spx_ctx, UINT32_C(3));
        for (uint64_t index = UINT64_C(0); index < source->len; ++index)
            payload[index] = spx_bytes_move(&source->ptr[index]);
        free(source->ptr); source->ptr = payload; source->capacity = target;
    }
    spx_vec_transfer_result(spx_ctx, source, entry, result); return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_vec_bytes_set(
    struct spx_context *spx_ctx, spx_vec_v1 *source, uint64_t index,
    spx_bytes_v1 *value, spx_vec_v1 *result
) {
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, source, UINT32_C(9));
    if (result == NULL || result == source) spx_runtime_invariant_failure("invalid owned Bytes Vec result");
    if (value == NULL) spx_runtime_invariant_failure("invalid owned Bytes Vec set value");
    spx_bytes_require_valid(*value);
    if (index >= source->len) return spx_vec_failure(spx_ctx, UINT32_C(2));
    spx_bytes_drop(&source->ptr[index]); source->ptr[index] = spx_bytes_move(value);
    spx_vec_transfer_result(spx_ctx, source, entry, result); return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_vec_bytes_clear(
    struct spx_context *spx_ctx, spx_vec_v1 *source, spx_vec_v1 *result
) {
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, source, UINT32_C(9));
    if (result == NULL || result == source) spx_runtime_invariant_failure("invalid owned Bytes Vec result");
    for (uint64_t index = UINT64_C(0); index < source->len; ++index)
        spx_bytes_drop(&source->ptr[index]);
    source->len = UINT64_C(0);
    spx_vec_transfer_result(spx_ctx, source, entry, result); return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) uint64_t spx_vec_len(
    struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag
) { (void)spx_vec_require_valid(spx_ctx, value, tag); return value->len; }
static __attribute__((unused)) uint64_t spx_vec_capacity(
    struct spx_context *spx_ctx, const spx_vec_v1 *value, uint32_t tag
) { (void)spx_vec_require_valid(spx_ctx, value, tag); return value->capacity; }
static __attribute__((unused)) spx_vec_v1 spx_vec_move(struct spx_context *spx_ctx, spx_vec_v1 *source) {
    if (source == NULL) spx_runtime_invariant_failure("invalid bounded Vec Bytes move source");
    (void)spx_vec_require_valid(spx_ctx, source, source->type_tag);
    spx_vec_v1 moved = *source; *source = (spx_vec_v1){0}; return moved;
}
static __attribute__((unused)) void spx_vec_drop(struct spx_context *spx_ctx, spx_vec_v1 *value) {
    if (value == NULL) spx_runtime_invariant_failure("invalid bounded Vec Bytes drop source");
    struct spx_vec_authority_entry *entry = spx_vec_require_valid(spx_ctx, value, value->type_tag);
    if (value->type_tag == UINT32_C(9))
        for (uint64_t index = UINT64_C(0); index < value->len; ++index) spx_bytes_drop(&value->ptr[index]);
    free(value->ptr); *entry = (struct spx_vec_authority_entry){0}; *value = (spx_vec_v1){0};
}

/* Mixed programs retain the scalar eight-byte storage and allocation hook. */
#ifndef SPX_VEC_REALLOC
#define SPX_VEC_REALLOC realloc
#endif
static __attribute__((unused)) uint64_t spx_vec_f32_bits(float value) { union { float value; uint32_t bits; } c = { .value = value }; return (uint64_t)c.bits; }
static __attribute__((unused)) uint64_t spx_vec_f64_bits(double value) { union { double value; uint64_t bits; } c = { .value = value }; return c.bits; }
static __attribute__((unused)) float spx_vec_bits_f32(uint64_t bits) { union { float value; uint32_t bits; } c = { .bits = (uint32_t)bits }; return c.value; }
static __attribute__((unused)) double spx_vec_bits_f64(uint64_t bits) { union { double value; uint64_t bits; } c = { .bits = bits }; return c.value; }
static __attribute__((unused)) uint32_t spx_vec_register_scalar(struct spx_context *c, spx_bytes_v1 *p, uint64_t n, uint32_t tag, uint64_t g) {
    for (uint32_t i=UINT32_C(0);i<SPX_VEC_AUTHORITY_CAPACITY;++i) { struct spx_vec_authority_entry *e=&c->vec_authority[i]; if(!e->live) { e->ptr=(void*)p;e->len=UINT64_C(0);e->capacity=n;e->generation=g;e->type_tag=tag;e->live=true;return i+UINT32_C(1); } } return UINT32_C(0);
}
static __attribute__((unused)) spx_status_token spx_vec_with_capacity(struct spx_context *c,uint32_t tag,uint64_t n,spx_vec_v1 *r) {
    if(c==NULL||c->state!=SPX_CONTEXT_INITIALIZED||r==NULL||tag<UINT32_C(1)||tag>UINT32_C(8))spx_runtime_invariant_failure("invalid bounded Vec allocation request"); *r=(spx_vec_v1){0}; if(n>SPX_VEC_MAX_CAPACITY)return spx_vec_failure(c,UINT32_C(3)); spx_bytes_v1*p=n==UINT64_C(0)?NULL:(spx_bytes_v1*)calloc((size_t)n,sizeof(uint64_t)); if(n&&p==NULL)return spx_vec_failure(c,UINT32_C(3)); uint64_t g=spx_vec_next_generation(c);uint32_t a=spx_vec_register_scalar(c,p,n,tag,g);if(!a){free(p);return spx_vec_failure(c,UINT32_C(3));} r->ptr=p;r->capacity=n;r->generation=g;r->type_tag=tag;r->authority=a;return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_vec_push(struct spx_context*c,uint32_t tag,spx_vec_v1*s,uint64_t b,spx_vec_v1*r) { if(tag==UINT32_C(9))spx_runtime_invariant_failure("owned Vec payload requires v2 operation");struct spx_vec_authority_entry*e=spx_vec_require_valid(c,s,tag);if(r==NULL||r==s)spx_runtime_invariant_failure("invalid bounded Vec push result");if(s->len==s->capacity)return spx_vec_failure(c,UINT32_C(1));((uint64_t*)(void*)s->ptr)[s->len++]=b;spx_vec_transfer_result(c,s,e,r);return SPX_STATUS_SUCCESS; }
static __attribute__((unused)) spx_status_token spx_vec_reserve_exact(struct spx_context*c,uint32_t tag,spx_vec_v1*s,uint64_t add,spx_vec_v1*r) { if(tag==UINT32_C(9))spx_runtime_invariant_failure("owned Vec payload requires v2 operation");struct spx_vec_authority_entry*e=spx_vec_require_valid(c,s,tag);if(r==NULL||r==s)spx_runtime_invariant_failure("invalid bounded Vec reserve result");if(add>UINT64_MAX-s->len)return spx_vec_failure(c,UINT32_C(3));uint64_t need=s->len+add,target=s->capacity>need?s->capacity:need;if(target>SPX_VEC_MAX_CAPACITY)return spx_vec_failure(c,UINT32_C(3));if(target!=s->capacity){spx_bytes_v1*p=(spx_bytes_v1*)SPX_VEC_REALLOC(s->ptr,(size_t)target*sizeof(uint64_t));if(!p)return spx_vec_failure(c,UINT32_C(3));s->ptr=p;s->capacity=target;}spx_vec_transfer_result(c,s,e,r);return SPX_STATUS_SUCCESS; }
static __attribute__((unused)) spx_status_token spx_vec_set(struct spx_context*c,uint32_t tag,spx_vec_v1*s,uint64_t i,uint64_t b,spx_vec_v1*r) { if(tag==UINT32_C(9))spx_runtime_invariant_failure("owned Vec payload requires v2 operation");struct spx_vec_authority_entry*e=spx_vec_require_valid(c,s,tag);if(r==NULL||r==s)spx_runtime_invariant_failure("invalid bounded Vec set result");if(i>=s->len)return spx_vec_failure(c,UINT32_C(2));((uint64_t*)(void*)s->ptr)[i]=b;spx_vec_transfer_result(c,s,e,r);return SPX_STATUS_SUCCESS; }
static __attribute__((unused)) spx_status_token spx_vec_clear(struct spx_context*c,uint32_t tag,spx_vec_v1*s,spx_vec_v1*r) { if(tag==UINT32_C(9))spx_runtime_invariant_failure("owned Vec payload requires v2 operation");struct spx_vec_authority_entry*e=spx_vec_require_valid(c,s,tag);s->len=UINT64_C(0);spx_vec_transfer_result(c,s,e,r);return SPX_STATUS_SUCCESS; }
static __attribute__((unused)) spx_status_token spx_vec_get(struct spx_context*c,const spx_vec_v1*s,uint32_t tag,uint64_t i,uint64_t*r) { (void)spx_vec_require_valid(c,s,tag);if(tag==UINT32_C(9)||r==NULL)spx_runtime_invariant_failure("owned Vec payload cannot be copied");if(i>=s->len)return spx_vec_failure(c,UINT32_C(2));*r=((uint64_t*)(void*)s->ptr)[i];return SPX_STATUS_SUCCESS; }
"#;
