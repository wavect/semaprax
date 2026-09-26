/* Raw authenticated provider ABI driver (native C11). Preceded by the v1 and
 * authenticated headers, descriptor/binding/cleanup constants, receipt.h and
 * the generated cases.h. Every status is reported, never repaired. */
#include <stdlib.h>
#include <string.h>
static uint8_t mx_out[2 * 65536 + 64], mx_left[65536], mx_right[65536];
static int mx_decode(size_t length, size_t *ln, size_t *rn) {
    size_t at = 0, sizes[2];
    uint64_t word;
    if (length < 8) return 0;
    memcpy(&word, mx_out, 8); at = 8;
    if (word != 2) return 0;
    for (int leaf = 0; leaf < 2; ++leaf) {
        if (length - at < 8) return 0;
        memcpy(&word, mx_out + at, 8); at += 8;
        if (word > 65536 || word > length - at) return 0;
        sizes[leaf] = (size_t)word;
        memcpy(leaf ? mx_right : mx_left, mx_out + at, sizes[leaf]);
        at += sizes[leaf];
    }
    *ln = sizes[0]; *rn = sizes[1];
    return at == length;
}
static void mx_run(const struct mx_case *c) {
    char note[160] = "";
    spx_pg_provider_v1 *provider = NULL;
    if (spx_pg_provider_open_v1(descriptor, sizeof(descriptor), binding, sizeof(binding), &provider) != 0) {
        fputs("provider-open-refused\n", stderr); exit(3);
    }
    const unsigned cycles = c->kind == MX_REPEATED ? c->cycles : 1;
    for (unsigned cycle = 0; cycle < cycles; ++cycle) {
        uint64_t generation = 0;
        long primary = 0, secondary = -1;
        int present = 0;
        size_t ln = 0, rn = 0, before = mx_allocations();
        note[0] = 0;
        if (spx_pg_authenticated_generation_v1(provider, &generation) != 0) exit(4);
        mx_arm(c->kind);
        spx_pg_value_v1 *input = NULL;
        const spx_pg_status_v1 prepared = spx_pg_authenticated_input_prepare_v1(provider, generation,
            0, cleanup, sizeof(cleanup), c->frame, c->frame_len, &input);
        if (prepared != 0) {
            primary = prepared;
            snprintf(note, sizeof(note), "effects=%zu,input=%d", mx_allocations() - before, input != NULL);
        } else {
            spx_pg_result_v1 *result = NULL, *duplicate = NULL;
            const spx_pg_status_v1 called = spx_pg_call_v1(provider, input, &result);
            const size_t dispatched = mx_dispatches();
            const spx_pg_status_v1 again = spx_pg_call_v1(provider, input, &duplicate);
            const int redispatched = mx_dispatches() != dispatched || duplicate != NULL;
            if (called != 0) {
                primary = called;
                /* Native consumes a failed call's input; a release must refuse. */
                snprintf(note, sizeof(note), "dup=%d,redispatch=%d,release=%d",
                    (int)again, redispatched, (int)spx_pg_value_release_v1(&input));
            } else {
                size_t required = 0;
                const spx_pg_status_v1 probe = spx_pg_result_export_v1(result, NULL, 0, &required);
                long export_status = probe;
                int untouched = 1;
                if (probe == 12 && required <= sizeof(mx_out)) {
                    if (c->kind == MX_SHORT_EXPORT) {
                        memset(mx_out, 0xa5, sizeof(mx_out));
                        size_t short_required = 0;
                        const spx_pg_status_v1 short_status = spx_pg_result_export_v1(result, mx_out,
                            required - 1, &short_required);
                        for (size_t i = 0; i < required; ++i) untouched &= mx_out[i] == 0xa5;
                        snprintf(note, sizeof(note), "short=%d,untouched=%d,", (int)short_status, untouched);
                    }
                    export_status = spx_pg_result_export_v1(result, mx_out, required, &required);
                    if (export_status == 0) present = mx_decode(required, &ln, &rn);
                    if (c->kind == MX_SHORT_EXPORT && export_status == 0) {
                        static uint8_t retry[sizeof(mx_out)];
                        size_t again_required = 0;
                        const spx_pg_status_v1 retried = spx_pg_result_export_v1(result, retry, required, &again_required);
                        const size_t used = strlen(note);
                        snprintf(note + used, sizeof(note) - used, "retry=%d,same=%d,", (int)retried,
                            again_required == required && memcmp(retry, mx_out, required) == 0);
                    }
                }
                primary = export_status;
                spx_pg_result_v1 *stale = result;
#ifndef MX_SKIP_RESULT_RELEASE
                secondary = spx_pg_result_release_v1(&result);
#else
                secondary = 0;
                stale = NULL;
#endif
                const size_t used = strlen(note);
                size_t stale_required = 99;
                snprintf(note + used, sizeof(note) - used, "dup=%d,redispatch=%d,stale=%d",
                    (int)again, redispatched,
                    stale ? (int)spx_pg_result_export_v1(stale, mx_out, sizeof(mx_out), &stale_required) : -1);
            }
        }
        if (c->kind == MX_REPEATED && cycle + 1 < cycles) {
            mx_emit(c, cycle, primary, secondary, present, mx_left, ln, mx_right, rn, note);
            continue;
        }
        const size_t handles = spx_pg_test_live_handles_v1(provider);
        const spx_pg_status_v1 closed = spx_pg_provider_close_v1(&provider);
        const size_t used = strlen(note);
        snprintf(note + used, sizeof(note) - used, "%shandles=%zu,close=%d,armed=%zu",
            used ? "," : "", handles, (int)closed, mx_armed());
        mx_emit(c, cycle, primary, secondary, present, mx_left, ln, mx_right, rn, note);
    }
}
int main(int argc, char **argv) {
    for (size_t i = 0; i < MX_CASE_COUNT; ++i) {
        if (argc > 1 && strcmp(argv[1], mx_cases[i].id) != 0) continue;
        mx_reset();
        mx_run(&mx_cases[i]);
    }
    return 0;
}
