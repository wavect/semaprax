/* Test-only matrix receipt accessors appended after the rendered provider.
 * They read the existing settlement-corpus observers (allocations.c and
 * ../settlement_corpus/observations.c); no production hook or ABI changes. */
static void mx_accept_payload(uint32_t leaf, const uint8_t *bytes, size_t length) {
    (void)leaf; (void)bytes; (void)length;
}
void mx_reset(void) {
    pg_observe_reset();
    fixture_peak = 0;
}
/* Kind 5: injected export failure plus injected result-leaf release failure.
 * Kind 6: injected first provider allocation failure during preparation. */
void mx_arm(uint32_t kind) {
    if (kind == 5) {
        pg_phase_enabled = 1;
        pg_phase_payload_check = mx_accept_payload;
        pg_phase_injections[0] = (struct pg_phase_injection){SPX_PG_PHASE_EXPORT_PENDING, 1, UINT32_MAX, 1};
        pg_phase_injections[1] = (struct pg_phase_injection){SPX_PG_PHASE_LEAF_RELEASED, 1, 1, 1};
    } else if (kind == 6) {
        pg_alloc_attempts = 0;
        pg_fail_allocation_at = 1;
    }
}
size_t mx_dispatches(void) { return pg_endpoint_invocations; }
size_t mx_live(void) { return fixture_live + pg_live_handles; }
size_t mx_allocations(void) { return fixture_allocations; }
size_t mx_peak_allocations(void) { return pg_peak_alloc; }
size_t mx_peak_handles(void) { return pg_peak_handles; }
size_t mx_armed(void) {
    return (size_t)(pg_phase_injections[0].armed + pg_phase_injections[1].armed) +
        (pg_fail_allocation_at != 0 && pg_alloc_attempts < pg_fail_allocation_at);
}
/* Writes "d.l,d.l,..." or "none"; never truncates silently. */
void mx_order(char *out, size_t capacity) {
    size_t used = 0;
    REQUIRE(capacity > 5);
    if (pg_release_count == 0) { (void)snprintf(out, capacity, "none"); return; }
    for (size_t i = 0; i < pg_release_count; ++i) {
        const int written = snprintf(out + used, capacity - used, "%s%u.%u", i ? "," : "",
            (unsigned)pg_release_order[i].direction, (unsigned)pg_release_order[i].leaf);
        REQUIRE(written > 0 && (size_t)written < capacity - used);
        used += (size_t)written;
    }
}
