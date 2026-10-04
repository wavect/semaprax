//! Saved LAW-15 finite request-identity pack, using the existing LAW-10 checker.
use super::*;
use semaprax::interpreter::{self, InterpreterOptions};

const SOURCE: &str =
    include_str!("../../../examples/law-packs/finite-retry/identity/src/machine.spx");
const MUTANT: &str = include_str!(
    "../../../examples/law-packs/finite-retry/identity/mutants/identity-confusion.spx"
);
const ID_MANIFEST: &str =
    include_str!("../../../examples/law-packs/finite-retry/identity/semaprax.toml");
const HELPER: &str =
    include_str!("../../../examples/law-packs/finite-retry/identity/src/helper.spx");

fn fixture(source: &str) -> Fixture {
    let parsed = semaprax::parse(source, Path::new("src/machine.spx")).unwrap();
    assert_eq!(semaprax::format::canonical(&parsed), source);
    let result = Fixture::new(source);
    std::fs::write(result.manifest(), ID_MANIFEST).unwrap();
    assert_eq!(
        std::fs::read_to_string(result.0.join("src/helper.spx")).unwrap(),
        HELPER
    );
    result
}

fn evaluate(fixture: &Fixture, state: i64, event: i64) -> i64 {
    let source = fixture.0.join("src/machine.spx");
    let result = interpreter::interpret(
        &source,
        "payment.step",
        &[state.to_string(), event.to_string()],
        &InterpreterOptions::default(),
    )
    .unwrap();
    assert!(result.returned, "{}", result.envelope);
    interpreter::verify_envelope(&result.envelope).unwrap();
    let view: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    view["payload"]["outcome"]["value"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn finite_request_identity_closes_domain_and_refuses_stale_version() {
    let fixture = fixture(SOURCE);
    let report = checked(&fixture, BOUNDS);
    assert_eq!(report.outcome, ProtocolSafetyOutcome::ModelChecked);
    assert_eq!(report.coverage.len(), 8);
    assert_eq!(
        report.state_domain,
        [
            "Idle",
            "Pending",
            "Succeeded",
            "Retry",
            "Failed",
            "Rejected"
        ]
    );
    assert_eq!(
        report.event_domain,
        ["charge", "success", "failure", "retry_a", "retry_b", "abort", "cancel", "timeout"]
    );
    // Same request resumes; a different request terminates without charge.
    assert_eq!(evaluate(&fixture, 3, 3), 0);
    assert_eq!(evaluate(&fixture, 3, 4), 10);
    assert_eq!(evaluate(&fixture, 0, 0), 3);
    for terminal in [2, 4, 5] {
        for event in 0..8 {
            assert_eq!(evaluate(&fixture, terminal, event), -1);
        }
    }
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&report, &snapshot.retain_revision()).map_err(|e| vec![e])
    })
    .unwrap();
    let view: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(
        view["claim"],
        "finite_pure_dispatcher_safety_only_no_external_exactly_once"
    );
    assert_eq!(view["fairness"], "none");
    // Version changes retain behavior but invalidate prior source evidence.
    let version2 = SOURCE.replace("payment-request-identity-v1", "payment-request-identity-v2");
    std::fs::write(fixture.0.join("src/machine.spx"), version2).unwrap();
    assert!(checked(&fixture, BOUNDS).model_checked());
    let refusal = with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&report, &snapshot.retain_revision()).map_err(|e| vec![e])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP407");
    eprintln!("LAW15 request identity: finite ModelChecked; retry101->Idle, retry202->Rejected; version drift SPX-LP407");
}

#[test]
fn identity_confusion_refuses_then_repair_replays_unchanged_law() {
    assert_eq!(
        MUTANT,
        SOURCE.replacen("request_id(event) == 101", "request_id(event) >= 0", 1)
    );
    let fixture = fixture(SOURCE);
    let original = checked(&fixture, BOUNDS);
    std::fs::write(fixture.0.join("src/machine.spx"), MUTANT).unwrap();
    // Concrete source observation: identity 202 incorrectly resumes request 101.
    assert_eq!(evaluate(&fixture, 3, 4), 0);
    let refusal = with_authenticated_project(&fixture.manifest(), |snapshot| {
        check_project_source_protocol(
            &snapshot.retain_revision(),
            "payment.protocol",
            "payment.dispatch",
            "payment.step",
            "Succeeded",
            "charge",
            BOUNDS,
        )
        .map_err(|e| vec![e])
    })
    .unwrap_err();
    assert_eq!(refusal[0].code, "SPX-LP406");
    let stale = with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&original, &snapshot.retain_revision()).map_err(|e| vec![e])
    })
    .unwrap_err();
    assert_eq!(stale[0].code, "SPX-LP406");
    // Repair only the body. The saved protocol, identities and bounds never change.
    std::fs::write(fixture.0.join("src/machine.spx"), SOURCE).unwrap();
    let repaired = checked(&fixture, BOUNDS);
    assert!(repaired.model_checked());
    assert_eq!(repaired.evidence_digest, original.evidence_digest);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        replay(&original, &snapshot.retain_revision()).map_err(|e| vec![e])
    })
    .unwrap();
    assert_eq!(evaluate(&fixture, 3, 4), 10);
    eprintln!("LAW15 request identity: bad state3/event4 returned0, expected10 -> SPX-LP406; unchanged-law repair ModelChecked");
}
