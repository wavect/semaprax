//! v5 exact-revision function-reference transport evidence, authored and unrun.
use semaprax::image_transport::{
    ImageHostCapability, ImageSession, McpSession, VNextPolicy, VNextSession,
};
use semaprax::project::{with_authenticated_project, ImageFacet, ProjectSemanticImage};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const EXPORT: &str = "image/function-reference-export";
const RESOLVE: &str = "image/function-reference-resolve";
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
            "spx-function-reference-v5-{}-{}",
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

fn frame(id: usize, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn reference_text(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}
fn export_params(image: &str, facet: Option<&str>) -> Value {
    let mut params = json!({"image_revision":image,"target":TARGET});
    if let Some(facet) = facet {
        params["facet"] = json!(facet);
    }
    params
}
fn resolve_params(image: &str, reference: &Value) -> Value {
    json!({"image_revision":image,"reference":reference_text(reference)})
}

#[test]
fn direct_export_and_resolve_equal_library_values_and_authenticated_parallel_batches() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    let revision = image.image_digest().to_owned();
    let expected_reference: Value = serde_json::from_str(
        &image
            .export_function_reference(&revision, TARGET, Some(ImageFacet::Signature))
            .unwrap(),
    )
    .unwrap();
    let expected_resolution: Value = serde_json::from_str(
        &image
            .resolve_function_reference(&revision, reference_text(&expected_reference).as_bytes())
            .unwrap(),
    )
    .unwrap();
    let mut direct = fixture.session();
    let actual_reference = payload(call(
        &mut direct,
        EXPORT,
        export_params(&revision, Some("signature")),
    ));
    assert_eq!(actual_reference, expected_reference);
    let actual_resolution = payload(call(
        &mut direct,
        RESOLVE,
        resolve_params(&revision, &actual_reference),
    ));
    assert_eq!(actual_resolution, expected_resolution);
    assert_eq!(actual_resolution["function_summary"]["id"], TARGET);
    assert_eq!(actual_resolution["facet"], "signature");
    assert_eq!(
        actual_resolution["facet_handle"],
        actual_resolution["function_summary"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["facet"] == "signature")
            .unwrap()["handle"]
    );
    for value in [&actual_reference, &actual_resolution] {
        for field in ["source_authority", "execution", "publication_authority"] {
            assert_eq!(value[field], false);
        }
    }
    let rebuilt_fixture = Fixture::new();
    assert_ne!(fixture.0, rebuilt_fixture.0);
    assert_eq!(fixture.bytes(), rebuilt_fixture.bytes());
    let mut rebuilt = rebuilt_fixture.session();
    assert_eq!(rebuilt.image_revision(), revision);
    assert_eq!(
        payload(call(
            &mut rebuilt,
            RESOLVE,
            resolve_params(&revision, &actual_reference),
        )),
        expected_resolution
    );
    rebuilt.finish().unwrap();

    let requests = [
        frame(8, EXPORT, export_params(&revision, None)),
        frame(3, EXPORT, export_params(&revision, Some("signature"))),
        frame(5, RESOLVE, resolve_params(&revision, &actual_reference)),
    ];
    let mut sequential = fixture.session();
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let borrowed = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            fixture
                .session()
                .handle_read_batch(&borrowed, workers)
                .unwrap(),
            expected
        );
    }
    let mut rpc = fixture.session().with_read_batch_workers(2).unwrap();
    let batch = payload(call(
        &mut rpc,
        "workspace/read-batch",
        json!({"image_revision":revision,"batch":{"frames":requests.iter().map(|request|std::str::from_utf8(request).unwrap()).collect::<Vec<_>>()}}),
    ));
    let expected_text = expected
        .iter()
        .map(|response| {
            response
                .as_ref()
                .map(|bytes| std::str::from_utf8(bytes).unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(batch["responses"], json!(expected_text));
    for method in [EXPORT, RESOLVE] {
        assert!(rpc.parallel_read_methods().contains(&method));
    }
    rpc.finish().unwrap();
    direct.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn discovery_bundles_closed_payloads_and_generated_clients_without_new_grants() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
    for method in [EXPORT, RESOLVE] {
        assert!(capabilities["methods"]
            .as_array()
            .unwrap()
            .contains(&json!(method)));
    }
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    for (method, parameter_count) in [(EXPORT, 3), (RESOLVE, 2)] {
        let descriptor = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method)
            .unwrap();
        assert_eq!(descriptor["capability"], "semantic_read");
        assert_eq!(descriptor["query"], true);
        let params = &descriptor["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(
            params["properties"].as_object().unwrap().len(),
            parameter_count
        );
        if method == EXPORT {
            assert_eq!(
                params["properties"]["facet"]["enum"],
                json!([
                    "signature",
                    "contracts",
                    "callers",
                    "ownership",
                    "loans",
                    "cleanup",
                    "relationships",
                    "data-access",
                    "unsafe-boundaries"
                ])
            );
        } else {
            assert_eq!(params["properties"]["reference"]["maxLength"], 16 * 1024);
        }
    }
    for (schema, fields) in [
        ("urn:semaprax.image-function-reference.v1", 14),
        ("urn:semaprax.image-function-reference-resolution.v1", 14),
    ] {
        let document = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["$id"] == schema)
            .unwrap();
        assert_eq!(document["additionalProperties"], false);
        assert_eq!(document["required"].as_array().unwrap().len(), fields);
        assert!(!schemas["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(schema)));
        for field in ["source_authority", "execution", "publication_authority"] {
            assert_eq!(document["properties"][field]["const"], false);
        }
    }
    let resolution = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.image-function-reference-resolution.v1")
        .unwrap();
    assert_eq!(
        resolution["properties"]["function_summary"]["additionalProperties"],
        false
    );
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        for fragment in [
            "ImageFunctionReferenceExportPayload",
            "ImageFunctionReferenceResolvePayload",
            "request_image_function_reference_export_typed",
            "decode_request_image_function_reference_resolve_typed",
        ] {
            assert!(source.contains(fragment), "{language}: {fragment}");
        }
    }
    session.finish().unwrap();

    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        for method in [EXPORT, RESOLVE] {
            let response: Value =
                serde_json::from_slice(&old.handle_frame(&frame(1, method, json!({}))).unwrap())
                    .unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
}

#[test]
fn catalogue_mcp_export_and_resolve_match_direct_json_rpc_bytes() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let revision = image.image_digest().to_owned();
    let reference: Value = serde_json::from_str(
        &image
            .export_function_reference(&revision, TARGET, Some(ImageFacet::Contracts))
            .unwrap(),
    )
    .unwrap();
    let arguments = resolve_params(&revision, &reference);
    let mut direct = fixture.session();
    let direct_bytes = direct
        .handle_frame(&frame(0, RESOLVE, arguments.clone()))
        .unwrap();

    let mut mcp = McpSession::new(fixture.session()).unwrap();
    let initialized = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},
        "clientInfo":{"name":"function-reference","version":"1"}}});
    mcp.handle_frame(initialized.to_string().as_bytes())
        .unwrap();
    assert!(mcp
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    let mut tools = Vec::new();
    let mut cursor = None;
    loop {
        let params = cursor
            .as_ref()
            .map_or_else(|| json!({}), |value| json!({"cursor":value}));
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":params});
        let page: Value =
            serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        tools.extend(
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
    assert!(tools.contains(&"image__function-reference-export".to_owned()));
    assert!(tools.contains(&"image__function-reference-resolve".to_owned()));
    let request = json!({"jsonrpc":"2.0","id":11,"method":"tools/call","params":{
        "name":"image__function-reference-resolve","arguments":arguments}});
    let response: Value =
        serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap()).unwrap();
    assert_eq!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .as_bytes(),
        direct_bytes
    );
    mcp.finish().unwrap();
    direct.finish().unwrap();
}

#[test]
fn hostile_requests_and_carriers_fail_closed_then_valid_reads_recover() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    let revision = image.image_digest().to_owned();
    let mut session = fixture.session();
    let reference = payload(call(
        &mut session,
        EXPORT,
        export_params(&revision, Some("callers")),
    ));
    for params in [
        json!({"image_revision":revision,"target":TARGET,"facet":"unknown"}),
        json!({"image_revision":revision,"target":TARGET,"facet":null}),
        json!({"image_revision":revision,"target":TARGET,"extra":true}),
        json!({"image_revision":revision}),
        json!({"image_revision":revision,"reference":null}),
        json!({"image_revision":revision,"reference":"{}","extra":true}),
    ] {
        let method = if params.get("target").is_some() || params.get("facet").is_some() {
            EXPORT
        } else {
            RESOLVE
        };
        assert_eq!(call(&mut session, method, params)["error"]["code"], -32602);
    }
    let mut extra = reference.clone();
    extra["extra"] = json!(true);
    let mut missing = reference.clone();
    missing.as_object_mut().unwrap().remove("target");
    let tampered = reference_text(&reference).replacen(TARGET, "calculator.subtract", 1);
    for carrier in [reference_text(&extra), reference_text(&missing), tampered] {
        let failed = call(
            &mut session,
            RESOLVE,
            json!({"image_revision":revision,"reference":carrier}),
        );
        assert_eq!(failed["error"]["code"], -32000);
        assert!(failed["error"].to_string().contains("SPX-G363"));
    }
    let stale = call(
        &mut session,
        RESOLVE,
        json!({"image_revision":format!("sha256:{}","0".repeat(64)),"reference":reference_text(&reference)}),
    );
    assert!(stale.get("error").is_some(), "{stale}");
    let recovered = payload(call(
        &mut session,
        RESOLVE,
        resolve_params(&revision, &reference),
    ));
    assert_eq!(recovered["target"], TARGET);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}
