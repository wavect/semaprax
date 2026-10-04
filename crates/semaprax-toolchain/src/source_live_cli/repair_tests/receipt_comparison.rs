//! Repair alternatives compare their final candidate receipts, never run totals.

use super::*;

#[test]
fn repair_route_with_rejected_attempt_compares_final_candidates() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.receipt-comparison.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));

    let report = execute_with_runner(
        v2_command("run", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts,
            calls,
        },
    )
    .unwrap();
    let report: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["rejected_candidates"], 1);
    assert_eq!(
        report["runtime_effect_accounting"]["total_model_attempts"],
        2
    );
    assert_eq!(
        report["runtime_effect_accounting"]["effect_budget"]["cumulative_terminal_journal"]
            ["dispatched_calls"],
        2
    );
    assert_eq!(
        report["runtime_effect_accounting"]["effect_budget"]["this_invocation"]["dispatched_calls"],
        2
    );

    let sidecar: Value =
        serde_json::from_slice(&fs::read(checkpoint.join("terminal-patch-receipt.json")).unwrap())
            .unwrap();
    let repaired_receipt = sidecar["receipt"]
        .as_str()
        .expect("terminal sidecar retains canonical candidate receipt");
    assert_eq!(
        report["patch_receipt"],
        serde_json::from_str::<Value>(repaired_receipt).unwrap(),
        "the repair report projects the retained final candidate receipt"
    );

    let revision =
        with_authenticated_project(&fixture.0.join("project/semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    let envelope = OfflineRepairEnvelope::new(revision, "fixture.repair.value").unwrap();
    let repaired = envelope.preview(7, false).unwrap();
    let alternative = envelope.preview(8, false).unwrap();
    let repaired_digest = repaired.candidate().candidate_digest();
    let alternative_digest = alternative.candidate().candidate_digest();
    let alternative_receipt = alternative
        .candidate()
        .patch_receipt(alternative_digest)
        .unwrap();

    let repaired_verification: Value = serde_json::from_str(
        &repaired
            .candidate()
            .verify_patch_receipt(repaired_digest, repaired_receipt.as_bytes())
            .unwrap(),
    )
    .unwrap();
    let alternative_verification: Value = serde_json::from_str(
        &alternative
            .candidate()
            .verify_patch_receipt(alternative_digest, alternative_receipt.as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(repaired_verification["result"], "exact_recomputation");
    assert_eq!(alternative_verification["result"], "exact_recomputation");
    assert_ne!(repaired_digest, alternative_digest);

    let comparison: Value = serde_json::from_str(
        &repaired
            .candidate()
            .compare_patch_receipts(
                repaired_digest,
                repaired_receipt.as_bytes(),
                alternative.candidate(),
                alternative_digest,
                alternative_receipt.as_bytes(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(comparison["result"], "comparable");
    assert_eq!(comparison["reasons"], json!([]));
    assert_eq!(
        comparison["comparison"]["effect_usage"]["left"]["status"],
        "not_applicable"
    );
    assert_eq!(
        comparison["comparison"]["effect_usage"]["right"]["status"],
        "not_applicable"
    );
    assert_eq!(
        report["patch_receipt"]["content"]["policy"]["effect_accounting_scope"],
        "not_applicable"
    );
    assert!(
        report["patch_receipt"]
            .get("runtime_effect_accounting")
            .is_none(),
        "candidate receipt does not relabel cumulative repair-run usage as candidate usage"
    );
}
