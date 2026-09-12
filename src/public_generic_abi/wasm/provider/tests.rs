use super::*;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};

const FIXTURE_DESCRIPTOR_BYTES: &[u8] = b"fixture-public-generic-descriptor-bytes-issue-155";

fn fixture_binding() -> WasmProviderBindingV1 {
    WasmProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            TargetProfile::CoreWasm,
            "runtime:core-wasm-fixture-issue-155",
        ),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        FIXTURE_ENDPOINT_EXPORT_NAME,
        "semaprax-0.4.1",
    )
}

fn open() -> WasmProvider {
    let binding = fixture_binding();
    let binding_bytes = binding.encode();
    WasmProvider::open(
        FIXTURE_DESCRIPTOR_BYTES,
        FIXTURE_DESCRIPTOR_BYTES,
        &binding_bytes,
        &binding,
    )
    .unwrap()
}

fn decode_leaves(exported: &[u8]) -> Vec<Vec<u8>> {
    let mut leaves = Vec::new();
    let mut offset = 0usize;
    while offset < exported.len() {
        let (field, next) =
            crate::public_generic_abi::read_frame(exported, offset, MAX_BYTES_PER_LEAF).unwrap();
        leaves.push(field.to_vec());
        offset = next;
    }
    leaves
}

#[test]
fn open_rejects_wrong_descriptor_bytes() {
    let binding = fixture_binding();
    let binding_bytes = binding.encode();
    let error = WasmProvider::open(
        b"not-the-trusted-bytes",
        FIXTURE_DESCRIPTOR_BYTES,
        &binding_bytes,
        &binding,
    )
    .unwrap_err();
    assert_eq!(error, WasmPgStatus::DescriptorReplayMismatch);
}

#[test]
fn open_rejects_wrong_binding_bytes() {
    let binding = fixture_binding();
    let error = WasmProvider::open(
        FIXTURE_DESCRIPTOR_BYTES,
        FIXTURE_DESCRIPTOR_BYTES,
        b"not-the-trusted-binding",
        &binding,
    )
    .unwrap_err();
    assert_eq!(error, WasmPgStatus::MalformedBinding);
}

#[test]
fn open_rejects_binding_replay_mismatch() {
    let trusted = fixture_binding();
    let wrong = WasmProviderBindingV1::new(
        trusted.carrier_binding().clone(),
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        FIXTURE_ENDPOINT_EXPORT_NAME,
        "semaprax-0.4.1",
    );
    let error = WasmProvider::open(
        FIXTURE_DESCRIPTOR_BYTES,
        FIXTURE_DESCRIPTOR_BYTES,
        &wrong.encode(),
        &trusted,
    )
    .unwrap_err();
    assert_eq!(error, WasmPgStatus::BindingReplayMismatch);
}

#[test]
fn full_success_round_trip_reverses_every_leaf_and_settles_zero() {
    let mut provider = open();
    let leaves = vec![b"hello".to_vec(), b"world!".to_vec(), Vec::new()];
    let value = provider.input_prepare(&leaves).unwrap();
    let result = provider.call(value).unwrap();

    let required = provider.result_export(result, 0).unwrap_err();
    assert_eq!(required, WasmPgStatus::BufferTooSmall);
    let exported = provider.result_export(result, 4096).unwrap();
    // Nonconsuming: a second export at the same capacity is byte-identical.
    assert_eq!(provider.result_export(result, 4096).unwrap(), exported);

    let decoded = decode_leaves(&exported);
    assert_eq!(
        decoded,
        vec![b"olleh".to_vec(), b"!dlrow".to_vec(), Vec::new()]
    );

    assert_eq!(provider.result_release(result), WasmPgStatus::Ok);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_bytes(), 0);
    assert_eq!(provider.live_handles(), 0);
    assert_eq!(provider.close(), WasmPgStatus::Ok);
}

#[test]
fn exact_leaf_count_bound_succeeds_and_first_over_bound_is_rejected() {
    let mut provider = open();
    let leaves: Vec<Vec<u8>> = (0..MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|_| Vec::new())
        .collect();
    let value = provider.input_prepare(&leaves).unwrap();
    provider.value_release(value);
    assert_eq!(provider.live_allocations(), 0);

    let over_bound: Vec<Vec<u8>> = (0..=MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|_| Vec::new())
        .collect();
    let error = provider.input_prepare(&over_bound).unwrap_err();
    assert_eq!(error, WasmPgStatus::CarrierCapacity);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_handles(), 0);
}

#[test]
fn exact_leaf_byte_bound_succeeds_and_first_over_bound_is_rejected() {
    let mut provider = open();
    let exact = vec![vec![0u8; MAX_BYTES_PER_LEAF]];
    let value = provider.input_prepare(&exact).unwrap();
    provider.value_release(value);
    assert_eq!(provider.live_allocations(), 0);

    let over_bound = vec![vec![0u8; MAX_BYTES_PER_LEAF + 1]];
    let error = provider.input_prepare(&over_bound).unwrap_err();
    assert_eq!(error, WasmPgStatus::CarrierCapacity);
    assert_eq!(provider.live_allocations(), 0);
}

#[test]
fn value_release_before_call_is_a_legal_abandon() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"abandoned".to_vec()]).unwrap();
    assert_eq!(provider.value_release(value), WasmPgStatus::Ok);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_handles(), 0);
    assert_eq!(provider.close(), WasmPgStatus::Ok);
}

#[test]
fn value_handle_is_stale_after_call_consumes_it() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    // `call` already removed the input handle from the registry — using it
    // again fails closed rather than double-consuming.
    let error = provider.call(value).unwrap_err();
    assert_eq!(error, WasmPgStatus::HandleInvalid);
    assert_eq!(provider.value_release(value), WasmPgStatus::HandleInvalid);
    provider.result_release(result);
}

#[test]
fn double_release_of_a_result_handle_is_rejected() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    assert_eq!(provider.result_release(result), WasmPgStatus::Ok);
    assert_eq!(provider.result_release(result), WasmPgStatus::HandleInvalid);
}

#[test]
fn wrong_kind_handle_is_rejected_both_directions() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    // A value handle presented where a result handle is required.
    let coerced_as_result = WasmHandle {
        tag: value.tag,
        handle: value.handle,
    };
    let error = provider.result_export(coerced_as_result, 4096).unwrap_err();
    assert_eq!(error, WasmPgStatus::NullOrWrongKind);
    let result = provider.call(value).unwrap();
    // A result handle presented where a value handle is required.
    let coerced_as_value = WasmHandle {
        tag: result.tag,
        handle: result.handle,
    };
    let error = provider.call(coerced_as_value).unwrap_err();
    // Live in the registry, just under the wrong role.
    assert_eq!(error, WasmPgStatus::NullOrWrongKind);
    provider.result_release(result);
}

#[test]
fn cross_provider_handle_is_rejected_even_on_a_colliding_id_and_generation() {
    let mut provider_a = open();
    let mut provider_b = open();
    let value_a = provider_a.input_prepare(&[b"a".to_vec()]).unwrap();
    let value_b = provider_b.input_prepare(&[b"b".to_vec()]).unwrap();
    // Both providers independently minted the same (id, generation) pair —
    // the physical proof that provider identity, not just the handle
    // value, gates every use.
    assert_eq!(value_a.handle, value_b.handle);
    let error = provider_b.call(value_a).unwrap_err();
    assert_eq!(error, WasmPgStatus::HandleInvalid);
    provider_a.value_release(value_a);
    provider_b.value_release(value_b);
}

#[test]
fn stale_generation_handle_is_rejected() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    // A handle naming a prior call's generation, presented against a
    // freshly opened call's provider-wide registry.
    let forged = WasmHandle {
        tag: result.tag,
        handle: Handle::root(result.handle.generation.wrapping_add(1)),
    };
    let error = provider.result_export(forged, 4096).unwrap_err();
    assert_eq!(error, WasmPgStatus::HandleInvalid);
    provider.result_release(result);
}

#[test]
fn malformed_carrier_over_capacity_never_allocates() {
    let mut provider = open();
    let error = provider
        .input_prepare(&vec![vec![0u8; MAX_BYTES_PER_LEAF + 1]; 1])
        .unwrap_err();
    assert_eq!(error, WasmPgStatus::CarrierCapacity);
    assert_eq!(provider.live_bytes(), 0);
    assert_eq!(provider.live_allocations(), 0);
}

#[test]
fn buffer_too_small_reports_exact_required_length_without_consuming() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"abcdef".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    let full = provider.result_export(result, usize::MAX).unwrap();
    let error = provider.result_export(result, full.len() - 1).unwrap_err();
    assert_eq!(error, WasmPgStatus::BufferTooSmall);
    // Still exportable afterward — the short call did not consume it.
    assert_eq!(provider.result_export(result, full.len()).unwrap(), full);
    provider.result_release(result);
}

#[test]
fn sticky_failure_survives_a_later_cleanup_failure_attempt() {
    let mut provider = open();
    provider.test_inject_failure(TraceLabel::ExecutionStarted);
    // Also arm the release-ordinal cleanup path in the same call: the
    // execution-stage failure has already selected `ContractFailure` by
    // the time input release happens, so this second injection must be
    // discarded, not override it. `test_inject_failure`'s multi-injection
    // support (issue #162: each call arms an independently consumable
    // ordinal rather than replacing the last one) is what makes this
    // actually expressible — a single `Option<TraceLabel>` slot could only
    // ever hold one of the two, which is why this second call used to be
    // absent here despite this comment already describing the intent.
    provider.test_inject_failure(TraceLabel::LeafRelease);
    let value = provider
        .input_prepare(&[b"x".to_vec(), b"y".to_vec()])
        .unwrap();
    let error = provider.call(value);
    assert_eq!(error, Err(WasmPgStatus::ContractFailure));
    assert_eq!(
        provider.test_settlement_overwrite_attempts(),
        1,
        "the compounding cleanup injection must be rejected and counted exactly once"
    );
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_bytes(), 0);
    assert_eq!(provider.live_handles(), 0);
}

#[test]
fn cleanup_failure_becomes_terminal_when_nothing_failed_earlier() {
    let mut provider = open();
    provider.test_inject_failure(TraceLabel::LeafRelease);
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let error = provider.call(value).unwrap_err();
    assert_eq!(error, WasmPgStatus::ContractFailure);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_bytes(), 0);
}

/// The full 0-13 failure-injection matrix: every non-terminal
/// [`TraceLabel`] ordinal is injected once, in its own fresh provider and
/// call, asserting the call fails and every live allocation/handle counter
/// returns to exactly zero afterward. Matches
/// [`crate::public_generic_abi::native`]'s own required "failure at every
/// logical injection point" evidence, restated for this physical adapter.
#[test]
fn failure_injection_covers_every_non_terminal_trace_ordinal_with_zero_live_state() {
    let injectable = [
        TraceLabel::FrameValidated,
        TraceLabel::LeafAllocationStarted,
        TraceLabel::LeafAllocationCommitted,
        TraceLabel::LeafPayloadCopied,
        TraceLabel::InputValuePrepared,
        TraceLabel::InputTransferCommitted,
        TraceLabel::ExecutionStarted,
        TraceLabel::ExecutionFinished,
        TraceLabel::ResultLeafAllocationStarted,
        TraceLabel::ResultLeafAllocationCommitted,
        TraceLabel::ResultValuePrepared,
        TraceLabel::ResultCommit,
        TraceLabel::LeafRelease,
        TraceLabel::CarrierRelease,
    ];
    assert_eq!(injectable.len(), 14);
    for label in injectable {
        let mut provider = open();
        provider.test_inject_failure(label);
        // Five of the fourteen ordinals (`FrameValidated` through
        // `InputValuePrepared`) fail during `input_prepare` itself; the
        // rest fail during `call`. Either way the call as a whole must
        // fail and settle to zero live state.
        let leaves = vec![b"leaf-one".to_vec(), b"leaf-two".to_vec()];
        let failed = match provider.input_prepare(&leaves) {
            Ok(value) => provider.call(value).is_err(),
            Err(_) => true,
        };
        assert!(failed, "expected failure injecting {label:?}");
        assert_eq!(
            provider.live_allocations(),
            0,
            "live allocations after injecting {label:?}"
        );
        assert_eq!(
            provider.live_bytes(),
            0,
            "live bytes after injecting {label:?}"
        );
        assert_eq!(
            provider.live_handles(),
            0,
            "live handles after injecting {label:?}"
        );
        assert_eq!(provider.close(), WasmPgStatus::Ok);
    }
}

#[test]
fn close_refuses_with_a_live_handle() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    // One root handle plus one leaf handle for the result.
    assert_eq!(provider.live_handles(), 2);
    // Closing with the result handle still live is refused.
    assert_eq!(provider.close(), WasmPgStatus::IllegalTransition);
    let _ = result;
}

#[test]
fn close_succeeds_once_every_handle_is_released() {
    let mut provider = open();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    provider.result_release(result);
    assert_eq!(provider.close(), WasmPgStatus::Ok);
}

#[test]
fn zero_leaf_call_round_trips() {
    let mut provider = open();
    let value = provider.input_prepare(&[]).unwrap();
    let result = provider.call(value).unwrap();
    let exported = provider.result_export(result, 4096).unwrap();
    assert!(exported.is_empty());
    provider.result_release(result);
    assert_eq!(provider.live_allocations(), 0);
}

/// Issue #135 (SPX-AI-036) closes the residual "large valid boundary" and
/// "returned byte values remain correct after ... memory growth" acceptance
/// items #155 left as narrower single-page fixtures. Three leaves at the
/// exact per-leaf bound (`MAX_BYTES_PER_LEAF`, one Wasm page each) push the
/// arena past a single `memory.grow` — `WasmLinearMemory` never shrinks, so
/// a real multi-page growth genuinely happened rather than being coincidence
/// — and each leaf carries a distinct, non-zero byte pattern so an
/// offset/aliasing bug would surface as wrong bytes, not as a fresh
/// zero-fill happening to match.
#[test]
fn large_multi_leaf_payload_forces_multi_page_growth_and_round_trips_byte_exact() {
    let mut provider = open();
    assert_eq!(provider.test_memory_pages(), 0);

    let leaves: Vec<Vec<u8>> = (0u8..3)
        .map(|pattern| vec![pattern.wrapping_mul(37).wrapping_add(1); MAX_BYTES_PER_LEAF])
        .collect();
    let expected: Vec<Vec<u8>> = leaves
        .iter()
        .map(|leaf| leaf.iter().rev().copied().collect())
        .collect();
    let total_bytes = (leaves.len() * MAX_BYTES_PER_LEAF) as u32;
    let expected_min_pages =
        total_bytes.div_ceil(crate::public_generic_abi::wasm::memory::PAGE_BYTES);
    assert!(
        expected_min_pages > 1,
        "fixture must span more than one page"
    );

    let value = provider.input_prepare(&leaves).unwrap();
    let pages_after_input = provider.test_memory_pages();
    assert!(
        pages_after_input >= expected_min_pages,
        "expected at least {expected_min_pages} pages after a {total_bytes}-byte input, got {pages_after_input}"
    );

    let result = provider.call(value).unwrap();
    // The arena never shrinks, so the input's freed span is still there for
    // the equally sized result to reuse: growth for the result allocation
    // must be a no-op, not a second independent grow.
    assert_eq!(
        provider.test_memory_pages(),
        pages_after_input,
        "result allocation of the same total size should reuse the already-grown arena"
    );

    let exported = provider
        .result_export(result, total_bytes as usize * 2)
        .unwrap();
    assert_eq!(decode_leaves(&exported), expected);

    assert_eq!(provider.result_release(result), WasmPgStatus::Ok);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_bytes(), 0);
    assert_eq!(provider.live_handles(), 0);
    assert_eq!(provider.close(), WasmPgStatus::Ok);
}

/// Two providers opened from the same trusted fixture binding are still two
/// fully independent carrier instances: interleaving their calls (rather
/// than finishing one before starting the other) proves neither's
/// registry, allocator, or generation counter is shared, matching this
/// issue's "simultaneous independent instances" requirement and the
/// "one module instance must not accept a handle or allocator state from
/// another, even when its descriptor matches" bounded-scope note.
#[test]
fn two_independent_providers_operate_simultaneously_without_cross_contamination() {
    let mut provider_a = open();
    let mut provider_b = open();

    let leaves_a = vec![b"alpha".to_vec()];
    let leaves_b = vec![b"bravo".to_vec(), b"charlie".to_vec()];
    let expected_a: Vec<Vec<u8>> = leaves_a
        .iter()
        .map(|leaf| leaf.iter().rev().copied().collect())
        .collect();
    let expected_b: Vec<Vec<u8>> = leaves_b
        .iter()
        .map(|leaf| leaf.iter().rev().copied().collect())
        .collect();

    // Interleaved, not sequential: both instances are live at once before
    // either call runs.
    let value_a = provider_a.input_prepare(&leaves_a).unwrap();
    let value_b = provider_b.input_prepare(&leaves_b).unwrap();

    // A handle minted by `a` is simply foreign to `b`'s registry, even
    // though both providers independently minted the identical
    // `(id, generation)` pair for their own first call.
    assert_eq!(
        provider_b.value_release(value_a),
        WasmPgStatus::HandleInvalid
    );
    assert_eq!(
        provider_a.value_release(value_b),
        WasmPgStatus::HandleInvalid
    );
    // Root + leaf handles: one root and one leaf for `a`, one root and two
    // leaves for `b`.
    assert_eq!(provider_a.live_handles(), 2);
    assert_eq!(provider_b.live_handles(), 3);

    let result_a = provider_a.call(value_a).unwrap();
    let result_b = provider_b.call(value_b).unwrap();

    assert_eq!(
        decode_leaves(&provider_a.result_export(result_a, 4096).unwrap()),
        expected_a
    );
    assert_eq!(
        decode_leaves(&provider_b.result_export(result_b, 4096).unwrap()),
        expected_b
    );

    assert_eq!(provider_a.result_release(result_a), WasmPgStatus::Ok);
    assert_eq!(provider_b.result_release(result_b), WasmPgStatus::Ok);
    assert_eq!(provider_a.live_allocations(), 0);
    assert_eq!(provider_b.live_allocations(), 0);
    assert_eq!(provider_a.live_handles(), 0);
    assert_eq!(provider_b.live_handles(), 0);
    assert_eq!(provider_a.close(), WasmPgStatus::Ok);
    assert_eq!(provider_b.close(), WasmPgStatus::Ok);
}

/// Negative control (ownership errors are diagnostics, never backend
/// accidents): a `CarrierBindingV1` built for `TargetProfile::NativeC11` -
/// a target this adapter never admits - wrapped in an otherwise
/// well-formed `WasmProviderBindingV1` and presented to `WasmProvider::open`
/// must fail closed with a normalized diagnostic status. `catch_unwind`
/// makes "not a panic" a literal, checked assertion rather than an implicit
/// side effect of the test merely not crashing, and the `Err` return proves
/// no half-constructed `WasmProvider` (a "malformed module") ever escapes.
#[test]
fn open_rejects_a_binding_naming_a_foreign_target_profile_as_a_diagnostic_not_a_panic() {
    let foreign = WasmProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            TargetProfile::NativeC11,
            "runtime:core-wasm-fixture-issue-155",
        ),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        FIXTURE_ENDPOINT_EXPORT_NAME,
        "semaprax-0.4.1",
    );
    let foreign_bytes = foreign.encode();

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        WasmProvider::open(
            FIXTURE_DESCRIPTOR_BYTES,
            FIXTURE_DESCRIPTOR_BYTES,
            &foreign_bytes,
            &foreign,
        )
    }))
    .expect("open must not panic on a binding naming a foreign target profile");
    assert_eq!(outcome.unwrap_err(), WasmPgStatus::MalformedBinding);
}
