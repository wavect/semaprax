#include <stdlib.h>
#include <stddef.h>
#include <assert.h>
static size_t process_live_allocations = 0;
static void *process_malloc(size_t size) { void *p=malloc(size); if(p!=NULL)++process_live_allocations; return p; }
static void *process_calloc(size_t count,size_t size) { void *p=calloc(count,size); if(p!=NULL)++process_live_allocations; return p; }
static void process_free(void *p) { if(p!=NULL){assert(process_live_allocations>0);--process_live_allocations;} free(p); }
#define malloc process_malloc
#define calloc process_calloc
#define free process_free
