//! Read-only cleanup dependency transport regressions, authored and unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectSemanticImage, SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const METHOD: &str = "image/cleanup-dependencies";
const TARGET: &str = "cleanup.packet.payload";
const PATHS: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-cleanup-dependency-rpc-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.manifest(),
            r#"schema = "semaprax.project.v8"
name = "cleanup-dependencies"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "cleanup.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["cleanup.public"]
tests = ["cleanup.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module cleanup.core;
@id("cleanup.packet") record Packet { @id("cleanup.packet.payload") payload:Bytes, @id("cleanup.packet.marker") marker:i64, }
@id("cleanup.forward") fn forward(packet:own Packet)->Packet {packet}
@id("cleanup.discard") fn discard(packet:own Packet)->i64 {0}
@id("cleanup.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/app.spx",
                r#"module cleanup.app;
use function @id("cleanup.public") from cleanup.core as public_value;
@id("cleanup.main") fn main()->i64 {public_value(42)}
"#,
            ),
            (
                "src/tests.spx",
                r#"module cleanup.tests;
use function @id("cleanup.public") from cleanup.core as public_value;
@id("cleanup.test") fn main()->i64 {if public_value(42)==42 {0}else{1}}
"#,
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(fixture.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        fixture
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(&self.manifest(), VNextPolicy::default()).unwrap()
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        PATHS
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
fn readonly_chunks_reassemble_exact_cleanup_report_and_do_not_grant_candidate_authority() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    let mut session = fixture.session();
    assert_eq!(session.image_revision(), image.image_digest());
    let expected = image
        .cleanup_dependencies(image.image_digest(), TARGET)
        .unwrap();
    let mut actual = String::new();
    for _ in 0..8193 {
        let chunk = payload(bound(
            &mut session,
            METHOD,
            json!({"target":TARGET,"offset":actual.len(),"chunk_bytes":1024}),
        ));
        assert_eq!(
            chunk["schema"],
            "semaprax.image-cleanup-dependencies-chunk.v1"
        );
        assert_eq!(
            chunk["report_schema"],
            "semaprax.image-cleanup-dependencies.v1"
        );
        assert_eq!(chunk["target"], TARGET);
        assert_eq!(chunk["image_revision"], image.image_digest());
        assert_eq!(chunk["source_authority"], false);
        assert_eq!(chunk["offset"], actual.len());
        assert_eq!(chunk["total_bytes"], expected.len());
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty() && text.len() <= 1024);
        actual.push_str(text);
        if chunk["next_offset"].is_null() {
            break;
        }
        assert_eq!(chunk["next_offset"], actual.len());
    }
    assert_eq!(actual, expected);
    let eof = payload(bound(
        &mut session,
        METHOD,
        json!({"target":TARGET,"offset":actual.len()}),
    ));
    assert_eq!(eof["chunk"], "");
    assert!(eof["next_offset"].is_null());
    for forbidden in ["candidate/open", "candidate/commit", "candidate/build"] {
        assert_eq!(
            bound(&mut session, forbidden, json!({}))["error"]["code"],
            -32601
        );
    }
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn discovery_exposes_closed_readonly_schema_and_clients_without_changing_legacy_profiles() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
    assert!(capabilities["methods"]
        .as_array()
        .unwrap()
        .contains(&json!(METHOD)));
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let method = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == METHOD)
        .unwrap();
    assert_eq!(method["capability"], "semantic_read");
    assert_eq!(method["query"], true);
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 4);
    assert_eq!(params["properties"]["offset"]["maximum"], 8 * 1024 * 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
    let chunk = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.image-cleanup-dependencies-chunk.v1")
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    assert_eq!(chunk["properties"]["source_authority"]["const"], false);
    assert_eq!(
        chunk["properties"]["report_schema"]["const"],
        "semaprax.image-cleanup-dependencies.v1"
    );
    assert!(schemas["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.image-cleanup-dependencies.v1")));
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
        assert!(source.contains("request_image_cleanup_dependencies"));
        assert!(source.contains("decode_request_image_cleanup_dependencies"));
    }
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let request = frame(
            1,
            METHOD,
            json!({"image_revision":old.image_revision(),"target":TARGET}),
        );
        let response: Value = serde_json::from_slice(&old.handle_frame(&request).unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}

#[test]
fn invalid_and_stale_reads_do_not_poison_reports_and_source_drift_remains_absorbing() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session();
    let expected = bound(&mut session, METHOD, json!({"target":TARGET}));
    payload(expected.clone());
    let stale = call(
        &mut session,
        METHOD,
        json!({"image_revision":format!("sha256:{}", "0".repeat(64)),"target":TARGET}),
    );
    assert_eq!(stale["error"]["code"], -32000);
    assert!(stale["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    for params in [
        json!({"target":"missing.field"}),
        json!({"target":TARGET,"offset":8*1024*1024}),
    ] {
        assert_eq!(bound(&mut session, METHOD, params)["error"]["code"], -32000);
    }
    for params in [
        json!({"target":TARGET,"offset":8*1024*1024+1}),
        json!({"target":TARGET,"chunk_bytes":1023}),
        json!({"target":TARGET,"chunk_bytes":65537}),
        json!({"target":TARGET,"source":"untrusted"}),
    ] {
        assert_eq!(bound(&mut session, METHOD, params)["error"]["code"], -32602);
    }
    assert_eq!(
        bound(&mut session, METHOD, json!({"target":TARGET})),
        expected
    );
    assert_eq!(fixture.bytes(), disk);
    let path = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{source}\n// independent editor change\n")).unwrap();
    let drifted = fixture.bytes();
    assert!(bound(&mut session, METHOD, json!({"target":TARGET}))
        .get("error")
        .is_some());
    let request = frame(
        1,
        METHOD,
        json!({"image_revision":session.image_revision(),"target":TARGET}),
    );
    assert!(session.handle_read_batch(&[request.as_slice()], 1).is_err());
    assert!(session.finish().is_err());
    assert_eq!(fixture.bytes(), drifted);
}

#[test]
fn parallel_cleanup_reads_preserve_sequential_bytes_order_and_read_only_whitelist() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut sequential = fixture.session();
    let mut parallel = fixture.session();
    let revision = parallel.image_revision();
    let requests = [
        frame(
            8,
            METHOD,
            json!({"image_revision":revision,"target":TARGET,"chunk_bytes":1024}),
        ),
        frame(
            2,
            METHOD,
            json!({"image_revision":revision,"target":"cleanup.packet"}),
        ),
        frame(
            9,
            METHOD,
            json!({"image_revision":revision,"target":TARGET}),
        ),
        frame(
            1,
            METHOD,
            json!({"image_revision":revision,"target":"missing.field"}),
        ),
        frame(
            5,
            "image/dependencies",
            json!({"image_revision":revision,"target":TARGET}),
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
    assert!(parallel.parallel_read_methods().contains(&METHOD));
    for forbidden in [
        "candidate/open",
        "candidate/commit",
        "workspace/refresh",
        "hole/recovery-restore",
    ] {
        assert!(!parallel.parallel_read_methods().contains(&forbidden));
        let request = frame(7, forbidden, json!({}));
        let responses = parallel
            .handle_read_batch(&[request.as_slice()], 1)
            .unwrap();
        let response: Value = serde_json::from_slice(responses[0].as_ref().unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
    parallel.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn candidate_chunks_bind_both_target_and_candidate_without_entering_immutable_read_batches() {
    const CANDIDATE_METHOD: &str = "candidate/cleanup-dependencies";
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut readonly = fixture.session();
    assert_eq!(
        bound(&mut readonly, CANDIDATE_METHOD, json!({}))["error"]["code"],
        -32601
    );
    let base = with_authenticated_project(&fixture.manifest(), |snapshot| {
        ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
    })
    .unwrap();
    let intent = json!({"kind":"replace_function_body","target":"cleanup.discard","body":{"kind":"i64","value":1}});
    let changed = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap();
    let expected = changed
        .cleanup_dependencies(changed.candidate_digest(), TARGET)
        .unwrap();
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        },
    )
    .unwrap();
    let root = payload(bound(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let applied = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    ));
    assert_eq!(applied["candidate_revision"], changed.candidate_digest());
    let mut actual = String::new();
    for _ in 0..8193 {
        let chunk = payload(bound(
            &mut session,
            CANDIDATE_METHOD,
            json!({"candidate_revision":changed.candidate_digest(),"target":TARGET,"offset":actual.len(),"chunk_bytes":1024}),
        ));
        assert_eq!(
            chunk["schema"],
            "semaprax.image-candidate-cleanup-dependencies-chunk.v1"
        );
        assert_eq!(
            chunk["report_schema"],
            "semaprax.project-candidate-cleanup-dependencies.v1"
        );
        assert_eq!(chunk["candidate_revision"], changed.candidate_digest());
        assert_eq!(chunk["target"], TARGET);
        assert_eq!(chunk["offset"], actual.len());
        assert_eq!(chunk["source_authority"], false);
        let text = chunk["chunk"].as_str().unwrap();
        assert!(!text.is_empty() && text.len() <= 1024);
        actual.push_str(text);
        if chunk["next_offset"].is_null() {
            assert_eq!(chunk["total_bytes"], actual.len());
            break;
        }
        assert_eq!(chunk["next_offset"], actual.len());
    }
    assert_eq!(actual, expected);
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    let method = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == CANDIDATE_METHOD)
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 5);
    assert!(!session.parallel_read_methods().contains(&CANDIDATE_METHOD));
    let request = frame(
        9,
        CANDIDATE_METHOD,
        json!({"image_revision":session.image_revision(),"candidate_revision":changed.candidate_digest(),"target":TARGET}),
    );
    let batch = session.handle_read_batch(&[request.as_slice()], 1).unwrap();
    let rejected: Value = serde_json::from_slice(batch[0].as_ref().unwrap()).unwrap();
    assert_eq!(rejected["error"]["code"], -32601);
    for params in [
        json!({"candidate_revision":changed.candidate_digest(),"target":TARGET,"offset":-1}),
        json!({"candidate_revision":changed.candidate_digest(),"target":TARGET,"source_authority":true}),
    ] {
        assert_eq!(
            bound(&mut session, CANDIDATE_METHOD, params)["error"]["code"],
            -32602
        );
    }
    let unknown = bound(
        &mut session,
        CANDIDATE_METHOD,
        json!({"candidate_revision":format!("sha256:{}", "0".repeat(64)),"target":TARGET}),
    );
    assert_eq!(unknown["error"]["code"], -32000);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}
