/* Scalar-only store into an already authenticated owning row. All checks
   precede the commit; neither an owned leaf nor a row backing is replaced. */
static __attribute__((unused)) spx_status_token spx_leaf_update_field(
    struct spx_context *c, spx_vec_v1 *s, const spx_leaf_layout_v1 *d,
    uint64_t index, uint32_t field, uint32_t kind, uint64_t bits, spx_vec_v1 *r
) {
    struct spx_vec_authority_entry *e = spx_leaf_check(c, s, d);
    if (!r || r == s || field >= d->count || !kind || kind > 8 || d->kinds[field] != kind)
        spx_runtime_invariant_failure("invalid owned leaf Vec scalar update");
    if ((kind == 2 && (uint64_t)(int64_t)(int32_t)bits != bits)
        || (kind == 3 && bits > UINT64_C(255))
        || (kind == 5 && (bits > UINT64_C(0x10ffff) || (bits >= UINT64_C(0xd800) && bits <= UINT64_C(0xdfff))))
        || (kind == 6 && bits > UINT64_C(0xffffffff)) || (kind == 8 && bits > UINT64_C(1)))
        spx_runtime_invariant_failure("noncanonical owned leaf Vec scalar update");
    if (index >= s->len) return spx_vec_failure(c, 2);
    if (!c->vec_next_generation || c->vec_next_generation == UINT64_MAX)
        spx_runtime_invariant_failure("owned leaf update generation exhausted");
    unsigned char *target = (unsigned char *)(void *)s->ptr + index * d->stride + d->offsets[field];
    memcpy(target, &bits, sizeof(bits));
    spx_vec_transfer_result(c, s, e, r);
    return SPX_STATUS_SUCCESS;
}
