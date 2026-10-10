int main(void) {
    REQUIRE(fixture_binary_stdout());
    const struct { const char *quoted; size_t length; } tokens[] = {
        {"\"\"", 2}, {"\"plain\"", 7}, {"\"\xc3\xa9\xf0\x9f\x98\x80\"", 8},
        {"\"\xef\xbb\xbf\xef\xbf\xbf\"", 8},
        {"\"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!?\"", 66}
    };
    for (unsigned repeat = 0; repeat < 16; ++repeat) {
        for (size_t index = 0; index < sizeof tokens / sizeof tokens[0]; ++index) {
            struct spx_status_entry entries[4]; struct spx_context context = {0};
            REQUIRE(spx_context_init(&context, 71, entries, 4, NULL, NULL, NULL));
            size_t length = tokens[index].length, payload = length - 2;
            uint8_t *owner = fixture_malloc(length);
            memcpy(owner, tokens[index].quoted, length);
            spx_slice_u8_v1 input = {.ptr=owner, .len=length};
            size_t before = fixture_allocations, freed = fixture_frees;
            fixture_watch(owner, length);
            char *result = NULL;
            REQUIRE(FIXTURE_COPY(&context, input, &result) == SPX_STATUS_SUCCESS);
            REQUIRE(fixture_allocations == before + 1 && fixture_frees == freed);
            REQUIRE(fixture_copy_calls == (size_t)(payload != 0) && fixture_copy_bytes == payload);
            REQUIRE(fixture_input_copy_calls == (size_t)(payload != 0) && fixture_input_copy_bytes == payload);
            REQUIRE(spx_string_length_v10(result) == payload);
            REQUIRE(!memcmp(result, tokens[index].quoted + 1, payload));
            REQUIRE(!memcmp(owner, tokens[index].quoted, length));
            memset(owner, 0xa5, length); fixture_free(owner); fixture_watch(NULL, 0);
            REQUIRE(!memcmp(result, tokens[index].quoted + 1, payload));
            spx_string_drop(result);
            REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
        }
        /* Execute the full generated decoder as well as its exact materializer.
         * Only copies from this JSON input interval are counted here: unrelated
         * Vec carrier copies are not incorrectly called String payload work. */
        const char json[] = "{\"labels\":[\"hello\xc3\xa9\"],\"rows\":[{\"text\":\"world\xf0\x9f\x98\x80\",\"number\":7}]}";
        uint8_t input_bytes[sizeof json - 1]; memcpy(input_bytes, json, sizeof input_bytes);
        struct spx_status_entry entries[8]; struct spx_context context = {0};
        REQUIRE(spx_context_init(&context, 72, entries, 8, NULL, NULL, NULL));
        spx_slice_u8_v1 input = {.ptr=input_bytes, .len=sizeof input_bytes};
        fixture_watch(input_bytes, sizeof input_bytes);
        int64_t result = -1;
        REQUIRE(FIXTURE_DECODE(&context, input, &result) == SPX_STATUS_SUCCESS && result == 42);
        REQUIRE(fixture_input_copy_calls == 2 && fixture_input_copy_bytes == 16);
        REQUIRE(!memcmp(input_bytes, json, sizeof input_bytes));
        fixture_watch(NULL, 0);
        REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
        REQUIRE(context.call_depth == 0 && context.borrowed_str_depth == 0);
    }
    (void)puts("native-ordinary-strings-settled");
    return 0;
}
