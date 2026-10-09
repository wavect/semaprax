/* Test-only allocator observation. Include before the exact generated provider;
 * undefine the macros before the harness. Nothing changes a production ABI. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct fixture_allocation { void *pointer; size_t size; };
static struct fixture_allocation fixture_table[512];
static size_t fixture_allocations, fixture_frees, fixture_live, fixture_peak;

static void fixture_require(bool condition, const char *message, unsigned line) {
    if (!condition) {
        (void)fprintf(stderr, "native String evidence line %u: %s\n", line, message);
        abort();
    }
}
#define REQUIRE(condition) fixture_require((condition), #condition, __LINE__)

static void *fixture_track(void *pointer, size_t size) {
    REQUIRE(pointer != NULL);
    size_t slot = 0;
    while (slot < 512 && fixture_table[slot].pointer != NULL) ++slot;
    REQUIRE(slot < 512);
    for (size_t index = 0; index < 512; ++index)
        REQUIRE(fixture_table[index].pointer != pointer);
    fixture_table[slot] = (struct fixture_allocation){pointer, size};
    ++fixture_allocations;
    ++fixture_live;
    if (fixture_live > fixture_peak) fixture_peak = fixture_live;
    return pointer;
}

static void *fixture_malloc(size_t size) {
    REQUIRE(size != 0);
    return fixture_track(malloc(size), size);
}

#if defined(FIXTURE_TRACK_CALLOC)
/* Owned Bytes zero-fill uses calloc; opt in only for probes that exercise it.
 * Register the physical allocation in the same live-pointer inventory so all
 * duplicate, foreign and interior-free checks remain unchanged. */
static void *fixture_calloc(size_t count, size_t size) {
    REQUIRE(count != 0 && size != 0);
    REQUIRE(count <= SIZE_MAX / size);
    return fixture_track(calloc(count, size), count * size);
}
#endif

static void fixture_free(void *pointer) {
    if (pointer == NULL) return; /* normalized empty Bytes */
    size_t slot = 0;
    while (slot < 512 && fixture_table[slot].pointer != pointer) ++slot;
    REQUIRE(slot < 512); /* catches duplicate, foreign and interior frees */
    REQUIRE(fixture_live != 0);
    fixture_table[slot] = (struct fixture_allocation){0};
    --fixture_live;
    ++fixture_frees;
    free(pointer);
}

#define malloc fixture_malloc
#if defined(FIXTURE_TRACK_CALLOC)
#define calloc fixture_calloc
#endif
#define free fixture_free
