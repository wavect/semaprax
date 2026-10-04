//! Fixed `repair-tested` host-profile regressions live outside the already
//! budgeted main repair harness.

use super::*;
use crate::source_live_cli::candidate_test::{
    FixedCandidateTestObserver, REPAIR_TEST_CAPABILITY_ID,
};

#[test]
fn fixed_candidate_test_profile_runs_the_real_candidate_test_and_replays_it() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair-tested.success.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let mut first_observer = FixedCandidateTestObserver::new();
    let mut first_host = CandidateTestHost::new(
        CandidateTestCapability::host_selected(REPAIR_TEST_CAPABILITY_ID).unwrap(),
        &mut first_observer,
    );
    let first = execute_with_runner_and_candidate_test(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
        Some(&mut first_host),
    )
    .unwrap();
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first["candidate_test_execution"]["status"], "passed");
    assert_eq!(first["candidate_test_execution"]["replayed"], false);
    assert!(first["candidate_test_execution"]["observation"]["detail"]
        .as_str()
        .is_some_and(|detail| detail.starts_with("candidate-test-report=sha256:")));
    assert_eq!(
        first["receipt_policy"]["coverage"]["candidate_test_execution"]["status"],
        "passed"
    );
    assert_eq!(
        first["receipt_policy"]["coverage"]["candidate_test_execution"]["coverage"],
        "partial_authorized_candidate_test_observation"
    );
    assert_eq!(
        first["receipt_policy"]["coverage"]["runtime_effects"]["this_invocation"]
            ["model_dispatches"],
        2
    );
    assert_eq!(
        first["receipt_policy"]["coverage"]["runtime_effects"]["cumulative_terminal_journal"]
            ["model_attempts"],
        2
    );
    assert_eq!(calls.get(), 2);
    let checkpoint_before = fs::read(checkpoint.join("checkpoint.json")).unwrap();

    let mut resumed_observer = FixedCandidateTestObserver::new();
    let mut resumed_host = CandidateTestHost::new(
        CandidateTestCapability::host_selected(REPAIR_TEST_CAPABILITY_ID).unwrap(),
        &mut resumed_observer,
    );
    let resumed = execute_with_runner_and_candidate_test(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
        Some(&mut resumed_host),
    )
    .unwrap();
    let resumed: serde_json::Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(resumed["candidate_test_execution"]["status"], "passed");
    assert_eq!(resumed["candidate_test_execution"]["replayed"], true);
    assert_eq!(
        resumed["receipt_policy"]["coverage"]["candidate_test_execution"]["coverage"],
        "partial_replayed_candidate_test_feedback_only"
    );
    assert_eq!(
        resumed["receipt_policy"]["coverage"]["runtime_effects"]["this_invocation"]
            ["model_dispatches"],
        0
    );
    assert_eq!(
        resumed["receipt_policy"]["coverage"]["runtime_effects"]["cumulative_terminal_journal"]
            ["model_attempts"],
        2
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(
        fs::read(checkpoint.join("checkpoint.json")).unwrap(),
        checkpoint_before
    );
}

#[test]
fn repair_tested_refuses_a_scripted_configuration_before_a_checkpoint_exists() {
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair-tested.refusal.v1");
    let error = crate::source_live_cli::run_repair_tested(&[
        "run".to_owned(),
        config.to_str().unwrap().to_owned(),
        checkpoint.to_str().unwrap().to_owned(),
    ])
    .expect_err("repair-tested must not grant candidate-test authority to scripted repair");
    assert_eq!(
        error.reason,
        "candidate-test capability requires OpenCode repair configuration"
    );
    assert!(!checkpoint.exists());
}

#[test]
fn interrupted_resume_reports_live_candidate_test_then_terminal_replay() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair-tested.interrupted.v1");
    let before = project_tree(&fixture.0.join("project"));
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let mut observer = FixedCandidateTestObserver::new();
    let mut host = CandidateTestHost::new(
        CandidateTestCapability::host_selected(REPAIR_TEST_CAPABILITY_ID).unwrap(),
        &mut observer,
    );
    super::barrier::set_test_post_settled_hook(|_| {
        panic!("interrupt after settled ACK before candidate test");
    });
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        execute_with_runner_and_candidate_test(
            v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
            RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0")]),
                last_answer: None,
                prompts: Rc::clone(&prompts),
                calls: Rc::clone(&calls),
            },
            Some(&mut host),
        )
    }));
    assert!(interrupted.is_err());
    assert_eq!(calls.get(), 1);
    let paused: Value =
        serde_json::from_slice(&fs::read(checkpoint.join("checkpoint.json")).unwrap()).unwrap();
    assert_eq!(
        paused["entries"].as_array().unwrap().last().unwrap()["kind"],
        "attempt_settled"
    );
    let live = execute_with_runner_and_candidate_test(
        v2_command(
            "resume",
            config.clone(),
            checkpoint.clone(),
            scratch.clone(),
        ),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
        Some(&mut host),
    )
    .unwrap();
    let live: Value = serde_json::from_str(&live).unwrap();
    assert_eq!(live["status"], "complete");
    assert_eq!(live["model_dispatches"], 1);
    assert_eq!(live["effect_dispatches"], 2);
    assert_eq!(live["candidate_test_execution"]["status"], "passed");
    assert_eq!(live["candidate_test_execution"]["replayed"], false);
    assert!(live["candidate_test_execution"]["observation"].is_object());
    assert_eq!(
        calls.get(),
        2,
        "settled first attempt is never redispatched"
    );
    let completed = fs::read(checkpoint.join("checkpoint.json")).unwrap();
    let replay = execute_with_runner_and_candidate_test(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
        Some(&mut host),
    )
    .unwrap();
    let replay: Value = serde_json::from_str(&replay).unwrap();
    assert_eq!(replay["model_dispatches"], 0);
    assert_eq!(replay["effect_dispatches"], 0);
    assert_eq!(replay["candidate_test_execution"]["status"], "passed");
    assert_eq!(replay["candidate_test_execution"]["replayed"], true);
    assert!(replay["candidate_test_execution"]["observation"].is_null());
    assert_eq!(calls.get(), 2);
    assert_eq!(
        fs::read(checkpoint.join("checkpoint.json")).unwrap(),
        completed
    );
    assert_eq!(project_tree(&fixture.0.join("project")), before);
}
