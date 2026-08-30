#undef malloc
#undef calloc
#undef free

/* This private same-TU witness observes the provider's existing invocation
 * boundary, not a public ABI field or a synthetic exported-wrapper counter.
 * The emitter increments invocation only after all borrowed-input guards,
 * immediately before semantic-context setup and the selected language call. */
static spx_context_v1 context;
static unsigned char context_before[sizeof(context)];
static uint8_t raw[65537], ascii[65537], copied[65537];
static const uint8_t unicode[] = {239, 187, 191, 0, 226, 130, 172, 240, 159, 152, 128};

struct tuple {
    const uint8_t *text; uint64_t text_len;
    const uint8_t *left; uint64_t left_len;
    const uint8_t *right; uint64_t right_len;
};
struct output {
    uint32_t tag;
    uint64_t handle;
    int64_t error;
    uint64_t slots[4];
};
struct observation {
    size_t malloc_calls, calloc_calls, free_calls;
    uint64_t invocation, serial;
};
enum operation { BYTES, TEXT, SOME, NONE, OK, ERR };

static struct observation observe(void) {
    REQUIRE(context.live_slots == 0 && fixture_live == 0);
    return (struct observation){fixture_malloc_calls, fixture_calloc_calls,
        fixture_free_calls, context.invocation,
        atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed)};
}

static void poison(struct output *out) {
    memset(out, 0xa5, sizeof(*out));
    out->tag = UINT32_MAX;
    /* The v8 ABI requires zero here BEFORE checking the borrowed tuple. */
    out->handle = UINT64_C(0);
    out->error = INT64_MIN;
    /* The v9 ABI requires every output byte to start at 0xff. */
    for (size_t index = 0; index < 4; ++index) out->slots[index] = UINT64_MAX;
}

#define ARGUMENTS value.text, value.text_len, value.left, value.left_len, value.right, value.right_len
static spx_owned_data_status_v1 invoke(enum operation operation, struct tuple value,
                                     struct output *out) {
#if FIXTURE_FLAT
    if (operation == BYTES)
        return spx_owned_data_call_spx_tuple_dot_bytes_v1(&context, ARGUMENTS, out->slots);
    REQUIRE(operation == TEXT);
    return spx_owned_data_call_spx_tuple_dot_text_v1(&context, ARGUMENTS, out->slots);
#else
    switch (operation) {
    case BYTES:
        return spx_owned_data_call_spx_tuple_dot_bytes_v1(&context, ARGUMENTS, &out->tag, &out->handle, &out->error);
    case TEXT:
        return spx_owned_data_call_spx_tuple_dot_text_v1(&context, ARGUMENTS, &out->tag, &out->handle, &out->error);
    case SOME: case NONE:
        return spx_owned_data_call_spx_tuple_dot_maybe_v1(&context, ARGUMENTS, (uint8_t)(operation == SOME), &out->tag, &out->handle, &out->error);
    case OK: case ERR:
        return spx_owned_data_call_spx_tuple_dot_result_v1(&context, ARGUMENTS, (uint8_t)(operation == OK), &out->tag, &out->handle, &out->error);
    }
    REQUIRE(false);
    return SPX_OWNED_DATA_ADAPTER_FAILURE;
#endif
}
#undef ARGUMENTS

static void successful(enum operation operation, struct tuple value) {
    struct observation before = observe();
    struct output out;
    poison(&out);
    REQUIRE(invoke(operation, value, &out) == SPX_OWNED_DATA_SUCCESS);
    REQUIRE(context.invocation == before.invocation + UINT64_C(1));
    REQUIRE(fixture_calloc_calls == before.calloc_calls);
    REQUIRE(fixture_free_calls == before.free_calls);
    uint64_t handle;
#if FIXTURE_FLAT
    handle = out.slots[0];
    REQUIRE(out.slots[1] == value.text_len);
    REQUIRE(out.slots[2] == value.left_len && out.slots[3] == value.right_len);
#else
    handle = out.handle;
    REQUIRE(out.tag == (uint32_t)(operation == SOME || operation == ERR));
    REQUIRE(out.error == (operation == ERR ? INT64_C(-7) : INT64_C(0)));
    if (operation == NONE || operation == ERR) {
        REQUIRE(handle == 0 && context.live_slots == 0 && fixture_live == 0);
        REQUIRE(fixture_malloc_calls == before.malloc_calls);
        REQUIRE(atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) == before.serial);
        return;
    }
#endif
    const uint8_t *expected = operation == TEXT ? value.text : value.left;
    uint64_t length = operation == TEXT ? value.text_len : value.left_len;
    REQUIRE(handle != 0 && context.live_slots == 1);
    REQUIRE(fixture_malloc_calls == before.malloc_calls + (length == 0 ? 0u : 1u));
    REQUIRE(fixture_live == (length == 0 ? 0u : 1u));
    REQUIRE(atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) == before.serial + UINT64_C(1));
    uint64_t actual_length = UINT64_MAX;
    REQUIRE(spx_owned_bytes_len_v1(&context, handle, &actual_length) == 0);
    REQUIRE(actual_length == length && length <= UINT64_C(65536));
    memset(copied, 0xa5, sizeof(copied));
    REQUIRE(spx_owned_bytes_copy_v1(&context, handle, length == 0 ? NULL : copied, length) == 0);
    REQUIRE(length == 0 || memcmp(copied, expected, (size_t)length) == 0);
    REQUIRE(copied[length] == UINT8_C(0xa5));
    REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == 0);
    /* spx_bytes_drop calls free even for normalized empty Bytes. */
    REQUIRE(fixture_free_calls == before.free_calls + 1);
    REQUIRE(fixture_live == 0 && context.live_slots == 0);
    REQUIRE(spx_owned_bytes_drop_v1(&context, handle) == SPX_OWNED_DATA_INVALID_HANDLE);
    REQUIRE(fixture_free_calls == before.free_calls + 1);
}

static void accepted(struct tuple value) {
#if FIXTURE_FLAT
    const unsigned count = 2;
#else
    const unsigned count = 6;
#endif
    for (unsigned operation = 0; operation < count; ++operation)
        successful((enum operation)operation, value);
}

static void rejected(struct tuple value) {
#if FIXTURE_FLAT
    const unsigned count = 2;
#else
    const unsigned count = 6;
#endif
    for (unsigned operation = 0; operation < count; ++operation) {
        struct observation before = observe();
        struct output out, original;
        poison(&out);
        memcpy(&original, &out, sizeof(out));
        memcpy(context_before, &context, sizeof(context));
        REQUIRE(invoke((enum operation)operation, value, &out) == SPX_OWNED_DATA_ADAPTER_FAILURE);
        REQUIRE(memcmp(&out, &original, sizeof(out)) == 0);
        REQUIRE(context.invocation == before.invocation);
        REQUIRE(memcmp(&context, context_before, sizeof(context)) == 0);
        REQUIRE(atomic_load_explicit(&spx_owned_data_next_serial_v1, memory_order_relaxed) == before.serial);
        REQUIRE(fixture_malloc_calls == before.malloc_calls);
        REQUIRE(fixture_calloc_calls == before.calloc_calls);
        REQUIRE(fixture_free_calls == before.free_calls);
        REQUIRE(fixture_live == 0 && context.live_slots == 0);
        /* Same physical context, not close/reinitialize. Both views and raw
         * invalid-UTF8 bytes have a successful calibrated control afterward. */
        accepted((struct tuple){unicode, sizeof(unicode), raw, 5, raw + 5, 3});
    }
}

static void exercise(void) {
    for (size_t index = 0; index < sizeof(raw); ++index) {
        raw[index] = (uint8_t)(index % 251);
        ascii[index] = UINT8_C(0x61);
    }
    raw[0] = 0; raw[1] = 255; raw[2] = 195; raw[3] = 40; raw[4] = 128;
    static const struct { uint8_t bytes[4]; uint64_t length; } malformed[] = {
        {{0xff, 0, 0, 0}, 1}, {{0xc0, 0x80, 0, 0}, 2},
        {{0xed, 0xa0, 0x80, 0}, 3}, {{0xf4, 0x90, 0x80, 0x80}, 4},
        {{0xe2, 0x82, 0, 0}, 2}, {{0xf0, 0x80, 0x80, 0x80}, 4},
        {{0x80, 0, 0, 0}, 1}
    };
    for (unsigned repetition = 0; repetition < 4; ++repetition) {
        accepted((struct tuple){NULL, 0, NULL, 0, NULL, 0});
        accepted((struct tuple){unicode, sizeof(unicode), raw, 5, raw + 5, 3});
        for (uint64_t length = 65535; length <= 65536; ++length) {
            accepted((struct tuple){NULL, 0, raw, length, NULL, 0});
            accepted((struct tuple){NULL, 0, NULL, 0, raw, length});
            accepted((struct tuple){NULL, 0, raw, 32768, raw, length - 32768});
            accepted((struct tuple){ascii, length, NULL, 0, NULL, 0});
            accepted((struct tuple){unicode, sizeof(unicode), raw, 20000, raw, length - sizeof(unicode) - 20000});
        }
        /* Every non-null advertised extent has genuinely allocated backing
         * storage. These are length/admission tests, not invalid C pointers. */
        rejected((struct tuple){NULL, 0, raw, 65537, NULL, 0});
        rejected((struct tuple){NULL, 0, NULL, 0, raw, 65537});
        rejected((struct tuple){NULL, 0, raw, 32768, raw, 32769});
        rejected((struct tuple){ascii, 1, raw, 32768, raw, 32768});
        rejected((struct tuple){ascii, 65537, NULL, 0, NULL, 0});
        rejected((struct tuple){unicode, sizeof(unicode), raw, 20000, raw, 45526});
        for (size_t index = 0; index < sizeof(malformed) / sizeof(malformed[0]); ++index)
            rejected((struct tuple){malformed[index].bytes, malformed[index].length, raw, 5, raw, 3});
    }
}

int main(void) {
    REQUIRE(fixture_binary_stdout());
    fixture_calibrate();
    REQUIRE(spx_owned_data_context_init_v1(&context, sizeof(context)) == 0);
    exercise();
    REQUIRE(fixture_live == 0 && context.live_slots == 0);
    for (size_t index = 0; index < 512; ++index) REQUIRE(fixture_pointers[index] == NULL);
    struct observation before = observe();
    REQUIRE(spx_owned_data_context_drop_v1(&context) == 0);
    REQUIRE(fixture_malloc_calls == before.malloc_calls);
    REQUIRE(fixture_calloc_calls == before.calloc_calls);
    REQUIRE(fixture_free_calls == before.free_calls);
    (void)puts("native-owned-tuple-admission-ok");
    return 0;
}
