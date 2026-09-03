//! Package consumer provenance regressions, authored without execution.
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{McpSession, VNextPolicy, VNextSession};
use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use semaprax::package_semantic_graph::PackageSemanticGraph;
use semaprax::package_source_capsule::{self, PackageSource, SourceCapsuleOptions};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    sources: Vec<PackageSource>,
    input: ResolutionInput,
    resolution_options: ResolutionOptions,
    evidence: String,
    capsule_options: SourceCapsuleOptions,
    capsule: String,
    provider: Coordinate,
}
fn canonical(text: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(text, path).unwrap())
}
/// The provider's exact source.
const PROVIDER_SOURCE: &str =
    "module libmath;\n@id(\"lib.answer\") fn answer()->i64 {41}\n@id(\"lib.unused\") fn unused()->i64 {99}\n";

impl Fixture {
    fn new(version: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-package-graph-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("project/src")).unwrap();
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/calculator-project")
                    .join(path),
                root.join("project").join(path),
            )
            .unwrap();
        }
        let app_interface = canonical(
            "module app.main;\n@id(\"app.main\") fn main()->i64 {0}\n",
            "app-interface.spx",
        );
        let provider_interface = canonical("module libmath;\n@id(\"lib.answer\") fn main()->i64 {0}\n@id(\"lib.unused\") fn unused()->i64 {0}\n", "lib-interface.spx");
        let reports = [app_interface, provider_interface]
            .iter()
            .zip(["app-interface.spx", "lib-interface.spx"])
            .map(|(source, path)| {
                let path = root.join(path);
                std::fs::write(&path, source).unwrap();
                package_report_v2::generate(&path, &PackageReportV2Options::default()).unwrap()
            })
            .collect::<Vec<_>>();
        let app = Coordinate {
            package: "app.main".into(),
            version: "1.0.0".into(),
        };
        let provider = Coordinate {
            package: "libmath".into(),
            version: version.into(),
        };
        let app_subject = package_lock_v2::create_subject(
            &app,
            &reports[0],
            std::slice::from_ref(&provider),
            &[],
        )
        .unwrap();
        let provider_subject =
            package_lock_v2::create_subject(&provider, &reports[1], &[], &[]).unwrap();
        let input = ResolutionInput {
            requirements: vec![Requirement {
                package: "app.main".into(),
                range: "=1.0.0".into(),
            }],
            subjects: vec![provider_subject, app_subject],
            target: "wasm32".into(),
            allowed_capabilities: vec![],
        };
        let resolution_options = ResolutionOptions::default();
        let evidence = package_resolver::generate(&input, &resolution_options).unwrap();
        let sources = vec![
            PackageSource { package:"app.main".into(), report:reports[0].clone(), source:canonical("module app.main;\nuse function @id(\"lib.answer\") from libmath as answer;\nuse function @id(\"lib.unused\") from libmath as unused;\n@id(\"app.main\") fn main()->i64 {answer()+1}\nfn private_helper()->i64 {answer()}\n", "app.spx") },
            PackageSource { package:"libmath".into(), report:reports[1].clone(), source:canonical(PROVIDER_SOURCE, "lib.spx") },
        ];
        let capsule_options = SourceCapsuleOptions::default();
        let capsule = package_source_capsule::generate(
            &sources,
            &evidence,
            &input,
            &resolution_options,
            &capsule_options,
        )
        .unwrap();
        Self {
            root: root.canonicalize().unwrap(),
            sources,
            input,
            resolution_options,
            evidence,
            capsule_options,
            capsule,
            provider,
        }
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.root.join("project/semaprax.toml"),
            VNextPolicy::default(),
        )
        .unwrap()
    }
    fn candidate_session(&self) -> VNextSession {
        VNextSession::open(
            &self.root.join("project/semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn graph(&self) -> PackageSemanticGraph {
        PackageSemanticGraph::derive(
            &self.capsule,
            &self.sources,
            &self.evidence,
            &self.input,
            &self.resolution_options,
            &self.capsule_options,
        )
        .unwrap()
    }
    fn project_bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.root.join("project").join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}
fn frame(id: u64, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let image = session.image_revision().to_owned();
    let mut params = params;
    params["image_revision"] = json!(image);
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}

fn canonical_json(value: Value, domain: &[u8]) -> (String, String) {
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

fn exact_environment_consumer_project(fixture: &Fixture) {
    std::fs::write(
        fixture.root.join("project/semaprax.toml"),
        r#"schema = "semaprax.project.v8"
name = "libmath"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "libmath.app"
sources = ["src/app.spx", "src/lib.spx", "src/tests.spx"]
web_exports = ["lib.answer", "lib.unused"]
tests = ["libmath.tests"]
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("project/src/lib.spx"),
        &fixture.sources[1].source,
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("project/src/app.spx"),
        canonical(
            "module libmath.app;\nuse function @id(\"lib.answer\") from libmath as answer;\n@id(\"libmath.app-main\") fn main()->i64 {answer()}\n",
            "app.spx",
        ),
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("project/src/tests.spx"),
        canonical(
            "module libmath.tests;\nuse function @id(\"lib.answer\") from libmath as answer;\n@id(\"libmath.test-main\") fn main()->i64 {if answer()==41 {0} else {1}}\n",
            "tests.spx",
        ),
    )
    .unwrap();
}

fn analysis_boundary_bundle(candidate: &str, coverage: &Value, root: &Path) -> (String, String) {
    let source = coverage["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["path"] == "src/lib.spx")
        .unwrap();
    let source_bytes = std::fs::read(root.join("project/src/lib.spx")).unwrap();
    let (generated, generated_digest) = canonical_json(
        json!({
            "schema":"semaprax.project-candidate-generated-file-provenance-declaration.v1",
            "candidate_revision":candidate,
            "files":[{
                "artifact":{"path":"src/lib.spx","bytes":source_bytes.len(),"sha256":source["source_digest"]},
                "source":{"path":"src/lib.spx","source_revision":source["source_revision"],"source_digest":source["source_digest"]},
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
    let (external, external_digest) = canonical_json(
        json!({
            "schema":"semaprax.project-candidate-external-api-contract-declaration.v1",
            "candidate_revision":candidate,
            "scope":{"kind":"manifest_exports"},
            "operations":operations
        }),
        b"semaprax.project-candidate-external-api-contract-declaration.v1\0",
    );
    let (deployment, deployment_digest) = canonical_json(
        json!({
            "schema":"semaprax.project-candidate-deployment-contract-declaration.v1",
            "candidate_revision":candidate,
            "manifest_exports":coverage["manifest"]["web_exports"],
            "configuration":[{"key":"SERVICE_MODE","type":"string","required":true}]
        }),
        b"semaprax.project-candidate-deployment-contract-declaration.v1\0",
    );
    canonical_json(
        json!({
            "schema":"semaprax.project-candidate-analysis-boundary-bundle.v1",
            "candidate_revision":candidate,
            "deployment_contract":{"declaration":deployment,"declaration_digest":deployment_digest},
            "generated_file_provenance":{"declaration":generated,"declaration_digest":generated_digest},
            "external_api_contract":{"declaration":external,"declaration_digest":external_digest}
        }),
        b"semaprax.project-candidate-analysis-boundary-bundle.v1\0",
    )
}

#[test]
fn package_graph_requires_exact_replayed_source_selection_and_interface_evidence() {
    let fixture = Fixture::new("1.0.0");
    let graph = fixture.graph();
    let before = graph.to_json().to_owned();
    let mut sources = fixture.sources.clone();
    sources[1].source = sources[1].source.replace("41", "42");
    assert_ne!(sources[1].source, fixture.sources[1].source);
    code(
        PackageSemanticGraph::derive(
            &fixture.capsule,
            &sources,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.capsule_options,
        ),
        "SPX-PS507",
    );
    sources = fixture.sources.clone();
    sources[0].source = canonical(
        "module app.main;\n@id(\"app.main\") fn main()->i64 {42}\n",
        "app.spx",
    );
    code(
        PackageSemanticGraph::derive(
            &fixture.capsule,
            &sources,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.capsule_options,
        ),
        "SPX-PS503",
    );
    sources = fixture.sources.clone();
    sources[0].source = canonical("module app.main;\nuse function @id(\"lib.answer\") from libmath as answer;\nuse function @id(\"lib.unused\") from libmath as unused;\n@id(\"app.main\") fn main()->i64 ensures result == 42 {answer()+1}\n", "app.spx");
    code(
        PackageSemanticGraph::derive(
            &fixture.capsule,
            &sources,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.capsule_options,
        ),
        "SPX-PS503",
    );
    sources = fixture.sources.clone();
    sources[0].report = fixture.sources[1].report.clone();
    code(
        PackageSemanticGraph::derive(
            &fixture.capsule,
            &sources,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.capsule_options,
        ),
        "SPX-PS503",
    );
    let wrong_root = SourceCapsuleOptions {
        root_package: "libmath".into(),
        ..fixture.capsule_options.clone()
    };
    assert!(PackageSemanticGraph::derive(
        &fixture.capsule,
        &fixture.sources,
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &wrong_root
    )
    .is_err());
    assert_eq!(graph.to_json(), before);
}

#[test]
fn actual_consumers_and_uncalled_imports_keep_exact_source_and_interface_provenance_separate() {
    let fixture = Fixture::new("1.0.0");
    let graph = fixture.graph();
    let again = fixture.graph();
    assert_eq!(graph.to_json(), again.to_json());
    assert_eq!(graph.graph_digest(), again.graph_digest());
    let summary: Value =
        serde_json::from_str(&graph.summary(graph.graph_digest()).unwrap()).unwrap();
    assert_eq!(summary["schema"], "semaprax.package-semantic-summary.v1");
    assert_eq!(summary["graph_revision"], graph.graph_digest());
    assert_eq!(summary["project_association"], "none");
    assert_eq!(
        summary["counts"],
        json!({"packages":2,"interface_functions":3,"imports":2,"cross_package_calls":2})
    );
    for flag in ["source_authority", "execution", "publication_authority"] {
        assert_eq!(summary[flag], false);
    }
    let called: Value = serde_json::from_str(
        &graph
            .consumers(graph.graph_digest(), &fixture.provider, "lib.answer")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(called["schema"], "semaprax.package-semantic-consumers.v1");
    assert_eq!(called["graph_revision"], graph.graph_digest());
    assert_eq!(called["project_association"], "none");
    assert_eq!(
        called["imports"],
        json!([{"dependent":{"package":"app.main","version":"1.0.0"},"dependency":{"package":"libmath","version":"1.0.0"},"target":"lib.answer","alias":"answer","ordinal":0}])
    );
    let calls = called["calls"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    let program = semaprax::parse(&fixture.sources[0].source, "app.spx").unwrap();
    let private = program
        .functions
        .iter()
        .find(|function| function.name == "private_helper")
        .unwrap();
    assert!(!private.explicit_id);
    for caller in ["app.main", private.stable_id.as_str()] {
        assert_eq!(
            calls.iter().filter(|row| row["caller"] == caller).count(),
            1
        );
    }
    for row in calls {
        assert_eq!(
            row["caller_package"],
            json!({"package":"app.main","version":"1.0.0"})
        );
        assert_eq!(
            row["target_package"],
            json!({"package":"libmath","version":"1.0.0"})
        );
        assert_eq!(row["target"], "lib.answer");
        assert_eq!(row["alias"], "answer");
    }
    let capsule: Value = serde_json::from_str(&fixture.capsule).unwrap();
    assert!(!capsule["payload"]["linked_functions"]
        .as_array()
        .unwrap()
        .contains(&json!(private.stable_id)));
    let receipt = package_source_capsule::verify(
        &fixture.capsule,
        &fixture.sources,
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.capsule_options,
    )
    .unwrap();
    assert_eq!(summary["source_capsule_digest"], receipt.digest());
    assert_eq!(summary["source_set_digest"], receipt.source_set_digest());
    assert_eq!(summary["link_digest"], receipt.link_digest());
    assert_eq!(
        summary["root_package"],
        json!({"package":"app.main","version":"1.0.0"})
    );
    for (coordinate, revision) in receipt.source_revisions() {
        let package = summary["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["coordinate"]["package"] == coordinate.package)
            .unwrap();
        assert_eq!(package["coordinate"]["version"], coordinate.version);
        assert_eq!(package["source_revision"], revision.as_str());
        assert!(!package["exports"]
            .as_array()
            .unwrap()
            .contains(&json!(private.stable_id)));
        assert_ne!(
            package["interface_source_revision"],
            package["source_revision"]
        );
        assert!(package["exports"]
            .as_array()
            .unwrap()
            .contains(&json!(if coordinate.package == "app.main" {
                "app.main"
            } else {
                "lib.answer"
            })));
        let key = if coordinate.package == "app.main" {
            "caller_source_revision"
        } else {
            "target_source_revision"
        };
        for row in calls {
            assert_eq!(row[key], revision.as_str());
        }
    }
    let uncalled: Value = serde_json::from_str(
        &graph
            .consumers(graph.graph_digest(), &fixture.provider, "lib.unused")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        uncalled["imports"],
        json!([{"dependent":{"package":"app.main","version":"1.0.0"},"dependency":{"package":"libmath","version":"1.0.0"},"target":"lib.unused","alias":"unused","ordinal":1}])
    );
    assert_eq!(uncalled["calls"], json!([]));
    assert_eq!(uncalled["project_association"], "none");
    for flag in ["source_authority", "execution", "publication_authority"] {
        assert_eq!(uncalled[flag], false);
    }
}

#[test]
fn query_selectors_are_graph_and_coordinate_scoped_even_when_stable_ids_match() {
    let first = Fixture::new("1.0.0");
    let second = Fixture::new("2.0.0");
    let first_graph = first.graph();
    let second_graph = second.graph();
    assert_ne!(first_graph.graph_digest(), second_graph.graph_digest());
    assert!(first_graph
        .consumers(first_graph.graph_digest(), &first.provider, "lib.answer")
        .is_ok());
    assert!(second_graph
        .consumers(second_graph.graph_digest(), &second.provider, "lib.answer")
        .is_ok());
    code(
        second_graph.consumers(second_graph.graph_digest(), &first.provider, "lib.answer"),
        "SPX-PS602",
    );
    code(
        second_graph.summary(first_graph.graph_digest()),
        "SPX-PS602",
    );
    code(
        first_graph.consumers(
            first_graph.graph_digest(),
            &first.provider,
            "missing.export",
        ),
        "SPX-PS602",
    );
    code(first_graph.summary("not-a-digest"), "SPX-PS601");
    code(first_graph.summary(&"x".repeat(72)), "SPX-PS603");
    code(
        first_graph.consumers(first_graph.graph_digest(), &first.provider, ""),
        "SPX-PS601",
    );
    code(
        first_graph.consumers(
            first_graph.graph_digest(),
            &first.provider,
            &"x".repeat(4097),
        ),
        "SPX-PS603",
    );
    let before = first_graph.to_json().to_owned();
    let summary = first_graph.summary(first_graph.graph_digest()).unwrap();
    std::fs::remove_file(first.root.join("app-interface.spx")).unwrap();
    std::fs::remove_file(first.root.join("lib-interface.spx")).unwrap();
    assert_eq!(
        first_graph.summary(first_graph.graph_digest()).unwrap(),
        summary
    );
    assert_eq!(first_graph.to_json(), before);
    assert!(!first.root.join("app-interface.spx").exists());
    assert!(!first.root.join("lib-interface.spx").exists());
}

#[test]
fn startup_attached_package_queries_remain_independent_of_project_and_match_read_batches() {
    let fixture = Fixture::new("1.0.0");
    let disk = fixture.project_bytes();
    let graph = Arc::new(fixture.graph());
    let mut absent = fixture.session();
    let image = absent.image_revision().to_owned();
    let schemas = payload(call(&mut absent, "protocol/schemas", json!({})));
    for schema in [
        "urn:semaprax.package-semantic-summary.v1",
        "urn:semaprax.package-semantic-consumers.v1",
    ] {
        assert!(!schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|document| document["$id"] == schema));
    }
    for method in ["package/summary", "package/consumers"] {
        assert!(!schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["method"] == method));
        assert!(!absent.parallel_read_methods().contains(&method));
        assert_eq!(
            call(&mut absent, method, json!({"image_revision":image}))["error"]["code"],
            -32601
        );
    }
    let mut sequential = fixture.session();
    let mut parallel = fixture.session();
    for session in [&mut sequential, &mut parallel] {
        session
            .attach_package_graph(Arc::clone(&graph), graph.graph_digest())
            .unwrap();
        assert_eq!(session.image_revision(), image);
        assert!(session.parallel_read_methods().contains(&"package/summary"));
        assert!(session
            .parallel_read_methods()
            .contains(&"package/consumers"));
    }
    let summary = payload(call(
        &mut sequential,
        "package/summary",
        json!({"image_revision":image}),
    ));
    assert_eq!(
        summary,
        serde_json::from_str::<Value>(&graph.summary(graph.graph_digest()).unwrap()).unwrap()
    );
    assert_eq!(summary["graph_revision"], graph.graph_digest());
    assert_eq!(summary["project_association"], "none");
    let requests = [
        frame(9, "package/summary", json!({"image_revision":image})),
        frame(
            2,
            "package/consumers",
            json!({"image_revision":image,"package_revision":graph.graph_digest(),"provider_package":"libmath","provider_version":"1.0.0","target":"lib.answer"}),
        ),
        frame(
            8,
            "package/consumers",
            json!({"image_revision":image,"package_revision":graph.graph_digest(),"provider_package":"libmath","provider_version":"1.0.0","target":"lib.unused"}),
        ),
        frame(
            4,
            "package/consumers",
            json!({"image_revision":image,"package_revision":graph.graph_digest(),"provider_package":"libmath","provider_version":"2.0.0","target":"lib.answer"}),
        ),
        frame(
            3,
            "package/summary",
            json!({"image_revision":image,"package_revision":graph.graph_digest()}),
        ),
    ];
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    for index in [3, 4] {
        let response: Value = serde_json::from_slice(expected[index].as_ref().unwrap()).unwrap();
        assert!(response.get("error").is_some());
    }
    let called: Value = serde_json::from_slice(expected[1].as_ref().unwrap()).unwrap();
    assert_eq!(
        payload(called),
        serde_json::from_str::<Value>(
            &graph
                .consumers(graph.graph_digest(), &fixture.provider, "lib.answer")
                .unwrap()
        )
        .unwrap()
    );
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            parallel.handle_read_batch(&refs, workers).unwrap(),
            expected
        );
    }
    for method in [
        "candidate/open",
        "candidate/build",
        "candidate/test",
        "candidate/commit",
    ] {
        assert_eq!(
            call(&mut parallel, method, json!({"image_revision":image}))["error"]["code"],
            -32601
        );
    }
    let schemas = payload(call(&mut parallel, "protocol/schemas", json!({})));
    for method in ["package/summary", "package/consumers"] {
        let descriptor = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method)
            .unwrap();
        assert_eq!(descriptor["query"], true);
        let params = &descriptor["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        if method == "package/summary" {
            assert_eq!(params["required"], json!(["image_revision"]));
        }
    }
    for schema in [
        "urn:semaprax.package-semantic-summary.v1",
        "urn:semaprax.package-semantic-consumers.v1",
    ] {
        let doc = schemas["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|doc| doc["$id"] == schema)
            .unwrap();
        assert_eq!(doc["additionalProperties"], false);
        assert_eq!(doc["properties"]["project_association"]["const"], "none");
        for flag in ["source_authority", "execution", "publication_authority"] {
            assert_eq!(doc["properties"][flag]["const"], false);
        }
    }
    parallel.finish().unwrap();
    assert_eq!(fixture.project_bytes(), disk);
}

#[test]
fn attachment_is_digest_bound_startup_only_and_cannot_replace_the_selected_subject() {
    let fixture = Fixture::new("1.0.0");
    let graph = Arc::new(fixture.graph());
    let mut session = fixture.session();
    let wrong = format!("sha256:{}", "0".repeat(64));
    code(
        session.attach_package_graph(Arc::clone(&graph), &wrong),
        "SPX-PS602",
    );
    session
        .attach_package_graph(Arc::clone(&graph), graph.graph_digest())
        .unwrap();
    code(
        session.attach_package_graph(Arc::clone(&graph), graph.graph_digest()),
        "SPX-G280",
    );
    for mode in ["frame", "empty_frame", "invalid_batch"] {
        let mut late = fixture.session();
        match mode {
            "frame" => {
                call(&mut late, "protocol/capabilities", json!({}));
            }
            "empty_frame" => {
                late.handle_frame(b"");
            }
            "invalid_batch" => {
                assert!(late.handle_read_batch(&[], 0).is_err());
            }
            _ => unreachable!(),
        }
        code(
            late.attach_package_graph(Arc::clone(&graph), graph.graph_digest()),
            "SPX-G280",
        );
    }
    let before = graph.summary(graph.graph_digest()).unwrap();
    let image = session.image_revision().to_owned();
    payload(call(
        &mut session,
        "package/summary",
        json!({"image_revision":image}),
    ));
    let path = fixture.root.join("project/src/app.spx");
    let changed = std::fs::read_to_string(&path).unwrap() + "\n// unrelated Project drift\n";
    std::fs::write(&path, &changed).unwrap();
    assert!(call(
        &mut session,
        "package/summary",
        json!({"image_revision":image})
    )
    .get("error")
    .is_some());
    assert_eq!(graph.summary(graph.graph_digest()).unwrap(), before);
    assert_eq!(std::fs::read_to_string(path).unwrap(), changed);
}

#[test]
fn environment_consumer_review_requires_both_host_selections_and_is_closed_but_not_parallel() {
    const METHOD: &str = "candidate/environment-consumer-review";
    const CHUNK: &str = "urn:semaprax.image-candidate-environment-consumer-review-chunk.v1";
    let fixture = Fixture::new("1.0.0");
    exact_environment_consumer_project(&fixture);
    let graph = Arc::new(fixture.graph());

    let mut candidate_only = fixture.candidate_session();
    assert!(!payload(bound(
        &mut candidate_only,
        "protocol/capabilities",
        json!({})
    ))["methods"]
        .as_array()
        .unwrap()
        .contains(&json!(METHOD)));
    assert!(
        !payload(bound(&mut candidate_only, "protocol/schemas", json!({})))["documents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|document| document["$id"] == CHUNK)
    );
    candidate_only.finish().unwrap();

    let mut graph_only = fixture.session();
    graph_only
        .attach_package_graph(Arc::clone(&graph), graph.graph_digest())
        .unwrap();
    assert!(
        !payload(bound(&mut graph_only, "protocol/capabilities", json!({})))["methods"]
            .as_array()
            .unwrap()
            .contains(&json!(METHOD))
    );
    assert!(
        !payload(bound(&mut graph_only, "protocol/schemas", json!({})))["documents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|document| document["$id"] == CHUNK)
    );
    graph_only.finish().unwrap();

    let mut selected = fixture.candidate_session();
    selected
        .attach_package_graph(Arc::clone(&graph), graph.graph_digest())
        .unwrap();
    assert!(!selected.parallel_read_methods().contains(&METHOD));
    let schemas = payload(bound(&mut selected, "protocol/schemas", json!({})));
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
    for required in [
        "image_revision",
        "candidate_revision",
        "package_revision",
        "bundle",
        "bundle_digest",
        "provider_package",
        "provider_version",
        "provider_source_path",
        "target",
    ] {
        assert!(params["required"]
            .as_array()
            .unwrap()
            .contains(&json!(required)));
    }
    assert_eq!(params["properties"]["offset"]["maximum"], 23_265_280);
    let chunk = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == CHUNK)
        .unwrap();
    assert_eq!(chunk["additionalProperties"], false);
    assert_eq!(
        chunk["properties"]["compatibility"]["const"],
        "not_assessed"
    );
    for field in [
        "source_authority",
        "filesystem_authority",
        "network_observation",
        "installed_consumer_observation",
        "consumer_discovery_complete",
        "package_acquisition_authority",
        "runtime_observation",
        "publication_authority",
        "deployment_authority",
    ] {
        assert_eq!(chunk["properties"][field]["const"], false);
    }
    for language in ["typescript", "python", "rust"] {
        let client = payload(bound(
            &mut selected,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        assert!(source.contains("request_candidate_environment_consumer_review"));
        assert!(source.contains("decode_request_candidate_environment_consumer_review"));
    }
    let image = selected.image_revision().to_owned();
    let candidate = payload(bound(&mut selected, "candidate/open", json!({})))
        ["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let coverage = payload(bound(
        &mut selected,
        "candidate/analysis-coverage",
        json!({"candidate_revision":candidate}),
    ));
    let (bundle, bundle_digest) = analysis_boundary_bundle(&candidate, &coverage, &fixture.root);
    let request = json!({
        "image_revision":image,
        "candidate_revision":candidate,
        "package_revision":graph.graph_digest(),
        "bundle":bundle,
        "bundle_digest":bundle_digest,
        "provider_package":"libmath",
        "provider_version":"1.0.0",
        "provider_source_path":"src/lib.spx",
        "target":"lib.answer",
        "chunk_bytes":1024
    });
    let mut stale = request.clone();
    stale["package_revision"] =
        json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(
        call(&mut selected, METHOD, stale)["error"]["data"]["diagnostics"][0]["code"],
        "SPX-G475"
    );
    let mut offset = 0;
    let mut chunks = 0;
    let mut report_bytes = String::new();
    let mut report_sha = None;
    loop {
        let mut params = request.clone();
        params["offset"] = json!(offset);
        let chunk = payload(call(&mut selected, METHOD, params));
        assert_eq!(chunk["schema"], CHUNK.trim_start_matches("urn:"));
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["package_revision"], graph.graph_digest());
        assert_eq!(chunk["bundle_digest"], bundle_digest);
        assert_eq!(chunk["compatibility"], "not_assessed");
        let sha = chunk["report_sha256"].as_str().unwrap().to_owned();
        assert!(report_sha.as_ref().is_none_or(|expected| expected == &sha));
        report_sha = Some(sha);
        report_bytes.push_str(chunk["chunk"].as_str().unwrap());
        chunks += 1;
        match chunk["next_offset"].as_u64() {
            Some(next) => offset = next as usize,
            None => break,
        }
    }
    assert!(chunks >= 2);
    assert_eq!(
        report_sha.unwrap(),
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(report_bytes.as_bytes()))
        )
    );
    let report: Value = serde_json::from_str(&report_bytes).unwrap();
    assert_eq!(report["candidate_revision"], candidate);
    assert_eq!(report["package_graph_revision"], graph.graph_digest());
    assert_eq!(report["provider"]["package"], "libmath");
    assert_eq!(report["provider_source"]["path"], "src/lib.spx");
    assert_eq!(report["target"], "lib.answer");
    assert!(report["counts"]["imports"].as_u64().unwrap() > 0);
    assert!(report["counts"]["calls"].as_u64().unwrap() > 0);
    assert_eq!(report["compatibility"], "not_assessed");
    selected.finish().unwrap();

    let mut mcp_host = fixture.candidate_session();
    mcp_host
        .attach_package_graph(Arc::clone(&graph), graph.graph_digest())
        .unwrap();
    let mut mcp = McpSession::new(mcp_host).unwrap();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},
        "clientInfo":{"name":"environment-consumers","version":"1"}}});
    mcp.handle_frame(initialize.to_string().as_bytes()).unwrap();
    mcp.handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    let mut names = Vec::new();
    let mut review_input = None;
    let mut cursor = None;
    loop {
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":
            cursor.as_ref().map_or_else(||json!({}),|value|json!({"cursor":value}))});
        let page: Value =
            serde_json::from_slice(&mcp.handle_frame(request.to_string().as_bytes()).unwrap())
                .unwrap();
        for tool in page["result"]["tools"].as_array().unwrap() {
            if tool["name"] == "candidate__environment-consumer-review" {
                review_input = Some(tool["inputSchema"].clone());
            }
            names.push(tool["name"].as_str().unwrap().to_owned());
        }
        cursor = page["result"]["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(names.contains(&"candidate__environment-consumer-review".to_owned()));
    let review_input = review_input.unwrap();
    assert_eq!(review_input["additionalProperties"], false);
    assert!(review_input["required"]
        .as_array()
        .unwrap()
        .contains(&json!("bundle_digest")));
    let opened = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"candidate__open","arguments":{"image_revision":image}}});
    let opened: Value =
        serde_json::from_slice(&mcp.handle_frame(opened.to_string().as_bytes()).unwrap()).unwrap();
    let opened: Value =
        serde_json::from_str(opened["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(opened["result"]["payload"]["candidate_revision"], candidate);
    let invoked = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"candidate__environment-consumer-review","arguments":request}});
    let invoked: Value =
        serde_json::from_slice(&mcp.handle_frame(invoked.to_string().as_bytes()).unwrap()).unwrap();
    let inner: Value =
        serde_json::from_str(invoked["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        inner["result"]["payload"]["schema"],
        CHUNK.trim_start_matches("urn:")
    );
    assert_eq!(inner["result"]["payload"]["candidate_revision"], candidate);
    mcp.finish().unwrap();
}
