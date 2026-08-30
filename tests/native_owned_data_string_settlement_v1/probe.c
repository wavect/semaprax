#undef malloc
#undef free

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#endif

static void begin(size_t *allocated, size_t *freed, spx_context_v1 *context) {
    REQUIRE(fixture_live == 0 && context->live_slots == 0);
    *allocated = fixture_allocations;
    *freed = fixture_frees;
}

static void failed(size_t allocated, size_t freed, size_t expected,
                   spx_context_v1 *context) {
    REQUIRE(fixture_allocations - allocated == expected);
    REQUIRE(fixture_frees - freed == expected);
    REQUIRE(fixture_live == 0 && context->live_slots == 0);
}

static void consume(size_t allocated, size_t freed, size_t expected,
                    spx_context_v1 *context, uint64_t handle,
                    const uint8_t *bytes, uint64_t length) {
    REQUIRE(handle != 0 && context->live_slots == 1);
    REQUIRE(fixture_allocations - allocated == expected);
    REQUIRE(fixture_live == (length == 0 ? 0u : 1u));
    uint64_t actual = UINT64_MAX;
    REQUIRE(spx_owned_bytes_len_v1(context, handle, &actual) == 0 && actual == length);
    uint8_t copied[8];
    REQUIRE(length <= sizeof(copied));
    memset(copied, 0xa5, sizeof(copied));
    REQUIRE(spx_owned_bytes_copy_v1(context, handle, length == 0 ? NULL : copied, length) == 0);
    REQUIRE(length == 0 || memcmp(copied, bytes, (size_t)length) == 0);
    REQUIRE(spx_owned_bytes_drop_v1(context, handle) == 0);
    REQUIRE(fixture_frees - freed == expected);
    REQUIRE(fixture_live == 0 && context->live_slots == 0);
    REQUIRE(spx_owned_bytes_drop_v1(context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
}

#if FIXTURE_FLAT
static void exercise(spx_context_v1 *context) {
    const uint8_t input[] = {0, 42, 255};
    for (unsigned repetition = 0; repetition < 32; ++repetition) {
        size_t allocated, freed;
        uint64_t slots[2] = {UINT64_MAX, UINT64_MAX};
        begin(&allocated, &freed, context);
        REQUIRE(spx_owned_data_call_spx_s_dot_record_v1(context, input, sizeof(input), 0, slots)
                == SPX_OWNED_DATA_SEMANTIC_FAILURE);
        REQUIRE(slots[0] == UINT64_MAX && slots[1] == UINT64_MAX);
        failed(allocated, freed, 2, context);
        begin(&allocated, &freed, context);
        REQUIRE(spx_owned_data_call_spx_s_dot_record_v1(context, input, sizeof(input), 1, slots) == 0);
        REQUIRE(slots[1] == 42);
        consume(allocated, freed, 2, context, slots[0], input, sizeof(input));
    }
}
#else
#define FAILURE(call, count) do { \
    size_t allocated, freed; begin(&allocated, &freed, context); \
    tag = UINT32_MAX; handle = 0; error = INT64_MIN; \
    REQUIRE((call) == SPX_OWNED_DATA_SEMANTIC_FAILURE); \
    REQUIRE(tag == UINT32_MAX && handle == 0 && error == INT64_MIN); \
    failed(allocated, freed, (count), context); \
} while (0)
#define SUCCESS(call, count, bytes, length) do { \
    size_t allocated, freed; begin(&allocated, &freed, context); \
    tag = UINT32_MAX; handle = 0; error = INT64_MIN; \
    REQUIRE((call) == 0 && tag == 0 && handle != 0 && error == 0); \
    consume(allocated, freed, (count), context, handle, (bytes), (length)); \
} while (0)
#define CALL(name, zero) spx_owned_data_call_spx_s_dot_##name##_v1(context, input, sizeof(input), zero, &tag, &handle, &error)
#define VALUE(name) spx_owned_data_call_spx_s_dot_##name##_v1(context, input, sizeof(input), &tag, &handle, &error)

static void exercise(spx_context_v1 *context) {
    const uint8_t input[] = {0, 42, 255};
    uint32_t tag;
    uint64_t handle;
    int64_t error;
    for (unsigned repetition = 0; repetition < 32; ++repetition) {
        FAILURE(CALL(local, 0), 1);
        SUCCESS(CALL(local, 1), 2, input, sizeof(input));
        FAILURE(CALL(late, 0), 1); /* the later String literal never executes */
        SUCCESS(CALL(late, 1), 3, input, sizeof(input));
        FAILURE(CALL(callee, 0), 2);
        SUCCESS(CALL(callee, 1), 3, input, sizeof(input));
        FAILURE(CALL(mixed, 0), 2);
        SUCCESS(CALL(mixed, 1), 2, input, sizeof(input));
        FAILURE(CALL(mixed_hyphen_late, 0), 2);
        SUCCESS(CALL(mixed_hyphen_late, 1), 2, input, sizeof(input));
        FAILURE(CALL(mixed_hyphen_reverse, 0), 2);
        SUCCESS(CALL(mixed_hyphen_reverse, 1), 2, input, sizeof(input));
        FAILURE(CALL(loop, 2), 3);
        SUCCESS(CALL(loop, 10), 5, input, sizeof(input));
        SUCCESS(VALUE(clone), 6, input, sizeof(input));
        SUCCESS(VALUE(concat), 6, input, sizeof(input));
        SUCCESS(VALUE(nul), 7, input, sizeof(input));
        /* Empty String still allocates its header; empty Bytes has a real
         * handle but no payload allocation. Closing the context is no oracle. */
        SUCCESS(spx_owned_data_call_spx_s_dot_empty_v1(context, NULL, 0, &tag, &handle, &error),
                1, NULL, 0);
    }
}
#endif

int main(void) {
#if defined(_WIN32)
    REQUIRE(_setmode(_fileno(stdout), _O_BINARY) != -1);
#endif
    spx_context_v1 storage;
    REQUIRE(spx_owned_data_context_init_v1(&storage, sizeof(storage)) == 0);
    exercise(&storage); /* one physical native context across every call */
    REQUIRE(fixture_allocations == fixture_frees && fixture_live == 0);
    REQUIRE(fixture_peak >= 2);
    for (size_t index = 0; index < 512; ++index) REQUIRE(fixture_table[index].pointer == NULL);
    REQUIRE(spx_owned_data_context_drop_v1(&storage) == 0);
    REQUIRE(fixture_live == 0);
    (void)puts("standalone-sdk-strings-settled");
    return 0;
}
