int main(void) {
    REQUIRE(fixture_binary_stdout());
    const struct { const char *bytes; size_t length; } valid[] = {
        {"", 0}, {"A\0B", 3}, {"\xef\xbb\xbf\xc3\xa9", 5},
        {"\xe0\xa0\x80\xef\xbf\xbf\xf4\x8f\xbf\xbf", 10}
    };
    for (unsigned repeat = 0; repeat < 16; ++repeat) {
        for (size_t index = 0; index < sizeof valid / sizeof valid[0]; ++index) {
            struct spx_status_entry entries[4]; struct spx_context context = {0};
            REQUIRE(spx_context_init(&context, 61, entries, 4, NULL, NULL, NULL));
            size_t length = valid[index].length;
            uint8_t *owner = fixture_malloc(length + 2);
            owner[0] = owner[length + 1] = 255; /* invalid bytes outside selected range */
            memcpy(owner + 1, valid[index].bytes, length);
            spx_slice_u8_v1 input = {.ptr=length ? owner + 1 : NULL, .len=length};
            size_t before = fixture_allocations, freed = fixture_frees;
            fixture_watch(input.ptr, length);
            char *result = NULL;
            REQUIRE(FIXTURE_COPY(&context, input, &result) == SPX_STATUS_SUCCESS);
            REQUIRE(fixture_allocations == before + 1 && fixture_frees == freed);
            REQUIRE(fixture_copy_calls == (size_t)(length != 0));
            REQUIRE(fixture_copy_bytes == length);
            REQUIRE(fixture_input_copy_calls == (size_t)(length != 0));
            REQUIRE(fixture_input_copy_bytes == length);
            REQUIRE(spx_string_length_v10(result) == length);
            REQUIRE(memcmp(result, valid[index].bytes, length) == 0);
            REQUIRE(memcmp(owner + 1, valid[index].bytes, length) == 0);
            REQUIRE(owner[0] == 255 && owner[length + 1] == 255);
            size_t allocation_size = 0;
            for (size_t slot = 0; slot < 512; ++slot)
                if (fixture_table[slot].pointer == spx_string_header_v10(result))
                    allocation_size = fixture_table[slot].size;
            REQUIRE(allocation_size == offsetof(struct spx_string_v10, data) + length + 1);
            memset(owner, 0xa5, length + 2);
            fixture_free(owner);
            fixture_watch(NULL, 0);
            REQUIRE(memcmp(result, valid[index].bytes, length) == 0 && result[length] == '\0');
            spx_string_drop(result);
            REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
            REQUIRE(context.call_depth == 0 && context.borrowed_str_depth == 0);
        }
        const uint8_t malformed[][7] = {
            {65,66,67,0xed,0xa0,0x80,68}, /* valid prefix, then surrogate */
            {65,66,67,0xf4,0x90,0x80,0x80} /* valid prefix, then > U+10FFFF */
        };
        for (size_t index = 0; index < sizeof malformed / sizeof malformed[0]; ++index) {
            struct spx_status_entry entries[4]; struct spx_context context = {0};
            REQUIRE(spx_context_init(&context, 62, entries, 4, NULL, NULL, NULL));
            uint8_t input_bytes[7]; memcpy(input_bytes, malformed[index], sizeof input_bytes);
            spx_slice_u8_v1 input = {.ptr=input_bytes, .len=sizeof input_bytes};
            char sentinel[] = "unpublished"; char *result = sentinel;
            size_t before = fixture_allocations, freed = fixture_frees;
            fixture_watch(input_bytes, sizeof input_bytes);
            spx_status_token token = FIXTURE_COPY(&context, input, &result);
            const struct spx_normalized_status *status = spx_status_resolve(&context, token);
            REQUIRE(token != SPX_STATUS_SUCCESS && status != NULL);
            REQUIRE(!strcmp(status->domain_id, "semaprax.convert.v1") && status->code == 1);
            REQUIRE(result == sentinel && fixture_allocations == before && fixture_frees == freed);
            REQUIRE(fixture_input_copy_calls == 0 && fixture_input_copy_bytes == 0);
            REQUIRE(memcmp(input_bytes, malformed[index], sizeof input_bytes) == 0);
            REQUIRE(fixture_live == 0 && context.call_depth == 0 && context.borrowed_str_depth == 0);
        }
        struct spx_status_entry entries[4]; struct spx_context context = {0};
        REQUIRE(spx_context_init(&context, 63, entries, 4, NULL, NULL, NULL));
        size_t before = fixture_allocations, freed = fixture_frees;
        fixture_watch(NULL, 0);
        char *detached = NULL;
        REQUIRE(FIXTURE_DETACHED(&context, &detached) == SPX_STATUS_SUCCESS);
        /* Authored Bytes owner settled inside the helper; only the String lives. */
        REQUIRE(fixture_allocations == before + 2 && fixture_frees == freed + 1 && fixture_live == 1);
        REQUIRE(fixture_copy_calls == 2 && fixture_copy_bytes == 8);
        REQUIRE(spx_string_length_v10(detached) == 4 && !memcmp(detached, "A\0\xc3\xa9", 4));
        spx_string_drop(detached);
        REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
    }
    (void)puts("native-ordinary-strings-settled");
    return 0;
}
