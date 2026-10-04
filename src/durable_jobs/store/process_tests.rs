//! Process-termination regressions kept outside `store::tests` so its
//! production-source authority scan remains exact.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::*;
use crate::durable_jobs::durable_fs::HookPoint;
use crate::durable_jobs::model::RetryPolicy;

const CHILD: &str = "SEMAPRAX_DURABLE_STAGE_CHILD";
const ROOT: &str = "SEMAPRAX_DURABLE_STAGE_ROOT";
const READY: &str = "SEMAPRAX_DURABLE_STAGE_READY";
const STAGE: &str = "SEMAPRAX_DURABLE_STAGE_KIND";

fn request(key: &[u8]) -> EnqueueRequest {
    EnqueueRequest {
        idempotency_key: key.to_vec(),
        payload: key.to_vec(),
        retry_policy: RetryPolicy::new(3, 1, 8),
        now_tick: 0,
    }
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "writer child did not reach stage barrier"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn interrupted_normal_writer_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
    let ready = PathBuf::from(std::env::var_os(READY).unwrap());
    let target = std::env::var(STAGE).unwrap().parse::<usize>().unwrap();
    let mut store = GenerationJobStore::open(&root).unwrap();
    let mut candidate = store.table.clone();
    candidate
        .side_records
        .insert(b"child".to_vec(), b"stage".to_vec());
    let mut stage_writes = 0usize;
    let mut hook = |point: HookPoint| -> std::io::Result<()> {
        if point == HookPoint::AfterStageWrite {
            stage_writes += 1;
            if stage_writes == target {
                std::fs::write(&ready, b"stage-written").unwrap();
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Ok(())
    };
    let mut hook: Option<&mut crate::durable_jobs::durable_fs::Hook<'_>> = Some(&mut hook);
    let _ = store.commit_with_hook(candidate, &mut hook);
}

#[test]
fn killed_normal_writer_after_generation_or_active_stage_recovers() {
    for (target, label) in [(1usize, "generation"), (2usize, "active")] {
        let root = std::env::temp_dir().join(format!(
            "semaprax-durable-normal-stage-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let ready = root.join("ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "durable_jobs::store::process_tests::interrupted_normal_writer_child",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(ROOT, &root)
            .env(READY, &ready)
            .env(STAGE, target.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait_for(&ready);
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());

        let mut recovered = GenerationJobStore::open(&root).unwrap();
        assert!(matches!(
            recovered.enqueue(request(label)),
            Ok(EnqueueOutcome::Created(_))
        ));
        drop(recovered);
        let reopened = GenerationJobStore::open(&root).unwrap();
        assert_eq!(reopened.get(JobId(1)).unwrap().payload, label.as_bytes());
        drop(reopened);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
