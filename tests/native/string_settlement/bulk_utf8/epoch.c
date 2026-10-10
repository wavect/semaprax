int main(int argc, char **argv) {
    REQUIRE(fixture_binary_stdout()); REQUIRE(argc == 2);
    struct spx_status_entry entries[4]; struct spx_context context = {0};
    REQUIRE(spx_context_init(&context, 81, entries, 4, NULL, NULL, NULL));
    uint64_t epoch = 9;
    uint8_t bytes[131072]; memset(bytes, 'x', sizeof bytes);
    spx_slice_u8_v1 input = {.ptr=bytes, .len=4, .epoch=&epoch, .captured_epoch=9};
    bool bad = false;
    /* Bypass only the foreign-entry meter to model a private nested call.
     * The ordinary operation itself must still authenticate every epoch. */
    context.call_depth = 1;
    if (!strcmp(argv[1], "stale")) {
        input.ptr = (const uint8_t *)(uintptr_t)1; input.captured_epoch = 8; bad = true;
    } else if (!strcmp(argv[1], "stale-empty")) {
        input.ptr = NULL; input.len = 0; input.captured_epoch = 8; bad = true;
    } else if (!strcmp(argv[1], "unleased")) {
        input.ptr = (const uint8_t *)(uintptr_t)1; input.epoch = NULL; bad = true;
    } else if (!strcmp(argv[1], "internal-max")) {
        input.len = sizeof bytes;
    } else if (!strcmp(argv[1], "foreign-max") || !strcmp(argv[1], "foreign-over")) {
        context.call_depth = 0; input.len = !strcmp(argv[1], "foreign-max") ? 65536 : 65537;
        bad = input.len == 65537;
    } else if (!strcmp(argv[1], "current-empty")) {
        input.ptr = NULL; input.len = 0;
    } else {
        REQUIRE(!strcmp(argv[1], "current"));
    }
    uint32_t initial_depth = context.call_depth;
    fixture_watch(input.ptr, (size_t)input.len);
    fixture_forbid_allocation = bad;
    char *result = NULL;
    spx_status_token token = FIXTURE_COPY(&context, input, &result);
    REQUIRE(!bad); /* each hostile process must terminate before reaching here */
    REQUIRE(token == SPX_STATUS_SUCCESS && spx_string_length_v10(result) == input.len);
    REQUIRE(fixture_allocations == 1 && fixture_live == 1 && fixture_frees == 0);
    REQUIRE(fixture_copy_calls == (size_t)(input.len != 0) && fixture_copy_bytes == input.len);
    REQUIRE(!memcmp(result, bytes, (size_t)input.len));
    spx_string_drop(result);
    REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
    REQUIRE(context.call_depth == initial_depth && context.borrowed_str_depth == 0);
    (void)puts("native-ordinary-strings-settled");
    return 0;
}
