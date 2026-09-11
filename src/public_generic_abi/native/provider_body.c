/* Stable body of the native C11 physical adapter for Public Generic Carrier
 * v1 (issue #154). This fragment is not standalone: it assumes
 * "spx_pg_v1.h" is already visible and that the trusted constant arrays
 * (SPX_PG_TRUSTED_DESCRIPTOR_BYTES, SPX_PG_TRUSTED_BINDING_BYTES, and the
 * SPX_PG_TRUSTED_*_LEN companions) are already defined above it — see
 * ../template.rs, which renders those constants from a real
 * VerifiedPublicGenericDescriptor and NativeProviderBindingV1 and pastes
 * this file's text below them into one translation unit.
 *
 * PHYSICAL layer only: this file allocates, copies, and releases bytes. It
 * never decides legality beyond replaying the trusted bytes it was
 * generated with and enforcing the handle-lifecycle rules that
 * docs/PUBLIC-GENERIC-CARRIER-V1.md already fixes as LOGICAL fact — every
 * rule enforced below cites the exact table row it restates. Single
 * translation unit, single-threaded: no concurrency claim is made anywhere
 * in this file.
 *
 * The bound endpoint is a fixture: it reverses each owned leaf's bytes.
 * Deriving a real checked-program endpoint from validated generic HIR is
 * #119's remaining prerequisite (owned-record ownership evidence); until
 * that lands, this adapter operates on the existing owned-Bytes shapes only
 * (a flat sequence of independent owned leaves), matching the "existing
 * owned-Bytes shapes" scope this issue's brief calls out.
 */

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/* --- Bounds, reused verbatim from Public Generic Boundary Profile v1 (see
 * src/public_generic_abi/boundary_profile.rs) so this file never restates a
 * number independently; tests/public_generic_native_adapter_v1 pins these
 * against the Rust constants so the two cannot silently drift. */
#define SPX_PG_MAX_OWNED_LEAVES 256u
#define SPX_PG_MAX_BYTES_PER_LEAF (64u * 1024u)
#define SPX_PG_MAX_TOTAL_PAYLOAD_BYTES (16u * 1024u * 1024u)

#define SPX_PG_KIND_NONE 0u
#define SPX_PG_KIND_VALUE 1u
#define SPX_PG_KIND_RESULT 2u

#define SPX_PG_REGISTRY_CAPACITY 256

struct spx_pg_provider_v1 {
    uint32_t magic;
    uint32_t next_generation;
};

struct spx_pg_value_v1 {
    uint32_t magic;
    uint32_t generation;
    struct spx_pg_provider_v1 *owner;
    uint32_t leaf_count;
    uint8_t **leaf_bytes;
    size_t *leaf_lens;
};

struct spx_pg_result_v1 {
    uint32_t magic;
    uint32_t generation;
    struct spx_pg_provider_v1 *owner;
    uint32_t leaf_count;
    uint8_t **leaf_bytes;
    size_t *leaf_lens;
};

#define SPX_PG_PROVIDER_MAGIC 0x53504750u /* 'SPGP' */
#define SPX_PG_VALUE_MAGIC 0x53504756u    /* 'SPGV' */
#define SPX_PG_RESULT_MAGIC 0x53504752u   /* 'SPGR' */

/* --- The registry: every live top-level value/result handle this
 * translation unit has minted, across every open provider. Handle safety
 * (wrong/stale/cross-paired/double-used/foreign) is decided here by
 * scanning for the exact pointer VALUE first; the pointee is dereferenced
 * only once a live, correctly-kinded entry is found, per "do not
 * dereference an arbitrary attacker pointer merely to discover that it is
 * invalid." A released or transferred entry is zeroed out immediately so a
 * later `malloc` returning the same address can never be mistaken for the
 * old, already-invalidated handle. */
typedef struct {
    void *pointer;
    uint32_t kind;
    void *owner;
    uint32_t generation;
} spx_pg_registry_entry;

static spx_pg_registry_entry g_spx_pg_registry[SPX_PG_REGISTRY_CAPACITY];
static uint32_t g_spx_pg_next_generation = 1;

/* --- The adapter's own bounded allocator. Every heap byte this file
 * allocates — provider/value/result structs, leaf pointer/length arrays,
 * and leaf payload bytes alike — passes through here, so
 * `spx_pg_test_live_allocations_v1` is exact, not a sample. */
static size_t g_spx_pg_live_allocations = 0;
static size_t g_spx_pg_live_bytes = 0;

static void *spx_pg_alloc(size_t size) {
    if (size == 0) {
        return NULL;
    }
    if (g_spx_pg_live_bytes + size > SPX_PG_MAX_TOTAL_PAYLOAD_BYTES) {
        return NULL; /* SPX-PG903: bounded allocator exhausted */
    }
    void *pointer = malloc(size);
    if (pointer == NULL) {
        return NULL;
    }
    g_spx_pg_live_allocations += 1;
    g_spx_pg_live_bytes += size;
    return pointer;
}

static void spx_pg_dealloc(void *pointer, size_t size) {
    if (pointer == NULL) {
        return;
    }
    free(pointer);
    g_spx_pg_live_allocations -= 1;
    g_spx_pg_live_bytes -= size;
}

/* --- The normalized trace: append-only, ordinal-numbered, closed
 * vocabulary. Carries only the SPX_PG_TRACE_* label — no payload byte, no
 * pointer, no timestamp — matching carrier::trace::TraceEvent's own shape. */
#define SPX_PG_TRACE_CAPACITY 4096
static uint32_t g_spx_pg_trace[SPX_PG_TRACE_CAPACITY];
static size_t g_spx_pg_trace_len = 0;

static void spx_pg_trace_record(uint32_t label) {
    if (g_spx_pg_trace_len < SPX_PG_TRACE_CAPACITY) {
        g_spx_pg_trace[g_spx_pg_trace_len] = label;
        g_spx_pg_trace_len += 1;
    }
}

/* --- Deterministic failure injection, test-only. Arms for exactly one
 * subsequent trace label. */
static uint32_t g_spx_pg_injected_ordinal = SPX_PG_TEST_NO_INJECTION;

static int spx_pg_should_inject(uint32_t label) {
    if (g_spx_pg_injected_ordinal == label) {
        g_spx_pg_injected_ordinal = SPX_PG_TEST_NO_INJECTION;
        return 1;
    }
    return 0;
}

void spx_pg_test_inject_failure_v1(uint32_t ordinal) {
    g_spx_pg_injected_ordinal = ordinal;
}

void spx_pg_test_clear_failure_injection_v1(void) {
    g_spx_pg_injected_ordinal = SPX_PG_TEST_NO_INJECTION;
}

size_t spx_pg_test_live_allocations_v1(void) {
    return g_spx_pg_live_allocations;
}

/* Production helper: counts live handles owned by `provider`, used both by
 * `spx_pg_provider_close_v1`'s own "close with live handles" refusal and by
 * the test-only accessor below. */
static size_t spx_pg_count_live_handles(const spx_pg_provider_v1 *provider) {
    size_t live = 0;
    for (size_t index = 0; index < SPX_PG_REGISTRY_CAPACITY; ++index) {
        if (g_spx_pg_registry[index].kind != SPX_PG_KIND_NONE &&
            g_spx_pg_registry[index].owner == (const void *)provider) {
            live += 1;
        }
    }
    return live;
}

size_t spx_pg_test_live_handles_v1(spx_pg_provider_v1 *provider) {
    return spx_pg_count_live_handles(provider);
}

size_t spx_pg_test_trace_len_v1(void) {
    return g_spx_pg_trace_len;
}

uint32_t spx_pg_test_trace_label_v1(size_t index) {
    if (index >= g_spx_pg_trace_len) {
        return SPX_PG_TEST_NO_INJECTION;
    }
    return g_spx_pg_trace[index];
}

/* --- Sticky settlement. The FIRST selected outcome wins; a later, distinct
 * attempt is counted here and discarded, exactly matching SPX-PG806's rule
 * as CallLedger::settle enforces it, restated in the physical layer instead
 * of reimplemented independently. */
static size_t g_spx_pg_settlement_overwrites = 0;
static int g_spx_pg_settlement_selected = 0;
static spx_pg_status_v1 g_spx_pg_settlement_status = SPX_PG_STATUS_OK;

static void spx_pg_reset_call_state(void) {
    g_spx_pg_settlement_selected = 0;
    g_spx_pg_settlement_status = SPX_PG_STATUS_OK;
}

/* Select `status` as the terminal outcome for the call in progress. Sticky:
 * once selected, a different later status is rejected (counted, not
 * applied) and the ORIGINAL status is returned. */
static spx_pg_status_v1 spx_pg_settle(spx_pg_status_v1 status) {
    if (!g_spx_pg_settlement_selected) {
        g_spx_pg_settlement_selected = 1;
        g_spx_pg_settlement_status = status;
    } else if (g_spx_pg_settlement_status != status) {
        g_spx_pg_settlement_overwrites += 1;
    }
    spx_pg_trace_record(SPX_PG_TRACE_TERMINAL_STATUS);
    return g_spx_pg_settlement_status;
}

size_t spx_pg_test_settlement_overwrite_attempts_v1(void) {
    return g_spx_pg_settlement_overwrites;
}

spx_pg_status_v1 spx_pg_test_force_settlement_conflict_v1(spx_pg_status_v1 status) {
    return spx_pg_settle(status);
}

/* --- Registry operations. --- */

static int spx_pg_registry_insert(void *pointer, uint32_t kind, void *owner, uint32_t generation) {
    for (size_t index = 0; index < SPX_PG_REGISTRY_CAPACITY; ++index) {
        if (g_spx_pg_registry[index].kind == SPX_PG_KIND_NONE) {
            g_spx_pg_registry[index].pointer = pointer;
            g_spx_pg_registry[index].kind = kind;
            g_spx_pg_registry[index].owner = owner;
            g_spx_pg_registry[index].generation = generation;
            return 1;
        }
    }
    return 0; /* registry exhausted: mapped to SPX-PG903 by the caller */
}

/* Find a live entry by pointer VALUE only (no dereference of `pointer`
 * itself). Returns the slot index, or SPX_PG_REGISTRY_CAPACITY if absent. */
static size_t spx_pg_registry_find(const void *pointer, uint32_t kind) {
    for (size_t index = 0; index < SPX_PG_REGISTRY_CAPACITY; ++index) {
        if (g_spx_pg_registry[index].kind == kind && g_spx_pg_registry[index].pointer == pointer) {
            return index;
        }
    }
    return SPX_PG_REGISTRY_CAPACITY;
}

static void spx_pg_registry_remove(size_t index) {
    g_spx_pg_registry[index].pointer = NULL;
    g_spx_pg_registry[index].kind = SPX_PG_KIND_NONE;
    g_spx_pg_registry[index].owner = NULL;
    g_spx_pg_registry[index].generation = 0;
}

/* --- Carrier byte codec. Framing matches src/public_generic_abi.rs's
 * `frame`/`read_frame`: an 8-byte little-endian length, then the bytes,
 * applied here as [u64 leaf_count][per leaf: u64 len][bytes]. */

static uint64_t spx_pg_read_u64le(const uint8_t *bytes) {
    uint64_t value = 0;
    for (int index = 7; index >= 0; --index) {
        value = (value << 8) | bytes[index];
    }
    return value;
}

static void spx_pg_write_u64le(uint8_t *out, uint64_t value) {
    for (int index = 0; index < 8; ++index) {
        out[index] = (uint8_t)(value & 0xffu);
        value >>= 8;
    }
}

/* Preflight only: framing, leaf count, and byte bounds. Allocates nothing.
 * Corresponds to the FrameValidated trace event / CarrierCallMachine's
 * `validate`. */
static spx_pg_status_v1 spx_pg_preflight_carrier(const uint8_t *bytes, size_t len,
                                                  uint32_t *out_leaf_count) {
    if (len < 8) {
        return SPX_PG_STATUS_MALFORMED_CARRIER;
    }
    uint64_t leaf_count64 = spx_pg_read_u64le(bytes);
    if (leaf_count64 > SPX_PG_MAX_OWNED_LEAVES) {
        return SPX_PG_STATUS_CARRIER_CAPACITY;
    }
    size_t offset = 8;
    size_t total_payload = 0;
    for (uint64_t leaf = 0; leaf < leaf_count64; ++leaf) {
        if (len - offset < 8) {
            return SPX_PG_STATUS_MALFORMED_CARRIER;
        }
        uint64_t leaf_len64 = spx_pg_read_u64le(bytes + offset);
        offset += 8;
        if (leaf_len64 > SPX_PG_MAX_BYTES_PER_LEAF) {
            return SPX_PG_STATUS_CARRIER_CAPACITY;
        }
        if (len - offset < leaf_len64) {
            return SPX_PG_STATUS_MALFORMED_CARRIER;
        }
        offset += (size_t)leaf_len64;
        total_payload += (size_t)leaf_len64;
        if (total_payload > SPX_PG_MAX_TOTAL_PAYLOAD_BYTES) {
            return SPX_PG_STATUS_CARRIER_CAPACITY;
        }
    }
    if (offset != len) {
        return SPX_PG_STATUS_MALFORMED_CARRIER;
    }
    *out_leaf_count = (uint32_t)leaf_count64;
    return SPX_PG_STATUS_OK;
}

/* Release `count` already-allocated leaves in exact reverse order, matching
 * "release order after a failed transfer is the exact reverse of the
 * canonical obligation order." Safe to call with a partially filled array:
 * `filled` names how many leaves actually hold owned memory. */
/* Injection here never skips the physical free: a "cleanup failure" is
 * something else going wrong during discharge, never a leaked allocation.
 * When armed, it settles CONTRACT_FAILURE as a side effect: if no outcome is
 * selected yet, that cleanup failure legally becomes the terminal status
 * (the success path exercises this); if a different outcome is already
 * selected, the attempt is rejected and counted, exactly matching "cleanup
 * cannot replace the selected status." */
static void spx_pg_release_leaves(uint8_t **leaf_bytes, size_t *leaf_lens, uint32_t filled) {
    for (uint32_t index = filled; index-- > 0;) {
        spx_pg_dealloc(leaf_bytes[index], leaf_lens[index]);
        spx_pg_trace_record(SPX_PG_TRACE_LEAF_RELEASE);
        if (spx_pg_should_inject(SPX_PG_TRACE_LEAF_RELEASE)) {
            (void)spx_pg_settle(SPX_PG_STATUS_CONTRACT_FAILURE);
        }
    }
    spx_pg_trace_record(SPX_PG_TRACE_CARRIER_RELEASE);
    if (spx_pg_should_inject(SPX_PG_TRACE_CARRIER_RELEASE)) {
        (void)spx_pg_settle(SPX_PG_STATUS_CONTRACT_FAILURE);
    }
}

/* Allocate and copy every leaf's payload bytes, root then leaves in
 * structural order (there is no separate root payload in this flat-Bytes
 * shape; the root is the aggregate the leaves belong to). On any failure
 * (bounded-allocator exhaustion or injected failure), every leaf allocated
 * so far is released in reverse order before returning, so the caller never
 * observes a partial value. */
static spx_pg_status_v1 spx_pg_fill_leaves(const uint8_t *bytes, size_t offset, uint32_t leaf_count,
                                            uint8_t **leaf_bytes, size_t *leaf_lens,
                                            uint32_t started_label, uint32_t committed_label,
                                            uint32_t payload_label) {
    for (uint32_t leaf = 0; leaf < leaf_count; ++leaf) {
        spx_pg_trace_record(started_label);
        if (spx_pg_should_inject(started_label)) {
            spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf);
            return SPX_PG_STATUS_ALLOCATION_FAILURE;
        }
        uint64_t leaf_len = spx_pg_read_u64le(bytes + offset);
        size_t payload_offset = offset + 8;
        uint8_t *storage = (leaf_len == 0) ? NULL : (uint8_t *)spx_pg_alloc((size_t)leaf_len);
        if (leaf_len != 0 && storage == NULL) {
            spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf);
            return SPX_PG_STATUS_ALLOCATION_FAILURE;
        }
        leaf_bytes[leaf] = storage;
        leaf_lens[leaf] = (size_t)leaf_len;
        spx_pg_trace_record(committed_label);
        if (spx_pg_should_inject(committed_label)) {
            spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf + 1);
            return SPX_PG_STATUS_ALLOCATION_FAILURE;
        }
        if (leaf_len != 0) {
            memcpy(storage, bytes + payload_offset, (size_t)leaf_len);
        }
        offset = payload_offset + (size_t)leaf_len;
        spx_pg_trace_record(payload_label);
        if (spx_pg_should_inject(payload_label)) {
            spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf + 1);
            return SPX_PG_STATUS_ALLOCATION_FAILURE;
        }
    }
    return SPX_PG_STATUS_OK;
}

/* --- The bound checked endpoint (fixture): reverse every leaf's bytes.
 * Stands in for a real checked generic export until #119 unblocks deriving
 * one from validated HIR; see this file's header comment. */
static spx_pg_status_v1 spx_pg_endpoint_reverse_bytes_v1(uint32_t leaf_count,
                                                          uint8_t *const *input_leaf_bytes,
                                                          const size_t *input_leaf_lens,
                                                          uint8_t **out_leaf_bytes,
                                                          size_t *out_leaf_lens) {
    for (uint32_t leaf = 0; leaf < leaf_count; ++leaf) {
        size_t length = input_leaf_lens[leaf];
        uint8_t *storage = (length == 0) ? NULL : (uint8_t *)spx_pg_alloc(length);
        if (length != 0 && storage == NULL) {
            for (uint32_t undo = leaf; undo-- > 0;) {
                spx_pg_dealloc(out_leaf_bytes[undo], out_leaf_lens[undo]);
            }
            return SPX_PG_STATUS_ALLOCATION_FAILURE;
        }
        for (size_t index = 0; index < length; ++index) {
            storage[index] = input_leaf_bytes[leaf][length - 1 - index];
        }
        out_leaf_bytes[leaf] = storage;
        out_leaf_lens[leaf] = length;
    }
    return SPX_PG_STATUS_OK;
}

/* --- Provider binding / descriptor replay. --- */

static int spx_pg_bytes_equal(const uint8_t *left, size_t left_len, const uint8_t *right,
                               size_t right_len) {
    return left_len == right_len && (left_len == 0 || memcmp(left, right, left_len) == 0);
}

spx_pg_status_v1 spx_pg_provider_open_v1(const uint8_t *descriptor_bytes, size_t descriptor_len,
                                          const uint8_t *provider_binding_bytes,
                                          size_t provider_binding_len,
                                          spx_pg_provider_v1 **out_provider) {
    if (out_provider == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    *out_provider = NULL;
    if (descriptor_bytes == NULL || provider_binding_bytes == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    if (!spx_pg_bytes_equal(descriptor_bytes, descriptor_len, SPX_PG_TRUSTED_DESCRIPTOR_BYTES,
                             SPX_PG_TRUSTED_DESCRIPTOR_LEN)) {
        return SPX_PG_STATUS_DESCRIPTOR_REPLAY_MISMATCH;
    }
    if (!spx_pg_bytes_equal(provider_binding_bytes, provider_binding_len,
                             SPX_PG_TRUSTED_BINDING_BYTES, SPX_PG_TRUSTED_BINDING_LEN)) {
        return SPX_PG_STATUS_BINDING_REPLAY_MISMATCH;
    }
    spx_pg_provider_v1 *provider =
        (spx_pg_provider_v1 *)spx_pg_alloc(sizeof(spx_pg_provider_v1));
    if (provider == NULL) {
        return SPX_PG_STATUS_ALLOCATION_FAILURE;
    }
    provider->magic = SPX_PG_PROVIDER_MAGIC;
    provider->next_generation = 1;
    *out_provider = provider;
    return SPX_PG_STATUS_OK;
}

spx_pg_status_v1 spx_pg_provider_close_v1(spx_pg_provider_v1 **provider) {
    if (provider == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    if (*provider == NULL) {
        return SPX_PG_STATUS_OK; /* closing null is a no-op success */
    }
    if (spx_pg_count_live_handles(*provider) != 0) {
        return SPX_PG_STATUS_ILLEGAL_TRANSITION;
    }
    spx_pg_dealloc(*provider, sizeof(spx_pg_provider_v1));
    *provider = NULL;
    return SPX_PG_STATUS_OK;
}

/* --- Input preparation. --- */

spx_pg_status_v1 spx_pg_input_prepare_v1(spx_pg_provider_v1 *provider, const uint8_t *carrier_bytes,
                                          size_t carrier_len, spx_pg_value_v1 **out_input) {
    if (out_input == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    *out_input = NULL;
    if (provider == NULL || provider->magic != SPX_PG_PROVIDER_MAGIC ||
        (carrier_bytes == NULL && carrier_len != 0)) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    spx_pg_reset_call_state();
    uint32_t leaf_count = 0;
    spx_pg_status_v1 status = spx_pg_preflight_carrier(carrier_bytes, carrier_len, &leaf_count);
    if (status != SPX_PG_STATUS_OK) {
        return spx_pg_settle(status);
    }
    spx_pg_trace_record(SPX_PG_TRACE_FRAME_VALIDATED);
    if (spx_pg_should_inject(SPX_PG_TRACE_FRAME_VALIDATED)) {
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }

    uint8_t **leaf_bytes = NULL;
    size_t *leaf_lens = NULL;
    if (leaf_count != 0) {
        leaf_bytes = (uint8_t **)spx_pg_alloc(sizeof(uint8_t *) * leaf_count);
        leaf_lens = (size_t *)spx_pg_alloc(sizeof(size_t) * leaf_count);
        if (leaf_bytes == NULL || leaf_lens == NULL) {
            if (leaf_bytes != NULL) {
                spx_pg_dealloc(leaf_bytes, sizeof(uint8_t *) * leaf_count);
            }
            if (leaf_lens != NULL) {
                spx_pg_dealloc(leaf_lens, sizeof(size_t) * leaf_count);
            }
            return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
        }
    }

    status = spx_pg_fill_leaves(carrier_bytes, 8, leaf_count, leaf_bytes, leaf_lens,
                                 SPX_PG_TRACE_LEAF_ALLOCATION_STARTED,
                                 SPX_PG_TRACE_LEAF_ALLOCATION_COMMITTED,
                                 SPX_PG_TRACE_LEAF_PAYLOAD_COPIED);
    if (status != SPX_PG_STATUS_OK) {
        if (leaf_bytes != NULL) {
            spx_pg_dealloc(leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (leaf_lens != NULL) {
            spx_pg_dealloc(leaf_lens, sizeof(size_t) * leaf_count);
        }
        return spx_pg_settle(status);
    }

    spx_pg_value_v1 *value = (spx_pg_value_v1 *)spx_pg_alloc(sizeof(spx_pg_value_v1));
    if (value == NULL) {
        spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf_count);
        if (leaf_bytes != NULL) {
            spx_pg_dealloc(leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (leaf_lens != NULL) {
            spx_pg_dealloc(leaf_lens, sizeof(size_t) * leaf_count);
        }
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }
    value->magic = SPX_PG_VALUE_MAGIC;
    value->generation = g_spx_pg_next_generation++;
    value->owner = provider;
    value->leaf_count = leaf_count;
    value->leaf_bytes = leaf_bytes;
    value->leaf_lens = leaf_lens;

    if (!spx_pg_registry_insert(value, SPX_PG_KIND_VALUE, provider, value->generation)) {
        spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf_count);
        if (leaf_bytes != NULL) {
            spx_pg_dealloc(leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (leaf_lens != NULL) {
            spx_pg_dealloc(leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(value, sizeof(spx_pg_value_v1));
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }

    spx_pg_trace_record(SPX_PG_TRACE_INPUT_VALUE_PREPARED);
    if (spx_pg_should_inject(SPX_PG_TRACE_INPUT_VALUE_PREPARED)) {
        size_t slot = spx_pg_registry_find(value, SPX_PG_KIND_VALUE);
        if (slot != SPX_PG_REGISTRY_CAPACITY) {
            spx_pg_registry_remove(slot);
        }
        spx_pg_release_leaves(leaf_bytes, leaf_lens, leaf_count);
        if (leaf_bytes != NULL) {
            spx_pg_dealloc(leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (leaf_lens != NULL) {
            spx_pg_dealloc(leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(value, sizeof(spx_pg_value_v1));
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }
    *out_input = value;
    return SPX_PG_STATUS_OK;
}

/* --- Call: commit input transfer, run the endpoint, stage and commit the
 * result. `input` is invalidated (removed from the registry) exactly once
 * here, success or failure. --- */

spx_pg_status_v1 spx_pg_call_v1(spx_pg_provider_v1 *provider, spx_pg_value_v1 *input,
                                 spx_pg_result_v1 **out_result) {
    if (out_result == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    *out_result = NULL;
    if (provider == NULL || provider->magic != SPX_PG_PROVIDER_MAGIC) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    size_t slot = spx_pg_registry_find(input, SPX_PG_KIND_VALUE);
    if (slot == SPX_PG_REGISTRY_CAPACITY || g_spx_pg_registry[slot].owner != (void *)provider) {
        return SPX_PG_STATUS_HANDLE_INVALID;
    }

    /* The one atomic commit point: `input` is removed from the registry
     * right here, before any of its bytes are read, so "transfer invalidates
     * caller ownership exactly once" holds regardless of what happens next.
     */
    spx_pg_registry_remove(slot);
    spx_pg_trace_record(SPX_PG_TRACE_INPUT_TRANSFER_COMMITTED);
    if (spx_pg_should_inject(SPX_PG_TRACE_INPUT_TRANSFER_COMMITTED)) {
        spx_pg_release_leaves(input->leaf_bytes, input->leaf_lens, input->leaf_count);
        if (input->leaf_bytes != NULL) {
            spx_pg_dealloc(input->leaf_bytes, sizeof(uint8_t *) * input->leaf_count);
        }
        if (input->leaf_lens != NULL) {
            spx_pg_dealloc(input->leaf_lens, sizeof(size_t) * input->leaf_count);
        }
        spx_pg_dealloc(input, sizeof(spx_pg_value_v1));
        return spx_pg_settle(SPX_PG_STATUS_ILLEGAL_TRANSITION);
    }

    spx_pg_trace_record(SPX_PG_TRACE_EXECUTION_STARTED);
    if (spx_pg_should_inject(SPX_PG_TRACE_EXECUTION_STARTED)) {
        spx_pg_release_leaves(input->leaf_bytes, input->leaf_lens, input->leaf_count);
        if (input->leaf_bytes != NULL) {
            spx_pg_dealloc(input->leaf_bytes, sizeof(uint8_t *) * input->leaf_count);
        }
        if (input->leaf_lens != NULL) {
            spx_pg_dealloc(input->leaf_lens, sizeof(size_t) * input->leaf_count);
        }
        spx_pg_dealloc(input, sizeof(spx_pg_value_v1));
        return spx_pg_settle(SPX_PG_STATUS_CONTRACT_FAILURE);
    }

    uint32_t leaf_count = input->leaf_count;
    uint8_t **result_leaf_bytes = NULL;
    size_t *result_leaf_lens = NULL;
    if (leaf_count != 0) {
        result_leaf_bytes = (uint8_t **)spx_pg_alloc(sizeof(uint8_t *) * leaf_count);
        result_leaf_lens = (size_t *)spx_pg_alloc(sizeof(size_t) * leaf_count);
    }
    spx_pg_status_v1 endpoint_status = SPX_PG_STATUS_OK;
    if (leaf_count != 0 && (result_leaf_bytes == NULL || result_leaf_lens == NULL)) {
        endpoint_status = SPX_PG_STATUS_ALLOCATION_FAILURE;
    } else {
        endpoint_status = spx_pg_endpoint_reverse_bytes_v1(leaf_count, input->leaf_bytes,
                                                            input->leaf_lens, result_leaf_bytes,
                                                            result_leaf_lens);
    }

    /* Non-result obligation: release the consumed input before the result is
     * ever published, matching "result publication follows postconditions
     * and non-result cleanup." */
    spx_pg_release_leaves(input->leaf_bytes, input->leaf_lens, input->leaf_count);
    if (input->leaf_bytes != NULL) {
        spx_pg_dealloc(input->leaf_bytes, sizeof(uint8_t *) * input->leaf_count);
    }
    if (input->leaf_lens != NULL) {
        spx_pg_dealloc(input->leaf_lens, sizeof(size_t) * input->leaf_count);
    }
    spx_pg_dealloc(input, sizeof(spx_pg_value_v1));

    int endpoint_already_succeeded = (endpoint_status == SPX_PG_STATUS_OK);
    spx_pg_trace_record(SPX_PG_TRACE_EXECUTION_FINISHED);
    if (spx_pg_should_inject(SPX_PG_TRACE_EXECUTION_FINISHED)) {
        endpoint_status = SPX_PG_STATUS_CONTRACT_FAILURE;
    }

    if (endpoint_status != SPX_PG_STATUS_OK) {
        /* If the endpoint had already fully allocated and populated every
         * result leaf before this injected post-hoc rejection, those leaf
         * payloads are real, live allocations that must be rolled back too
         * — unlike a genuine endpoint failure (already self-cleaned above)
         * or an array-allocation failure (nothing was allocated yet). */
        if (endpoint_already_succeeded) {
            for (uint32_t index = 0; index < leaf_count; ++index) {
                spx_pg_dealloc(result_leaf_bytes[index], result_leaf_lens[index]);
            }
        }
        if (result_leaf_bytes != NULL) {
            spx_pg_dealloc(result_leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result_leaf_lens != NULL) {
            spx_pg_dealloc(result_leaf_lens, sizeof(size_t) * leaf_count);
        }
        return spx_pg_settle(endpoint_status);
    }

    spx_pg_result_v1 *result = (spx_pg_result_v1 *)spx_pg_alloc(sizeof(spx_pg_result_v1));
    if (result == NULL) {
        if (result_leaf_bytes != NULL) {
            for (uint32_t index = 0; index < leaf_count; ++index) {
                spx_pg_dealloc(result_leaf_bytes[index], result_leaf_lens[index]);
            }
            spx_pg_dealloc(result_leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result_leaf_lens != NULL) {
            spx_pg_dealloc(result_leaf_lens, sizeof(size_t) * leaf_count);
        }
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }
    result->magic = SPX_PG_RESULT_MAGIC;
    result->generation = g_spx_pg_next_generation++;
    result->owner = provider;
    result->leaf_count = leaf_count;
    result->leaf_bytes = result_leaf_bytes;
    result->leaf_lens = result_leaf_lens;

    /* The endpoint already performed the physical allocation above; these
     * events record it in the normalized trace in structural order.
     * Injection here models a deliberate post-hoc rejection (the engine
     * chooses to fail after the physical work already succeeded), so on
     * injection every result leaf actually allocated is rolled back rather
     * than leaked. */
    int result_leaf_trace_injected = 0;
    for (uint32_t index = 0; index < leaf_count; ++index) {
        spx_pg_trace_record(SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_STARTED);
        if (spx_pg_should_inject(SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_STARTED)) {
            result_leaf_trace_injected = 1;
        }
        spx_pg_trace_record(SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_COMMITTED);
        if (spx_pg_should_inject(SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_COMMITTED)) {
            result_leaf_trace_injected = 1;
        }
    }
    if (result_leaf_trace_injected) {
        for (uint32_t index = 0; index < leaf_count; ++index) {
            spx_pg_dealloc(result->leaf_bytes[index], result->leaf_lens[index]);
        }
        if (result->leaf_bytes != NULL) {
            spx_pg_dealloc(result->leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result->leaf_lens != NULL) {
            spx_pg_dealloc(result->leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(result, sizeof(spx_pg_result_v1));
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }
    spx_pg_trace_record(SPX_PG_TRACE_RESULT_VALUE_PREPARED);
    if (spx_pg_should_inject(SPX_PG_TRACE_RESULT_VALUE_PREPARED)) {
        for (uint32_t index = 0; index < leaf_count; ++index) {
            spx_pg_dealloc(result->leaf_bytes[index], result->leaf_lens[index]);
        }
        if (result->leaf_bytes != NULL) {
            spx_pg_dealloc(result->leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result->leaf_lens != NULL) {
            spx_pg_dealloc(result->leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(result, sizeof(spx_pg_result_v1));
        return spx_pg_settle(SPX_PG_STATUS_CONTRACT_FAILURE);
    }

    spx_pg_trace_record(SPX_PG_TRACE_RESULT_COMMIT);
    if (spx_pg_should_inject(SPX_PG_TRACE_RESULT_COMMIT)) {
        for (uint32_t index = 0; index < leaf_count; ++index) {
            spx_pg_dealloc(result->leaf_bytes[index], result->leaf_lens[index]);
        }
        if (result->leaf_bytes != NULL) {
            spx_pg_dealloc(result->leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result->leaf_lens != NULL) {
            spx_pg_dealloc(result->leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(result, sizeof(spx_pg_result_v1));
        return spx_pg_settle(SPX_PG_STATUS_CONTRACT_FAILURE);
    }

    if (!spx_pg_registry_insert(result, SPX_PG_KIND_RESULT, provider, result->generation)) {
        for (uint32_t index = 0; index < leaf_count; ++index) {
            spx_pg_dealloc(result->leaf_bytes[index], result->leaf_lens[index]);
        }
        if (result->leaf_bytes != NULL) {
            spx_pg_dealloc(result->leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result->leaf_lens != NULL) {
            spx_pg_dealloc(result->leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(result, sizeof(spx_pg_result_v1));
        return spx_pg_settle(SPX_PG_STATUS_ALLOCATION_FAILURE);
    }

    /* A cleanup-failure injection during the non-result input release above
     * may already have settled a sticky, non-OK outcome even though every
     * other step here succeeded (see docs/PUBLIC-GENERIC-CARRIER-V1.md's
     * "a cleanup failure with no earlier failure may legally become the
     * terminal status"). When that happens, the result must never become
     * observable to the caller: settle first, and only publish `*out_result`
     * if the STICKY outcome is truly OK. */
    spx_pg_status_v1 final_status = spx_pg_settle(SPX_PG_STATUS_OK);
    if (final_status != SPX_PG_STATUS_OK) {
        size_t result_slot = spx_pg_registry_find(result, SPX_PG_KIND_RESULT);
        if (result_slot != SPX_PG_REGISTRY_CAPACITY) {
            spx_pg_registry_remove(result_slot);
        }
        for (uint32_t index = 0; index < leaf_count; ++index) {
            spx_pg_dealloc(result->leaf_bytes[index], result->leaf_lens[index]);
        }
        if (result->leaf_bytes != NULL) {
            spx_pg_dealloc(result->leaf_bytes, sizeof(uint8_t *) * leaf_count);
        }
        if (result->leaf_lens != NULL) {
            spx_pg_dealloc(result->leaf_lens, sizeof(size_t) * leaf_count);
        }
        spx_pg_dealloc(result, sizeof(spx_pg_result_v1));
        return final_status;
    }
    *out_result = result;
    return final_status;
}

/* --- Result export/release. --- */

spx_pg_status_v1 spx_pg_result_export_v1(spx_pg_result_v1 *result, uint8_t *out_bytes,
                                          size_t out_capacity, size_t *out_required) {
    if (out_required == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    *out_required = 0;
    size_t slot = spx_pg_registry_find(result, SPX_PG_KIND_RESULT);
    if (slot == SPX_PG_REGISTRY_CAPACITY) {
        return SPX_PG_STATUS_HANDLE_INVALID;
    }
    size_t required = 8;
    for (uint32_t index = 0; index < result->leaf_count; ++index) {
        required += 8 + result->leaf_lens[index];
    }
    *out_required = required;
    if (out_bytes == NULL || out_capacity < required) {
        return SPX_PG_STATUS_BUFFER_TOO_SMALL;
    }
    size_t offset = 0;
    spx_pg_write_u64le(out_bytes + offset, result->leaf_count);
    offset += 8;
    for (uint32_t index = 0; index < result->leaf_count; ++index) {
        spx_pg_write_u64le(out_bytes + offset, result->leaf_lens[index]);
        offset += 8;
        if (result->leaf_lens[index] != 0) {
            memcpy(out_bytes + offset, result->leaf_bytes[index], result->leaf_lens[index]);
        }
        offset += result->leaf_lens[index];
    }
    return SPX_PG_STATUS_OK;
}

spx_pg_status_v1 spx_pg_value_release_v1(spx_pg_value_v1 **value) {
    if (value == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    if (*value == NULL) {
        return SPX_PG_STATUS_OK;
    }
    size_t slot = spx_pg_registry_find(*value, SPX_PG_KIND_VALUE);
    if (slot == SPX_PG_REGISTRY_CAPACITY) {
        size_t wrong_kind = spx_pg_registry_find(*value, SPX_PG_KIND_RESULT);
        *value = NULL;
        return wrong_kind == SPX_PG_REGISTRY_CAPACITY ? SPX_PG_STATUS_HANDLE_INVALID
                                                       : SPX_PG_STATUS_ILLEGAL_TRANSITION;
    }
    spx_pg_value_v1 *owned = *value;
    spx_pg_registry_remove(slot);
    spx_pg_release_leaves(owned->leaf_bytes, owned->leaf_lens, owned->leaf_count);
    if (owned->leaf_bytes != NULL) {
        spx_pg_dealloc(owned->leaf_bytes, sizeof(uint8_t *) * owned->leaf_count);
    }
    if (owned->leaf_lens != NULL) {
        spx_pg_dealloc(owned->leaf_lens, sizeof(size_t) * owned->leaf_count);
    }
    spx_pg_dealloc(owned, sizeof(spx_pg_value_v1));
    *value = NULL;
    return SPX_PG_STATUS_OK;
}

spx_pg_status_v1 spx_pg_result_release_v1(spx_pg_result_v1 **result) {
    if (result == NULL) {
        return SPX_PG_STATUS_NULL_OR_WRONG_KIND;
    }
    if (*result == NULL) {
        return SPX_PG_STATUS_OK;
    }
    size_t slot = spx_pg_registry_find(*result, SPX_PG_KIND_RESULT);
    if (slot == SPX_PG_REGISTRY_CAPACITY) {
        size_t wrong_kind = spx_pg_registry_find(*result, SPX_PG_KIND_VALUE);
        *result = NULL;
        return wrong_kind == SPX_PG_REGISTRY_CAPACITY ? SPX_PG_STATUS_HANDLE_INVALID
                                                       : SPX_PG_STATUS_ILLEGAL_TRANSITION;
    }
    spx_pg_result_v1 *owned = *result;
    spx_pg_registry_remove(slot);
    for (uint32_t index = 0; index < owned->leaf_count; ++index) {
        spx_pg_dealloc(owned->leaf_bytes[index], owned->leaf_lens[index]);
    }
    if (owned->leaf_bytes != NULL) {
        spx_pg_dealloc(owned->leaf_bytes, sizeof(uint8_t *) * owned->leaf_count);
    }
    if (owned->leaf_lens != NULL) {
        spx_pg_dealloc(owned->leaf_lens, sizeof(size_t) * owned->leaf_count);
    }
    spx_pg_dealloc(owned, sizeof(spx_pg_result_v1));
    *result = NULL;
    return SPX_PG_STATUS_OK;
}
