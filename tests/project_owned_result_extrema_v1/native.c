#undef malloc
#undef calloc
#undef free

static spx_context_v1 context;
static const uint8_t input[5] = {0, 255, 128, 65, 0};

static void run_case(uint64_t length) {
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    size_t allocations = fixture_malloc_calls, releases = fixture_free_calls;
    size_t zeroed = fixture_calloc_calls;
    uint64_t invocation = context.invocation;
    uint32_t tag = UINT32_MAX;
    uint64_t handle = 0; /* Required native ABI precondition, including Err. */
    int64_t error = INT64_C(12345);
    spx_owned_data_status_v1 status = spx_owned_data_call_spx_result_dot_value_v1(
        &context, input, length, &tag, &handle, &error);
    REQUIRE(context.invocation == invocation + 1);
    if (length < 3) {
        int64_t expected = length == 0 ? INT64_C(0) : length == 1 ? INT64_MIN : INT64_MAX;
        REQUIRE(status == 0 && tag == 1 && handle == 0 && error == expected);
        uint64_t bits = 0;
        memcpy(&bits, &error, sizeof(bits));
        REQUIRE(bits == (length == 0 ? UINT64_C(0) : length == 1 ? UINT64_C(0x8000000000000000) : UINT64_C(0x7fffffffffffffff)));
        REQUIRE(fixture_malloc_calls == allocations && fixture_free_calls == releases);
    } else if (length == 4) {
        REQUIRE(status == SPX_OWNED_DATA_SEMANTIC_FAILURE);
        REQUIRE(tag == UINT32_MAX && handle == 0 && error == INT64_C(12345));
        REQUIRE(fixture_malloc_calls == allocations + 1 && fixture_free_calls == releases + 1);
    } else {
        REQUIRE(status == 0 && tag == 0 && error == 0 && handle != 0);
        REQUIRE(fixture_live == 1 && context.live_slots == 1);
        REQUIRE(fixture_malloc_calls == allocations + 1 && fixture_free_calls == releases);
        uint8_t copied[5] = {0};
        uint64_t actual = UINT64_MAX;
        REQUIRE(spx_owned_bytes_len_v1(&context, handle, &actual) == 0 && actual == length);
        REQUIRE(spx_owned_bytes_copy_v1(&context, handle, copied, length) == 0);
        REQUIRE(memcmp(copied, input, (size_t)length) == 0);
        REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == 0);
        REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
        REQUIRE(fixture_free_calls == releases + 1);
    }
    REQUIRE(fixture_calloc_calls == zeroed && fixture_live == 0 && context.live_slots == 0);
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    fixture_calibrate();
    REQUIRE(spx_owned_data_context_init_v1(&context, sizeof(context)) == 0);
    const uint64_t order[10] = {0, 1, 2, 3, 4, 5, 2, 1, 0, 3};
    for (size_t round = 0; round < 8; round++)
        for (size_t i = 0; i < 10; i++) run_case(order[i]);
    REQUIRE(spx_owned_data_context_drop_v1(&context) == 0 && fixture_live == 0);
    return 0;
}
