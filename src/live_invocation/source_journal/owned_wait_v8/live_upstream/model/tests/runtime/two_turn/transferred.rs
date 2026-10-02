//! Public recovery consumes an exact first TransferCompleted in another process.
use super::*;
use crate::live_invocation::source_journal::{SourceOwnedAgentJournalV1, SourceOwnedAgentStatusV1};
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;
use std::os::unix::fs::PermissionsExt;
const MODE: &str = "SEMAPRAX_TRANSFERRED_RESTART_MODE";
const META: &str = "SEMAPRAX_TRANSFERRED_RESTART_META";

#[test]
fn public_transferred_relaunch_child() {
    let Some(mode) = std::env::var_os(MODE) else {
        return;
    };
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let metadata = std::path::PathBuf::from(std::env::var_os(META).unwrap());
            CheckedOwnedWaitJournalContextV8::test_with_actual_two_turn_store(
                |context, _lease, _key, fixture| {
                    let counts = Rc::new(RefCell::new(Counts::default()));
                    let mut factory = factory(
                        Rc::clone(&counts),
                        script(&document(&context)),
                        Rc::new(|_| {}),
                    );
                    let mut adapter = source(&context, &mut factory);
                    let cancel = crate::agent_runtime::AgentCancellation::new();
                    let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
                    let mut host = Host {
                        calls: 0,
                        fail: false,
                    };
                    if mode == "prepare" || mode == "prepare-completed" {
                        let directory = fixture.parent().unwrap().join("transferred-restart");
                        std::fs::create_dir(&directory).unwrap();
                        std::fs::set_permissions(
                            &directory,
                            std::fs::Permissions::from_mode(0o700),
                        )
                        .unwrap();
                        let mut registration = None;
                        let opened = SourceOwnedAgentJournalV1::create_fresh(
                            context.test_runtime_arc(),
                            "src/app.spx",
                            "fixture.agent",
                            "fixture.agent.type.step",
                            &adapter,
                            &public_restart_policy(&context),
                            &cancel,
                            &Clock,
                            context.test_runtime_execution().1.evaluation_fuel(),
                            File::open(&directory).unwrap(),
                            7,
                            SourceCheckpointKey::new([75; 32]),
                            true,
                            |facts| {
                                registration = Some(facts.clone());
                                true
                            },
                        )
                        .unwrap();
                        assert!(opened
                            .restart_first_transferred_state(
                                &policy,
                                &cancel,
                                &Clock,
                                &mut adapter,
                                &mut host,
                                |_| panic!("fresh cleanup"),
                                true
                            )
                            .is_err());
                        let journal = opened.test_journal();
                        let mut runtime = OwnedLifecycleRuntimeV8::open(journal, &cancel).unwrap();
                        assert_eq!(
                            runtime.session().run_first_turn_model(
                                super::super::super::super::super::tests::input(journal.context()),
                                &mut adapter,
                                &Clock,
                            ),
                            OwnedLifecycleStatusV8::ModelCompleted
                        );
                        let before = journal.begin_session().unwrap().sequence();
                        if mode == "prepare" {
                            journal
                                .test_observe_lease()
                                .borrow_mut()
                                .test_fail_before_write(before + 4);
                            assert_eq!(
                                runtime.session().finish_two_turn_run(
                                    &policy,
                                    &mut adapter,
                                    &mut host,
                                    |_| panic!("no cleanup before Authorize")
                                ),
                                OwnedLifecycleStatusV8::Quarantined("first-authorize")
                            );
                            let bytes = journal
                                .test_observe_lease()
                                .borrow()
                                .test_persisted_snapshot()
                                .unwrap();
                            assert_eq!(
                                bytes
                                    .split(|b| *b == b'\n')
                                    .filter(|r| !r.is_empty())
                                    .count(),
                                before + 3
                            );
                        } else {
                            assert_eq!(runtime.status(), OwnedLifecycleStatusV8::ModelCompleted);
                        }
                        assert_eq!((counts.borrow().starts, host.calls), (1, 0));
                        let meta = PublicRestartMeta {
                            journal_file: std::fs::read_dir(&directory)
                                .unwrap()
                                .next()
                                .unwrap()
                                .unwrap()
                                .path(),
                            directory,
                            retained_registration: registration.unwrap(),
                            preparer_pid: std::process::id(),
                            resumed_pid_marker: metadata.with_extension("pid"),
                        };
                        drop(runtime);
                        drop(adapter);
                        drop(opened);
                        std::fs::write(&metadata, serde_json::to_vec(&meta).unwrap()).unwrap();
                    } else {
                        let meta: PublicRestartMeta =
                            serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
                        let opened = SourceOwnedAgentJournalV1::recover(
                            context.test_runtime_arc(),
                            "src/app.spx",
                            "fixture.agent",
                            "fixture.agent.type.step",
                            &adapter,
                            &public_restart_policy(&context),
                            &cancel,
                            &Clock,
                            context.test_runtime_execution().1.evaluation_fuel(),
                            File::open(&meta.directory).unwrap(),
                            7,
                            SourceCheckpointKey::new([75; 32]),
                            true,
                            &meta.retained_registration,
                        )
                        .unwrap();
                        let journal = opened.test_journal();
                        let prefix = journal
                            .test_observe_lease()
                            .borrow()
                            .test_persisted_snapshot()
                            .unwrap();
                        assert!(opened
                            .restart_first_transferred_state(
                                &policy,
                                &cancel,
                                &Clock,
                                &mut adapter,
                                &mut host,
                                |_| panic!("no authority"),
                                false
                            )
                            .is_err());
                        assert_eq!(
                            journal
                                .test_observe_lease()
                                .borrow()
                                .test_persisted_snapshot()
                                .unwrap(),
                            prefix
                        );
                        if mode == "completed-only" {
                            assert!(opened
                                .restart_first_transferred_state(
                                    &policy,
                                    &cancel,
                                    &Clock,
                                    &mut adapter,
                                    &mut host,
                                    |_| panic!("no settled-owner remint"),
                                    true
                                )
                                .is_err());
                            assert_eq!((counts.borrow().starts, host.calls), (0, 0));
                            assert_eq!(
                                journal
                                    .test_observe_lease()
                                    .borrow()
                                    .test_persisted_snapshot()
                                    .unwrap(),
                                prefix
                            );
                            std::fs::write(meta.resumed_pid_marker, std::process::id().to_string())
                                .unwrap();
                            return;
                        }
                        let session = journal.begin_session().unwrap();
                        let facts = session
                            .inventory
                            .first_turn_transfer_completed_authorization_recovery()
                            .unwrap();
                        let before = session.sequence();
                        assert_eq!(facts.sequence, before);
                        drop(session);
                        match mode.to_str().unwrap() {
                            "reservation-fault" => journal
                                .test_observe_lease()
                                .borrow_mut()
                                .test_fail_before_write(1),
                            "decision-fault" => journal
                                .test_observe_lease()
                                .borrow_mut()
                                .test_fail_after_write(2),
                            "resume" => {}
                            _ => panic!("unknown child mode"),
                        }
                        let mut releases = 0;
                        let run = opened
                            .restart_first_transferred_state(
                                &policy,
                                &cancel,
                                &Clock,
                                &mut adapter,
                                &mut host,
                                |_| releases += 1,
                                true,
                            )
                            .unwrap();
                        if mode == "resume" {
                            assert_eq!(run.status(), SourceOwnedAgentStatusV1::Complete);
                            assert_eq!((counts.borrow().starts, host.calls, releases), (1, 2, 4));
                            assert_eq!(run.delivery_projection().unwrap()["kind"], "complete");
                            assert!(matches!(run.try_close(), Ok(Some(_))));
                            assert!(opened
                                .restart_first_transferred_state(
                                    &policy,
                                    &cancel,
                                    &Clock,
                                    &mut adapter,
                                    &mut host,
                                    |_| panic!("terminal retry"),
                                    true
                                )
                                .is_err());
                        } else {
                            assert_eq!(
                                run.status(),
                                SourceOwnedAgentStatusV1::Quarantined("restored-first-authorize")
                            );
                            assert_eq!((counts.borrow().starts, host.calls, releases), (0, 0, 0));
                            let weak = run.test_backings();
                            assert!(!weak.is_empty());
                            assert!(weak.iter().all(|root| root.strong_count() == 1));
                            let bytes = journal
                                .test_observe_lease()
                                .borrow()
                                .test_persisted_snapshot()
                                .unwrap();
                            let run = run
                                .try_close()
                                .err()
                                .expect("actual State/Decision must remain in run custody");
                            assert!(opened
                                .restart_first_transferred_state(
                                    &policy,
                                    &cancel,
                                    &Clock,
                                    &mut adapter,
                                    &mut host,
                                    |_| panic!("retry cleanup"),
                                    true
                                )
                                .is_err());
                            assert_eq!(
                                journal
                                    .test_observe_lease()
                                    .borrow()
                                    .test_persisted_snapshot()
                                    .unwrap(),
                                bytes
                            );
                            assert!(weak.iter().all(|root| root.strong_count() == 1));
                            drop(run);
                            assert!(weak.iter().all(|root| root.upgrade().is_none()));
                            assert_eq!(
                                journal
                                    .test_observe_lease()
                                    .borrow()
                                    .test_persisted_snapshot()
                                    .unwrap(),
                                bytes
                            );
                        }
                        std::fs::write(meta.resumed_pid_marker, std::process::id().to_string())
                            .unwrap();
                    }
                },
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
fn relaunch(mode: &str) {
    let root = std::env::temp_dir().join(format!(
        "spx-transferred-public-{}-{mode}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let metadata = root.join("retained.json");
    let prepare = if mode == "completed-only" {
        "prepare-completed"
    } else {
        "prepare"
    };
    for child_mode in [prepare, mode] {
        let module = module_path!();
        let module = module
            .strip_prefix(concat!(env!("CARGO_PKG_NAME"), "::"))
            .unwrap_or(module);
        let test = format!("{module}::public_transferred_relaunch_child");
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &test, "--nocapture"])
            .env(MODE, child_mode)
            .env(META, &metadata)
            .stdin(std::process::Stdio::null());
        if child_mode.starts_with("prepare") {
            command.env("SEMAPRAX_KEEP_OWNED_WAIT_CONTEXT_FIXTURE", "1");
        } else {
            command.env_remove("SEMAPRAX_KEEP_OWNED_WAIT_CONTEXT_FIXTURE");
        }
        assert!(command.status().unwrap().success(), "child {child_mode}");
    }
    let meta: PublicRestartMeta =
        serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
    let resumed = std::fs::read_to_string(&meta.resumed_pid_marker)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert_ne!(meta.preparer_pid, resumed);
    assert_ne!(meta.preparer_pid, std::process::id());
    std::fs::remove_dir_all(meta.directory.parent().unwrap()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn public_transferred_relaunch_authorizes_once_and_completes() {
    relaunch("resume");
}
#[test]
fn public_transferred_relaunch_authorize_faults_retain_custody() {
    for mode in ["reservation-fault", "decision-fault"] {
        relaunch(mode);
    }
}

#[test]
fn public_transferred_relaunch_refuses_settled_without_transfer() {
    relaunch("completed-only");
}
