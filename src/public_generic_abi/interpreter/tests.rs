//! Coverage for [`InterpreterProvider`] on its own: a success round trip
//! with byte-identical repeated export, capacity bounds, handle hostility,
//! the full failure-injection matrix (zero live resources after every
//! terminal case), and sticky-settlement overwrite counting. Cross-engine
//! comparison against [`crate::public_generic_abi::wasm::provider::WasmProvider`]
//! lives in `carrier::settlement_corpus`, not here.

use super::*;
use crate::public_generic_abi::carrier::CarrierBindingV1;

const DESCRIPTOR_FIXTURE: &[u8] = b"fixture-descriptor-bytes";

fn trusted_binding() -> CarrierBindingV1 {
    CarrierBindingV1::new(
        "sha256:descriptor-identity-fixture",
        TargetProfile::Interpreter,
        "sha256:runtime-identity-fixture",
    )
}

fn open_provider() -> InterpreterProvider {
    let trusted = trusted_binding();
    InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &trusted.encode(),
        &trusted,
    )
    .expect("a byte-identical replay must open")
}

#[test]
fn open_rejects_a_binding_naming_another_target_profile() {
    let wrong = CarrierBindingV1::new(
        "sha256:descriptor-identity-fixture",
        TargetProfile::NativeC11,
        "sha256:runtime-identity-fixture",
    );
    let error = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &wrong.encode(),
        &wrong,
    )
    .expect_err("a non-Interpreter target profile must be rejected");
    assert_eq!(error, InterpreterPgStatus::MalformedBinding);
}

#[test]
fn open_rejects_a_descriptor_replay_mismatch() {
    let trusted = trusted_binding();
    let error = InterpreterProvider::open(
        b"different-descriptor-bytes",
        DESCRIPTOR_FIXTURE,
        &trusted.encode(),
        &trusted,
    )
    .expect_err("a descriptor byte mismatch must be rejected");
    assert_eq!(error, InterpreterPgStatus::DescriptorReplayMismatch);
}

#[test]
fn open_rejects_a_binding_replay_mismatch() {
    let trusted = trusted_binding();
    let other = CarrierBindingV1::new(
        "sha256:different-descriptor-identity",
        TargetProfile::Interpreter,
        "sha256:runtime-identity-fixture",
    );
    let error = InterpreterProvider::open(
        DESCRIPTOR_FIXTURE,
        DESCRIPTOR_FIXTURE,
        &other.encode(),
        &trusted,
    )
    .expect_err("a cross-paired binding must not replay");
    assert_eq!(error, InterpreterPgStatus::BindingReplayMismatch);
}

#[test]
fn a_success_round_trip_settles_and_zeroes_every_resource() {
    let mut provider = open_provider();
    let value = provider
        .input_prepare(&[b"hello".to_vec()])
        .expect("preparing one small leaf must succeed");
    let result = provider.call(value).expect("the fixture call must succeed");

    // Two-pass export: capacity 0 reports the required size without
    // consuming anything, and repeating it is byte-identical.
    let required_err = provider
        .result_export(result, 0)
        .expect_err("capacity 0 must report BufferTooSmall, not silently truncate");
    assert_eq!(required_err, InterpreterPgStatus::BufferTooSmall);
    let first = provider
        .result_export(result, 4096)
        .expect("a large-enough capacity must export");
    let second = provider
        .result_export(result, 4096)
        .expect("repeating the export must be byte-identical");
    assert_eq!(first, second);

    assert_eq!(provider.result_release(result), InterpreterPgStatus::Ok);
    assert_eq!(provider.live_handles(), 0);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_bytes(), 0);
    assert_eq!(provider.close(), InterpreterPgStatus::Ok);
}

#[test]
fn zero_length_leaf_round_trips_with_no_null_ambiguity() {
    let mut provider = open_provider();
    let value = provider
        .input_prepare(&[Vec::new()])
        .expect("a zero-length leaf must be admitted");
    let result = provider.call(value).expect("the fixture call must succeed");
    let exported = provider
        .result_export(result, 4096)
        .expect("export must succeed");
    // One framed field, zero bytes long: exactly 8 length-prefix bytes, no
    // trailing content and no phantom allocation obligation.
    assert_eq!(exported.len(), 8);
    provider.result_release(result);
    assert_eq!(provider.live_handles(), 0);
}

#[test]
fn embedded_zero_bytes_are_preserved_exactly() {
    let mut provider = open_provider();
    let payload = vec![0u8, 1, 0, 2, 0, 3, 0];
    let value = provider
        .input_prepare(std::slice::from_ref(&payload))
        .expect("a payload with embedded zero bytes must be admitted");
    let result = provider.call(value).expect("the fixture call must succeed");
    let exported = provider.result_export(result, 4096).unwrap();
    let mut expected = Vec::new();
    let mut reversed = payload.clone();
    reversed.reverse();
    crate::public_generic_abi::frame(&mut expected, &reversed);
    assert_eq!(
        exported, expected,
        "no C-string truncation at an embedded zero byte"
    );
    provider.result_release(result);
}

#[test]
fn leaf_over_max_bytes_is_rejected_before_endpoint_execution() {
    let mut provider = open_provider();
    let oversized = vec![0u8; MAX_BYTES_PER_LEAF + 1];
    let error = provider
        .input_prepare(&[oversized])
        .expect_err("one byte over MAX_BYTES_PER_LEAF must be rejected");
    assert_eq!(error, InterpreterPgStatus::CarrierCapacity);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_handles(), 0);
}

#[test]
fn leaf_count_over_max_is_rejected_before_endpoint_execution() {
    let mut provider = open_provider();
    let leaves = vec![Vec::new(); MAX_OWNED_LEAVES_PER_INSTANCE + 1];
    let error = provider
        .input_prepare(&leaves)
        .expect_err("one leaf over MAX_OWNED_LEAVES_PER_INSTANCE must be rejected");
    assert_eq!(error, InterpreterPgStatus::CarrierCapacity);
    assert_eq!(provider.live_allocations(), 0);
}

/// Real aggregate-bound fixture shared in shape with the native and Wasm
/// adapter gates: 255 full leaves and a final leaf one byte short or full.
/// Each leaf carries a distinct pattern so successful copy, reverse, export,
/// and release cannot be satisfied by merely counting bytes.
fn aggregate_boundary_leaves(final_leaf_len: usize) -> Vec<Vec<u8>> {
    assert!(final_leaf_len <= MAX_BYTES_PER_LEAF);
    let leaves: Vec<Vec<u8>> = (0..MAX_OWNED_LEAVES_PER_INSTANCE)
        .map(|leaf| {
            let len = if leaf + 1 == MAX_OWNED_LEAVES_PER_INSTANCE {
                final_leaf_len
            } else {
                MAX_BYTES_PER_LEAF
            };
            (0..len)
                .map(|offset| {
                    (leaf as u8)
                        .wrapping_mul(17)
                        .wrapping_add((offset as u8).wrapping_mul(31))
                        .wrapping_add((offset >> 8) as u8)
                })
                .collect()
        })
        .collect();
    assert_eq!(
        leaves.iter().map(Vec::len).sum::<usize>(),
        MAX_TOTAL_PAYLOAD_BYTES - (MAX_BYTES_PER_LEAF - final_leaf_len)
    );
    leaves
}

#[test]
fn aggregate_payload_one_under_and_at_the_total_bound_round_trip_and_settle() {
    for final_leaf_len in [MAX_BYTES_PER_LEAF - 1, MAX_BYTES_PER_LEAF] {
        let leaves = aggregate_boundary_leaves(final_leaf_len);
        let expected: Vec<Vec<u8>> = leaves
            .iter()
            .map(|leaf| leaf.iter().rev().copied().collect())
            .collect();
        let total_payload = leaves.iter().map(Vec::len).sum::<usize>();
        assert_eq!(
            total_payload,
            MAX_TOTAL_PAYLOAD_BYTES - (MAX_BYTES_PER_LEAF - final_leaf_len)
        );

        let mut provider = open_provider();
        let value = provider
            .input_prepare(&leaves)
            .expect("a real aggregate at the total payload boundary must prepare");
        let result = provider
            .call(value)
            .expect("the reverse endpoint must accept the prepared aggregate");
        assert_eq!(
            provider.call(value),
            Err(InterpreterPgStatus::HandleInvalid),
            "the consumed input handle must stay rejected while the result remains live"
        );
        let exported = provider
            .result_export(result, total_payload + 8 * MAX_OWNED_LEAVES_PER_INSTANCE)
            .expect("the complete reversed aggregate must export");
        let mut offset = 0usize;
        for expected_leaf in &expected {
            let (actual, next) =
                crate::public_generic_abi::read_frame(&exported, offset, MAX_BYTES_PER_LEAF)
                    .expect("each exported leaf must retain its canonical frame");
            assert_eq!(actual, expected_leaf.as_slice());
            offset = next;
        }
        assert_eq!(offset, exported.len());
        assert_eq!(provider.result_release(result), InterpreterPgStatus::Ok);
        assert_eq!(provider.live_allocations(), 0);
        assert_eq!(provider.live_bytes(), 0);
        assert_eq!(provider.live_handles(), 0);
        assert_eq!(provider.close(), InterpreterPgStatus::Ok);
    }
}

#[test]
fn value_release_before_call_is_a_legal_abandon() {
    // Mirrors `WasmProvider`'s own
    // `value_release_before_call_is_a_legal_abandon` exactly: this adapter
    // had no direct test of its own for the plain legal-abandon path (only
    // the stale-prior-generation negative case below exercised
    // `value_release` at all), so the "consumer-side release" outcome
    // documented for both adapters was unverified for the interpreter on
    // its own.
    let mut provider = open_provider();
    let value = provider.input_prepare(&[b"abandoned".to_vec()]).unwrap();
    assert_eq!(provider.value_release(value), InterpreterPgStatus::Ok);
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_handles(), 0);
    assert_eq!(provider.live_bytes(), 0);
    assert_eq!(provider.close(), InterpreterPgStatus::Ok);
}

#[test]
fn double_release_of_a_result_handle_is_rejected() {
    // Mirrors `WasmProvider`'s own
    // `double_release_of_a_result_handle_is_rejected`: releasing a result
    // handle a second time (or releasing one that was never exported first)
    // must fail closed rather than double-freeing physical storage. This
    // adapter's own suite only ever released a result handle after a
    // successful export, so a release-without-export was never exercised
    // here on its own either.
    let mut provider = open_provider();
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let result = provider.call(value).unwrap();
    assert_eq!(provider.result_release(result), InterpreterPgStatus::Ok);
    assert_eq!(
        provider.result_release(result),
        InterpreterPgStatus::HandleInvalid
    );
}

#[test]
fn a_stale_handle_from_a_prior_provider_generation_is_rejected() {
    let mut first = open_provider();
    let value = first.input_prepare(&[b"x".to_vec()]).unwrap();
    let mut second = open_provider();
    let status = second.value_release(value);
    assert_eq!(
        status,
        InterpreterPgStatus::HandleInvalid,
        "a handle minted by one provider must not be honored by another, even sharing an (id, generation) pair"
    );
    // Release it against its real owner, the only provider that ever
    // registered it.
    first.value_release(value);
}

#[test]
fn every_non_terminal_trace_label_injection_zeroes_every_resource() {
    let labels = [
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
    for label in labels {
        let mut provider = open_provider();
        provider.test_inject_failure(label);
        // `input_prepare` itself may already have failed for the earliest
        // labels; either failure point is treated uniformly below.
        let value = provider
            .input_prepare(&[b"payload".to_vec()])
            .and_then(|value| provider.call(value));
        // Whichever call site failed, no live allocation or handle may
        // survive the terminal case.
        let _ = value;
        assert_eq!(
            provider.live_allocations(),
            0,
            "{label:?} must leave zero live allocations"
        );
        assert_eq!(
            provider.live_handles(),
            0,
            "{label:?} must leave zero live handles"
        );
        assert_eq!(
            provider.live_bytes(),
            0,
            "{label:?} must leave zero live bytes"
        );
    }
}

#[test]
fn cleanup_cannot_overwrite_an_earlier_sticky_failure() {
    let mut provider = open_provider();
    provider.test_inject_failure(TraceLabel::ExecutionStarted);
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let error = provider
        .call(value)
        .expect_err("execution-start injection must fail the call");
    assert_eq!(error, InterpreterPgStatus::ContractFailure);
    // No release-ordinal injection was armed for this call, so the
    // release-ordinal check `call` now runs right after this same
    // `ExecutionStarted` branch (see issue #162's compound case, exercised
    // directly by `cleanup_failure_is_rejected_and_counted_when_it_compounds_an_earlier_sticky_failure`
    // below) never finds anything armed and never attempts a settle.
    assert_eq!(provider.test_settlement_overwrite_attempts(), 0);

    provider.test_inject_failure(TraceLabel::LeafRelease);
    let value2 = provider.input_prepare(&[b"y".to_vec()]).unwrap();
    // This call succeeds logically (no earlier failure), so the cleanup
    // injection legally becomes the terminal status rather than being
    // discarded — matching `WasmProvider`'s own documented case.
    let error2 = provider
        .call(value2)
        .expect_err("a cleanup failure with no earlier failure becomes terminal");
    assert_eq!(error2, InterpreterPgStatus::ContractFailure);
    assert_eq!(provider.test_settlement_overwrite_attempts(), 0);
}

/// Issue #162's required "cleanup failure after an input/runtime failure"
/// case, at the single-engine level: unlike the test above (which arms
/// only `ExecutionStarted`, so the release-ordinal check never finds
/// anything armed), this one arms BOTH `ExecutionStarted` and
/// `LeafRelease` before the SAME call.
/// `InterpreterProvider::test_inject_failure`'s multi-injection support
/// (each call arms an independently consumable ordinal rather than
/// replacing the last one) is what makes this expressible at all — a
/// single `Option<TraceLabel>` slot could only ever hold one of the two.
/// `ExecutionStarted` settles `ContractFailure` first and physically
/// releases the input; `LeafRelease` is checked immediately afterward
/// (still in the same early-return branch) and must be REJECTED and
/// COUNTED, never applied, proving "cleanup cannot replace the selected
/// status" holds even when the cleanup failure is real, not merely absent.
#[test]
fn cleanup_failure_is_rejected_and_counted_when_it_compounds_an_earlier_sticky_failure() {
    let mut provider = open_provider();
    provider.test_inject_failure(TraceLabel::ExecutionStarted);
    provider.test_inject_failure(TraceLabel::LeafRelease);
    let value = provider.input_prepare(&[b"x".to_vec()]).unwrap();
    let error = provider
        .call(value)
        .expect_err("execution-start injection must fail the call");
    assert_eq!(
        error,
        InterpreterPgStatus::ContractFailure,
        "the earlier ExecutionStarted failure must remain the sticky status, not be replaced by \
         the compounding cleanup failure"
    );
    assert_eq!(
        provider.test_settlement_overwrite_attempts(),
        1,
        "the compounding cleanup injection must be rejected and counted exactly once"
    );
    assert_eq!(provider.live_allocations(), 0);
    assert_eq!(provider.live_handles(), 0);
    assert_eq!(provider.live_bytes(), 0);
}

#[test]
fn repeated_invocation_leaks_no_state_between_independent_calls() {
    let mut provider = open_provider();
    for iteration in 0..5u8 {
        let payload = vec![iteration; 3];
        let value = provider
            .input_prepare(std::slice::from_ref(&payload))
            .unwrap();
        let result = provider.call(value).unwrap();
        let exported = provider.result_export(result, 4096).unwrap();
        let mut expected = Vec::new();
        let mut reversed = payload;
        reversed.reverse();
        crate::public_generic_abi::frame(&mut expected, &reversed);
        assert_eq!(
            exported, expected,
            "iteration {iteration} must reproduce exactly"
        );
        provider.result_release(result);
        assert_eq!(
            provider.live_handles(),
            0,
            "iteration {iteration} must leave zero live handles"
        );
        assert_eq!(provider.live_allocations(), 0);
    }
    assert_eq!(provider.close(), InterpreterPgStatus::Ok);
}

#[test]
fn provider_recreation_rejects_stale_child_handles() {
    let mut first = open_provider();
    let value = first.input_prepare(&[b"a".to_vec()]).unwrap();
    let result = first.call(value).unwrap();
    // Do not release; drop `first` outright (simulating close-without-
    // teardown is out of scope) and open a fresh provider generation.
    assert_eq!(first.close(), InterpreterPgStatus::IllegalTransition);
    let mut second = open_provider();
    let status = second.result_release(result);
    assert_eq!(
        status,
        InterpreterPgStatus::HandleInvalid,
        "a result handle from a prior provider generation must never be honored by a new one"
    );
}
