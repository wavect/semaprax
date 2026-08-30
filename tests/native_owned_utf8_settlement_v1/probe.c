#undef malloc
#undef free

/* The provider deliberately normalizes semantic errors to status 1. Wasm's
 * arithmetic code 4 is NOT the public native provider status vocabulary. */
#define FAILURE(call, expected_allocations) do { \
    REQUIRE(fixture_live == 0 && context->live_slots == 0); \
    size_t before_alloc = fixture_allocations, before_free = fixture_frees; \
    tag = UINT32_MAX; handle = 0; error = INT64_MIN; \
    REQUIRE((call) == SPX_OWNED_DATA_SEMANTIC_FAILURE); \
    REQUIRE(tag == UINT32_MAX && handle == 0 && error == INT64_MIN); \
    REQUIRE(fixture_allocations - before_alloc == (expected_allocations)); \
    REQUIRE(fixture_frees - before_free == (expected_allocations)); \
    REQUIRE(fixture_live == 0 && context->live_slots == 0); \
} while (0)

#define SUCCESS(call, expected_allocations, expected, length) do { \
    REQUIRE(fixture_live == 0 && context->live_slots == 0); \
    size_t before_alloc = fixture_allocations, before_free = fixture_frees; \
    tag = UINT32_MAX; handle = 0; error = INT64_MIN; \
    REQUIRE((call) == SPX_OWNED_DATA_SUCCESS); \
    REQUIRE(tag == 0 && handle != 0 && error == 0); \
    REQUIRE(fixture_allocations - before_alloc == (expected_allocations)); \
    REQUIRE(fixture_live == ((length) == 0 ? 0u : 1u)); \
    REQUIRE(context->live_slots == 1); \
    uint64_t actual_length = UINT64_MAX; \
    REQUIRE(spx_owned_bytes_len_v1(context, handle, &actual_length) == 0); \
    REQUIRE(actual_length == (length)); \
    memset(copied, 0xa5, sizeof(copied)); \
    REQUIRE(spx_owned_bytes_copy_v1(context, handle, (length) == 0 ? NULL : copied, actual_length) == 0); \
    REQUIRE(memcmp(copied, (expected), (length)) == 0); \
    REQUIRE(spx_owned_bytes_drop_v1(context, handle) == 0); \
    REQUIRE(fixture_frees - before_free == (expected_allocations)); \
    REQUIRE(fixture_live == 0 && context->live_slots == 0); \
    REQUIRE(spx_owned_bytes_drop_v1(context, handle) == SPX_OWNED_DATA_INVALID_HANDLE); \
} while (0)

#define CALL0(name) spx_owned_data_call_spx_s_dot_##name##_v1(context, &tag, &handle, &error)
#define CALL1(name, value) spx_owned_data_call_spx_s_dot_##name##_v1(context, value, &tag, &handle, &error)
#define MIXED(zero) spx_owned_data_call_spx_s_dot_mixed_v1(context, input, sizeof(input), zero, &tag, &handle, &error)

static void private_emitted_functions(void) {
    /* These calls intentionally bypass the public descriptor and opaque ABI.
     * They exercise emitted-but-unselected semantic functions, not admission. */
    struct spx_status_entry entries[16];
    struct spx_context semantic = {0};
    REQUIRE(spx_context_init(&semantic, 99, entries, 16, NULL, NULL, NULL));
    char poison = 'x';
    char *value = &poison;
    size_t before = fixture_allocations;
    spx_status_token status = FIXTURE_PRIVATE_POST(&semantic, false, &value);
    const struct spx_normalized_status *normalized = spx_status_resolve(&semantic, status);
    REQUIRE(normalized != NULL && normalized->code == SPX_STATUS_CONTRACT_ENSURES_FALSE);
    REQUIRE(strcmp(normalized->domain_id, "semaprax.contract.v1") == 0);
    REQUIRE(value == &poison && fixture_allocations - before == 1 && fixture_live == 0);
    REQUIRE(FIXTURE_PRIVATE_POST(&semantic, true, &value) == 0);
    REQUIRE(fixture_live == 1 && spx_string_length_v10(value) == 4);
    REQUIRE(memcmp(value, "post", 4) == 0);
    spx_string_drop(value);
    REQUIRE(fixture_live == 0);
    const uint8_t clone_bytes[] = {'a','l','p','h','a',0,0xe4,0xb8,0x96,0xe7,0x95,0x8c};
    before = fixture_allocations;
    REQUIRE(FIXTURE_PRIVATE_CLONE(&semantic, &value) == 0);
    REQUIRE(fixture_allocations - before == 3 && fixture_live == 1);
    REQUIRE(spx_string_length_v10(value) == sizeof(clone_bytes));
    REQUIRE(memcmp(value, clone_bytes, sizeof(clone_bytes)) == 0);
    spx_string_drop(value);
    REQUIRE(fixture_live == 0);

    for (unsigned arm = 0; arm < 3; ++arm) {
        before = fixture_allocations;
        REQUIRE(FIXTURE_PRIVATE_MATCH(&semantic, arm == 2 ? 1 : 0, arm == 0, &value) == 0);
        REQUIRE(fixture_allocations - before == (arm == 2 ? 1u : 2u));
        const char *expected = arm == 0 ? "yes" : "fallback";
        size_t length = arm == 0 ? 3u : 8u;
        REQUIRE(fixture_live == 1 && spx_string_length_v10(value) == length);
        REQUIRE(memcmp(value, expected, length) == 0);
        spx_string_drop(value);
        REQUIRE(fixture_live == 0);
    }

    int64_t number = INT64_MIN;
    before = fixture_allocations;
    status = FIXTURE_PRIVATE_INTRINSIC(&semantic, 0, &number);
    REQUIRE(status != 0 && number == INT64_MIN);
    REQUIRE(fixture_allocations - before == 5 && fixture_live == 0);
    REQUIRE(FIXTURE_PRIVATE_INTRINSIC(&semantic, 1, &number) == 0 && number == 9);
    REQUIRE(fixture_live == 0);
    bool boolean = false;
    before = fixture_allocations;
    status = FIXTURE_PRIVATE_EQUALITY(&semantic, 0, &boolean);
    REQUIRE(status != 0 && !boolean);
    REQUIRE(fixture_allocations - before == 3 && fixture_live == 0);
    REQUIRE(FIXTURE_PRIVATE_EQUALITY(&semantic, 1, &boolean) == 0 && boolean);
    REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
}

int main(void) {
    spx_context_v1 storage;
    spx_context_v1 *context = &storage;
    REQUIRE(spx_owned_data_context_init_v1(context, sizeof(storage)) == 0);
    uint32_t tag;
    uint64_t handle;
    int64_t error;
    uint8_t copied[64];
    const uint8_t input[] = {7, 0, 255};
    const uint8_t text[] = {'h','e','l','l','o',0,0xe4,0xb8,0x96,0xe7,0x95,0x8c};
    for (unsigned repetition = 0; repetition < 3; ++repetition) {
        FAILURE(CALL1(before, 0), 0);
        SUCCESS(CALL1(before, 1), 4, "done", 4);
        FAILURE(CALL1(local, 0), 1);
        SUCCESS(CALL1(local, 1), 3, "done", 4);
        FAILURE(CALL1(late, 0), 1);
        SUCCESS(CALL1(late, 1), 4, "done", 4);
        FAILURE(CALL1(nested, 0), 2);
        SUCCESS(CALL1(nested, 1), 5, "done", 4);
        FAILURE(CALL1(callee, 0), 2);
        SUCCESS(CALL1(callee, 1), 4, "done", 4);
        /* One String is retained across the Copy-only loop. Success also
         * allocates the returned String and the provider's owned byte copy. */
        FAILURE(CALL1(condition, 3), 1);
        SUCCESS(CALL1(condition, 10), 3, "done", 4);
        FAILURE(CALL1(body, 2), 1);
        SUCCESS(CALL1(body, 10), 3, "done", 4);
        FAILURE(MIXED(0), 2);
        SUCCESS(MIXED(1), 4, "done", 4);
        SUCCESS(CALL0(clone), 5, "done", 4);
        SUCCESS(CALL1(branch, 0), 3, "done", 4);
        SUCCESS(CALL1(branch, 1), 3, "done", 4);
        SUCCESS(CALL0(pressure), 20, "done", 4);
        SUCCESS(CALL0(empty), 1, "", 0);
        SUCCESS(CALL0(text), 2, text, sizeof(text));
        /* Closure alone is insufficient: heap counters must already be zero. */
        REQUIRE(fixture_live == 0);
        REQUIRE(spx_owned_data_context_drop_v1(context) == 0);
        REQUIRE(fixture_live == 0);
        REQUIRE(spx_owned_data_context_init_v1(context, sizeof(storage)) == 0);
    }
    REQUIRE(fixture_peak >= 18);
    REQUIRE(fixture_allocations == fixture_frees && fixture_live == 0);
    for (size_t index = 0; index < 512; ++index) REQUIRE(fixture_table[index].pointer == NULL);
    REQUIRE(spx_owned_data_context_drop_v1(context) == 0);
    private_emitted_functions();
    (void)puts("native-owned-utf8-settled");
    return 0;
}
