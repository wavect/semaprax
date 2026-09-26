/* Generated C11 caller driver. The generated client performs framing,
 * admission, dispatch, export and settlement; this file only supplies host
 * values and prints the shared receipt. @INPUT0@.. are field tokens. */
#include <stdlib.h>
#include <string.h>
#include "spx_pg_calling_consumer.c"
static spx_pg_owned_bytes mx_owned(const uint8_t *bytes, size_t length) {
    spx_pg_owned_bytes value = {NULL, length};
    if (length) {
        value.data = (uint8_t *)malloc(length);
        if (!value.data) exit(5);
        memcpy(value.data, bytes, length);
    }
    return value;
}
static void mx_run(const struct mx_case *c) {
    spx_pg_calling_consumer *consumer = NULL;
    if (spx_pg_consumer_open(spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len,
            spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer) != SPX_PG_CONSUMER_OK) exit(3);
    const unsigned cycles = c->kind == MX_REPEATED ? c->cycles : 1;
    for (unsigned cycle = 0; cycle < cycles; ++cycle) {
        char note[128] = "";
        spx_pg_input input = {0}; spx_pg_output output = {0};
        input.@INPUT0@ = mx_owned(c->left, c->left_len);
        input.@INPUT1@ = mx_owned(c->right, c->right_len);
        mx_arm(c->kind);
        spx_pg_consumer_settlement_v1 report;
        const spx_pg_consumer_status status = spx_pg_consumer_transform_with_settlement(consumer, &input, &output, &report);
        const int consumed = !input.@INPUT0@.data && !input.@INPUT1@.data;
        const int present = status == SPX_PG_CONSUMER_OK;
        const long primary = present ? 0 : (long)report.native_status;
        if (!present && (output.@OUTPUT0@.data || output.@OUTPUT1@.data)) exit(6);
        snprintf(note, sizeof(note), "consumer=%d,consumed=%d", (int)status, consumed);
        if (c->kind == MX_REPEATED && cycle + 1 < cycles) {
            mx_emit(c, cycle, primary, report.release_status, present, output.@OUTPUT0@.data,
                output.@OUTPUT0@.len, output.@OUTPUT1@.data, output.@OUTPUT1@.len, note);
            spx_pg_output_free(&output);
            continue;
        }
        const size_t handles = spx_pg_consumer_test_live_handles(consumer);
        int close_status = -1;
        const int closed = spx_pg_consumer_close_checked(&consumer, &close_status);
        const size_t used = strlen(note);
        snprintf(note + used, sizeof(note) - used, ",handles=%zu,close=%d/%d,armed=%zu",
            handles, closed, close_status, mx_armed());
        mx_emit(c, cycle, primary, report.release_status, present, output.@OUTPUT0@.data,
            output.@OUTPUT0@.len, output.@OUTPUT1@.data, output.@OUTPUT1@.len, note);
        spx_pg_output_free(&output);
    }
}
int main(void) {
    for (size_t i = 0; i < MX_CASE_COUNT; ++i) {
        mx_reset();
        mx_run(&mx_cases[i]);
    }
    return 0;
}
