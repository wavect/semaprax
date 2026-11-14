typedef char spx_partition_status_domain[SPX_STATUS_DOMAIN_MAX_BYTES];

/* 0 publishes provider-owned bytes, 1 publishes a language status,
   and 2 reports provider allocation refusal with all outputs untouched. */
uint32_t spx_partition_copy_out_v1(
    spx_partition_provider_bytes_v1 *result_out,
    spx_partition_status_domain *domain_out,
    uint32_t *code_out
) {
    if (result_out == NULL || domain_out == NULL || code_out == NULL)
        spx_runtime_invariant_failure("partition copy-out output is absent");
    spx_shb_issuer issuer = {0};
    spx_shb_registry *registry = calloc(1u, sizeof(*registry));
    if (registry == NULL) spx_runtime_invariant_failure("partition registry allocation failed");
    struct spx_context context = {0};
    struct spx_status_entry entries[1] = {0};
    if (!spx_context_init(&context, UINT64_C(1), entries, 1u, NULL, NULL, NULL))
        spx_runtime_invariant_failure("partition copy-out context initialization failed");
    spx_shb_issuer_init(&issuer);
    spx_shb_attach(&context, registry, &issuer);

    spx_shb_bytes child = {0};
    spx_partition_provider_bytes_v1 staged = {0};
    spx_status_token status = $ENTRY(&context, &child);
    uint32_t copied = SPX_SHB_OK;
    if (status == SPX_STATUS_SUCCESS) {
        copied = spx_shb_copy_out(registry, &child, &staged);
        if (copied == SPX_SHB_PROVIDER_ALLOCATION) spx_shb_drop(registry, &child);
    }
    spx_shb_detach(&context);
    free(registry);
    if (status != SPX_STATUS_SUCCESS) {
        if (context.status_arena.length != 1u)
            spx_runtime_invariant_failure("partition copy-out status is not selected");
        size_t domain_size = 0;
        if (!spx_status_domain_size(entries[0].status.domain_id, &domain_size))
            spx_runtime_invariant_failure("partition copy-out status domain is invalid");
        memcpy(*domain_out, entries[0].status.domain_id, domain_size);
        *code_out = entries[0].status.code;
        return 1u;
    }
    if (copied == SPX_SHB_PROVIDER_ALLOCATION) return 2u;
    *result_out = staged;
    return 0u;
}

void spx_partition_provider_drop_v1(spx_partition_provider_bytes_v1 *value) {
    spx_shb_provider_drop(value);
}
