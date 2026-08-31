//! Read-only candidate merge preview transport: authored tests, unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateArchive, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-merge-preview-v5-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
    fn session(&self, parents: &[&ProjectCandidate]) -> VNextSession {
        let mut session = VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap();
        for parent in parents {
            let archive =
                ProjectCandidateArchive::prepare(parent, parent.candidate_digest()).unwrap();
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn changed(base: &ProjectCandidate, target: &str, value: i64) -> ProjectCandidate {
    let intent =
        json!({"kind":"replace_function_body","target":target,"body":{"kind":"i64","value":value}});
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
    )
    .unwrap()
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
fn params(left: &ProjectCandidate, right: &ProjectCandidate) -> Value {
    json!({"candidate_revision":left.candidate_digest(),"other_candidate_revision":right.candidate_digest()})
}

fn inventory(session: &mut VNextSession, project: &str) -> Value {
    // Existing explicit refresh exposes the retained inventory. With unchanged
    // source it supplies before/after registry observations for the read test.
    payload(call(
        session,
        "workspace/refresh",
        json!({"expected_new_project_revision":project}),
    ))["retained_candidates"]
        .clone()
}

#[test]
fn sequential_and_bounded_parallel_previews_match_library_bytes_without_registering_either_result()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = changed(&base, "calculator.add", 17);
    let right = changed(&base, "calculator.subtract", 23);
    let left_bytes = left.to_json().to_owned();
    let right_bytes = right.to_json().to_owned();
    let expected: Value = serde_json::from_str(
        &left
            .merge_preview(left.candidate_digest(), &right, right.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    let reverse: Value = serde_json::from_str(
        &right
            .merge_preview(right.candidate_digest(), &left, left.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    let mut sequential = fixture.session(&[&left, &right]);
    let mut parallel = fixture.session(&[&left, &right]);
    let before = inventory(&mut parallel, base.base_revision().project_revision());
    assert_eq!(before.as_array().unwrap().len(), 2);
    let image = parallel.image_revision().to_owned();
    assert_eq!(image, sequential.image_revision());
    let requests = vec![
        frame(
            &parallel,
            9,
            "candidate/merge-preview",
            params(&left, &right),
        ),
        frame(
            &parallel,
            4,
            "candidate/merge-preview",
            params(&right, &left),
        ),
        frame(
            &parallel,
            7,
            "candidate/query",
            json!({"candidate_revision":left.candidate_digest(),"chunk_bytes":1024}),
        ),
        frame(
            &parallel,
            6,
            "candidate/query",
            json!({"candidate_revision":right.candidate_digest(),"chunk_bytes":1024}),
        ),
    ];
    let ordinary = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let decoded = ordinary
        .iter()
        .map(|response| serde_json::from_slice::<Value>(response.as_ref().unwrap()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(payload(decoded[0].clone()), expected);
    assert_eq!(payload(decoded[1].clone()), reverse);
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            parallel.handle_read_batch(&refs, workers).unwrap(),
            ordinary
        );
    }
    assert_eq!(parallel.image_revision(), image);
    for direction in ["left_then_right", "right_then_left"] {
        assert_eq!(expected[direction]["status"], "accepted");
        let selected = expected[direction]["result_candidate_revision"]
            .as_str()
            .unwrap();
        assert_ne!(selected, left.candidate_digest());
        assert_ne!(selected, right.candidate_digest());
        let unknown = call(
            &mut parallel,
            "candidate/query",
            json!({"candidate_revision":selected}),
        );
        assert!(unknown["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SPX-G224"));
    }
    let after = inventory(&mut parallel, base.base_revision().project_revision());
    assert_eq!(after, before);
    assert_eq!(left.to_json(), left_bytes);
    assert_eq!(right.to_json(), right_bytes);
    assert_eq!(fixture.bytes(), disk);
    sequential.finish().unwrap();
    parallel.finish().unwrap();
}

#[test]
fn candidate_grant_selects_closed_readonly_schema_and_clients_but_old_protocols_do_not_gain_the_route(
) {
    let fixture = Fixture::new();
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(
        call(&mut readonly, "candidate/merge-preview", json!({}))["error"]["code"],
        -32601
    );
    let readonly_bundle = payload(call(&mut readonly, "protocol/schemas", json!({})));
    assert!(!readonly_bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["$id"] == "urn:semaprax.project-candidate-merge-preview.v1"));
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let request = json!({"jsonrpc":"2.0","id":1,"method":"candidate/merge-preview","params":{"image_revision":old.image_revision()}}).to_string();
        let response: Value =
            serde_json::from_slice(&old.handle_frame(request.as_bytes()).unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
    let mut session = fixture.session(&[]);
    let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == "candidate/merge-preview")
        .unwrap();
    assert_eq!(method["query"], true);
    assert_eq!(method["capability"], "candidate_prepare");
    let request = &method["request_schema"]["properties"]["params"];
    assert_eq!(request["additionalProperties"], false);
    assert_eq!(request["properties"].as_object().unwrap().len(), 3);
    for key in [
        "image_revision",
        "candidate_revision",
        "other_candidate_revision",
    ] {
        assert!(request["required"]
            .as_array()
            .unwrap()
            .contains(&json!(key)));
    }
    assert!(session
        .parallel_read_methods()
        .contains(&"candidate/merge-preview"));
    let document = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.project-candidate-merge-preview.v1")
        .unwrap();
    assert_eq!(document["additionalProperties"], false);
    assert_eq!(document["properties"]["source_authority"]["const"], false);
    assert_eq!(document["properties"]["candidate_retained"]["const"], false);
    assert!(document["required"]
        .as_array()
        .unwrap()
        .contains(&json!("same_source")));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        assert!(source.contains("candidate/merge-preview"));
        assert!(source.contains("CandidateMergePreviewPayload"));
        assert!(source.contains("decode_request_candidate_merge_preview_typed"));
        assert_eq!(client["io"], false);
    }
}

#[test]
fn malformed_and_unknown_parent_requests_match_sequential_errors_and_do_not_relax_worker_bounds() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = changed(&base, "calculator.add", 17);
    let right = changed(&base, "calculator.subtract", 23);
    let mut sequential = fixture.session(&[&left, &right]);
    let mut parallel = fixture.session(&[&left, &right]);
    let unknown = format!("sha256:{}", "0".repeat(64));
    let requests = vec![
        frame(
            &parallel,
            1,
            "candidate/merge-preview",
            json!({"candidate_revision":unknown,"other_candidate_revision":right.candidate_digest()}),
        ),
        frame(
            &parallel,
            2,
            "candidate/merge-preview",
            json!({"candidate_revision":left.candidate_digest()}),
        ),
        frame(
            &parallel,
            3,
            "candidate/merge-preview",
            json!({"candidate_revision":left.candidate_digest(),"other_candidate_revision":right.candidate_digest(),"commit":true}),
        ),
    ];
    let ordinary = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert_eq!(parallel.handle_read_batch(&refs, 2).unwrap(), ordinary);
    let errors = ordinary
        .iter()
        .map(|row| serde_json::from_slice::<Value>(row.as_ref().unwrap()).unwrap())
        .collect::<Vec<_>>();
    assert!(errors[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G224"));
    assert_eq!(errors[1]["error"]["code"], -32602);
    assert_eq!(errors[2]["error"]["code"], -32602);
    for workers in [0, 5] {
        let errors = parallel.handle_read_batch(&refs, workers).err().unwrap();
        assert!(errors.iter().any(|error| error.code == "SPX-G294"));
    }
    let too_many = vec![requests[0].as_slice(); 17];
    assert!(parallel
        .handle_read_batch(&too_many, 1)
        .err()
        .unwrap()
        .iter()
        .any(|error| error.code == "SPX-G294"));
    let mut stale: Value = serde_json::from_slice(&frame(
        &parallel,
        4,
        "candidate/merge-preview",
        params(&left, &right),
    ))
    .unwrap();
    stale["params"]["image_revision"] = json!(unknown);
    let response: Value =
        serde_json::from_slice(&parallel.handle_frame(stale.to_string().as_bytes()).unwrap())
            .unwrap();
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn source_drift_prevents_preview_output_in_sequential_and_parallel_paths() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let left = changed(&base, "calculator.add", 17);
    let right = changed(&base, "calculator.subtract", 23);
    let mut sequential = fixture.session(&[&left, &right]);
    let mut parallel = fixture.session(&[&left, &right]);
    let request = frame(
        &parallel,
        1,
        "candidate/merge-preview",
        params(&left, &right),
    );
    let path = fixture.0.join("src/app.spx");
    let old = std::fs::read_to_string(&path).unwrap();
    let changed = old.replace("multiply(6, 7)", "multiply(6, 8)");
    assert_ne!(changed, old);
    std::fs::write(path, changed).unwrap();
    let disk = fixture.bytes();
    let response: Value =
        serde_json::from_slice(&sequential.handle_frame(&request).unwrap()).unwrap();
    assert!(response.get("error").is_some());
    assert!(response.get("result").is_none());
    assert!(parallel
        .handle_read_batch(&[request.as_slice()], 2)
        .is_err());
    assert_eq!(fixture.bytes(), disk);
}
