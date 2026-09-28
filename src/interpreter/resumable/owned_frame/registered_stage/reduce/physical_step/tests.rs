//! Actual root tests under genuine runtime/store; the committed envelope
//! producers here bypass pending §23 journal ACKs and are test-only.
use super::super::super::effect::{with_staged_complete_reduce_v2, with_staged_effect_reduce_v2};
use super::*;

fn committed(staged: StagedExecutedOwnedReduceV2<'_>) -> CommittedExecutedOwnedReduceCleanupV2<'_> {
    CommittedExecutedOwnedReduceCleanupV2 {
        staged,
        started: 29,
        observations: Vec::new(),
    }
}
fn transfer(ready: ReadyExecutedOwnedStepV2<'_>) -> CommittedExecutedOwnedStepTransferV2<'_> {
    assert_eq!(ready.cleanup_started, 29);
    CommittedExecutedOwnedStepTransferV2 {
        ready,
        reserved: 31,
    }
}
fn ready(settled: ExecutedOwnedReduceSettledV2<'_>) -> ReadyExecutedOwnedStepV2<'_> {
    match settled {
        ExecutedOwnedReduceSettledV2::Ready(r) => r,
        _ => panic!("expected actual Step"),
    }
}

#[test]
fn owned_reduce_held_step_moves_original_state_backing_to_report_and_retains_store() {
    with_staged_complete_reduce_v2(1000, |staged, weak, outcome, _, _| {
        assert!(staged.failure().is_none());
        let mut observed = 0;
        let ready = ready(
            settle_executed_owned_reduce_v2(
                committed(staged),
                || true,
                |_| {
                    observed += 1;
                    assert_eq!(weak[0].strong_count(), 1);
                    assert!(outcome.upgrade().is_none(), "drop precedes observation");
                },
            )
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic)),
        );
        assert_eq!(observed, 1);
        assert!(ready.validate_store());
        assert_eq!(ready.observations().len(), 1);
        assert!(ready.observations()[0].succeeded);
        let held = consume_executed_owned_step_v2(transfer(ready), || true)
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
        assert_eq!(held.kind(), "complete");
        assert_eq!(held.causal_refs(), (27, 31));
        assert!(held.validate_store());
        let Some(OwnedStepTransferV2::Complete(report)) = held.owner.as_ref() else {
            panic!()
        };
        let Value::Record(root) = report.root.as_ref().unwrap() else {
            panic!()
        };
        let bytes = root
            .fields
            .values()
            .find_map(|v| match v {
                Value::Bytes(b) => Some(&b.bytes),
                _ => None,
            })
            .unwrap();
        assert!(
            Arc::ptr_eq(bytes, &weak[0].upgrade().unwrap()),
            "actual moved backing"
        );
        assert_eq!(weak[0].strong_count(), 1);
        drop(held);
        assert!(weak[0].upgrade().is_none());
    });
}

#[test]
fn owned_reduce_failure_preserves_each_mixed_observer_outcome_in_compiler_order() {
    for panic_at in 0..2 {
        with_staged_effect_reduce_v2(1, |staged, weak, outcome, _, _| {
            assert_eq!(staged.failure(), Some(&OwnedFrameFailure::FuelExhausted));
            let mut n = 0;
            let settled = settle_executed_owned_reduce_v2(
                committed(staged),
                || true,
                |_| {
                    let index = n;
                    n += 1;
                    if index == panic_at {
                        panic!("actual post-drop observer");
                    }
                },
            )
            .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            let ExecutedOwnedReduceSettledV2::Failed(failed) = settled else {
                panic!()
            };
            assert_eq!(n, 2, "both real input Bytes owners released");
            assert_eq!(failed.failure(), &OwnedFrameFailure::FuelExhausted);
            assert_eq!(failed.operations().len(), 2);
            assert!(!failed.observations_succeeded());
            assert_eq!(
                failed
                    .observations()
                    .iter()
                    .map(|o| o.operation.clone())
                    .collect::<Vec<_>>(),
                failed.operations()
            );
            assert_eq!(
                failed
                    .observations()
                    .iter()
                    .map(|o| o.succeeded)
                    .collect::<Vec<_>>(),
                (0..2).map(|i| i != panic_at).collect::<Vec<_>>()
            );
            assert!(failed.validate_store());
            assert!(weak[0].upgrade().is_none());
            assert!(outcome.upgrade().is_none());
        });
    }
}

#[test]
fn owned_reduce_cleanup_authority_loss_retains_partial_receipt_and_forbids_retry() {
    with_staged_effect_reduce_v2(1, |staged, weak, outcome, _, _| {
        let authority = std::cell::Cell::new(true);
        let rejected = settle_executed_owned_reduce_v2(
            committed(staged),
            || authority.get(),
            |_| authority.set(false),
        )
        .err()
        .expect("post-drop guard loss");
        assert_eq!(rejected.committed.observations.len(), 1);
        assert!(rejected.committed.observations[0].succeeded);
        assert!(rejected.committed.staged.staged.settlement_started);
        let mut retried = 0;
        let rejected =
            settle_executed_owned_reduce_v2(rejected.committed, || true, |_| retried += 1)
                .err()
                .expect("physical settlement cannot retry");
        assert_eq!(retried, 0);
        assert_eq!(rejected.committed.observations.len(), 1);
        assert_eq!(weak[0].strong_count() + outcome.strong_count(), 1);
        drop(rejected);
    });
}

#[test]
fn owned_reduce_cancel_after_cleanup_start_still_releases_but_blocks_step_move() {
    with_staged_effect_reduce_v2(1000, |staged, weak, outcome, cancellation, _| {
        cancellation.cancel();
        let mut observed = 0;
        let ready = ready(
            settle_executed_owned_reduce_v2(committed(staged), || true, |_| observed += 1)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic)),
        );
        assert_eq!(observed, 1);
        assert!(outcome.upgrade().is_none());
        let rejected = consume_executed_owned_step_v2(transfer(ready), || true)
            .err()
            .expect("cancelled holder cannot publish result");
        assert_eq!(weak[0].strong_count(), 1);
        drop(rejected);
    });
}

#[cfg(unix)]
#[test]
fn owned_reduce_normal_poisoned_or_foreign_drop_disarms_semantic_disposal() {
    for mode in 0..3 {
        with_staged_complete_reduce_v2(1000, |staged, weak, _, _, directory| {
            let identity = staged.inputs.store.registration().identity();
            let ready = ready(
                settle_executed_owned_reduce_v2(committed(staged), || true, |_| {})
                    .unwrap_or_else(|e| panic!("{:?}", e.diagnostic)),
            );
            let mut held = consume_executed_owned_step_v2(transfer(ready), || true)
                .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
            if mode == 2 {
                held.creator = held.creator.wrapping_add(1);
            } else if mode == 1 {
                let journal =
                    pinned_journal_entry(directory, (identity.file_device, identity.file_inode));
                let displaced = directory.join("physical-step-displaced");
                std::fs::rename(&journal, &displaced).unwrap();
                assert!(!held.validate_store());
                std::fs::rename(&displaced, &journal).unwrap();
                assert!(
                    !held.validate_store(),
                    "restoring path cannot revive poisoned borrower"
                );
            }
            held.discard_unpublished_backing();
            let Some(OwnedStepTransferV2::Complete(report)) = &held.owner else {
                panic!()
            };
            assert!(
                report.root.is_none(),
                "inner semantic Drop cannot see backing"
            );
            assert!(weak[0].upgrade().is_none());
        });
    }
}

#[cfg(unix)]
fn pinned_journal_entry(directory: &std::path::Path, identity: (u64, u64)) -> std::path::PathBuf {
    use std::os::unix::fs::MetadataExt;
    let matches: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let metadata = std::fs::symlink_metadata(path).unwrap();
            metadata.is_file() && (metadata.dev(), metadata.ino()) == identity
        })
        .collect();
    assert_eq!(matches.len(), 1, "exact retained journal file pin");
    matches.into_iter().next().unwrap()
}
