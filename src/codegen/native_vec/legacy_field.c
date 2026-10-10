/* Bind immutable nominal metadata before the first legacy row is installed.
   Generation transfers reuse this authority entry; settlement clears it. */
static __attribute__((unused)) spx_status_token spx_leaf_legacy_new(
    struct spx_context *c, const spx_leaf_layout_v1 *d, uint64_t capacity, spx_vec_v1 *r
) {
    spx_leaf_layout_check(d);
    if (d->tag != 10 || d->stride != sizeof(spx_vec_record_v1) || d->count != 3)
        spx_runtime_invariant_failure("invalid legacy owned record descriptor");
    spx_status_token status = spx_vec_record_with_capacity(c, capacity, r);
    if (status != SPX_STATUS_SUCCESS) return status;
    struct spx_vec_authority_entry *e = spx_vec_require_valid(c, r, UINT32_C(10));
    if (e->owned_leaf_layout != NULL)
        spx_runtime_invariant_failure("legacy descriptor authority already bound");
    e->owned_leaf_layout = d;
    return SPX_STATUS_SUCCESS;
}
