//! Candidate-bound analysis coverage transport; authored and intentionally unrun.
use semaprax::image_transport::{
    ImageHostCapability, ImageSession, McpSession, VNextPolicy, VNextSession,
};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive, SemanticChange,
    PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "candidate/analysis-coverage";
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
            "spx-candidate-analysis-coverage-{}-{}",
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
    fn base(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn session(&self, candidates: &[&ProjectCandidate]) -> VNextSession {
        let mut session = VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap();
        for candidate in candidates {
            let archive =
                ProjectCandidateArchive::prepare(candidate, candidate.candidate_digest()).unwrap();
            session
                .restore_candidate_archive(
                    archive.to_json().as_bytes(),
                    archive.archive_digest(),
                    archive.candidate_digest(),
                )
                .unwrap();
        }
        session
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

fn changed(base: &ProjectCandidate, target: &str, value: i64) -> ProjectCandidate {
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":target,"body":{"kind":"i64","value":value}}),
    )
    .unwrap();
    base.apply(base.candidate_digest(), &change).unwrap()
}
fn frame(session: &VNextSession, id: usize, candidate: &str) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":METHOD,"params":{
        "image_revision":session.image_revision(),"candidate_revision":candidate
    }})
    .to_string()
    .into_bytes()
}
fn call(session: &mut VNextSession, mut params: Value) -> Value {
    if params.get("image_revision").is_none() {
        params["image_revision"] = json!(session.image_revision());
    }
    let request = json!({"jsonrpc":"2.0","id":1,"method":METHOD,"params":params});
    serde_json::from_slice(
        &session
            .handle_frame(request.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn inventory(session: &mut VNextSession, project: &str) -> Value {
    let request = json!({"jsonrpc":"2.0","id":1,"method":"workspace/refresh","params":{
        "image_revision":session.image_revision(),"expected_new_project_revision":project
    }});
    payload(
        serde_json::from_slice(
            &session
                .handle_frame(request.to_string().as_bytes())
                .unwrap(),
        )
        .unwrap(),
    )["retained_candidates"]
        .clone()
}

#[test]
fn candidate_reports_match_library_and_parallel_reads_without_registry_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let left = changed(&base, "calculator.multiply", 41);
    let right = changed(&base, "calculator.subtract", 43);
    let left_json = left.to_json().to_owned();
    let right_json = right.to_json().to_owned();
    let expected = [&left, &right]
        .into_iter()
        .map(|candidate| {
            serde_json::from_str::<Value>(
                &candidate
                    .analysis_coverage(candidate.candidate_digest())
                    .unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut sequential = fixture.session(&[&left, &right]);
    let mut parallel = fixture.session(&[&left, &right]);
    let before = inventory(&mut parallel, base.revision().project_revision());
    let requests = [
        frame(&parallel, 7, left.candidate_digest()),
        frame(&parallel, 3, right.candidate_digest()),
    ];
    let responses = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    for ((response, expected), id) in responses.iter().zip(&expected).zip([7, 3]) {
        let response: Value = serde_json::from_slice(response.as_ref().unwrap()).unwrap();
        assert_eq!(response["id"], id);
        assert_eq!(
            response["result"]["image_revision"],
            parallel.image_revision()
        );
        assert_eq!(response["result"]["payload"], expected.clone());
        assert_ne!(expected["image_revision"], parallel.image_revision());
        assert_eq!(
            expected["schema"],
            PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA
        );
        assert_eq!(expected["candidate_retained"], false);
        assert_eq!(expected["source_authority"], false);
        assert_eq!(expected["external_io"], false);
        assert_eq!(expected["execution"], false);
        assert_eq!(expected["publication_authority"], false);
    }
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            parallel.handle_read_batch(&refs, workers).unwrap(),
            responses
        );
    }
    assert!(parallel.parallel_read_methods().contains(&METHOD));
    assert_eq!(
        inventory(&mut parallel, base.revision().project_revision()),
        before
    );
    assert_eq!(left.to_json(), left_json);
    assert_eq!(right.to_json(), right_json);
    assert_eq!(fixture.bytes(), disk);
    sequential.finish().unwrap();
    parallel.finish().unwrap();
}

#[test]
fn candidate_grant_selects_closed_schema_clients_and_mcp_without_widening_old_profiles() {
    let fixture = Fixture::new();
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(call(&mut readonly, json!({}))["error"]["code"], -32601);
    let readonly_bundle = payload({
        let request = json!({"jsonrpc":"2.0","id":1,"method":"protocol/schemas","params":{}});
        serde_json::from_slice(
            &readonly
                .handle_frame(request.to_string().as_bytes())
                .unwrap(),
        )
        .unwrap()
    });
    assert!(!readonly_bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["$id"] == "urn:semaprax.project-candidate-analysis-coverage.v1"));
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let request = json!({"jsonrpc":"2.0","id":1,"method":METHOD,"params":{
            "image_revision":old.image_revision(),"candidate_revision":format!("sha256:{}","0".repeat(64))
        }});
        let response: Value =
            serde_json::from_slice(&old.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
    let mut session = fixture.session(&[]);
    let bundle = payload({
        let request = json!({"jsonrpc":"2.0","id":1,"method":"protocol/schemas","params":{}});
        serde_json::from_slice(
            &session
                .handle_frame(request.to_string().as_bytes())
                .unwrap(),
        )
        .unwrap()
    });
    let descriptor = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == METHOD)
        .unwrap();
    assert_eq!(descriptor["capability"], "candidate_prepare");
    assert_eq!(descriptor["query"], true);
    let params = &descriptor["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(
        params["required"],
        json!(["image_revision", "candidate_revision"])
    );
    assert_eq!(params["properties"].as_object().unwrap().len(), 2);
    let document = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.project-candidate-analysis-coverage.v1")
        .unwrap();
    assert_eq!(document["additionalProperties"], false);
    assert_eq!(document["properties"].as_object().unwrap().len(), 20);
    assert_eq!(document["properties"]["blind_spots"]["minItems"], 3);
    assert_eq!(document["properties"]["blind_spots"]["maxItems"], 3);
    for field in [
        "candidate_retained",
        "source_authority",
        "external_io",
        "execution",
        "publication_authority",
    ] {
        assert_eq!(document["properties"][field]["const"], false);
    }
    for language in ["typescript", "python", "rust"] {
        let request = json!({"jsonrpc":"2.0","id":1,"method":"protocol/client","params":{"language":language}});
        let client = payload(
            serde_json::from_slice(
                &session
                    .handle_frame(request.to_string().as_bytes())
                    .unwrap(),
            )
            .unwrap(),
        );
        let source = client["source"].as_str().unwrap();
        for fragment in [
            METHOD,
            "CandidateAnalysisCoveragePayload",
            "CandidateAnalysisCoverageTypedParams",
            "request_candidate_analysis_coverage",
        ] {
            assert!(source.contains(fragment), "{language} missing {fragment}");
        }
        assert_eq!(client["io"], false);
    }
    let mut mcp = McpSession::new(fixture.session(&[])).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"candidate-coverage-evidence","version":"1"}
    }});
    let response: Value =
        serde_json::from_slice(&mcp.handle_frame(initialize.to_string().as_bytes()).unwrap())
            .unwrap();
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
    assert!(mcp
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    let mut names = Vec::new();
    let mut cursor = None;
    loop {
        let params = cursor
            .as_ref()
            .map_or_else(|| json!({}), |cursor| json!({"cursor":cursor}));
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":params});
        let tools: Value =
            serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        names.extend(
            tools["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_owned()),
        );
        cursor = tools["result"]["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(names.contains(&"candidate__analysis-coverage".to_owned()));
    mcp.finish().unwrap();
    session.finish().unwrap();
    readonly.finish().unwrap();
}

#[test]
fn stale_image_unknown_candidate_and_extra_parameters_fail_before_valid_recovery() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let candidate = changed(&base, "calculator.multiply", 41);
    let mut session = fixture.session(&[&candidate]);
    let unknown = format!("sha256:{}", "0".repeat(64));
    let stale = call(
        &mut session,
        json!({"image_revision":unknown,"candidate_revision":candidate.candidate_digest()}),
    );
    assert!(stale["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    let missing = call(&mut session, json!({"candidate_revision":unknown}));
    assert!(missing["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G224"));
    assert_eq!(
        call(
            &mut session,
            json!({"candidate_revision":candidate.candidate_digest(),"external_io":true})
        )["error"]["code"],
        -32602
    );
    let valid = payload(call(
        &mut session,
        json!({"candidate_revision":candidate.candidate_digest()}),
    ));
    assert_eq!(valid["candidate_revision"], candidate.candidate_digest());
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn live_source_drift_blocks_transport_while_candidate_report_remains_immutable() {
    let fixture = Fixture::new();
    let base = fixture.base();
    let candidate = changed(&base, "calculator.multiply", 41);
    let before = candidate
        .analysis_coverage(candidate.candidate_digest())
        .unwrap();
    let mut session = fixture.session(&[&candidate]);
    payload(call(
        &mut session,
        json!({"candidate_revision":candidate.candidate_digest()}),
    ));
    let path = fixture.0.join("src/app.spx");
    let changed = std::fs::read_to_string(&path).unwrap() + "\n// external source drift\n";
    std::fs::write(&path, changed).unwrap();
    let disk = fixture.bytes();
    let error = call(
        &mut session,
        json!({"candidate_revision":candidate.candidate_digest()}),
    );
    assert!(error.get("error").is_some());
    assert_eq!(
        candidate
            .analysis_coverage(candidate.candidate_digest())
            .unwrap(),
        before
    );
    assert_eq!(fixture.bytes(), disk);
}
