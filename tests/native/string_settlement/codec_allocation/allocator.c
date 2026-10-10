/* Same pointer inventory as the ordinary settlement observer. A failed attempt
 * is never registered as an owner; only the selected ordinal returns NULL. */
static size_t fault_attempts, fault_selected;
static void *fault_malloc(size_t size) {
    ++fault_attempts;
    if (fault_attempts == fault_selected) return NULL;
    return fixture_malloc(size);
}
static void *fault_calloc(size_t count, size_t size) {
    ++fault_attempts;
    if (fault_attempts == fault_selected) return NULL;
    return fixture_calloc(count, size);
}
