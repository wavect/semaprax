/* Test-only allocator observation for the native C11 physical adapter
 * fixture (issue #154). Included before the rendered provider so every raw
 * malloc/free the provider's own bounded allocator performs is independently
 * counted here too, giving a second, external proof of exact settlement on
 * top of the provider's own spx_pg_test_live_allocations_v1 counter.
 * Undefine the macros before this file ends; nothing changes a production
 * ABI. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct fixture_allocation { void *pointer; size_t size; };
static struct fixture_allocation fixture_table[4096];
static size_t fixture_allocations, fixture_frees, fixture_live, fixture_peak;

static void fixture_require(bool condition, const char *message, unsigned line) {
    if (!condition) {
        (void)fprintf(stderr, "native public generic adapter evidence line %u: %s\n", line, message);
        abort();
    }
}
#define REQUIRE(condition) fixture_require((condition), #condition, __LINE__)

static void *fixture_malloc(size_t size) {
    REQUIRE(size != 0);
    void *pointer = malloc(size);
    REQUIRE(pointer != NULL);
    size_t slot = 0;
    while (slot < 4096 && fixture_table[slot].pointer != NULL) ++slot;
    REQUIRE(slot < 4096);
    for (size_t index = 0; index < 4096; ++index)
        REQUIRE(fixture_table[index].pointer != pointer);
    fixture_table[slot] = (struct fixture_allocation){pointer, size};
    ++fixture_allocations;
    ++fixture_live;
    if (fixture_live > fixture_peak) fixture_peak = fixture_live;
    return pointer;
}

static void fixture_free(void *pointer) {
    if (pointer == NULL) return; /* normalized empty leaf */
    size_t slot = 0;
    while (slot < 4096 && fixture_table[slot].pointer != pointer) ++slot;
    REQUIRE(slot < 4096); /* catches duplicate, foreign, and interior frees */
    REQUIRE(fixture_live != 0);
    fixture_table[slot] = (struct fixture_allocation){0};
    --fixture_live;
    ++fixture_frees;
    free(pointer);
}

#define malloc fixture_malloc
#define free fixture_free
