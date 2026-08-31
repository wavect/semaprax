#undef malloc
#undef free

typedef uint32_t (*fixture_call)(spx_context_v1 *, const uint8_t *, uint64_t,
                                const uint8_t *, uint64_t, int64_t, int64_t,
                                uint32_t *, uint64_t *, int64_t *);
struct fixture_case {
    fixture_call call;
    size_t before_failure, on_success;
    bool branches;
};

#define CALL(name) spx_owned_data_call_spx_stage_dot_##name##_v1
static const struct fixture_case cases[] = {
    {CALL(direct), 1, 2, false},
    {CALL(place), 1, 2, false},
    {CALL(projected), 2, 3, false},
    {CALL(temporary), 2, 3, false},
    {CALL(nested), 1, 2, false},
    {CALL(block), 1, 2, false},
    {CALL(conditional), 1, 2, true},
    {CALL(multiple), 2, 3, false},
};

static void empty(const spx_context_v1 *context) {
    REQUIRE(context->live_slots == 0 && fixture_live == 0);
    for (size_t index = 0; index < 512; ++index)
        REQUIRE(fixture_table[index].pointer == NULL);
}

static void exercise(spx_context_v1 *context, struct fixture_case subject, int64_t branch) {
    const uint8_t left[] = {0, 255, 128};
    const uint8_t right[] = {65, 0, 127, 255, 42};
    const uint8_t *expected = subject.branches && branch != 0 ? right : left;
    const size_t length = subject.branches && branch != 0 ? sizeof(right) : sizeof(left);
    for (int64_t zero = 0; zero <= 1; ++zero) {
        empty(context);
        const size_t allocated = fixture_allocations, freed = fixture_frees;
        uint32_t tag = UINT32_MAX;
        uint64_t handle = 0;
        int64_t error = INT64_MIN;
        const uint32_t status = subject.call(context, left, sizeof(left), right, sizeof(right),
                                             branch, zero, &tag, &handle, &error);
        const size_t allocations = zero == 0 ? subject.before_failure : subject.on_success;
        REQUIRE(fixture_allocations - allocated == allocations);
        if (zero == 0) {
            REQUIRE(status == SPX_OWNED_DATA_SEMANTIC_FAILURE);
            REQUIRE(tag == UINT32_MAX && handle == 0 && error == INT64_MIN);
        } else {
            REQUIRE(status == 0 && tag == 0 && handle != 0 && error == 0);
            REQUIRE(context->live_slots == 1 && fixture_live == 1);
            uint64_t actual = UINT64_MAX;
            REQUIRE(spx_owned_bytes_len_v1(context, handle, &actual) == 0 && actual == length);
            uint8_t copied[7];
            memset(copied, 0xa5, sizeof(copied));
            REQUIRE(spx_owned_bytes_copy_v1(context, handle, copied + 1, actual) == 0);
            REQUIRE(copied[0] == 0xa5 && copied[1 + length] == 0xa5);
            REQUIRE(memcmp(copied + 1, expected, length) == 0);
            REQUIRE(spx_owned_bytes_drop_v1(context, handle) == 0);
            REQUIRE(spx_owned_bytes_drop_v1(context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
        }
        REQUIRE(fixture_frees - freed == allocations);
        empty(context);
    }
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    spx_context_v1 context;
    REQUIRE(spx_owned_data_context_init_v1(&context, sizeof(context)) == 0);
    for (unsigned round = 0; round < 32; ++round)
        for (size_t index = 0; index < sizeof(cases) / sizeof(cases[0]); ++index)
            for (int64_t branch = 0; branch <= 1; ++branch)
                exercise(&context, cases[index], branch);
    REQUIRE(fixture_allocations == fixture_frees && fixture_peak >= 3);
    empty(&context);
    REQUIRE(spx_owned_data_context_drop_v1(&context) == 0);
    empty(&context);
    (void)puts("native-bytes-call-staging-settled");
    return 0;
}
