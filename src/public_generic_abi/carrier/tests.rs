//! Coverage for the Public Generic Carrier v1 reference implementation:
//! every legal and illegal handle transition, a full phase-ledger run,
//! sticky settlement, exact-reverse release order, wrong-generation
//! handles, and the carrier-binding codec's determinism and hostile-input
//! handling.

use super::*;

// ---------------------------------------------------------------------
// The logical value state machine
// ---------------------------------------------------------------------

#[test]
fn every_legal_transition_succeeds() {
    let legal = [
        (
            CarrierState::Created,
            Event::Fill,
            CarrierState::Initialized,
        ),
        (
            CarrierState::Initialized,
            Event::Commit,
            CarrierState::Transferred,
        ),
        (
            CarrierState::Transferred,
            Event::BeginLoan,
            CarrierState::Borrowed,
        ),
        (
            CarrierState::Borrowed,
            Event::EndLoan,
            CarrierState::Transferred,
        ),
        (
            CarrierState::Transferred,
            Event::Consume,
            CarrierState::Consumed,
        ),
        (
            CarrierState::Consumed,
            Event::Discharge,
            CarrierState::Released,
        ),
        (
            CarrierState::Created,
            Event::ReleaseBeforeTransfer,
            CarrierState::Released,
        ),
        (
            CarrierState::Initialized,
            Event::ReleaseBeforeTransfer,
            CarrierState::Released,
        ),
        (
            CarrierState::Transferred,
            Event::ReleaseAfterTransfer,
            CarrierState::Released,
        ),
    ];
    for (from, event, to) in legal {
        assert_eq!(
            transition(from, event).unwrap_or_else(|error| {
                panic!("{from:?} -({event:?})-> {to:?} must be legal, got {error:?}")
            }),
            to
        );
    }
}

#[test]
fn every_other_transition_is_illegal_and_lands_on_invalid() {
    let states = [
        CarrierState::Created,
        CarrierState::Initialized,
        CarrierState::Transferred,
        CarrierState::Borrowed,
        CarrierState::Consumed,
        CarrierState::Released,
        CarrierState::Invalid,
    ];
    let events = [
        Event::Fill,
        Event::Commit,
        Event::BeginLoan,
        Event::EndLoan,
        Event::Consume,
        Event::Discharge,
        Event::ReleaseBeforeTransfer,
        Event::ReleaseAfterTransfer,
    ];
    let legal: std::collections::HashSet<(CarrierState, Event)> = [
        (CarrierState::Created, Event::Fill),
        (CarrierState::Initialized, Event::Commit),
        (CarrierState::Transferred, Event::BeginLoan),
        (CarrierState::Borrowed, Event::EndLoan),
        (CarrierState::Transferred, Event::Consume),
        (CarrierState::Consumed, Event::Discharge),
        (CarrierState::Created, Event::ReleaseBeforeTransfer),
        (CarrierState::Initialized, Event::ReleaseBeforeTransfer),
        (CarrierState::Transferred, Event::ReleaseAfterTransfer),
    ]
    .into_iter()
    .collect();

    let mut illegal_count = 0;
    for state in states {
        for event in events {
            if legal.contains(&(state, event)) {
                continue;
            }
            illegal_count += 1;
            let mut ledger = HandleLedger::new(Handle::root(1));
            // Force the ledger into the state under test by direct
            // construction rather than replaying a legal path, since some
            // states (Borrowed, Consumed, Released, Invalid) require one.
            ledger.state = state;
            let error = ledger
                .apply(event)
                .expect_err(&format!("{state:?} -({event:?})-> must be illegal"));
            assert_eq!(error.code, ILLEGAL_TRANSITION);
            assert_eq!(
                ledger.state(),
                CarrierState::Invalid,
                "an illegal event latches the ledger to Invalid"
            );
            // Invalid is absorbing: applying any further event stays illegal.
            let error = ledger
                .apply(Event::Fill)
                .expect_err("Invalid has no legal outgoing transition");
            assert_eq!(error.code, ILLEGAL_TRANSITION);
        }
    }
    // Sanity: this really did exercise a large illegal surface, not zero
    // cases that happened to all be legal.
    assert!(
        illegal_count >= 40,
        "expected a broad illegal surface, got {illegal_count}"
    );
}

#[test]
fn invalid_has_no_legal_outgoing_transition() {
    for event in [
        Event::Fill,
        Event::Commit,
        Event::BeginLoan,
        Event::EndLoan,
        Event::Consume,
        Event::Discharge,
        Event::ReleaseBeforeTransfer,
        Event::ReleaseAfterTransfer,
    ] {
        assert!(transition(CarrierState::Invalid, event).is_err());
    }
}

// ---------------------------------------------------------------------
// The call phase ledger
// ---------------------------------------------------------------------

#[test]
fn a_full_success_run_transitions_every_phase_and_every_handle() {
    let mut call = CallLedger::new();
    assert_eq!(call.phase(), Phase::Preparing);
    call.advance(Phase::Validated).unwrap();
    call.advance(Phase::Committed).unwrap();
    assert_eq!(call.phase(), Phase::Committed);

    let mut root = HandleLedger::new(Handle::root(7));
    let mut leaf = HandleLedger::new(Handle::leaf(0, 7));
    for handle in [&mut root, &mut leaf] {
        handle.apply(Event::Fill).unwrap();
        handle.apply(Event::Commit).unwrap();
        handle.apply(Event::Consume).unwrap();
        handle.apply(Event::Discharge).unwrap();
        assert_eq!(handle.state(), CarrierState::Released);
    }

    call.settle(Settlement::Success).unwrap();
    assert_eq!(call.settlement(), Some(Settlement::Success));
}

#[test]
fn phase_advance_rejects_skipping_or_going_backward() {
    let mut call = CallLedger::new();
    assert!(
        call.advance(Phase::Committed).is_err(),
        "cannot skip Validated"
    );
    call.advance(Phase::Validated).unwrap();
    assert!(
        call.advance(Phase::Preparing).is_err(),
        "cannot go backward"
    );
}

#[test]
fn settlement_is_sticky() {
    let mut call = CallLedger::new();
    call.settle(Settlement::ProviderFailure).unwrap();
    // Reasserting the same outcome is idempotent.
    call.settle(Settlement::ProviderFailure).unwrap();
    // A different outcome, in either direction, is rejected.
    let error = call
        .settle(Settlement::Success)
        .expect_err("failure cannot become success");
    assert_eq!(error.code, STICKY_SETTLEMENT_VIOLATION);
    let error = call
        .settle(Settlement::AllocationFailure)
        .expect_err("one failure reason cannot become another");
    assert_eq!(error.code, STICKY_SETTLEMENT_VIOLATION);
    assert_eq!(call.settlement(), Some(Settlement::ProviderFailure));
}

// ---------------------------------------------------------------------
// Release order
// ---------------------------------------------------------------------

fn leaves(generation: u32, count: u32) -> Vec<Handle> {
    (0..count)
        .map(|index| Handle::leaf(index, generation))
        .collect()
}

#[test]
fn failure_before_commit_releases_in_reverse_allocation_order() {
    let obligation_order = leaves(1, 3);
    let mut root = HandleLedger::new(Handle::root(1));
    let mut handles: Vec<HandleLedger> = obligation_order
        .iter()
        .map(|handle| HandleLedger::new(*handle))
        .collect();

    // Nothing reached Transferred: release straight from Created/Initialized.
    root.apply(Event::ReleaseBeforeTransfer).unwrap();
    for handle in &mut handles {
        handle.apply(Event::ReleaseBeforeTransfer).unwrap();
    }

    let submitted: Vec<Handle> = obligation_order.iter().rev().copied().collect();
    verify_release_order(&obligation_order, &submitted).expect("exact reverse order must verify");
}

#[test]
fn failure_after_commit_releases_from_transferred_without_consuming() {
    let obligation_order = leaves(2, 2);
    let mut handles: Vec<HandleLedger> = obligation_order
        .iter()
        .map(|handle| HandleLedger::new(*handle))
        .collect();
    for handle in &mut handles {
        handle.apply(Event::Fill).unwrap();
        handle.apply(Event::Commit).unwrap();
    }
    // Consumer refusal: release without a Consumed step.
    for handle in &mut handles {
        handle.apply(Event::ReleaseAfterTransfer).unwrap();
        assert_eq!(handle.state(), CarrierState::Released);
    }

    let submitted: Vec<Handle> = obligation_order.iter().rev().copied().collect();
    verify_release_order(&obligation_order, &submitted).expect("exact reverse order must verify");
}

#[test]
fn a_reordered_or_partial_release_sequence_is_rejected() {
    let obligation_order = leaves(3, 3);
    let correct_reverse: Vec<Handle> = obligation_order.iter().rev().copied().collect();

    // Same set, wrong order.
    let mut shuffled = correct_reverse.clone();
    shuffled.swap(0, 1);
    let error = verify_release_order(&obligation_order, &shuffled)
        .expect_err("a reordered release sequence must be rejected");
    assert_eq!(error.code, ILLEGAL_TRANSITION);

    // Partial release: missing the last element.
    let partial = &correct_reverse[..correct_reverse.len() - 1];
    let error = verify_release_order(&obligation_order, partial)
        .expect_err("a partial release sequence must be rejected");
    assert_eq!(error.code, ILLEGAL_TRANSITION);

    // Forward order (not reversed at all).
    let error = verify_release_order(&obligation_order, &obligation_order)
        .expect_err("the un-reversed order must be rejected");
    assert_eq!(error.code, ILLEGAL_TRANSITION);
}

// ---------------------------------------------------------------------
// Handle generation and capacity
// ---------------------------------------------------------------------

#[test]
fn a_wrong_generation_handle_is_rejected() {
    let handle = Handle::leaf(0, 5);
    verify_generation(handle, 5).expect("matching generation must verify");
    let error = verify_generation(handle, 6).expect_err("a mismatched generation must be rejected");
    assert_eq!(error.code, HANDLE_GENERATION_MISMATCH);
}

#[test]
fn handle_capacity_rejects_over_the_bound() {
    check_handle_capacity(MAX_LIVE_HANDLES).expect("exactly the bound must be admitted");
    let error = check_handle_capacity(MAX_LIVE_HANDLES + 1)
        .expect_err("one over the bound must be rejected");
    assert_eq!(error.code, CARRIER_CAPACITY);
}

// ---------------------------------------------------------------------
// Carrier binding codec
// ---------------------------------------------------------------------

fn sample_binding() -> CarrierBindingV1 {
    CarrierBindingV1::new(
        "sha256:descriptor-identity-fixture",
        TargetProfile::NativeC11,
        "sha256:runtime-identity-fixture",
    )
}

#[test]
fn binding_encode_is_deterministic() {
    assert_eq!(sample_binding().encode(), sample_binding().encode());
    assert_eq!(
        sample_binding().binding_digest(),
        sample_binding().binding_digest()
    );
}

#[test]
fn binding_encode_decode_round_trips() {
    let original = sample_binding();
    let decoded = decode_binding(&original.encode()).expect("well-formed bytes decode");
    assert_eq!(decoded, original);
}

#[test]
fn binding_replay_accepts_an_identical_candidate() {
    let trusted = sample_binding();
    let replayed = replay_binding(&trusted.encode(), &trusted).expect("identical bytes replay");
    assert_eq!(replayed, trusted);
}

#[test]
fn binding_decode_rejects_truncated_bytes() {
    let mut bytes = sample_binding().encode();
    bytes.truncate(bytes.len() - 2);
    let error = decode_binding(&bytes).expect_err("truncated bytes must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn binding_decode_rejects_trailing_bytes() {
    let mut bytes = sample_binding().encode();
    bytes.push(0);
    let error = decode_binding(&bytes).expect_err("trailing bytes must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn binding_decode_rejects_an_unknown_target_profile() {
    let mut bytes = Vec::new();
    frame(&mut bytes, CARRIER_SCHEMA.as_bytes());
    frame(&mut bytes, b"sha256:descriptor-identity-fixture");
    frame(&mut bytes, b"quantum-target");
    frame(&mut bytes, b"sha256:runtime-identity-fixture");
    let error = decode_binding(&bytes).expect_err("an unknown target profile must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn binding_decode_rejects_an_unknown_schema() {
    let mut bytes = Vec::new();
    frame(&mut bytes, b"semaprax.some-other-carrier.v9");
    frame(&mut bytes, b"sha256:descriptor-identity-fixture");
    frame(&mut bytes, b"native-c11");
    frame(&mut bytes, b"sha256:runtime-identity-fixture");
    let error = decode_binding(&bytes).expect_err("an unknown schema must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn binding_decode_rejects_an_oversized_length_claim() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(u64::MAX).to_le_bytes());
    bytes.extend_from_slice(b"short");
    let error = decode_binding(&bytes).expect_err("an oversized length claim must not decode");
    assert_eq!(error.code, MALFORMED_CARRIER);
}

#[test]
fn binding_replay_rejects_a_cross_paired_binding_on_every_bound_field() {
    let trusted = sample_binding();

    let mut different_descriptor = trusted.clone();
    different_descriptor.descriptor_identity_digest = "sha256:different-descriptor".to_owned();
    let error = replay_binding(&different_descriptor.encode(), &trusted)
        .expect_err("a mismatched descriptor digest must not replay");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    let mut different_target = trusted.clone();
    different_target.target_profile = TargetProfile::CoreWasm;
    let error = replay_binding(&different_target.encode(), &trusted)
        .expect_err("a mismatched target profile must not replay");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    let mut different_runtime = trusted.clone();
    different_runtime.runtime_identity = "sha256:different-runtime".to_owned();
    let error = replay_binding(&different_runtime.encode(), &trusted)
        .expect_err("a mismatched runtime identity must not replay");
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);
}
