//! Authenticated process restart rejoins the actual two-turn runtime custody.
use super::two_turn::Host;
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::wait::{
    FirstTurnPreparedContinuationHostGrantV8, FirstTurnPreparedRecoveryHostGrantV8,
};
use crate::resumable_effects::CapabilityPolicy;

const RESTART_MODE: &str = "SEMAPRAX_OWNED_RUNTIME_RESTART_MODE";
const RESTART_META: &str = "SEMAPRAX_OWNED_RUNTIME_RESTART_META";

fn restart<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    adapter: &mut StreamingSourceProposalAdapter<'_>,
) -> Result<OwnedLifecycleRuntimeV8<'j>, SourceJournalError> {
    OwnedLifecycleRuntimeV8::restart_first_prepared(
        journal,
        FirstTurnPreparedRecoveryHostGrantV8::for_trusted_host(true).unwrap(),
        FirstTurnPreparedContinuationHostGrantV8::for_trusted_host(true).unwrap(),
        adapter,
        &Clock,
        cancellation,
    )
}

fn journal_path(directory: &std::path::Path) -> PathBuf {
    let entries = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    entries.into_iter().next().unwrap()
}

fn finish(
    journal: &SourceOwnedWaitJournalV8,
    cancellation: &crate::agent_runtime::AgentCancellation,
    adapter: &mut StreamingSourceProposalAdapter<'_>,
    counts: &Rc<RefCell<Counts>>,
) -> Vec<u8> {
    let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
    let mut runtime = restart(journal, cancellation, adapter).unwrap();
    assert_eq!(runtime.status(), OwnedLifecycleStatusV8::ModelCompleted);
    let weak = runtime.test_backings();
    assert!(!weak.is_empty());
    assert!(weak.iter().all(|root| root.strong_count() == 1));
    let before = journal.begin_session().unwrap();
    assert_eq!(before.sequence(), 15);
    let accounting = before.fold_for_live_test();
    let fuel = journal
        .context()
        .test_runtime_execution()
        .1
        .evaluation_fuel() as u64;
    assert_eq!(accounting.reserved_total, 4 * fuel);
    assert_eq!(accounting.stages, 2);
    drop(before);
    assert_eq!(counts.borrow().starts, 1);
    runtime = runtime.try_close().err().expect("actual State is retained");
    assert_eq!(
        runtime.session().status(),
        OwnedLifecycleStatusV8::ModelCompleted
    );
    let mut host = Host {
        calls: 0,
        fail: false,
    };
    let mut releases = 0;
    assert_eq!(
        runtime
            .session()
            .finish_two_turn_run(&policy, adapter, &mut host, |_| releases += 1),
        OwnedLifecycleStatusV8::Complete
    );
    assert_eq!((counts.borrow().starts, host.calls, releases), (2, 2, 4));
    assert!(weak.iter().all(|root| root.upgrade().is_none()));
    let evidence = journal.terminal_evidence().unwrap().evidence().to_vec();
    assert_eq!(
        runtime.delivery_projection().unwrap()["terminal_evidence"]
            .as_str()
            .map(str::as_bytes),
        Some(evidence.as_slice())
    );
    let persisted = journal
        .test_observe_lease()
        .borrow()
        .test_persisted_snapshot()
        .unwrap();
    assert_eq!(
        runtime
            .session()
            .finish_two_turn_run(&policy, adapter, &mut host, |_| releases += 1),
        OwnedLifecycleStatusV8::Complete
    );
    assert_eq!((counts.borrow().starts, host.calls, releases), (2, 2, 4));
    assert_eq!(
        journal
            .test_observe_lease()
            .borrow()
            .test_persisted_snapshot()
            .unwrap(),
        persisted
    );
    assert!(runtime.try_close().is_ok());
    journal.hold().expect("terminal Report retired the hold");
    evidence
}

#[test]
fn owned_runtime_restart_prepared_model_faults_retain_custody_without_retry() {
    for append in [11, 12, 13, 14, 15] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let context = Arc::new(context.with_cumulative_initialization(&lease).unwrap());
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
                let cancellation = crate::agent_runtime::AgentCancellation::new();
                let parked = park(&journal, &cancellation);
                let original = parked.owner.test_weak();
                drop(parked);
                drop(journal);
                assert!(original.iter().all(|root| root.upgrade().is_none()));
                let mut lease = crate::resumable_effects::owned_frame::recover_source_owned_wait_v8(
                File::open(directory).unwrap(), context.registration(),
                context.registration().expected_facts().clone(),
                crate::resumable_effects::owned_frame::ExplicitStoreRegistrationGrant::for_trusted_host(true).unwrap(),
            ).unwrap();
                lease.test_fail_before_write(append);
                let journal = SourceOwnedWaitJournalV8::open(
                    Arc::clone(&context),
                    crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                    lease,
                )
                .unwrap();
                let counts = Rc::new(RefCell::new(Counts::default()));
                let mut factory = factory(
                    Rc::clone(&counts),
                    script(&document(&context)),
                    Rc::new(|_| {}),
                );
                let mut adapter = source(&context, &mut factory);
                let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                let mut runtime = restart(&journal, &cancellation, &mut adapter).unwrap();
                assert_eq!(
                    runtime.status(),
                    OwnedLifecycleStatusV8::Quarantined("restart-model")
                );
                let weak = runtime.test_backings();
                assert!(!weak.is_empty());
                assert!(weak.iter().all(|root| root.strong_count() == 1));
                assert_eq!(counts.borrow().starts, usize::from(append > 11));
                assert!(weak.iter().all(|new| original
                    .iter()
                    .all(|old| !std::sync::Weak::ptr_eq(new, old))));
                let bytes = journal
                    .test_observe_lease()
                    .borrow()
                    .test_persisted_snapshot()
                    .unwrap();
                assert_eq!(
                    bytes.iter().filter(|byte| **byte == b'\n').count(),
                    append - 1
                );
                let mut host = Host {
                    calls: 0,
                    fail: false,
                };
                let mut releases = 0;
                let status = runtime.status();
                assert_eq!(
                    runtime
                        .session()
                        .finish_two_turn_run(&policy, &mut adapter, &mut host, |_| releases += 1),
                    status
                );
                runtime = runtime
                    .try_close()
                    .err()
                    .expect("restart failure retains actual owner");
                assert_eq!(
                    runtime.session().status(),
                    OwnedLifecycleStatusV8::Quarantined("restart-model")
                );
                assert_eq!(
                    (counts.borrow().starts, host.calls, releases),
                    (usize::from(append > 11), 0, 0)
                );
                assert_eq!(
                    journal
                        .test_observe_lease()
                        .borrow()
                        .test_persisted_snapshot()
                        .unwrap(),
                    bytes
                );
                assert!(runtime.delivery_projection().is_none());
                drop(runtime);
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
                assert!(journal.hold().is_err());
            },
        );
    }
}

#[test]
fn owned_runtime_restart_prepared_fresh_or_non_cumulative_store_refuses_before_model() {
    for cumulative in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let context = Arc::new(if cumulative {
                    context.with_cumulative_initialization(&lease).unwrap()
                } else {
                    context.with_initialization(&lease).unwrap()
                });
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
                let cancellation = crate::agent_runtime::AgentCancellation::new();
                let parked = park(&journal, &cancellation);
                let weak = parked.owner.test_weak();
                let before = std::fs::read(journal_path(directory)).unwrap();
                let counts = Rc::new(RefCell::new(Counts::default()));
                let mut factory = factory(
                    Rc::clone(&counts),
                    script(&document(&context)),
                    Rc::new(|_| {}),
                );
                let mut adapter = source(&context, &mut factory);
                assert!(matches!(
                    restart(&journal, &cancellation, &mut adapter),
                    Err(SourceJournalError::Binding)
                ));
                assert_eq!(
                    (
                        counts.borrow().factories,
                        counts.borrow().starts,
                        counts.borrow().polls
                    ),
                    (0, 0, 0)
                );
                assert_eq!(std::fs::read(journal_path(directory)).unwrap(), before);
                assert!(weak.iter().all(|root| root.strong_count() == 1));
                drop(parked);
                assert!(weak.iter().all(|root| root.upgrade().is_none()));
            },
        );
    }
}

fn child(mode: &str, metadata: &std::path::Path, keep: bool) {
    let module = module_path!();
    let module = module
        .strip_prefix(concat!(env!("CARGO_PKG_NAME"), "::"))
        .unwrap_or(module);
    let selector = format!("{module}::owned_runtime_restart_prepared_process_child");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &selector, "--nocapture"])
        .env(RESTART_MODE, mode)
        .env(RESTART_META, metadata)
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    if keep {
        command.env(PREPARED_RESTART_KEEP_FIXTURE, "1");
    } else {
        command.env_remove(PREPARED_RESTART_KEEP_FIXTURE);
    }
    assert!(
        command.status().unwrap().success(),
        "runtime restart child {mode}"
    );
}

#[test]
fn owned_runtime_restart_prepared_process_child() {
    let Some(mode) = std::env::var_os(RESTART_MODE) else {
        return;
    };
    let path = PathBuf::from(std::env::var_os(RESTART_META).unwrap());
    if mode == "prepare" {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let context = Arc::new(context.with_cumulative_initialization(&lease).unwrap());
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
                let cancellation = crate::agent_runtime::AgentCancellation::new();
                let parked = park(&journal, &cancellation);
                // Start stages Created and Reserved after the committed sequence 10.
                assert_eq!(parked.session.sequence(), 12);
                let metadata = PreparedRestartProcessMeta {
                    directory: directory.to_owned(),
                    registration: context.registration().test_retained_restart_facts(),
                    preparer_pid: std::process::id(),
                    resumed_pid_marker: path.with_extension("resumed-pid"),
                };
                // Simulate loss of process backing without fabricating a cleanup.
                drop(parked);
                drop(journal);
                std::fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
            },
        );
        return;
    }
    let metadata: PreparedRestartProcessMeta =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let registration = metadata.registration.registration().unwrap();
    CheckedOwnedWaitJournalContextV8::test_with_actual_recovered_runtime_store(
        &metadata.directory,
        registration,
        |context, lease, key| {
            let context = Arc::new(context.with_cumulative_initialization(&lease).unwrap());
            let journal = SourceOwnedWaitJournalV8::open(Arc::clone(&context), key, lease).unwrap();
            let cancellation = crate::agent_runtime::AgentCancellation::new();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(
                Rc::clone(&counts),
                script(&document(&context)),
                Rc::new(|_| {}),
            );
            let mut adapter = source(&context, &mut factory);
            if mode == "complete" {
                finish(&journal, &cancellation, &mut adapter, &counts);
                std::fs::write(&metadata.resumed_pid_marker, std::process::id().to_string())
                    .unwrap();
            } else {
                assert!(mode == "terminal" || mode == "hostile" || mode == "cancelled");
                if mode == "cancelled" {
                    cancellation.cancel();
                }
                let before = std::fs::read(journal_path(&metadata.directory)).unwrap();
                assert!(restart(&journal, &cancellation, &mut adapter).is_err());
                assert_eq!(
                    (
                        counts.borrow().factories,
                        counts.borrow().starts,
                        counts.borrow().polls
                    ),
                    (0, 0, 0)
                );
                assert_eq!(
                    std::fs::read(journal_path(&metadata.directory)).unwrap(),
                    before
                );
                if mode == "terminal" {
                    journal.terminal_evidence().unwrap();
                }
            }
        },
    );
}

#[test]
fn owned_runtime_restart_prepared_process_relaunch_reaches_terminal_once() {
    let serial = PREPARED_RESTART_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "spx-runtime-restart-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("registration.json");
    child("prepare", &path, true);
    let metadata: PreparedRestartProcessMeta =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_ne!(metadata.preparer_pid, std::process::id());
    child("cancelled", &path, false);
    child("complete", &path, false);
    let resumed: u32 = std::fs::read_to_string(&metadata.resumed_pid_marker)
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(metadata.preparer_pid, resumed);
    assert_ne!(std::process::id(), resumed);
    // A third independent process may read terminal evidence, never restore
    // a Report or dispatch the original model from that historical Prepared.
    child("terminal", &path, false);
    std::fs::OpenOptions::new()
        .append(true)
        .open(journal_path(&metadata.directory))
        .unwrap()
        .write_all(b"hostile")
        .unwrap();
    child("hostile", &path, false);
    std::fs::remove_dir_all(metadata.directory.parent().unwrap()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
