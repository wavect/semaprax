/* Exact observed explicit memcpy traffic, not all machine memory traffic.
 * String-only helper phases have no Vec/Bytes carrier allocations. Full decode
 * includes its internal Bytes builder and per-value exactness-check clones;
 * it is separately labeled total work, never attributed wholly to payloads.
 * One context is retained across all 264 calls and all phases of this case.
 * Ordinary native context has no interpreter Fixed materialization counter. */
static void metrics(const char *phase, size_t allocated, size_t freed,
                    bool string_only) {
    (void)printf("{\"phase\":\"%s\",\"values\":264,\"native_allocations\":%zu,"
                 "\"native_frees\":%zu,\"explicit_memcpy_calls\":%zu,"
                 "\"explicit_memcpy_bytes\":%zu,\"input_memcpy_calls\":%zu,"
                 "\"input_memcpy_bytes\":%zu,\"all_allocations_are_strings\":%s}\n",
                 phase, fixture_allocations - allocated, fixture_frees - freed,
                 fixture_copy_calls, fixture_copy_bytes,
                 fixture_input_copy_calls, fixture_input_copy_bytes,
                 string_only ? "true" : "false");
}
int main(void) {
    REQUIRE(fixture_binary_stdout());
    struct spx_status_entry entries[16]; struct spx_context context = {0};
    REQUIRE(spx_context_init(&context, 93, entries, 16, NULL, NULL, NULL));
    spx_slice_u8_v1 input = {.ptr=token, .len=sizeof token};
    for (unsigned phase = 0; phase < 2; ++phase) {
        size_t allocated = fixture_allocations, freed = fixture_frees;
        fixture_watch(token, sizeof token);
        for (unsigned item = 0; item < 264; ++item) {
            char *result = NULL;
            spx_status_token status = phase == 0
                ? FIXTURE_COPY(&context, input, &result)
                : FIXTURE_RENDER(&context, input, &result);
            REQUIRE(status == SPX_STATUS_SUCCESS);
            const uint8_t *wanted = phase == 0 ? payload : rendered;
            size_t length = phase == 0 ? sizeof payload : sizeof rendered;
            REQUIRE(spx_string_length_v10(result) == length);
            REQUIRE(!memcmp(result, wanted, length));
            spx_string_drop(result);
            REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
            REQUIRE(context.call_depth == 0 && context.borrowed_str_depth == 0);
        }
        metrics(phase == 0 ? "materialize" : "materialize-and-quote",allocated,freed,true);
    }
    size_t allocated = fixture_allocations, freed = fixture_frees;
    fixture_watch(NULL,0);
    int64_t result = 0;
    REQUIRE(FIXTURE_FULL(&context, &result) == SPX_STATUS_SUCCESS && result == 264);
    REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
    REQUIRE(context.call_depth == 0 && context.borrowed_str_depth == 0);
    metrics("internal-input-full-decode-and-exact-values",allocated,freed,false);
    return 0;
}
