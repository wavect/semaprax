/* One registry covers all actual source helpers in this invocation. */
typedef char spx_partition_status_domain[SPX_STATUS_DOMAIN_MAX_BYTES];
static __attribute__((unused)) uint32_t spx_partition_run(
    int64_t *result_out, spx_partition_status_domain *domain_out, uint32_t *code_out
) {
    if (result_out == NULL || domain_out == NULL || code_out == NULL)
        spx_runtime_invariant_failure("partition entry output is absent");
    spx_shb_issuer issuer = {0};
    spx_shb_registry *registry = calloc(1u, sizeof(*registry));
    if (registry == NULL) spx_runtime_invariant_failure("partition registry allocation failed");
    struct spx_context context = {0};
    struct spx_status_entry entries[1] = {0};
    if (!spx_context_init(&context, UINT64_C(1), entries, 1u, NULL, NULL, NULL))
        spx_runtime_invariant_failure("partition entry context initialization failed");
    spx_shb_issuer_init(&issuer);
    spx_shb_attach(&context, registry, &issuer);
    int64_t staged = INT64_C(0);
    spx_status_token status = $ENTRY(&context, &staged);
    spx_shb_detach(&context);
    free(registry);
    if (status != SPX_STATUS_SUCCESS) {
        if (context.status_arena.length != 1u)
            spx_runtime_invariant_failure("partition entry status is not selected");
        size_t domain_size = 0;
        if (!spx_status_domain_size(entries[0].status.domain_id, &domain_size))
            spx_runtime_invariant_failure("partition entry status domain is invalid");
        memcpy(*domain_out, entries[0].status.domain_id, domain_size);
        *code_out = entries[0].status.code;
        return 1u;
    }
    *result_out = staged;
    return 0u;
}
#ifndef SPX_NO_ENTRY_WRAPPER
int main(void) {
    int64_t result = INT64_C(0);
    spx_partition_status_domain domain = {0}; uint32_t code = 0u;
    if (spx_partition_run(&result, &domain, &code) != 0u) {
        fprintf(stderr, "%s:%u\n", domain, code); return 1;
    }
    printf("%lld\n", (long long)result); return 0;
}
#endif
