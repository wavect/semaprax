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
fn owned_frame_durable_start_refuses_existing_history_and_entire_granted_scope_prewrite() {
    use crate::interpreter::resumable::owned_frame::durable::{
        evaluation_count, reset_evaluations,
    };
    let scopes = [
        SourceCheckpointScope::new("sha256:other-program", "owned-journal", 7).unwrap(),
        SourceCheckpointScope::new("sha256:program", "other-invocation", 7).unwrap(),
        SourceCheckpointScope::new("sha256:program", "owned-journal", 8).unwrap(),
    ];
    for existing in [false, true] {
        for changed_scope in std::iter::once(None).chain(scopes.iter().map(Some)) {
            if !existing && changed_scope.is_none() {
                continue;
            }
            let directory = Directory::new();
            let (plan, input, scope, key) = fixture();
            let mut lease =
                RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope)
                    .unwrap();
            let identity = lease.identity();
            if existing {
                let state = State::new(plan.clone(), scope.clone(), 100, 2000, identity).unwrap();
                let mut journal = Journal::fresh(lease, &key, state).unwrap();
                journal
                    .append(created(&journal.state, &input).unwrap(), &[])
                    .unwrap();
                drop(journal);
                let grant =
                    OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true)
                        .unwrap();
                lease = RegisteredJournalLease::recover(directory.file(), grant, &scope).unwrap();
            }
            let before = lease.read().unwrap();
            let actual = admit_owned_frame_input(&plan, input.clone())
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let weak = snapshot::argument_weak(&actual);
            let prepared = PreparedOwnedFrame::new(
                &plan,
                &actual,
                changed_scope.unwrap_or(&scope).clone(),
                100,
                2000,
            )
            .unwrap();
            reset_evaluations();
            match OwnedFrameInvocation::start(prepared, actual, lease, &key, &policy()) {
                OwnedFrameStart::Rejected { argument, error } => {
                    assert_eq!(error, Error::Binding);
                    assert_eq!(
                        codec::input(&plan, &snapshot::argument_input(&argument).unwrap()).unwrap(),
                        codec::input(&plan, &input).unwrap()
                    );
                    assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
                    drop(argument);
                }
                OwnedFrameStart::Invocation { .. } => panic!("wrong history/scope committed"),
            }
            assert_eq!(evaluation_count(), 0);
            let grant = OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true)
                .unwrap();
            let mut lease =
                RegisteredJournalLease::recover(directory.file(), grant, &scope).unwrap();
            assert_eq!(lease.read().unwrap(), before);
        }
    }
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
            use super::super::OwnedFrameInvocationStatus as Status;
            if recovered.journal.state.phase == Phase::Dispatched {
                assert_eq!(
                    recovered.status(),
                    Status::Active(super::super::OwnedFrameActivePhase::DispatchedInDoubt)
                );
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
                let expected = match recovered.journal.state.phase {
                    Phase::Empty => Status::UncommittedStart { created: false },
                    Phase::Created => Status::UncommittedStart { created: true },
                    Phase::CleanupStarted => Status::CleanupInDoubt { failed: false },
                    Phase::CleanupSettled | Phase::Claimed => Status::ResultDeliveryInDoubt,
                    _ => panic!("classified tail"),
                };
                assert_eq!(recovered.status(), expected);
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
        for number in 1..=if matches!(phase, Phase::Answered | Phase::Resuming) {
            4
        } else {
            2
        } {
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
                    } else if number == 4 {
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
                    prior
                        + if number == 1 && !after {
                            0
                        } else if number == 4 || number == 3 && after {
                            200
                        } else {
                            100
                        }
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

#[test]
fn owned_frame_durable_answered_recovery_charges_each_historical_phase_separately() {
    let (plan, input, scope, key) = fixture();
    let argument = admit_owned_frame_input(&plan, input.clone())
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let mut start_budget = OwnedFrameBudget::new(100).unwrap();
    let owner = DurableOwner::from_argument(argument).start(&mut start_budget);
    let mut resume_budget = OwnedFrameBudget::new(100).unwrap();
    let owner = owner.resume(ArgumentValue::Int(1), &mut resume_budget);
    assert!(owner.failure().is_none());
    let start_steps = start_budget.consumed() as u64;
    let resume_steps = resume_budget.consumed() as u64;
    let allowance = start_steps.max(resume_steps);
    assert!(start_steps > 0 && resume_steps > 0 && start_steps + resume_steps > allowance);
    drop(owner);
    let directory = Directory::new();
    let argument =
        admit_owned_frame_input(&plan, input).unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let prepared =
        PreparedOwnedFrame::new(&plan, &argument, scope.clone(), allowance, 8 * allowance).unwrap();
    let lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope).unwrap();
    let mut invocation =
        match OwnedFrameInvocation::start(prepared, argument, lease, &key, &policy()) {
            OwnedFrameStart::Invocation {
                invocation,
                acknowledgement,
            } => {
                acknowledgement.unwrap();
                invocation
            }
            OwnedFrameStart::Rejected { error, .. } => panic!("{error:?}"),
        };
    invocation.begin(&policy(), &scope).unwrap();
    invocation
        .dispatch(&policy(), &scope, &mut |_| Ok(ArgumentValue::Int(1)))
        .unwrap();
    let identity = invocation.journal.lease.identity();
    drop(invocation);
    let registration =
        OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true).unwrap();
    let lease = RegisteredJournalLease::recover(directory.file(), registration, &scope).unwrap();
    let (mut recovered, validation) = OwnedFrameInvocation::recover(
        lease,
        &key,
        &plan,
        scope.clone(),
        allowance,
        8 * allowance,
        &policy(),
    )
    .unwrap();
    validation.unwrap();
    assert_eq!(recovered.journal.state.reserved_total, 3 * allowance);
    assert_eq!(recovered.journal.state.reservation_count, 3);
    recovered.resume(&policy(), &scope).unwrap();
    assert_eq!(recovered.journal.state.phase, Phase::Completed);
    assert_eq!(recovered.journal.state.reserved_total, 4 * allowance);
    assert_eq!(
        recovered.journal.state.consumed_total,
        2 * (start_steps + resume_steps)
    );
}

#[test]
fn owned_frame_durable_capacity_after_replay_ack_keeps_fresh_retry_reservation() {
    for phase in [Phase::Starting, Phase::Resuming] {
        let directory = Directory::new();
        let (plan, input, scope, key) = fixture();
        let mut invocation = started(&directory, &key, &plan, input, &scope, None);
        if phase == Phase::Starting {
            invocation.reserve(Kind::StartReserved).unwrap();
        } else {
            invocation.begin(&policy(), &scope).unwrap();
            invocation
                .dispatch(&policy(), &scope, &mut |_| Ok(ArgumentValue::Int(1)))
                .unwrap();
            invocation.reserve(Kind::ResumeReserved).unwrap();
        }
        let basis = invocation.journal.state.basis().unwrap();
        invocation.reserve(Kind::ReplayReserved).unwrap();
        let validated=Record::new(Kind::ReplayValidated,json!({"reservation_sequence":invocation.journal.state.records.len() as u64-1,"basis":basis,"consumed_steps":0})).unwrap();
        let branches =
            capacity::remaining(&invocation.journal.state, &validated, &key, false).unwrap();
        let retry = if phase == Phase::Starting {
            Kind::StartReserved
        } else {
            Kind::ResumeReserved
        };
        assert!(branches
            .iter()
            .any(|branch| branch.iter().any(|row| row.kind == retry)));
    }
}

#[test]
fn owned_frame_durable_drop_disposes_last_backing_before_releasing_registered_lock() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let invocation = started(&directory, &key, &plan, input, &scope, None);
    let identity = invocation.journal.lease.identity();
    let weak = invocation.owner.as_ref().unwrap().weak_leaves();
    let observed = std::rc::Rc::new(std::cell::Cell::new(false));
    let output = observed.clone();
    let held = directory.file();
    let expected = scope.clone();
    DROP_OBSERVER.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            assert!(
                weak.iter().all(|leaf| leaf.upgrade().is_none()),
                "actual backing still owned when lock could release"
            );
            let grant =
                OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &expected, true)
                    .unwrap();
            assert_eq!(
                RegisteredJournalLease::recover(held, grant, &expected).err(),
                Some(Error::Busy)
            );
            output.set(true);
        }))
    });
    drop(invocation);
    assert!(observed.get());
    let grant =
        OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope, true).unwrap();
    assert!(
        RegisteredJournalLease::recover(directory.file(), grant, &scope).is_ok(),
        "control: lease is released after backing disposal"
    );
}
