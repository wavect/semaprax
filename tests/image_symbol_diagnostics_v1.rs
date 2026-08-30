//! Exact-symbol rejected-attempt discovery. Authored and intentionally unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateAttempt,
    ProjectCandidateAttemptOutcome, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[test]
fn single_attempt_report_replays_predecessor_history_and_rejects_tampering() {
    let fixture = Fixture::new();
    let revision = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let original =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let rename = SemanticChange::new(
        original.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
    )
    .unwrap();
    let predecessor = Arc::new(
        original
            .apply(original.candidate_digest(), &rename)
            .unwrap(),
    );
    let rejected = match ProjectCandidateAttempt::apply(
        Arc::clone(&predecessor),
        predecessor.candidate_digest(),
        &json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":7}}),
    )
    .unwrap()
    {
        ProjectCandidateAttemptOutcome::Rejected(attempt) => attempt,
        ProjectCandidateAttemptOutcome::Accepted(_) => panic!("expected typed rejection"),
    };
    let report = rejected
        .symbol_diagnostics(
            rejected.attempt_digest(),
            predecessor.candidate_digest(),
            "calculator.add",
        )
        .unwrap();
    let verify = |bytes: &[u8]| {
        rejected.verify_symbol_diagnostics(
            rejected.attempt_digest(),
            predecessor.candidate_digest(),
            "calculator.add",
            bytes,
        )
    };
    let receipt: Value = serde_json::from_str(&verify(report.as_bytes()).unwrap()).unwrap();
    assert_eq!(
        receipt["schema"],
        "semaprax.project-candidate-symbol-diagnostic-verification.v1"
    );
    assert_eq!(receipt["source_authority"], false);
    assert_eq!(receipt["execution"], false);
    assert_eq!(
        receipt["base_candidate_revision"],
        predecessor.candidate_digest()
    );
    let mut changed: Value = serde_json::from_str(&report).unwrap();
    changed["diagnostics"][0]["code"] = json!("SPX-FAKE");
    for bytes in [format!("{}\n", changed), format!("{report}\n")] {
        let diagnostics = verify(bytes.as_bytes()).unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-G243"));
    }
    let diagnostics = rejected
        .verify_symbol_diagnostics(
            rejected.attempt_digest(),
            original.candidate_digest(),
            "calculator.add",
            report.as_bytes(),
        )
        .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G224"));
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-symbol-diagnostics-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self, diagnostics: bool) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                diagnostics,
                ..Default::default()
            },
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if method != "protocol/capabilities" && method != "workspace/open" {
        params["image_revision"] = json!(session.image_revision());
    }
    let bytes = json!({"jsonrpc":"2.0","id":"symbol-diagnostics","method":method,"params":params})
        .to_string();
    serde_json::from_slice(&session.handle_frame(bytes.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn error(response: Value, code: &str) {
    assert!(response.get("error").is_some(), "{response}");
    assert!(response["error"].to_string().contains(code), "{response}");
}
fn root(session: &mut VNextSession) -> String {
    payload(call(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn attempt(
    session: &mut VNextSession,
    candidate: &str,
    target: &str,
    kind: &str,
    value: Value,
) -> String {
    let result = payload(call(
        session,
        "candidate/attempt",
        json!({"candidate_revision":candidate,"intent":{"kind":"replace_function_body","target":target,"body":{"kind":kind,"value":value}}}),
    ));
    assert_eq!(result["status"], "rejected");
    assert!(result["candidate"].is_null());
    result["attempt"]["attempt_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn report(session: &mut VNextSession, candidate: &str, target: &str) -> Value {
    let mut bytes = String::new();
    let mut revision = None::<String>;
    for _ in 0..1024 {
        let mut params = json!({"candidate_revision":candidate,"target":target,"offset":bytes.len(),"chunk_bytes":1024});
        if let Some(revision) = &revision {
            params["expected_report_revision"] = json!(revision);
        }
        let chunk = payload(call(session, "candidate/symbol-diagnostics", params));
        assert_eq!(
            chunk["schema"],
            "semaprax.image-symbol-diagnostics-chunk.v1"
        );
        let actual = chunk["report_revision"].as_str().unwrap();
        if let Some(expected) = &revision {
            assert_eq!(actual, expected);
        } else {
            revision = Some(actual.to_owned());
        }
        assert_eq!(chunk["offset"].as_u64().unwrap() as usize, bytes.len());
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty());
        bytes.push_str(text);
        if chunk["next_offset"].is_null() {
            assert_eq!(chunk["total_bytes"].as_u64().unwrap() as usize, bytes.len());
            return serde_json::from_str(&bytes).unwrap();
        }
        assert_eq!(chunk["next_offset"].as_u64().unwrap() as usize, bytes.len());
    }
    panic!("symbol diagnostic chunk limit")
}

#[test]
fn exact_symbol_reports_rejected_diagnostic_references_and_actual_admitted_repairs() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let candidate = root(&mut session);
    let rejected = attempt(&mut session, &candidate, "calculator.add", "i32", json!(1));
    attempt(
        &mut session,
        &candidate,
        "calculator.subtract",
        "bool",
        json!(true),
    );
    let selected = report(&mut session, &candidate, "calculator.add");
    assert_eq!(
        selected["schema"],
        "semaprax.project-candidate-symbol-diagnostics.v1"
    );
    assert_eq!(selected["candidate_state"], "admitted");
    assert_eq!(selected["candidate_diagnostic_inventory"], "not_retained");
    assert_eq!(selected["matching_attempt_count"], 1);
    assert_eq!(selected["target_provenance"]["id"], "calculator.add");
    assert_eq!(selected["target_provenance"]["path"], "src/core.spx");
    let entry = &selected["attempts"][0];
    assert_eq!(entry["attempt_revision"], rejected);
    assert_eq!(entry["checked_image"], false);
    assert_eq!(entry["materializable"], false);
    assert!(entry["diagnostic_count"].as_u64().unwrap() > 0);
    assert_eq!(entry["diagnostics"][0]["index"], 0);
    assert!(entry["diagnostics"][0].get("span").is_none());
    assert_eq!(
        entry["diagnostic_location_basis"],
        "uncommitted_attempt_or_constructor_input_not_authenticated_base_span"
    );
    assert_eq!(
        entry["repair_catalog"]["repairs"][0]["class"],
        "retag_integer_literal_to_retained_return_type"
    );
    assert_eq!(
        entry["repair_catalog"]["repairs"][0]["validation"],
        "normal_full_candidate_apply"
    );
    let unsupported = report(&mut session, &candidate, "calculator.subtract");
    assert_eq!(unsupported["matching_attempt_count"], 1);
    assert_eq!(
        unsupported["attempts"][0]["repair_catalog"]["repairs"],
        json!([])
    );
    assert_eq!(selected["work"]["repair_catalog_evaluations"], 1);
    session.finish().unwrap();
}

#[test]
fn another_predecessor_or_target_is_not_current_candidate_diagnostic_evidence() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let candidate = root(&mut session);
    let sibling=payload(call(&mut session,"candidate/apply-intent",json!({"candidate_revision":candidate,"intent":{"kind":"rename_declaration","target":"calculator.add","name":"addition"}})))["candidate_revision"].as_str().unwrap().to_owned();
    attempt(&mut session, &sibling, "calculator.add", "i32", json!(1));
    attempt(
        &mut session,
        &candidate,
        "calculator.subtract",
        "i32",
        json!(2),
    );
    let selected = report(&mut session, &candidate, "calculator.add");
    assert_eq!(
        selected["availability"],
        "no_matching_retained_rejected_attempts"
    );
    assert_eq!(selected["attempts"], json!([]));
    assert_eq!(selected["candidate_diagnostic_inventory"], "not_retained");
    assert_eq!(selected["work"]["repair_catalog_evaluations"], 0);
    error(
        call(
            &mut session,
            "candidate/symbol-diagnostics",
            json!({"candidate_revision":candidate,"target":"missing.symbol"}),
        ),
        "SPX-G241",
    );
    let mut no_grant = fixture.session(false);
    let denied = call(
        &mut no_grant,
        "candidate/symbol-diagnostics",
        json!({"candidate_revision":candidate,"target":"calculator.add"}),
    );
    assert_eq!(denied["error"]["code"], -32601);
    session.finish().unwrap();
    no_grant.finish().unwrap();
}

#[test]
fn continuations_bind_report_revision_and_refresh_clears_attempt_associations() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let candidate = root(&mut session);
    attempt(&mut session, &candidate, "calculator.add", "i32", json!(1));
    let first = payload(call(
        &mut session,
        "candidate/symbol-diagnostics",
        json!({"candidate_revision":candidate,"target":"calculator.add","chunk_bytes":1024}),
    ));
    let next = first["next_offset"].as_u64().unwrap();
    error(
        call(
            &mut session,
            "candidate/symbol-diagnostics",
            json!({"candidate_revision":candidate,"target":"calculator.add","offset":next}),
        ),
        "SPX-G243",
    );
    attempt(&mut session, &candidate, "calculator.add", "i32", json!(2));
    error(
        call(
            &mut session,
            "candidate/symbol-diagnostics",
            json!({"candidate_revision":candidate,"target":"calculator.add","offset":next,"expected_report_revision":first["report_revision"]}),
        ),
        "SPX-G243",
    );
    let workspace = payload(call(&mut session, "workspace/open", json!({})));
    let refreshed = payload(call(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":workspace["project_revision"]}),
    ));
    assert_eq!(refreshed["cleared_attempts"], 2);
    assert_eq!(
        report(&mut session, &candidate, "calculator.add")["availability"],
        "no_matching_retained_rejected_attempts"
    );
    session.finish().unwrap();
}

#[test]
fn fifth_matching_attempt_rejects_instead_of_truncating_repair_discovery() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let candidate = root(&mut session);
    let mut last = String::new();
    for value in 1..=5 {
        last = attempt(
            &mut session,
            &candidate,
            "calculator.add",
            "i32",
            json!(value),
        );
    }
    error(
        call(
            &mut session,
            "candidate/symbol-diagnostics",
            json!({"candidate_revision":candidate,"target":"calculator.add"}),
        ),
        "SPX-G242",
    );
    let summary = payload(call(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":last}),
    ));
    assert_eq!(summary["state"], "rejected");
    let unrelated = report(&mut session, &candidate, "calculator.multiply");
    assert_eq!(unrelated["matching_attempt_count"], 0);
    session.finish().unwrap();
}
