//! Build-granted candidate analysis-artifact transport; authored and intentionally unrun.
use semaprax::image_transport::{
    ImageHostCapability, ImageSession, McpSession, VNextPolicy, VNextSession,
};
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectCandidate, SemanticChange,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "candidate/analysis-artifact-evidence";
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
            "spx-analysis-artifact-transport-{}-{}",
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
    json!({"jsonrpc":"2.0","id":"analysis-artifact","method":method,"params":params})
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
fn sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
fn report(session: &mut VNextSession, candidate: &str) -> String {
    let mut report = String::new();
    let mut digest = None;
    for _ in 0..162 {
        let chunk = payload(bound(
            session,
            METHOD,
            json!({"candidate_revision":candidate,"kind":"web","offset":report.len(),"chunk_bytes":65536}),
        ));
        assert_eq!(chunk.as_object().unwrap().len(), 14);
        assert_eq!(
            chunk["schema"],
            "semaprax.image-analysis-artifact-evidence-chunk.v1"
        );
        assert_eq!(
            chunk["report_schema"],
            "semaprax.project-candidate-analysis-artifact-evidence.v1"
        );
        assert_eq!(chunk["image_revision"], session.image_revision());
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["target"], Value::Null);
        assert_eq!(chunk["kind"], "web");
        for field in [
            "source_authority",
            "artifact_materialization",
            "target_execution",
        ] {
            assert_eq!(chunk[field], false);
        }
        assert_eq!(chunk["offset"].as_u64().unwrap() as usize, report.len());
        let selected_digest = chunk["report_sha256"].as_str().unwrap().to_owned();
        if let Some(expected) = &digest {
            assert_eq!(&selected_digest, expected);
        } else {
            digest = Some(selected_digest);
        }
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty() && text.len() <= 65536);
        report.push_str(text);
        if chunk["next_offset"].is_null() {
            assert_eq!(
                chunk["total_bytes"].as_u64().unwrap() as usize,
                report.len()
            );
            assert_eq!(digest.unwrap(), sha256(report.as_bytes()));
            return report;
        }
        assert_eq!(
            chunk["next_offset"].as_u64().unwrap() as usize,
            report.len()
        );
    }
    panic!("bounded analysis-artifact report did not terminate")
}

#[test]
fn exact_chunks_reassemble_unchanged_and_changed_library_reports_without_writes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let initial = fixture.candidate();
    let mut session = fixture.session(true, true);
    let root = open(&mut session);
    assert_eq!(root, initial.candidate_digest());
    let unchanged = initial
        .analysis_artifact_evidence(initial.candidate_digest(), ImageArtifactKind::Web)
        .unwrap();
    assert_eq!(report(&mut session, &root), unchanged);
    assert!(!unchanged.ends_with('\n'));

    let intent = json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"});
    let change = SemanticChange::new(initial.revision().project_revision(), &intent).unwrap();
    let changed = initial.apply(initial.candidate_digest(), &change).unwrap();
    let selected = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    ))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(selected, changed.candidate_digest());
    let expected = changed
        .analysis_artifact_evidence(changed.candidate_digest(), ImageArtifactKind::Web)
        .unwrap();
    assert_eq!(report(&mut session, &selected), expected);
    assert_ne!(expected, unchanged);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn build_grant_selects_closed_schema_generated_clients_and_mcp_but_never_parallel_reads() {
    let fixture = Fixture::new();
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
            .find(|row| row["method"] == METHOD)
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
        assert_eq!(params["properties"]["offset"]["maximum"], 10 * 1024 * 1024);
        let chunk = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["$id"] == "urn:semaprax.image-analysis-artifact-evidence-chunk.v1")
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(
            chunk["properties"]["report_sha256"]["pattern"],
            "^sha256:[0-9a-f]{64}$"
        );
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
            .contains(&json!(
                "urn:semaprax.project-candidate-analysis-artifact-evidence.v1"
            )));
        for language in ["typescript", "python", "rust"] {
            let client = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = client["source"].as_str().unwrap();
            assert!(source.contains("request_candidate_analysis_artifact_evidence"));
            assert!(source.contains("decode_request_candidate_analysis_artifact_evidence"));
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
        let response: Value =
            serde_json::from_slice(&session.handle_frame(&frame(METHOD, json!({}))).unwrap())
                .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }

    let mut direct = fixture.session(true, true);
    let image = direct.image_revision().to_owned();
    let direct_candidate = open(&mut direct);
    let mcp_host = fixture.session(true, true);
    assert_eq!(mcp_host.image_revision(), image);
    let mut mcp = McpSession::new(mcp_host).unwrap();
    let initialized = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},
        "clientInfo":{"name":"analysis-artifact-evidence","version":"1"}}});
    mcp.handle_frame(initialized.to_string().as_bytes())
        .unwrap();
    assert!(mcp
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    let mut names = Vec::new();
    let mut cursor = None;
    loop {
        let params = cursor
            .as_ref()
            .map_or_else(|| json!({}), |value| json!({"cursor":value}));
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":params});
        let page: Value =
            serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        names.extend(
            page["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_owned()),
        );
        cursor = page["result"]["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(names.contains(&"candidate__analysis-artifact-evidence".to_owned()));
    let opened = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"candidate__open","arguments":{"image_revision":image}}});
    let opened: Value =
        serde_json::from_slice(&mcp.handle_frame(opened.to_string().as_bytes()).unwrap()).unwrap();
    let opened: Value =
        serde_json::from_str(opened["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        opened["result"]["payload"]["candidate_revision"],
        direct_candidate
    );
    let arguments = json!({"image_revision":image,"candidate_revision":direct_candidate,
        "kind":"web","offset":0,"chunk_bytes":1024});
    let direct_request = json!({"jsonrpc":"2.0","id":0,"method":METHOD,"params":arguments});
    let direct_bytes = direct
        .handle_frame(direct_request.to_string().as_bytes())
        .unwrap();
    let invoked = json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{
        "name":"candidate__analysis-artifact-evidence","arguments":arguments}});
    let invoked: Value =
        serde_json::from_slice(&mcp.handle_frame(invoked.to_string().as_bytes()).unwrap()).unwrap();
    assert_eq!(
        invoked["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .as_bytes(),
        direct_bytes
    );
    mcp.finish().unwrap();
    direct.finish().unwrap();
}

#[test]
fn hostile_parameters_stale_state_drift_and_batching_never_publish_partial_evidence() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture
        .session(true, true)
        .with_read_batch_workers(2)
        .unwrap();
    let candidate = open(&mut session);
    let expected = report(&mut session, &candidate);
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
    assert!(outside["error"].to_string().contains("SPX-G354"));
    let unknown = format!("sha256:{}", "0".repeat(64));
    assert_eq!(
        bound(
            &mut session,
            METHOD,
            json!({"candidate_revision":unknown,"kind":"web"})
        )["error"]["code"],
        -32000
    );
    assert_eq!(
        call(
            &mut session,
            METHOD,
            json!({"image_revision":unknown,"candidate_revision":candidate,"kind":"web"})
        )["error"]["code"],
        -32000
    );

    let inner = String::from_utf8(frame(
        METHOD,
        json!({"image_revision":session.image_revision(),"candidate_revision":candidate,"kind":"web"}),
    ))
    .unwrap();
    let batched = payload(bound(
        &mut session,
        "workspace/read-batch",
        json!({"batch":{"frames":[inner]}}),
    ));
    let inner_response: Value =
        serde_json::from_str(batched["responses"][0].as_str().unwrap()).unwrap();
    assert_eq!(inner_response["error"]["code"], -32601);
    assert_eq!(report(&mut session, &candidate), expected);
    assert_eq!(fixture.bytes(), disk);

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
