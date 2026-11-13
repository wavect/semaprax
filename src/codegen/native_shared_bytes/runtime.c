/* Ordinary shared-owner Bytes v1: the selected internal native profile for
   OWNED-BYTE-PARTITION-V1. Admission is CLOSED; see native_shared_bytes.rs.

   Representation (versioned, internal, never public):
   - An owner is the 24-byte carrier {issuer, slot, nonce}. It holds no pointer
     and no length. Offset and extent come only from the authenticated slot.
   - The issuer is the explicitly held registry's 64-bit identity. It is only
     compared, never dereferenced. The registry is always passed explicitly;
     there is no current-registry global and no header before any payload.
   - slot = (backing index + 1) << 32 | lease index. Lease 0 is the whole-
     backing lease; lease i + 1 is partition child i. Slot 0 is never valid,
     so the all-zero carrier is vacant and carries no authority.
   - Every issuance and every renewal takes a fresh nonce from one strictly
     increasing per-registry counter that refuses to wrap. A lease keeps its
     last nonce after release and is never reissued, so a stale copy cannot
     match (no ABA).
   - Backing records are never reused. Each record is created by exactly one
     charged backing event, so the existing 32-event quota bounds the table.
   - Vec rows keep the existing 16-byte element budget: they store the compact
     {slot, nonce}; the Vec header's issuer authenticates the domain.

   Quotas: backing events (32) and cumulative payload bytes (2 MiB) are the
   existing source-derived limits with no refund. Their independent static
   proof is the authority; reaching either here is a compiler defect and fails
   stop before any write. Partition issues leases and charges no payload.
   Partition child metadata is a separate finite inventory (CHILD_SLOTS per
   registry) that is held before allocation and charged until the complete
   backing lease table is freed. Physical allocator exhaustion stays invariant
   fail-stop except where the existing Vec contract selects `semaprax.vec.v1` code 3.

   Payload and metadata allocations go through distinct hooks so a physical
   census can tell them apart. Public copy-out uses the provider hooks and
   produces a fresh whole allocation; no interior pointer leaves this file. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#ifndef SPX_SHB_FAIL_STOP
#define SPX_SHB_FAIL_STOP(message) spx_runtime_invariant_failure(message)
#endif
#ifndef SPX_SHB_PAYLOAD_CALLOC
#define SPX_SHB_PAYLOAD_CALLOC calloc
#endif
#ifndef SPX_SHB_PAYLOAD_FREE
#define SPX_SHB_PAYLOAD_FREE free
#endif
#ifndef SPX_SHB_METADATA_CALLOC
#define SPX_SHB_METADATA_CALLOC calloc
#endif
#ifndef SPX_SHB_METADATA_FREE
#define SPX_SHB_METADATA_FREE free
#endif
#ifndef SPX_SHB_PROVIDER_MALLOC
#define SPX_SHB_PROVIDER_MALLOC malloc
#endif
#ifndef SPX_SHB_PROVIDER_FREE
#define SPX_SHB_PROVIDER_FREE free
#endif
#define SPX_SHB_FN static __attribute__((unused))
#define SPX_SHB_EVENTS 32u
#define SPX_SHB_CUMULATIVE UINT64_C(2097152)
#define SPX_SHB_EXTENT UINT64_C(131072)
#define SPX_SHB_EXTERNAL_ROOT UINT64_C(65536)
#define SPX_SHB_VEC_CAPACITY UINT64_C(8192)
#define SPX_SHB_VECS 64u
#define SPX_SHB_CHILD_SLOTS UINT64_C(8192)

/* Private outcomes. The bridge maps the first five onto the existing
   byte-buffer and vec domains; the four partition outcomes are mapped by
   the bridge from the shared operation owner's assigned domain and codes. */
enum {
    SPX_SHB_OK = 0,
    SPX_SHB_BUFFER_INDEX,
    SPX_SHB_BUFFER_BOUND,
    SPX_SHB_VEC_FULL,
    SPX_SHB_VEC_INDEX,
    SPX_SHB_VEC_CAPACITY_REFUSED,
    SPX_SHB_PARTITION_WIDTH_ZERO,
    SPX_SHB_PARTITION_EXTENT_OVERFLOW,
    SPX_SHB_PARTITION_EXTENT_MISMATCH,
    SPX_SHB_PARTITION_METADATA_CAPACITY,
    SPX_SHB_PROVIDER_ALLOCATION
};
enum { SPX_SHB_VACANT, SPX_SHB_WHOLE, SPX_SHB_PARTITIONED, SPX_SHB_RELEASED };
enum { SPX_SHB_VEC_FREE, SPX_SHB_VEC_LIVE, SPX_SHB_ITER_LIVE, SPX_SHB_VEC_RESERVED };

static unsigned char spx_shb_module_identity;
/* Every held issuer uses one module-local monotonic namespace. It is never
   reset by issuer initialization and grants no current-registry authority. */
static _Atomic(uint64_t) spx_shb_next_issuer = UINT64_C(1);
typedef struct { uint64_t issuer, slot, nonce; } spx_shb_bytes;
typedef struct { uint64_t slot, nonce; } spx_shb_stored;
typedef struct { uint64_t issuer, slot, nonce; } spx_shb_loan;
typedef struct { uint64_t nonce; uint32_t live, loans; } spx_shb_lease;
typedef struct {
    unsigned char *payload; spx_shb_lease *child;
    uint64_t extent, width;
    spx_shb_lease whole;
    uint32_t state, count, live, partition_authority;
} spx_shb_backing;
typedef struct {
    spx_shb_stored *rows; spx_shb_lease *table;
    uint64_t source_nonce, ticket;
    uint32_t width; uint16_t count, backing_index;
} spx_shb_partition_reservation;
typedef struct {
    union {
        struct { spx_shb_stored *rows; uint64_t len, capacity, generation, end; };
        spx_shb_partition_reservation partition;
    };
    uint32_t state;
} spx_shb_vec_entry;
typedef struct {
    spx_shb_stored *rows; uint64_t len, capacity, generation, issuer;
    uint32_t authority, reserved;
} spx_shb_vec;
typedef struct { spx_shb_vec vec; uint64_t cursor; } spx_shb_iter;
/* Caller-held and never reset: registry identities strictly increase. */
typedef struct { const void *module; uint64_t next; uint32_t sealed, reserved; } spx_shb_issuer;
typedef struct spx_shb_registry {
    const void *module; uint64_t issuer, next_nonce, next_generation, cumulative;
    uint64_t child_table_slots, child_held, next_partition_ticket;
    uint32_t sealed, finished, events, loans;
    spx_shb_backing backing[SPX_SHB_EVENTS];
    spx_shb_vec_entry vec[SPX_SHB_VECS];
} spx_shb_registry;
/* The plan only identifies an unpublished reservation. The registry owns
   its allocation pointers, exact source lease and geometry. */
typedef struct {
    uint64_t issuer, ticket;
    uint32_t authority, armed;
} spx_shb_partition_plan;
_Static_assert(SPX_SHB_EXTENT <= UINT32_MAX, "partition reservation stores the maximum child width");
_Static_assert(SPX_SHB_VEC_CAPACITY <= UINT16_MAX, "partition reservation stores the maximum child count");
_Static_assert(SPX_SHB_EVENTS <= UINT16_MAX, "partition reservation stores the backing index");
#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(spx_shb_bytes) == 24, "native64 shared owner carrier");
_Static_assert(sizeof(spx_shb_stored) == 16, "native64 stored owner keeps the Vec element budget");
_Static_assert(sizeof(spx_shb_lease) == 16, "native64 lease record");
_Static_assert(sizeof(spx_shb_backing) == 64, "native64 backing record");
_Static_assert(sizeof(spx_shb_vec_entry) == 48, "native64 Vec authority record");
_Static_assert(sizeof(spx_shb_vec) == 48, "native64 Vec header");
_Static_assert(sizeof(spx_shb_iter) == 56, "native64 iterator header");
_Static_assert(sizeof(spx_shb_registry) == 5200, "native64 held registry");
#endif

SPX_SHB_FN void spx_shb_issuer_init(spx_shb_issuer *issuer) {
    if (issuer == NULL || issuer->sealed) SPX_SHB_FAIL_STOP("shared Bytes issuer is not fresh");
    issuer->module = &spx_shb_module_identity; issuer->next = UINT64_C(1); issuer->sealed = 1u;
}
SPX_SHB_FN void spx_shb_registry_init(spx_shb_registry *registry, spx_shb_issuer *issuer) {
    if (registry == NULL || registry->sealed || registry->module != NULL)
        SPX_SHB_FAIL_STOP("shared Bytes registry is not canonically zero");
    if (issuer == NULL || !issuer->sealed || issuer->module != &spx_shb_module_identity
        || issuer->next == UINT64_C(0) || issuer->next == UINT64_MAX)
        SPX_SHB_FAIL_STOP("shared Bytes issuer is not held by this module");
    uint64_t next = atomic_load_explicit(&spx_shb_next_issuer, memory_order_relaxed);
    for (;;) {
        if (next == UINT64_C(0) || next == UINT64_MAX)
            SPX_SHB_FAIL_STOP("shared Bytes issuer namespace exhausted");
        if (atomic_compare_exchange_weak_explicit(
                &spx_shb_next_issuer, &next, next + UINT64_C(1),
                memory_order_relaxed, memory_order_relaxed)) break;
    }
    registry->module = &spx_shb_module_identity; registry->issuer = next;
    registry->next_nonce = UINT64_C(1); registry->next_generation = UINT64_C(1);
    registry->next_partition_ticket = UINT64_C(1);
    registry->sealed = 1u;
}
SPX_SHB_FN void spx_shb_require(const spx_shb_registry *registry) {
    if (registry == NULL || !registry->sealed || registry->finished
        || registry->module != &spx_shb_module_identity || registry->issuer == UINT64_C(0))
        SPX_SHB_FAIL_STOP("shared Bytes registry is not the held live registry");
}
SPX_SHB_FN uint64_t spx_shb_fresh_nonce(spx_shb_registry *registry) {
    if (registry->next_nonce == UINT64_C(0) || registry->next_nonce == UINT64_MAX)
        SPX_SHB_FAIL_STOP("shared Bytes lease generation exhausted");
    return registry->next_nonce++;
}
SPX_SHB_FN uint64_t spx_shb_fresh_generation(spx_shb_registry *registry) {
    if (registry->next_generation == UINT64_C(0) || registry->next_generation == UINT64_MAX)
        SPX_SHB_FAIL_STOP("shared Bytes Vec generation exhausted");
    return registry->next_generation++;
}
SPX_SHB_FN bool spx_shb_is_vacant(const spx_shb_bytes *value) {
    return value->issuer == UINT64_C(0) && value->slot == UINT64_C(0) && value->nonce == UINT64_C(0);
}

/* O(1): issuer, backing index, lease index and nonce are rejected before any
   backing address is formed. Nothing reachable from the carrier is read. */
typedef struct {
    spx_shb_backing *backing; spx_shb_lease *lease;
    uint64_t index, lease_index, offset, len;
} spx_shb_resolved;
SPX_SHB_FN spx_shb_resolved spx_shb_resolve(spx_shb_registry *registry, const spx_shb_bytes *value) {
    spx_shb_require(registry);
    if (value == NULL) SPX_SHB_FAIL_STOP("shared Bytes owner has a null carrier");
    if (value->issuer != registry->issuer) SPX_SHB_FAIL_STOP("foreign or vacant shared Bytes issuer");
    uint64_t index = value->slot >> 32, lease_index = value->slot & UINT64_C(0xffffffff);
    if (index == UINT64_C(0) || index > (uint64_t)registry->events)
        SPX_SHB_FAIL_STOP("unknown shared Bytes backing slot");
    spx_shb_resolved r = { .backing = &registry->backing[index - 1u], .index = index - 1u,
        .lease_index = lease_index };
    if (r.backing->state == SPX_SHB_WHOLE && lease_index == UINT64_C(0)) {
        r.lease = &r.backing->whole; r.offset = UINT64_C(0); r.len = r.backing->extent;
    } else if (r.backing->state == SPX_SHB_PARTITIONED && lease_index != UINT64_C(0)
        && lease_index <= (uint64_t)r.backing->count && r.backing->child != NULL) {
        r.lease = &r.backing->child[lease_index - 1u];
        r.offset = (lease_index - 1u) * r.backing->width; r.len = r.backing->width;
    } else {
        SPX_SHB_FAIL_STOP("released, forged or overlapping shared Bytes lease");
    }
    if (!r.lease->live || r.lease->nonce != value->nonce)
        SPX_SHB_FAIL_STOP("stale shared Bytes lease generation");
    return r;
}
SPX_SHB_FN spx_shb_resolved spx_shb_resolve_unloaned(spx_shb_registry *registry, const spx_shb_bytes *value) {
    spx_shb_resolved r = spx_shb_resolve(registry, value);
    if (r.lease->loans != 0u) SPX_SHB_FAIL_STOP("shared Bytes owner has an outstanding borrow");
    return r;
}
SPX_SHB_FN void spx_shb_release_backing(spx_shb_registry *registry, spx_shb_backing *backing) {
    if (backing->state != SPX_SHB_WHOLE && backing->state != SPX_SHB_PARTITIONED)
        SPX_SHB_FAIL_STOP("shared Bytes backing released twice");
    if (backing->live != 0u) SPX_SHB_FAIL_STOP("shared Bytes backing released with live leases");
    bool partitioned = backing->state == SPX_SHB_PARTITIONED;
    if (partitioned && (backing->child == NULL || backing->count == 0u
        || backing->count > registry->child_table_slots))
        SPX_SHB_FAIL_STOP("shared Bytes backing table accounting invariant");
    backing->state = SPX_SHB_RELEASED;
    unsigned char *payload = backing->payload; spx_shb_lease *child = backing->child;
    backing->payload = NULL; backing->child = NULL;
    if (payload != NULL) SPX_SHB_PAYLOAD_FREE(payload);
    if (partitioned) {
        SPX_SHB_METADATA_FREE(child);
        registry->child_table_slots -= backing->count;
    }
}

/* Producers. The event and payload charge precedes the allocation it covers. */
SPX_SHB_FN void spx_shb_produce(
    spx_shb_registry *registry, uint64_t len, const unsigned char *source, spx_shb_bytes *out
) {
    spx_shb_require(registry);
    if (out == NULL) SPX_SHB_FAIL_STOP("shared Bytes producer has no result carrier");
    *out = (spx_shb_bytes){0};
    if (len > SPX_SHB_EXTENT) SPX_SHB_FAIL_STOP("owned bytes exceed the exact length bound");
    if (registry->events >= SPX_SHB_EVENTS || registry->cumulative > SPX_SHB_CUMULATIVE
        || len > SPX_SHB_CUMULATIVE - registry->cumulative)
        SPX_SHB_FAIL_STOP("shared Bytes backing quota exceeded its static admission");
    uint64_t nonce = spx_shb_fresh_nonce(registry);
    uint32_t index = registry->events++;
    registry->cumulative += len;
    spx_shb_backing *backing = &registry->backing[index];
    if (backing->state != SPX_SHB_VACANT) SPX_SHB_FAIL_STOP("shared Bytes backing record reused");
    unsigned char *payload = NULL;
    if (len != UINT64_C(0)) {
        payload = (unsigned char *)SPX_SHB_PAYLOAD_CALLOC((size_t)len, sizeof(unsigned char));
        if (payload == NULL) SPX_SHB_FAIL_STOP("owned byte allocation failed");
        if (source != NULL) memcpy(payload, source, (size_t)len);
    }
    *backing = (spx_shb_backing){ .payload = payload, .extent = len,
        .whole = { .nonce = nonce, .live = 1u }, .state = SPX_SHB_WHOLE, .live = 1u };
    *out = (spx_shb_bytes){ .issuer = registry->issuer,
        .slot = ((uint64_t)index + 1u) << 32, .nonce = nonce };
}
SPX_SHB_FN void spx_shb_zeroed(spx_shb_registry *registry, uint64_t count, spx_shb_bytes *out) {
    spx_shb_produce(registry, count, NULL, out);
}
SPX_SHB_FN uint32_t spx_shb_bounded_check(uint64_t requested, uint64_t bound) {
    if (bound > SPX_SHB_EXTENT) SPX_SHB_FAIL_STOP("forged bounded byte allocation extent");
    return requested <= bound ? SPX_SHB_OK : SPX_SHB_BUFFER_BOUND;
}
SPX_SHB_FN void spx_shb_require_view(spx_slice_u8_v1 value, uint64_t bound) {
    if (value.len > bound) SPX_SHB_FAIL_STOP("byte view exceeds the exact length bound");
    if ((value.len == UINT64_C(0)) != (value.ptr == NULL))
        SPX_SHB_FAIL_STOP("byte view is not normalized");
}
SPX_SHB_FN void spx_shb_copy(spx_shb_registry *registry, spx_slice_u8_v1 value, spx_shb_bytes *out) {
    spx_shb_require_view(value, SPX_SHB_EXTENT);
    spx_shb_produce(registry, value.len, value.ptr, out);
}
/* Public input is snapshotted into fresh charged selected backing. */
SPX_SHB_FN void spx_shb_snapshot_in(spx_shb_registry *registry, spx_slice_u8_v1 value, spx_shb_bytes *out) {
    spx_shb_require_view(value, SPX_SHB_EXTERNAL_ROOT);
    spx_shb_produce(registry, value.len, value.ptr, out);
}

/* Borrowed views. A view is the ordinary non-owning slice over the exact
   authenticated range; it is never dropped and never reaches any free. */
SPX_SHB_FN spx_slice_u8_v1 spx_shb_view(spx_shb_registry *registry, const spx_shb_bytes *value) {
    spx_shb_resolved r = spx_shb_resolve(registry, value);
    if (r.len == UINT64_C(0)) return (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
    return (spx_slice_u8_v1){ .ptr = r.backing->payload + (size_t)r.offset, .len = r.len };
}
SPX_SHB_FN uint64_t spx_shb_len(spx_shb_registry *registry, const spx_shb_bytes *value) {
    return spx_shb_resolve(registry, value).len;
}
/* Explicit loans bind the current generation. While one is live the owner
   cannot be dropped, moved, stored into, transferred or partitioned. */
SPX_SHB_FN spx_shb_loan spx_shb_loan_begin(spx_shb_registry *registry, const spx_shb_bytes *value) {
    spx_shb_resolved r = spx_shb_resolve(registry, value);
    if (r.lease->loans == UINT32_MAX || registry->loans == UINT32_MAX) SPX_SHB_FAIL_STOP("shared Bytes loan count exhausted");
    r.lease->loans++; registry->loans++;
    return (spx_shb_loan){ .issuer = value->issuer, .slot = value->slot, .nonce = value->nonce };
}
SPX_SHB_FN void spx_shb_loan_end(spx_shb_registry *registry, spx_shb_loan *loan) {
    if (loan == NULL) SPX_SHB_FAIL_STOP("shared Bytes loan has a null carrier");
    spx_shb_bytes bound = { .issuer = loan->issuer, .slot = loan->slot, .nonce = loan->nonce };
    spx_shb_resolved r = spx_shb_resolve(registry, &bound);
    if (r.lease->loans == 0u || registry->loans == 0u) SPX_SHB_FAIL_STOP("shared Bytes loan ended twice");
    r.lease->loans--; registry->loans--; *loan = (spx_shb_loan){0};
}

SPX_SHB_FN spx_shb_bytes spx_shb_move(spx_shb_registry *registry, spx_shb_bytes *source) {
    (void)spx_shb_resolve_unloaned(registry, source);
    spx_shb_bytes moved = *source; *source = (spx_shb_bytes){0}; return moved;
}
/* A vacant carrier is a moved-from slot and settles nothing. Every other
   carrier must authenticate; the last live lease releases the backing once. */
SPX_SHB_FN void spx_shb_drop(spx_shb_registry *registry, spx_shb_bytes *value) {
    if (value == NULL) SPX_SHB_FAIL_STOP("shared Bytes drop has a null carrier");
    if (spx_shb_is_vacant(value)) { spx_shb_require(registry); return; }
    spx_shb_resolved r = spx_shb_resolve_unloaned(registry, value);
    *value = (spx_shb_bytes){0};
    r.lease->live = 0u;
    if (r.backing->live == 0u) SPX_SHB_FAIL_STOP("shared Bytes backing live count underflow");
    if (--r.backing->live == 0u) spx_shb_release_backing(registry, r.backing);
}
/* Deep copy into fresh ordinary backing with the ordinary charges. */
SPX_SHB_FN void spx_shb_clone(spx_shb_registry *registry, const spx_shb_bytes *source, spx_shb_bytes *out) {
    spx_slice_u8_v1 view = spx_shb_view(registry, source);
    spx_shb_produce(registry, view.len, view.ptr, out);
}

/* Stores: check selects the existing byte-buffer failure before transfer;
   the commit renews the same exact range under a fresh nonce. */
SPX_SHB_FN uint32_t spx_shb_store_check(
    spx_shb_registry *registry, const spx_shb_bytes *buffer, uint64_t index, uint64_t width
) {
    spx_shb_resolved r = spx_shb_resolve(registry, buffer);
    return index <= r.len && r.len - index >= width ? SPX_SHB_OK : SPX_SHB_BUFFER_INDEX;
}
SPX_SHB_FN unsigned char *spx_shb_store_window(
    spx_shb_registry *registry, spx_shb_bytes *buffer, uint64_t index, uint64_t width,
    spx_shb_bytes *out
) {
    if (out == NULL || out == buffer) SPX_SHB_FAIL_STOP("shared Bytes store has no distinct result");
    spx_shb_resolved r = spx_shb_resolve_unloaned(registry, buffer);
    if (index > r.len || r.len - index < width)
        SPX_SHB_FAIL_STOP("owned byte buffer store interval is outside its capacity");
    r.lease->nonce = spx_shb_fresh_nonce(registry);
    *out = (spx_shb_bytes){ .issuer = buffer->issuer, .slot = buffer->slot, .nonce = r.lease->nonce };
    *buffer = (spx_shb_bytes){0};
    return r.backing->payload + (size_t)(r.offset + index);
}
SPX_SHB_FN uint32_t spx_shb_set_check(spx_shb_registry *registry, const spx_shb_bytes *buffer, uint64_t index) {
    return spx_shb_store_check(registry, buffer, index, UINT64_C(1));
}
SPX_SHB_FN uint32_t spx_shb_set5_check(spx_shb_registry *registry, const spx_shb_bytes *buffer, uint64_t index) {
    return spx_shb_store_check(registry, buffer, index, UINT64_C(5));
}
SPX_SHB_FN uint32_t spx_shb_set1_or5_check(
    spx_shb_registry *registry, const spx_shb_bytes *buffer, uint64_t selector, uint64_t index
) {
    return spx_shb_store_check(registry, buffer, index,
        (selector & (UINT64_C(1) << 63)) != 0 ? UINT64_C(5) : UINT64_C(1));
}
SPX_SHB_FN uint32_t spx_shb_set1_or6_or48_check(
    spx_shb_registry *registry, const spx_shb_bytes *buffer, uint64_t selector, uint64_t index
) {
    uint64_t width = (selector & (UINT64_C(1) << 63)) == 0 ? UINT64_C(1)
        : ((selector & (UINT64_C(1) << 62)) != 0 ? UINT64_C(48) : UINT64_C(6));
    return spx_shb_store_check(registry, buffer, index, width);
}
SPX_SHB_FN void spx_shb_set(
    spx_shb_registry *registry, spx_shb_bytes *buffer, uint64_t index, uint8_t value, spx_shb_bytes *out
) {
    *spx_shb_store_window(registry, buffer, index, UINT64_C(1), out) = value;
}
SPX_SHB_FN void spx_shb_set5(
    spx_shb_registry *registry, spx_shb_bytes *buffer, uint64_t index, uint8_t first,
    uint8_t second, uint8_t third, uint8_t fourth, uint8_t fifth, spx_shb_bytes *out
) {
    unsigned char *w = spx_shb_store_window(registry, buffer, index, UINT64_C(5), out);
    w[0] = first; w[1] = second; w[2] = third; w[3] = fourth; w[4] = fifth;
}
SPX_SHB_FN void spx_shb_store_from(
    unsigned char *window, uint64_t width, spx_slice_u8_v1 source, uint64_t start
) {
    for (uint64_t offset = UINT64_C(0); offset < width; ++offset) {
        uint64_t at = start + offset;
        window[offset] = at >= start && at < source.len ? source.ptr[(size_t)at] : UINT8_C(0);
    }
}
SPX_SHB_FN void spx_shb_set1_or5(
    spx_shb_registry *registry, spx_shb_bytes *buffer, uint64_t index, uint8_t one,
    spx_slice_u8_v1 source, uint64_t selector, spx_shb_bytes *out
) {
    spx_shb_require_view(source, SPX_SHB_EXTENT);
    bool wide = (selector & (UINT64_C(1) << 63)) != 0;
    unsigned char *w = spx_shb_store_window(registry, buffer, index, wide ? UINT64_C(5) : UINT64_C(1), out);
    if (!wide) { *w = one; return; }
    spx_shb_store_from(w, UINT64_C(5), source, selector & ~(UINT64_C(1) << 63));
}
SPX_SHB_FN void spx_shb_set1_or6_or48(
    spx_shb_registry *registry, spx_shb_bytes *buffer, uint64_t index, uint8_t one,
    spx_slice_u8_v1 source, uint64_t selector, spx_shb_bytes *out
) {
    spx_shb_require_view(source, SPX_SHB_EXTENT);
    bool copy = (selector & (UINT64_C(1) << 63)) != 0;
    uint64_t width = !copy ? UINT64_C(1)
        : ((selector & (UINT64_C(1) << 62)) != 0 ? UINT64_C(48) : UINT64_C(6));
    unsigned char *w = spx_shb_store_window(registry, buffer, index, width, out);
    if (!copy) { *w = one; return; }
    spx_shb_store_from(w, width, source, selector & ((UINT64_C(1) << 62) - UINT64_C(1)));
}

/* Vec<Bytes>: the registry-held authority binds rows, len, capacity and
   generation. A copied header is stale after any committed operation. */
SPX_SHB_FN spx_shb_vec_entry *spx_shb_vec_require(
    spx_shb_registry *registry, const spx_shb_vec *value, uint32_t state, uint64_t len
) {
    spx_shb_require(registry);
    if (value == NULL || value->issuer != registry->issuer || value->authority == 0u
        || value->authority > SPX_SHB_VECS || value->capacity > SPX_SHB_VEC_CAPACITY
        || value->len > value->capacity || (value->capacity == UINT64_C(0)) != (value->rows == NULL))
        SPX_SHB_FAIL_STOP("invalid shared Bytes Vec carrier");
    spx_shb_vec_entry *e = &registry->vec[value->authority - 1u];
    if (e->state != state || e->rows != value->rows || e->len != len || e->capacity != value->capacity
        || e->generation != value->generation || e->generation == UINT64_C(0))
        SPX_SHB_FAIL_STOP("stale or forged shared Bytes Vec carrier");
    return e;
}
SPX_SHB_FN spx_shb_vec_entry *spx_shb_vec_valid(spx_shb_registry *registry, const spx_shb_vec *value) {
    return spx_shb_vec_require(registry, value, SPX_SHB_VEC_LIVE, value == NULL ? UINT64_C(0) : value->len);
}
SPX_SHB_FN uint32_t spx_shb_vec_reserve_authority(spx_shb_registry *registry) {
    for (uint32_t i = 0u; i < SPX_SHB_VECS; ++i)
        if (registry->vec[i].state == SPX_SHB_VEC_FREE) {
            registry->vec[i] = (spx_shb_vec_entry){ .state = SPX_SHB_VEC_RESERVED };
            return i + 1u;
        }
    return 0u;
}
SPX_SHB_FN void spx_shb_vec_publish(
    spx_shb_registry *registry, spx_shb_vec *source, spx_shb_vec_entry *e, spx_shb_vec *out
) {
    if (out == NULL || out == source) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec result");
    source->generation = spx_shb_fresh_generation(registry);
    *e = (spx_shb_vec_entry){ .rows = source->rows, .len = source->len,
        .capacity = source->capacity, .generation = source->generation, .state = SPX_SHB_VEC_LIVE };
    *out = *source; *source = (spx_shb_vec){0};
}
SPX_SHB_FN spx_shb_bytes spx_shb_row_owner(const spx_shb_vec *value, uint64_t index) {
    return (spx_shb_bytes){ .issuer = value->issuer, .slot = value->rows[index].slot,
        .nonce = value->rows[index].nonce };
}
SPX_SHB_FN spx_shb_stored spx_shb_take_owner(spx_shb_registry *registry, spx_shb_bytes *value) {
    spx_shb_bytes moved = spx_shb_move(registry, value);
    return (spx_shb_stored){ .slot = moved.slot, .nonce = moved.nonce };
}
/* Every row below len is a live owner; a vacant one is a runtime defect. */
SPX_SHB_FN void spx_shb_drop_row(spx_shb_registry *registry, const spx_shb_vec *value, uint64_t index) {
    if (value->rows[index].slot == UINT64_C(0))
        SPX_SHB_FAIL_STOP("shared Bytes Vec row below len is vacant");
    spx_shb_bytes owner = spx_shb_row_owner(value, index);
    (void)spx_shb_resolve_unloaned(registry, &owner);
    value->rows[index] = (spx_shb_stored){0};
    spx_shb_drop(registry, &owner);
}
/* Refuse live row loans before the first canonical finalizer. Individual
   finalizers still authenticate their exact row before changing it. */
SPX_SHB_FN void spx_shb_preflight_rows(
    spx_shb_registry *registry, const spx_shb_vec *value, uint64_t start
) {
    spx_shb_require(registry);
    /* No live loans is the common iterator path: never rescan its suffix. */
    if (registry->loans == 0u) return;
    for (uint64_t i = start; i < value->len; ++i) {
        spx_shb_bytes owner = spx_shb_row_owner(value, i);
        (void)spx_shb_resolve_unloaned(registry, &owner);
    }
}
SPX_SHB_FN uint32_t spx_shb_vec_with_capacity(spx_shb_registry *registry, uint64_t capacity, spx_shb_vec *out) {
    spx_shb_require(registry);
    if (out == NULL) SPX_SHB_FAIL_STOP("shared Bytes Vec has no result carrier");
    *out = (spx_shb_vec){0};
    if (capacity > SPX_SHB_VEC_CAPACITY) return SPX_SHB_VEC_CAPACITY_REFUSED;
    uint32_t authority = spx_shb_vec_reserve_authority(registry);
    if (authority == 0u) return SPX_SHB_VEC_CAPACITY_REFUSED;
    spx_shb_stored *rows = capacity == UINT64_C(0) ? NULL
        : (spx_shb_stored *)SPX_SHB_METADATA_CALLOC((size_t)capacity, sizeof(spx_shb_stored));
    if (capacity != UINT64_C(0) && rows == NULL) {
        registry->vec[authority - 1u] = (spx_shb_vec_entry){0};
        return SPX_SHB_VEC_CAPACITY_REFUSED;
    }
    spx_shb_vec staged = { .rows = rows, .capacity = capacity, .issuer = registry->issuer,
        .authority = authority };
    spx_shb_vec_publish(registry, &staged, &registry->vec[authority - 1u], out);
    return SPX_SHB_OK;
}
/* A refused push leaves the staged owner with the caller, as legacy does. */
SPX_SHB_FN uint32_t spx_shb_vec_push(
    spx_shb_registry *registry, spx_shb_vec *source, spx_shb_bytes *value, spx_shb_vec *out
) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, source);
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    (void)spx_shb_resolve_unloaned(registry, value);
    if (out == NULL || out == source) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec result");
    if (source->len == source->capacity) return SPX_SHB_VEC_FULL;
    source->rows[source->len] = spx_shb_take_owner(registry, value); source->len++;
    spx_shb_vec_publish(registry, source, e, out); return SPX_SHB_OK;
}
/* Pop transfers the original lease; nothing is renewed or allocated. */
SPX_SHB_FN void spx_shb_vec_pop(
    spx_shb_registry *registry, spx_shb_vec *source, uint32_t *tag, spx_shb_bytes *item, spx_shb_vec *out
) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, source);
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    if (tag == NULL || item == NULL || out == NULL || out == source)
        SPX_SHB_FAIL_STOP("invalid shared Bytes Vec pop output");
    *tag = 0u; *item = (spx_shb_bytes){0};
    if (source->len != UINT64_C(0)) {
        uint64_t last = source->len - 1u;
        spx_shb_bytes owner = spx_shb_row_owner(source, last);
        *item = spx_shb_move(registry, &owner);
        source->rows[last] = (spx_shb_stored){0}; source->len = last; *tag = 1u;
    }
    spx_shb_vec_publish(registry, source, e, out);
}
SPX_SHB_FN uint32_t spx_shb_vec_view(
    spx_shb_registry *registry, const spx_shb_vec *source, uint64_t index, spx_slice_u8_v1 *out
) {
    (void)spx_shb_vec_valid(registry, source);
    if (out == NULL) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec view output");
    *out = (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
    if (index >= source->len) return SPX_SHB_VEC_INDEX;
    spx_shb_bytes owner = spx_shb_row_owner(source, index);
    *out = spx_shb_view(registry, &owner); return SPX_SHB_OK;
}
SPX_SHB_FN uint32_t spx_shb_vec_set(
    spx_shb_registry *registry, spx_shb_vec *source, uint64_t index, spx_shb_bytes *value, spx_shb_vec *out
) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, source);
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    (void)spx_shb_resolve_unloaned(registry, value);
    if (out == NULL || out == source) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec result");
    if (index >= source->len) return SPX_SHB_VEC_INDEX;
    if (source->rows[index].slot == value->slot)
        SPX_SHB_FAIL_STOP("shared Bytes Vec set value aliases its replaced row");
    spx_shb_drop_row(registry, source, index);
    source->rows[index] = spx_shb_take_owner(registry, value);
    spx_shb_vec_publish(registry, source, e, out); return SPX_SHB_OK;
}
/* Canonical ascending element order, as the legacy runtime settles rows. */
SPX_SHB_FN void spx_shb_vec_clear(spx_shb_registry *registry, spx_shb_vec *source, spx_shb_vec *out) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, source);
    if (out == NULL || out == source) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec result");
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    for (uint64_t i = UINT64_C(0); i < source->len; ++i) spx_shb_drop_row(registry, source, i);
    source->len = UINT64_C(0);
    spx_shb_vec_publish(registry, source, e, out);
}
SPX_SHB_FN uint32_t spx_shb_vec_reserve_exact(
    spx_shb_registry *registry, spx_shb_vec *source, uint64_t additional, spx_shb_vec *out
) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, source);
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    if (out == NULL || out == source) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec result");
    if (additional > UINT64_MAX - source->len) return SPX_SHB_VEC_CAPACITY_REFUSED;
    uint64_t required = source->len + additional;
    uint64_t target = source->capacity > required ? source->capacity : required;
    if (target > SPX_SHB_VEC_CAPACITY) return SPX_SHB_VEC_CAPACITY_REFUSED;
    if (target != source->capacity) {
        spx_shb_stored *rows = (spx_shb_stored *)SPX_SHB_METADATA_CALLOC((size_t)target, sizeof(spx_shb_stored));
        if (rows == NULL) return SPX_SHB_VEC_CAPACITY_REFUSED;
        if (source->len != UINT64_C(0)) memcpy(rows, source->rows, (size_t)source->len * sizeof(spx_shb_stored));
        if (source->rows != NULL) SPX_SHB_METADATA_FREE(source->rows);
        source->rows = rows; source->capacity = target;
    }
    spx_shb_vec_publish(registry, source, e, out); return SPX_SHB_OK;
}
SPX_SHB_FN uint32_t spx_shb_vec_clone_at(
    spx_shb_registry *registry, const spx_shb_vec *source, uint64_t index, spx_shb_bytes *out
) {
    (void)spx_shb_vec_valid(registry, source);
    if (out == NULL) SPX_SHB_FAIL_STOP("invalid shared Bytes Vec clone output");
    *out = (spx_shb_bytes){0};
    if (index >= source->len) return SPX_SHB_VEC_INDEX;
    spx_shb_bytes owner = spx_shb_row_owner(source, index);
    spx_shb_clone(registry, &owner, out); return SPX_SHB_OK;
}
SPX_SHB_FN uint64_t spx_shb_vec_len(spx_shb_registry *registry, const spx_shb_vec *value) {
    (void)spx_shb_vec_valid(registry, value); return value->len;
}
SPX_SHB_FN uint64_t spx_shb_vec_capacity(spx_shb_registry *registry, const spx_shb_vec *value) {
    (void)spx_shb_vec_valid(registry, value); return value->capacity;
}
SPX_SHB_FN spx_shb_vec spx_shb_vec_move(spx_shb_registry *registry, spx_shb_vec *source) {
    (void)spx_shb_vec_valid(registry, source);
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    spx_shb_vec moved = *source; *source = (spx_shb_vec){0}; return moved;
}
SPX_SHB_FN void spx_shb_vec_drop(spx_shb_registry *registry, spx_shb_vec *value) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, value);
    spx_shb_preflight_rows(registry, value, UINT64_C(0));
    for (uint64_t i = UINT64_C(0); i < value->len; ++i) spx_shb_drop_row(registry, value, i);
    spx_shb_stored *rows = value->rows;
    *e = (spx_shb_vec_entry){0}; *value = (spx_shb_vec){0};
    if (rows != NULL) SPX_SHB_METADATA_FREE(rows);
}

/* Consuming iteration: the authority changes kind at Vec -> Iter, its len is
   the cursor and end is the fixed element count. */
SPX_SHB_FN spx_shb_vec_entry *spx_shb_iter_valid(spx_shb_registry *registry, const spx_shb_iter *value) {
    if (value == NULL || value->cursor > value->vec.len) SPX_SHB_FAIL_STOP("invalid shared Bytes iterator");
    spx_shb_vec_entry *e = spx_shb_vec_require(registry, &value->vec, SPX_SHB_ITER_LIVE, value->cursor);
    if (e->end != value->vec.len) SPX_SHB_FAIL_STOP("stale shared Bytes iterator window");
    return e;
}
SPX_SHB_FN spx_shb_iter spx_shb_iter_from_vec(spx_shb_registry *registry, spx_shb_vec *source) {
    spx_shb_vec_entry *e = spx_shb_vec_valid(registry, source);
    spx_shb_preflight_rows(registry, source, UINT64_C(0));
    spx_shb_iter r = { .vec = *source, .cursor = UINT64_C(0) };
    r.vec.generation = spx_shb_fresh_generation(registry);
    e->state = SPX_SHB_ITER_LIVE; e->end = source->len; e->len = UINT64_C(0); e->generation = r.vec.generation;
    *source = (spx_shb_vec){0}; return r;
}
SPX_SHB_FN void spx_shb_iter_drop(spx_shb_registry *registry, spx_shb_iter *value) {
    spx_shb_vec_entry *e = spx_shb_iter_valid(registry, value);
    spx_shb_preflight_rows(registry, &value->vec, value->cursor);
    for (uint64_t i = value->cursor; i < value->vec.len; ++i) spx_shb_drop_row(registry, &value->vec, i);
    spx_shb_stored *rows = value->vec.rows;
    *e = (spx_shb_vec_entry){0}; *value = (spx_shb_iter){0};
    if (rows != NULL) SPX_SHB_METADATA_FREE(rows);
}
SPX_SHB_FN void spx_shb_iter_next(
    spx_shb_registry *registry, spx_shb_iter *source, uint32_t *tag, spx_shb_bytes *item, spx_shb_iter *rest
) {
    spx_shb_vec_entry *e = spx_shb_iter_valid(registry, source);
    spx_shb_preflight_rows(registry, &source->vec, source->cursor);
    if (tag == NULL || item == NULL || rest == NULL || rest == source)
        SPX_SHB_FAIL_STOP("invalid shared Bytes iterator output");
    *tag = 0u; *item = (spx_shb_bytes){0}; *rest = (spx_shb_iter){0};
    if (source->cursor == source->vec.len) { spx_shb_iter_drop(registry, source); return; }
    spx_shb_bytes owner = spx_shb_row_owner(&source->vec, source->cursor);
    *item = spx_shb_move(registry, &owner);
    source->vec.rows[source->cursor] = (spx_shb_stored){0};
    *rest = *source; rest->cursor++;
    rest->vec.generation = spx_shb_fresh_generation(registry);
    e->len = rest->cursor; e->generation = rest->vec.generation;
    *tag = 1u; *source = (spx_shb_iter){0};
}

/* Partition. Prepare validates in the specified order, then holds and
   allocates the complete unpublished inventory; the source stays a caller-
   owned staged value. Abort releases only that unpublished metadata. Commit
   re-authenticates the reservation and source before it invalidates the whole
   lease and publishes every child at once. Child leases and offset-zero or whole-extent
   children are never partition inputs. */
SPX_SHB_FN uint32_t spx_shb_partition_prepare(
    spx_shb_registry *registry, const spx_shb_bytes *source, uint64_t count, uint64_t width,
    spx_shb_partition_plan *plan
) {
    if (plan == NULL || plan->armed) SPX_SHB_FAIL_STOP("shared Bytes partition plan is not fresh");
    spx_shb_resolved r = spx_shb_resolve_unloaned(registry, source);
    if (r.backing->state != SPX_SHB_WHOLE || r.lease_index != UINT64_C(0))
        SPX_SHB_FAIL_STOP("partition input is not an ordinary whole-backing lease");
    if (r.backing->partition_authority != 0u)
        SPX_SHB_FAIL_STOP("shared Bytes backing already has a partition reservation");
    if (width == UINT64_C(0)) return SPX_SHB_PARTITION_WIDTH_ZERO;
    if (count != UINT64_C(0) && width > UINT64_MAX / count) return SPX_SHB_PARTITION_EXTENT_OVERFLOW;
    if (count * width != r.len) return SPX_SHB_PARTITION_EXTENT_MISMATCH;
    if (count > SPX_SHB_VEC_CAPACITY || registry->child_table_slots > SPX_SHB_CHILD_SLOTS
        || registry->child_held > SPX_SHB_CHILD_SLOTS - registry->child_table_slots
        || count > SPX_SHB_CHILD_SLOTS - registry->child_table_slots - registry->child_held)
        return SPX_SHB_PARTITION_METADATA_CAPACITY;
    uint32_t authority = spx_shb_vec_reserve_authority(registry);
    if (authority == 0u) return SPX_SHB_PARTITION_METADATA_CAPACITY;
    if (registry->next_partition_ticket == UINT64_C(0) || registry->next_partition_ticket == UINT64_MAX) {
        registry->vec[authority - 1u] = (spx_shb_vec_entry){0};
        SPX_SHB_FAIL_STOP("shared Bytes partition ticket exhausted");
    }
    r.backing->partition_authority = authority;
    registry->child_held += count;
    spx_shb_stored *rows = NULL; spx_shb_lease *table = NULL;
    if (count != UINT64_C(0)) {
        rows = (spx_shb_stored *)SPX_SHB_METADATA_CALLOC((size_t)count, sizeof(spx_shb_stored));
        table = rows == NULL ? NULL
            : (spx_shb_lease *)SPX_SHB_METADATA_CALLOC((size_t)count, sizeof(spx_shb_lease));
        if (table == NULL) {
            if (rows != NULL) SPX_SHB_METADATA_FREE(rows);
            registry->child_held -= count;
            r.backing->partition_authority = 0u;
            registry->vec[authority - 1u] = (spx_shb_vec_entry){0};
            SPX_SHB_FAIL_STOP("shared Bytes partition metadata allocation failed");
        }
    }
    uint64_t ticket = registry->next_partition_ticket++;
    /* A nonempty child's width is bounded by the admitted backing extent.
       An empty partition has no ranges and needs no stored width. */
    registry->vec[authority - 1u].partition = (spx_shb_partition_reservation){
        .rows = rows, .table = table, .source_nonce = source->nonce, .ticket = ticket,
        .width = count == UINT64_C(0) ? 0u : (uint32_t)width,
        .count = (uint16_t)count, .backing_index = (uint16_t)r.index };
    *plan = (spx_shb_partition_plan){ .issuer = registry->issuer, .ticket = ticket,
        .authority = authority, .armed = 1u };
    return SPX_SHB_OK;
}
SPX_SHB_FN spx_shb_vec_entry *spx_shb_plan_require(spx_shb_registry *registry, const spx_shb_partition_plan *plan) {
    spx_shb_require(registry);
    if (plan == NULL || !plan->armed || plan->issuer != registry->issuer || plan->authority == 0u
        || plan->authority > SPX_SHB_VECS || plan->ticket == UINT64_C(0))
        SPX_SHB_FAIL_STOP("forged shared Bytes partition plan");
    spx_shb_vec_entry *e = &registry->vec[plan->authority - 1u];
    if (e->state != SPX_SHB_VEC_RESERVED || e->partition.ticket != plan->ticket
        || e->partition.backing_index >= registry->events
        || registry->backing[e->partition.backing_index].partition_authority != plan->authority)
        SPX_SHB_FAIL_STOP("forged shared Bytes partition reservation");
    return e;
}
SPX_SHB_FN void spx_shb_partition_abort(spx_shb_registry *registry, spx_shb_partition_plan *plan) {
    spx_shb_vec_entry *e = spx_shb_plan_require(registry, plan);
    spx_shb_partition_reservation reservation = e->partition;
    registry->child_held -= reservation.count;
    registry->backing[reservation.backing_index].partition_authority = 0u;
    *e = (spx_shb_vec_entry){0}; *plan = (spx_shb_partition_plan){0};
    if (reservation.table != NULL) SPX_SHB_METADATA_FREE(reservation.table);
    if (reservation.rows != NULL) SPX_SHB_METADATA_FREE(reservation.rows);
}
SPX_SHB_FN void spx_shb_partition_commit(
    spx_shb_registry *registry, spx_shb_partition_plan *plan, spx_shb_bytes *source, spx_shb_vec *out
) {
    spx_shb_vec_entry *e = spx_shb_plan_require(registry, plan);
    spx_shb_partition_reservation reservation = e->partition;
    uint64_t whole_slot = ((uint64_t)reservation.backing_index + 1u) << 32;
    if (source == NULL || out == NULL || source->issuer != registry->issuer
        || source->slot != whole_slot || source->nonce != reservation.source_nonce)
        SPX_SHB_FAIL_STOP("shared Bytes partition commit source differs from its plan");
    spx_shb_resolved r = spx_shb_resolve_unloaned(registry, source);
    if (r.backing->state != SPX_SHB_WHOLE || r.lease_index != UINT64_C(0)
        || (uint64_t)reservation.count * reservation.width != r.len)
        SPX_SHB_FAIL_STOP("shared Bytes partition commit lost its preflight");
    if (registry->next_nonce == UINT64_C(0) || reservation.count > UINT64_MAX - registry->next_nonce)
        SPX_SHB_FAIL_STOP("shared Bytes lease generation exhausted");
    if (registry->next_generation == UINT64_C(0) || registry->next_generation == UINT64_MAX)
        SPX_SHB_FAIL_STOP("shared Bytes Vec generation exhausted");
    spx_shb_backing *backing = r.backing;
    *source = (spx_shb_bytes){0};
    backing->whole.live = 0u;
    backing->partition_authority = 0u;
    registry->child_held -= reservation.count; registry->child_table_slots += reservation.count;
    if (reservation.count == UINT64_C(0)) {
        backing->live = 0u; spx_shb_release_backing(registry, backing);
    } else {
        for (uint64_t i = UINT64_C(0); i < reservation.count; ++i) {
            reservation.table[i] = (spx_shb_lease){ .nonce = spx_shb_fresh_nonce(registry), .live = 1u };
            reservation.rows[i] = (spx_shb_stored){ .slot = whole_slot | (i + 1u), .nonce = reservation.table[i].nonce };
        }
        backing->child = reservation.table; backing->width = reservation.width;
        backing->count = reservation.count; backing->live = reservation.count;
        backing->state = SPX_SHB_PARTITIONED;
    }
    spx_shb_vec staged = { .rows = reservation.rows, .len = reservation.count, .capacity = reservation.count,
        .issuer = registry->issuer, .authority = plan->authority };
    *plan = (spx_shb_partition_plan){0};
    spx_shb_vec_publish(registry, &staged, e, out);
}

/* Public boundary: copy out into a fresh provider allocation, then settle the
   private lease and publish. A provider allocation failure leaves both the
   owner and the caller's result slot intact. */
SPX_SHB_FN uint32_t spx_shb_copy_out(spx_shb_registry *registry, spx_shb_bytes *value, spx_bytes_v1 *out) {
    if (out == NULL) SPX_SHB_FAIL_STOP("shared Bytes copy-out has no public carrier");
    spx_shb_resolved r = spx_shb_resolve_unloaned(registry, value);
    uint8_t *copy = NULL;
    if (r.len != UINT64_C(0)) {
        copy = (uint8_t *)SPX_SHB_PROVIDER_MALLOC((size_t)r.len);
        if (copy == NULL) return SPX_SHB_PROVIDER_ALLOCATION;
        memcpy(copy, r.backing->payload + (size_t)r.offset, (size_t)r.len);
    }
    spx_shb_drop(registry, value);
    *out = (spx_bytes_v1){ .ptr = copy, .len = r.len };
    return SPX_SHB_OK;
}
SPX_SHB_FN void spx_shb_provider_drop(spx_bytes_v1 *value) {
    if (value == NULL || value->len > SPX_SHB_EXTENT || (value->len == UINT64_C(0)) != (value->ptr == NULL))
        SPX_SHB_FAIL_STOP("invalid provider-owned Bytes copy");
    uint8_t *ptr = value->ptr; *value = (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    if (ptr != NULL) SPX_SHB_PROVIDER_FREE(ptr);
}

/* Final settlement: every backing released exactly once, every Vec settled,
   no partition hold outstanding. A leak is an invariant failure. */
SPX_SHB_FN void spx_shb_registry_finish(spx_shb_registry *registry) {
    spx_shb_require(registry);
    if (registry->loans != 0u) SPX_SHB_FAIL_STOP("shared Bytes registry finished with a live loan");
    if (registry->child_held != UINT64_C(0)) SPX_SHB_FAIL_STOP("shared Bytes partition hold outlived its call");
    for (uint32_t i = 0u; i < registry->events; ++i) {
        const spx_shb_backing *backing = &registry->backing[i];
        if (backing->partition_authority != 0u)
            SPX_SHB_FAIL_STOP("shared Bytes partition hold outlived its call");
        if (backing->state != SPX_SHB_RELEASED || backing->live != 0u)
            SPX_SHB_FAIL_STOP("shared Bytes registry finished with a live lease");
    }
    if (registry->child_table_slots != UINT64_C(0))
        SPX_SHB_FAIL_STOP("shared Bytes child table outlived its backing");
    for (uint32_t i = 0u; i < SPX_SHB_VECS; ++i)
        if (registry->vec[i].state != SPX_SHB_VEC_FREE)
            SPX_SHB_FAIL_STOP("shared Bytes registry finished with a live Vec");
    registry->finished = 1u;
}
