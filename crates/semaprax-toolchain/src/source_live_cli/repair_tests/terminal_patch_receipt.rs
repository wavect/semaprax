//! Durable terminal patch-receipt replay belongs with repair recovery tests.

use super::*;

#[test]
fn repair_v2_terminal_resume_reuses_exact_retained_patch_receipt_without_dispatch() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.patch-receipt.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    reset_test_effect_handler_calls();
    let first = execute_with_runner(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let first: Value = serde_json::from_str(&first).unwrap();
    let patch_receipt = first["patch_receipt"].clone();
    assert_eq!(patch_receipt["schema"], "semaprax.patch-receipt.v1");
    assert!(patch_receipt["receipt_digest"].is_string());
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let patch_receipt_document = checkpoint.join("terminal-patch-receipt.json");
    let checkpoint_before = fs::read(&checkpoint_document).unwrap();
    let patch_receipt_before = fs::read(&patch_receipt_document).unwrap();

    let retained = execute_with_runner(
        v2_command(
            "receipt",
            config.clone(),
            checkpoint.clone(),
            scratch.clone(),
        ),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let retained: Value = serde_json::from_str(&retained).unwrap();
    assert_eq!(retained["patch_receipt"], patch_receipt);
    assert_eq!(
        retained["runtime_effect_accounting"]["total_model_attempts"],
        first["runtime_effect_accounting"]["total_model_attempts"]
    );
    assert_eq!(
        retained["runtime_effect_accounting"]["this_invocation_model_dispatches"],
        0
    );
    assert_eq!(
        retained["runtime_effect_accounting"]["effect_budget"],
        first["runtime_effect_accounting"]["effect_budget"],
        "terminal receipt reuses the checkpoint-authenticated charge ledger"
    );
    assert_eq!(
        retained["receipt_policy"]["coverage"]["runtime_effects"]["replayed_without_dispatch"],
        true
    );
    assert_eq!(calls.get(), 2, "terminal receipt must not start OpenCode");
    assert_eq!(
        test_effect_handler_calls(),
        2,
        "terminal receipt must not enter the effect handler"
    );
    assert_eq!(fs::read(&checkpoint_document).unwrap(), checkpoint_before);
    assert_eq!(
        fs::read(&patch_receipt_document).unwrap(),
        patch_receipt_before
    );

    let resumed = execute_with_runner(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let resumed: Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(resumed["patch_receipt"], patch_receipt);
    assert_eq!(
        resumed["patch_receipt"]["receipt_digest"],
        first["patch_receipt"]["receipt_digest"]
    );
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(calls.get(), 2, "terminal resume must not start OpenCode");
    assert_eq!(
        test_effect_handler_calls(),
        2,
        "terminal resume must not enter the effect handler"
    );
    assert_eq!(fs::read(&checkpoint_document).unwrap(), checkpoint_before);
    assert_eq!(
        fs::read(&patch_receipt_document).unwrap(),
        patch_receipt_before
    );
}

#[test]
fn repair_v2_terminal_resume_refuses_patch_receipt_with_foreign_journal_binding() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.patch-receipt-hostile.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    reset_test_effect_handler_calls();
    execute_with_runner(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let patch_receipt_document = checkpoint.join("terminal-patch-receipt.json");
    let mut retained: Value =
        serde_json::from_slice(&fs::read(&patch_receipt_document).unwrap()).unwrap();
    retained["journal_binding"]["chain"] = json!("sha256:foreign-terminal-chain");
    fs::write(
        &patch_receipt_document,
        serde_json::to_vec(&retained).unwrap(),
    )
    .unwrap();
    let checkpoint_before = fs::read(&checkpoint_document).unwrap();
    let patch_receipt_before = fs::read(&patch_receipt_document).unwrap();

    let error = execute_with_runner(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .expect_err("foreign terminal receipt binding must refuse before replay");
    assert_eq!(
        error.reason,
        "terminal patch receipt binding is stale or mismatched"
    );
    assert_eq!(calls.get(), 2, "refusal must not start OpenCode");
    assert_eq!(
        test_effect_handler_calls(),
        2,
        "refusal must not enter the effect handler"
    );
    assert_eq!(fs::read(&checkpoint_document).unwrap(), checkpoint_before);
    assert_eq!(
        fs::read(&patch_receipt_document).unwrap(),
        patch_receipt_before
    );
}

#[test]
fn repair_v2_terminal_resume_refuses_receipt_sidecar_mutation_against_commitment() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) =
        setup_v2(&fixture, "test.repair.patch-receipt-commitment.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    reset_test_effect_handler_calls();
    execute_with_runner(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let patch_receipt_document = checkpoint.join("terminal-patch-receipt.json");
    let commitment_document = checkpoint.join("terminal-patch-receipt-commitment.json");
    let mut retained: Value =
        serde_json::from_slice(&fs::read(&patch_receipt_document).unwrap()).unwrap();
    let limit = &mut retained["runtime_effect_accounting"]["effect_budget"]["effective_limits"]
        ["total_charged_bytes"];
    assert_eq!(*limit, json!(8192));
    *limit = json!(8193);
    fs::write(
        &patch_receipt_document,
        serde_json::to_vec(&retained).unwrap(),
    )
    .unwrap();
    let checkpoint_before = fs::read(&checkpoint_document).unwrap();
    let receipt_before = fs::read(&patch_receipt_document).unwrap();
    let commitment_before = fs::read(&commitment_document).unwrap();

    let error = execute_with_runner(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .expect_err("receipt sidecar mutation must fail its retained checkpoint commitment");
    assert_eq!(
        error.reason,
        "terminal patch receipt commitment is stale or mismatched"
    );
    assert_eq!(calls.get(), 2, "refusal must not start OpenCode");
    assert_eq!(
        test_effect_handler_calls(),
        2,
        "refusal must not enter the effect handler"
    );
    assert_eq!(fs::read(&checkpoint_document).unwrap(), checkpoint_before);
    assert_eq!(fs::read(&patch_receipt_document).unwrap(), receipt_before);
    assert_eq!(fs::read(&commitment_document).unwrap(), commitment_before);
}
