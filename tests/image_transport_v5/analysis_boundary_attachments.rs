//! Generated-file and external-API declaration transports, authored and unrun.

use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const GENERATED_METHOD: &str = "candidate/analysis-generated-file-provenance-evidence";
const EXTERNAL_METHOD: &str = "candidate/analysis-external-api-contract-evidence";
const BUNDLE_METHOD: &str = "candidate/analysis-boundary-bundle";
const GENERATED_DECLARATION: &str =
    "semaprax.project-candidate-generated-file-provenance-declaration.v1";
const EXTERNAL_DECLARATION: &str =
    "semaprax.project-candidate-external-api-contract-declaration.v1";
const GENERATED_REPORT: &str = "semaprax.project-candidate-generated-file-provenance-evidence.v1";
const EXTERNAL_REPORT: &str = "semaprax.project-candidate-external-api-contract-evidence.v1";
const GENERATED_CHUNK: &str =
    "semaprax.image-candidate-generated-file-provenance-evidence-chunk.v1";
const EXTERNAL_CHUNK: &str = "semaprax.image-candidate-external-api-contract-evidence-chunk.v1";
const BUNDLE_SCHEMA: &str = "semaprax.project-candidate-analysis-boundary-bundle.v1";
const BUNDLE_REPORT: &str = "semaprax.project-candidate-analysis-boundary-bundle-report.v1";
const BUNDLE_CHUNK: &str = "semaprax.image-candidate-analysis-boundary-bundle-report-chunk.v1";
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
            "spx-analysis-boundary-attachments-{}-{}",
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
    fn session(&self, candidates: bool) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: candidates,
                ..Default::default()
            },
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if params.get("image_revision").is_none() {
        params["image_revision"] = json!(session.image_revision());
    }
    let frame = json!({"jsonrpc":"2.0","id":"attachments","method":method,"params":params});
    serde_json::from_slice(&session.handle_frame(frame.to_string().as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn canonical(value: Value, domain: &[u8]) -> (String, String) {
    let mut value = value;
    value.sort_all_objects();
    let mut bytes = serde_json::to_string(&value).unwrap();
    bytes.push('\n');
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes.as_bytes());
    (
        bytes,
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hash.finalize())
        ),
    )
}

#[test]
fn candidate_only_attachments_are_closed_chunked_typed_and_mcp_catalogued() {
    let fixture = Fixture::new();
    let mut unavailable = fixture.session(false);
    for method in [GENERATED_METHOD, EXTERNAL_METHOD, BUNDLE_METHOD] {
        assert_eq!(
            call(&mut unavailable, method, json!({}))["error"]["code"],
            -32601
        );
    }
    unavailable.finish().unwrap();

    let mut session = fixture.session(true);
    let candidate = payload(call(&mut session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let coverage = payload(call(
        &mut session,
        "candidate/analysis-coverage",
        json!({"candidate_revision":candidate}),
    ));
    let source = &coverage["sources"][0];
    let path = source["path"].as_str().unwrap();
    let source_bytes = std::fs::read(fixture.0.join(path)).unwrap();
    let (generated, generated_digest) = canonical(
        json!({
            "schema":GENERATED_DECLARATION,
            "candidate_revision":candidate,
            "files":[{
                "artifact":{"path":path,"bytes":source_bytes.len(),"sha256":source["source_digest"]},
                "source":{"path":path,"source_revision":source["source_revision"],"source_digest":source["source_digest"]},
                "generator":{"id":"fixture.generator:v1","digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111"}
            }]
        }),
        b"semaprax.project-candidate-generated-file-provenance-declaration.v1\0",
    );
    let operations = coverage["manifest"]["web_exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| json!({
            "export_id":id,
            "operation_digest":"sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "schema_digest":"sha256:3333333333333333333333333333333333333333333333333333333333333333"
        }))
        .collect::<Vec<_>>();
    let (external, external_digest) = canonical(
        json!({
            "schema":EXTERNAL_DECLARATION,
            "candidate_revision":candidate,
            "scope":{"kind":"manifest_exports"},
            "operations":operations
        }),
        b"semaprax.project-candidate-external-api-contract-declaration.v1\0",
    );
    let (deployment, deployment_digest) = canonical(
        json!({
            "schema":"semaprax.project-candidate-deployment-contract-declaration.v1",
            "candidate_revision":candidate,
            "manifest_exports":coverage["manifest"]["web_exports"],
            "configuration":[{"key":"SERVICE_MODE","type":"string","required":true}]
        }),
        b"semaprax.project-candidate-deployment-contract-declaration.v1\0",
    );
    let (bundle, bundle_digest) = canonical(
        json!({
            "schema":BUNDLE_SCHEMA,
            "candidate_revision":candidate,
            "deployment_contract":{"declaration":deployment,"declaration_digest":deployment_digest},
            "generated_file_provenance":{"declaration":generated.clone(),"declaration_digest":generated_digest.clone()},
            "external_api_contract":{"declaration":external.clone(),"declaration_digest":external_digest.clone()}
        }),
        b"semaprax.project-candidate-analysis-boundary-bundle.v1\0",
    );

    for (method, declaration, digest, chunk_schema, report_schema, false_fields) in [
        (
            GENERATED_METHOD,
            generated.as_str(),
            generated_digest.as_str(),
            GENERATED_CHUNK,
            GENERATED_REPORT,
            &[
                "source_authority",
                "filesystem_scan",
                "generator_execution",
                "artifact_materialization",
                "runtime_observation",
                "deployment_authority",
            ][..],
        ),
        (
            EXTERNAL_METHOD,
            external.as_str(),
            external_digest.as_str(),
            EXTERNAL_CHUNK,
            EXTERNAL_REPORT,
            &[
                "source_authority",
                "external_io",
                "network_observation",
                "provider_observation",
                "runtime_observation",
                "conformance_evidence",
                "ambient_authority",
                "deployment_authority",
            ][..],
        ),
    ] {
        let chunk = payload(call(
            &mut session,
            method,
            json!({"candidate_revision":candidate,"declaration":declaration,
                "declaration_digest":digest,"offset":0,"chunk_bytes":65536}),
        ));
        assert_eq!(chunk["schema"], chunk_schema);
        assert_eq!(chunk["report_schema"], report_schema);
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["declaration_digest"], digest);
        for field in false_fields {
            assert_eq!(chunk[*field], false);
        }
        let report: Value = serde_json::from_str(chunk["chunk"].as_str().unwrap()).unwrap();
        assert_eq!(report["schema"], report_schema);
    }
    let chunk = payload(call(
        &mut session,
        BUNDLE_METHOD,
        json!({"candidate_revision":candidate,"bundle":bundle.clone(),"bundle_digest":bundle_digest.clone(),
            "offset":0,"chunk_bytes":65536}),
    ));
    assert_eq!(chunk["schema"], BUNDLE_CHUNK);
    assert_eq!(chunk["report_schema"], BUNDLE_REPORT);
    assert_eq!(chunk["candidate_revision"], candidate);
    assert_eq!(chunk["bundle_digest"], bundle_digest);
    for field in [
        "source_authority",
        "external_io",
        "filesystem_scan",
        "generator_execution",
        "artifact_materialization",
        "network_observation",
        "provider_observation",
        "runtime_observation",
        "conformance_evidence",
        "ambient_authority",
        "publication_authority",
        "deployment_authority",
    ] {
        assert_eq!(chunk[field], false);
    }
    let report: Value = serde_json::from_str(chunk["chunk"].as_str().unwrap()).unwrap();
    assert_eq!(report["schema"], BUNDLE_REPORT);
    assert_eq!(
        report["analysis_boundary_bundle"]["owned_partial_areas"],
        json!([
            "deployment_configuration",
            "generated_file_provenance",
            "external_api_behavior"
        ])
    );

    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    for (method, chunk_schema, report_schema, max) in [
        (GENERATED_METHOD, GENERATED_CHUNK, GENERATED_REPORT, 65536),
        (EXTERNAL_METHOD, EXTERNAL_CHUNK, EXTERNAL_REPORT, 131072),
    ] {
        let descriptor = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method)
            .unwrap();
        assert_eq!(descriptor["capability"], "candidate_prepare");
        let params = &descriptor["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(params["properties"]["declaration"]["maxLength"], max);
        let chunk = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["$id"] == format!("urn:{chunk_schema}"))
            .unwrap();
        assert_eq!(chunk["additionalProperties"], false);
        assert_eq!(chunk["properties"]["report_schema"]["const"], report_schema);
        assert!(schemas["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(format!("urn:{report_schema}"))));
    }
    let descriptor = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == BUNDLE_METHOD)
        .unwrap();
    assert_eq!(descriptor["capability"], "candidate_prepare");
    let params = &descriptor["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"]["bundle"]["maxLength"], 24576);
    let document = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == format!("urn:{BUNDLE_CHUNK}"))
        .unwrap();
    assert_eq!(document["additionalProperties"], false);
    assert_eq!(
        document["properties"]["report_schema"]["const"],
        BUNDLE_REPORT
    );
    assert!(schemas["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(format!("urn:{BUNDLE_REPORT}"))));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        for name in [
            "candidate_analysis_generated_file_provenance_evidence",
            "candidate_analysis_external_api_contract_evidence",
            "candidate_analysis_boundary_bundle",
        ] {
            assert!(source.contains(&format!("request_{name}")));
            assert!(source.contains(&format!("decode_request_{name}")));
        }
    }

    let mut mcp = McpSession::new(fixture.session(true)).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"attachments","version":"1"}}});
    mcp.handle_frame(initialize.to_string().as_bytes()).unwrap();
    mcp.handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let mut names = Vec::new();
    let mut cursor = None;
    loop {
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":
            cursor.as_ref().map_or_else(||json!({}),|value|json!({"cursor":value}))});
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
    assert!(names.contains(&"candidate__analysis-generated-file-provenance-evidence".to_owned()));
    assert!(names.contains(&"candidate__analysis-external-api-contract-evidence".to_owned()));
    assert!(names.contains(&"candidate__analysis-boundary-bundle".to_owned()));
    mcp.finish().unwrap();
    session.finish().unwrap();
}
