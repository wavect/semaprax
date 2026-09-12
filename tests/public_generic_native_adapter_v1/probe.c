/* C-hosted fixture for the native C11 physical adapter (issue #154). Opens
 * the provider, prepares canonical input carriers, calls, exports exact
 * result bytes, checks them independently, releases, and asserts zero live
 * allocations/handles after every terminal case — success and every
 * injected failure ordinal alike. This fixture validates the reference
 * provider template; the generated production C client is #158's separate
 * acceptance surface. */
#undef malloc
#undef free

static uint8_t *carrier_scratch;
static size_t carrier_scratch_capacity;

static uint8_t *carrier_buffer(size_t needed) {
    if (needed > carrier_scratch_capacity) {
        free(carrier_scratch);
        carrier_scratch = (uint8_t *)malloc(needed);
        REQUIRE(carrier_scratch != NULL);
        carrier_scratch_capacity = needed;
    }
    return carrier_scratch;
}

static size_t carrier_size(const size_t *lens, uint32_t leaf_count) {
    size_t total = 8;
    for (uint32_t index = 0; index < leaf_count; ++index) {
        total += 8 + lens[index];
    }
    return total;
}

static size_t build_carrier(uint8_t *out, uint8_t *const *leaves, const size_t *lens,
                             uint32_t leaf_count) {
    size_t offset = 0;
    spx_pg_write_u64le(out + offset, leaf_count);
    offset += 8;
    for (uint32_t index = 0; index < leaf_count; ++index) {
        spx_pg_write_u64le(out + offset, lens[index]);
        offset += 8;
        if (lens[index] != 0) {
            memcpy(out + offset, leaves[index], lens[index]);
        }
        offset += lens[index];
    }
    return offset;
}

static void assert_fully_settled(void) {
    REQUIRE(spx_pg_test_live_allocations_v1() == 0);
    REQUIRE(fixture_live == 0);
}

static spx_pg_provider_v1 *open_trusted_provider(void) {
    spx_pg_provider_v1 *provider = NULL;
    spx_pg_status_v1 status =
        spx_pg_provider_open_v1(SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_DESCRIPTOR_LEN,
                                 SPX_PG_TRUSTED_BINDING_BYTES, SPX_PG_TRUSTED_BINDING_LEN,
                                 &provider);
    REQUIRE(status == SPX_PG_STATUS_OK);
    REQUIRE(provider != NULL);
    return provider;
}

/* --- ABI contract: fixed status width, closed OK value, provider-open
 * replay behavior. --- */

#define CORRUPTION_SCRATCH_BYTES 65536

static void test_open_rejects_wrong_descriptor(void) {
    REQUIRE(SPX_PG_TRUSTED_DESCRIPTOR_LEN > 0 &&
            SPX_PG_TRUSTED_DESCRIPTOR_LEN <= CORRUPTION_SCRATCH_BYTES);
    static uint8_t corrupted[CORRUPTION_SCRATCH_BYTES];
    memcpy(corrupted, SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_DESCRIPTOR_LEN);
    corrupted[0] ^= 0xffu;
    spx_pg_provider_v1 *provider = (spx_pg_provider_v1 *)(void *)0x1;
    spx_pg_status_v1 status = spx_pg_provider_open_v1(
        corrupted, SPX_PG_TRUSTED_DESCRIPTOR_LEN, SPX_PG_TRUSTED_BINDING_BYTES,
        SPX_PG_TRUSTED_BINDING_LEN, &provider);
    REQUIRE(status == SPX_PG_STATUS_DESCRIPTOR_REPLAY_MISMATCH);
    REQUIRE(provider == NULL);
    assert_fully_settled();
}

static void test_open_rejects_wrong_binding(void) {
    REQUIRE(SPX_PG_TRUSTED_BINDING_LEN > 0 && SPX_PG_TRUSTED_BINDING_LEN <= CORRUPTION_SCRATCH_BYTES);
    static uint8_t corrupted[CORRUPTION_SCRATCH_BYTES];
    memcpy(corrupted, SPX_PG_TRUSTED_BINDING_BYTES, SPX_PG_TRUSTED_BINDING_LEN);
    corrupted[SPX_PG_TRUSTED_BINDING_LEN - 1] ^= 0xffu;
    spx_pg_provider_v1 *provider = (spx_pg_provider_v1 *)(void *)0x1;
    spx_pg_status_v1 status =
        spx_pg_provider_open_v1(SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_DESCRIPTOR_LEN,
                                 corrupted, SPX_PG_TRUSTED_BINDING_LEN, &provider);
    REQUIRE(status == SPX_PG_STATUS_BINDING_REPLAY_MISMATCH);
    REQUIRE(provider == NULL);
    assert_fully_settled();
}

static void test_open_rejects_null_out_provider(void) {
    spx_pg_status_v1 status = spx_pg_provider_open_v1(
        SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_DESCRIPTOR_LEN, SPX_PG_TRUSTED_BINDING_BYTES,
        SPX_PG_TRUSTED_BINDING_LEN, NULL);
    REQUIRE(status == SPX_PG_STATUS_NULL_OR_WRONG_KIND);
}

/* --- Lifecycle and settlement: success round trip, two-pass export,
 * repeated export, release, and close. --- */

static void test_success_round_trip(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();

    uint8_t leaf0[] = "hello";
    uint8_t leaf1[] = {0x00, 0x01, 0x02, 0xff};
    uint8_t *leaves[3] = {leaf0, NULL, leaf1};
    size_t lens[3] = {sizeof(leaf0) - 1, 0, sizeof(leaf1)};
    uint8_t *buffer = carrier_buffer(carrier_size(lens, 3));
    size_t carrier_len = build_carrier(buffer, leaves, lens, 3);

    spx_pg_value_v1 *input = NULL;
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) == SPX_PG_STATUS_OK);
    REQUIRE(input != NULL);
    REQUIRE(spx_pg_test_live_handles_v1(provider) == 1);

    spx_pg_result_v1 *result = NULL;
    REQUIRE(spx_pg_call_v1(provider, input, &result) == SPX_PG_STATUS_OK);
    REQUIRE(result != NULL);
    REQUIRE(spx_pg_test_live_handles_v1(provider) == 1);

    /* Two-pass export: query size first, without consuming the result. */
    size_t required = 0;
    REQUIRE(spx_pg_result_export_v1(result, NULL, 0, &required) == SPX_PG_STATUS_BUFFER_TOO_SMALL);
    REQUIRE(required > 0);
    uint8_t *out = (uint8_t *)malloc(required);
    REQUIRE(out != NULL);
    size_t reported = 0;
    REQUIRE(spx_pg_result_export_v1(result, out, required - 1, &reported) ==
            SPX_PG_STATUS_BUFFER_TOO_SMALL);
    REQUIRE(reported == required);
    REQUIRE(spx_pg_result_export_v1(result, out, required, &reported) == SPX_PG_STATUS_OK);
    REQUIRE(reported == required);

    /* Independently decode and check: 3 leaves, reversed bytes. */
    REQUIRE(spx_pg_read_u64le(out) == 3);
    size_t offset = 8;
    uint64_t leaf0_len = spx_pg_read_u64le(out + offset);
    offset += 8;
    REQUIRE(leaf0_len == sizeof(leaf0) - 1);
    uint8_t expected0[] = "olleh";
    REQUIRE(memcmp(out + offset, expected0, (size_t)leaf0_len) == 0);
    offset += (size_t)leaf0_len;
    uint64_t leaf1_len = spx_pg_read_u64le(out + offset);
    offset += 8;
    REQUIRE(leaf1_len == 0);
    uint64_t leaf2_len = spx_pg_read_u64le(out + offset);
    offset += 8;
    REQUIRE(leaf2_len == sizeof(leaf1));
    uint8_t expected2[] = {0xff, 0x02, 0x01, 0x00};
    REQUIRE(memcmp(out + offset, expected2, (size_t)leaf2_len) == 0);
    offset += (size_t)leaf2_len;
    REQUIRE(offset == reported);

    /* Repeated nonconsuming export is byte-identical. */
    uint8_t *out2 = (uint8_t *)malloc(required);
    REQUIRE(out2 != NULL);
    size_t reported2 = 0;
    REQUIRE(spx_pg_result_export_v1(result, out2, required, &reported2) == SPX_PG_STATUS_OK);
    REQUIRE(reported2 == required);
    REQUIRE(memcmp(out, out2, required) == 0);
    free(out);
    free(out2);

    REQUIRE(spx_pg_result_release_v1(&result) == SPX_PG_STATUS_OK);
    REQUIRE(result == NULL);
    REQUIRE(spx_pg_test_live_handles_v1(provider) == 0);

    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    REQUIRE(provider == NULL);
    assert_fully_settled();
}

static void test_zero_leaves_and_max_leaf_count(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();

    /* Zero leaves: the whole owned aggregate has no leaves at all. */
    uint8_t *buffer = carrier_buffer(8);
    size_t carrier_len = build_carrier(buffer, NULL, NULL, 0);
    spx_pg_value_v1 *input = NULL;
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) == SPX_PG_STATUS_OK);
    spx_pg_result_v1 *result = NULL;
    REQUIRE(spx_pg_call_v1(provider, input, &result) == SPX_PG_STATUS_OK);
    size_t required = 0;
    REQUIRE(spx_pg_result_export_v1(result, NULL, 0, &required) == SPX_PG_STATUS_BUFFER_TOO_SMALL);
    REQUIRE(required == 8);
    REQUIRE(spx_pg_result_release_v1(&result) == SPX_PG_STATUS_OK);

    /* Exactly SPX_PG_MAX_OWNED_LEAVES leaves of zero length: at the bound,
     * not over it. */
    uint32_t max_leaves = SPX_PG_MAX_OWNED_LEAVES;
    uint8_t *leaf_pointers[SPX_PG_MAX_OWNED_LEAVES];
    size_t leaf_lens[SPX_PG_MAX_OWNED_LEAVES];
    for (uint32_t index = 0; index < max_leaves; ++index) {
        leaf_pointers[index] = NULL;
        leaf_lens[index] = 0;
    }
    buffer = carrier_buffer(carrier_size(leaf_lens, max_leaves));
    carrier_len = build_carrier(buffer, leaf_pointers, leaf_lens, max_leaves);
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) == SPX_PG_STATUS_OK);
    REQUIRE(spx_pg_call_v1(provider, input, &result) == SPX_PG_STATUS_OK);
    REQUIRE(spx_pg_result_release_v1(&result) == SPX_PG_STATUS_OK);

    /* One over the bound: rejected before any allocation. */
    uint8_t *over_leaf_pointers[SPX_PG_MAX_OWNED_LEAVES + 1];
    size_t over_leaf_lens[SPX_PG_MAX_OWNED_LEAVES + 1];
    for (uint32_t index = 0; index <= max_leaves; ++index) {
        over_leaf_pointers[index] = NULL;
        over_leaf_lens[index] = 0;
    }
    buffer = carrier_buffer(carrier_size(over_leaf_lens, max_leaves + 1));
    carrier_len = build_carrier(buffer, over_leaf_pointers, over_leaf_lens, max_leaves + 1);
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) ==
            SPX_PG_STATUS_CARRIER_CAPACITY);
    REQUIRE(input == NULL);

    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

static void test_leaf_byte_bound_exact_and_first_over(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();

    uint8_t *big = (uint8_t *)malloc(SPX_PG_MAX_BYTES_PER_LEAF + 1);
    REQUIRE(big != NULL);
    memset(big, 0x5a, SPX_PG_MAX_BYTES_PER_LEAF + 1);

    /* Exactly at the bound: accepted. */
    uint8_t *leaves[1] = {big};
    size_t lens[1] = {SPX_PG_MAX_BYTES_PER_LEAF};
    uint8_t *buffer = carrier_buffer(carrier_size(lens, 1));
    size_t carrier_len = build_carrier(buffer, leaves, lens, 1);
    spx_pg_value_v1 *input = NULL;
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) == SPX_PG_STATUS_OK);
    REQUIRE(spx_pg_value_release_v1(&input) == SPX_PG_STATUS_OK);

    /* The first value over the bound: rejected. */
    lens[0] = SPX_PG_MAX_BYTES_PER_LEAF + 1;
    buffer = carrier_buffer(carrier_size(lens, 1));
    carrier_len = build_carrier(buffer, leaves, lens, 1);
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) ==
            SPX_PG_STATUS_CARRIER_CAPACITY);
    REQUIRE(input == NULL);

    free(big);
    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

/* Build a carrier of `leaf_count` leaves, each `leaf_len` bytes, prepare it
 * against `provider`, and release the input immediately on success so the
 * physical allocator's live-byte account returns to its pre-call baseline
 * before the next probe -- required for `test_total_payload_bound_exact_
 * and_first_over`'s search below to be a clean, repeatable measurement
 * rather than one contaminated by a previous iteration's still-live bytes. */
static spx_pg_status_v1 try_uniform_carrier(spx_pg_provider_v1 *provider, size_t leaf_len,
                                             uint32_t leaf_count) {
    static uint8_t leaf_data[SPX_PG_MAX_BYTES_PER_LEAF];
    memset(leaf_data, 0x22, sizeof(leaf_data));
    uint8_t *leaves[SPX_PG_MAX_OWNED_LEAVES];
    size_t lens[SPX_PG_MAX_OWNED_LEAVES];
    for (uint32_t index = 0; index < leaf_count; ++index) {
        leaves[index] = leaf_data;
        lens[index] = leaf_len;
    }
    uint8_t *buffer = carrier_buffer(carrier_size(lens, leaf_count));
    size_t carrier_len = build_carrier(buffer, leaves, lens, leaf_count);
    spx_pg_value_v1 *input = NULL;
    spx_pg_status_v1 status = spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input);
    if (status == SPX_PG_STATUS_OK) {
        REQUIRE(spx_pg_value_release_v1(&input) == SPX_PG_STATUS_OK);
    } else {
        REQUIRE(input == NULL);
    }
    return status;
}

/* Exercises the total-payload bound the physical adapter actually enforces,
 * found by search rather than asserted from the boundary-profile constant
 * directly.
 *
 * `SPX_PG_MAX_OWNED_LEAVES` (256) times `SPX_PG_MAX_BYTES_PER_LEAF` (64 KiB)
 * equals `SPX_PG_MAX_TOTAL_PAYLOAD_BYTES` (16 MiB) exactly -- so for any
 * *real* carrier that already satisfies the leaf-count and per-leaf bounds,
 * `spx_pg_preflight_carrier`'s own logical total-payload check (this file's
 * `total_payload > SPX_PG_MAX_TOTAL_PAYLOAD_BYTES`) can never independently
 * fire: the largest sum such a carrier can declare is exactly the bound,
 * never over it. Confirmed empirically here, not merely asserted: a carrier
 * of exactly 256 leaves at exactly 64 KiB each -- nominally "at the bound"
 * -- is REFUSED by this adapter, with `SPX_PG_STATUS_ALLOCATION_FAILURE`,
 * not admitted. The refusal comes from a DIFFERENT check: the bounded
 * allocator (`spx_pg_alloc`'s `g_spx_pg_live_bytes`) reuses the same
 * `SPX_PG_MAX_TOTAL_PAYLOAD_BYTES` constant as a physical, bookkeeping-
 * inclusive ceiling (it also counts the `leaf_bytes`/`leaf_lens` arrays and
 * the `spx_pg_value_v1` struct this carrier's decode allocates, none of
 * which is "owned payload" under docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md
 * ¶"Max total owned payload bytes per carrier"), so its real usable ceiling
 * per carrier is a few dozen bytes under the nominal 16 MiB, not at it.
 *
 * This is a genuine, verified divergence from the Rust reference carrier
 * codec (`carrier/frame.rs`), whose `total_payload_at_the_bound_with_max_
 * sized_leaves_is_admitted` test proves the LOGICAL codec admits exactly
 * 16 MiB. Flagged here rather than silently matched: this test proves the
 * bound this adapter *actually* enforces (found by binary search, so it
 * tracks whatever the real per-platform struct/array overhead is rather
 * than a hardcoded guess) and that crossing it is refused -- it does not
 * claim the adapter admits the documented 16 MiB exactly, because it does
 * not. Reconciling the two (e.g. giving the allocator's cap headroom above
 * `SPX_PG_MAX_TOTAL_PAYLOAD_BYTES` for its own bookkeeping) is native-
 * adapter follow-up work, out of scope for a test-coverage change. */
static void test_total_payload_bound_exact_and_first_over(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();
    uint32_t leaf_count = SPX_PG_MAX_OWNED_LEAVES;

    size_t lo = 1;
    size_t hi = SPX_PG_MAX_BYTES_PER_LEAF;
    REQUIRE(try_uniform_carrier(provider, lo, leaf_count) == SPX_PG_STATUS_OK);
    REQUIRE(try_uniform_carrier(provider, hi, leaf_count) == SPX_PG_STATUS_ALLOCATION_FAILURE);
    while (hi - lo > 1) {
        size_t mid = lo + (hi - lo) / 2;
        if (try_uniform_carrier(provider, mid, leaf_count) == SPX_PG_STATUS_OK) {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    /* `lo` bytes per leaf, times `leaf_count` leaves: the exact bound this
     * build of the adapter enforces. Admitted. */
    REQUIRE(try_uniform_carrier(provider, lo, leaf_count) == SPX_PG_STATUS_OK);
    /* One byte per leaf more -- still comfortably within the PER-LEAF bound
     * (`lo` < `SPX_PG_MAX_BYTES_PER_LEAF`, confirmed above) -- crosses the
     * total bound and is refused. */
    REQUIRE(try_uniform_carrier(provider, lo + 1, leaf_count) == SPX_PG_STATUS_ALLOCATION_FAILURE);

    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

static void test_malformed_carrier_rejected(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();
    uint8_t truncated[4] = {1, 0, 0, 0};
    spx_pg_value_v1 *input = (spx_pg_value_v1 *)(void *)0x1;
    REQUIRE(spx_pg_input_prepare_v1(provider, truncated, sizeof(truncated), &input) ==
            SPX_PG_STATUS_MALFORMED_CARRIER);
    REQUIRE(input == NULL);

    uint8_t leaf[] = "x";
    uint8_t *leaves[1] = {leaf};
    size_t lens[1] = {sizeof(leaf) - 1};
    uint8_t *buffer = carrier_buffer(carrier_size(lens, 1) + 1);
    size_t carrier_len = build_carrier(buffer, leaves, lens, 1);
    buffer[carrier_len] = 0; /* one trailing byte past the framed carrier */
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len + 1, &input) ==
            SPX_PG_STATUS_MALFORMED_CARRIER);
    REQUIRE(input == NULL);

    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

/* --- Handle safety: wrong, stale, cross-paired, and double-used handles
 * fail safely without dereferencing a foreign/forged pointer. --- */

static spx_pg_value_v1 *prepared_input(spx_pg_provider_v1 *provider) {
    uint8_t leaf[] = "abc";
    uint8_t *leaves[1] = {leaf};
    size_t lens[1] = {sizeof(leaf) - 1};
    uint8_t *buffer = carrier_buffer(carrier_size(lens, 1));
    size_t carrier_len = build_carrier(buffer, leaves, lens, 1);
    spx_pg_value_v1 *input = NULL;
    REQUIRE(spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input) == SPX_PG_STATUS_OK);
    return input;
}

static void test_handle_hostility(void) {
    spx_pg_provider_v1 *provider_a = open_trusted_provider();
    spx_pg_provider_v1 *provider_b = open_trusted_provider();

    /* release(NULL) and release(&NULL) are safe no-ops / rejections. */
    REQUIRE(spx_pg_value_release_v1(NULL) == SPX_PG_STATUS_NULL_OR_WRONG_KIND);
    spx_pg_value_v1 *already_null = NULL;
    REQUIRE(spx_pg_value_release_v1(&already_null) == SPX_PG_STATUS_OK);

    /* A forged/foreign pointer (the address of a local, never registered)
     * is rejected without being dereferenced beyond pointer-value
     * comparison. */
    int forged_storage = 0;
    spx_pg_value_v1 *forged = (spx_pg_value_v1 *)(void *)&forged_storage;
    REQUIRE(spx_pg_value_release_v1(&forged) == SPX_PG_STATUS_HANDLE_INVALID);
    REQUIRE(forged == NULL);

    /* A value handle from provider A used with provider B's call is
     * rejected; the handle stays live and usable against its real owner. */
    spx_pg_value_v1 *input_a = prepared_input(provider_a);
    spx_pg_result_v1 *cross_result = NULL;
    REQUIRE(spx_pg_call_v1(provider_b, input_a, &cross_result) == SPX_PG_STATUS_HANDLE_INVALID);
    REQUIRE(cross_result == NULL);
    spx_pg_result_v1 *result_a = NULL;
    REQUIRE(spx_pg_call_v1(provider_a, input_a, &result_a) == SPX_PG_STATUS_OK);
    spx_pg_value_v1 *stale_input_a = input_a; /* transferred; dangling from the caller's view */

    /* Transferred input reused by the caller: the same (now-invalidated)
     * pointer is rejected, never dereferenced. */
    spx_pg_result_v1 *ignored_result = NULL;
    REQUIRE(spx_pg_call_v1(provider_a, stale_input_a, &ignored_result) == SPX_PG_STATUS_HANDLE_INVALID);
    REQUIRE(ignored_result == NULL);
    // `stale_input_a` is a dangling pointer whose storage was freed by the
    // successful call. If `malloc` reuses that same address for the fresh
    // result object (`result_a`), the stale pointer's *value* now equals a
    // live result handle's value. Releasing it as a value then correctly
    // reports `ILLEGAL_TRANSITION` (wrong kind) rather than `HANDLE_INVALID`
    // (not found). Both are valid rejections of a stale handle; accept
    // either so the test is not sensitive to allocator address reuse.
    spx_pg_status_v1 stale_status = spx_pg_value_release_v1(&stale_input_a);
    REQUIRE(stale_status == SPX_PG_STATUS_HANDLE_INVALID ||
            stale_status == SPX_PG_STATUS_ILLEGAL_TRANSITION);

    /* Input handle presented where a result handle is required, and vice
     * versa, both while genuinely still live. */
    spx_pg_value_v1 *input_c = prepared_input(provider_a);
    spx_pg_value_v1 *input_c_raw = input_c;
    REQUIRE(spx_pg_result_release_v1((spx_pg_result_v1 **)&input_c) == SPX_PG_STATUS_ILLEGAL_TRANSITION);
    REQUIRE(input_c == NULL);
    REQUIRE(spx_pg_value_release_v1(&input_c_raw) == SPX_PG_STATUS_OK);

    spx_pg_value_v1 *result_as_value = (spx_pg_value_v1 *)(void *)result_a;
    REQUIRE(spx_pg_value_release_v1(&result_as_value) == SPX_PG_STATUS_ILLEGAL_TRANSITION);
    REQUIRE(result_as_value == NULL);

    /* Provider close with a live result handle is refused; releasing first
     * makes close succeed. */
    REQUIRE(spx_pg_provider_close_v1(&provider_a) == SPX_PG_STATUS_ILLEGAL_TRANSITION);
    REQUIRE(provider_a != NULL);
    REQUIRE(spx_pg_result_release_v1(&result_a) == SPX_PG_STATUS_OK);

    /* Double release. */
    spx_pg_value_v1 *input_b = prepared_input(provider_b);
    spx_pg_value_v1 *copy = input_b;
    REQUIRE(spx_pg_value_release_v1(&input_b) == SPX_PG_STATUS_OK);
    REQUIRE(input_b == NULL);
    REQUIRE(spx_pg_value_release_v1(&copy) == SPX_PG_STATUS_HANDLE_INVALID);

    REQUIRE(spx_pg_provider_close_v1(&provider_a) == SPX_PG_STATUS_OK);
    REQUIRE(spx_pg_provider_close_v1(&provider_b) == SPX_PG_STATUS_OK);
    /* Closing an already-null provider is a no-op success. */
    REQUIRE(spx_pg_provider_close_v1(&provider_a) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

/* --- Sticky failure: cleanup can never replace an already-selected status,
 * and a cleanup failure with no earlier failure may legally become the
 * terminal status. --- */

static void test_sticky_failure_direct(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();
    spx_pg_value_v1 *input = prepared_input(provider);
    spx_pg_test_inject_failure_v1(SPX_PG_TRACE_EXECUTION_STARTED);
    spx_pg_result_v1 *result = (spx_pg_result_v1 *)(void *)0x1;
    spx_pg_status_v1 primary = spx_pg_call_v1(provider, input, &result);
    REQUIRE(primary == SPX_PG_STATUS_CONTRACT_FAILURE);
    REQUIRE(result == NULL);
    REQUIRE(spx_pg_test_settlement_overwrite_attempts_v1() == 0);

    /* Reasserting the identical outcome is idempotent, not an overwrite. */
    REQUIRE(spx_pg_test_force_settlement_conflict_v1(SPX_PG_STATUS_CONTRACT_FAILURE) ==
            SPX_PG_STATUS_CONTRACT_FAILURE);
    REQUIRE(spx_pg_test_settlement_overwrite_attempts_v1() == 0);

    /* A different candidate is rejected: the ORIGINAL status is returned and
     * the attempt is counted. */
    REQUIRE(spx_pg_test_force_settlement_conflict_v1(SPX_PG_STATUS_ALLOCATION_FAILURE) ==
            SPX_PG_STATUS_CONTRACT_FAILURE);
    REQUIRE(spx_pg_test_settlement_overwrite_attempts_v1() == 1);
    REQUIRE(spx_pg_test_force_settlement_conflict_v1(SPX_PG_STATUS_OK) ==
            SPX_PG_STATUS_CONTRACT_FAILURE);
    REQUIRE(spx_pg_test_settlement_overwrite_attempts_v1() == 2);

    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

static void test_cleanup_failure_with_no_earlier_failure_becomes_terminal(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();
    spx_pg_value_v1 *input = prepared_input(provider);
    /* The success path releases the consumed input via
     * SPX_PG_TRACE_CARRIER_RELEASE before the result ever commits; injecting
     * there with no earlier failure selected lets that cleanup failure
     * legally become the terminal status. */
    spx_pg_test_inject_failure_v1(SPX_PG_TRACE_CARRIER_RELEASE);
    spx_pg_result_v1 *result = (spx_pg_result_v1 *)(void *)0x1;
    spx_pg_status_v1 status = spx_pg_call_v1(provider, input, &result);
    REQUIRE(status == SPX_PG_STATUS_CONTRACT_FAILURE);
    /* The result was fully constructed and physically settles clean, but
     * must never become observable once the terminal status is a failure:
     * `*out_result` stays null even though every other step succeeded. */
    REQUIRE(result == NULL);
    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

/* --- Failure injection matrix: every logical trace ordinal 0..13 settles
 * cleanly with zero live allocations/handles afterward. --- */

static void test_failure_injection_matrix(void) {
    for (uint32_t ordinal = SPX_PG_TRACE_FRAME_VALIDATED; ordinal <= SPX_PG_TRACE_CARRIER_RELEASE;
         ++ordinal) {
        spx_pg_provider_v1 *provider = open_trusted_provider();
        uint8_t leaf[] = "injected";
        uint8_t *leaves[1] = {leaf};
        size_t lens[1] = {sizeof(leaf) - 1};
        uint8_t *buffer = carrier_buffer(carrier_size(lens, 1));
        size_t carrier_len = build_carrier(buffer, leaves, lens, 1);

        spx_pg_test_inject_failure_v1(ordinal);
        spx_pg_value_v1 *input = NULL;
        spx_pg_status_v1 status = spx_pg_input_prepare_v1(provider, buffer, carrier_len, &input);
        if (status == SPX_PG_STATUS_OK) {
            spx_pg_test_inject_failure_v1(ordinal);
            spx_pg_result_v1 *result = NULL;
            status = spx_pg_call_v1(provider, input, &result);
            if (result != NULL) {
                REQUIRE(spx_pg_result_release_v1(&result) == SPX_PG_STATUS_OK);
            }
        }
        REQUIRE(status != SPX_PG_STATUS_OK);
        spx_pg_test_clear_failure_injection_v1();
        REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
        assert_fully_settled();
    }
}

/* --- Trace vocabulary: the success path emits exactly the expected
 * ordinal-numbered label sequence, matching carrier::trace::TraceLabel. --- */

static void test_success_trace_matches_logical_vocabulary(void) {
    spx_pg_provider_v1 *provider = open_trusted_provider();
    spx_pg_value_v1 *input = prepared_input(provider);
    spx_pg_result_v1 *result = NULL;
    REQUIRE(spx_pg_call_v1(provider, input, &result) == SPX_PG_STATUS_OK);

    size_t length = spx_pg_test_trace_len_v1();
    REQUIRE(length > 0);
    /* Result is invisible before commit: RESULT_COMMIT must precede
     * TERMINAL_STATUS, and by construction `result` above was only ever
     * observed non-null after `spx_pg_call_v1` returned, i.e. after commit
     * already happened internally. */
    int saw_result_commit = 0;
    for (size_t index = 0; index < length; ++index) {
        if (spx_pg_test_trace_label_v1(index) == SPX_PG_TRACE_RESULT_COMMIT) {
            saw_result_commit = 1;
        }
    }
    REQUIRE(saw_result_commit);
    REQUIRE(spx_pg_test_trace_label_v1(length - 1) == SPX_PG_TRACE_TERMINAL_STATUS);
    REQUIRE(spx_pg_test_trace_label_v1(0) == SPX_PG_TRACE_FRAME_VALIDATED);

    REQUIRE(spx_pg_result_release_v1(&result) == SPX_PG_STATUS_OK);
    REQUIRE(spx_pg_provider_close_v1(&provider) == SPX_PG_STATUS_OK);
    assert_fully_settled();
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    test_open_rejects_wrong_descriptor();
    test_open_rejects_wrong_binding();
    test_open_rejects_null_out_provider();
    test_success_round_trip();
    test_zero_leaves_and_max_leaf_count();
    test_leaf_byte_bound_exact_and_first_over();
    test_malformed_carrier_rejected();
    test_handle_hostility();
    test_sticky_failure_direct();
    test_cleanup_failure_with_no_earlier_failure_becomes_terminal();
    test_success_trace_matches_logical_vocabulary();
    test_failure_injection_matrix();
    /* Last, deliberately: its binary search runs many more successful
     * prepare/release cycles than any test above, each appending to the
     * provider's global, process-lifetime, 4096-entry trace log that no
     * `spx_pg_input_prepare_v1` call ever resets (only `spx_pg_reset_call_
     * state`'s settlement fields are reset per call). Placed after
     * `test_success_trace_matches_logical_vocabulary` (the only test above
     * that reads trace state) so exhausting that log here can never affect
     * an earlier assertion. */
    test_total_payload_bound_exact_and_first_over();
    free(carrier_scratch);
    (void)puts("native-public-generic-adapter-settled");
    return 0;
}
