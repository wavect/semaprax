//! Retained analysis boundaries: authored regressions, intentionally unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectSemanticImage, IMAGE_ANALYSIS_COVERAGE_SCHEMA,
    MAX_IMAGE_ANALYSIS_COVERAGE_BYTES,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "image/analysis-coverage";
const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new(interface: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-analysis-coverage-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "analysis-coverage"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "coverage.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["coverage.public"]
tests = ["coverage.tests"]
"#,
        )
        .unwrap();
        let mut core = "module coverage.core;\n@id(\"coverage.public\") fn public_value(value:i64)->i64 {value}\n".to_owned();
        if interface {
            core.push_str(
                r#"@id("coverage.host") interface Host permits {} {
    @id("coverage.host.echo") import rust fn echo(value:i64)->unit effects {} failure infallible;
}
"#,
            );
        }
        for (path, text) in [
            (
                "src/app.spx",
                "module coverage.app;\n@id(\"coverage.main\") fn main()->i64 {0}\n",
            ),
            ("src/core.spx", core.as_str()),
            (
                "src/tests.spx",
                "module coverage.tests;\n@id(\"coverage.test\") fn main()->i64 {0}\n",
            ),
        ] {
            let program = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), VNextPolicy::default()).unwrap()
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
fn report(image: &ProjectSemanticImage) -> (String, Value) {
    let text = image.analysis_coverage(image.image_digest()).unwrap();
    assert!(text.len() <= MAX_IMAGE_ANALYSIS_COVERAGE_BYTES);
    assert!(!text.ends_with('\n'));
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}
fn area<'a>(report: &'a Value, name: &str) -> &'a Value {
    let rows = report["areas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["area"] == name)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]
}
fn frame(id: u64, method: &str, params: Value) -> Vec<u8> {
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

#[test]
fn no_imports_still_explicitly_leave_external_generated_deployment_and_runtime_areas_uninspected() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let image = fixture.image();
    let image_before = image.to_json().to_owned();
    let (text, value) = report(&image);
    assert_eq!(text, image.analysis_coverage(image.image_digest()).unwrap());
    assert_eq!(value["schema"], IMAGE_ANALYSIS_COVERAGE_SCHEMA);
    assert_eq!(value["image_revision"], image.image_digest());
    assert_eq!(
        value["project_revision"],
        image.revision().project_revision()
    );
    assert_eq!(
        value["workspace_revision"],
        image.revision().workspace_revision()
    );
    assert_eq!(
        value["project_graph_digest"],
        image.revision().semantic_graph_digest()
    );
    for flag in ["source_authority", "external_io", "execution"] {
        assert_eq!(value[flag], false);
    }
    assert_eq!(value["inventory"]["source_modules"], 3);
    assert_eq!(value["inventory"]["interface_imports"], 0);
    assert_eq!(value["external_contracts"], json!([]));
    assert_eq!(value["areas"].as_array().unwrap().len(), 8);
    assert_eq!(area(&value, "declared_source_inputs")["status"], "known");
    for name in [
        "declared_external_contracts",
        "deployment_configuration",
        "generated_file_provenance",
        "generated_artifacts",
        "external_api_behavior",
        "runtime_environment",
        "external_consumers",
    ] {
        let entry = area(&value, name);
        assert_eq!(entry["status"], "not_inspected");
        assert!(!entry["limitations"].as_array().unwrap().is_empty());
        assert!(!entry["required_evidence"].as_array().unwrap().is_empty());
    }
    assert!(area(&value, "declared_external_contracts")["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!(
            "zero_imports_does_not_prove_no_external_or_network_dependencies"
        )));
    assert!(area(&value, "external_consumers")["limitations"]
        .as_array()
        .unwrap()
        .contains(&json!(
            "absence_of_graph_edges_is_not_absence_of_external_callers"
        )));
    let sources = value["sources"].as_array().unwrap();
    assert_eq!(sources.len(), image.revision().sources().len());
    for retained in image.revision().sources() {
        let selected = sources
            .iter()
            .find(|row| row["path"] == retained.path())
            .unwrap();
        assert_eq!(selected["source_revision"], retained.source_revision());
        assert_eq!(selected["source_digest"], retained.source_digest());
        assert_eq!(
            selected["source_graph_schema"],
            retained.source_graph_schema()
        );
    }
    assert!(sources
        .windows(2)
        .all(|pair| pair[0]["path"].as_str() < pair[1]["path"].as_str()));
    assert_eq!(image.to_json(), image_before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn declared_native_interface_is_partial_contract_evidence_never_implementation_verification() {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let image = fixture.image();
    let (_, value) = report(&image);
    assert_eq!(value["inventory"]["interfaces"], 1);
    assert_eq!(value["inventory"]["interface_imports"], 1);
    assert_eq!(
        value["external_contracts"],
        json!([{"path":"src/core.spx","module":"coverage.core","interface_id":"coverage.host","import_id":"coverage.host.echo","name":"echo","import_key":"coverage.host.echo","native_rust":true,"effects":[],"required_authority":[]}])
    );
    let declared = area(&value, "declared_external_contracts");
    assert_eq!(declared["status"], "partial");
    assert!(declared["limitations"].as_array().unwrap().contains(&json!(
        "declarations_are_not_external_implementation_evidence"
    )));
    assert_eq!(
        area(&value, "external_api_behavior")["status"],
        "not_inspected"
    );
    assert!(value["nonclaims"].as_array().unwrap().contains(&json!(
        "no_external_implementation_contract_or_runtime_conformance_proof"
    )));
    assert_eq!(value["external_io"], false);
    assert_eq!(value["execution"], false);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn retained_report_is_not_a_disk_scan_and_wrong_image_selectors_fail_without_mutation() {
    let fixture = Fixture::new(false);
    let image = fixture.image();
    let (before, _) = report(&image);
    let stale = format!("sha256:{}", "0".repeat(64));
    for (selector, code) in [(stale.as_str(), "SPX-G221"), ("not-a-digest", "SPX-G219")] {
        let errors = image.analysis_coverage(selector).unwrap_err();
        assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
    }
    std::fs::write(
        fixture.0.join("deployment.secret"),
        b"unlisted value must never enter report",
    )
    .unwrap();
    std::fs::remove_file(fixture.0.join("src/core.spx")).unwrap();
    assert_eq!(
        image.analysis_coverage(image.image_digest()).unwrap(),
        before
    );
    assert!(!fixture.0.join("src/core.spx").exists());
    assert!(!before.contains("unlisted value"));
    assert_eq!(
        std::fs::read(fixture.0.join("deployment.secret")).unwrap(),
        b"unlisted value must never enter report"
    );
}

#[test]
fn readonly_transport_exposes_closed_schema_exact_payload_and_parallel_parity() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let image = fixture.image();
    let (_, expected) = report(&image);
    let mut sequential = fixture.session();
    let mut parallel = fixture.session();
    assert_eq!(sequential.image_revision(), image.image_digest());
    let requests = [
        frame(9, METHOD, json!({"image_revision":image.image_digest()})),
        frame(
            2,
            METHOD,
            json!({"image_revision":format!("sha256:{}", "0".repeat(64))}),
        ),
        frame(
            5,
            METHOD,
            json!({"image_revision":image.image_digest(),"execution":true}),
        ),
    ];
    let responses = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let response: Value = serde_json::from_slice(responses[0].as_ref().unwrap()).unwrap();
    assert_eq!(response["result"]["image_revision"], image.image_digest());
    assert_eq!(payload(response), expected);
    for response in &responses[1..] {
        let value: Value = serde_json::from_slice(response.as_ref().unwrap()).unwrap();
        assert!(value.get("error").is_some());
    }
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            parallel.handle_read_batch(&refs, workers).unwrap(),
            responses
        );
    }
    assert!(parallel.parallel_read_methods().contains(&METHOD));
    for denied in [
        "candidate/open",
        "candidate/build",
        "candidate/test",
        "candidate/commit",
    ] {
        assert_eq!(
            call(
                &mut parallel,
                denied,
                json!({"image_revision":image.image_digest()})
            )["error"]["code"],
            -32601
        );
    }
    let bundle = payload(call(&mut parallel, "protocol/schemas", json!({})));
    let method = bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == METHOD)
        .unwrap();
    assert_eq!(method["query"], true);
    assert_eq!(method["capability"], "semantic_read");
    let params = &method["request_schema"]["properties"]["params"];
    assert_eq!(params["additionalProperties"], false);
    assert_eq!(params["required"], json!(["image_revision"]));
    let doc = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.image-analysis-coverage.v1")
        .unwrap();
    assert_eq!(doc["additionalProperties"], false);
    for flag in ["source_authority", "external_io", "execution"] {
        assert_eq!(doc["properties"][flag]["const"], false);
    }
    assert_eq!(doc["properties"]["areas"]["minItems"], 8);
    assert_eq!(doc["properties"]["areas"]["maxItems"], 8);
    assert!(!bundle["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.image-analysis-coverage.v1")));
    parallel.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn live_transport_source_drift_fails_while_retained_library_evidence_stays_immutable() {
    let fixture = Fixture::new(false);
    let image = fixture.image();
    let (before, _) = report(&image);
    let mut session = fixture.session();
    payload(call(
        &mut session,
        METHOD,
        json!({"image_revision":image.image_digest()}),
    ));
    let path = fixture.0.join("src/app.spx");
    let changed = std::fs::read_to_string(&path).unwrap() + "\n// external source drift\n";
    std::fs::write(&path, changed).unwrap();
    let disk = fixture.bytes();
    let error = call(
        &mut session,
        METHOD,
        json!({"image_revision":image.image_digest()}),
    );
    assert!(error.get("error").is_some());
    assert_eq!(
        image.analysis_coverage(image.image_digest()).unwrap(),
        before
    );
    assert_eq!(fixture.bytes(), disk);
}
