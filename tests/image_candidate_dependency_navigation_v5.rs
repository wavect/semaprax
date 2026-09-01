//! Candidate-bound compact dependency navigation: authored and intentionally unrun.
use semaprax::image_transport::{
    ImageHostCapability, ImageSession, McpSession, VNextPolicy, VNextSession,
};
use semaprax::project::{
    with_authenticated_project, ImageDependencyPageOptions, ImageDependencyView, ProjectCandidate,
    ProjectCandidateArchive, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TARGET: &str = "calculator.add";
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
            "spx-candidate-dependency-navigation-{}-{}",
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
fn frame(session: &VNextSession, id: usize, method: &str, mut params: Value) -> Vec<u8> {
    params["image_revision"] = json!(session.image_revision());
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request = frame(session, 1, method, params);
    serde_json::from_slice(&session.handle_frame(&request).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn handle(summary: &Value, view: &str) -> String {
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|facet| facet["view"] == view)
        .unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn page_params(candidate: &ProjectCandidate, summary: &Value, view: &str) -> Value {
    json!({"candidate_revision":candidate.candidate_digest(),"target":TARGET,"view":view,
        "handle":handle(summary,view),"page_size":1,"max_bytes":65536})
}
fn inventory(session: &mut VNextSession, project: &str) -> Value {
    payload(call(
        session,
        "workspace/refresh",
        json!({"expected_new_project_revision":project}),
    ))["retained_candidates"]
        .clone()
}

#[test]
fn candidate_summary_and_pages_equal_library_reports_without_retaining_results() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let candidate = changed(&base, "calculator.multiply", 41);
    let mut session = fixture.session(&[&candidate]);
    let before = inventory(&mut session, base.revision().project_revision());
    assert_eq!(before, json!([candidate.candidate_digest()]));
    let expected: Value = serde_json::from_str(
        &candidate
            .dependency_summary(candidate.candidate_digest(), TARGET)
            .unwrap(),
    )
    .unwrap();
    let summary = payload(call(
        &mut session,
        "candidate/dependency-summary",
        json!({"candidate_revision":candidate.candidate_digest(),"target":TARGET}),
    ));
    assert_eq!(summary, expected);
    assert_eq!(summary["candidate_revision"], candidate.candidate_digest());
    assert_eq!(
        summary["base_project_revision"],
        base.revision().project_revision()
    );
    assert_eq!(summary["candidate_retained"], false);
    assert_eq!(summary["source_authority"], false);
    assert_eq!(summary["execution"], false);
    assert_eq!(summary["publication_authority"], false);
    assert_eq!(summary["facets"].as_array().unwrap().len(), 4);
    for view in ImageDependencyView::ALL {
        let token = handle(&summary, view.name());
        let actual = payload(call(
            &mut session,
            "candidate/dependency-page",
            page_params(&candidate, &summary, view.name()),
        ));
        let expected: Value = serde_json::from_str(
            &candidate
                .dependency_page(
                    candidate.candidate_digest(),
                    TARGET,
                    view,
                    &token,
                    None,
                    ImageDependencyPageOptions::new(1, 65536).unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual["candidate_retained"], false);
        assert_eq!(actual["source_authority"], false);
        assert_eq!(actual["execution"], false);
        assert_eq!(actual["publication_authority"], false);
        assert!(actual["items"].as_array().unwrap().len() <= 1);
    }
    assert_eq!(
        inventory(&mut session, base.revision().project_revision()),
        before
    );
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn candidate_grant_selects_closed_schemas_clients_and_mcp_tools_without_widening_old_profiles() {
    let fixture = Fixture::new();
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    for method in ["candidate/dependency-summary", "candidate/dependency-page"] {
        assert_eq!(
            call(&mut readonly, method, json!({}))["error"]["code"],
            -32601
        );
    }
    let readonly_bundle = payload(call(&mut readonly, "protocol/schemas", json!({})));
    assert!(!readonly_bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["$id"] == "urn:semaprax.project-candidate-dependency-summary.v1"));
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        for method in ["candidate/dependency-summary", "candidate/dependency-page"] {
            let request = json!({"jsonrpc":"2.0","id":1,"method":method,
                "params":{"image_revision":old.image_revision()}})
            .to_string();
            let response: Value =
                serde_json::from_slice(&old.handle_frame(request.as_bytes()).unwrap()).unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
    let mut session = fixture.session(&[]);
    let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
    for (method_name, parameter_count, schema) in [
        (
            "candidate/dependency-summary",
            3,
            "urn:semaprax.project-candidate-dependency-summary.v1",
        ),
        (
            "candidate/dependency-page",
            8,
            "urn:semaprax.project-candidate-dependency-page.v1",
        ),
    ] {
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method_name)
            .unwrap();
        assert_eq!(method["capability"], "candidate_prepare");
        assert_eq!(method["query"], true);
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(
            params["properties"].as_object().unwrap().len(),
            parameter_count
        );
        for required in if method_name.ends_with("summary") {
            vec!["image_revision", "candidate_revision", "target"]
        } else {
            vec![
                "image_revision",
                "candidate_revision",
                "target",
                "view",
                "handle",
            ]
        } {
            assert!(params["required"]
                .as_array()
                .unwrap()
                .contains(&json!(required)));
        }
        if method_name.ends_with("page") {
            assert_eq!(params["properties"]["page_size"]["minimum"], 1);
            assert_eq!(params["properties"]["page_size"]["maximum"], 128);
            assert_eq!(params["properties"]["max_bytes"]["minimum"], 1024);
            assert_eq!(params["properties"]["max_bytes"]["maximum"], 1_048_576);
            assert_eq!(
                params["properties"]["view"]["enum"],
                json!(["sites", "callers", "calls", "members"])
            );
            assert_eq!(params["properties"]["cursor"]["maxLength"], 128);
        }
        let document = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["$id"] == schema)
            .unwrap();
        assert_eq!(document["additionalProperties"], false);
        for field in [
            "candidate_retained",
            "source_authority",
            "execution",
            "publication_authority",
        ] {
            assert_eq!(document["properties"][field]["const"], false);
        }
    }
    assert!(bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.image-dependency-item.v1")));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        for fragment in [
            "candidate/dependency-summary",
            "candidate/dependency-page",
            "CandidateDependencySummaryPayload",
            "CandidateDependencyPagePayload",
        ] {
            assert!(source.contains(fragment), "{language} missing {fragment}");
        }
        assert_eq!(client["io"], false);
    }
    let mut mcp = McpSession::new(fixture.session(&[])).unwrap();
    let initialized = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"candidate-dependency-evidence","version":"1"}}});
    let response: Value = serde_json::from_slice(
        &mcp.handle_frame(initialized.to_string().as_bytes())
            .unwrap(),
    )
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
    assert!(names.contains(&"candidate__dependency-summary".to_owned()));
    assert!(names.contains(&"candidate__dependency-page".to_owned()));
    mcp.finish().unwrap();
    session.finish().unwrap();
    readonly.finish().unwrap();
}

#[test]
fn sequential_and_parallel_candidate_navigation_are_byte_identical_without_registry_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let left = changed(&base, "calculator.multiply", 41);
    let right = changed(&base, "calculator.subtract", 43);
    let left_summary: Value = serde_json::from_str(
        &left
            .dependency_summary(left.candidate_digest(), TARGET)
            .unwrap(),
    )
    .unwrap();
    let right_summary: Value = serde_json::from_str(
        &right
            .dependency_summary(right.candidate_digest(), TARGET)
            .unwrap(),
    )
    .unwrap();
    let mut sequential = fixture.session(&[&left, &right]);
    let mut parallel = fixture.session(&[&left, &right]);
    let before = inventory(&mut parallel, base.revision().project_revision());
    let requests = [
        frame(
            &parallel,
            9,
            "candidate/dependency-summary",
            json!({"candidate_revision":left.candidate_digest(),"target":TARGET}),
        ),
        frame(
            &parallel,
            3,
            "candidate/dependency-page",
            page_params(&left, &left_summary, "callers"),
        ),
        frame(
            &parallel,
            7,
            "candidate/dependency-summary",
            json!({"candidate_revision":right.candidate_digest(),"target":TARGET}),
        ),
        frame(
            &parallel,
            4,
            "candidate/dependency-page",
            page_params(&right, &right_summary, "sites"),
        ),
    ];
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            parallel.handle_read_batch(&refs, workers).unwrap(),
            expected
        );
    }
    for (response, id) in expected.iter().zip([9, 3, 7, 4]) {
        let response: Value = serde_json::from_slice(response.as_ref().unwrap()).unwrap();
        assert_eq!(response["id"], id);
        assert!(response.get("error").is_none(), "{response}");
    }
    for method in ["candidate/dependency-summary", "candidate/dependency-page"] {
        assert!(parallel.parallel_read_methods().contains(&method));
    }
    assert_eq!(
        inventory(&mut parallel, base.revision().project_revision()),
        before
    );
    assert_eq!(fixture.bytes(), disk);
    sequential.finish().unwrap();
    parallel.finish().unwrap();
}

#[test]
fn stale_candidates_foreign_references_and_changed_options_fail_without_poisoning_valid_reads() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.base();
    let left = changed(&base, "calculator.multiply", 41);
    let right = changed(&base, "calculator.subtract", 43);
    let mut session = fixture.session(&[&left, &right]);
    let left_summary = payload(call(
        &mut session,
        "candidate/dependency-summary",
        json!({"candidate_revision":left.candidate_digest(),"target":TARGET}),
    ));
    let right_summary = payload(call(
        &mut session,
        "candidate/dependency-summary",
        json!({"candidate_revision":right.candidate_digest(),"target":TARGET}),
    ));
    let valid = page_params(&left, &left_summary, "callers");
    let first = payload(call(
        &mut session,
        "candidate/dependency-page",
        valid.clone(),
    ));
    let cursor = first["next_cursor"]
        .as_str()
        .expect("calculator add has multiple candidate reverse callers");
    let right_first = payload(call(
        &mut session,
        "candidate/dependency-page",
        page_params(&right, &right_summary, "callers"),
    ));
    let foreign_cursor = right_first["next_cursor"]
        .as_str()
        .expect("sibling candidate has multiple reverse callers");
    for (field, value) in [
        ("target", json!("calculator.subtract")),
        ("view", json!("sites")),
        ("handle", json!(handle(&right_summary, "callers"))),
    ] {
        let mut wrong = valid.clone();
        wrong[field] = value;
        assert_eq!(
            call(&mut session, "candidate/dependency-page", wrong)["error"]["code"],
            -32000
        );
    }
    for (field, value) in [("page_size", json!(2)), ("max_bytes", json!(65537))] {
        let mut wrong = valid.clone();
        wrong["cursor"] = json!(cursor);
        wrong[field] = value;
        assert_eq!(
            call(&mut session, "candidate/dependency-page", wrong)["error"]["code"],
            -32000
        );
    }
    let mut foreign = valid.clone();
    foreign["cursor"] = json!(foreign_cursor);
    assert_eq!(
        call(&mut session, "candidate/dependency-page", foreign)["error"]["code"],
        -32000
    );
    let unknown = format!("sha256:{}", "0".repeat(64));
    assert!(call(
        &mut session,
        "candidate/dependency-summary",
        json!({"candidate_revision":unknown,"target":TARGET})
    )["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G224"));
    let mut stale = valid.clone();
    stale["image_revision"] = json!(unknown);
    let request =
        json!({"jsonrpc":"2.0","id":1,"method":"candidate/dependency-page","params":stale});
    assert!(serde_json::from_slice::<Value>(
        &session
            .handle_frame(request.to_string().as_bytes())
            .unwrap()
    )
    .unwrap()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    for (field, value) in [
        ("page_size", json!(0)),
        ("max_bytes", json!(1023)),
        ("cursor", Value::Null),
    ] {
        let mut malformed = valid.clone();
        malformed[field] = value;
        assert_eq!(
            call(&mut session, "candidate/dependency-page", malformed)["error"]["code"],
            -32602
        );
    }
    assert_eq!(
        payload(call(&mut session, "candidate/dependency-page", valid)),
        first
    );
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}
