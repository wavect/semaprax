#undef malloc
#undef free

/* Ordinary emit_c exposes semantic status tokens, not provider ABI statuses.
 * Each failure checks the original normalized cause and untouched result. */
#define FAILURE(call, expected, domain, code_value, sentinel) do { \
    REQUIRE(fixture_live == 0); \
    size_t allocated = fixture_allocations, freed = fixture_frees; \
    spx_status_token status = (call); \
    const struct spx_normalized_status *normalized = spx_status_resolve(&context, status); \
    REQUIRE(status != 0 && normalized != NULL); \
    REQUIRE(strcmp(normalized->domain_id, (domain)) == 0 && normalized->code == (code_value)); \
    REQUIRE(sentinel); \
    REQUIRE(fixture_allocations - allocated == (expected)); \
    REQUIRE(fixture_frees - freed == (expected) && fixture_live == 0); \
} while (0)
#define ARITHMETIC(call, expected, sentinel) \
    FAILURE(call, expected, "semaprax.arithmetic.v1", 4, sentinel)
#define STRING_SUCCESS(call, expected, content) do { \
    REQUIRE(fixture_live == 0); \
    size_t allocated = fixture_allocations, freed = fixture_frees; \
    REQUIRE((call) == 0 && value != &poison); \
    REQUIRE(spx_string_length_v10(value) == sizeof(content) - 1u); \
    REQUIRE(memcmp(value, (content), sizeof(content) - 1u) == 0 && fixture_live == 1); \
    REQUIRE(fixture_allocations - allocated == (expected)); \
    spx_string_drop(value); value = &poison; \
    REQUIRE(fixture_frees - freed == (expected) && fixture_live == 0); \
} while (0)
#define NUMBER_SUCCESS(call, expected, result) do { \
    REQUIRE(fixture_live == 0); \
    size_t allocated = fixture_allocations, freed = fixture_frees; \
    REQUIRE((call) == 0 && number == (result)); \
    REQUIRE(fixture_allocations - allocated == (expected)); \
    REQUIRE(fixture_frees - freed == (expected) && fixture_live == 0); \
    number = INT64_MIN; \
} while (0)

int main(void) {
    REQUIRE(fixture_binary_stdout());
    /* One appended status per failure; the arena intentionally persists across
     * all 32 rounds, so capacity covers the exact 13-failure matrix. */
    enum { repetitions = 32, failures_per_round = 13 };
    struct spx_status_entry entries[repetitions * failures_per_round];
    struct spx_context context = {0};
    REQUIRE(spx_context_init(&context, 99, entries, repetitions * failures_per_round, NULL, NULL, NULL));
    char poison = 'x';
    char *value = &poison;
    int64_t number = INT64_MIN;
    bool boolean = false;
    const uint8_t bytes[] = {7, 0, 255};
    spx_slice_u8_v1 input = {.ptr = bytes, .len = sizeof(bytes)};
    for (unsigned repetition = 0; repetition < repetitions; ++repetition) {
        ARITHMETIC(FIXTURE_BEFORE(&context, 0, &value), 0, value == &poison);
        STRING_SUCCESS(FIXTURE_BEFORE(&context, 1, &value), 1, "\0late");
        ARITHMETIC(FIXTURE_LOCAL(&context, 0, &number), 1, number == INT64_MIN);
        NUMBER_SUCCESS(FIXTURE_LOCAL(&context, 1, &number), 1, 1);
        ARITHMETIC(FIXTURE_LATE(&context, 0, &value), 1, value == &poison);
        STRING_SUCCESS(FIXTURE_LATE(&context, 1, &value), 3, "done\0tail");
        ARITHMETIC(FIXTURE_NESTED(&context, 0, &value), 2, value == &poison);
        STRING_SUCCESS(FIXTURE_NESTED(&context, 1, &value), 4, "done\0tail");
        ARITHMETIC(FIXTURE_CALLEE(&context, 0, &value), 2, value == &poison);
        STRING_SUCCESS(FIXTURE_CALLEE(&context, 1, &value), 3, "done\0tail");
        ARITHMETIC(FIXTURE_CONDITION(&context, 3, &number), 7, number == INT64_MIN);
        NUMBER_SUCCESS(FIXTURE_CONDITION(&context, 10, &number), 9, 4);
        ARITHMETIC(FIXTURE_BODY(&context, 2, &number), 6, number == INT64_MIN);
        NUMBER_SUCCESS(FIXTURE_BODY(&context, 10, &number), 9, 4);
        ARITHMETIC(FIXTURE_MIXED(&context, input, 0, &number), 2, number == INT64_MIN);
        NUMBER_SUCCESS(FIXTURE_MIXED(&context, input, 1, &number), 2, 1);
        FAILURE(FIXTURE_PRE(&context, false, &value), 1,
                "semaprax.contract.v1", 1, value == &poison);
        STRING_SUCCESS(FIXTURE_PRE(&context, true, &value), 2, "body\0");
        FAILURE(FIXTURE_POST(&context, false, &value), 1,
                "semaprax.contract.v1", 2, value == &poison);
        STRING_SUCCESS(FIXTURE_POST(&context, true, &value), 1, "provisional\0tail");
        STRING_SUCCESS(FIXTURE_CLONE(&context, &value), 3, "alpha\0\xe4\xb8\x96\xe7\x95\x8c");
        STRING_SUCCESS(FIXTURE_BRANCH(&context, true, &value), 1, "left\0tail");
        STRING_SUCCESS(FIXTURE_BRANCH(&context, false, &value), 1, "\0right");
        STRING_SUCCESS(FIXTURE_MATCH(&context, 0, true, &value), 2, "yes\0tail");
        STRING_SUCCESS(FIXTURE_MATCH(&context, 0, false, &value), 2, "fallback\0");
        STRING_SUCCESS(FIXTURE_MATCH(&context, 1, false, &value), 1, "fallback\0");
        STRING_SUCCESS(FIXTURE_PRESSURE(&context, &value), 18, "payload");
        STRING_SUCCESS(FIXTURE_EMPTY(&context, &value), 1, "");
        ARITHMETIC(FIXTURE_OPS(&context, 0, &number), 5, number == INT64_MIN);
        NUMBER_SUCCESS(FIXTURE_OPS(&context, 1, &number), 5, 10);
        ARITHMETIC(FIXTURE_FROM_CHAR(&context, 0, &number), 2, number == INT64_MIN);
        NUMBER_SUCCESS(FIXTURE_FROM_CHAR(&context, 1, &number), 2, 1);
        ARITHMETIC(FIXTURE_EQUALITY(&context, 0, &boolean), 3, !boolean);
        size_t allocated = fixture_allocations, freed = fixture_frees;
        REQUIRE(FIXTURE_EQUALITY(&context, 1, &boolean) == 0 && boolean);
        REQUIRE(fixture_allocations - allocated == 3 && fixture_frees - freed == 3);
        boolean = false;
        REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
        REQUIRE(context.status_arena.length == (repetition + 1) * failures_per_round);
    }
    REQUIRE(fixture_peak >= 18);
    for (size_t index = 0; index < 512; ++index) REQUIRE(fixture_table[index].pointer == NULL);
    (void)puts("native-ordinary-strings-settled");
    return 0;
}
