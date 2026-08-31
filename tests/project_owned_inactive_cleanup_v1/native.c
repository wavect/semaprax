/* The provider above is unchanged. Only its libc allocations are observed by
 * the independently calibrated fixture; these macros never affect the probe. */
#undef malloc
#undef calloc
#undef free

static spx_context_v1 context;
static uint8_t large[65536], copied[65537];
static const uint8_t binary[5] = {0, 255, 195, 40, 128};
static const uint8_t recovery[3] = {255, 0, 128};
static uint64_t last_handle;
static unsigned calls, active_calls, inactive_calls;

static uint64_t issuer(void) {
    return atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed);
}

static void empty_inventory(void) {
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    for (size_t index = 0; index < 512; ++index)
        REQUIRE(fixture_pointers[index] == NULL);
    for (uint32_t index = 0; index < context.next_slot; ++index)
        REQUIRE(!context.slots[index].live);
}

static void invoke(unsigned shape, const uint8_t *input, uint64_t length, bool active) {
    empty_inventory();
    size_t allocations = fixture_malloc_calls, releases = fixture_free_calls;
    size_t zeroed = fixture_calloc_calls;
    uint64_t invocation = context.invocation, serial = issuer();
    uint32_t tag = UINT32_C(99);
    uint64_t handle = UINT64_C(0); /* Required raw ABI destination precondition. */
    int64_t error = INT64_C(12345);
    spx_owned_data_status_v1 status;
    if (shape == 0) {
        status = spx_owned_data_call_spx_inactive_dot_maybe_v1(
            &context, input, length, active ? UINT8_C(1) : UINT8_C(0), &tag, &handle, &error);
    } else {
        REQUIRE(shape == 1);
        status = spx_owned_data_call_spx_inactive_dot_result_v1(
            &context, input, length, active ? UINT8_C(1) : UINT8_C(0), &tag, &handle, &error);
    }
    ++calls;
    /* These observations precede ALL host len/copy/drop and context-close
     * operations. A deferred cleanup cannot satisfy the inactive branch. */
    REQUIRE(status == SPX_OWNED_DATA_SUCCESS);
    REQUIRE(context.invocation == invocation + UINT64_C(1));
    REQUIRE(fixture_malloc_calls == allocations + (length == 0 ? 0u : 1u));
    REQUIRE(fixture_calloc_calls == zeroed);
    REQUIRE(tag == (shape == 0 ? (active ? 1u : 0u) : (active ? 0u : 1u)));
    REQUIRE(error == (shape == 1 && !active ? INT64_C(-7) : INT64_C(0)));
    if (!active) {
        ++inactive_calls;
        REQUIRE(handle == 0 && issuer() == serial);
        /* Empty Bytes performs free(NULL): one finalizer call, no physical
         * payload allocation. Nonempty Bytes frees the tracked allocation. */
        REQUIRE(fixture_free_calls == releases + 1);
        empty_inventory();
        return;
    }

    ++active_calls;
    REQUIRE(handle != 0 && handle > last_handle);
    last_handle = handle;
    REQUIRE(issuer() == serial + UINT64_C(1));
    REQUIRE(context.live_slots == 1);
    REQUIRE(fixture_live == (length == 0 ? 0u : 1u));
    REQUIRE(fixture_free_calls == releases);
    uint64_t actual_length = UINT64_MAX;
    REQUIRE(spx_owned_bytes_len_v1(&context, handle, &actual_length) == 0);
    REQUIRE(actual_length == length && length <= UINT64_C(65536));
    memset(copied, 0xa5, sizeof(copied));
    REQUIRE(spx_owned_bytes_copy_v1(&context, handle, length == 0 ? NULL : copied, length) == 0);
    REQUIRE(length == 0 || memcmp(copied, input, (size_t)length) == 0);
    REQUIRE(copied[length] == UINT8_C(0xa5));
    REQUIRE(fixture_malloc_calls == allocations + (length == 0 ? 0u : 1u));
    REQUIRE(fixture_calloc_calls == zeroed && fixture_free_calls == releases);
    REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == 0);
    REQUIRE(fixture_free_calls == releases + 1);
    empty_inventory();
    REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
    REQUIRE(fixture_free_calls == releases + 1);
    REQUIRE(fixture_malloc_calls == allocations + (length == 0 ? 0u : 1u));
    REQUIRE(fixture_calloc_calls == zeroed && issuer() == serial + UINT64_C(1));
    REQUIRE(length == 0 || memcmp(copied, input, (size_t)length) == 0);
    empty_inventory();
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    fixture_calibrate();
    for (size_t index = 0; index < sizeof(large); ++index)
        large[index] = (uint8_t)(index % 251);
    size_t allocations = fixture_malloc_calls, zeroed = fixture_calloc_calls;
    size_t releases = fixture_free_calls;
    uint64_t serial = issuer();
    REQUIRE(spx_owned_data_context_init_v1(&context, sizeof(context)) == 0);
    REQUIRE(fixture_malloc_calls == allocations && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == releases && issuer() == serial);
    const uint8_t *inputs[4] = {NULL, binary, large, large};
    const uint64_t lengths[4] = {0, 5, 65535, 65536};
    for (unsigned round = 0; round < 4; ++round) {
        for (unsigned index = 0; index < 4; ++index) {
            for (unsigned shape = 0; shape < 2; ++shape) {
                invoke(shape, inputs[index], lengths[index], true);
                invoke(shape, inputs[index], lengths[index], false);
                invoke(shape, recovery, sizeof(recovery), true);
            }
        }
    }
    REQUIRE(calls == 96 && active_calls == 64 && inactive_calls == 32);
    REQUIRE(context.invocation == UINT64_C(96) && issuer() == serial + UINT64_C(64));
    REQUIRE(fixture_malloc_calls == allocations + 80 && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == releases + 96);
    empty_inventory();
    REQUIRE(spx_owned_data_context_drop_v1(&context) == 0);
    REQUIRE(fixture_malloc_calls == allocations + 80 && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == releases + 96 && issuer() == serial + UINT64_C(64));
    empty_inventory();
    for (size_t index = 0; index < sizeof(large); ++index)
        REQUIRE(large[index] == (uint8_t)(index % 251));
    REQUIRE(memcmp(binary, (const uint8_t[]){0, 255, 195, 40, 128}, sizeof(binary)) == 0);
    REQUIRE(memcmp(recovery, (const uint8_t[]){255, 0, 128}, sizeof(recovery)) == 0);
    REQUIRE(puts("project-owned-inactive-native-ok") >= 0);
    return 0;
}
