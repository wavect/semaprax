/* Matrix-only recipe before the exact generated provider. Arm the next real
 * provider malloc after execution, before result-root acquisition. This is
 * not the legacy post-hoc per-leaf trace-rejection seam. */
static int mx_result_acquisition_armed;
static int mx_boundary_hits;
static int mx_occurrence(uint32_t label) {
    if (mx_result_acquisition_armed && label == 7u) {
        mx_result_acquisition_armed = 0;
        pg_fail_allocation_at = pg_alloc_attempts + 1;
        ++mx_boundary_hits;
    }
    int injected = pg_inject_occurrence(label);
    if (injected) ++mx_boundary_hits;
    return injected;
}
#undef SPX_PG_OCCURRENCE_INJECTION
#define SPX_PG_OCCURRENCE_INJECTION(label) mx_occurrence(label)
