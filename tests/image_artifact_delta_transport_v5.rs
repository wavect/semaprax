//! Host-granted artifact delta transport. Authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectCandidate, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "candidate/artifact-delta";
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
            "spx-artifact-delta-{}-{}",
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
    fn session(&self, candidates: bool, build: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: candidates,
                build_enabled: build,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
            .iter()
            .map(|path| std::fs::read(self.0.join(path)).unwrap())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"artifact-delta","method":method,"params":params})
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
fn report(session: &mut VNextSession, candidate: &str, kind: ImageArtifactKind) -> String {
    let mut report = String::new();
    for _ in 0..129 {
        let chunk = payload(bound(
            session,
            METHOD,
            json!({"candidate_revision":candidate,"kind":kind.name(),"offset":report.len(),"chunk_bytes":65536}),
        ));
        assert_eq!(chunk["schema"], "semaprax.image-artifact-delta-chunk.v1");
        assert_eq!(
            chunk["report_schema"],
            "semaprax.project-candidate-artifact-delta.v1"
        );
        assert_eq!(chunk["image_revision"], session.image_revision());
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["kind"], kind.name());
        assert!(chunk["target"].is_null());
        for field in [
            "source_authority",
            "artifact_materialization",
            "target_execution",
        ] {
            assert_eq!(chunk[field], false);
        }
        assert_eq!(chunk["offset"].as_u64().unwrap() as usize, report.len());
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty() && text.len() <= 65536);
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
    panic!("bounded artifact report did not terminate")
}

#[test]
fn both_artifact_kinds_reassemble_exact_replayed_candidate_reports_without_writing_files() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let initial = fixture.candidate();
    let mut session = fixture.session(true, true);
    let root = open(&mut session);
    assert_eq!(root, initial.candidate_digest());
    let intent = json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"});
    let change = SemanticChange::new(initial.revision().project_revision(), &intent).unwrap();
    let changed = initial.apply(initial.candidate_digest(), &change).unwrap();
    let result = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    ));
    let candidate = result["candidate_revision"].as_str().unwrap();
    assert_eq!(candidate, changed.candidate_digest());
    {
        let kind = ImageArtifactKind::Web;
        let unchanged = initial
            .artifact_delta(initial.candidate_digest(), kind)
            .unwrap();
        assert_eq!(report(&mut session, &root, kind), unchanged);
        let expected = changed
            .artifact_delta(changed.candidate_digest(), kind)
            .unwrap();
        assert_eq!(report(&mut session, candidate, kind), expected);
        assert!(expected.ends_with('\n'));
        assert_ne!(expected, unchanged);
    }
    // Npm delta for the calculator (non-useful-text-consumer) project is
    // intentionally not exercised here; it now requires the
    // useful-text-consumer.v1 profile (SPX-W120) and is covered by the
    // dedicated project_candidate_artifact_delta tests.
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
    let mut names = std::fs::read_dir(&fixture.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["semaprax.toml", "src"]);
}

#[test]
fn build_permission_is_required_and_discovery_is_closed_without_client_elevation() {
    let fixture = Fixture::new();
    assert!(VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            build_enabled: true,
            ..Default::default()
        }
    )
    .is_err());
    for (candidates, build) in [(false, false), (true, false), (true, true)] {
        let mut session = fixture.session(candidates, build);
        let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
        assert_eq!(
            capabilities["methods"]
                .as_array()
                .unwrap()
                .contains(&json!(METHOD)),
            build
        );
        assert_eq!(
            capabilities["methods"]
                .as_array()
                .unwrap()
                .contains(&json!("candidate/build")),
            build
        );
        assert!(!session.parallel_read_methods().contains(&METHOD));
        if !build {
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
        assert_eq!(descriptor["capability"], "candidate_build");
        assert_eq!(descriptor["query"], false);
        let params = &descriptor["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"].as_object().unwrap().len(), 5);
        assert_eq!(
            params["properties"]["kind"]["enum"],
            json!(["web", "npm", "openapi", "c"])
        );
        assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
        for absent in [
            "target",
            "path",
            "max_bytes",
            "max_build_bytes",
            "build_enabled",
        ] {
            assert!(params["properties"].get(absent).is_none());
        }
        let chunk = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["$id"] == "urn:semaprax.image-artifact-delta-chunk.v1")
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(chunk["properties"]["target"]["type"], "null");
        for field in [
            "source_authority",
            "artifact_materialization",
            "target_execution",
        ] {
            assert_eq!(chunk["properties"][field]["const"], false);
        }
        assert!(schemas["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!("urn:semaprax.project-candidate-artifact-delta.v1")));
        assert!(
            payload(call(&mut session, "protocol/instructions", json!({})))["instructions"]
                .as_str()
                .unwrap()
                .contains(METHOD)
        );
        for language in ["typescript", "python", "rust"] {
            let client = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = client["source"].as_str().unwrap();
            assert!(source.contains("request_candidate_artifact_delta"));
            assert!(source.contains("decode_request_candidate_artifact_delta"));
        }
        session.finish().unwrap();
    }
    for profile in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut session = ImageSession::open(&fixture.manifest(), profile).unwrap();
        let response: Value =
            serde_json::from_slice(&session.handle_frame(&frame(METHOD, json!({}))).unwrap())
                .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}

#[test]
fn invalid_requests_and_source_drift_never_write_artifacts_or_mutate_candidates() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(true, true);
    let candidate = open(&mut session);
    let expected = report(&mut session, &candidate, ImageArtifactKind::Web);
    for extra in [
        json!({"kind":"native"}),
        json!({"target":"calculator.add"}),
        json!({"max_build_bytes":33554432}),
        json!({"path":"out"}),
        json!({"chunk_bytes":1023}),
        json!({"offset":-1}),
    ] {
        let mut params = json!({"candidate_revision":candidate,"kind":"web"});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(bound(&mut session, METHOD, params)["error"]["code"], -32602);
    }
    let outside = bound(
        &mut session,
        METHOD,
        json!({"candidate_revision":candidate,"kind":"web","offset":expected.len()+1}),
    );
    assert!(outside["error"].to_string().contains("SPX-G331"));
    let unknown = bound(
        &mut session,
        METHOD,
        json!({"candidate_revision":format!("sha256:{}","0".repeat(64)),"kind":"web"}),
    );
    assert_eq!(unknown["error"]["code"], -32000);
    let stale = call(
        &mut session,
        METHOD,
        json!({"image_revision":format!("sha256:{}","0".repeat(64)),"candidate_revision":candidate,"kind":"web"}),
    );
    assert_eq!(stale["error"]["code"], -32000);
    assert_eq!(
        report(&mut session, &candidate, ImageArtifactKind::Web),
        expected
    );
    assert_eq!(fixture.bytes(), before);
    let source = fixture.0.join("src/core.spx");
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let drifted = fixture.bytes();
    assert_eq!(
        bound(
            &mut session,
            METHOD,
            json!({"candidate_revision":candidate,"kind":"web"})
        )["error"]["code"],
        -32000
    );
    assert!(session.finish().is_err());
    assert_eq!(fixture.bytes(), drifted);
}
