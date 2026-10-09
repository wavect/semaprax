static __attribute__((unused)) struct spx_vec_authority_entry *spx_leaf_iter_check(
    struct spx_context *c, const spx_iter_v1 *s, const spx_leaf_layout_v1 *expected
) {
    if (!c || c->state != SPX_CONTEXT_INITIALIZED || !s || s->vec.type_tag != 13
        || !s->vec.authority || s->vec.authority > SPX_VEC_AUTHORITY_CAPACITY
        || !s->vec.generation || s->cursor > s->vec.len || s->vec.len > s->vec.capacity || !s->vec.ptr)
        spx_runtime_invariant_failure("invalid owned leaf iterator");
    struct spx_vec_authority_entry *e = &c->vec_authority[s->vec.authority - 1];
    if (!e->live || e->type_tag != 13 || e->ptr != (void *)s->vec.ptr
        || e->capacity != s->vec.capacity || e->generation != s->vec.generation
        || e->len != s->cursor || e->iterator_end != s->vec.len)
        spx_runtime_invariant_failure("stale owned leaf iterator");
    const spx_leaf_layout_v1 *d = spx_leaf_storage((unsigned char *)(void *)s->vec.ptr)->layout;
    spx_leaf_layout_check(d);
    if (d->tag != 12 || s->vec.capacity > d->capacity || (expected && d != expected))
        spx_runtime_invariant_failure("owned leaf iterator descriptor mismatch");
    return e;
}
static __attribute__((unused)) spx_iter_v1 spx_leaf_iter_from_vec(
    struct spx_context *c, spx_vec_v1 *s, const spx_leaf_layout_v1 *d
) {
    struct spx_vec_authority_entry *e = spx_leaf_check(c, s, d);
    if (d->tag != 12) spx_runtime_invariant_failure("legacy record reached new iterator");
    uint64_t generation = spx_vec_next_generation(c);
    spx_iter_v1 r = { .vec = *s, .cursor = 0 };
    r.vec.type_tag = 13; r.vec.generation = generation;
    e->type_tag = 13; e->iterator_end = s->len; e->len = 0; e->generation = generation;
    *s = (spx_vec_v1){0}; return r;
}
static __attribute__((unused)) void spx_leaf_iter_drop(struct spx_context *c, spx_iter_v1 *s) {
    struct spx_vec_authority_entry *e = spx_leaf_iter_check(c, s, NULL);
    spx_leaf_storage_v1 *storage = spx_leaf_storage((unsigned char *)(void *)s->vec.ptr);
    const spx_leaf_layout_v1 *d = storage->layout;
    for (uint64_t i = s->cursor; i < s->vec.len; ++i) spx_leaf_drop_row(storage->data + i * d->stride, d);
    free(storage); *e = (struct spx_vec_authority_entry){0}; *s = (spx_iter_v1){0};
}
static __attribute__((unused)) spx_status_token spx_leaf_iter_next(
    struct spx_context *c, spx_iter_v1 *s, const spx_leaf_layout_v1 *d,
    uint32_t *tag, unsigned char *item, spx_iter_v1 *rest
) {
    struct spx_vec_authority_entry *e = spx_leaf_iter_check(c, s, d);
    if (!tag || !item || !rest) spx_runtime_invariant_failure("missing owned leaf iterator result");
    *tag = 0; memset(item, 0, d->stride); *rest = (spx_iter_v1){0};
    if (s->cursor == s->vec.len) { spx_leaf_iter_drop(c, s); return SPX_STATUS_SUCCESS; }
    unsigned char *row = (unsigned char *)(void *)s->vec.ptr + s->cursor * d->stride;
    memcpy(item, row, d->stride); memset(row, 0, d->stride);
    uint64_t generation = spx_vec_next_generation(c);
    *rest = *s; ++rest->cursor; rest->vec.generation = generation;
    ++e->len; e->generation = generation; *tag = 1; *s = (spx_iter_v1){0};
    return SPX_STATUS_SUCCESS;
}
