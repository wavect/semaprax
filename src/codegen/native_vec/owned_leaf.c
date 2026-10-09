/* Each descriptor is one immutable compiler-emitted object. Offsets are
   physical storage, while comparison and settlement follow declaration order. */
typedef struct {
    uint32_t count, stride, capacity, tag;
    uint32_t kinds[8], offsets[8];
    const char *identity;
} spx_leaf_layout_v1;
typedef struct {
    const spx_leaf_layout_v1 *layout;
    uint64_t alignment;
    unsigned char data[];
} spx_leaf_storage_v1;

static __attribute__((unused)) spx_leaf_storage_v1 *spx_leaf_storage(unsigned char *data) {
    return (spx_leaf_storage_v1 *)(void *)(data - offsetof(spx_leaf_storage_v1, data));
}
static __attribute__((unused)) void spx_leaf_layout_check(const spx_leaf_layout_v1 *d) {
    if (!d || !d->identity || !d->count || d->count > 8 || !d->stride || d->stride > 80
        || !d->capacity || d->capacity > 8192 || (d->tag != 10 && d->tag != 12))
        spx_runtime_invariant_failure("invalid owned leaf Vec layout");
    uint32_t owners = 0, scalars = 0;
    for (uint32_t k = 0; k < d->count; ++k) {
        uint32_t kind = d->kinds[k], width = kind == 10 ? 16 : 8;
        if (!kind || kind > 10 || d->offsets[k] > d->stride || width > d->stride - d->offsets[k])
            spx_runtime_invariant_failure("invalid owned leaf Vec field");
        for (uint32_t j = 0; j < k; ++j) {
            uint32_t other = d->kinds[j] == 10 ? 16 : 8;
            if (d->offsets[k] < d->offsets[j] + other && d->offsets[j] < d->offsets[k] + width)
                spx_runtime_invariant_failure("overlapping owned leaf Vec fields");
        }
        if (kind >= 9) ++owners; else ++scalars;
    }
    uint32_t bound = scalars ? 8192 / scalars : 8192;
    if (!owners || owners > 2 || d->capacity != (bound < 8192 / owners ? bound : 8192 / owners))
        spx_runtime_invariant_failure("owned leaf Vec capacity drift");
}
static __attribute__((unused)) struct spx_vec_authority_entry *spx_leaf_check(
    struct spx_context *c, const spx_vec_v1 *s, const spx_leaf_layout_v1 *d
) {
    spx_leaf_layout_check(d);
    struct spx_vec_authority_entry *e = spx_vec_require_valid(c, s, d->tag);
    if (s->capacity > d->capacity || (d->tag == 12 && spx_leaf_storage((unsigned char *)(void *)s->ptr)->layout != d))
        spx_runtime_invariant_failure("owned leaf Vec descriptor mismatch");
    if (d->tag == 10 && (d->stride != sizeof(spx_vec_record_v1) || d->count != 3))
        spx_runtime_invariant_failure("legacy owned record layout mismatch");
    return e;
}
static __attribute__((unused)) void spx_leaf_drop_row(unsigned char *row, const spx_leaf_layout_v1 *d) {
    for (uint32_t k = 0; k < d->count; ++k) {
        unsigned char *p = row + d->offsets[k];
        if (d->kinds[k] == 9) { char *value; memcpy(&value, p, sizeof(value)); spx_string_drop(value); }
        if (d->kinds[k] == 10) { spx_bytes_v1 value; memcpy(&value, p, sizeof(value)); spx_bytes_drop(&value); }
    }
    memset(row, 0, d->stride);
}
static __attribute__((unused)) void spx_leaf_drop_storage(spx_vec_v1 *s) {
    spx_leaf_storage_v1 *storage = spx_leaf_storage((unsigned char *)(void *)s->ptr);
    const spx_leaf_layout_v1 *d = storage->layout;
    spx_leaf_layout_check(d);
    if (d->tag != 12 || s->len > s->capacity || s->capacity > d->capacity)
        spx_runtime_invariant_failure("invalid owned leaf Vec settlement");
    for (uint64_t i = 0; i < s->len; ++i) spx_leaf_drop_row(storage->data + i * d->stride, d);
    free(storage);
}
static __attribute__((unused)) spx_status_token spx_leaf_new(
    struct spx_context *c, const spx_leaf_layout_v1 *d, uint64_t n, spx_vec_v1 *r
) {
    spx_leaf_layout_check(d);
    if (!r || d->tag != 12) spx_runtime_invariant_failure("invalid owned leaf Vec constructor");
    *r = (spx_vec_v1){0};
    if (n > d->capacity) return spx_vec_failure(c, 3);
    size_t bytes = offsetof(spx_leaf_storage_v1, data) + (size_t)n * d->stride;
    spx_leaf_storage_v1 *storage = calloc(1, bytes);
    if (!storage) return spx_vec_failure(c, 3);
    storage->layout = d;
    uint64_t generation = spx_vec_next_generation(c);
    uint32_t authority = spx_vec_register(c, (spx_bytes_v1 *)(void *)storage->data, n, generation, 12);
    if (!authority) { free(storage); return spx_vec_failure(c, 3); }
    *r = (spx_vec_v1){ .ptr = (spx_bytes_v1 *)(void *)storage->data, .capacity = n,
        .generation = generation, .type_tag = 12, .authority = authority };
    return SPX_STATUS_SUCCESS;
}
/* Preflight runs while the caller's canonical staged owners remain live. */
static __attribute__((unused)) spx_status_token spx_leaf_write_check(
    struct spx_context *c, const spx_vec_v1 *s, const spx_leaf_layout_v1 *d, uint64_t index, bool push
) {
    (void)spx_leaf_check(c, s, d);
    if (push && s->len == s->capacity) return spx_vec_failure(c, 1);
    if (!push && index >= s->len) return spx_vec_failure(c, 2);
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) void spx_leaf_write(
    struct spx_context *c, spx_vec_v1 *s, const spx_leaf_layout_v1 *d,
    uint64_t index, bool push, unsigned char *row, spx_vec_v1 *r
) {
    struct spx_vec_authority_entry *e = spx_leaf_check(c, s, d);
    if (!row || !r || r == s || (push ? s->len == s->capacity : index >= s->len))
        spx_runtime_invariant_failure("owned leaf Vec commit without preflight");
    unsigned char *target = (unsigned char *)(void *)s->ptr + (push ? s->len : index) * d->stride;
    if (!push) spx_leaf_drop_row(target, d);
    memcpy(target, row, d->stride); memset(row, 0, d->stride);
    if (push) ++s->len;
    spx_vec_transfer_result(c, s, e, r);
}
static __attribute__((unused)) spx_status_token spx_leaf_clear(
    struct spx_context *c, spx_vec_v1 *s, const spx_leaf_layout_v1 *d, spx_vec_v1 *r
) {
    struct spx_vec_authority_entry *e = spx_leaf_check(c, s, d);
    for (uint64_t i = 0; i < s->len; ++i) spx_leaf_drop_row((unsigned char *)(void *)s->ptr + i * d->stride, d);
    s->len = 0; spx_vec_transfer_result(c, s, e, r); return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_leaf_reserve(
    struct spx_context *c, spx_vec_v1 *s, const spx_leaf_layout_v1 *d, uint64_t add, spx_vec_v1 *r
) {
    struct spx_vec_authority_entry *e = spx_leaf_check(c, s, d);
    if (add > UINT64_MAX - s->len || s->len + add > d->capacity) return spx_vec_failure(c, 3);
    uint64_t n = s->len + add;
    if (n > s->capacity) {
        if (d->tag == 12) {
            spx_leaf_storage_v1 *old = spx_leaf_storage((unsigned char *)(void *)s->ptr);
            spx_leaf_storage_v1 *next = SPX_VEC_REALLOC(old, offsetof(spx_leaf_storage_v1, data) + (size_t)n * d->stride);
            if (!next) return spx_vec_failure(c, 3);
            s->ptr = (spx_bytes_v1 *)(void *)next->data;
        } else {
            void *next = SPX_VEC_REALLOC(s->ptr, (size_t)n * d->stride);
            if (!next) return spx_vec_failure(c, 3);
            s->ptr = next;
        }
        s->capacity = n;
    }
    spx_vec_transfer_result(c, s, e, r); return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) uint64_t spx_leaf_scalar_key(uint32_t kind, uint64_t b) {
    if (kind == 1) return b ^ UINT64_C(0x8000000000000000);
    if (kind == 2) return (b & UINT64_C(0xffffffff)) ^ UINT64_C(0x80000000);
    if (kind == 6) { b &= UINT64_C(0xffffffff); return b & UINT64_C(0x80000000) ? (~b & UINT64_C(0xffffffff)) : b ^ UINT64_C(0x80000000); }
    if (kind == 7) return b & UINT64_C(0x8000000000000000) ? ~b : b ^ UINT64_C(0x8000000000000000);
    return b;
}
static __attribute__((unused)) int spx_leaf_compare(const unsigned char *a, const unsigned char *b, const spx_leaf_layout_v1 *d) {
    for (uint32_t k = 0; k < d->count; ++k) {
        const unsigned char *x = a + d->offsets[k], *y = b + d->offsets[k];
        uint32_t kind = d->kinds[k];
        if (kind < 9) {
            uint64_t av, bv; memcpy(&av, x, 8); memcpy(&bv, y, 8);
            av = spx_leaf_scalar_key(kind, av); bv = spx_leaf_scalar_key(kind, bv);
            if (av != bv) return av > bv ? 1 : -1;
        } else {
            const unsigned char *ap, *bp; uint64_t an, bn;
            if (kind == 9) { char *av, *bv; memcpy(&av, x, sizeof(av)); memcpy(&bv, y, sizeof(bv));
                ap = (const unsigned char *)av; bp = (const unsigned char *)bv;
                an = spx_string_length_v10(av); bn = spx_string_length_v10(bv);
            } else { spx_bytes_v1 av, bv; memcpy(&av, x, sizeof(av)); memcpy(&bv, y, sizeof(bv));
                spx_bytes_require_valid(av); spx_bytes_require_valid(bv); ap = av.ptr; bp = bv.ptr; an = av.len; bn = bv.len; }
            uint64_t common = an < bn ? an : bn;
            int order = common ? memcmp(ap, bp, (size_t)common) : 0;
            if (order) return order > 0 ? 1 : -1;
            if (an != bn) return an > bn ? 1 : -1;
        }
    }
    return 0;
}
static __attribute__((unused)) spx_status_token spx_leaf_sort(
    struct spx_context *c, spx_vec_v1 *s, const spx_leaf_layout_v1 *d, spx_vec_v1 *r
) {
    struct spx_vec_authority_entry *e = spx_leaf_check(c, s, d);
    unsigned char *rows = (unsigned char *)(void *)s->ptr;
    for (uint64_t i = 1; i < s->len; ++i) {
        unsigned char row[80]; memcpy(row, rows + i * d->stride, d->stride);
        uint64_t j = i;
        while (j && spx_leaf_compare(rows + (j - 1) * d->stride, row, d) > 0) {
            memcpy(rows + j * d->stride, rows + (j - 1) * d->stride, d->stride); --j;
        }
        memcpy(rows + j * d->stride, row, d->stride);
    }
    spx_vec_transfer_result(c, s, e, r); return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_leaf_clone(
    struct spx_context *c, const spx_vec_v1 *s, const spx_leaf_layout_v1 *d,
    uint64_t index, unsigned char *out
) {
    (void)spx_leaf_check(c, s, d);
    if (!out) spx_runtime_invariant_failure("missing owned leaf clone result");
    memset(out, 0, d->stride);
    if (index >= s->len) return spx_vec_failure(c, 2);
    const unsigned char *source = (const unsigned char *)(const void *)s->ptr + index * d->stride;
    uint32_t completed = 0;
    for (uint32_t k = 0; k < d->count; ++k) {
        uint32_t kind = d->kinds[k];
        const unsigned char *from = source + d->offsets[k];
        unsigned char *to = out + d->offsets[k];
        if (kind < 9) memcpy(to, from, 8);
        else if (kind == 9) {
            char *text; memcpy(&text, from, sizeof(text));
            uint64_t length = spx_string_length_v10(text);
            if (length > (uint64_t)SIZE_MAX - offsetof(struct spx_string_v10, data) - 1) goto allocation_failure;
            struct spx_string_v10 *copy = malloc(offsetof(struct spx_string_v10, data) + (size_t)length + 1);
            if (!copy) goto allocation_failure;
            copy->len = length;
            if (length) memcpy(copy->data, text, (size_t)length);
            copy->data[length] = '\0'; text = copy->data; memcpy(to, &text, sizeof(text));
        } else {
            spx_bytes_v1 value; memcpy(&value, from, sizeof(value)); spx_bytes_require_valid(value);
            spx_bytes_v1 copy = {0};
            if (value.len) {
                copy.ptr = malloc((size_t)value.len);
                if (!copy.ptr) goto allocation_failure;
                memcpy(copy.ptr, value.ptr, (size_t)value.len);
            }
            copy.len = value.len; memcpy(to, &copy, sizeof(copy));
        }
        completed = k + 1;
    }
    return SPX_STATUS_SUCCESS;
allocation_failure:
    /* Reverse completed prefix, without consulting zeroed uninitialized leaves.
       The failure is selected before cleanup; cleanup cannot replace it. */
    {
        spx_status_token failure = spx_vec_failure(c, 3);
        while (completed) {
            uint32_t k = --completed; unsigned char *p = out + d->offsets[k];
            if (d->kinds[k] == 9) { char *value; memcpy(&value, p, sizeof(value)); spx_string_drop(value); }
            if (d->kinds[k] == 10) { spx_bytes_v1 value; memcpy(&value, p, sizeof(value)); spx_bytes_drop(&value); }
        }
        memset(out, 0, d->stride); return failure;
    }
}
