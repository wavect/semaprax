/* Test-only observation, before the exact generated provider. All libc calls
 * in this file precede the macros. The harness undefines them after provider
 * inclusion. No allocator state or instrumentation enters production bytes. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void fixture_require(bool condition, const char *message, unsigned line) {
    if (!condition) {
        (void)fprintf(stderr, "native tuple evidence line %u: %s\n", line, message);
        abort();
    }
}
#define REQUIRE(condition) fixture_require((condition), #condition, __LINE__)

static void *fixture_pointers[512];
static size_t fixture_malloc_calls, fixture_calloc_calls, fixture_free_calls;
static size_t fixture_live;

static void fixture_track(void *pointer) {
    if (pointer == NULL) return;
    size_t slot = 0;
    while (slot < 512 && fixture_pointers[slot] != NULL) ++slot;
    REQUIRE(slot < 512);
    for (size_t index = 0; index < 512; ++index)
        REQUIRE(fixture_pointers[index] != pointer);
    fixture_pointers[slot] = pointer;
    ++fixture_live;
}

static void *fixture_malloc(size_t size) {
    ++fixture_malloc_calls;
    void *pointer = malloc(size);
    fixture_track(pointer);
    return pointer;
}

static void *fixture_calloc(size_t count, size_t size) {
    ++fixture_calloc_calls;
    /* Delegate size overflow and zero-size semantics unchanged to libc. */
    void *pointer = calloc(count, size);
    fixture_track(pointer);
    return pointer;
}

static void fixture_free(void *pointer) {
    ++fixture_free_calls; /* free(NULL) is an observable call, not a live owner. */
    if (pointer != NULL) {
        size_t slot = 0;
        while (slot < 512 && fixture_pointers[slot] != pointer) ++slot;
        REQUIRE(slot < 512 && fixture_live != 0);
        fixture_pointers[slot] = NULL;
        --fixture_live;
    }
    free(pointer);
}

static void fixture_calibrate(void) {
    REQUIRE(fixture_malloc_calls == 0 && fixture_calloc_calls == 0);
    REQUIRE(fixture_free_calls == 0 && fixture_live == 0);
    void *first = fixture_malloc(3);
    unsigned char *second = fixture_calloc(4, 2);
    REQUIRE(first != NULL && second != NULL && first != second);
    for (size_t index = 0; index < 8; ++index) REQUIRE(second[index] == 0);
    REQUIRE(fixture_malloc_calls == 1 && fixture_calloc_calls == 1);
    REQUIRE(fixture_live == 2);
    fixture_free(first);
    fixture_free(second);
    fixture_free(NULL);
    REQUIRE(fixture_free_calls == 3 && fixture_live == 0);
}

#define malloc fixture_malloc
#define calloc fixture_calloc
#define free fixture_free
