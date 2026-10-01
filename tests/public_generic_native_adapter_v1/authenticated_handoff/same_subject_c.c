#include <assert.h>
#include "spx_pg_calling_consumer.c"
extern size_t auth_allocations(void);
extern size_t auth_live(void);
extern size_t auth_calls(void);
#ifndef EXPECT_REFUSAL
#define EXPECT_REFUSAL 14
#endif
#if MODE == 1
/* CANONICAL_FRAME */
#endif
static const uint8_t first[] = {1, 7, 13}, second[] = {2, 11, 17, 23};
static spx_pg_owned_bytes owned(const uint8_t *bytes, size_t len) {
    spx_pg_owned_bytes value = {(uint8_t *)malloc(len), len};
    assert(value.data); memcpy(value.data, bytes, len); return value;
}
int main(void) {
    spx_pg_calling_consumer *consumer = NULL;
    uint8_t *mutated = (uint8_t *)malloc(spx_pg_trusted_descriptor_len);
    assert(mutated); memcpy(mutated, spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len);
    mutated[0] ^= 1;
    assert(spx_pg_consumer_open(mutated, spx_pg_trusted_descriptor_len,
        spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer) == SPX_PG_CONSUMER_DESCRIPTOR_REJECTED);
    free(mutated); assert(!consumer && auth_allocations() == 0 && auth_calls() == 0);
    assert(spx_pg_consumer_open(spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len,
        spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer) == SPX_PG_CONSUMER_OK);
    for (size_t iteration = 0; iteration < 2; ++iteration) {
        spx_pg_input input = {0}; spx_pg_output output = {0};
        input.INPUT0 = owned(first, sizeof(first)); input.INPUT1 = owned(second, sizeof(second));
#if MODE == 1
        uint8_t *encoded = NULL; size_t length = 0;
        assert(spx_pg_ccc_encode_input(&input, &encoded, &length) == SPX_PG_CONSUMER_OK);
        assert(length == sizeof(expected_frame) && memcmp(encoded, expected_frame, length) == 0);
        free(encoded);
        uint64_t generation = 0, next = 0;
        assert(spx_pg_authenticated_generation_v1(consumer->provider, &generation) == 0);
#else
        const size_t before = auth_allocations();
#endif
        spx_pg_consumer_settlement_v1 report;
        const spx_pg_consumer_status status = spx_pg_consumer_transform_with_settlement(consumer, &input, &output, &report);
        assert(!input.INPUT0.data && !input.INPUT0.len && !input.INPUT1.data && !input.INPUT1.len);
        assert(report.release_status == 0);
#if MODE == 1
        assert(report.native_status == EXPECT_CALL_STATUS);
        assert(status == (EXPECT_CALL_STATUS ? SPX_PG_CONSUMER_EXECUTION_FAILED : SPX_PG_CONSUMER_OK));
        assert(auth_calls() == iteration + 1);
        assert(spx_pg_authenticated_generation_v1(consumer->provider, &next) == 0 && next != generation);
        if (EXPECT_CALL_STATUS == 0) {
            assert(output.OUTPUT0.len == sizeof(first) && memcmp(output.OUTPUT0.data, first, sizeof(first)) == 0);
            assert(output.OUTPUT1.len == sizeof(second) && memcmp(output.OUTPUT1.data, second, sizeof(second)) == 0);
        } else { assert(!output.OUTPUT0.data && !output.OUTPUT1.data && !output.OUTPUT0.len && !output.OUTPUT1.len); }
#elif MODE == 2
        const spx_pg_consumer_status expected = EXPECT_REFUSAL == 7 || EXPECT_REFUSAL == 8
            ? SPX_PG_CONSUMER_EXECUTION_FAILED : SPX_PG_CONSUMER_CARRIER_REJECTED;
        if (status != expected || report.native_status != EXPECT_REFUSAL) {
            assert(auth_calls() == 1 && auth_allocations() > before);
            spx_pg_output_free(&output);
            int close_status = -1;
            assert(spx_pg_consumer_close_checked(&consumer, &close_status) == 0 && close_status == 0);
            assert(!consumer && auth_live() == 0);
            return 77; /* A check-omission control must actually reach physical work. */
        }
#else
        assert(status == SPX_PG_CONSUMER_CARRIER_REJECTED && report.native_status == SPX_PG_STATUS_MALFORMED_CARRIER);
#endif
#if MODE != 1
        assert(auth_calls() == 0 && auth_allocations() == before);
        assert(!output.OUTPUT0.data && !output.OUTPUT1.data && !output.OUTPUT0.len && !output.OUTPUT1.len);
#endif
        spx_pg_output_free(&output);
        assert(spx_pg_consumer_test_live_handles(consumer) == 0);
    }
    int close_status = -1;
    assert(spx_pg_consumer_close_checked(&consumer, &close_status) == 0 && close_status == 0);
    assert(!consumer && auth_live() == 0);
    assert(spx_pg_consumer_close_checked(&consumer, &close_status) == 0);
    return 0;
}
