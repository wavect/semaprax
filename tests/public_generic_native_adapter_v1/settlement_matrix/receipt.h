/* Shared matrix receipt: one line per case (and per lifecycle cycle):
 * id primary secondary dispatch live peak order leaf0 leaf1 note
 * "-" marks a value this engine does not observe; "e" is an empty leaf. */
#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
void mx_reset(void);
void mx_arm(uint32_t kind);
size_t mx_dispatches(void);
size_t mx_live(void);
size_t mx_allocations(void);
size_t mx_peak_allocations(void);
size_t mx_peak_handles(void);
size_t mx_armed(void);
void mx_order(char *out, size_t capacity);
#ifdef __cplusplus
}
#endif
struct mx_case {
    const char *id;
    unsigned kind, cycles;
    const uint8_t *left; size_t left_len;
    const uint8_t *right; size_t right_len;
    const uint8_t *frame; size_t frame_len;
};
enum { MX_TRANSFORM = 0, MX_REPEATED = 1, MX_SHORT_EXPORT = 2, MX_WRONG_PATH = 3,
       MX_REFUSAL_EFFECTS = 4, MX_INJECT_EXPORT_RELEASE = 5, MX_INJECT_PREPARE = 6 };
static void mx_leaf(int present, const uint8_t *bytes, size_t length) {
    if (!present) { fputs(" -", stdout); return; }
    if (length == 0) { fputs(" e", stdout); return; }
    fputc(' ', stdout);
    for (size_t i = 0; i < length; ++i) printf("%02x", (unsigned)bytes[i]);
}
static void mx_emit(const struct mx_case *c, unsigned cycle, long primary, long secondary,
                    int present, const uint8_t *l, size_t ln, const uint8_t *r, size_t rn,
                    const char *note) {
    char order[512];
    mx_order(order, sizeof(order));
    if (c->kind == MX_REPEATED) printf("%s#%u", c->id, cycle); else fputs(c->id, stdout);
    printf(" %ld", primary);
    if (secondary < 0) fputs(" -", stdout); else printf(" %ld", secondary);
    printf(" %zu %zu %zu/%zu %s", mx_dispatches(), mx_live(), mx_peak_allocations(),
           mx_peak_handles(), order);
    mx_leaf(present, l, ln);
    mx_leaf(present, r, rn);
    printf(" %s\n", note && note[0] ? note : "-");
}
