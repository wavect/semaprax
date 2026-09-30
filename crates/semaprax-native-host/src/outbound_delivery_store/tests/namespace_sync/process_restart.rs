use super::*;
use semaprax::outbound_host_adapter::{
    prepare_email_delivery, prepare_webhook_delivery, DurableEmailDeliveryOutcome,
    DurableWebhookDeliveryOutcome, EmailDeliverySession, EmailDeliverySessionRestoreCapability,
    EmailRequest, HttpDeliverySessionRestoreRefusal, WebhookDeliverySession,
    WebhookDeliverySessionRestoreCapability, WebhookRequest, WebhookSigningSecret,
};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CHILD_MODE: &str = "SEMAPRAX_OUTBOUND_RESTART_CHILD_MODE";
const CHILD_DIRECTORY: &str = "SEMAPRAX_OUTBOUND_RESTART_DIRECTORY";
const CHILD_DIGEST: &str = "SEMAPRAX_OUTBOUND_RESTART_DIGEST";
const CHILD_CAPACITY: &str = "SEMAPRAX_OUTBOUND_RESTART_CAPACITY";
const CHILD_TAMPERED: &str = "SEMAPRAX_OUTBOUND_RESTART_TAMPERED";
const ACCEPTED_DISPATCH_MARKER: &str = "outbound-accepted-dispatch.marker";
const CHILD_WAIT: Duration = Duration::from_secs(5);
const CHILD_POLL: Duration = Duration::from_millis(10);

// The parent releases its held directory before the child acquires a fresh
// caller-selected hold. These local tests bind restored sessions to exact
// checkpoint bytes, digest, and capacity; they do not claim root-identity
// continuity, copied-byte refusal, crash/power-loss durability, or an external
// delivery outcome.
struct PanicOnDispatch;

impl OutboundAdapter for PanicOnDispatch {
    fn send(&mut self, _: &PreparedRequest) -> AdapterObservation {
        panic!("a restored terminal checkpoint must not redispatch")
    }
}

fn child_value(name: &str, maximum: usize) -> String {
    let value = std::env::var(name).unwrap_or_else(|_| panic!("child missing {name}"));
    assert!(
        !value.is_empty() && value.len() <= maximum,
        "child {name} exceeds its bounded fixture input"
    );
    value
}

fn child_fixture() -> (std::path::PathBuf, String, usize) {
    assert_eq!(
        std::env::var(CHILD_MODE).as_deref(),
        Ok("replay-v1"),
        "the child mode is one-shot and does not recurse"
    );
    let directory = std::path::PathBuf::from(child_value(CHILD_DIRECTORY, 4_096));
    let digest = child_value(CHILD_DIGEST, 71);
    let capacity = child_value(CHILD_CAPACITY, 20)
        .parse::<usize>()
        .expect("child capacity is decimal");
    assert!(
        capacity > 0,
        "child capacity remains an exact positive bound"
    );
    (directory, digest, capacity)
}

fn assert_one_accepted_dispatch(directory: &std::path::Path) {
    assert_eq!(
        fs::read(directory.join(ACCEPTED_DISPATCH_MARKER)).expect("read accepted dispatch marker"),
        b"accepted-dispatch-v1\n",
        "the parent accepted exactly one local fixture dispatch before child replay"
    );
}

fn inherit_platform_loader_environment(command: &mut Command) {
    // The fresh child gets no configuration environment. These are the small
    // platform loader allowlist needed by dynamically linked test executables.
    for name in [
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
        "LIBPATH",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn spawn_child(
    test_name: &str,
    directory: &std::path::Path,
    digest: &str,
    capacity: usize,
    tampered: bool,
) {
    // `current_exe` is only the Rust test harness; it is not a product process
    // authority or provider route. The child gets no inherited configuration.
    let executable = std::env::current_exe().expect("current test executable");
    let mut command = Command::new(executable);
    command
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env_clear();
    inherit_platform_loader_environment(&mut command);
    let mut child = command
        .env(CHILD_MODE, "replay-v1")
        .env(CHILD_DIRECTORY, directory)
        .env(CHILD_DIGEST, digest)
        .env(CHILD_CAPACITY, capacity.to_string())
        .env(CHILD_TAMPERED, if tampered { "yes" } else { "no" })
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn bounded local replay child");
    let deadline = Instant::now() + CHILD_WAIT;
    loop {
        if let Some(status) = child.try_wait().expect("poll replay child") {
            assert!(status.success(), "local replay child must pass: {status}");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("local replay child exceeded its bounded wait");
        }
        std::thread::sleep(CHILD_POLL);
    }
}

macro_rules! typed_process_restart_case {
    ($module:ident, $session:ident, $restore:ident, $outcome:ident, $kind:ident, $prepare:expr) => {
        mod $module {
            use super::*;

            const TEST_NAME: &str = concat!(
                "outbound_delivery_store::tests::namespace_sync::process_restart::",
                stringify!($module),
                "::separate_process_reopens_exact_checkpoint_without_redispatch"
            );

            #[test]
            fn separate_process_reopens_exact_checkpoint_without_redispatch() {
                if std::env::var_os(CHILD_MODE).is_some() {
                    let (directory_path, digest, capacity) = child_fixture();
                    assert_eq!(std::env::var(CHILD_TAMPERED).as_deref(), Ok("no"));
                    let directory = platform::hold_directory(&directory_path)
                        .expect("child freshly holds caller-selected directory");
                    assert_one_accepted_dispatch(&directory_path);
                    let store = namespace_store(&directory);
                    let bytes = store
                        .load(OutboundCheckpointKind::$kind, &digest)
                        .expect("child loads only its parent-retained exact digest");
                    let mut restored = $session::restore_authenticated(
                        &bytes,
                        $restore::grant_for_trusted_host(&digest, capacity)
                            .expect("child binds the handed exact digest and capacity"),
                    )
                    .expect("child authenticates the typed terminal checkpoint");
                    let mut replay_store = namespace_store(&directory);
                    let mut replay_adapter = PanicOnDispatch;
                    assert!(matches!(
                        restored
                            .reconcile_durable(($prepare)(), &mut replay_store, &mut replay_adapter)
                            .expect("terminal recovery is local replay"),
                        $outcome::Replayed(_)
                    ));
                    return;
                }

                let temp = TempDirectory::new();
                let directory = platform::hold_directory(temp.path())
                    .expect("parent holds caller-selected directory");
                let mut store = namespace_store(&directory);
                let mut session = $session::new(1).expect("bounded typed session");
                let mut adapter = RecordingAdapter::default();
                assert!(matches!(
                    session
                        .reconcile_durable(($prepare)(), &mut store, &mut adapter)
                        .expect("namespace-synced local dispatch"),
                    $outcome::Dispatched(_)
                ));
                assert_eq!(adapter.0.len(), 1, "one accepted fixture dispatch");
                fs::write(
                    temp.path().join(ACCEPTED_DISPATCH_MARKER),
                    b"accepted-dispatch-v1\n",
                )
                .expect("record one accepted local fixture dispatch");
                let checkpoint = session.session_checkpoint().expect("terminal checkpoint");
                let digest = checkpoint.digest();
                let capacity = checkpoint.capacity();
                drop(directory);
                spawn_child(TEST_NAME, temp.path(), &digest, capacity, false);
            }
        }
    };
}

typed_process_restart_case!(
    http,
    HttpDeliverySession,
    HttpDeliverySessionRestoreCapability,
    DurableHttpDeliveryOutcome,
    HttpSession,
    || prepare_http_delivery(capability(), request()).unwrap()
);

#[test]
fn separate_process_content_tamper_refuses_before_adapter() {
    const TEST_NAME: &str = "outbound_delivery_store::tests::namespace_sync::process_restart::separate_process_content_tamper_refuses_before_adapter";
    if std::env::var_os(CHILD_MODE).is_some() {
        let (directory_path, digest, capacity) = child_fixture();
        assert_eq!(std::env::var(CHILD_TAMPERED).as_deref(), Ok("yes"));
        let directory = platform::hold_directory(&directory_path)
            .expect("child freshly holds caller-selected directory");
        assert_one_accepted_dispatch(&directory_path);
        let store = namespace_store(&directory);
        let bytes = store
            .load(OutboundCheckpointKind::HttpSession, &digest)
            .expect("child reads replacement bytes as untrusted data");
        assert!(matches!(
            HttpDeliverySession::restore_authenticated(
                &bytes,
                HttpDeliverySessionRestoreCapability::grant_for_trusted_host(&digest, capacity)
                    .expect("child binds the handed exact digest and capacity"),
            ),
            Err(HttpDeliverySessionRestoreRefusal::Checkpoint(
                DeliverySessionCheckpointRefusal::BindingMismatch
            ))
        ));
        return;
    }

    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).expect("parent holds caller directory");
    let mut store = namespace_store(&directory);
    let mut session = HttpDeliverySession::new(1).expect("bounded HTTP session");
    let mut adapter = RecordingAdapter::default();
    assert!(matches!(
        session
            .reconcile_durable(
                prepare_http_delivery(capability(), request()).unwrap(),
                &mut store,
                &mut adapter,
            )
            .unwrap(),
        DurableHttpDeliveryOutcome::Dispatched(_)
    ));
    assert_eq!(adapter.0.len(), 1, "one accepted fixture dispatch");
    fs::write(
        temp.path().join(ACCEPTED_DISPATCH_MARKER),
        b"accepted-dispatch-v1\n",
    )
    .unwrap();
    let checkpoint = session.session_checkpoint().unwrap();
    let digest = checkpoint.digest();
    let capacity = checkpoint.capacity();
    let filename = checkpoint_filename(OutboundCheckpointKind::HttpSession, &digest).unwrap();
    let replacement = temp.path().join("tampered-checkpoint-replacement");
    fs::write(&replacement, b"tampered checkpoint").unwrap();
    fs::rename(replacement, temp.path().join(filename))
        .expect("hostile owner rename-replaces checkpoint before fresh child open");
    drop(directory);
    spawn_child(TEST_NAME, temp.path(), &digest, capacity, true);
}

typed_process_restart_case!(
    webhook,
    WebhookDeliverySession,
    WebhookDeliverySessionRestoreCapability,
    DurableWebhookDeliveryOutcome,
    WebhookSession,
    || prepare_webhook_delivery(
        capability(),
        WebhookSigningSecret::from_trusted_host_bytes([7; 32]),
        WebhookRequest {
            endpoint: "https://store.example.test/events".into(),
            delivery_id: "process-restart-webhook".into(),
            idempotency_key: "process-restart-webhook-key".into(),
            content_type: "application/json".into(),
            body: b"{}".to_vec(),
            deadline_ms: 1_000,
        }
    )
    .unwrap()
);

typed_process_restart_case!(
    email,
    EmailDeliverySession,
    EmailDeliverySessionRestoreCapability,
    DurableEmailDeliveryOutcome,
    EmailSession,
    || prepare_email_delivery(
        capability(),
        EmailRequest {
            endpoint: "https://store.example.test/email".into(),
            delivery_id: "process-restart-email".into(),
            idempotency_key: "process-restart-email-key".into(),
            sender: "sender@example.test".into(),
            recipients: vec!["recipient@example.test".into()],
            reply_to: None,
            subject: "Ready".into(),
            body: b"ready".to_vec(),
            attachments: Vec::new(),
            deadline_ms: 1_000,
        }
    )
    .unwrap()
);

/// Real, separate-process crash/restart proofs for the `service_invocation`
/// production entry point at each of the three delivery phases the R17 slice
/// requires: before any commit, after dispatch but before the terminal ack
/// (uncertain), and after a fully settled delivery. Every "crash" here is a
/// genuine child process that exits (or is killed) rather than a caught
/// panic, so recovery is proven against real process death, not unwinding.
mod service_invocation_phases {
    use super::*;
    use crate::outbound_delivery_store::service_invocation::{
        commit_pending_intent_marker, deliver_http_durable, PendingIntentCommit,
        PriorTerminalReference, ServiceHttpDeliveryOutcome,
    };

    const PHASE_MODE: &str = "SEMAPRAX_OUTBOUND_SVC_PHASE_MODE";
    const PHASE_DIRECTORY: &str = "SEMAPRAX_OUTBOUND_SVC_PHASE_DIRECTORY";
    const DISPATCH_ENTERED_MARKER: &str = "svc-dispatch-entered.marker";
    const CRASH_AFTER_DISPATCH_CODE: i32 = 77;
    const CRASH_AFTER_INTENT_CODE: i32 = 68;

    fn svc_capability() -> OutboundCapability {
        OutboundCapability::grant_for_trusted_host(
            "sha256:service-invocation-restart-deployment",
            "service-invocation-restart-invocation",
            OutboundPolicy::new(
                "service-invocation-restart-policy",
                ["https://store.example.test".to_owned()],
                1_024,
                512,
                5_000,
                4,
                3,
            )
            .expect("bounded fixture policy"),
        )
        .expect("trusted fixture authority")
    }

    fn svc_request() -> HttpRequest {
        HttpRequest {
            method: HttpMethod::Post,
            endpoint: "https://store.example.test/events".into(),
            request_id: "service-invocation-restart-request".into(),
            idempotency_key: "service-invocation-restart-key".into(),
            content_type: Some("application/json".into()),
            headers: Vec::new(),
            body: br#"{"ready":true}"#.to_vec(),
            deadline_ms: 1_000,
        }
    }

    /// An adapter that proves it was entered by writing a marker before
    /// deliberately crashing the whole process, simulating a real failure
    /// between physical dispatch and the local terminal acknowledgment.
    struct DispatchThenCrash<'a> {
        directory: &'a std::path::Path,
    }

    impl OutboundAdapter for DispatchThenCrash<'_> {
        fn send(&mut self, _: &PreparedRequest) -> AdapterObservation {
            fs::write(self.directory.join(DISPATCH_ENTERED_MARKER), b"entered\n")
                .expect("record that dispatch was entered before the crash");
            std::process::exit(CRASH_AFTER_DISPATCH_CODE);
        }
    }

    struct PanicOnRedispatch;

    impl OutboundAdapter for PanicOnRedispatch {
        fn send(&mut self, _: &PreparedRequest) -> AdapterObservation {
            panic!("a restored or guarded session must never redispatch");
        }
    }

    /// A store wrapper that commits the real intent marker plus the typed
    /// provisional checkpoint -- exactly what `deliver_http_durable`'s own
    /// guard does -- and then crashes the whole process, before the ledger
    /// ever calls into an adapter. This is a strictly earlier crash point
    /// than `DispatchThenCrash`: here the adapter is never entered at all.
    struct CommitIntentThenCrash<'a, 'directory> {
        inner: &'a mut OutboundDeliveryStore<'directory>,
        identity_key: String,
        probed: bool,
    }

    impl HttpDeliverySessionCheckpointStore for CommitIntentThenCrash<'_, '_> {
        fn commit(&mut self, checkpoint: &HttpDeliverySessionCheckpoint) -> CheckpointCommit {
            if !self.probed {
                self.probed = true;
                let marker = commit_pending_intent_marker(
                    self.inner.directory(),
                    self.inner.sync_mode(),
                    OutboundCheckpointKind::HttpSession,
                    &self.identity_key,
                );
                assert_eq!(
                    marker,
                    PendingIntentCommit::Fresh,
                    "test setup: the first attempt's marker must be fresh"
                );
                let outcome = HttpDeliverySessionCheckpointStore::commit(self.inner, checkpoint);
                assert_eq!(
                    outcome,
                    CheckpointCommit::Committed,
                    "test setup: the real intent commit must succeed before the simulated crash"
                );
                // The intent (marker + typed provisional checkpoint) is now
                // durably committed. Crash here, before ever returning to the
                // ledger, which would otherwise call the adapter next.
                std::process::exit(CRASH_AFTER_INTENT_CODE);
            }
            HttpDeliverySessionCheckpointStore::commit(self.inner, checkpoint)
        }
    }

    fn env_directory() -> std::path::PathBuf {
        std::path::PathBuf::from(child_value(PHASE_DIRECTORY, 4_096))
    }

    fn phase_mode() -> Option<String> {
        std::env::var(PHASE_MODE).ok()
    }

    enum ExpectedExit {
        Success,
        Code(i32),
    }

    fn spawn_phase_child(
        test_name: &str,
        mode: &str,
        directory: &std::path::Path,
        extra: &[(&str, &str)],
        expected: ExpectedExit,
    ) {
        let executable = std::env::current_exe().expect("current test executable");
        let mut command = Command::new(executable);
        command
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env_clear();
        inherit_platform_loader_environment(&mut command);
        command
            .env(PHASE_MODE, mode)
            .env(PHASE_DIRECTORY, directory);
        for (name, value) in extra {
            command.env(name, value);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn bounded local service-invocation phase child");
        let deadline = Instant::now() + CHILD_WAIT;
        loop {
            if let Some(status) = child.try_wait().expect("poll phase child") {
                match expected {
                    ExpectedExit::Success => {
                        assert!(status.success(), "phase child must pass: {status}")
                    }
                    ExpectedExit::Code(code) => assert_eq!(
                        status.code(),
                        Some(code),
                        "phase child must crash with its exact simulated code: {status}"
                    ),
                }
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("local service-invocation phase child exceeded its bounded wait");
            }
            std::thread::sleep(CHILD_POLL);
        }
    }

    /// Phase 1: a crash before anything is durably committed. Restart simply
    /// performs the first attempt; nothing needs recovering and the adapter
    /// dispatches exactly once.
    #[test]
    fn before_dispatch_crash_then_restart_dispatches_once() {
        const TEST_NAME: &str = "outbound_delivery_store::tests::namespace_sync::process_restart::service_invocation_phases::before_dispatch_crash_then_restart_dispatches_once";

        if phase_mode().as_deref() == Some("crashed-before-anything") {
            // The directory is never even opened: this simulates the
            // earliest possible crash, strictly before dispatch.
            std::process::exit(66);
        }
        if phase_mode().as_deref() == Some("first-attempt-after-restart") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("fresh restart holds directory");
            let mut store = namespace_store(&directory);
            let mut adapter = RecordingAdapter::default();
            let outcome = deliver_http_durable(
                &mut store,
                2,
                None,
                svc_capability(),
                svc_request(),
                &mut adapter,
            )
            .expect("first attempt after an early crash dispatches normally");
            assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Dispatched(_)));
            assert_eq!(adapter.0.len(), 1);
            return;
        }

        let temp = TempDirectory::new();
        spawn_phase_child(
            TEST_NAME,
            "crashed-before-anything",
            temp.path(),
            &[],
            ExpectedExit::Code(66),
        );
        spawn_phase_child(
            TEST_NAME,
            "first-attempt-after-restart",
            temp.path(),
            &[],
            ExpectedExit::Success,
        );
    }

    /// Phase 1 addition: the intent -- both the identity marker and the typed
    /// provisional checkpoint -- is durably committed, but the process
    /// crashes before the ledger ever calls into the adapter (a strictly
    /// earlier crash point than the "uncertain" phase below, where the
    /// adapter *is* entered). Per this module's documented contract, a
    /// durably committed marker is never cleared: restart finds it and stays
    /// permanently `Uncertain` rather than guessing an outcome or
    /// redispatching.
    #[test]
    fn intent_committed_crash_before_send_then_restart_is_permanently_uncertain() {
        const TEST_NAME: &str = "outbound_delivery_store::tests::namespace_sync::process_restart::service_invocation_phases::intent_committed_crash_before_send_then_restart_is_permanently_uncertain";

        if phase_mode().as_deref() == Some("producer-crashes-after-intent-commit") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("producer holds directory");
            let mut session = HttpDeliverySession::new(2).expect("bounded session");
            let prepared = prepare_http_delivery(svc_capability(), svc_request())
                .expect("admitted fixture request");
            let identity_key = prepared.pending_identity_key().to_owned();
            let mut store = namespace_store(&directory);
            let mut crash_store = CommitIntentThenCrash {
                inner: &mut store,
                identity_key,
                probed: false,
            };
            let mut adapter = PanicOnRedispatch;
            let _ = session.reconcile_durable(prepared, &mut crash_store, &mut adapter);
            unreachable!(
                "CommitIntentThenCrash always exits the process after committing the intent"
            );
        }
        if phase_mode().as_deref() == Some("restart-after-intent-crash") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("restart holds directory");
            let mut store = namespace_store(&directory);
            let mut adapter = PanicOnRedispatch;
            let outcome = deliver_http_durable(
                &mut store,
                2,
                None,
                svc_capability(),
                svc_request(),
                &mut adapter,
            )
            .expect("a guarded fresh session settles instead of erroring");
            assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Uncertain));
            return;
        }

        let temp = TempDirectory::new();
        spawn_phase_child(
            TEST_NAME,
            "producer-crashes-after-intent-commit",
            temp.path(),
            &[],
            ExpectedExit::Code(CRASH_AFTER_INTENT_CODE),
        );
        spawn_phase_child(
            TEST_NAME,
            "restart-after-intent-crash",
            temp.path(),
            &[],
            ExpectedExit::Success,
        );
    }

    /// Phase 2: a crash after the provisional intent is durably committed and
    /// after the (real, in-process fixture) transport was actually entered,
    /// but before the terminal acknowledgment. Restart with a brand-new
    /// session must surface `Uncertain` and must never re-enter the adapter.
    #[test]
    fn uncertain_after_dispatch_before_ack_crash_then_restart_is_uncertain_and_not_redispatched() {
        const TEST_NAME: &str = "outbound_delivery_store::tests::namespace_sync::process_restart::service_invocation_phases::uncertain_after_dispatch_before_ack_crash_then_restart_is_uncertain_and_not_redispatched";

        if phase_mode().as_deref() == Some("producer-crashes-after-dispatch") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("producer holds directory");
            let mut store = namespace_store(&directory);
            let mut adapter = DispatchThenCrash {
                directory: &directory_path,
            };
            // This call never returns: `DispatchThenCrash` exits the process
            // from inside `send`, after the provisional checkpoint is
            // already durably committed by `deliver_http_durable`.
            let _ = deliver_http_durable(
                &mut store,
                2,
                None,
                svc_capability(),
                svc_request(),
                &mut adapter,
            );
            unreachable!("DispatchThenCrash always exits the process");
        }
        if phase_mode().as_deref() == Some("restart-surfaces-uncertain") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("restart holds directory");
            let mut store = namespace_store(&directory);
            let mut adapter = PanicOnRedispatch;
            let outcome = deliver_http_durable(
                &mut store,
                2,
                None,
                svc_capability(),
                svc_request(),
                &mut adapter,
            )
            .expect("a guarded fresh session settles instead of erroring");
            assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Uncertain));
            return;
        }

        let temp = TempDirectory::new();
        spawn_phase_child(
            TEST_NAME,
            "producer-crashes-after-dispatch",
            temp.path(),
            &[],
            ExpectedExit::Code(CRASH_AFTER_DISPATCH_CODE),
        );
        assert!(
            temp.path().join(DISPATCH_ENTERED_MARKER).exists(),
            "the producer must have actually entered the adapter before its simulated crash"
        );
        spawn_phase_child(
            TEST_NAME,
            "restart-surfaces-uncertain",
            temp.path(),
            &[],
            ExpectedExit::Success,
        );
    }

    /// Phase 3: a fully settled delivery. The host's own separately retained
    /// terminal digest/capacity (written here to a plain marker file, playing
    /// the part of the host's own durable job manifest) lets a fresh restart
    /// replay through the production entry point without redispatch.
    #[test]
    fn settled_then_restart_replays_via_retained_terminal_reference() {
        const TEST_NAME: &str = "outbound_delivery_store::tests::namespace_sync::process_restart::service_invocation_phases::settled_then_restart_replays_via_retained_terminal_reference";
        const REFERENCE_FILE: &str = "svc-terminal-reference.txt";

        if phase_mode().as_deref() == Some("producer-settles") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("producer holds directory");
            let mut store = namespace_store(&directory);
            // The producer completes its settled delivery through the exact
            // same underlying session type `deliver_http_durable` uses; it
            // additionally needs the live session afterward to read back the
            // checkpoint digest a real host would durably retain itself.
            let mut session = HttpDeliverySession::new(2).expect("bounded session");
            let mut adapter = RecordingAdapter::default();
            let prepared = prepare_http_delivery(svc_capability(), svc_request())
                .expect("admitted fixture request");
            let outcome = session
                .reconcile_durable(prepared, &mut store, &mut adapter)
                .expect("producer completes a settled delivery");
            assert!(
                matches!(outcome, DurableHttpDeliveryOutcome::Dispatched(_)),
                "the producer must have actually dispatched, not replayed or refused"
            );
            assert_eq!(
                adapter.0.len(),
                1,
                "the producer must have entered the adapter exactly once"
            );
            let checkpoint = session.session_checkpoint().expect("terminal checkpoint");
            // A real host durably retains its own reference to the exact
            // checkpoint outside this module; this simulates that with a
            // plain marker file rather than deriving it from configuration.
            let reference_path = directory_path.join(REFERENCE_FILE);
            fs::write(&reference_path, checkpoint.digest())
                .expect("record the host's own retained terminal reference");
            assert!(
                reference_path.exists(),
                "the host's own retained terminal reference must be durably written before restart"
            );
            return;
        }
        if phase_mode().as_deref() == Some("restart-replays") {
            let directory_path = env_directory();
            let directory =
                platform::hold_directory(&directory_path).expect("restart holds directory");
            let mut store = namespace_store(&directory);
            let digest = fs::read_to_string(directory_path.join(REFERENCE_FILE))
                .expect("read the host's own retained terminal reference");
            let mut adapter = PanicOnRedispatch;
            let outcome = deliver_http_durable(
                &mut store,
                2,
                Some(PriorTerminalReference {
                    digest: digest.trim(),
                    capacity: 2,
                }),
                svc_capability(),
                svc_request(),
                &mut adapter,
            )
            .expect("a retained terminal reference restores and replays");
            assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Replayed(_)));
            return;
        }

        let temp = TempDirectory::new();
        spawn_phase_child(
            TEST_NAME,
            "producer-settles",
            temp.path(),
            &[],
            ExpectedExit::Success,
        );
        spawn_phase_child(
            TEST_NAME,
            "restart-replays",
            temp.path(),
            &[],
            ExpectedExit::Success,
        );
    }
}
