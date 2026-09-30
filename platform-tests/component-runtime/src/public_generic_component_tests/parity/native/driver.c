/* One checked call over the exact compiler-derived native moves provider,
 * for the R14 (issue #292) interpreter/Core-provider/Component/native
 * differential parity column. Left and right are hex-encoded on argv; the
 * reported outcome is one line on stdout:
 *   "OK <left-hex> <right-hex>"                -- settled successful call
 *   "CONTRACT_VIOLATION"                        -- checked requires/ensures failed
 *   "REFUSAL <primary_status> <native_status>"  -- provider/codec refusal
 * A nonzero exit code means the driver itself could not perform the call at
 * all (bad argv, allocation failure, open/close refusal); it never reports a
 * checked outcome that way. INPUT0/INPUT1/OUTPUT0/OUTPUT1 are macro-bound by
 * the Rust harness to this exact descriptor's own generated field names. */
#include "spx_pg_calling_consumer.c"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int hex_nibble(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static int decode_hex(const char *text, uint8_t **out_data, size_t *out_len) {
    const size_t text_len = strlen(text);
    if (text_len % 2 != 0) return 0;
    const size_t len = text_len / 2;
    uint8_t *data = len ? (uint8_t *)malloc(len) : NULL;
    if (len && !data) return 0;
    for (size_t i = 0; i < len; ++i) {
        const int high = hex_nibble(text[2 * i]);
        const int low = hex_nibble(text[2 * i + 1]);
        if (high < 0 || low < 0) { free(data); return 0; }
        data[i] = (uint8_t)((high << 4) | low);
    }
    *out_data = data;
    *out_len = len;
    return 1;
}

static void print_hex(const uint8_t *data, size_t len) {
    for (size_t i = 0; i < len; ++i) printf("%02x", data[i]);
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: probe <left-hex> <right-hex>\n"); return 90; }
    uint8_t *left_data = NULL, *right_data = NULL;
    size_t left_len = 0, right_len = 0;
    if (!decode_hex(argv[1], &left_data, &left_len) || !decode_hex(argv[2], &right_data, &right_len)) {
        fprintf(stderr, "malformed hex argv\n");
        free(left_data);
        free(right_data);
        return 91;
    }
    spx_pg_calling_consumer *consumer = NULL;
    if (spx_pg_consumer_open(spx_pg_trusted_descriptor_bytes, spx_pg_trusted_descriptor_len,
            spx_pg_trusted_binding_bytes, spx_pg_trusted_binding_len, &consumer) != SPX_PG_CONSUMER_OK) {
        fprintf(stderr, "consumer open refused\n");
        free(left_data);
        free(right_data);
        return 92;
    }
    spx_pg_input input;
    memset(&input, 0, sizeof(input));
    input.INPUT0.data = left_data;
    input.INPUT0.len = left_len;
    input.INPUT1.data = right_data;
    input.INPUT1.len = right_len;
    spx_pg_output output;
    memset(&output, 0, sizeof(output));
    spx_pg_consumer_settlement_v1 report;
    memset(&report, 0, sizeof(report));
    const spx_pg_consumer_status status =
        spx_pg_consumer_transform_with_settlement(consumer, &input, &output, &report);
    if (status == SPX_PG_CONSUMER_OK) {
        printf("OK ");
        print_hex(output.OUTPUT0.data, output.OUTPUT0.len);
        printf(" ");
        print_hex(output.OUTPUT1.data, output.OUTPUT1.len);
        printf("\n");
    } else if (status == SPX_PG_CONSUMER_EXECUTION_FAILED && report.native_status == 11) {
        printf("CONTRACT_VIOLATION\n");
    } else {
        printf("REFUSAL %d %d\n", (int)status, report.native_status);
    }
    spx_pg_output_free(&output);
    int close_status = 0;
    const spx_pg_consumer_status closed = spx_pg_consumer_close_checked(&consumer, &close_status);
    if (closed != SPX_PG_CONSUMER_OK || close_status != 0) {
        fprintf(stderr, "consumer close refused\n");
        return 93;
    }
    return 0;
}
