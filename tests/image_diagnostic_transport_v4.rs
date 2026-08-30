//! Diagnostics protocol regression cases; authored, deliberately unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, DIAGNOSTIC_PROTOCOL_SCHEMA};
use semaprax::project::CandidateTestPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-diagnostics-protocol-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in FILES {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root)
    }
    fn session(&self, capability: ImageHostCapability) -> ImageSession {
        ImageSession::open(&self.0.join("semaprax.toml"), capability).unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
            .iter()
            .map(|file| std::fs::read(self.0.join(file)).unwrap())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut ImageSession, method: &str, mut params: Value) -> Value {
    if method.starts_with("candidate/")
        || method.starts_with("attempt/")
        || method == "validation/catalog"
    {
        params["image_revision"] = json!(session.image_revision());
    }
    let request = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["protocol"], DIAGNOSTIC_PROTOCOL_SCHEMA);
    response["result"]["payload"].clone()
}
fn open(session: &mut ImageSession) -> String {
    payload(call(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn rejected(session: &mut ImageSession, candidate: &str, value: i64) -> String {
    let result = payload(call(
        session,
        "candidate/attempt",
        json!({"candidate_revision":candidate,"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":value}}}),
    ));
    assert_eq!(result["status"], "rejected");
    assert_eq!(result["candidate"], Value::Null);
    result["attempt"]["attempt_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn chunks(session: &mut ImageSession, method: &str, mut params: Value) -> String {
    let mut report = String::new();
    let mut offset = 0;
    loop {
        params["offset"] = json!(offset);
        params["chunk_bytes"] = json!(1024);
        let part = payload(call(session, method, params.clone()));
        assert_eq!(part["offset"], offset);
        report.push_str(part["chunk"].as_str().unwrap());
        match part["next_offset"].as_u64() {
            Some(next) => offset = next as usize,
            None => break,
        }
    }
    report
}

#[test]
fn protocol_conformance_is_read_only_chunked_and_excluded_from_legacy_profiles() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut legacy = fixture.session(ImageHostCapability::CandidateOnly);
    assert_eq!(
        call(&mut legacy, "protocol/conformance", json!({}))["error"]["code"],
        -32601
    );
    let mut session = fixture.session(ImageHostCapability::CandidateDiagnostics);
    let image = session.image_revision().to_owned();
    let report = chunks(
        &mut session,
        "protocol/conformance",
        json!({"image_revision":image}),
    );
    let value: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(
        value["schema"],
        semaprax::project::IMAGE_PROTOCOL_CONFORMANCE_SCHEMA
    );
    assert_eq!(value["image_revision"], image);
    assert_eq!(value["modules"], json!([]));
    let candidate = open(&mut session);
    let candidate_report = chunks(
        &mut session,
        "protocol/conformance",
        json!({"image_revision":image,"candidate_revision":candidate}),
    );
    assert_eq!(candidate_report, report);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn expression_holes_are_discovered_filled_and_kept_out_of_legacy_profiles() {
    let fixture = Fixture::new();
    let mut legacy = fixture.session(ImageHostCapability::CandidateOnly);
    assert_eq!(
        call(&mut legacy, "hole/open-expression", json!({}))["error"]["code"],
        -32601
    );
    let before = fixture.bytes();
    let mut session = fixture.session(ImageHostCapability::CandidateDiagnostics);
    let candidate = open(&mut session);
    let image = session.image_revision().to_owned();
    let catalog = payload(call(
        &mut session,
        "expression/catalog",
        json!({"image_revision":image,"candidate_revision":candidate,"target":"calculator.add"}),
    ));
    let expression = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|expression| {
            expression["phase"] == "body"
                && expression["kind"] == "binary"
                && expression["replaceable"] == true
        })
        .unwrap()["expression_id"]
        .clone();
    let opened = payload(call(
        &mut session,
        "hole/open-expression",
        json!({"image_revision":image,"candidate_revision":candidate,"target":"calculator.add","expression_id":expression,"hole_id":"sum"}),
    ));
    let draft = opened["draft_revision"].as_str().unwrap();
    let context = payload(call(
        &mut session,
        "hole/query",
        json!({"image_revision":image,"draft_revision":draft,"hole_id":"sum"}),
    ));
    assert_eq!(
        context["schema"],
        "semaprax.project-candidate-expression-hole-context.v1"
    );
    assert_eq!(context["expected_type_id"], "i64");
    let rejected = call(
        &mut session,
        "hole/complete",
        json!({"image_revision":image,"draft_revision":draft}),
    );
    assert!(rejected.get("error").is_some());
    let filled = payload(call(
        &mut session,
        "hole/fill",
        json!({"image_revision":image,"draft_revision":draft,"hole_id":"sum","expression":{"kind":"place","name":"left"}}),
    ));
    let completed = payload(call(
        &mut session,
        "hole/complete",
        json!({"image_revision":image,"draft_revision":filled["draft_revision"]}),
    ));
    assert_ne!(completed["candidate_revision"], candidate);
    assert_eq!(fixture.bytes(), before);
}
#[test]
fn diagnostics_are_independent_of_tests_and_absent_from_legacy_profiles() {
    let fixture = Fixture::new();
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
    ] {
        let mut session = fixture.session(capability);
        let caps = call(&mut session, "protocol/capabilities", json!({}));
        assert_ne!(caps["result"]["protocol"], DIAGNOSTIC_PROTOCOL_SCHEMA);
        for name in [
            "candidate/attempt",
            "attempt/query",
            "attempt/repair-apply",
            "candidate/semantic-delta",
        ] {
            assert_eq!(call(&mut session, name, json!({}))["error"]["code"], -32601);
            assert!(!caps["result"]["payload"]["methods"]
                .as_array()
                .unwrap()
                .iter()
                .any(|method| method == name));
        }
    }
    let mut session = fixture.session(ImageHostCapability::CandidateDiagnostics);
    let caps = payload(call(&mut session, "protocol/capabilities", json!({})));
    assert_eq!(caps["test_execution"], false);
    assert_eq!(caps["max_attempts"], 16);
    assert!(caps["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "candidate_diagnostics"));
    assert_eq!(
        call(&mut session, "candidate/test", json!({}))["error"]["code"],
        -32601
    );
    let policy = CandidateTestPolicy::new(100, 4096, 16384).unwrap();
    let mut session =
        ImageSession::open_diagnostics(&fixture.0.join("semaprax.toml"), Some(policy)).unwrap();
    let caps = payload(call(&mut session, "protocol/capabilities", json!({})));
    assert_eq!(caps["test_execution"], true);
    assert_eq!(caps["test_policy"]["max_steps"], 100);
    assert_eq!(caps["source_authority"], false);
    let candidate = open(&mut session);
    assert_eq!(
        call(
            &mut session,
            "candidate/test",
            json!({"candidate_revision":candidate,"max_steps":1000})
        )["error"]["code"],
        -32602
    );
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let attempt = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/attempt")
        .unwrap();
    assert_eq!(attempt["capability"], "candidate_diagnostics");
}

#[test]
fn rejected_attempts_survive_base_discard_and_repair_returns_a_valid_candidate() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(ImageHostCapability::CandidateDiagnostics);
    let base = open(&mut session);
    let attempt = rejected(&mut session, &base, 42);
    let report = chunks(
        &mut session,
        "attempt/query",
        json!({"attempt_revision":attempt}),
    );
    let report: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["materializable"], false);
    assert_eq!(report["checked_image"], false);
    assert!(!report["diagnostics"].as_array().unwrap().is_empty());
    assert!(call(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":attempt})
    )
    .get("error")
    .is_some());
    let catalog = payload(call(
        &mut session,
        "attempt/repair-catalog",
        json!({"attempt_revision":attempt}),
    ));
    let repair = catalog["repairs"][0]["repair_id"].as_str().unwrap();
    payload(call(
        &mut session,
        "candidate/discard",
        json!({"candidate_revision":base}),
    ));
    let repaired = payload(call(
        &mut session,
        "attempt/repair-apply",
        json!({"attempt_revision":attempt,"repair_id":repair}),
    ));
    let candidate = repaired["candidate_revision"].as_str().unwrap();
    let summary = payload(call(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":attempt}),
    ));
    assert_eq!(summary["state"], "rejected");
    let delta = chunks(
        &mut session,
        "candidate/semantic-delta",
        json!({"candidate_revision":candidate,"target":"calculator.add"}),
    );
    assert_eq!(
        serde_json::from_str::<Value>(&delta).unwrap()["schema"],
        "semaprax.project-candidate-semantic-delta.v1"
    );
    let catalog = chunks(
        &mut session,
        "candidate/semantic-delta-catalog",
        json!({"candidate_revision":candidate}),
    );
    assert_eq!(
        serde_json::from_str::<Value>(&catalog).unwrap()["schema"],
        "semaprax.project-candidate-semantic-delta-catalog.v1"
    );
    payload(call(
        &mut session,
        "attempt/discard",
        json!({"attempt_revision":attempt}),
    ));
    assert!(call(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":attempt})
    )
    .get("error")
    .is_some());
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn attempt_capacity_failures_preserve_existing_records_and_source_drift_absorbs() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateDiagnostics);
    let base = open(&mut session);
    let mut retained = Vec::new();
    for value in 0..16 {
        retained.push(rejected(&mut session, &base, value));
    }
    let overflow = call(
        &mut session,
        "candidate/attempt",
        json!({"candidate_revision":base,"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":16}}}),
    );
    assert!(overflow.get("error").is_some());
    payload(call(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":retained[0]}),
    ));
    payload(call(
        &mut session,
        "attempt/discard",
        json!({"attempt_revision":retained[0]}),
    ));
    let new = rejected(&mut session, &base, 16);
    let source = fixture.0.join("src/core.spx");
    let original = std::fs::read(&source).unwrap();
    std::fs::write(
        &source,
        String::from_utf8(original.clone())
            .unwrap()
            .replace("left + right", "left - right"),
    )
    .unwrap();
    assert!(call(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":new})
    )
    .get("error")
    .is_some());
    std::fs::write(source, original).unwrap();
    assert!(call(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":new})
    )
    .get("error")
    .is_some());
}
