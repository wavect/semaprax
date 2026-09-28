use super::*;
use std::path::Path;

fn state() -> Value {
    serde_json::json!({"declaration":"fixture.agent.type.state","fields":[
        {"identity":"fixture.agent.type.state.objective","value":{"kind":"bytes","hex":"00"}},
        {"identity":"fixture.agent.type.state.budget","value":{"tag":"i64","value":10}},
        {"identity":"fixture.agent.type.state.epoch","value":{"tag":"i64","value":0}}]})
}
fn rows(context: &CheckedOwnedWaitJournalContextV8) -> [EntryV8; 3] {
    let state = state();
    [
        EntryV8::Owned(context.fold().created.clone()),
        EntryV8::Ordinary(SourceJournalEntry::RunOpened),
        EntryV8::Owned(model::OwnedBodyV8::OwnedStateCommitted {
            turn: 0,
            argument_digest: crate::live_invocation::identity::digest(
                b"semaprax.source-owned-frame-args.v2\0",
                &wire::canonical(&state),
            ),
            state,
            cleanup_plan_digest: context.fold().cleanup_plan_digest.clone(),
        }),
    ]
}
fn physical_path(directory: &Path) -> std::path::PathBuf {
    let entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(entries.len(), 1, "exact test journal inventory");
    entries.into_iter().next().unwrap()
}
fn success<'a>(result: Result<AppendSessionV8<'a>, AppendFailureV8<'a>>) -> AppendSessionV8<'a> {
    match result {
        Ok(session) => session,
        Err(_) => panic!("actual append unexpectedly refused"),
    }
}

#[test]
fn owned_wait_physical_append_acks_exact_authenticated_same_file_prefix() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let expected = context.test_state_document(&key, state());
            let rows = rows(&context);
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let held = journal.hold().unwrap();
            assert_eq!(held.generation(), held.registration().generation());
            let mut session = journal.begin_session().unwrap();
            for row in rows {
                session = success(session.append(row));
                held.validate_guard().unwrap();
            }
            assert_eq!(session.sequence(), 3);
            assert_eq!(session.acknowledged_bytes(), expected.len());
            assert_eq!(std::fs::read(physical_path(directory)).unwrap(), expected);
            assert_eq!(
                journal.begin_session().unwrap().sequence(),
                3,
                "recovery reads actual held file"
            );
            journal.append_active.set(true);
            assert_eq!(held.validate_guard(), Err(SourceJournalError::Order));
            journal.append_active.set(false);
            held.validate_guard().unwrap();
        },
    );
}

#[test]
fn owned_wait_physical_append_missing_retention_is_known_prewrite_and_sticky() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        false,
        |context, lease, key, directory| {
            let row = rows(&context)[0].clone();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let held = journal.hold().unwrap();
            let failure = journal.begin_session().unwrap().append(row);
            assert!(matches!(
                failure,
                Err(AppendFailureV8::PrewriteRefused { .. })
            ));
            assert_eq!(std::fs::read(physical_path(directory)).unwrap(), b"");
            assert_eq!(held.validate_guard(), Err(SourceJournalError::Poisoned));
            assert!(journal.begin_session().is_err());
        },
    );
}

#[test]
fn owned_wait_physical_append_stage_faults_never_mint_ack_or_retry() {
    for stage in 0..4 {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, mut lease, key, directory| {
                let row = rows(&context)[0].clone();
                let expected = context.test_state_document(&key, state());
                let first = expected
                    .split_inclusive(|b| *b == b'\n')
                    .next()
                    .unwrap()
                    .to_vec();
                match stage {
                    0 => lease.test_fail_before_write(1),
                    1 => lease.test_fail_after_write(1),
                    2 => lease.test_fail_before_sync(1),
                    3 => lease.test_fail_after_sync(1),
                    _ => unreachable!(),
                }
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let held = journal.hold().unwrap();
                let failure = journal.begin_session().unwrap().append(row);
                assert!(matches!(failure, Err(AppendFailureV8::InDoubt { .. })));
                assert_eq!(
                    std::fs::read(physical_path(directory)).unwrap(),
                    if stage == 0 { vec![] } else { first }
                );
                assert_eq!(held.validate_guard(), Err(SourceJournalError::Poisoned));
                assert!(journal.begin_session().is_err());
                assert!(!journal.append_active.get());
            },
        );
    }
}

#[test]
fn owned_wait_physical_append_postwrite_unwind_retains_poison_and_backing() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let row = rows(&context)[0].clone();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let held = journal.hold().unwrap();
            journal.panic_after_append.set(true);
            let failure = journal.begin_session().unwrap().append(row);
            assert!(matches!(failure, Err(AppendFailureV8::InDoubt { .. })));
            assert!(!std::fs::read(physical_path(directory)).unwrap().is_empty());
            assert_eq!(held.validate_guard(), Err(SourceJournalError::Poisoned));
            assert!(journal.begin_session().is_err());
            assert!(!journal.append_active.get());
        },
    );
}

#[test]
fn owned_wait_physical_append_pure_rejection_preserves_session_and_zero_write() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let row = rows(&context)[0].clone();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let rejected = journal
                .begin_session()
                .unwrap()
                .append(EntryV8::Ordinary(SourceJournalEntry::RunOpened));
            let session = match rejected {
                Err(AppendFailureV8::CandidateRefused { session, error, .. }) => {
                    assert_eq!(error, SourceJournalError::Order);
                    session
                }
                _ => panic!("invalid first row must be pure refusal"),
            };
            assert_eq!(std::fs::read(physical_path(directory)).unwrap(), b"");
            journal.hold().unwrap().validate_guard().unwrap();
            assert_eq!(success(session.append(row)).sequence(), 1);
        },
    );
}

#[test]
fn owned_wait_physical_append_stale_session_detects_actual_prefix_before_write() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, directory| {
            let row = rows(&context)[0].clone();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let old = journal.begin_session().unwrap();
            let live = success(journal.begin_session().unwrap().append(row.clone()));
            let before = std::fs::read(physical_path(directory)).unwrap();
            assert!(matches!(
                old.append(row),
                Err(AppendFailureV8::PrewriteRefused { .. })
            ));
            assert_eq!(std::fs::read(physical_path(directory)).unwrap(), before);
            assert!(live
                .append(EntryV8::Ordinary(SourceJournalEntry::RunOpened))
                .is_err());
            assert!(journal.hold().is_err());
        },
    );
}

#[test]
fn owned_wait_physical_append_replaced_file_and_foreign_pid_are_prewrite_refusals() {
    for foreign in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let row = rows(&context)[0].clone();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let session = journal.begin_session().unwrap();
                let path = physical_path(directory);
                if foreign {
                    journal.lease.borrow_mut().test_mark_foreign();
                } else {
                    std::fs::rename(&path, path.with_extension("retained")).unwrap();
                    std::fs::write(&path, b"replacement").unwrap();
                }
                // Candidate's closed validation may reject known changed pins first.
                assert!(session.append(row).is_err());
                assert!(journal.hold().is_err());
                if !foreign {
                    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
                }
            },
        );
    }
}

#[test]
fn owned_wait_physical_append_uncertainty_retains_exclusive_lock_until_container_drop() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, mut lease, key, directory| {
            let row = rows(&context)[0].clone();
            lease.test_fail_after_sync(1);
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let held = journal.hold().unwrap();
            let failure = journal.begin_session().unwrap().append(row);
            assert!(matches!(failure, Err(AppendFailureV8::InDoubt { .. })));
            let path = physical_path(directory);
            let durable = std::fs::read(&path).unwrap();
            assert!(!durable.is_empty());
            let competitor = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            assert!(rustix::fs::flock(
                &competitor,
                rustix::fs::FlockOperation::NonBlockingLockExclusive
            )
            .is_err());
            drop(failure);
            assert!(
                rustix::fs::flock(
                    &competitor,
                    rustix::fs::FlockOperation::NonBlockingLockExclusive
                )
                .is_err(),
                "failure disposal cannot release container lock"
            );
            drop(held);
            assert!(rustix::fs::flock(
                &competitor,
                rustix::fs::FlockOperation::NonBlockingLockExclusive
            )
            .is_err());
            drop(journal);
            rustix::fs::flock(
                &competitor,
                rustix::fs::FlockOperation::NonBlockingLockExclusive,
            )
            .unwrap();
            assert_eq!(
                std::fs::read(&path).unwrap(),
                durable,
                "backing-only Drop leaves exact uncertain physical bytes"
            );
        },
    );
}
