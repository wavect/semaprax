//! Recovery-only repair CLI regressions live here so the main integration
//! harness remains within the repository's per-module source budget.

use super::*;

/// A terminal V1 fixture journal is already sufficient for its immutable
/// receipt. Recovery must therefore happen before fixture-only target lookup
/// or diagnostic derivation: an invalid replacement target is not authority
/// to turn a replay into candidate construction.
#[test]
fn repair_v1_terminal_resume_skips_fixture_target_and_diagnostic_derivation() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.fixture-preflight.v1");
    run_repair("run", &config, &checkpoint).unwrap();
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let bytes_before = fs::read(&checkpoint_document).unwrap();

    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    // `target` is intentionally a syntactically valid token but names no
    // Project declaration. The old pre-replay envelope construction would
    // refuse it; a terminal replay must not reach that construction.
    value["target"] = serde_json::json!("fixture.repair.missing");
    value["malformed_replacement"] = serde_json::json!(7);
    value["malformed_bool_literal"] = serde_json::json!(false);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let resumed: serde_json::Value =
        serde_json::from_str(&run_repair("resume", &config, &checkpoint).unwrap()).unwrap();
    assert_eq!(resumed["status"], "complete");
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(resumed["candidate_digest"], serde_json::Value::Null);
    assert_eq!(fs::read(&checkpoint_document).unwrap(), bytes_before);
}

/// V2 recovery admits and classifies hostile retained input before it creates
/// an OpenCode adapter, effect handler, or candidate-test host. The wire cases
/// cover hex admission plus the exact-key and sequence checks around a settled
/// response; the binding cases prove those bytes cannot be replayed under a
/// changed checked source or task.
#[test]
fn repair_v2_hostile_resume_inputs_have_stable_pre_replay_refusals() {
    unix_checkpoint_host!();
    for (case, expected) in [
        (
            "malformed-settled-wire",
            "repair V2 retained checkpoint is malformed",
        ),
        (
            "settled-wire-extra-field",
            "repair V2 retained checkpoint is malformed",
        ),
        (
            "settled-wire-sequence-mismatch",
            "repair V2 retained checkpoint is malformed",
        ),
        (
            "envelope-extra-field",
            "repair V2 retained checkpoint is malformed",
        ),
        (
            "stale-source",
            "repair V2 checkpoint binding is stale or mismatched",
        ),
        (
            "wrong-task-binding",
            "repair V2 checkpoint binding is stale or mismatched",
        ),
    ] {
        let fixture = Fixture::new();
        let (config, checkpoint, digest) = setup_v2(&fixture, &format!("test.repair.v2.{case}.v1"));
        let scratch = fixture.0.join("scratch");
        fs::create_dir(&scratch).unwrap();
        let calls = Rc::new(Cell::new(0));
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let candidate_calls = Rc::new(Cell::new(0));
        reset_test_effect_handler_calls();
        let capability = || CandidateTestCapability::host_selected("preflight").unwrap();
        let mut first_observer = RecordedCandidateTestObserver {
            reply: CandidateTestReply::Canonical {
                status: "passed",
                detail: "candidate test passed",
            },
            calls: Rc::clone(&candidate_calls),
            subjects: Rc::new(RefCell::new(Vec::new())),
        };
        let mut first_host = CandidateTestHost::new(capability(), &mut first_observer);
        execute_with_runner_and_candidate_test(
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
        assert_eq!(calls.get(), 2, "{case} starts from a real settled V2 wire");
        assert_eq!(candidate_calls.get(), 1, "{case}: fresh candidate count");
        assert_eq!(test_effect_handler_calls(), 2, "{case}: fresh effect count");
        let checkpoint_document = checkpoint.join("checkpoint.json");
        let source_path = fixture.0.join("project/src/app.spx");
        let config_value: serde_json::Value =
            serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
        let task_path = PathBuf::from(config_value["task_path"].as_str().unwrap());
        match case {
            "malformed-settled-wire"
            | "settled-wire-extra-field"
            | "settled-wire-sequence-mismatch"
            | "envelope-extra-field" => {
                let mut journal: serde_json::Value =
                    serde_json::from_slice(&fs::read(&checkpoint_document).unwrap()).unwrap();
                if case == "envelope-extra-field" {
                    journal["unexpected"] = serde_json::json!("hostile");
                } else {
                    let settled = journal["entries"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|entry| entry["kind"] == "attempt_settled")
                        .unwrap();
                    match case {
                        // This reaches bounded response-hex admission before
                        // response replay or chain acceptance.
                        "malformed-settled-wire" => settled["response"] = serde_json::json!("g"),
                        "settled-wire-extra-field" => {
                            settled["unexpected"] = serde_json::json!("hostile")
                        }
                        "settled-wire-sequence-mismatch" => settled["seq"] = serde_json::json!(0),
                        _ => unreachable!("closed settled-wire inventory"),
                    }
                }
                fs::write(&checkpoint_document, serde_json::to_vec(&journal).unwrap()).unwrap();
            }
            "stale-source" => {
                let changed = opencode_app_source().replacen(
                    "@id(\"fixture.repair.value\")\nfn repair_value() -> i64\n{\n    0\n}",
                    "@id(\"fixture.repair.value\")\nfn repair_value() -> i64\n{\n    9\n}",
                    1,
                );
                assert_ne!(changed, opencode_app_source());
                fs::write(&source_path, changed).unwrap();
            }
            "wrong-task-binding" => fs::write(&task_path, b"a different checked task").unwrap(),
            _ => unreachable!("closed hostile fixture inventory"),
        }
        let bytes_before = fs::read(&checkpoint_document).unwrap();
        let source_after_mutation = fs::read(&source_path).unwrap();
        let task_after_mutation = fs::read(&task_path).unwrap();
        let mut resumed_observer = RecordedCandidateTestObserver {
            reply: CandidateTestReply::Canonical {
                status: "passed",
                detail: "candidate test passed",
            },
            calls: Rc::clone(&candidate_calls),
            subjects: Rc::new(RefCell::new(Vec::new())),
        };
        let mut resumed_host = CandidateTestHost::new(capability(), &mut resumed_observer);
        let error = execute_with_runner_and_candidate_test(
            v2_command("resume", config, checkpoint.clone(), scratch),
            RecordedOpenCodeRunner {
                answers: VecDeque::new(),
                last_answer: None,
                prompts,
                calls: Rc::clone(&calls),
            },
            Some(&mut resumed_host),
        )
        .expect_err("hostile V2 checkpoint must refuse before replay");
        assert_eq!(error.reason, expected, "{case}");
        assert_eq!(
            calls.get(),
            2,
            "{case} must not start OpenCode during recovery"
        );
        assert_eq!(
            test_effect_handler_calls(),
            2,
            "{case} must not enter the effect handler during recovery"
        );
        assert_eq!(
            candidate_calls.get(),
            1,
            "{case} must not invoke the candidate-test handler during recovery"
        );
        assert_eq!(
            fs::read(&checkpoint_document).unwrap(),
            bytes_before,
            "{case} must not advance or rewrite the hostile checkpoint"
        );
        assert_eq!(
            fs::read(&source_path).unwrap(),
            source_after_mutation,
            "{case} must preserve the source bytes present at failed resume"
        );
        assert_eq!(
            fs::read(&task_path).unwrap(),
            task_after_mutation,
            "{case} must preserve the task bytes present at failed resume"
        );
    }
}
