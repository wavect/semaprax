//! Candidate-test feedback and uncertain-observation regressions.

use super::*;

#[test]
fn v2_failed_candidate_test_feedback_reaches_the_next_real_provider_prompt() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) =
        setup_v2_with_feedback_turn(&fixture, "test.repair.v2-test-feedback.v1");
    let project_before = project_tree(&fixture.0.join("project"));
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let test_calls = Rc::new(Cell::new(0));
    let subjects = Rc::new(RefCell::new(Vec::new()));
    let mut observer = RecordedCandidateTestObserver {
        reply: CandidateTestReply::Canonical {
            status: "failed",
            detail: "the candidate test must be repaired",
        },
        calls: Rc::clone(&test_calls),
        subjects,
    };
    let capability = CandidateTestCapability::host_selected("test.feedback.v1").unwrap();
    let mut host = CandidateTestHost::new(capability, &mut observer);
    let refused = execute_with_runner_and_candidate_test(
        v2_command("run", config, checkpoint, scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([
                proposal(&digest, "0", "0"),
                proposal(&digest, "7", "1"),
                proposal(&digest, "0", "0"),
            ]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
        Some(&mut host),
    )
    .unwrap_err();
    // The feedback reaches the third provider turn, but its final no-change
    // proposal has no candidate preview from which to commit a receipt.
    assert_eq!(
        refused.reason,
        "repair terminal candidate preview is unavailable"
    );
    assert_eq!(test_calls.get(), 1);
    assert_eq!(calls.get(), 3, "the third request must reach the provider");
    let third: serde_json::Value = serde_json::from_str(&prompts.borrow()[2]).unwrap();
    let feedback = third["previous_effect_hex"]
        .as_str()
        .expect("the next provider request carries the settled effect feedback");
    let bytes = decode_hex(feedback);
    let feedback: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let code = feedback["fields"][0][1]
        .as_str()
        .and_then(|value| value.parse::<i64>().ok())
        .expect("candidate-test feedback remains a canonically encoded typed i64 result");
    assert!(
        code < 0,
        "the actual failed test must feed a negative result"
    );
    assert_eq!(project_tree(&fixture.0.join("project")), project_before);
}
#[test]
fn v2_candidate_test_observation_refuses_malformed_oversized_and_withheld_output() {
    unix_checkpoint_host!();
    for raw in [
        Vec::new(),
        b"{not canonical}".to_vec(),
        vec![b'x'; MAX_CANDIDATE_TEST_OBSERVATION_BYTES + 1],
    ] {
        let fixture = Fixture::new();
        let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2-test-hostile.v1");
        let scratch = fixture.0.join("scratch");
        fs::create_dir(&scratch).unwrap();
        let calls = Rc::new(Cell::new(0));
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let test_calls = Rc::new(Cell::new(0));
        let subjects = Rc::new(RefCell::new(Vec::new()));
        let mut observer = RecordedCandidateTestObserver {
            reply: CandidateTestReply::Raw(raw),
            calls: Rc::clone(&test_calls),
            subjects,
        };
        let capability = CandidateTestCapability::host_selected("test.hostile.v1").unwrap();
        let mut host = CandidateTestHost::new(capability, &mut observer);
        let source_path = fixture.0.join("project/src/app.spx");
        let source_before = fs::read(&source_path).unwrap();
        let project_before = project_tree(&fixture.0.join("project"));
        assert!(execute_with_runner_and_candidate_test(
            v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
            RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
                last_answer: None,
                prompts,
                calls: Rc::clone(&calls),
            },
            Some(&mut host),
        )
        .is_err());
        assert_eq!(calls.get(), 2);
        assert_eq!(test_calls.get(), 1);
        assert_eq!(fs::read(&source_path).unwrap(), source_before);
        assert_eq!(project_tree(&fixture.0.join("project")), project_before);
        let refused = execute_with_runner_and_candidate_test(
            v2_command("resume", config, checkpoint, scratch),
            RecordedOpenCodeRunner {
                answers: VecDeque::new(),
                last_answer: None,
                prompts: Rc::new(RefCell::new(Vec::new())),
                calls: Rc::clone(&calls),
            },
            Some(&mut host),
        )
        .unwrap_err();
        // The malformed observation leaves a terminal checkpoint without its
        // committed patch receipt. Recovery fails closed before redispatch.
        assert_eq!(refused.reason, "terminal patch receipt is unavailable");
        assert_eq!(
            calls.get(),
            2,
            "uncertain observer work must not redispatch"
        );
        assert_eq!(test_calls.get(), 1);
        assert_eq!(fs::read(&source_path).unwrap(), source_before);
        assert_eq!(project_tree(&fixture.0.join("project")), project_before);
    }
}
