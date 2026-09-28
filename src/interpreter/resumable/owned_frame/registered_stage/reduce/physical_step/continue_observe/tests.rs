//! Genuine baseline Continue/E/store and actual effect; all ACK envelope
//! construction is test-only, not proof of an enabled §23 producer/driver.
use super::super::super::super::effect::{
    with_staged_effect_reduce_v2, with_staged_task_zero_reduce_v2,
};
use super::*;

fn with_continue(
    callback: impl FnOnce(
        CommittedContinueObserveV2<'_>,
        std::sync::Weak<[u8]>,
        &crate::agent_runtime::AgentCancellation,
        &std::path::Path,
    ),
) {
    with_continue_fixture(false, callback)
}
fn with_continue_fixture(
    task_zero: bool,
    callback: impl FnOnce(
        CommittedContinueObserveV2<'_>,
        std::sync::Weak<[u8]>,
        &crate::agent_runtime::AgentCancellation,
        &std::path::Path,
    ),
) {
    let exercise = |staged: super::super::super::StagedExecutedOwnedReduceV2<'_>,
                    weak: [std::sync::Weak<[u8]>; 2],
                    outcome: std::sync::Weak<[u8]>,
                    cancel: &crate::agent_runtime::AgentCancellation,
                    directory: &std::path::Path| {
        assert!(staged.inputs.execution.ordinary().max_iterations() >= 2);
        let cleanup = CommittedExecutedOwnedReduceCleanupV2 {
            staged,
            started: 29,
            observations: Vec::new(),
        };
        let ExecutedOwnedReduceSettledV2::Ready(ready) =
            settle_executed_owned_reduce_v2(cleanup, || true, |_| {})
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic))
        else {
            panic!("actual Continue Step");
        };
        let held = consume_executed_owned_step_v2(
            CommittedExecutedOwnedStepTransferV2 {
                ready,
                reserved: 31,
            },
            || true,
        )
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        assert_eq!(held.kind(), "continue");
        assert!(outcome.upgrade().is_none());
        let Some(OwnedStepTransferV2::Continue(state)) = held.owner.as_ref() else {
            panic!()
        };
        let Value::Record(root) = state.root.as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(
            root.fields[&crate::hir::DeclarationId::new("fixture.agent.type.state.budget")],
            Value::Int(10)
        );
        assert_eq!(
            root.fields[&crate::hir::DeclarationId::new("fixture.agent.type.state.epoch")],
            Value::Int(1)
        );
        callback(
            CommittedContinueObserveV2 {
                held,
                transition: 33,
                reservation: 34,
                turn: 1,
                fuel: 1000,
            },
            weak[0].clone(),
            cancel,
            directory,
        );
    };
    if task_zero {
        with_staged_task_zero_reduce_v2(1000, exercise)
    } else {
        with_staged_effect_reduce_v2(1000, exercise)
    }
}

#[test]
fn owned_continue_observe_uses_actual_moved_state_once_and_returns_ordered_copy_data() {
    with_continue(|committed, weak, _, _| {
        let mut fuel = OwnedFrameBudget::new(1000).unwrap();
        let mut guard_calls = 0;
        let result = observe_continued_owned_state_v2(committed, &mut fuel, || {
            guard_calls += 1;
            true
        })
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let ContinuedOwnedObserveV2::Observed(observed) = result else {
            panic!()
        };
        assert_eq!(
            guard_calls, 2,
            "one pre and one post guard around the sole Observe call"
        );
        assert_eq!(observed.turn(), 1);
        assert_eq!(observed.causal_refs(), (33, 34));
        assert_eq!(observed.consumed(), fuel.consumed());
        assert!(observed.consumed() > 0 && observed.consumed() < 1000);
        assert_eq!(
            observed.observation(),
            &ResumableChannelValue::Record {
                declaration: crate::hir::DeclarationId::new("fixture.agent.type.observation"),
                fields: vec![ArgumentValue::Int(10), ArgumentValue::Int(1)],
            }
        );
        assert!(observed.validate_store());
        assert_eq!(
            weak.strong_count(),
            1,
            "same backing, no retained Observe alias"
        );
        drop(observed);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn owned_continue_observe_refuses_wrong_funding_spent_budget_turn_and_foreign_process() {
    for mode in 0..6 {
        with_continue(|mut committed, weak, _, _| {
            let mut fuel = OwnedFrameBudget::new(1000).unwrap();
            match mode {
                0 => committed.fuel = 999,
                1 => {
                    fuel.remaining = 999;
                    fuel.consumed = 1;
                }
                2 => committed.turn = 2,
                3 => committed.held.creator = committed.held.creator.wrapping_add(1),
                4 => committed.reservation = committed.transition,
                5 => committed.held.inputs.as_mut().unwrap().turn = u32::MAX,
                _ => unreachable!(),
            }
            let before = (fuel.remaining, fuel.consumed);
            let mut calls = 0;
            let rejected = observe_continued_owned_state_v2(committed, &mut fuel, || {
                calls += 1;
                true
            })
            .err()
            .expect("funding/turn/creator refusal before Observe");
            assert_eq!((fuel.remaining, fuel.consumed), before);
            assert_eq!(calls, 0);
            assert_eq!(weak.strong_count(), 1);
            drop(rejected);
            assert!(weak.upgrade().is_none());
        });
    }
}

#[test]
fn owned_continue_observe_pre_cancel_refuses_and_post_cancel_quarantines_actual_owner() {
    for post in [false, true] {
        with_continue(|committed, weak, cancellation, _| {
            let mut fuel = OwnedFrameBudget::new(1000).unwrap();
            if !post {
                cancellation.cancel();
            }
            let mut calls = 0;
            let result = observe_continued_owned_state_v2(committed, &mut fuel, || {
                calls += 1;
                if post && calls == 2 {
                    cancellation.cancel();
                }
                true
            });
            if post {
                let ContinuedOwnedObserveV2::Quarantined(tail) =
                    result.unwrap_or_else(|e| panic!("{:?}", e.diagnostic))
                else {
                    panic!()
                };
                assert!(fuel.consumed() > 0);
                assert_eq!(calls, 2);
                assert!(matches!(&tail.step, OwnedObserveStepV2::Observed(_)));
                assert_eq!(weak.strong_count(), 1);
                drop(tail);
            } else {
                let rejection = result.err().expect("cancel before Observe");
                assert_eq!(fuel.consumed(), 0);
                assert_eq!(calls, 0);
                assert_eq!(weak.strong_count(), 1);
                drop(rejection);
            }
            assert!(weak.upgrade().is_none());
        });
    }
}

#[cfg(unix)]
#[test]
fn owned_continue_observe_post_callback_pin_loss_is_sticky_and_retains_backing_only() {
    with_continue(|committed, weak, _, directory| {
        let mut fuel = OwnedFrameBudget::new(1000).unwrap();
        let mut calls = 0;
        let identity = committed
            .held
            .inputs
            .as_ref()
            .unwrap()
            .store
            .registration()
            .identity();
        let journal = pinned_journal_entry(directory, identity);
        let displaced = directory.join("continue-displaced");
        let result = observe_continued_owned_state_v2(committed, &mut fuel, || {
            calls += 1;
            if calls == 2 {
                std::fs::rename(&journal, &displaced).unwrap();
            }
            true
        })
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        let ContinuedOwnedObserveV2::Quarantined(tail) = result else {
            panic!()
        };
        assert_eq!(calls, 2);
        assert!(fuel.consumed() > 0);
        assert!(!tail.context.store.validate_guard().is_ok());
        std::fs::rename(displaced, &journal).unwrap();
        assert!(
            !tail.context.store.validate_guard().is_ok(),
            "path restoration grants no revival"
        );
        assert_eq!(weak.strong_count(), 1);
        drop(tail);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn owned_continue_observe_aliased_state_diagnostic_preserves_owner_without_evaluation() {
    with_continue(|committed, weak, _, _| {
        let Some(OwnedStepTransferV2::Continue(state)) = committed.held.owner.as_ref() else {
            panic!()
        };
        let Some(Value::Record(record)) = state.root.as_ref() else {
            panic!()
        };
        let alias = Arc::clone(record);
        let mut fuel = OwnedFrameBudget::new(1000).unwrap();
        let rejected = observe_continued_owned_state_v2(committed, &mut fuel, || true)
            .err()
            .expect("actual exclusive State required");
        assert_eq!(fuel.consumed(), 0);
        assert_eq!(weak.strong_count(), 1);
        assert!(matches!(
            &rejected.committed.held.owner,
            Some(OwnedStepTransferV2::Continue(_))
        ));
        drop(alias);
        drop(rejected);
        assert!(weak.upgrade().is_none());
    });
}

#[cfg(unix)]
fn pinned_journal_entry(
    directory: &std::path::Path,
    identity: crate::resumable_effects::owned_frame::OwnedFrameStoreIdentity,
) -> std::path::PathBuf {
    use std::os::unix::fs::MetadataExt;
    let matches: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let metadata = std::fs::symlink_metadata(path).unwrap();
            metadata.is_file()
                && (metadata.dev(), metadata.ino()) == (identity.file_device, identity.file_inode)
        })
        .collect();
    assert_eq!(matches.len(), 1, "exact retained journal file pin");
    matches.into_iter().next().unwrap()
}

#[test]
fn owned_continue_observe_does_not_treat_authored_task_budget_as_host_iteration_limit() {
    with_continue_fixture(true, |committed, weak, _, _| {
        let inputs = committed.held.inputs.as_ref().unwrap();
        assert_eq!(
            inputs
                .runtime
                .owned_wait_task_v8(inputs.execution)
                .unwrap()
                .budget,
            0
        );
        assert_eq!(inputs.execution.ordinary().max_iterations(), 32);
        let mut fuel = OwnedFrameBudget::new(1000).unwrap();
        let ContinuedOwnedObserveV2::Observed(observed) =
            observe_continued_owned_state_v2(committed, &mut fuel, || true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic))
        else {
            panic!("authored Task budget is not a host limit")
        };
        assert_eq!(observed.turn(), 1);
        assert!(observed.consumed() > 0);
        assert!(observed.validate_store());
        assert_eq!(weak.strong_count(), 1);
        drop(observed);
        assert!(weak.upgrade().is_none());
    });
}
