//! Process-bound writer ownership for the generation-backed job store.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use semaprax::durable_jobs::{
    AttemptOutcome, EnqueueOutcome, EnqueueRequest, GenerationJobStore, JobId, JobState, JobStore,
    JobStoreError, LeaseToken, RetryPolicy,
};

const CHILD: &str = "SEMAPRAX_DURABLE_JOB_LOCK_CHILD";
const ROOT: &str = "SEMAPRAX_DURABLE_JOB_LOCK_ROOT";
const READY: &str = "SEMAPRAX_DURABLE_JOB_LOCK_READY";
const RELEASE: &str = "SEMAPRAX_DURABLE_JOB_LOCK_RELEASE";

fn request(key: &[u8]) -> EnqueueRequest {
    EnqueueRequest {
        idempotency_key: key.to_vec(),
        payload: key.to_vec(),
        retry_policy: RetryPolicy::new(3, 1, 8),
        now_tick: 0,
    }
}

fn wait_for(path: &Path, description: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn writer_lock_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
    let ready = PathBuf::from(std::env::var_os(READY).unwrap());
    let release = PathBuf::from(std::env::var_os(RELEASE).unwrap());
    let mut store = GenerationJobStore::open(&root).unwrap();
    assert!(matches!(
        store.enqueue_with_side_record(
            request(b"child-job"),
            b"child-side".to_vec(),
            b"present".to_vec(),
        ),
        Ok(EnqueueOutcome::Created(JobId(1)))
    ));
    assert_eq!(store.claim(7, 0, 10).unwrap().0, JobId(1));
    std::fs::write(ready, b"ready").unwrap();
    wait_for(&release, "parent release marker");
}

#[test]
fn child_process_writer_is_exclusive_and_releases_on_exit() {
    let root =
        std::env::temp_dir().join(format!("semaprax-durable-job-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let ready = root.join("child-ready");
    let release = root.join("child-release");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "durable_job_writer::writer_lock_child",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env(ROOT, &root)
        .env(READY, &ready)
        .env(RELEASE, &release)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for(&ready, "child writer lock");
    assert!(matches!(
        GenerationJobStore::open(&root),
        Err(JobStoreError::WriterBusy)
    ));
    std::fs::write(&release, b"release").unwrap();
    assert!(child.wait().unwrap().success());

    let mut reopened = GenerationJobStore::open(&root).unwrap();
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
    drop(reopened);
    std::fs::remove_dir_all(&root).unwrap();
}
