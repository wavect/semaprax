use super::super::journal::tests::{fixture, Directory};
use super::super::store::OwnedFrameStoreRegistration;
use super::*;
use crate::interpreter::resumable::owned_frame::admit_owned_frame_input;

fn policy() -> CapabilityPolicy {
    CapabilityPolicy::new(vec!["fixture.park".into()]).unwrap()
}
fn started<'key>(
    directory: &Directory,
    key: &'key SourceCheckpointKey,
    plan: &CheckedOwnedFramePlan,
    input: OwnedFrameInput,
    scope: &SourceCheckpointScope,
    fault: Option<(usize, bool)>,
) -> OwnedFrameInvocation<'key> {
    let argument =
        admit_owned_frame_input(plan, input).unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let prepared = PreparedOwnedFrame::new(plan, &argument, scope.clone(), 100, 2000).unwrap();
    let mut lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), scope).unwrap();
    if let Some((number, after)) = fault {
        lease.fail_append(number, after);
    }
    match OwnedFrameInvocation::start(prepared, argument, lease, key, &policy()) {
        OwnedFrameStart::Invocation {
            invocation,
            acknowledgement,
        } => {
            if fault.is_none() {
                acknowledgement.unwrap();
            }
            invocation
        }
        OwnedFrameStart::Rejected { error, .. } => panic!("{error:?}"),
    }
}
#[test]
fn owned_frame_durable_route_owns_real_backing_through_claim_and_evidence() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let mut invocation = started(&directory, &key, &plan, input, &scope, None);
    let weak = invocation.owner.as_ref().unwrap().weak_leaves();
    invocation.begin(&policy(), &scope).unwrap();
    let mut calls = 0;
    invocation
        .dispatch(&policy(), &scope, &mut |request| {
            calls += 1;
            assert_eq!(request, &ArgumentValue::Int(4));
            Ok(ArgumentValue::Int(9))
        })
        .unwrap();
    invocation.resume(&policy(), &scope).unwrap();
    let mut observations = 0;
    invocation
        .settle(&policy(), &scope, &mut |_| {
            observations += 1;
            true
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        observations, 0,
        "identity result's leaves remain owned by unpublished result"
    );
    assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
    let result = invocation.claim(&policy(), &scope).unwrap();
    assert!(invocation.claim(&policy(), &scope).is_err());
    let (bytes, digest) = invocation.evidence().unwrap();
    let evidence: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(evidence["reserved_total"], 200);
    assert_eq!(evidence["reservation_count"], 2);
    assert!(evidence["consumed_total"].as_u64().unwrap() > 0);
    assert!(evidence["result_claimed_sequence"].is_u64());
    assert_eq!(
        digest,
        codec::digest(b"semaprax.source-owned-frame-evidence.v1\0", &bytes)
    );
    assert!(!bytes.ends_with(b"\n"));
    drop(result);
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
}

#[test]
fn owned_frame_durable_start_recompares_actual_argument_before_any_write() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let prepared_argument = admit_owned_frame_input(&plan, input.clone())
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let prepared =
        PreparedOwnedFrame::new(&plan, &prepared_argument, scope.clone(), 100, 2000).unwrap();
    drop(prepared_argument);
    let mut changed = input;
    let crate::interpreter::resumable::owned_frame::OwnedFrameInputValue::Bytes(bytes) =
        &mut changed.fields[0].value
    else {
        panic!("fixture")
    };
    bytes.push(9);
    let actual = admit_owned_frame_input(&plan, changed.clone())
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let weak = snapshot::argument_weak(&actual);
    let lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope).unwrap();
    let identity = lease.identity();
    match OwnedFrameInvocation::start(prepared, actual, lease, &key, &policy()) {
        OwnedFrameStart::Rejected { argument, error } => {
            assert_eq!(error, Error::Binding);
            assert_eq!(
                codec::input(&plan, &snapshot::argument_input(&argument).unwrap()).unwrap(),
                codec::input(&plan, &changed).unwrap()
            );
            assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
            drop(argument);
        }
        OwnedFrameStart::Invocation { .. } => panic!("mismatched prepared facts committed"),
    }
    let grant =
        OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true).unwrap();
    let mut lease = RegisteredJournalLease::recover(directory.file(), grant, &scope).unwrap();
    assert!(lease.read().unwrap().is_empty());
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
}
#[test]
fn owned_frame_durable_abandon_releases_actual_leaves_in_compiler_order_after_drop() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let mut invocation = started(&directory, &key, &plan, input, &scope, None);
    let weak = invocation.owner.as_ref().unwrap().weak_leaves(); // BTree inventory a,m,z
    invocation.begin(&policy(), &scope).unwrap();
    invocation.abandon(&policy(), &scope).unwrap();
    let mut order = Vec::new();
    let mut facts = Vec::new();
    invocation
        .settle(&policy(), &scope, &mut |action| {
            order.push(action.source.projections[0].as_str().to_owned());
            facts.push(
                weak.iter()
                    .map(|leaf| leaf.upgrade().is_none())
                    .collect::<Vec<_>>(),
            );
            true
        })
        .unwrap();
    assert_eq!(
        order,
        ["fixture.state.m", "fixture.state.a", "fixture.state.z"]
    );
    assert_eq!(
        facts,
        [
            vec![false, true, false],
            vec![true, true, false],
            vec![true, true, true]
        ]
    );
    assert!(invocation.claim(&policy(), &scope).is_err());
    let evidence: Value = serde_json::from_slice(&invocation.evidence().unwrap().0).unwrap();
    assert_eq!(evidence["terminal"]["failure"], "host_abandoned");
}
#[test]
fn owned_frame_durable_all_original_ack_windows_before_and_after_persistence_poison() {
    // Created through ResultClaimed: a synchronized success has eleven rows.
    // Every injected ACK loss must stop evaluator/host/release/result progress.
    for number in 1..=11 {
        for after in [false, true] {
            let directory = Directory::new();
            let (plan, input, scope, key) = fixture();
            let mut invocation = started(
                &directory,
                &key,
                &plan,
                input,
                &scope,
                Some((number, after)),
            );
            let identity = invocation.journal.lease.identity();
            let mut calls = 0;
            let mut observers = 0;
            let outcome = (|| {
                invocation.begin(&policy(), &scope)?;
                invocation.dispatch(&policy(), &scope, &mut |_| {
                    calls += 1;
                    Ok(ArgumentValue::Int(1))
                })?;
                invocation.resume(&policy(), &scope)?;
                invocation.settle(&policy(), &scope, &mut |_| {
                    observers += 1;
                    true
                })?;
                let result = invocation.claim(&policy(), &scope)?;
                drop(result);
                Ok::<(), Error>(())
            })();
            assert_eq!(outcome, Err(Error::InDoubt), "row {number} after={after}");
            assert!(invocation.begin(&policy(), &scope).is_err());
            let before = calls;
            assert!(invocation
                .dispatch(&policy(), &scope, &mut |_| {
                    calls += 1;
                    Ok(ArgumentValue::Int(0))
                })
                .is_err());
            assert_eq!(calls, before);
            assert_eq!(calls, usize::from(number > 5));
            assert_eq!(observers, 0);
            assert!(invocation.claim(&policy(), &scope).is_err());
            drop(invocation);
            let grant = OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true)
                .unwrap();
            let lease = RegisteredJournalLease::recover(directory.file(), grant, &scope).unwrap();
            let (mut recovered, validation) = OwnedFrameInvocation::recover(
                lease,
                &key,
                &plan,
                scope.clone(),
                100,
                2000,
                &policy(),
            )
            .unwrap();
            validation.unwrap();
            if recovered.journal.state.phase == Phase::Dispatched {
                let mut recovered_calls = 0;
                assert_eq!(
                    recovered.dispatch(&policy(), &scope, &mut |_| {
                        recovered_calls += 1;
                        Ok(ArgumentValue::Int(0))
                    }),
                    Err(Error::InDoubt)
                );
                assert_eq!(recovered_calls, 0);
            }
            if matches!(
                recovered.journal.state.phase,
                Phase::Created
                    | Phase::CleanupStarted
                    | Phase::CleanupSettled
                    | Phase::Claimed
                    | Phase::Empty
            ) {
                assert!(
                    recovered.owner.is_none(),
                    "forbidden remint tail {:?}",
                    recovered.journal.state.phase
                );
                assert!(recovered.claim(&policy(), &scope).is_err());
            }
        }
    }
}

#[test]
fn owned_frame_durable_replay_ack_windows_preserve_charges_without_unreserved_evaluation() {
    use crate::interpreter::resumable::owned_frame::durable::{
        evaluation_count, reset_evaluations,
    };
    for phase in [
        Phase::Starting,
        Phase::Yielded,
        Phase::Answered,
        Phase::Resuming,
    ] {
        for number in [1, 2] {
            for after in [false, true] {
                let directory = Directory::new();
                let (plan, input, scope, key) = fixture();
                let mut invocation = started(&directory, &key, &plan, input, &scope, None);
                if phase == Phase::Starting {
                    invocation.reserve(Kind::StartReserved).unwrap();
                } else {
                    invocation.begin(&policy(), &scope).unwrap();
                    if matches!(phase, Phase::Answered | Phase::Resuming) {
                        invocation
                            .dispatch(&policy(), &scope, &mut |_| Ok(ArgumentValue::Int(1)))
                            .unwrap();
                        if phase == Phase::Resuming {
                            invocation.reserve(Kind::ResumeReserved).unwrap();
                        }
                    }
                }
                let identity = invocation.journal.lease.identity();
                let prior = invocation.journal.state.reserved_total;
                drop(invocation);
                let registration =
                    OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true)
                        .unwrap();
                let mut lease =
                    RegisteredJournalLease::recover(directory.file(), registration, &scope)
                        .unwrap();
                lease.fail_append(number, after);
                reset_evaluations();
                let (mut recovered, validation) = OwnedFrameInvocation::recover(
                    lease,
                    &key,
                    &plan,
                    scope.clone(),
                    100,
                    2000,
                    &policy(),
                )
                .unwrap();
                assert_eq!(
                    validation,
                    Err(Error::InDoubt),
                    "{phase:?} row {number} after={after}"
                );
                assert_eq!(
                    evaluation_count(),
                    if number == 1 {
                        0
                    } else if matches!(phase, Phase::Answered | Phase::Resuming) {
                        2
                    } else {
                        1
                    }
                );
                assert!(recovered.begin(&policy(), &scope).is_err());
                assert!(recovered.resume(&policy(), &scope).is_err());
                let count = evaluation_count();
                let mut calls = 0;
                assert!(recovered
                    .dispatch(&policy(), &scope, &mut |_| {
                        calls += 1;
                        Ok(ArgumentValue::Int(0))
                    })
                    .is_err());
                assert_eq!(calls, 0);
                assert_eq!(evaluation_count(), count);
                drop(recovered);
                let registration =
                    OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true)
                        .unwrap();
                let lease = RegisteredJournalLease::recover(directory.file(), registration, &scope)
                    .unwrap();
                let state = State::new(plan.clone(), scope.clone(), 100, 2000, identity).unwrap();
                let journal = Journal::reopen(lease, &key, state).unwrap();
                assert_eq!(
                    journal.state.reserved_total,
                    prior + if number == 1 && !after { 0 } else { 100 }
                );
                assert_eq!(journal.state.phase, phase);
            }
        }
    }
}

#[test]
fn owned_frame_durable_cleanup_ack_loss_never_repeats_physical_release() {
    for number in [6, 7] {
        for after in [false, true] {
            let directory = Directory::new();
            let (plan, input, scope, key) = fixture();
            let mut invocation = started(
                &directory,
                &key,
                &plan,
                input,
                &scope,
                Some((number, after)),
            );
            let weak = invocation.owner.as_ref().unwrap().weak_leaves();
            let identity = invocation.journal.lease.identity();
            invocation.begin(&policy(), &scope).unwrap();
            invocation.abandon(&policy(), &scope).unwrap();
            let mut observations = 0;
            assert_eq!(
                invocation.settle(&policy(), &scope, &mut |_| {
                    observations += 1;
                    true
                }),
                Err(Error::InDoubt)
            );
            assert_eq!(observations, if number == 6 { 0 } else { 3 });
            assert!(invocation
                .settle(&policy(), &scope, &mut |_| {
                    observations += 1;
                    true
                })
                .is_err());
            assert_eq!(observations, if number == 6 { 0 } else { 3 });
            assert_eq!(
                weak.iter().filter(|leaf| leaf.upgrade().is_none()).count(),
                if number == 6 { 0 } else { 3 }
            );
            drop(invocation);
            let grant = OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true)
                .unwrap();
            let lease = RegisteredJournalLease::recover(directory.file(), grant, &scope).unwrap();
            let (mut recovered, validation) = OwnedFrameInvocation::recover(
                lease,
                &key,
                &plan,
                scope.clone(),
                100,
                2000,
                &policy(),
            )
            .unwrap();
            validation.unwrap();
            if number != 6 || after {
                assert!(recovered.owner.is_none());
                assert!(recovered
                    .settle(&policy(), &scope, &mut |_| {
                        observations += 1;
                        true
                    })
                    .is_err());
                assert!(recovered.claim(&policy(), &scope).is_err());
                if recovered.journal.state.phase == Phase::CleanupStarted {
                    let grant = recovered
                        .grant_cleanup_confirmation_for_trusted_host(&policy(), &scope)
                        .unwrap();
                    recovered.confirm_cleanup(&policy(), &scope, grant).unwrap();
                    assert_eq!(
                        recovered.journal.state.cleanup_settled().unwrap().1.fields["receipt"]
                            ["kind"],
                        "host_confirmed"
                    );
                    assert!(recovered.claim(&policy(), &scope).is_err());
                }
            }
        }
    }
}
