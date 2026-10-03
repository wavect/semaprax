//! Regression for the operator-only post-settlement interruption seam.

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::*;

#[test]
fn post_settled_pause_is_an_ordered_explicit_opencode_operand() {
    let arguments = [
        "run",
        "/config.json",
        "/checkpoint",
        "--opencode",
        "/bin/opencode",
        "--scratch",
        "/scratch",
        "--pause-after-settled",
    ]
    .map(str::to_owned);
    let Command::Run {
        provider: Some(provider),
        ..
    } = Command::parse(&arguments).expect("ordered pause operands are admitted")
    else {
        panic!("pause is available only with the explicit OpenCode provider");
    };
    assert!(provider.pause_after_settled);
    let unordered = [
        "run",
        "/config.json",
        "/checkpoint",
        "--pause-after-settled",
        "--opencode",
        "/bin/opencode",
        "--scratch",
        "/scratch",
    ]
    .map(str::to_owned);
    assert!(Command::parse(&unordered).is_err());
}

/// The checkpoint wrapper observes the real physical `attempt_settled` ACK
/// before the source loop can decode the response or invoke an effect. A
/// resumed invocation replays that settled response, dispatches its following
/// effect, and sends only the later provider attempt.
#[test]
fn repair_v2_post_settled_barrier_precedes_effect_and_resume_never_redispatches_it() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2.post-settled.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let markers = Rc::new(RefCell::new(Vec::new()));
    let hook_markers = Rc::clone(&markers);
    let marker_path = scratch.join(".semaprax-repair-post-settled-pause.json");
    let hook_marker_path = marker_path.clone();
    reset_test_effect_handler_calls();
    super::barrier::set_test_post_settled_hook(move |marker| {
        assert_eq!(test_effect_handler_calls(), 0, "ACK pause precedes effects");
        assert!(
            hook_marker_path.is_file(),
            "marker is visible before the pause"
        );
        hook_markers.borrow_mut().push(marker);
        panic!("test interruption immediately after durable attempt_settled ACK");
    });

    let mut command = v2_command("run", config.clone(), checkpoint.clone(), scratch.clone());
    let Command::Run {
        provider: Some(provider),
        ..
    } = &mut command
    else {
        panic!("V2 helper supplies the explicit provider operands");
    };
    provider.pause_after_settled = true;

    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        execute_with_runner(
            command,
            RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0")]),
                last_answer: None,
                prompts: Rc::clone(&prompts),
                calls: Rc::clone(&calls),
            },
        )
    }));
    assert!(interrupted.is_err(), "test hook models controller SIGKILL");
    assert_eq!(
        calls.get(),
        1,
        "one provider response reached the durable ACK"
    );
    assert_eq!(
        test_effect_handler_calls(),
        0,
        "the interrupted run called no effect"
    );
    assert!(
        marker_path.is_file(),
        "the scratch marker survives the interruption"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    assert_eq!(
        marker["schema"],
        "semaprax.source-live-cli.repair-post-settled-pause.v1"
    );

    let checkpoint_document = checkpoint.join("checkpoint.json");
    let paused: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint_document).unwrap()).unwrap();
    let settled = paused["entries"]
        .as_array()
        .and_then(|entries| entries.last())
        .expect("the durable document retains its final causal entry");
    assert_eq!(settled["kind"], "attempt_settled");
    assert!(
        paused["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["kind"] != "effect_intent" && entry["kind"] != "effect_observed"),
        "nothing after the settled ACK invoked or recorded an effect"
    );
    let markers = markers.borrow();
    assert_eq!(markers.len(), 1, "the operator marker is one-shot");
    assert!(markers[0].generation > 0);
    assert_eq!(markers[0].turn, settled["turn"].as_u64().unwrap() as u32);
    assert_eq!(
        markers[0].attempt,
        settled["attempt"].as_u64().unwrap() as u32
    );
    assert_eq!(
        markers[0].response_digest,
        settled["response_digest"].as_str().unwrap()
    );
    assert_eq!(marker["invocation"], paused["invocation"]);
    assert_eq!(marker["turn"], settled["turn"]);
    assert_eq!(marker["attempt"], settled["attempt"]);
    assert_eq!(marker["response_digest"], settled["response_digest"]);
    drop(markers);

    let resumed = execute_with_runner(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .expect("disabled-barrier resume completes the retained repair");
    let resumed: serde_json::Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(resumed["status"], "complete");
    assert_eq!(resumed["model_dispatches"], 1);
    assert_eq!(resumed["effect_dispatches"], 2);
    assert!(
        !marker_path.exists(),
        "disabled-barrier resume authenticates and removes its owned marker"
    );
    assert_eq!(
        calls.get(),
        2,
        "resume replays the settled attempt and dispatches only the later one"
    );
    assert_eq!(test_effect_handler_calls(), 2);
}

/// A scratch occupant with the right marker shape cannot make resume delete
/// controller-owned bytes. Only the authenticated checkpoint's exact settled
/// entry supplies the canonical marker that may be removed.
#[test]
fn repair_v2_post_settled_resume_preserves_a_forged_marker() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2.post-settled.forged.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let marker_path = scratch.join(".semaprax-repair-post-settled-pause.json");
    reset_test_effect_handler_calls();
    super::barrier::set_test_post_settled_hook(|_| {
        panic!("test interruption immediately after durable attempt_settled ACK");
    });

    let mut command = v2_command("run", config.clone(), checkpoint.clone(), scratch.clone());
    let Command::Run {
        provider: Some(provider),
        ..
    } = &mut command
    else {
        panic!("V2 helper supplies the explicit provider operands");
    };
    provider.pause_after_settled = true;
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        execute_with_runner(
            command,
            RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0")]),
                last_answer: None,
                prompts: Rc::clone(&prompts),
                calls: Rc::clone(&calls),
            },
        )
    }));
    assert!(interrupted.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(test_effect_handler_calls(), 0);

    let mut forged: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    forged["response_digest"] = serde_json::Value::String("f".repeat(64));
    let forged = serde_json::to_vec(&forged).unwrap();
    fs::write(&marker_path, &forged).unwrap();

    let refusal = execute_with_runner(
        v2_command("resume", config, checkpoint, scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .expect_err("mismatched pause marker must refuse before provider or effect work");
    assert_eq!(
        refusal.reason,
        "repair OpenCode post-settlement pause marker does not match authenticated checkpoint"
    );
    assert_eq!(
        fs::read(&marker_path).unwrap(),
        forged,
        "the mismatched scratch bytes remain available to their controller"
    );
    assert_eq!(
        calls.get(),
        1,
        "refusal cannot redispatch the settled attempt"
    );
    assert_eq!(
        test_effect_handler_calls(),
        0,
        "refusal cannot invoke an effect"
    );
}

/// Exercise the real physical ACK/marker path in a killable child process.
/// A queued token and repeated explicit wakes must never admit the next stage.
#[cfg(unix)]
#[test]
fn physical_post_settled_pause_survives_unpark() {
    use semaprax::agent_lifecycle::CheckpointStore;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Child, Command as ProcessCommand, Stdio};
    use std::time::Instant;
    const CHILD_ROOT: &str = "SEMAPRAX_TEST_POST_SETTLED_UNPARK_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        let scratch = root.join("scratch");
        fs::create_dir(&scratch).unwrap();
        let executable = root.join("never-dispatched");
        fs::write(&executable, "#!/bin/sh\nexit 99\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let host = OpenCodeHostConfig::new_process_image(
            executable,
            scratch,
            Duration::from_secs(1),
            4096,
        )
        .unwrap();
        let mut store =
            CheckpointDir::fresh(&root.join("checkpoint"), &root.join("project")).unwrap();
        let hook_root = root.clone();
        super::barrier::set_test_post_settled_hook(move |_| {
            let paused = std::thread::current();
            paused.unpark(); // Queue a token before the first park.
            std::thread::spawn(move || {
                for _ in 0..32 {
                    paused.unpark();
                    std::thread::sleep(Duration::from_millis(2));
                }
                fs::write(hook_root.join("unparks-finished"), b"done").unwrap();
            });
        });
        let mut barrier = super::barrier::PostSettledBarrierStore::new(&mut store, Some(host));
        let document = r#"{"invocation":"pause-fixture","entries":[{"kind":"attempt_settled","turn":0,"attempt":0,"response_digest":"fixture-digest"}]}"#;
        barrier.commit(1, document).unwrap();
        fs::write(root.join("escaped-pause"), b"unsafe continuation").unwrap();
        return;
    }
    struct KillChild(Child);
    impl Drop for KillChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let fixture = Fixture::new();
    let mut child = KillChild(ProcessCommand::new(std::env::current_exe().unwrap())
        .args(["--exact", "source_live_cli::repair::tests::post_settled_barrier::physical_post_settled_pause_survives_unpark", "--nocapture"])
        .env(CHILD_ROOT, &fixture.0)
        .stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !fixture.0.join("unparks-finished").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "pause child exited before wake attempts completed"
        );
        assert!(
            Instant::now() < deadline,
            "pause child did not publish bounded wake evidence"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(fixture
        .0
        .join("scratch/.semaprax-repair-post-settled-pause.json")
        .is_file());
    assert!(fixture.0.join("checkpoint/checkpoint.json").is_file());
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "physical pause returned after unpark"
    );
    assert!(
        !fixture.0.join("escaped-pause").exists(),
        "pause cannot authorize subsequent work"
    );
}
