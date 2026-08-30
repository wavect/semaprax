//! Declaration dependency transport evidence, authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectSemanticImage};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TARGET: &str = "calculator.add";
const PATHS: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-dependency-transport-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in PATHS {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(&self.manifest(), VNextPolicy::default()).unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        PATHS
            .iter()
            .map(|path| std::fs::read(self.0.join(path)).unwrap())
            .collect()
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn frame(id: u64, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}

#[test]
fn readonly_chunks_reassemble_the_exact_independent_library_report() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session();
    let image = fixture.image();
    assert_eq!(session.image_revision(), image.image_digest());
    let expected = image
        .declaration_dependencies(image.image_digest(), TARGET)
        .unwrap();
    let mut actual = String::new();
    let mut offset = 0usize;
    loop {
        let response = bound(
            &mut session,
            "image/dependencies",
            json!({"target":TARGET,"offset":offset,"chunk_bytes":1024}),
        );
        assert_eq!(response["result"]["image_revision"], image.image_digest());
        let chunk = payload(response);
        assert_eq!(
            chunk["schema"],
            "semaprax.image-declaration-dependencies-chunk.v1"
        );
        assert_eq!(
            chunk["report_schema"],
            "semaprax.image-declaration-dependencies.v1"
        );
        assert_eq!(chunk["target"], TARGET);
        assert_eq!(chunk["image_revision"], image.image_digest());
        assert_eq!(chunk["source_authority"], false);
        assert_eq!(chunk["offset"], offset);
        assert_eq!(chunk["total_bytes"], expected.len());
        let text = chunk["chunk"].as_str().unwrap();
        assert!(text.len() <= 1024);
        actual.push_str(text);
        match chunk["next_offset"].as_u64() {
            Some(next) => {
                assert_eq!(next as usize, actual.len());
                assert!(next as usize > offset);
                offset = next as usize;
            }
            None => break,
        }
    }
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    let eof = payload(bound(
        &mut session,
        "image/dependencies",
        json!({"target":TARGET,"offset":actual.len()}),
    ));
    assert_eq!(eof["chunk"], "");
    assert!(eof["next_offset"].is_null());
    assert_eq!(
        bound(&mut session, "candidate/open", json!({}))["error"]["code"],
        -32601
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn stale_unknown_and_invalid_offsets_leave_sources_and_readability_unchanged() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session();
    let original = bound(&mut session, "image/dependencies", json!({"target":TARGET}));
    payload(original.clone());
    let stale = call(
        &mut session,
        "image/dependencies",
        json!({"image_revision":format!("sha256:{}","0".repeat(64)),"target":TARGET}),
    );
    assert_eq!(stale["error"]["code"], -32000);
    assert!(stale["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    for params in [
        json!({"target":"unknown.declaration"}),
        json!({"target":TARGET,"offset":8*1024*1024}),
    ] {
        assert_eq!(
            bound(&mut session, "image/dependencies", params)["error"]["code"],
            -32000
        );
    }
    for params in [
        json!({"target":TARGET,"offset":8*1024*1024+1}),
        json!({"target":TARGET,"chunk_bytes":1023}),
        json!({"target":TARGET,"chunk_bytes":65537}),
        json!({"target":TARGET,"unexpected":true}),
    ] {
        assert_eq!(
            bound(&mut session, "image/dependencies", params)["error"]["code"],
            -32602
        );
    }
    assert_eq!(
        bound(&mut session, "image/dependencies", json!({"target":TARGET})),
        original
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn older_profiles_do_not_acquire_the_v5_dependency_method() {
    let fixture = Fixture::new();
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut session = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let request = frame(
            1,
            "image/dependencies",
            json!({"image_revision":session.image_revision(),"target":TARGET}),
        );
        let response: Value =
            serde_json::from_slice(&session.handle_frame(&request).unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}

#[test]
fn batched_dependency_reads_match_sequential_bytes_order_and_whitelist() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut sequential = fixture.session();
    let mut parallel = fixture.session();
    let revision = parallel.image_revision();
    let requests = [
        frame(
            8,
            "image/dependencies",
            json!({"image_revision":revision,"target":TARGET,"chunk_bytes":1024}),
        ),
        frame(
            2,
            "image/dependencies",
            json!({"image_revision":revision,"target":"calculator.subtract"}),
        ),
        frame(
            9,
            "image/dependencies",
            json!({"image_revision":revision,"target":TARGET}),
        ),
        frame(
            1,
            "image/dependencies",
            json!({"image_revision":revision,"target":"unknown.declaration"}),
        ),
        frame(5, "workspace/open", json!({})),
    ];
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        let actual = parallel.handle_read_batch(&refs, workers).unwrap();
        assert_eq!(actual, expected);
        for (response, id) in actual.iter().zip([8, 2, 9, 1, 5]) {
            let decoded: Value = serde_json::from_slice(response.as_ref().unwrap()).unwrap();
            assert_eq!(decoded["id"], id);
        }
    }
    assert!(parallel
        .parallel_read_methods()
        .contains(&"image/dependencies"));
    for method in [
        "candidate/open",
        "candidate/build",
        "candidate/commit",
        "workspace/refresh",
        "hole/recovery-restore",
    ] {
        assert!(!parallel.parallel_read_methods().contains(&method));
        let request = frame(1, method, json!({}));
        let responses = parallel
            .handle_read_batch(&[request.as_slice()], 1)
            .unwrap();
        let response: Value = serde_json::from_slice(responses[0].as_ref().unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
    parallel.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn live_source_drift_rejects_dependency_reads_without_rewriting_inputs() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    payload(bound(
        &mut session,
        "image/dependencies",
        json!({"target":TARGET}),
    ));
    let path = fixture.0.join("src/app.spx");
    let mut changed = std::fs::read(&path).unwrap();
    changed.extend_from_slice(b"\n// external edit after image retention\n");
    std::fs::write(&path, changed).unwrap();
    let before = fixture.bytes();
    assert!(
        bound(&mut session, "image/dependencies", json!({"target":TARGET}))
            .get("error")
            .is_some()
    );
    let request = frame(
        1,
        "image/dependencies",
        json!({"image_revision":session.image_revision(),"target":TARGET}),
    );
    assert!(session.handle_read_batch(&[request.as_slice()], 1).is_err());
    assert_eq!(fixture.bytes(), before);
}
