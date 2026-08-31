#undef malloc
#undef calloc
#undef free

static spx_context_v1 context;
static const uint8_t text2[] = {195, 169, 0, 65};
static const uint8_t bytes3[] = {0, 255, 128};
static const uint8_t text6[] = {90, 0, 206, 187, 33};
static const uint8_t bytes7[] = {65, 0, 255, 127, 128, 42};

struct arguments {
    int64_t p0; uint8_t p1;
    const uint8_t *p2; uint64_t n2;
    const uint8_t *p3; uint64_t n3;
    int64_t p4; uint8_t p5;
    const uint8_t *p6; uint64_t n6;
    const uint8_t *p7; uint64_t n7;
};

static struct arguments healthy(void) {
    return (struct arguments){-13, 1, text2, 4, bytes3, 3, 29, 0, text6, 5, bytes7, 6};
}

#define OUT &tag, &handle, &error
static void observe(unsigned arity, struct arguments a, bool good) {
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    uint64_t invocation = context.invocation;
    uint32_t tag = UINT32_MAX;
    uint64_t handle = 0;
    int64_t error = INT64_MIN;
    spx_owned_data_status_v1 status = SPX_OWNED_DATA_ADAPTER_FAILURE;
    switch (arity) {
    case 0: status = spx_owned_data_call_spx_mixed_dot_arity0_v1(&context, OUT); break;
    case 1: status = spx_owned_data_call_spx_mixed_dot_arity1_v1(&context, a.p0, OUT); break;
    case 2: status = spx_owned_data_call_spx_mixed_dot_arity2_v1(&context, a.p0, a.p1, OUT); break;
    case 3: status = spx_owned_data_call_spx_mixed_dot_arity3_v1(&context, a.p0, a.p1, a.p2, a.n2, OUT); break;
    case 4: status = spx_owned_data_call_spx_mixed_dot_arity4_v1(&context, a.p0, a.p1, a.p2, a.n2, a.p3, a.n3, OUT); break;
    case 5: status = spx_owned_data_call_spx_mixed_dot_arity5_v1(&context, a.p0, a.p1, a.p2, a.n2, a.p3, a.n3, a.p4, OUT); break;
    case 6: status = spx_owned_data_call_spx_mixed_dot_arity6_v1(&context, a.p0, a.p1, a.p2, a.n2, a.p3, a.n3, a.p4, a.p5, OUT); break;
    case 7: status = spx_owned_data_call_spx_mixed_dot_arity7_v1(&context, a.p0, a.p1, a.p2, a.n2, a.p3, a.n3, a.p4, a.p5, a.p6, a.n6, OUT); break;
    case 8: status = spx_owned_data_call_spx_mixed_dot_arity8_v1(&context, a.p0, a.p1, a.p2, a.n2, a.p3, a.n3, a.p4, a.p5, a.p6, a.n6, a.p7, a.n7, OUT); break;
    default: REQUIRE(false);
    }
    REQUIRE(status == 0 && tag == 0 && error == 0 && handle != 0);
    REQUIRE(context.invocation == invocation + 1);
    REQUIRE(context.live_slots == 1 && fixture_live == 1);
    uint64_t length = UINT64_MAX;
    REQUIRE(spx_owned_bytes_len_v1(&context, handle, &length) == 0);
    REQUIRE(length == (good ? 2u : 3u));
    uint8_t copy[4] = {0xa5, 0xa5, 0xa5, 0xa5};
    REQUIRE(spx_owned_bytes_copy_v1(&context, handle, copy, length) == 0);
    REQUIRE(memcmp(copy, good ? "ok" : "bad", (size_t)length) == 0);
    REQUIRE(copy[length] == UINT8_C(0xa5));
    size_t releases = fixture_free_calls;
    REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == 0);
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    REQUIRE(fixture_free_calls == releases + 1);
    REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
    REQUIRE(fixture_free_calls == releases + 1);
    REQUIRE(memcmp(copy, good ? "ok" : "bad", (size_t)length) == 0);
}
#undef OUT

static struct arguments wrong(unsigned position) {
    struct arguments a = healthy();
    switch (position) {
    case 0: a.p0 = 29; break;
    case 1: a.p1 = 0; break;
    case 2: a.n2 = 0; break;
    case 3: a.n3 = 0; break;
    case 4: a.p4 = -13; break;
    case 5: a.p5 = 1; break;
    case 6: a.n6 = 0; break;
    case 7: a.n7 = 0; break;
    default: REQUIRE(false);
    }
    return a;
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    fixture_calibrate();
    REQUIRE(spx_owned_data_context_init_v1(&context, sizeof(context)) == 0);
    for (unsigned round = 0; round < 2; ++round) {
        for (unsigned arity = 0; arity <= 8; ++arity) {
            observe(arity, healthy(), true);
            for (unsigned position = 0; position < arity; ++position) {
                observe(arity, wrong(position), false);
                observe(arity, healthy(), true);
            }
        }
        for (unsigned pair = 0; pair < 4; ++pair) {
            struct arguments a = healthy();
            switch (pair) {
            case 0: a.p0 = 29; a.p4 = -13; break;
            case 1: a.p1 = 0; a.p5 = 1; break;
            case 2: a.p2 = text6; a.n2 = 5; a.p6 = text2; a.n6 = 4; break;
            case 3: a.p3 = bytes7; a.n3 = 6; a.p7 = bytes3; a.n7 = 3; break;
            default: REQUIRE(false);
            }
            observe(8, a, false);
            observe(8, healthy(), true);
        }
    }
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    REQUIRE(spx_owned_data_context_drop_v1(&context) == 0);
    (void)puts("mixed-arity-native-ok");
    return 0;
}
