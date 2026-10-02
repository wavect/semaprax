//! Genuine failed initial/continued Observe, actual cleanup and fixed ACKs.
//! No reconstructed history produces an owner, release permit or terminal claim.
use super::super::tests::{ack, initial, ordinary, with_continued};
use super::*;
use crate::agent_runtime::AgentCancellation;
use std::{
    path::Path,
    sync::{Arc, Weak},
};
fn with_failed(
    initial_route: bool,
    callback: impl for<'j> FnOnce(
        &'j SourceOwnedWaitJournalV8,
        LiveSettledObserveV8<'j>,
        Vec<Weak<[u8]>>,
        Option<TargetAccounting>,
        &'j AgentCancellation,
        Option<&Path>,
    ),
) {
    if initial_route {
        CheckedOwnedWaitJournalContextV8::test_with_actual_initial_observe_ensures_store(
            |context, lease, key, directory| {
                let context = context.with_cumulative_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = AgentCancellation::new();
                let initialized = initial(&journal, &cancel);
                let weak = initialized.owner.test_weak();
                let failure = observe_live_actor_v8(initialized)
                    .err()
                    .expect("actual failed initial Observe");
                let LiveObserveFailureV8::Settlement(LiveObserveSettlementActorFailureV8::Failed(
                    failed,
                )) = failure
                else {
                    panic!("actual ACKed failed owner")
                };
                callback(&journal, failed, weak, None, &cancel, Some(directory));
            },
        );
    } else {
        with_continued(
            true,
            |journal, obligation, weak, ledger, observation, consumed, cancel| {
                assert!(observation.is_none());
                let failed = ack(journal, obligation);
                assert_eq!(failed.owner.data().unwrap().consumed, consumed as u64);
                callback(journal, failed, weak, Some(ledger), cancel, None);
            },
        );
    }
}
fn generic_refuses_without_io(journal: &SourceOwnedWaitJournalV8, row: EntryV8) {
    let before = journal.test_observe_lease().borrow_mut().read().unwrap();
    if let EntryV8::Ordinary(SourceJournalEntry::Stop { turn, attempt, .. }) = &row {
        let mut coordinate_free = row.clone();
        let EntryV8::Ordinary(SourceJournalEntry::Stop {
            turn: t,
            attempt: a,
            ..
        }) = &mut coordinate_free
        else {
            unreachable!()
        };
        *t = None;
        *a = None;
        assert!(journal
            .begin_session()
            .unwrap()
            .append(coordinate_free)
            .is_err());
        let mut different_attempt = row.clone();
        let EntryV8::Ordinary(SourceJournalEntry::Stop {
            turn: t,
            attempt: a,
            ..
        }) = &mut different_attempt
        else {
            unreachable!()
        };
        *t = *turn;
        *a = Some(attempt.unwrap_or(0).checked_add(1).unwrap());
        assert!(journal
            .begin_session()
            .unwrap()
            .append(different_attempt)
            .is_err());
    }
    assert!(journal.begin_session().unwrap().append(row).is_err());
    assert_eq!(
        journal.test_observe_lease().borrow_mut().read().unwrap(),
        before
    );
    assert!(
        journal.hold().is_ok(),
        "pure producer refusal preserves live authority"
    );
}
fn append_failure_description(
    failure: &crate::live_invocation::source_journal::owned_wait_v8::append::LiveFailedObserveStateAppendFailureV8<'_>,
) -> String {
    use crate::live_invocation::source_journal::owned_wait_v8::append::{
        AppendFailureV8 as Append, LiveFailedObserveStateAppendFailureV8 as Actual,
    };
    match failure {
        Actual::Before { error, .. } => format!("Before: {error:?}"),
        Actual::Acknowledged { error, .. } => format!("Acknowledged: {error:?}"),
        Actual::After { error, .. } => format!("After: {error:?}"),
        Actual::Append { _failure, .. } => match _failure {
            Append::PhysicalBeforeCandidate { error, .. } => {
                format!("PhysicalBeforeCandidate: {error:?}")
            }
            Append::CandidateRefused { error, .. } => format!("CandidateRefused: {error:?}"),
            Append::PrewriteRefused { error, .. } => format!("PrewriteRefused: {error:?}"),
            Append::InDoubt { error, .. } => format!("InDoubt: {error:?}"),
        },
    }
}
fn start<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    failed: LiveSettledObserveV8<'j>,
) -> (LiveStartedFailedObserveStateV8<'j>, Vec<Weak<[u8]>>) {
    let obligation = failed
        .prepare_failed_state_cleanup()
        .unwrap_or_else(|_| panic!("actual failure selects State cleanup"));
    let FailedObserveAppendOwnerV8::Failed(source) = &obligation.owner else {
        panic!()
    };
    let weak = match &source.owner {
        FailedObserveOwnerV8::Initial(x) => x.test_cleanup_weak_v8(),
        FailedObserveOwnerV8::Continued { failed, .. } => failed.test_cleanup_weak_v8(),
    };
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    generic_refuses_without_io(journal, obligation.selected_row().clone());
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    let ack = journal
        .begin_session()
        .unwrap()
        .append_failed_observe_state(obligation)
        .unwrap_or_else(|failure| {
            panic!("true Started ACK: {}", append_failure_description(&failure))
        })
        .advance_failed_observe_state()
        .unwrap_or_else(|_| panic!("same owner Started successor"));
    let LiveFailedObserveStateAcknowledgedV8::Started(started) = ack else {
        panic!("Started")
    };
    (started, weak)
}
fn receipt<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    released: LiveReleasedFailedObserveStateV8<'j>,
) -> LiveReleasedFailedObserveStateV8<'j> {
    let selected = released
        .prepare_receipt()
        .unwrap_or_else(|_| panic!("actual exact receipt"));
    generic_refuses_without_io(journal, selected.selected_row().clone());
    let ack = journal
        .begin_session()
        .unwrap()
        .append_failed_observe_state(selected)
        .unwrap_or_else(|_| panic!("true receipt ACK"))
        .advance_failed_observe_state()
        .unwrap_or_else(|_| panic!("actual receipt successor"));
    let LiveFailedObserveStateAcknowledgedV8::Released(released) = ack else {
        panic!("receipt")
    };
    released
}
#[test]
fn failed_observe_state_cleanup_initial_and_continued_actual_receipt_sticky_stop_no_recharge() {
    for initial_route in [true, false] {
        with_failed(initial_route, |journal, failed, old_weak, ledger, _, _| {
            let data = failed.owner.data().unwrap();
            let oracle = ordinary(journal, &data.state);
            let crate::interpreter::retained_call::RetainedCallOutcome::LanguageFailure(status) =
                oracle.outcome
            else {
                panic!("ordinary Ensures oracle")
            };
            assert_eq!(data.failure, Some(OwnedFrameFailure::Language(status)));
            assert_eq!(data.consumed, oracle.steps_used as u64);
            let before = journal.begin_session().unwrap();
            let (reserved, stages, turn, _, _, _, _) =
                before.failed_observe_cleanup_facts().unwrap();
            let expected = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
                &journal
                    .context()
                    .ready_runtime()
                    .unwrap()
                    .1
                    .wait()
                    .observe()
                    .helper()
                    .liveness()
                    .failure_cleanup,
            )
            .unwrap();
            let mut observed = 0;
            let stopped = stop_failed_observe_state_v8(failed, |_| {
                observed += 1;
            })
            .unwrap_or_else(|_| panic!("actual State cleanup, receipt, and Stop"));
            assert_eq!(observed, expected.as_array().unwrap().len());
            assert!(old_weak.iter().all(|w| w.upgrade().is_none()));
            match (&stopped._released.owner, ledger) {
                (ReleasedObserveOwnerV8::Initial(_), None) => {}
                (ReleasedObserveOwnerV8::Continued { accounting, .. }, Some(expected)) => {
                    assert_eq!(*accounting, expected)
                }
                _ => panic!("original route and ledger preserved"),
            }
            assert_eq!(stopped._released.receipt["settlement"], "completed");
            assert_eq!(
                stopped._released.receipt["operations"]
                    .as_array()
                    .unwrap()
                    .len(),
                observed
            );
            let current = journal.begin_session().unwrap();
            let (r, s, t, _) = current
                .test_observe_inventory()
                .failed_observe_cleanup_current_facts()
                .unwrap();
            assert_eq!((r, s, t), (reserved, stages, turn));
            assert!(matches!(
                current.test_observe_inventory().test_observe_entries().last().unwrap().entry,
                EntryV8::Ordinary(SourceJournalEntry::Stop {
                    turn: Some(actual_turn),
                    attempt: None,
                    status: SourceStopStatus::Rejected,
                    reason: SourceStopReason::StageRefused,
                }) if *actual_turn == turn
            ));
            drop(stopped);
        });
    }
}
#[test]
fn failed_observe_state_cleanup_cancellation_before_started_has_zero_state_work() {
    for initial_route in [true, false] {
        with_failed(initial_route, |journal, failed, weak, _, cancel, _| {
            let before = journal.begin_session().unwrap().acknowledged_bytes();
            cancel.cancel();
            let actual = stop_failed_observe_state_v8(failed, |_| {
                panic!("cancelled before the State cleanup observer")
            })
            .err()
            .expect("private Stop driver seals pre-Started cancellation");
            assert_eq!(actual.status(), SourceJournalError::Poisoned);
            assert_eq!(
                journal
                    .test_observe_lease()
                    .borrow_mut()
                    .read()
                    .unwrap()
                    .len(),
                before
            );
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            drop(actual);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
#[cfg(unix)]
fn failed_observe_state_driver_prewrite_fault_retains_actual_owner() {
    for initial_route in [true, false] {
        with_failed(initial_route, |journal, failed, weak, _, _, _| {
            let next = journal.begin_session().unwrap().sequence() + 1;
            journal
                .test_observe_lease()
                .borrow_mut()
                .test_fail_before_write(next);
            let quarantined = stop_failed_observe_state_v8(failed, |_| {
                panic!("prewrite fault cannot release State")
            })
            .err()
            .expect("opaque quarantine retains actual failed Observe owner");
            assert_eq!(quarantined.status(), SourceJournalError::Poisoned);
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            drop(quarantined);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn failed_observe_state_cleanup_poststarted_cancel_allows_real_receipt_but_never_stop() {
    for initial_route in [true, false] {
        with_failed(initial_route, |journal, failed, _, _, cancel, _| {
            let (started, weak) = start(journal, failed);
            cancel.cancel();
            let mut work = 0;
            let released = started
                .release(|_| work += 1)
                .unwrap_or_else(|_| panic!("incurred State release survives cancellation"));
            assert_eq!(work, weak.len());
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
            let released = receipt(journal, released);
            let before = journal.test_observe_lease().borrow_mut().read().unwrap();
            let retained = released
                .prepare_stop()
                .err()
                .expect("full Stop guard resumes");
            assert_eq!(
                journal.test_observe_lease().borrow_mut().read().unwrap(),
                before
            );
            assert!(journal.hold().is_err());
            drop(retained);
        });
    }
}
#[test]
fn failed_observe_state_cleanup_observer_panic_receipt_never_changes_original_failure() {
    for initial_route in [true, false] {
        with_failed(initial_route, |journal, failed, _, _, _, _| {
            let (started, weak) = start(journal, failed);
            let failure = started.lineage.cache.failure.clone();
            let mut calls = 0;
            let released = started
                .release(|_| {
                    calls += 1;
                    if calls == 1 {
                        panic!("actual observer panic after drop");
                    }
                })
                .unwrap_or_else(|_| panic!("typed actual failed observation receipt"));
            assert_eq!(calls, weak.len());
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
            assert_eq!(released.lineage.cache.failure, failure);
            assert_eq!(released.receipt["operations"][0]["outcome"], "failed");
            for value in released.receipt["operations"]
                .as_array()
                .unwrap()
                .iter()
                .skip(1)
            {
                assert_eq!(value["outcome"], "completed");
            }
            let released = receipt(journal, released);
            assert_eq!(released.receipt["settlement"], "failed");
            assert!(released.prepare_stop().is_err());
            assert!(journal.hold().is_err());
        });
    }
}
#[test]
#[cfg(unix)]
fn failed_observe_state_cleanup_actual_faults_all_three_phases_are_permanent() {
    for phase in 0..3 {
        for mode in 0..4 {
            with_failed(phase != 1, |journal, failed, _, _, _, _| {
                let (obligation, weak) = if phase == 0 {
                    let selected = failed
                        .prepare_failed_state_cleanup()
                        .unwrap_or_else(|_| panic!("actual Started obligation"));
                    let FailedObserveAppendOwnerV8::Failed(source) = &selected.owner else {
                        panic!()
                    };
                    let weak = match &source.owner {
                        FailedObserveOwnerV8::Initial(x) => x.test_cleanup_weak_v8(),
                        FailedObserveOwnerV8::Continued { failed, .. } => {
                            failed.test_cleanup_weak_v8()
                        }
                    };
                    (selected, weak)
                } else {
                    let (started, weak) = start(journal, failed);
                    let released = started
                        .release(|_| {})
                        .unwrap_or_else(|_| panic!("actual State release"));
                    let selected = if phase == 1 {
                        released
                            .prepare_receipt()
                            .unwrap_or_else(|_| panic!("receipt"))
                    } else {
                        receipt(journal, released)
                            .prepare_stop()
                            .unwrap_or_else(|_| panic!("Stop"))
                    };
                    (selected, weak)
                };
                let before = journal.test_observe_lease().borrow_mut().read().unwrap();
                let number = obligation.sequence() + 1;
                {
                    let mut lease = journal.test_observe_lease().borrow_mut();
                    match mode {
                        0 => lease.test_fail_before_write(number),
                        1 => lease.test_fail_after_write(number),
                        2 => lease.test_fail_before_sync(number),
                        _ => lease.test_fail_after_sync(number),
                    }
                }
                let actual = journal
                    .begin_session()
                    .unwrap()
                    .append_failed_observe_state(obligation)
                    .err()
                    .expect("physical fault cannot mint ACK");
                assert!(
                    matches!(&actual,
                        crate::live_invocation::source_journal::owned_wait_v8::append::LiveFailedObserveStateAppendFailureV8::Append {
                            _failure: crate::live_invocation::source_journal::owned_wait_v8::append::AppendFailureV8::InDoubt { .. }, ..
                        }),
                    "physical phase {phase} mode {mode}: {}", append_failure_description(&actual)
                );
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                let after = {
                    let mut lease = journal.test_observe_lease().borrow_mut();
                    assert!(matches!(
                        lease.read(),
                        Err(crate::resumable_effects::owned_frame::OwnedFrameError::InDoubt)
                    ));
                    let bytes = lease.test_persisted_snapshot().unwrap();
                    assert!(matches!(
                        lease.read(),
                        Err(crate::resumable_effects::owned_frame::OwnedFrameError::InDoubt)
                    ));
                    bytes
                };
                if mode == 0 {
                    assert_eq!(after, before);
                } else {
                    assert!(after.len() > before.len());
                    assert!(after.starts_with(&before));
                }
                if phase == 0 {
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                } else {
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                }
                drop(actual);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}
#[test]
#[cfg(unix)]
fn failed_observe_state_cleanup_started_pinloss_restore_never_revives_release() {
    use std::os::unix::fs::MetadataExt;
    with_failed(true, |journal, failed, _, _, _, directory| {
        let (started, weak) = start(journal, failed);
        let before = journal.test_observe_lease().borrow_mut().read().unwrap();
        let identity = journal.hold().unwrap().registration().identity();
        let matches: Vec<_> = std::fs::read_dir(directory.unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                std::fs::symlink_metadata(p).is_ok_and(|m| {
                    m.is_file() && m.dev() == identity.file_device && m.ino() == identity.file_inode
                })
            })
            .collect();
        assert_eq!(matches.len(), 1, "exact retained journal entry");
        let file = &matches[0];
        let displaced = file.with_extension("failed-observe-displaced");
        std::fs::rename(file, &displaced).unwrap();
        std::fs::write(file, b"replacement").unwrap();
        let mut state_work = 0;
        let retained = started
            .release(|_| state_work += 1)
            .err()
            .expect("detected pinned entry substitution");
        assert_eq!(state_work, 0);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        std::fs::remove_file(file).unwrap();
        std::fs::rename(displaced, file).unwrap();
        assert_eq!(
            journal.test_observe_lease().borrow_mut().read().unwrap(),
            before
        );
        assert!(journal.hold().is_err());
        assert!(journal.begin_session().is_err());
        let LiveFailedObserveStateFailureV8::Started { owner, .. } = retained else {
            panic!("actual un-released owner retained")
        };
        assert!(owner.release(|_| state_work += 1).is_err());
        assert_eq!(state_work, 0);
    });
}
#[test]
fn failed_observe_state_cleanup_wrong_container_preflight_is_zero_io() {
    with_failed(true, |journal, failed, weak, _, _, _| {
        let before = journal.test_observe_lease().borrow_mut().read().unwrap();
        let obligation = failed
            .prepare_failed_state_cleanup()
            .unwrap_or_else(|_| panic!("actual same-container owner"));
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, _| {
                let other = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let old = other.test_observe_lease().borrow_mut().read().unwrap();
                let retained = other
                    .begin_session()
                    .unwrap()
                    .append_failed_observe_state(obligation)
                    .err()
                    .expect("wrong actual container");
                assert_eq!(other.test_observe_lease().borrow_mut().read().unwrap(), old);
                assert_eq!(
                    journal.test_observe_lease().borrow_mut().read().unwrap(),
                    before
                );
                assert!(other.hold().is_ok());
                assert!(journal.hold().is_ok());
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(retained);
            },
        );
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn failed_observe_state_cleanup_fresh_mac_drift_preserves_actual_history_and_never_grants_work() {
    with_failed(true, |journal, failed, _, _, _, _| {
        let (started, weak) = start(journal, failed);
        let released = receipt(
            journal,
            started
                .release(|_| {})
                .unwrap_or_else(|_| panic!("actual release")),
        );
        let stopped = journal
            .begin_session()
            .unwrap()
            .append_failed_observe_state(
                released
                    .prepare_stop()
                    .unwrap_or_else(|_| panic!("sticky selected Stop")),
            )
            .unwrap_or_else(|_| panic!("true Stop ACK"))
            .advance_failed_observe_state()
            .unwrap_or_else(|_| panic!("stopped actual owner"));
        let current = journal.begin_session().unwrap();
        let baseline: Vec<_> = current
            .test_observe_inventory()
            .test_observe_entries()
            .iter()
            .map(|x| x.entry.clone())
            .collect();
        let start = baseline
            .iter()
            .position(|e| {
                matches!(
                    e,
                    EntryV8::Owned(OwnedBodyV8::OwnedCleanupStarted {
                        owner: journal_model::OwnerV8::State,
                        ..
                    })
                )
            })
            .unwrap();
        let settled = baseline
            .iter()
            .position(|e| {
                matches!(
                    e,
                    EntryV8::Owned(OwnedBodyV8::OwnedCleanupSettled {
                        owner: journal_model::OwnerV8::State,
                        ..
                    })
                )
            })
            .unwrap();
        let observe = baseline
            .iter()
            .position(|e| {
                matches!(
                    e,
                    EntryV8::Owned(OwnedBodyV8::OwnedObserveSettled {
                        settlement: journal_model::ObserveSettlementV8::Failed { .. },
                        ..
                    })
                )
            })
            .unwrap();
        let key = crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]);
        let encode = |rows: &[EntryV8]| {
            let mut mac = "0".repeat(64);
            let mut document = Vec::new();
            for (seq, row) in rows.iter().enumerate() {
                let expected =
                    crate::live_invocation::source_journal::owned_wait_v8::ExpectedRowV8 {
                        invocation: journal.context().ordinary().invocation(),
                        generation: journal.context().generation(),
                        seq: u32::try_from(seq).unwrap(),
                        prev_mac: &mac,
                        ordinary: journal.context().ordinary(),
                    };
                let bytes = wire::encode(row, &expected, &key).unwrap();
                mac = serde_json::from_slice::<Json>(&bytes).unwrap()["authentication"]
                    .as_str()
                    .unwrap()
                    .into();
                document.extend(bytes);
            }
            document
        };
        let validate = |bytes: &[u8]| {
            crate::live_invocation::source_journal::owned_wait_v8::inventory::checked_inventory_v8(
                journal.context(),
                &journal.test_observe_lease().borrow(),
                &key,
                bytes,
            )
            .is_ok()
        };
        assert!(
            validate(&encode(&baseline)),
            "positive authenticated current-context full cleanup history"
        );
        let before = journal.test_observe_lease().borrow_mut().read().unwrap();
        for mode in 0..12 {
            let mut rows = baseline.clone();
            if mode <= 2 {
                let EntryV8::Owned(OwnedBodyV8::OwnedCleanupStarted {
                    basis,
                    terminal,
                    operations,
                    operations_digest,
                    ..
                }) = &mut rows[start]
                else {
                    panic!()
                };
                match mode {
                    0 => *basis += 1,
                    1 => terminal["language_status"]["code"] = json!("foreign_status"),
                    _ => {
                        // The genuine baseline has one objective Bytes leaf.
                        // Duplicate its exact canonical action: a reminted digest
                        // must not make a second physical release admissible.
                        assert_eq!(operations.as_array().unwrap().len(), 1);
                        let duplicate = operations[0].clone();
                        operations.as_array_mut().unwrap().push(duplicate);
                    }
                }
                *operations_digest=wire::recipe_digest(wire::RecipeV8::Operations,&json!({"owner":"state","basis":basis,"terminal":terminal,"operations":operations})).unwrap();
            } else if mode <= 5 {
                let EntryV8::Owned(OwnedBodyV8::OwnedCleanupSettled {
                    started,
                    receipt,
                    receipt_digest,
                    ..
                }) = &mut rows[settled]
                else {
                    panic!()
                };
                match mode {
                    3 => *started += 1,
                    4 => receipt["operations"][0]["outcome"] = json!("failed"),
                    _ => {
                        assert_eq!(receipt["operations"].as_array().unwrap().len(), 1);
                        let duplicate = receipt["operations"][0].clone();
                        receipt["operations"]
                            .as_array_mut()
                            .unwrap()
                            .push(duplicate);
                    }
                }
                *receipt_digest = wire::recipe_digest(wire::RecipeV8::Receipt, receipt).unwrap();
            } else if mode == 6 {
                let EntryV8::Owned(OwnedBodyV8::OwnedObserveSettled { state_digest, .. }) =
                    &mut rows[observe]
                else {
                    panic!()
                };
                let last = state_digest.pop().unwrap();
                state_digest.push(if last == '0' { '1' } else { '0' });
            } else if mode == 7 {
                let EntryV8::Owned(OwnedBodyV8::OwnedRunCreated { binding, .. }) = &mut rows[0]
                else {
                    panic!()
                };
                let last = binding.pop().unwrap();
                binding.push(if last == '0' { '1' } else { '0' });
            } else if mode == 8 {
                let EntryV8::Owned(OwnedBodyV8::OwnedRunCreated { scope, .. }) = &mut rows[0]
                else {
                    panic!()
                };
                scope["policy_epoch"] = json!(scope["policy_epoch"].as_u64().unwrap() + 1);
            } else {
                let EntryV8::Ordinary(SourceJournalEntry::Stop { turn, attempt, .. }) =
                    rows.last_mut().unwrap()
                else {
                    panic!()
                };
                match mode {
                    9 => *turn = Some(1),
                    10 => *turn = None,
                    _ => *attempt = Some(0),
                }
            }
            assert!(!validate(&encode(&rows)), "reminted drift mode {mode}");
            assert_eq!(
                journal.test_observe_lease().borrow_mut().read().unwrap(),
                before
            );
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        }
        drop(stopped);
    });
}
