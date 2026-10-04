use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::durable_jobs::durable_fs::HookPoint;
use crate::durable_jobs::model::RetryPolicy;

fn tempdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "semaprax-durable-jobs-store-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn policy(max_attempts: u32) -> RetryPolicy {
    RetryPolicy::new(max_attempts, 1, 100)
}

fn request(key: &[u8], payload: &[u8], max_attempts: u32, now_tick: u64) -> EnqueueRequest {
    EnqueueRequest {
        idempotency_key: key.to_vec(),
        payload: payload.to_vec(),
        retry_policy: policy(max_attempts),
        now_tick,
    }
}

const WRITER_LOCK_CHILD: &str = "SEMAPRAX_DURABLE_JOB_WRITER_LOCK_CHILD";
const WRITER_LOCK_ROOT: &str = "SEMAPRAX_DURABLE_JOB_WRITER_LOCK_ROOT";
const WRITER_LOCK_READY: &str = "SEMAPRAX_DURABLE_JOB_WRITER_LOCK_READY";
const WRITER_LOCK_RELEASE: &str = "SEMAPRAX_DURABLE_JOB_WRITER_LOCK_RELEASE";

fn wait_for_file(path: &std::path::Path, description: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn writer_lock_child() {
    if std::env::var_os(WRITER_LOCK_CHILD).is_none() {
        return;
    }
    let root = PathBuf::from(std::env::var_os(WRITER_LOCK_ROOT).unwrap());
    let ready = PathBuf::from(std::env::var_os(WRITER_LOCK_READY).unwrap());
    let release = PathBuf::from(std::env::var_os(WRITER_LOCK_RELEASE).unwrap());
    let mut store = GenerationJobStore::open(&root).unwrap();
    let EnqueueOutcome::Created(job_id) = store
        .enqueue_with_side_record(
            request(b"child-writer", b"payload", 3, 0),
            b"child-side".to_vec(),
            b"present".to_vec(),
        )
        .unwrap()
    else {
        panic!("child writer must create its job");
    };
    let (claimed, _) = store.claim(7, 0, 10).unwrap();
    assert_eq!(claimed, job_id);
    fs::write(ready, b"ready").unwrap();
    wait_for_file(&release, "parent lock release marker");
}

// ---------------------------------------------------------------------
// Idempotency, including the racing case.
// ---------------------------------------------------------------------

#[test]
fn enqueuing_the_same_idempotency_key_twice_sequentially_executes_once() {
    let dir = tempdir("idempotent-sequential");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    let first = store
        .enqueue(request(b"order-1", b"payload", 3, 0))
        .unwrap();
    let second = store
        .enqueue(request(b"order-1", b"payload", 3, 0))
        .unwrap();
    let EnqueueOutcome::Created(created_id) = first else {
        panic!("expected Created, got {first:?}")
    };
    assert_eq!(second, EnqueueOutcome::Duplicate(created_id));
    assert_eq!(store.table.jobs.len(), 1);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_conflicting_payload_under_the_same_key_is_a_closed_refusal_not_a_merge() {
    let dir = tempdir("idempotent-conflict");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    let EnqueueOutcome::Created(created_id) = store
        .enqueue(request(b"order-1", b"payload-a", 3, 0))
        .unwrap()
    else {
        panic!("expected Created")
    };
    let error = store
        .enqueue(request(b"order-1", b"payload-b", 3, 0))
        .unwrap_err();
    assert_eq!(error, JobStoreError::IdempotencyConflict(created_id));
    assert_eq!(store.table.jobs.len(), 1);
    fs::remove_dir_all(&dir).ok();
}

/// The required racing case: many real OS threads submit the identical
/// idempotency key concurrently. The store's mutating methods take `&mut
/// self`, so callers share it behind a lock exactly the way
/// `src/job_fixture.rs`'s own
/// `real_os_thread_concurrent_claim_race_grants_the_lease_to_exactly_one_worker`
/// does; a `Barrier` hands the actual call-ordering decision to the OS
/// scheduler, not to this test, so which thread's call is authoritative is
/// genuinely nondeterministic — the property under test is that regardless
/// of that order, exactly one job is ever created and every other caller
/// observes the same id as a duplicate, never a second row.
#[test]
fn racing_enqueue_calls_with_the_same_idempotency_key_create_exactly_one_job() {
    let dir = tempdir("idempotent-race");
    let store = Arc::new(Mutex::new(GenerationJobStore::open(&dir).unwrap()));
    const WORKERS: usize = 16;
    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut handles = Vec::new();
    for _ in 0..WORKERS {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let mut store = store.lock().unwrap();
            store
                .enqueue(request(b"racing-key", b"same-payload", 3, 0))
                .unwrap()
        }));
    }
    let outcomes: Vec<EnqueueOutcome> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let created: Vec<JobId> = outcomes
        .iter()
        .filter_map(|o| match o {
            EnqueueOutcome::Created(id) => Some(*id),
            EnqueueOutcome::Duplicate(_) => None,
        })
        .collect();
    assert_eq!(created.len(), 1, "exactly one winner: {outcomes:?}");
    let winner = created[0];
    for outcome in &outcomes {
        match outcome {
            EnqueueOutcome::Created(id) => assert_eq!(*id, winner),
            EnqueueOutcome::Duplicate(id) => assert_eq!(*id, winner),
        }
    }
    drop(store);
    let reopened = GenerationJobStore::open(&dir).unwrap();
    assert_eq!(reopened.table.jobs.len(), 1);
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// Retries under a bounded policy, reaching a terminal state.
// ---------------------------------------------------------------------

#[test]
fn a_retryable_job_that_never_succeeds_is_dead_lettered_at_the_ceiling_not_retried_forever() {
    let dir = tempdir("retry-dead-letter");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    let EnqueueOutcome::Created(job_id) =
        store.enqueue(request(b"flaky", b"payload", 3, 0)).unwrap()
    else {
        panic!("expected Created")
    };
    let mut tick = 0u64;
    let mut last_state = JobState::Pending;
    for attempt in 1..=5u32 {
        let claim = store.claim(1, tick, 100);
        if attempt > 3 {
            // Already dead-lettered; nothing left to claim.
            assert!(matches!(claim, Err(JobStoreError::NothingToClaim)));
            break;
        }
        let (claimed_id, token) = claim.unwrap();
        assert_eq!(claimed_id, job_id);
        store.begin_execution(token, tick).unwrap();
        last_state = store
            .complete(
                token,
                tick,
                AttemptOutcome::Retryable,
                Some(b"boom".to_vec()),
            )
            .unwrap();
        tick += 1;
    }
    assert_eq!(last_state, JobState::DeadLettered);
    assert!(last_state.is_terminal());
    assert_eq!(store.get(job_id).unwrap().attempt, 3);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_job_that_eventually_succeeds_within_the_ceiling_reaches_succeeded() {
    let dir = tempdir("retry-eventual-success");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    let EnqueueOutcome::Created(job_id) = store
        .enqueue(request(b"eventually-fine", b"payload", 5, 0))
        .unwrap()
    else {
        panic!("expected Created")
    };
    for tick in 0..2u64 {
        let (_, token) = store.claim(1, tick, 100).unwrap();
        store.begin_execution(token, tick).unwrap();
        store
            .complete(token, tick, AttemptOutcome::Retryable, None)
            .unwrap();
    }
    let (_, token) = store.claim(1, 10, 100).unwrap();
    store.begin_execution(token, 10).unwrap();
    let state = store
        .complete(token, 10, AttemptOutcome::Success, None)
        .unwrap();
    assert_eq!(state, JobState::Succeeded);
    assert_eq!(store.get(job_id).unwrap().state, JobState::Succeeded);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_permanent_failure_is_terminal_on_the_first_attempt() {
    let dir = tempdir("retry-permanent");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    store
        .enqueue(request(b"doomed", b"payload", 10, 0))
        .unwrap();
    let (_, token) = store.claim(1, 0, 100).unwrap();
    store.begin_execution(token, 0).unwrap();
    let state = store
        .complete(token, 0, AttemptOutcome::Permanent, Some(b"nope".to_vec()))
        .unwrap();
    assert_eq!(state, JobState::PermanentFailure);
    assert!(state.is_terminal());
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// Crash durability: drop the store, reopen from disk, assert survival.
// ---------------------------------------------------------------------

#[test]
fn an_acknowledged_job_survives_a_simulated_crash_and_reopen() {
    let dir = tempdir("crash-durability");
    {
        let mut store = GenerationJobStore::open(&dir).unwrap();
        let EnqueueOutcome::Created(job_id) = store
            .enqueue(request(b"survives", b"payload", 3, 0))
            .unwrap()
        else {
            panic!("expected Created")
        };
        let (_, token) = store.claim(7, 0, 50).unwrap();
        store.begin_execution(token, 0).unwrap();
        let state = store
            .complete(token, 0, AttemptOutcome::Success, None)
            .unwrap();
        assert_eq!(state, JobState::Succeeded);
        assert_eq!(job_id, JobId(1));
        // `store` is dropped here with no explicit flush/close call: every
        // acknowledged mutation above already durably committed inside its
        // own call, which is exactly the property this test exists to
        // check.
    }
    let reopened = GenerationJobStore::open(&dir).unwrap();
    let record = reopened.get(JobId(1)).expect("job present after reopen");
    assert_eq!(record.state, JobState::Succeeded);
    assert_eq!(record.idempotency_key, b"survives");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn reopening_an_empty_directory_starts_a_fresh_empty_store() {
    let dir = tempdir("crash-durability-empty");
    let store = GenerationJobStore::open(&dir).unwrap();
    assert!(store.table.jobs.is_empty());
    assert_eq!(store.current_generation, 0);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn reopening_skips_crash_left_stages_without_deleting_or_promoting_them() {
    let dir = tempdir("crash-left-stages");
    let generations = dir.join("generations");
    fs::create_dir(&generations).unwrap();
    let generation_stage = generations.join(".stage-generation-0");
    let active_stage = dir.join(".stage-active-1");
    fs::write(&generation_stage, b"incomplete-generation").unwrap();
    fs::write(&active_stage, b"incomplete-active").unwrap();

    let mut store = GenerationJobStore::open(&dir).unwrap();
    assert!(matches!(
        store
            .enqueue(request(b"after-crash", b"payload", 3, 0))
            .unwrap(),
        EnqueueOutcome::Created(_)
    ));
    assert_eq!(
        fs::read(&generation_stage).unwrap(),
        b"incomplete-generation"
    );
    assert_eq!(fs::read(&active_stage).unwrap(), b"incomplete-active");

    // Reopening again must still select a fresh stage, even though the
    // preexisting crash artifacts remain intentionally untouched.
    drop(store);
    let mut reopened = GenerationJobStore::open(&dir).unwrap();
    assert!(matches!(
        reopened
            .enqueue(request(b"after-second-reopen", b"payload", 3, 1))
            .unwrap(),
        EnqueueOutcome::Created(_)
    ));
    assert_eq!(reopened.table.jobs.len(), 2);
    assert_eq!(
        fs::read(&generation_stage).unwrap(),
        b"incomplete-generation"
    );
    assert_eq!(fs::read(&active_stage).unwrap(), b"incomplete-active");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_refuses_a_bounded_namespace_of_abandoned_stages() {
    let dir = tempdir("too-many-crash-left-stages");
    let generations = dir.join("generations");
    fs::create_dir(&generations).unwrap();
    for sequence in 0..=1024 {
        fs::write(
            generations.join(format!(".stage-generation-{sequence}")),
            b"uncommitted",
        )
        .unwrap();
    }

    assert!(matches!(
        GenerationJobStore::open(&dir),
        Err(JobStoreError::Io)
    ));
    assert_eq!(
        fs::read(generations.join(".stage-generation-0")).unwrap(),
        b"uncommitted"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn live_writer_excludes_second_handle_and_canonical_alias_until_it_closes() {
    let dir = tempdir("writer-lock");
    let alias = dir.join(".");
    let mut first = GenerationJobStore::open(&dir).unwrap();
    assert!(matches!(
        GenerationJobStore::open(&alias),
        Err(JobStoreError::WriterBusy)
    ));
    assert!(matches!(
        first.enqueue_with_side_record(
            request(b"held-owner", b"payload", 3, 0),
            b"orders-total".to_vec(),
            b"1".to_vec(),
        ),
        Ok(EnqueueOutcome::Created(_))
    ));
    drop(first);

    let reopened = GenerationJobStore::open(&alias).unwrap();
    assert_eq!(reopened.table.jobs.len(), 1);
    assert_eq!(reopened.side_record(b"orders-total"), Some(&b"1".to_vec()));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn child_writer_lock_excludes_parent_then_releases_after_process_exit() {
    let dir = tempdir("writer-lock-child");
    let ready = dir.join("child-ready");
    let release = dir.join("child-release");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "durable_jobs::store::tests::writer_lock_child",
            "--nocapture",
        ])
        .env(WRITER_LOCK_CHILD, "1")
        .env(WRITER_LOCK_ROOT, &dir)
        .env(WRITER_LOCK_READY, &ready)
        .env(WRITER_LOCK_RELEASE, &release)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_file(&ready, "child writer lock");
    assert!(matches!(
        GenerationJobStore::open(&dir),
        Err(JobStoreError::WriterBusy)
    ));

    fs::write(&release, b"release").unwrap();
    assert!(child.wait().unwrap().success(), "child writer failed");
    let mut reopened = GenerationJobStore::open(&dir).unwrap();
    assert_eq!(reopened.table.jobs.len(), 1);
    assert_eq!(
        reopened.side_record(b"child-side"),
        Some(&b"present".to_vec())
    );
    let token = LeaseToken {
        job_id: JobId(1),
        worker_id: 7,
        generation: 1,
    };
    reopened.begin_execution(token, 0).unwrap();
    assert_eq!(
        reopened
            .complete(token, 0, AttemptOutcome::Success, None)
            .unwrap(),
        JobState::Succeeded
    );
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// Determinism: identical operation sequences produce byte-identical
// generations, and job ids are assigned in a fixed, replayable order.
// ---------------------------------------------------------------------

#[test]
fn identical_operation_sequences_produce_byte_identical_generations() {
    let dir_a = tempdir("determinism-a");
    let dir_b = tempdir("determinism-b");
    let mut a = GenerationJobStore::open(&dir_a).unwrap();
    let mut b = GenerationJobStore::open(&dir_b).unwrap();

    for store in [&mut a, &mut b] {
        store
            .enqueue(request(b"job-a", b"payload-a", 3, 5))
            .unwrap();
        store
            .enqueue(request(b"job-b", b"payload-b", 4, 6))
            .unwrap();
        let (_, token) = store.claim(1, 10, 20).unwrap();
        store.begin_execution(token, 10).unwrap();
        store
            .complete(token, 10, AttemptOutcome::Retryable, Some(b"e".to_vec()))
            .unwrap();
    }

    assert_eq!(a.table, b.table);
    assert_eq!(a.current_generation, b.current_generation);
    let bytes_a = fs::read(
        dir_a
            .join("generations")
            .join(a.current_generation.to_string()),
    )
    .unwrap();
    let bytes_b = fs::read(
        dir_b
            .join("generations")
            .join(b.current_generation.to_string()),
    )
    .unwrap();
    assert_eq!(bytes_a, bytes_b);
    fs::remove_dir_all(&dir_a).ok();
    fs::remove_dir_all(&dir_b).ok();
}

#[test]
fn job_ids_are_assigned_in_strictly_increasing_enqueue_order() {
    let dir = tempdir("determinism-ids");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    let mut ids = Vec::new();
    for n in 0..5u8 {
        let key = [n];
        match store.enqueue(request(&key, b"payload", 3, 0)).unwrap() {
            EnqueueOutcome::Created(id) => ids.push(id.0),
            EnqueueOutcome::Duplicate(_) => panic!("unexpected duplicate"),
        }
    }
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// No execution without a current, valid lease.
// ---------------------------------------------------------------------

#[test]
fn completing_with_a_stale_lease_generation_is_refused_and_never_mutates_the_job() {
    let dir = tempdir("lease-stale");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    store.enqueue(request(b"job", b"payload", 3, 0)).unwrap();
    let (job_id, first_token) = store.claim(1, 0, 5).unwrap();
    // The worker's own first lease expires (it went slow, not dead) and the
    // *same* worker id reclaims the job at a later tick. Fencing must come
    // from `lease_epoch` alone, never from `worker_id` equality, or a
    // worker's own late-arriving stale call would satisfy its own new
    // lease by coincidence — exactly the historical bug
    // `docs/DURABLE-JOBS-V1.md` records fixing in `src/job_fixture.rs`.
    let (_, second_token) = store.claim(1, 100, 5).unwrap();
    assert_ne!(first_token.generation, second_token.generation);
    // Advance the new holder to `Running` so the stale completion below is
    // refused specifically by generation fencing, not merely by the state
    // check `complete` also performs.
    store.begin_execution(second_token, 100).unwrap();

    let before = store.get(job_id).unwrap().clone();
    let error = store
        .complete(first_token, 100, AttemptOutcome::Success, None)
        .unwrap_err();
    assert_eq!(error, JobStoreError::LeaseNotCurrent);
    assert_eq!(store.get(job_id).unwrap(), &before);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn completing_past_the_lease_deadline_is_refused() {
    let dir = tempdir("lease-expired");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    store.enqueue(request(b"job", b"payload", 3, 0)).unwrap();
    let (_, token) = store.claim(1, 0, 10).unwrap();
    store.begin_execution(token, 0).unwrap();
    let error = store
        .complete(token, 11, AttemptOutcome::Success, None)
        .unwrap_err();
    assert_eq!(error, JobStoreError::LeaseNotCurrent);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn completing_before_begin_execution_is_refused() {
    let dir = tempdir("lease-not-running");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    store.enqueue(request(b"job", b"payload", 3, 0)).unwrap();
    let (_, token) = store.claim(1, 0, 10).unwrap();
    let error = store
        .complete(token, 0, AttemptOutcome::Success, None)
        .unwrap_err();
    assert_eq!(error, JobStoreError::LeaseNotCurrent);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_forged_lease_token_naming_the_wrong_worker_is_refused() {
    let dir = tempdir("lease-forged");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    store.enqueue(request(b"job", b"payload", 3, 0)).unwrap();
    let (job_id, token) = store.claim(1, 0, 10).unwrap();
    store.begin_execution(token, 0).unwrap();
    let forged = LeaseToken {
        job_id,
        worker_id: 999,
        generation: token.generation,
    };
    let error = store
        .complete(forged, 0, AttemptOutcome::Success, None)
        .unwrap_err();
    assert_eq!(error, JobStoreError::LeaseNotCurrent);
    assert_eq!(store.get(job_id).unwrap().state, JobState::Running);
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// Transactional integration: enqueue plus one application-state side
// record commit atomically, or neither is ever visible.
// ---------------------------------------------------------------------

#[test]
fn enqueue_with_side_record_commits_both_or_neither_on_success() {
    let dir = tempdir("txn-success");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    let outcome = store
        .enqueue_with_side_record(
            request(b"order-42", b"ship-widget", 3, 0),
            b"orders-total".to_vec(),
            b"1".to_vec(),
        )
        .unwrap();
    assert!(matches!(outcome, EnqueueOutcome::Created(_)));
    assert_eq!(store.side_record(b"orders-total"), Some(&b"1".to_vec()));
    drop(store);
    let reopened = GenerationJobStore::open(&dir).unwrap();
    assert_eq!(reopened.table.jobs.len(), 1);
    assert_eq!(reopened.side_record(b"orders-total"), Some(&b"1".to_vec()));
    fs::remove_dir_all(&dir).ok();
}

/// The atomicity proof: a fault injected partway through the joint commit's
/// durable write sequence must leave *neither* the new job *nor* the new
/// side record visible after reopening — never the job without the side
/// record, and never a torn write of either individually. This is what
/// "database transaction integration" means against ADR 0005's chosen
/// medium: atomicity against this durable store, not a SQL engine.
#[test]
fn a_fault_during_the_joint_commit_leaves_neither_the_job_nor_the_side_record_visible() {
    for fault_point in [
        HookPoint::AfterStageWrite,
        HookPoint::AfterStageFsync,
        HookPoint::AfterRename,
    ] {
        let dir = tempdir(&format!("txn-fault-{fault_point:?}"));
        let mut store = GenerationJobStore::open(&dir).unwrap();
        let mut closure = |seen: HookPoint| {
            if seen == fault_point {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        };
        let mut hook: Option<&mut durable_fs::Hook<'_>> = Some(&mut closure);
        let mut candidate = store.table.clone();
        let id = JobId(candidate.next_job_id);
        candidate.next_job_id += 1;
        candidate.jobs.insert(
            id,
            JobRecord {
                id,
                idempotency_key: b"order-99".to_vec(),
                payload: b"ship-gadget".to_vec(),
                state: JobState::Pending,
                attempt: 0,
                lease_epoch: 0,
                retry_policy: policy(3),
                lease: None,
                created_at_tick: 0,
                last_error: Vec::new(),
            },
        );
        candidate
            .side_records
            .insert(b"orders-total".to_vec(), b"1".to_vec());
        let result = store.commit_with_hook(candidate, &mut hook);
        assert!(result.is_err(), "{fault_point:?}");

        drop(store);
        let reopened = GenerationJobStore::open(&dir).unwrap();
        assert!(
            reopened.table.jobs.is_empty(),
            "job leaked at {fault_point:?}"
        );
        assert!(
            reopened.table.side_records.is_empty(),
            "side record leaked at {fault_point:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }
}

// ---------------------------------------------------------------------
// Cancellation.
// ---------------------------------------------------------------------

#[test]
fn a_pending_job_can_be_cancelled_and_a_running_one_cannot() {
    let dir = tempdir("cancel");
    let mut store = GenerationJobStore::open(&dir).unwrap();
    store.enqueue(request(b"a", b"p", 3, 0)).unwrap();
    let cancelled = store.cancel(JobId(1)).unwrap();
    assert_eq!(cancelled, JobState::Cancelled);
    assert!(cancelled.is_terminal());

    store.enqueue(request(b"b", b"p", 3, 0)).unwrap();
    let (job_id, token) = store.claim(1, 0, 100).unwrap();
    store.begin_execution(token, 0).unwrap();
    let error = store.cancel(job_id).unwrap_err();
    assert_eq!(error, JobStoreError::IllegalTransition);
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// No ambient authority: this module tree never reaches for network,
// process spawning, or environment-derived secrets.
// ---------------------------------------------------------------------

#[test]
fn module_source_never_reaches_for_network_process_or_env_authority() {
    let sources: &[&str] = &[
        include_str!("../model.rs"),
        include_str!("../retry.rs"),
        include_str!("../codec.rs"),
        include_str!("../durable_fs.rs"),
        include_str!("../store.rs"),
    ];
    let forbidden = [
        "TcpStream",
        "TcpListener",
        "UdpSocket",
        "std::net",
        "std::process::Command",
        "std::env::var",
        "reqwest",
        "hyper",
    ];
    for source in sources {
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "found forbidden authority-shaped token {needle:?} in durable_jobs source"
            );
        }
    }
}
