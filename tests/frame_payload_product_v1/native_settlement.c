/* Test-only physical allocation witness, after the exact generated provider.
 * The ordinary, uninstrumented execution remains a separate lane. */
#undef malloc
#undef calloc
#undef free

static spx_context_v1 fixture_context;
static size_t initial_malloc, initial_calloc, initial_free;
static uint32_t calls, drops;
static uint8_t copied[65529];

struct observation {
    size_t malloc_calls, calloc_calls, free_calls;
    uint64_t invocation, serial;
};

static struct observation observe(spx_context_v1 *context) {
    REQUIRE(context->live_slots == 0 && fixture_live == 0);
    return (struct observation){fixture_malloc_calls, fixture_calloc_calls,
        fixture_free_calls, context->invocation,
        atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed)};
}

static spx_context_v1 *fixture_begin(void) {
    fixture_calibrate();
    size_t allocations = fixture_malloc_calls, zeroed = fixture_calloc_calls;
    size_t frees = fixture_free_calls;
    REQUIRE(spx_owned_data_context_init_v1(&fixture_context, sizeof(fixture_context)) == 0);
    REQUIRE(fixture_malloc_calls == allocations && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == frees && fixture_live == 0);
    initial_malloc = fixture_malloc_calls;
    initial_calloc = fixture_calloc_calls;
    initial_free = fixture_free_calls;
    return &fixture_context;
}

static void copy_drop(spx_context_v1 *context, uint64_t handle,
                      const uint8_t *expected, uint64_t length) {
    REQUIRE(handle != 0 && context->live_slots == 1);
    REQUIRE(length <= UINT64_C(65528));
    size_t allocations = fixture_malloc_calls, zeroed = fixture_calloc_calls;
    size_t frees = fixture_free_calls, live = fixture_live;
    uint64_t actual = UINT64_MAX;
    REQUIRE(spx_owned_bytes_len_v1(context, handle, &actual) == 0);
    REQUIRE(actual == length);
    memset(copied, 0xa5, sizeof(copied));
    REQUIRE(spx_owned_bytes_copy_v1(context, handle, length == 0 ? NULL : copied, length) == 0);
    REQUIRE(length == 0 || memcmp(copied, expected, (size_t)length) == 0);
    REQUIRE(copied[length] == UINT8_C(0xa5));
    REQUIRE(fixture_malloc_calls == allocations && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == frees && fixture_live == live);
    REQUIRE(spx_owned_bytes_drop_v1(context, handle) == 0);
    ++drops;
    /* Empty active Bytes owns a handle, but only free(NULL), not a payload. */
    REQUIRE(fixture_free_calls == frees + 1);
    REQUIRE(fixture_malloc_calls == allocations && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_live == 0 && context->live_slots == 0);
    REQUIRE(spx_owned_bytes_drop_v1(context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
    REQUIRE(fixture_malloc_calls == allocations && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == frees + 1 && fixture_live == 0);
    REQUIRE(context->live_slots == 0);
}

static int run_case(spx_context_v1 *context, const uint8_t *frame, uint64_t length,
                    uint8_t valid, int64_t expected_error) {
    uint64_t payload_length = valid ? length - UINT64_C(8) : UINT64_C(0);
    const uint8_t *payload = valid ? frame + UINT64_C(8) : NULL;
    /* Same maybe/result/direct order as the uninstrumented corpus. The direct
     * API is never invoked for malformed frames. These are successful language
     * None/Err outcomes, not fabricated provider failures. */
    for (unsigned operation = 0; operation < (valid ? 3u : 2u); ++operation) {
        struct observation before = observe(context);
        uint32_t tag = UINT32_C(99);
        uint64_t handle = UINT64_C(0);
        int64_t error = INT64_C(99);
        spx_owned_data_status_v1 status;
        if (operation == 0) {
            status = spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(
                context, frame, length, &tag, &handle, &error);
        } else if (operation == 1) {
            status = spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(
                context, frame, length, &tag, &handle, &error);
        } else {
            status = spx_owned_data_call_spx_frame_dot_payload_v1(
                context, frame, length, &tag, &handle, &error);
        }
        REQUIRE(status == SPX_OWNED_DATA_SUCCESS);
        ++calls;
        REQUIRE(context->invocation == before.invocation + UINT64_C(1));
        REQUIRE(fixture_calloc_calls == before.calloc_calls);
        REQUIRE(fixture_free_calls == before.free_calls);
        if (valid) {
            REQUIRE(tag == (operation == 0 ? UINT32_C(1) : UINT32_C(0)));
            REQUIRE(error == INT64_C(0));
            REQUIRE(fixture_malloc_calls == before.malloc_calls + (payload_length == 0 ? 0u : 1u));
            REQUIRE(fixture_live == (payload_length == 0 ? 0u : 1u));
            REQUIRE(atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) == before.serial + UINT64_C(1));
            copy_drop(context, handle, payload, payload_length);
        } else {
            REQUIRE(tag == (operation == 0 ? UINT32_C(0) : UINT32_C(1)));
            REQUIRE(error == (operation == 0 ? INT64_C(0) : expected_error));
            REQUIRE(handle == UINT64_C(0));
            REQUIRE(context->live_slots == 0 && fixture_live == 0);
            REQUIRE(fixture_malloc_calls == before.malloc_calls);
            REQUIRE(atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) == before.serial);
        }
    }
    return 0;
}

static void fixture_finish(spx_context_v1 *context) {
    REQUIRE(calls == UINT32_C(23) && drops == UINT32_C(15));
    REQUIRE(fixture_malloc_calls == initial_malloc + 12);
    REQUIRE(fixture_calloc_calls == initial_calloc);
    REQUIRE(fixture_free_calls == initial_free + 15);
    REQUIRE(fixture_live == 0 && context->live_slots == 0);
    for (size_t index = 0; index < 512; ++index) REQUIRE(fixture_pointers[index] == NULL);
    REQUIRE(spx_owned_data_context_drop_v1(context) == 0);
    REQUIRE(fixture_malloc_calls == initial_malloc + 12);
    REQUIRE(fixture_calloc_calls == initial_calloc);
    REQUIRE(fixture_free_calls == initial_free + 15 && fixture_live == 0);
}
