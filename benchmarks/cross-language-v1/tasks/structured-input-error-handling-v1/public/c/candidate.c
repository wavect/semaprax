/* Candidate: classify a bounded versioned record envelope, first-error
 * precedence. Unchanged between the public and hidden phases; the hidden
 * overlay replaces only `main.c`, which #includes this file, mirroring the
 * Rust port's `mod candidate;` / TypeScript port's `import { validate }`
 * separation as closely as this suite's single-entry-file C build allows.
 */
static long long validate(long long kind, long long version, long long payload_len) {
    if (kind != 7) {
        return 1;
    }
    if (version != 1) {
        return 2;
    }
    if (payload_len < 1 || payload_len > 64) {
        return 3;
    }
    return 0;
}
