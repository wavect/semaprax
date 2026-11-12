/* The wrappers observe all generated allocation sites. Never infer ownership
   merely from a count: reject foreign/interior pointers and duplicate frees. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static struct { void *pointer; size_t size; unsigned zeroed_payload; } fixture_owners[256];
static unsigned fixture_live, fixture_clone_attempts, fixture_clone_failure;
static unsigned fixture_realloc_failure, fixture_refusals, fixture_sort_active;
static unsigned fixture_byte_allocations, fixture_byte_frees, fixture_byte_live, fixture_byte_peak;
static size_t fixture_byte_requested;
static unsigned fixture_find(void *pointer) {
    for(unsigned i=0;i<256;++i)if(fixture_owners[i].pointer==pointer)return i;
    abort();
}
static void fixture_track(void *pointer,size_t size) {
    if(!pointer)return;
    for(unsigned i=0;i<256;++i)if(fixture_owners[i].pointer==pointer)abort();
    for(unsigned i=0;i<256;++i)if(!fixture_owners[i].pointer){
        fixture_owners[i].pointer=pointer;fixture_owners[i].size=size;++fixture_live;return;
    }
    abort();
}
static __attribute__((unused)) void *fixture_malloc(size_t size) {
    if(fixture_sort_active)abort();
    void *pointer=malloc(size);fixture_track(pointer,size);return pointer;
}
static __attribute__((unused)) void *fixture_calloc(size_t count,size_t width) {
    if(fixture_sort_active)abort();
    if(width&&count>SIZE_MAX/width)abort();
    void *pointer=calloc(count,width);fixture_track(pointer,count*width);return pointer;
}
static __attribute__((unused)) void *fixture_zeroed_payload(size_t count,size_t width) {
    void *pointer=fixture_calloc(count,width);
    if(pointer) {
        unsigned slot=fixture_find(pointer);
        fixture_owners[slot].zeroed_payload=1;
        ++fixture_byte_allocations;
        fixture_byte_requested+=fixture_owners[slot].size;
        ++fixture_byte_live;
        if(fixture_byte_live>fixture_byte_peak)fixture_byte_peak=fixture_byte_live;
    }
    return pointer;
}
static __attribute__((unused)) void *fixture_clone_malloc(size_t size) {
    if(++fixture_clone_attempts==fixture_clone_failure){++fixture_refusals;return NULL;}
    return fixture_malloc(size);
}
static __attribute__((unused)) void *fixture_realloc(void *pointer,size_t size) {
    if(fixture_sort_active)abort();
    if(fixture_realloc_failure){++fixture_refusals;return NULL;}
    if(!pointer)return fixture_malloc(size);
    if(!size)abort();
    unsigned slot=fixture_find(pointer);
    if(fixture_owners[slot].zeroed_payload)abort();
    /* Force relocation so stale metadata/pointers cannot hide behind in-place
       realloc success. The source allocation stays live on physical failure. */
    void *next=malloc(size);if(!next)return NULL;
    memcpy(next,pointer,size<fixture_owners[slot].size?size:fixture_owners[slot].size);
    memset(pointer,0xdd,fixture_owners[slot].size);free(pointer);
    fixture_owners[slot].pointer=next;fixture_owners[slot].size=size;return next;
}
static __attribute__((unused)) void fixture_free(void *pointer) {
    if(fixture_sort_active)abort();
    if(!pointer)return;
    unsigned slot=fixture_find(pointer);
    if(fixture_owners[slot].zeroed_payload) {
        fixture_owners[slot].zeroed_payload=0;
        --fixture_byte_live;
        ++fixture_byte_frees;
    }
    memset(pointer,0xdd,fixture_owners[slot].size);free(pointer);
    fixture_owners[slot].pointer=NULL;fixture_owners[slot].size=0;--fixture_live;
}
#define malloc fixture_malloc
#define calloc fixture_calloc
#define realloc fixture_realloc
#define free fixture_free
