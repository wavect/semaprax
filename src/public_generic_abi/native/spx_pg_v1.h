/* Native C11 physical adapter ABI for Public Generic Carrier v1 (issue #154).
 * PHYSICAL layer only: this header names opaque handles and operations, never
 * an internal aggregate layout. See ../../../docs/PUBLIC-GENERIC-CARRIER-V1.md
 * ("Native C11 physical adapter") for the owning specification. Generated
 * verbatim into every emitted provider translation unit; never hand-edited
 * per provider.
 */
#ifndef SPX_PG_V1_H
#define SPX_PG_V1_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* All public integer widths are fixed. Every byte slice is (pointer, length).
 */
typedef int32_t spx_pg_status_v1;

/* Opaque, incomplete on purpose: no public field, no copy constructor. A
 * foreign caller may only hold a pointer and pass it back to this ABI. */
typedef struct spx_pg_provider_v1 spx_pg_provider_v1;
typedef struct spx_pg_value_v1 spx_pg_value_v1;
typedef struct spx_pg_result_v1 spx_pg_result_v1;

/* Closed, normalized status vocabulary. Every failure this adapter can
 * report maps to exactly one of these; new reasons require a new constant
 * here, never an ad hoc negative number. Each constant cites the SPX-PG
 * diagnostic identity it restates for a machine-consumed caller; see the
 * owning specification's status table for the full mapping and precedence
 * order. */
#define SPX_PG_STATUS_OK 0
/* SPX-PG701: malformed descriptor bytes (framing/schema). */
#define SPX_PG_STATUS_MALFORMED_DESCRIPTOR 1
/* SPX-PG703: descriptor bytes do not replay against the trusted value this
 * provider was generated from. */
#define SPX_PG_STATUS_DESCRIPTOR_REPLAY_MISMATCH 2
/* SPX-PG901: malformed native provider binding bytes. */
#define SPX_PG_STATUS_MALFORMED_BINDING 3
/* SPX-PG902: provider binding bytes do not replay against the trusted value,
 * or name a different descriptor/target/provider artifact/endpoint than the
 * one this provider was generated for. */
#define SPX_PG_STATUS_BINDING_REPLAY_MISMATCH 4
/* SPX-PG801: malformed input/result carrier bytes (framing, leaf count). */
#define SPX_PG_STATUS_MALFORMED_CARRIER 5
/* SPX-PG802: a carrier bound was reached (leaf count or byte total). */
#define SPX_PG_STATUS_CARRIER_CAPACITY 6
/* SPX-PG804: an illegal handle-lifecycle operation: release after transfer,
 * double release, close with a live handle, result read before commit, a
 * value handle presented where a result handle is required or vice versa. */
#define SPX_PG_STATUS_ILLEGAL_TRANSITION 7
/* SPX-PG805: the handle pointer is not live in this provider's registry:
 * foreign, stale, already consumed/released, or forged. */
#define SPX_PG_STATUS_HANDLE_INVALID 8
/* SPX-PG806: internal invariant guard; a later settlement attempt tried to
 * replace an already-selected outcome. Never expected to reach a caller in
 * v1 because the adapter itself never issues a second, different outcome,
 * but the status exists so the vocabulary matches the logical carrier
 * exactly and a future adapter defect fails closed with an identified code
 * rather than a silent overwrite. */
#define SPX_PG_STATUS_STICKY_SETTLEMENT_VIOLATION 9
/* SPX-PG903: the adapter's bounded allocator could not satisfy a leaf
 * allocation. */
#define SPX_PG_STATUS_ALLOCATION_FAILURE 10
/* SPX-PG904: the checked endpoint itself refused or failed. */
#define SPX_PG_STATUS_CONTRACT_FAILURE 11
/* Not a failure: `out_capacity` was smaller than the exact required byte
 * count. `*out_required` is always set; no result byte was written and the
 * result is not consumed. */
#define SPX_PG_STATUS_BUFFER_TOO_SMALL 12
/* SPX-PG905: a required pointer argument was null, or a handle argument was
 * the wrong kind (value passed where result is required or vice versa),
 * independent of the handle's own lifecycle state. */
#define SPX_PG_STATUS_NULL_OR_WRONG_KIND 13

/* Provider open binds one exact provider artifact to one verified descriptor
 * and provider-binding record; see NativeProviderBindingV1 in
 * ../binding.rs. `descriptor_bytes`/`provider_binding_bytes` must
 * byte-exactly replay the values this provider was generated from — this is
 * the trusted descriptor verification step, already performed once,
 * independently, by the compiler at generation time (Public Generic
 * Descriptor v1's verifier); the runtime check here is the replay half of
 * that same trust chain, never a re-derivation from raw claims.
 * `*out_provider` is null on any non-OK status. */
spx_pg_status_v1 spx_pg_provider_open_v1(const uint8_t *descriptor_bytes,
                                          size_t descriptor_len,
                                          const uint8_t *provider_binding_bytes,
                                          size_t provider_binding_len,
                                          spx_pg_provider_v1 **out_provider);

/* Fully validates `carrier_bytes` (framing, leaf count and byte bounds)
 * before any allocation, then allocates and copies every owned leaf through
 * the adapter's bounded allocator. `*out_input` is null on any non-OK
 * status, and nothing is left allocated on failure. */
spx_pg_status_v1 spx_pg_input_prepare_v1(spx_pg_provider_v1 *provider,
                                          const uint8_t *carrier_bytes,
                                          size_t carrier_len,
                                          spx_pg_value_v1 **out_input);

/* Commits input transfer exactly once (invalidating `input` for the caller
 * even on failure — see "Transfer invalidates caller ownership exactly
 * once"), invokes the bound endpoint, and stages the whole result privately.
 * `*out_result` is null on any non-OK status. `input` must never be passed
 * to `spx_pg_value_release_v1` after this call, succeeding or not: ownership
 * already transferred. */
spx_pg_status_v1 spx_pg_call_v1(spx_pg_provider_v1 *provider,
                                 spx_pg_value_v1 *input,
                                 spx_pg_result_v1 **out_result);

/* Two-pass, nonconsuming export. Call once with `out_bytes == NULL` (or
 * `out_capacity == 0`) to learn `*out_required`; the result is not consumed
 * and no byte is written. Call again with a buffer of at least
 * `*out_required` bytes to receive canonical result carrier bytes,
 * byte-identical on every repeated call. Release remains mandatory and
 * separate. */
spx_pg_status_v1 spx_pg_result_export_v1(spx_pg_result_v1 *result,
                                          uint8_t *out_bytes,
                                          size_t out_capacity,
                                          size_t *out_required);

/* Releases exactly once. `*value`/`*result` is set to NULL after a
 * successful release, or after a rejected double/foreign/stale release, so
 * caller-side reuse is observable either way. Null-pointee is accepted and a
 * no-op success, matching "release null" from the required lifecycle
 * tests. */
spx_pg_status_v1 spx_pg_value_release_v1(spx_pg_value_v1 **value);
spx_pg_status_v1 spx_pg_result_release_v1(spx_pg_result_v1 **result);

/* Closing a provider with a live input or result handle is refused
 * (SPX_PG_STATUS_ILLEGAL_TRANSITION); release every handle first. */
spx_pg_status_v1 spx_pg_provider_close_v1(spx_pg_provider_v1 **provider);

/* --- Test-only surface: not part of the production ABI, never emitted for
 * a support/publication claim. Deterministic failure injection mapped to
 * the logical trace ordinals below, plus live allocation/handle counters
 * for exact-settlement assertions. Single-threaded only: no concurrency
 * claim is made anywhere in this adapter. */

/* The closed, normalized trace vocabulary, in the exact order
 * docs/PUBLIC-GENERIC-CARRIER-V1.md#the-normalized-trace lists it and the
 * exact order carrier::trace::TraceLabel declares its variants. Failure
 * injection ordinals below are these same positions: injecting at ordinal N
 * makes the adapter fail immediately after recording trace event N, before
 * that event's work completes. */
#define SPX_PG_TRACE_FRAME_VALIDATED 0
#define SPX_PG_TRACE_LEAF_ALLOCATION_STARTED 1
#define SPX_PG_TRACE_LEAF_ALLOCATION_COMMITTED 2
#define SPX_PG_TRACE_LEAF_PAYLOAD_COPIED 3
#define SPX_PG_TRACE_INPUT_VALUE_PREPARED 4
#define SPX_PG_TRACE_INPUT_TRANSFER_COMMITTED 5
#define SPX_PG_TRACE_EXECUTION_STARTED 6
#define SPX_PG_TRACE_EXECUTION_FINISHED 7
#define SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_STARTED 8
#define SPX_PG_TRACE_RESULT_LEAF_ALLOCATION_COMMITTED 9
#define SPX_PG_TRACE_RESULT_VALUE_PREPARED 10
#define SPX_PG_TRACE_RESULT_COMMIT 11
#define SPX_PG_TRACE_LEAF_RELEASE 12
#define SPX_PG_TRACE_CARRIER_RELEASE 13
#define SPX_PG_TRACE_TERMINAL_STATUS 14
#define SPX_PG_TRACE_LABEL_COUNT 15
#define SPX_PG_TEST_NO_INJECTION UINT32_MAX

/* Arm deterministic failure injection at trace ordinal `ordinal` (one of the
 * SPX_PG_TRACE_* constants above) for the NEXT `spx_pg_input_prepare_v1` /
 * `spx_pg_call_v1` / `spx_pg_result_export_v1` sequence only; injection
 * disarms itself once it fires, or on `spx_pg_test_clear_failure_injection_v1`.
 * Every ordinal 0..13 makes the adapter fail immediately after recording
 * that event, rolling back every allocation already made so far — including
 * the two release ordinals (`SPX_PG_TRACE_LEAF_RELEASE`,
 * `SPX_PG_TRACE_CARRIER_RELEASE`), where the physical free still happens
 * unconditionally and injection instead simulates "something else failed
 * during discharge," settling accordingly. Ordinal 14
 * (`SPX_PG_TRACE_TERMINAL_STATUS`) is the settlement call itself, which has
 * nothing left to abort; use `spx_pg_test_force_settlement_conflict_v1`
 * instead to test an attempt to override an already-selected outcome at
 * exactly that point. */
void spx_pg_test_inject_failure_v1(uint32_t ordinal);
void spx_pg_test_clear_failure_injection_v1(void);

/* Live allocation/handle counters. Zero after every terminal case (success
 * or failure) once every handle the caller minted has been released.
 * `spx_pg_test_live_handles_v1` counts only handles owned by `provider`. */
size_t spx_pg_test_live_allocations_v1(void);
size_t spx_pg_test_live_handles_v1(spx_pg_provider_v1 *provider);

/* The append-only normalized trace recorded since the provider opened.
 * `spx_pg_test_trace_len_v1` is the event count; `spx_pg_test_trace_label_v1`
 * returns the ordinal-th event's label as one of the SPX_PG_TRACE_*
 * constants, matching carrier::trace::TraceLabel's own vocabulary exactly. */
size_t spx_pg_test_trace_len_v1(void);
uint32_t spx_pg_test_trace_label_v1(size_t index);

/* Count of settlement attempts that tried to replace an already-selected,
 * different terminal outcome. Sticky failure held whenever this reads > 0
 * yet the call's returned status still names the FIRST selected outcome. */
size_t spx_pg_test_settlement_overwrite_attempts_v1(void);

/* Directly attempt to settle the call currently in progress (or most
 * recently finished) at `status`, exactly like an internal cleanup-failure
 * attempt would. Returns the STICKY status: if a different outcome was
 * already selected, `status` is discarded, `spx_pg_test_settlement_overwrite_attempts_v1`
 * increments, and the original outcome is returned unchanged — this is the
 * direct, isolated proof that "cleanup cannot replace the selected status."
 */
spx_pg_status_v1 spx_pg_test_force_settlement_conflict_v1(spx_pg_status_v1 status);

#ifdef __cplusplus
}
#endif

#endif /* SPX_PG_V1_H */
