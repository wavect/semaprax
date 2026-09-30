/* Direct C ABI caller for the measured native column. Each line is
 * "<case> <raw status> <endpoint entries> <provider allocation delta>
 * <live handles>"; the canonical line then carries the exported flat result. */
#include <assert.h>
#include <stdio.h>
size_t auth_allocations(void);
size_t auth_live(void);
size_t auth_calls(void);
static void refused(const char *id, spx_pg_provider_v1 *provider, uint64_t generation,
    uint32_t ownership, const uint8_t *plan, size_t plan_len, const uint8_t *frame, size_t len) {
    const size_t allocations = auth_allocations(), calls = auth_calls();
    spx_pg_value_v1 *input = NULL;
    const spx_pg_status_v1 status = spx_pg_authenticated_input_prepare_v1(provider, generation,
        ownership, plan, plan_len, frame, len, &input);
    assert(status != 0 && input == NULL);
    assert(printf("%s %d %zu %zu %zu\n", id, (int)status, auth_calls() - calls,
        auth_allocations() - allocations, spx_pg_test_live_handles_v1(provider)) > 0);
}
int main(void) {
    spx_pg_provider_v1 *provider = NULL;
    assert(spx_pg_provider_open_v1(descriptor, sizeof(descriptor), binding, sizeof(binding), &provider) == 0);
    uint64_t generation = 0;
    assert(spx_pg_authenticated_generation_v1(provider, &generation) == 0 && generation != 0);
    refused("stale_generation_replay", provider, generation - 1, 0, cleanup, sizeof(cleanup), canonical, sizeof(canonical));
    refused("future_generation_replay", provider, generation + 1, 0, cleanup, sizeof(cleanup), canonical, sizeof(canonical));
    refused("zero_generation_replay", provider, 0, 0, cleanup, sizeof(cleanup), canonical, sizeof(canonical));
    refused("provider_owned_before_transfer", provider, generation, 1, cleanup, sizeof(cleanup), canonical, sizeof(canonical));
    refused("substituted_cleanup_plan", provider, generation, 0, alternate, sizeof(alternate), canonical, sizeof(canonical));
    refused("substituted_leaf_path", provider, generation, 0, cleanup, sizeof(cleanup), wrong_path, sizeof(wrong_path));
    refused("unknown_leaf_kind_tag_one_over", provider, generation, 0, cleanup, sizeof(cleanup), wrong_tag, sizeof(wrong_tag));
    {
        const size_t allocations = auth_allocations();
        spx_pg_value_v1 *input = NULL;
        const spx_pg_status_v1 status = spx_pg_input_prepare_v1(provider, flat, sizeof(flat), &input);
        assert(status != 0 && input == NULL);
        assert(printf("legacy_flat %d %zu %zu %zu\n", (int)status, auth_calls(),
            auth_allocations() - allocations, spx_pg_test_live_handles_v1(provider)) > 0);
    }
    const size_t allocations = auth_allocations();
    spx_pg_value_v1 *input = NULL;
    assert(spx_pg_authenticated_input_prepare_v1(provider, generation, 0, cleanup, sizeof(cleanup),
        canonical, sizeof(canonical), &input) == 0 && input != NULL);
    spx_pg_result_v1 *result = NULL, *duplicate = NULL;
    const spx_pg_status_v1 status = spx_pg_call_v1(provider, input, &result);
    assert(spx_pg_call_v1(provider, input, &duplicate) == 8 && duplicate == NULL);
    uint8_t out[4096]; size_t required = 0;
    if (status == 0) {
        assert(spx_pg_result_export_v1(result, out, sizeof(out), &required) == 0);
        assert(spx_pg_result_release_v1(&result) == 0);
    }
    assert(result == NULL);
    assert(printf("canonical %d %zu %zu %zu ", (int)status, auth_calls(),
        (size_t)(auth_allocations() > allocations), spx_pg_test_live_handles_v1(provider)) > 0);
    /* ASCII hex avoids platform stdout text-mode byte rewriting. */
    for (size_t i = 0; i < required; ++i) assert(printf("%02x", (unsigned)out[i]) == 2);
    assert(printf("\n") == 1);
    assert(spx_pg_provider_close_v1(&provider) == 0 && provider == NULL);
    assert(auth_live() == 0);
    assert(printf("closed 0 %zu 0 0\n", auth_calls()) > 0);
    return 0;
}
