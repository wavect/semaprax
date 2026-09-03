//! Candidate deployment-contract v5 transport, authored and intentionally unrun.

use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "candidate/analysis-deployment-contract-evidence";
const DECLARATION_SCHEMA: &str = "semaprax.project-candidate-deployment-contract-declaration.v1";
const EVIDENCE_SCHEMA: &str = "semaprax.project-candidate-deployment-contract-evidence.v1";
const CHUNK_SCHEMA: &str = "semaprax.image-candidate-deployment-contract-evidence-chunk.v1";
const DOMAIN: &[u8] = b"semaprax.project-candidate-deployment-contract-declaration.v1\0";
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
            "spx-deployment-contract-transport-{}-{}",
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
    fn session(&self, candidates: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: candidates,
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"deployment-contract","method":method,"params":params})
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
fn declaration(candidate: &str) -> (String, String) {
    let mut value = json!({
        "schema":DECLARATION_SCHEMA,
        "candidate_revision":candidate,
        "manifest_exports":[
            "calculator.add","calculator.divide","calculator.is-negative",
            "calculator.multiply","calculator.not","calculator.subtract"
        ],
        "configuration":[
            {"key":"API_BASE_URL","type":"string","required":true},
            {"key":"DEPLOYMENT_REGION","type":"string","required":false},
            {"key":"MAX_RETRIES","type":"integer","required":true}
        ]
    });
    value.sort_all_objects();
    let mut declaration = serde_json::to_string(&value).unwrap();
    declaration.push('\n');
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((declaration.len() as u64).to_le_bytes());
    hash.update(declaration.as_bytes());
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    );
    (declaration, digest)
}
fn report(
    session: &mut VNextSession,
    candidate: &str,
    declaration: &str,
    declaration_digest: &str,
) -> String {
    let mut report = String::new();
    let mut report_digest = None;
    for _ in 0..34 {
        let chunk = payload(bound(
            session,
            METHOD,
            json!({"candidate_revision":candidate,"declaration":declaration,
                "declaration_digest":declaration_digest,"offset":report.len(),
                "chunk_bytes":1024}),
        ));
        assert_eq!(chunk.as_object().unwrap().len(), 14);
        assert_eq!(chunk["schema"], CHUNK_SCHEMA);
        assert_eq!(chunk["report_schema"], EVIDENCE_SCHEMA);
        assert_eq!(chunk["image_revision"], session.image_revision());
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["declaration_digest"], declaration_digest);
        assert_eq!(chunk["offset"].as_u64().unwrap() as usize, report.len());
        for field in [
            "source_authority",
            "external_io",
            "environment_observation",
            "deployment_authority",
        ] {
            assert_eq!(chunk[field], false);
        }
        let selected = chunk["report_sha256"].as_str().unwrap().to_owned();
        if let Some(expected) = &report_digest {
            assert_eq!(&selected, expected);
        } else {
            report_digest = Some(selected);
        }
        report.push_str(chunk["chunk"].as_str().unwrap());
        if chunk["next_offset"].is_null() {
            assert_eq!(
                chunk["total_bytes"].as_u64().unwrap() as usize,
                report.len()
            );
            let digest = format!(
                "sha256:{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(report.as_bytes()))
            );
            assert_eq!(report_digest.unwrap(), digest);
            return report;
        }
        assert_eq!(
            chunk["next_offset"].as_u64().unwrap() as usize,
            report.len()
        );
    }
    panic!("bounded deployment evidence report did not terminate")
}

#[test]
fn exact_caller_declaration_reassembles_candidate_bound_read_only_evidence() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true);
    let candidate = open(&mut session);
    let (declaration, declaration_digest) = declaration(&candidate);
    let report = report(&mut session, &candidate, &declaration, &declaration_digest);
    let value: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(value["schema"], EVIDENCE_SCHEMA);
    assert_eq!(value["candidate_revision"], candidate);
    assert_eq!(
        value["deployment_contract_declaration"]["canonical_json"],
        declaration
    );
    assert_eq!(
        value["deployment_contract_declaration"]["digest"],
        declaration_digest
    );
    assert_eq!(
        value["deployment_contract_declaration"]["source_authority"],
        false
    );
    assert_eq!(
        value["deployment_contract_declaration"]["environment_observation"],
        false
    );
    assert_eq!(
        value["deployment_contract_declaration"]["deployment_authority"],
        false
    );
    assert!(!session.parallel_read_methods().contains(&METHOD));
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn candidate_read_grant_selects_closed_schema_generated_clients_and_mcp_tool() {
    let fixture = Fixture::new();
    let mut unavailable = fixture.session(false);
    assert_eq!(
        call(&mut unavailable, METHOD, json!({}))["error"]["code"],
        -32601
    );
    unavailable.finish().unwrap();

    let mut direct = fixture.session(true);
    let image = direct.image_revision().to_owned();
    let candidate = open(&mut direct);
    let (declaration, declaration_digest) = declaration(&candidate);
    let capabilities = payload(call(&mut direct, "protocol/capabilities", json!({})));
    assert!(capabilities["methods"]
        .as_array()
        .unwrap()
        .contains(&json!(METHOD)));
    let schemas = payload(call(&mut direct, "protocol/schemas", json!({})));
    let descriptor = schemas["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == METHOD)
        .unwrap();
    assert_eq!(descriptor["capability"], "candidate_prepare");
    assert_eq!(descriptor["query"], true);
    let params = &descriptor["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["properties"].as_object().unwrap().len(), 6);
    assert_eq!(params["properties"]["declaration"]["maxLength"], 65536);
    assert!(params["properties"]["declaration"].get("pattern").is_none());
    assert_eq!(params["properties"]["chunk_bytes"]["minimum"], 1024);
    assert_eq!(params["properties"]["chunk_bytes"]["maximum"], 65536);
    let chunk = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == format!("urn:{CHUNK_SCHEMA}"))
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    assert_eq!(chunk["properties"].as_object().unwrap().len(), 14);
    assert_eq!(
        chunk["properties"]["report_schema"]["const"],
        EVIDENCE_SCHEMA
    );
    for field in [
        "source_authority",
        "external_io",
        "environment_observation",
        "deployment_authority",
    ] {
        assert_eq!(chunk["properties"][field]["const"], false);
    }
    assert!(schemas["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!(format!("urn:{EVIDENCE_SCHEMA}"))));
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut direct,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        assert!(source.contains("request_candidate_analysis_deployment_contract_evidence"));
        assert!(source.contains("decode_request_candidate_analysis_deployment_contract_evidence"));
    }

    let mcp_host = fixture.session(true);
    assert_eq!(mcp_host.image_revision(), image);
    let mut mcp = McpSession::new(mcp_host).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},
        "clientInfo":{"name":"deployment-contract-evidence","version":"1"}}});
    mcp.handle_frame(initialize.to_string().as_bytes()).unwrap();
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
    assert!(names.contains(&"candidate__analysis-deployment-contract-evidence".to_owned()));
    // The MCP host keeps its own candidate registry and `McpSession::new`
    // requires a pristine session, so the handle has to be opened through the
    // tool surface. The digest is content addressed, so both sessions agree.
    let opened = json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{
        "name":"candidate__open","arguments":{"image_revision":image}}});
    let opened: Value =
        serde_json::from_slice(&mcp.handle_frame(opened.to_string().as_bytes()).unwrap()).unwrap();
    let opened: Value =
        serde_json::from_str(opened["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(opened["result"]["payload"]["candidate_revision"], candidate);
    let arguments = json!({"image_revision":image,"candidate_revision":candidate,
        "declaration":declaration,"declaration_digest":declaration_digest,
        "offset":0,"chunk_bytes":1024});
    let direct_bytes = direct
        .handle_frame(
            json!({"jsonrpc":"2.0","id":0,"method":METHOD,"params":arguments})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let invoked = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"candidate__analysis-deployment-contract-evidence","arguments":arguments}});
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
fn stale_digest_unknown_parameters_and_invalid_chunks_fail_before_any_authority() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true);
    let candidate = open(&mut session);
    let (declaration, declaration_digest) = declaration(&candidate);
    let valid = json!({"candidate_revision":candidate,"declaration":declaration,
        "declaration_digest":declaration_digest});
    for extra in [
        json!({"path":"deployment.json"}),
        json!({"environment":{"API_BASE_URL":"secret"}}),
        json!({"chunk_bytes":1023}),
        json!({"offset":-1}),
    ] {
        let mut params = valid.clone();
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(bound(&mut session, METHOD, params)["error"]["code"], -32602);
    }
    let mut wrong_digest = valid.clone();
    wrong_digest["declaration_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert!(
        bound(&mut session, METHOD, wrong_digest)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SPX-G426")
    );
    let mut stale = valid.clone();
    stale["candidate_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert_eq!(bound(&mut session, METHOD, stale)["error"]["code"], -32000);
    let mut outside = valid;
    outside["offset"] = json!(2 * 1024 * 1024);
    assert!(bound(&mut session, METHOD, outside)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G427"));
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}
