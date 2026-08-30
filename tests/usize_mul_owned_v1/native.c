#undef malloc
#undef calloc
#undef free

static spx_context_v1 context;
static const uint8_t input[3] = {19, 23, 29};
static const uint8_t expected[3] = {7, 0, 255};
typedef spx_owned_data_status_v1 (*operation)(spx_context_v1 *, const uint8_t *, uint64_t,
                                            uint32_t *, uint64_t *, int64_t *);

static void run_case(operation call, uint64_t length, uint32_t arithmetic_status) {
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    size_t allocations = fixture_malloc_calls, releases = fixture_free_calls;
    size_t zeroed = fixture_calloc_calls;
    uint64_t invocation = context.invocation;
    uint32_t tag = UINT32_MAX;
    uint64_t handle = 0;
    int64_t error = INT64_MIN;
    spx_owned_data_status_v1 status = call(&context, input, length, &tag, &handle, &error);
    REQUIRE(context.invocation == invocation + 1);
    REQUIRE(fixture_malloc_calls == allocations + 1 && fixture_calloc_calls == zeroed);
    if (arithmetic_status != 0) {
        /* The native public ABI deliberately normalizes language failures to
         * one. The interpreter/Wasm lanes distinguish arithmetic codes 1/3. */
        REQUIRE(status == SPX_OWNED_DATA_SEMANTIC_FAILURE);
        REQUIRE(tag == UINT32_MAX && handle == 0 && error == INT64_MIN);
    } else {
        REQUIRE(status == SPX_OWNED_DATA_SUCCESS && tag == 0 && error == 0 && handle != 0);
        REQUIRE(fixture_live == 1 && context.live_slots == 1 && fixture_free_calls == releases);
        uint64_t actual = UINT64_MAX;
        uint8_t copied[3] = {0, 0, 0};
        REQUIRE(spx_owned_bytes_len_v1(&context, handle, &actual) == 0 && actual == 3);
        REQUIRE(spx_owned_bytes_copy_v1(&context, handle, copied, sizeof(copied)) == 0);
        REQUIRE(memcmp(copied, expected, sizeof(expected)) == 0);
        REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == 0);
        REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
    }
    REQUIRE(fixture_malloc_calls == allocations + 1 && fixture_calloc_calls == zeroed);
    REQUIRE(fixture_free_calls == releases + 1 && fixture_live == 0 && context.live_slots == 0);
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    fixture_calibrate();
    REQUIRE(spx_owned_data_context_init_v1(&context, sizeof(context)) == 0);
    /* Generated below from the same literal case table as the interpreter
     * and Node observer. Repeat after failures in this exact native context. */
    FIXTURE_CASES();
    REQUIRE(spx_owned_data_context_drop_v1(&context) == 0);
    REQUIRE(fixture_live == 0);
    return 0;
}
