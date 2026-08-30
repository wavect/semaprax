//! Whole-candidate contract review transport. Authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "candidate/contract-delta";
const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-contract-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in FILES {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self, candidate_prepare: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
            .iter()
            .map(|path| std::fs::read(self.0.join(path)).unwrap())
            .collect()
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"contract-review","method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(method, params)).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn open(session: &mut VNextSession) -> String {
    payload(bound(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn report(session: &mut VNextSession, candidate: &str) -> String {
    let mut report = String::new();
    for _ in 0..8193 {
        let chunk = payload(bound(
            session,
            METHOD,
            json!({"candidate_revision":candidate,"offset":report.len(),"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["schema"], "semaprax.image-contract-delta-chunk.v1");
        assert_eq!(
            chunk["report_schema"],
            "semaprax.project-candidate-contract-delta.v1"
        );
        assert_eq!(chunk["image_revision"], session.image_revision());
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["source_authority"], false);
        assert!(chunk.get("target").is_none());
        assert_eq!(chunk["offset"].as_u64().unwrap() as usize, report.len());
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty());
        assert!(text.len() <= 1024);
        report.push_str(text);
        if chunk["next_offset"].is_null() {
            assert_eq!(
                chunk["total_bytes"].as_u64().unwrap() as usize,
                report.len()
            );
            return report;
        }
        assert_eq!(
            chunk["next_offset"].as_u64().unwrap() as usize,
            report.len()
        );
    }
    panic!("bounded contract report did not terminate")
}

#[test]
fn initial_and_changed_contract_chunks_equal_independent_library_reports() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let initial = fixture.candidate();
    let mut session = fixture.session(true);
    let root = open(&mut session);
    assert_eq!(root, initial.candidate_digest());
    assert_eq!(
        report(&mut session, &root),
        initial.contract_delta(initial.candidate_digest()).unwrap()
    );
    let intent = json!({"kind":"add_contract","target":"calculator.add","phase":"ensures","predicate":{"kind":"bool","value":true}});
    let change = SemanticChange::new(initial.revision().project_revision(), &intent).unwrap();
    let candidate = initial.apply(initial.candidate_digest(), &change).unwrap();
    let changed = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    ));
    let revision = changed["candidate_revision"].as_str().unwrap();
    assert_eq!(revision, candidate.candidate_digest());
    let expected = candidate
        .contract_delta(candidate.candidate_digest())
        .unwrap();
    assert_eq!(report(&mut session, revision), expected);
    assert!(expected.ends_with('\n'));
    assert_eq!(
        report(&mut session, &root),
        initial.contract_delta(initial.candidate_digest()).unwrap()
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn host_selection_closed_chunk_schema_and_clients_are_explicit() {
    let fixture = Fixture::new();
    for candidates in [false, true] {
        let mut session = fixture.session(candidates);
        let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
        assert_eq!(
            capabilities["methods"]
                .as_array()
                .unwrap()
                .contains(&json!(METHOD)),
            candidates
        );
        assert!(!session.parallel_read_methods().contains(&METHOD));
        if !candidates {
            assert_eq!(
                call(&mut session, METHOD, json!({}))["error"]["code"],
                -32601
            );
            continue;
        }
        let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
        let descriptor = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["method"] == METHOD)
            .unwrap();
        assert_eq!(descriptor["capability"], "candidate_prepare");
        assert_eq!(descriptor["query"], true);
        let params = &descriptor["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"].as_object().unwrap().len(), 4);
        assert!(params["properties"].get("target").is_none());
        assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
        assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
        assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
        let chunk = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["$id"] == "urn:semaprax.image-contract-delta-chunk.v1")
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(chunk["properties"]["source_authority"]["const"], false);
        assert_eq!(
            chunk["properties"]["report_schema"]["const"],
            "semaprax.project-candidate-contract-delta.v1"
        );
        assert!(schemas["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!("urn:semaprax.project-candidate-contract-delta.v1")));
        let instructions = payload(call(&mut session, "protocol/instructions", json!({})));
        assert!(instructions["instructions"]
            .as_str()
            .unwrap()
            .contains("candidate/contract-delta"));
        for language in ["typescript", "python", "rust"] {
            let client = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            assert!(client["source"]
                .as_str()
                .unwrap()
                .contains("request_candidate_contract_delta"));
            assert!(client["source"]
                .as_str()
                .unwrap()
                .contains("decode_request_candidate_contract_delta"));
        }
        session.finish().unwrap();
    }
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut session = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let rejected: Value =
            serde_json::from_slice(&session.handle_frame(&frame(METHOD, json!({}))).unwrap())
                .unwrap();
        assert_eq!(rejected["error"]["code"], -32601);
    }
}

#[test]
fn invalid_binding_offset_and_extra_target_preserve_the_candidate_and_sources() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(true);
    let candidate = open(&mut session);
    let expected = report(&mut session, &candidate);
    for extra in [
        json!({"target":"calculator.add"}),
        json!({"chunk_bytes":1023}),
        json!({"offset":-1}),
    ] {
        let mut params = json!({"candidate_revision":candidate});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(bound(&mut session, METHOD, params)["error"]["code"], -32602);
    }
    let outside = bound(
        &mut session,
        METHOD,
        json!({"candidate_revision":candidate,"offset":expected.len()+1}),
    );
    assert!(outside["error"].to_string().contains("SPX-G325"));
    let unknown = bound(
        &mut session,
        METHOD,
        json!({"candidate_revision":format!("sha256:{}","0".repeat(64))}),
    );
    assert_eq!(unknown["error"]["code"], -32000);
    let stale = call(
        &mut session,
        METHOD,
        json!({"candidate_revision":candidate,"image_revision":format!("sha256:{}","0".repeat(64))}),
    );
    assert_eq!(stale["error"]["code"], -32000);
    assert_eq!(report(&mut session, &candidate), expected);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn physical_source_drift_rejects_contract_review_without_implicit_refresh() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let candidate = open(&mut session);
    let path = fixture.0.join("src/core.spx");
    let changed = format!("{}\n", std::fs::read_to_string(&path).unwrap());
    std::fs::write(&path, changed).unwrap();
    let drifted = fixture.bytes();
    let rejected = bound(
        &mut session,
        METHOD,
        json!({"candidate_revision":candidate}),
    );
    assert_eq!(rejected["error"]["code"], -32000);
    assert!(session.finish().is_err());
    assert_eq!(fixture.bytes(), drifted);
}
