//! Closed source-review transport evidence, authored and intentionally unrun.
use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, SemanticChange,
    PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-source-review-v5-{}-{}",
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
    fn candidate(&self, changed: bool) -> ProjectCandidate {
        let base = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap();
        if !changed {
            return base;
        }
        let change = SemanticChange::new(
            base.revision().project_revision(),
            &json!({
                "kind":"rename_declaration", "target":"calculator.add", "name":"addition"
            }),
        )
        .unwrap();
        base.apply(base.candidate_digest(), &change).unwrap()
    }
    fn host(&self) -> (VNextSession, String) {
        let candidate = self.candidate(true);
        let digest = candidate.candidate_digest().to_owned();
        let mut host = VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap();
        host.retain_archived_candidate(candidate, &digest).unwrap();
        (host, digest)
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0", "id":7, "method":method, "params":params})
        .to_string()
        .into_bytes()
}
fn call(host: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&host.handle_frame(&frame(method, params)).unwrap()).unwrap()
}
fn payload(response: &Value) -> &Value {
    assert!(response.get("error").is_none(), "{response}");
    &response["result"]["payload"]
}

#[test]
fn source_review_chunks_reassemble_exact_library_report_without_source_changes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let candidate = fixture.candidate(true);
    let expected = candidate
        .source_review(candidate.candidate_digest())
        .unwrap();
    let (mut host, digest) = fixture.host();
    let image = host.image_revision().to_owned();
    let mut offset = 0;
    let mut assembled = String::new();
    loop {
        let response = call(
            &mut host,
            "candidate/source-review",
            json!({
                "image_revision":image,"candidate_revision":digest,"offset":offset,"chunk_bytes":1024
            }),
        );
        let chunk = payload(&response);
        assert_eq!(chunk.as_object().unwrap().len(), 9);
        assert_eq!(chunk["schema"], "semaprax.image-source-review-chunk.v1");
        assert_eq!(
            chunk["report_schema"],
            PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA
        );
        assert_eq!(chunk["candidate_revision"], digest);
        assert_eq!(chunk["image_revision"], image);
        assert_eq!(chunk["offset"], offset);
        assert_eq!(chunk["total_bytes"], expected.len());
        assert_eq!(chunk["source_authority"], false);
        let text = chunk["chunk"].as_str().unwrap();
        assert!(text.len() <= 1024);
        assembled.push_str(text);
        let Some(next) = chunk["next_offset"].as_u64() else {
            assert!(chunk["next_offset"].is_null());
            break;
        };
        assert_eq!(next as usize, assembled.len());
        assert!(next > offset);
        offset = next;
    }
    assert_eq!(assembled, expected);
    let report: Value = serde_json::from_str(&assembled).unwrap();
    assert_eq!(report.as_object().unwrap().len(), 7);
    assert!(!report["files"].as_array().unwrap().is_empty());
    for file in report["files"].as_array().unwrap() {
        assert_eq!(file.as_object().unwrap().len(), 7);
        assert_ne!(file["base_source"], file["candidate_source"]);
    }
    let empty = fixture.candidate(false);
    let report: Value =
        serde_json::from_str(&empty.source_review(empty.candidate_digest()).unwrap()).unwrap();
    assert_eq!(report["files"], json!([]));
    assert_eq!(fixture.bytes(), disk);
    host.finish().unwrap();
}

#[test]
fn discovery_bundles_closed_report_and_generates_selected_typed_and_mcp_routes() {
    let fixture = Fixture::new();
    let (mut host, digest) = fixture.host();
    let image = host.image_revision().to_owned();
    let schemas = call(&mut host, "protocol/schemas", json!({}));
    let schemas = payload(&schemas);
    let method = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|method| method["method"] == "candidate/source-review")
        .unwrap();
    assert_eq!(method["capability"], "candidate_prepare");
    assert_eq!(method["query"], true);
    assert_eq!(
        method["request_schema"]["properties"]["params"]["required"],
        json!(["image_revision", "candidate_revision"])
    );
    let id = format!("urn:{PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA}");
    let report = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == id)
        .unwrap();
    assert_eq!(report["additionalProperties"], false);
    assert_eq!(report["properties"]["files"]["maxItems"], 16);
    assert_eq!(
        report["properties"]["files"]["items"]["additionalProperties"],
        false
    );
    assert!(!schemas["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(id)));
    for language in ["typescript", "python", "rust"] {
        let result = call(&mut host, "protocol/client", json!({"language":language}));
        let source = payload(&result)["source"].as_str().unwrap();
        assert!(source.contains("CandidateSourceReviewPayload"));
        assert!(source.contains("decode_request_candidate_source_review_typed"));
    }
    let (mcp_host, _) = fixture.host();
    let mut mcp = McpSession::new(mcp_host).unwrap();
    mcp.handle_frame(&frame("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"source-review-evidence","version":"1"}}))).unwrap();
    assert!(mcp
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    let args = json!({"image_revision":image,"candidate_revision":digest,"chunk_bytes":1024});
    let direct = host
        .handle_frame(
            &json!({"jsonrpc":"2.0","id":0,"method":"candidate/source-review","params":args})
                .to_string()
                .into_bytes(),
        )
        .unwrap();
    let response: Value = serde_json::from_slice(
        &mcp.handle_frame(&frame(
            "tools/call",
            json!({"name":"candidate__source-review","arguments":args}),
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(response["result"]["isError"], false);
    assert_eq!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .as_bytes(),
        direct
    );
    mcp.finish().unwrap();
    host.finish().unwrap();
}

#[test]
fn immutable_batch_matches_sequential_and_rejects_bad_selectors_and_offsets() {
    let fixture = Fixture::new();
    let (mut sequential, digest) = fixture.host();
    let (mut parallel, _) = fixture.host();
    let image = sequential.image_revision().to_owned();
    let report = fixture.candidate(true).source_review(&digest).unwrap();
    let params = json!({"image_revision":image,"candidate_revision":digest,"chunk_bytes":1024});
    let mut past_end = params.clone();
    past_end["offset"] = json!(report.len() + 1);
    let mut unknown = params.clone();
    unknown["candidate_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    let frames = [
        frame("candidate/source-review", params),
        frame("candidate/source-review", past_end),
        frame("candidate/source-review", unknown),
    ];
    let expected = frames
        .iter()
        .map(|frame| sequential.handle_frame(frame))
        .collect::<Vec<_>>();
    let refs = frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let batch = parallel.handle_read_batch(&refs, 3).unwrap();
    assert_eq!(batch, expected);
    for (index, code) in [(1, "SPX-G222"), (2, "SPX-G224")] {
        let response: Value = serde_json::from_slice(batch[index].as_ref().unwrap()).unwrap();
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains(code));
    }
    sequential.finish().unwrap();
    parallel.finish().unwrap();
}

#[test]
fn no_candidate_grant_and_live_source_drift_cannot_expose_source_review() {
    let fixture = Fixture::new();
    let mut readonly =
        VNextSession::open(&fixture.0.join("semaprax.toml"), VNextPolicy::default()).unwrap();
    let response = call(&mut readonly, "candidate/source-review", json!({}));
    assert_eq!(response["error"]["code"], -32601);
    readonly.finish().unwrap();
    let (mut host, digest) = fixture.host();
    let image = host.image_revision().to_owned();
    std::fs::write(fixture.0.join("src/app.spx"), b"drift\n").unwrap();
    let response = call(
        &mut host,
        "candidate/source-review",
        json!({"image_revision":image,"candidate_revision":digest}),
    );
    assert!(response.get("error").is_some());
    assert!(host.finish().is_err());
}
