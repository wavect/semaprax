/* Authored hostile and positive regressions for the shared-owner runtime.
   Every fixture that a fail-stop may interrupt is static. */
static spx_shb_issuer issuer;
static spx_shb_registry reg, other;
static spx_shb_bytes a, b, c, d, stale, zero_owner, many[32];
static spx_shb_vec v, w, stale_vec, vecs[SPX_SHB_VECS];
static spx_shb_iter it, rest, stale_it;
static spx_shb_partition_plan plan, forged_plan, third_plan;
static spx_shb_loan loan;
static spx_bytes_v1 published;
static uint32_t tag;

PROBE_FN void fresh(spx_shb_registry *r) {
    memset(r, 0, sizeof(*r));
    spx_shb_registry_init(r, &issuer);
}
PROBE_FN spx_slice_u8_v1 slice(const void *data, uint64_t len) {
    return (spx_slice_u8_v1){ .ptr = len == 0u ? NULL : (const uint8_t *)data, .len = len };
}
PROBE_FN bool view_is(spx_shb_bytes *owner, const char *text, uint64_t len) {
    spx_slice_u8_v1 s = spx_shb_view(&reg, owner);
    return s.len == len && (len == 0u || memcmp(s.ptr, text, (size_t)len) == 0);
}
PROBE_FN void partition(spx_shb_bytes *source, uint64_t count, uint64_t width, spx_shb_vec *out) {
    assert(spx_shb_partition_prepare(&reg, source, count, width, &plan) == SPX_SHB_OK);
    spx_shb_partition_commit(&reg, &plan, source, out);
    assert(spx_shb_is_vacant(source) && !plan.armed);
}

/* Sum, order, NUL, Unicode and non-UTF-8 survive; partition charges nothing. */
PROBE_FN void exact_bytes_and_no_payload_charge(void) {
    static const uint8_t text[12] = { 'a', 0x00, 0xC3, 0xA9, 0xFF, 'z', 0xE2, 0x82, 0xAC, 0x00, 0x80, 'q' };
    census_reset(); fresh(&reg);
    spx_shb_copy(&reg, slice(text, 12u), &a);
    assert(payload_pool.calls == 1u && reg.events == 1u && reg.cumulative == 12u);
    size_t metadata = metadata_pool.calls;
    partition(&a, 4u, 3u, &v);
    assert(payload_pool.calls == 1u && reg.events == 1u && reg.cumulative == 12u);
    assert(metadata_pool.calls == metadata + 2u && reg.child_table_slots == 4u);
    uint8_t joined[12]; uint64_t sum = 0u, expected = 0u;
    const uint8_t *previous_end = NULL;
    for (uint64_t i = 0u; i < 4u; ++i) {
        spx_slice_u8_v1 s;
        assert(spx_shb_vec_view(&reg, &v, i, &s) == SPX_SHB_OK && s.len == 3u);
        assert(previous_end == NULL || s.ptr == previous_end);
        previous_end = s.ptr + 3;
        memcpy(joined + 3u * i, s.ptr, 3u);
        for (uint64_t k = 0u; k < 3u; ++k) sum += s.ptr[k];
    }
    for (size_t k = 0u; k < 12u; ++k) expected += text[k];
    assert(memcmp(joined, text, 12u) == 0 && sum == expected);
    spx_slice_u8_v1 out_of_range;
    assert(spx_shb_vec_view(&reg, &v, 4u, &out_of_range) == SPX_SHB_VEC_INDEX && out_of_range.ptr == NULL);
    spx_shb_vec_drop(&reg, &v);
    assert(payload_pool.frees == 1u && payload_pool.live == 0u && metadata_pool.live == 0u);
    spx_shb_registry_finish(&reg);
}

/* Child stores stay inside their range and renew only their own lease. */
PROBE_FN void mutation_isolation(void) {
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 6u, &a);
    partition(&a, 3u, 2u, &v);
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    assert(tag == 1u && spx_shb_vec_len(&reg, &w) == 2u);
    assert(spx_shb_set_check(&reg, &c, 2u) == SPX_SHB_BUFFER_INDEX);
    assert(spx_shb_set5_check(&reg, &c, 0u) == SPX_SHB_BUFFER_INDEX);
    assert(spx_shb_set_check(&reg, &c, 1u) == SPX_SHB_OK);
    stale = c;
    spx_shb_set(&reg, &c, 1u, 0x7Fu, &d);
    assert(spx_shb_is_vacant(&c));
    EXPECT_STOP("stale", (void)spx_shb_view(&reg, &stale));
    EXPECT_STOP("outside its capacity", spx_shb_set(&reg, &d, 2u, 1u, &c));
    assert(view_is(&d, "\0\x7f", 2u));
    for (uint64_t i = 0u; i < 2u; ++i) {
        spx_slice_u8_v1 s;
        assert(spx_shb_vec_view(&reg, &w, i, &s) == SPX_SHB_OK && s.ptr[0] == 0u && s.ptr[1] == 0u);
    }
    spx_shb_drop(&reg, &d);
    spx_shb_vec_drop(&reg, &w);
    assert(payload_pool.frees == 1u);
    spx_shb_registry_finish(&reg);
}

/* A popped child survives its source Vec and its siblings; the last lease
   frees the backing exactly once. */
PROBE_FN void pop_survives_parent_and_sibling_drop(void) {
    census_reset(); fresh(&reg);
    spx_shb_copy(&reg, slice("abcdef", 6u), &a);
    partition(&a, 3u, 2u, &v);
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    spx_shb_vec_pop(&reg, &w, &tag, &d, &v);
    assert(tag == 1u && view_is(&c, "ef", 2u) && view_is(&d, "cd", 2u));
    spx_shb_vec_drop(&reg, &v);
    assert(payload_pool.frees == 0u && view_is(&c, "ef", 2u));
    spx_shb_drop(&reg, &d);
    assert(payload_pool.frees == 0u && view_is(&c, "ef", 2u));
    spx_shb_drop(&reg, &c);
    assert(payload_pool.frees == 1u && payload_pool.live == 0u && metadata_pool.live == 0u);
    spx_shb_registry_finish(&reg);
}

/* Iteration transfers leases; abandonment settles the rest in order. */
PROBE_FN void iteration_and_abandonment(void) {
    census_reset(); fresh(&reg);
    spx_shb_copy(&reg, slice("01234567", 8u), &a);
    partition(&a, 4u, 2u, &v);
    it = spx_shb_iter_from_vec(&reg, &v);
    /* Vec has its own carrier, not the Bytes carrier layout. */
    assert(v.rows == NULL && v.len == 0u && v.capacity == 0u
        && v.generation == 0u && v.issuer == 0u && v.authority == 0u && v.reserved == 0u);
    stale_it = it;
    spx_shb_iter_next(&reg, &it, &tag, &c, &rest);
    assert(tag == 1u && view_is(&c, "01", 2u));
    EXPECT_STOP("stale", spx_shb_iter_drop(&reg, &stale_it));
    spx_shb_iter_drop(&reg, &rest);
    assert(payload_pool.frees == 0u && view_is(&c, "01", 2u));
    spx_shb_drop(&reg, &c);
    assert(payload_pool.frees == 1u);
    spx_shb_copy(&reg, slice("ab", 2u), &a);
    partition(&a, 2u, 1u, &v);
    it = spx_shb_iter_from_vec(&reg, &v);
    for (uint32_t step = 0u; ; ++step) {
        spx_shb_iter_next(&reg, &it, &tag, &c, &rest);
        if (tag == 0u) { assert(step == 2u); break; }
        spx_shb_drop(&reg, &c);
        it = rest;
    }
    assert(payload_pool.frees == 2u && metadata_pool.live == 0u);
    spx_shb_registry_finish(&reg);
}

/* Clone is a fresh charged deep copy, isolated from later writes. */
PROBE_FN void deep_clone_isolation_and_charges(void) {
    census_reset(); fresh(&reg);
    spx_shb_copy(&reg, slice("xyz", 3u), &a);
    partition(&a, 3u, 1u, &v);
    assert(spx_shb_vec_clone_at(&reg, &v, 3u, &c) == SPX_SHB_VEC_INDEX && reg.events == 1u);
    assert(spx_shb_vec_clone_at(&reg, &v, 1u, &c) == SPX_SHB_OK);
    assert(reg.events == 2u && reg.cumulative == 4u && payload_pool.calls == 2u);
    spx_shb_set(&reg, &c, 0u, 'Q', &d);
    spx_slice_u8_v1 s;
    assert(view_is(&d, "Q", 1u) && spx_shb_vec_view(&reg, &v, 1u, &s) == SPX_SHB_OK && s.ptr[0] == 'y');
    spx_shb_clone(&reg, &d, &b);
    assert(reg.events == 3u && view_is(&b, "Q", 1u));
    spx_shb_drop(&reg, &d); spx_shb_drop(&reg, &b);
    assert(payload_pool.frees == 2u);
    spx_shb_vec_drop(&reg, &v);
    assert(payload_pool.frees == 3u && payload_pool.live == 0u);
    spx_shb_registry_finish(&reg);
}

/* Foreign, vacant, forged, overlapping, stale and double-drop carriers. */
PROBE_FN void hostile_carriers(void) {
    census_reset(); fresh(&reg);
    /* Distinct held issuers may initialize concurrently or after re-entry;
       neither may reuse another registry's carrier namespace. Populate both
       with the same slot/nonce to make a foreign-token collision observable. */
    static spx_shb_issuer second_issuer;
    spx_shb_issuer_init(&second_issuer);
    memset(&other, 0, sizeof(other));
    spx_shb_registry_init(&other, &second_issuer);
    assert(reg.issuer != other.issuer);
    spx_shb_zeroed(&reg, 4u, &a);
    spx_shb_bytes other_owner;
    spx_shb_zeroed(&other, 4u, &other_owner);
    assert(a.slot == other_owner.slot && a.nonce == other_owner.nonce);
    EXPECT_STOP("foreign", (void)spx_shb_view(&reg, &other_owner));
    EXPECT_STOP("foreign", (void)spx_shb_view(&other, &a));
    spx_shb_drop(&other, &other_owner);
    spx_shb_registry_finish(&other);
    EXPECT_STOP("foreign or vacant", (void)spx_shb_view(&reg, &zero_owner));
    stale = a; stale.slot = UINT64_C(5) << 32;
    EXPECT_STOP("unknown shared Bytes backing", (void)spx_shb_view(&reg, &stale));
    stale = a; stale.slot |= 1u;
    EXPECT_STOP("forged or overlapping", (void)spx_shb_view(&reg, &stale));
    stale = a;
    partition(&a, 2u, 2u, &v);
    EXPECT_STOP("forged or overlapping", (void)spx_shb_view(&reg, &stale));
    stale.slot |= 3u;
    EXPECT_STOP("forged or overlapping", (void)spx_shb_view(&reg, &stale));
    stale = spx_shb_row_owner(&v, 0u); stale.nonce += 1u;
    EXPECT_STOP("stale", (void)spx_shb_view(&reg, &stale));
    stale_vec = v;
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    EXPECT_STOP("stale or forged shared Bytes Vec", (void)spx_shb_vec_len(&reg, &stale_vec));
    stale_vec = w; stale_vec.issuer = other.issuer;
    EXPECT_STOP("invalid shared Bytes Vec", (void)spx_shb_vec_len(&reg, &stale_vec));
    stale = c;
    spx_shb_drop(&reg, &c);
    EXPECT_STOP("stale", spx_shb_drop(&reg, &stale));
    /* A forged duplicate of a stored row cannot replace that row. */
    stale = spx_shb_row_owner(&w, 0u);
    EXPECT_STOP("aliases its replaced row", (void)spx_shb_vec_set(&reg, &w, 0u, &stale, &v));
    assert(spx_shb_vec_len(&reg, &w) == 1u && view_is(&stale, "\0\0", 2u));
    other.module = NULL;
    EXPECT_STOP("not the held live registry", (void)spx_shb_view(&other, &stale));
    spx_shb_vec_drop(&reg, &w);
    assert(payload_pool.frees == 2u && payload_pool.live == 0u);
    spx_shb_registry_finish(&reg);
}

/* Child leases, including offset-zero whole-extent ones, are not inputs. */
PROBE_FN void child_partition_refused(void) {
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 4u, &a);
    partition(&a, 1u, 4u, &v);
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    EXPECT_STOP("not an ordinary whole-backing lease", (void)spx_shb_partition_prepare(&reg, &c, 1u, 4u, &plan));
    EXPECT_STOP("not an ordinary whole-backing lease", (void)spx_shb_partition_prepare(&reg, &c, 2u, 2u, &plan));
    assert(!plan.armed && reg.child_held == 0u);
    spx_shb_drop(&reg, &c); spx_shb_vec_drop(&reg, &w);
    spx_shb_registry_finish(&reg);
}

/* Geometry refusals select in spec order, allocate nothing and keep the source. */
PROBE_FN void geometry_refusals_preserve_source(void) {
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 6u, &a);
    size_t metadata = metadata_pool.calls;
    assert(spx_shb_partition_prepare(&reg, &a, 3u, 0u, &plan) == SPX_SHB_PARTITION_WIDTH_ZERO);
    assert(spx_shb_partition_prepare(&reg, &a, 0u, 0u, &plan) == SPX_SHB_PARTITION_WIDTH_ZERO);
    assert(spx_shb_partition_prepare(&reg, &a, UINT64_MAX, 2u, &plan) == SPX_SHB_PARTITION_EXTENT_OVERFLOW);
    assert(spx_shb_partition_prepare(&reg, &a, 3u, 3u, &plan) == SPX_SHB_PARTITION_EXTENT_MISMATCH);
    assert(spx_shb_partition_prepare(&reg, &a, 7u, 1u, &plan) == SPX_SHB_PARTITION_EXTENT_MISMATCH);
    assert(spx_shb_partition_prepare(&reg, &a, 0u, 6u, &plan) == SPX_SHB_PARTITION_EXTENT_MISMATCH);
    assert(metadata_pool.calls == metadata && !plan.armed && reg.child_held == 0u);
    assert(spx_shb_len(&reg, &a) == 6u);
    assert(spx_shb_partition_prepare(&reg, &a, 3u, 2u, &plan) == SPX_SHB_OK && reg.child_held == 3u);
    spx_shb_partition_abort(&reg, &plan);
    assert(reg.child_held == 0u && reg.child_table_slots == 0u && metadata_pool.live == 0u && spx_shb_len(&reg, &a) == 6u);
    partition(&a, 6u, 1u, &v);
    spx_shb_vec_drop(&reg, &v);
    spx_shb_registry_finish(&reg);
}

/* Both physical metadata failures unwind preparation before fail-stop. */
PROBE_FN void metadata_allocation_failure_releases_unpublished_preparation(void) {
    for (size_t failed_request = 1u; failed_request <= 2u; ++failed_request) {
        census_reset(); fresh(&reg);
        spx_shb_copy(&reg, slice("hello!", 6u), &a);
        metadata_pool.fail_at = failed_request;
        EXPECT_STOP("partition metadata allocation failed", (void)spx_shb_partition_prepare(&reg, &a, 3u, 2u, &plan));
        assert(!plan.armed && reg.child_held == 0u && reg.child_table_slots == 0u);
        assert(metadata_pool.calls == failed_request && metadata_pool.live == 0u);
        assert(metadata_pool.frees == failed_request - 1u);
        assert(view_is(&a, "hello!", 6u) && payload_pool.live == 1u && payload_pool.frees == 0u);
        assert(reg.events == 1u && reg.cumulative == 6u);
        for (size_t i = 0u; i < SPX_SHB_VECS; ++i) assert(reg.vec[i].state == SPX_SHB_VEC_FREE);
        spx_shb_drop(&reg, &a);
        assert(payload_pool.frees == 1u);
        spx_shb_registry_finish(&reg);
    }
}

/* Empty input with any positive width, including the maximum, is a real
   zero-child success; the empty owner had a real lease. */
PROBE_FN void empty_partition(void) {
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 0u, &a);
    assert(payload_pool.calls == 0u && reg.events == 1u && view_is(&a, "", 0u));
    assert(spx_shb_partition_prepare(&reg, &a, 1u, 1u, &plan) == SPX_SHB_PARTITION_EXTENT_MISMATCH);
    partition(&a, 0u, UINT64_MAX, &v);
    assert(spx_shb_vec_len(&reg, &v) == 0u && reg.backing[0].state == SPX_SHB_RELEASED);
    spx_shb_vec_drop(&reg, &v);
    spx_shb_zeroed(&reg, 0u, &b);
    spx_shb_drop(&reg, &b);
    spx_shb_registry_finish(&reg);
}

/* Child metadata inventory: exact fits, one over refuses, abort releases. */
PROBE_FN void metadata_exact_and_one_over(void) {
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 8192u, &a);
    partition(&a, 8192u, 1u, &v);
    spx_shb_zeroed(&reg, 1u, &b);
    assert(spx_shb_partition_prepare(&reg, &b, 1u, 1u, &plan) == SPX_SHB_PARTITION_METADATA_CAPACITY);
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    assert(tag != 0u);
    spx_shb_vec_drop(&reg, &w);
    assert(reg.child_table_slots == 8192u && metadata_pool.live == 1u);
    assert(spx_shb_partition_prepare(&reg, &b, 1u, 1u, &plan) == SPX_SHB_PARTITION_METADATA_CAPACITY);
    stale = c;
    spx_shb_drop(&reg, &c);
    assert(reg.child_table_slots == 0u && metadata_pool.live == 0u);
    partition(&b, 1u, 1u, &v);
    EXPECT_STOP("released, forged or overlapping", (void)spx_shb_len(&reg, &stale));
    assert(reg.events == 2u && reg.cumulative == 8193u);
    spx_shb_vec_drop(&reg, &v);
    assert(payload_pool.live == 0u && metadata_pool.live == 0u);
    spx_shb_registry_finish(&reg);

    fresh(&reg);
    spx_shb_zeroed(&reg, 8193u, &a);
    assert(spx_shb_partition_prepare(&reg, &a, 8193u, 1u, &plan) == SPX_SHB_PARTITION_METADATA_CAPACITY);
    spx_shb_drop(&reg, &a);
    spx_shb_registry_finish(&reg);

    fresh(&reg);
    spx_shb_zeroed(&reg, 8191u, &a); spx_shb_zeroed(&reg, 1u, &b); spx_shb_zeroed(&reg, 1u, &c);
    assert(spx_shb_partition_prepare(&reg, &a, 8191u, 1u, &plan) == SPX_SHB_OK);
    assert(spx_shb_partition_prepare(&reg, &b, 1u, 1u, &forged_plan) == SPX_SHB_OK);
    assert(spx_shb_partition_prepare(&reg, &c, 1u, 1u, &third_plan) == SPX_SHB_PARTITION_METADATA_CAPACITY);
    assert(!third_plan.armed && reg.child_held == 8192u);
    spx_shb_partition_abort(&reg, &forged_plan);
    spx_shb_partition_abort(&reg, &plan);
    assert(reg.child_held == 0u);
    partition(&a, 8191u, 1u, &v);
    partition(&b, 1u, 1u, &w);
    assert(spx_shb_partition_prepare(&reg, &c, 1u, 1u, &plan) == SPX_SHB_PARTITION_METADATA_CAPACITY);
    spx_shb_drop(&reg, &c); spx_shb_vec_drop(&reg, &v); spx_shb_vec_drop(&reg, &w);
    spx_shb_registry_finish(&reg);

    fresh(&reg);
    for (uint32_t i = 0u; i < SPX_SHB_VECS; ++i)
        assert(spx_shb_vec_with_capacity(&reg, 0u, &vecs[i]) == SPX_SHB_OK);
    assert(spx_shb_vec_with_capacity(&reg, 0u, &v) == SPX_SHB_VEC_CAPACITY_REFUSED);
    spx_shb_zeroed(&reg, 1u, &a);
    assert(spx_shb_partition_prepare(&reg, &a, 1u, 1u, &plan) == SPX_SHB_PARTITION_METADATA_CAPACITY);
    for (uint32_t i = 0u; i < SPX_SHB_VECS; ++i) spx_shb_vec_drop(&reg, &vecs[i]);
    spx_shb_drop(&reg, &a);
    spx_shb_registry_finish(&reg);
}

/* Existing 32-event, 2 MiB and 128 KiB limits with their +1 controls. */
PROBE_FN void unchanged_quotas_and_plus_one(void) {
    census_reset(); fresh(&reg);
    for (uint32_t i = 0u; i < 32u; ++i) spx_shb_zeroed(&reg, 1u, &many[i]);
    EXPECT_STOP("quota", spx_shb_zeroed(&reg, 1u, &a));
    assert(reg.events == 32u && payload_pool.calls == 32u);
    for (uint32_t i = 0u; i < 32u; ++i) spx_shb_drop(&reg, &many[i]);
    spx_shb_registry_finish(&reg);

    fresh(&reg);
    for (uint32_t i = 0u; i < 16u; ++i) spx_shb_zeroed(&reg, SPX_SHB_EXTENT, &many[i]);
    assert(reg.cumulative == SPX_SHB_CUMULATIVE);
    EXPECT_STOP("quota", spx_shb_zeroed(&reg, 1u, &a));
    EXPECT_STOP("exact length bound", spx_shb_zeroed(&reg, SPX_SHB_EXTENT + 1u, &a));
    for (uint32_t i = 0u; i < 16u; ++i) spx_shb_drop(&reg, &many[i]);
    spx_shb_registry_finish(&reg);
    assert(payload_pool.live == 0u);
}

/* Outstanding loans forbid drop, move, store and partition. */
PROBE_FN void loans_pin_their_generation(void) {
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 2u, &a);
    loan = spx_shb_loan_begin(&reg, &a);
    EXPECT_STOP("outstanding borrow", spx_shb_drop(&reg, &a));
    EXPECT_STOP("outstanding borrow", (void)spx_shb_move(&reg, &a));
    EXPECT_STOP("outstanding borrow", spx_shb_set(&reg, &a, 0u, 1u, &b));
    EXPECT_STOP("outstanding borrow", (void)spx_shb_partition_prepare(&reg, &a, 2u, 1u, &plan));
    EXPECT_STOP("outstanding borrow", (void)spx_shb_copy_out(&reg, &a, &published));
    assert(view_is(&a, "\0\0", 2u));
    spx_shb_loan_end(&reg, &loan);
    EXPECT_STOP("foreign or vacant", spx_shb_loan_end(&reg, &loan));
    spx_shb_drop(&reg, &a);
    spx_shb_registry_finish(&reg);
}

/* A loan on a later row refuses before any earlier canonical settlement. */
PROBE_FN void loans_preflight_all_container_finalizers(void) {
    static spx_shb_stored saved_rows[3];
    static spx_shb_vec saved_vec;
    static spx_shb_iter saved_iter;
    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 6u, &a); partition(&a, 3u, 2u, &v);
    b = spx_shb_row_owner(&v, 1u); loan = spx_shb_loan_begin(&reg, &b);
    saved_vec = v; memcpy(saved_rows, v.rows, sizeof(saved_rows));
    assert(reg.loans == 1u);
    EXPECT_STOP("outstanding borrow", (void)spx_shb_vec_move(&reg, &v));
    EXPECT_STOP("outstanding borrow", (void)spx_shb_iter_from_vec(&reg, &v));
    EXPECT_STOP("outstanding borrow", spx_shb_vec_pop(&reg, &v, &tag, &c, &w));
    EXPECT_STOP("outstanding borrow", (void)spx_shb_vec_reserve_exact(&reg, &v, 1u, &w));
    EXPECT_STOP("outstanding borrow", spx_shb_vec_clear(&reg, &v, &w));
    assert(memcmp(&v, &saved_vec, sizeof(v)) == 0);
    assert(memcmp(v.rows, saved_rows, sizeof(saved_rows)) == 0);
    EXPECT_STOP("outstanding borrow", spx_shb_vec_drop(&reg, &v));
    assert(memcmp(&v, &saved_vec, sizeof(v)) == 0);
    assert(memcmp(v.rows, saved_rows, sizeof(saved_rows)) == 0);
    assert(payload_pool.frees == 0u && metadata_pool.frees == 0u);
    spx_shb_loan_end(&reg, &loan);
    assert(reg.loans == 0u);
    EXPECT_STOP("pop output", spx_shb_vec_pop(&reg, &v, &tag, &c, NULL));
    assert(memcmp(&v, &saved_vec, sizeof(v)) == 0);
    assert(memcmp(v.rows, saved_rows, sizeof(saved_rows)) == 0);
    spx_shb_vec_clear(&reg, &v, &w); spx_shb_vec_drop(&reg, &w);
    assert(payload_pool.frees == 1u); spx_shb_registry_finish(&reg);

    census_reset(); fresh(&reg);
    spx_shb_zeroed(&reg, 6u, &a); partition(&a, 3u, 2u, &v);
    it = spx_shb_iter_from_vec(&reg, &v);
    b = spx_shb_row_owner(&it.vec, 1u); loan = spx_shb_loan_begin(&reg, &b);
    saved_iter = it; memcpy(saved_rows, it.vec.rows, sizeof(saved_rows));
    EXPECT_STOP("outstanding borrow", spx_shb_iter_next(&reg, &it, &tag, &c, &rest));
    EXPECT_STOP("outstanding borrow", spx_shb_iter_drop(&reg, &it));
    assert(memcmp(&it, &saved_iter, sizeof(it)) == 0);
    assert(memcmp(it.vec.rows, saved_rows, sizeof(saved_rows)) == 0);
    assert(payload_pool.frees == 0u && metadata_pool.frees == 0u);
    spx_shb_loan_end(&reg, &loan); spx_shb_iter_drop(&reg, &it);
    assert(payload_pool.frees == 1u); spx_shb_registry_finish(&reg);
}

/* Public copy-out never exports an interior pointer; provider failure
   leaves the private owner intact. */
PROBE_FN void copy_out_and_provider_drop(void) {
    census_reset(); fresh(&reg);
    spx_shb_copy(&reg, slice("hello!", 6u), &a);
    partition(&a, 2u, 3u, &v);
    const uint8_t *base = reg.backing[0].payload;
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    assert(spx_shb_copy_out(&reg, &c, &published) == SPX_SHB_OK && spx_shb_is_vacant(&c));
    assert(published.len == 3u && memcmp(published.ptr, "lo!", 3u) == 0);
    uintptr_t copy_at = (uintptr_t)published.ptr, base_at = (uintptr_t)base;
    assert(copy_at + 3u <= base_at || copy_at >= base_at + 6u);
    assert(provider_pool.live == 1u && payload_pool.frees == 0u);
    spx_shb_provider_drop(&published);
    assert(provider_pool.live == 0u && published.ptr == NULL);
    spx_shb_vec_pop(&reg, &w, &tag, &d, &v);
    uint8_t marker = 0x5au;
    published = (spx_bytes_v1){ .ptr = &marker, .len = UINT64_C(1) };
    provider_pool.fail_at = provider_pool.calls + 1u;
    assert(spx_shb_copy_out(&reg, &d, &published) == SPX_SHB_PROVIDER_ALLOCATION);
    assert(published.ptr == &marker && published.len == 1u && marker == 0x5au);
    assert(view_is(&d, "hel", 3u) && provider_pool.live == 0u && payload_pool.frees == 0u);
    assert(spx_shb_copy_out(&reg, &d, &published) == SPX_SHB_OK && spx_shb_is_vacant(&d));
    assert(published.len == 3u && memcmp(published.ptr, "hel", 3u) == 0);
    assert(provider_pool.live == 1u && payload_pool.frees == 1u);
    spx_shb_provider_drop(&published);
    spx_shb_vec_drop(&reg, &v);
    assert(provider_pool.live == 0u && payload_pool.frees == 1u);
    spx_shb_registry_finish(&reg);
}

/* Stale or forged plans cannot commit; finish detects leaks and holds. */
PROBE_FN void plans_and_final_settlement(void) {
    census_reset(); fresh(&reg); fresh(&other);
    spx_shb_zeroed(&reg, 4u, &a);
    assert(spx_shb_partition_prepare(&reg, &a, 2u, 2u, &plan) == SPX_SHB_OK);
    EXPECT_STOP("hold outlived", spx_shb_registry_finish(&reg));
    stale = a;
    spx_shb_set(&reg, &a, 0u, 9u, &b);
    EXPECT_STOP("differs from its plan", spx_shb_partition_commit(&reg, &plan, &b, &v));
    EXPECT_STOP("stale", spx_shb_partition_commit(&reg, &plan, &stale, &v));
    forged_plan = plan; forged_plan.registry = &other;
    EXPECT_STOP("forged shared Bytes partition plan", spx_shb_partition_commit(&reg, &forged_plan, &b, &v));
    spx_shb_partition_abort(&reg, &plan);
    EXPECT_STOP("forged shared Bytes partition plan", spx_shb_partition_abort(&reg, &forged_plan));
    EXPECT_STOP("live lease", spx_shb_registry_finish(&reg));
    spx_shb_drop(&reg, &b);
    spx_shb_registry_finish(&reg);
    EXPECT_STOP("not the held live registry", spx_shb_zeroed(&reg, 1u, &a));
    assert(payload_pool.live == 0u && metadata_pool.live == 0u);
}

/* Generation exhaustion refuses before any charge or write; no ABA reuse. */
PROBE_FN void generation_exhaustion(void) {
    census_reset(); fresh(&reg);
    reg.next_nonce = UINT64_MAX;
    EXPECT_STOP("generation exhausted", spx_shb_zeroed(&reg, 1u, &a));
    assert(reg.events == 0u && payload_pool.calls == 0u);
    reg.next_nonce = UINT64_MAX - 3u;
    spx_shb_zeroed(&reg, 4u, &a);
    assert(spx_shb_partition_prepare(&reg, &a, 4u, 1u, &plan) == SPX_SHB_OK);
    EXPECT_STOP("generation exhausted", spx_shb_partition_commit(&reg, &plan, &a, &v));
    assert(reg.backing[0].state == SPX_SHB_WHOLE && reg.backing[0].whole.live == 1u && plan.armed);
    spx_shb_partition_abort(&reg, &plan);
    spx_shb_drop(&reg, &a);
    spx_shb_registry_finish(&reg);
    assert(payload_pool.live == 0u && metadata_pool.live == 0u);
}

/* Ordinary Vec<Bytes> transitions keep the legacy failure selection. */
PROBE_FN void vec_operations(void) {
    census_reset(); fresh(&reg);
    assert(spx_shb_vec_with_capacity(&reg, 8193u, &v) == SPX_SHB_VEC_CAPACITY_REFUSED);
    assert(spx_shb_vec_with_capacity(&reg, 1u, &v) == SPX_SHB_OK);
    spx_shb_copy(&reg, slice("A", 1u), &a); spx_shb_copy(&reg, slice("B", 1u), &b);
    assert(spx_shb_vec_push(&reg, &v, &a, &w) == SPX_SHB_OK && spx_shb_is_vacant(&a));
    assert(spx_shb_vec_push(&reg, &w, &b, &v) == SPX_SHB_VEC_FULL && view_is(&b, "B", 1u));
    assert(spx_shb_vec_len(&reg, &w) == 1u);
    assert(spx_shb_vec_set(&reg, &w, 1u, &b, &v) == SPX_SHB_VEC_INDEX && view_is(&b, "B", 1u));
    assert(spx_shb_vec_set(&reg, &w, 0u, &b, &v) == SPX_SHB_OK && payload_pool.frees == 1u);
    assert(spx_shb_vec_reserve_exact(&reg, &v, 3u, &w) == SPX_SHB_OK && spx_shb_vec_capacity(&reg, &w) == 4u);
    spx_slice_u8_v1 s;
    assert(spx_shb_vec_view(&reg, &w, 0u, &s) == SPX_SHB_OK && s.len == 1u && s.ptr[0] == 'B');
    spx_shb_vec_clear(&reg, &w, &v);
    assert(payload_pool.frees == 2u && spx_shb_vec_len(&reg, &v) == 0u);
    spx_shb_vec_pop(&reg, &v, &tag, &c, &w);
    assert(tag == 0u && spx_shb_is_vacant(&c));
    v = spx_shb_vec_move(&reg, &w);
    spx_shb_vec_drop(&reg, &v);
    spx_shb_registry_finish(&reg);
    assert(payload_pool.live == 0u && metadata_pool.live == 0u);
}

int main(void) {
    spx_shb_issuer_init(&issuer);
    exact_bytes_and_no_payload_charge();
    mutation_isolation();
    pop_survives_parent_and_sibling_drop();
    iteration_and_abandonment();
    deep_clone_isolation_and_charges();
    hostile_carriers();
    child_partition_refused();
    geometry_refusals_preserve_source();
    metadata_allocation_failure_releases_unpublished_preparation();
    empty_partition();
    metadata_exact_and_one_over();
    unchanged_quotas_and_plus_one();
    loans_pin_their_generation();
    loans_preflight_all_container_finalizers();
    copy_out_and_provider_drop();
    plans_and_final_settlement();
    generation_exhaustion();
    vec_operations();
    puts("shared-owner Bytes probe: ok");
    return 0;
}
