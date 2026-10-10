/* Count explicit production payload copies independently of malloc/free.
 * Watch a source interval to distinguish JSON materialization from Vec storage.
 * The observer itself and probe setup use unwrapped libc memcpy. */
static size_t fixture_copy_calls, fixture_copy_bytes;
static size_t fixture_input_copy_calls, fixture_input_copy_bytes;
static const uint8_t *fixture_input_start;
static size_t fixture_input_length;
static bool fixture_forbid_allocation;

static void *fixture_bulk_malloc(size_t size) {
    if (fixture_forbid_allocation) {
        (void)fputs("unexpected allocation before stale-slice rejection\n", stderr);
        exit(91);
    }
    return fixture_malloc(size);
}
static void *fixture_bulk_memcpy(void *output, const void *input, size_t length) {
    ++fixture_copy_calls;
    fixture_copy_bytes += length;
    uintptr_t at = (uintptr_t)input, start = (uintptr_t)fixture_input_start;
    if (fixture_input_start && at >= start && at - start <= fixture_input_length
        && length <= fixture_input_length - (at - start)) {
        ++fixture_input_copy_calls;
        fixture_input_copy_bytes += length;
    }
    return memcpy(output, input, length);
}
static void fixture_watch(const uint8_t *input, size_t length) {
    fixture_input_start = input;
    fixture_input_length = length;
    fixture_copy_calls = fixture_copy_bytes = 0;
    fixture_input_copy_calls = fixture_input_copy_bytes = 0;
}
#undef malloc
#define malloc fixture_bulk_malloc
#define memcpy fixture_bulk_memcpy
