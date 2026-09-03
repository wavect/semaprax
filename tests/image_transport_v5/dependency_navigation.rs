//! Compact dependency navigation regressions, authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ImageDependencyPageOptions, ImageDependencyView,
    ProjectSemanticImage,
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
            "spx-dependency-navigation-{}-{}",
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
    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
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
fn page_params(summary: &Value, view: &str) -> Value {
    json!({"target":TARGET,"view":view,"handle":handle(summary,view),"page_size":1,"max_bytes":65536})
}

#[test]
fn summary_and_every_view_page_equal_the_library_without_report_chunks() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let image = fixture.image();
    let mut session = fixture.session();
    let expected: Value = serde_json::from_str(
        &image
            .dependency_summary(image.image_digest(), TARGET)
            .unwrap(),
    )
    .unwrap();
    let summary = payload(bound(
        &mut session,
        "image/dependency-summary",
        json!({"target":TARGET}),
    ));
    assert_eq!(summary, expected);
    assert_eq!(summary["source_authority"], false);
    assert!(summary.get("chunk").is_none());
    assert_eq!(summary["facets"].as_array().unwrap().len(), 4);
    for view in ImageDependencyView::ALL {
        let token = handle(&summary, view.name());
        let mut cursor: Option<String> = None;
        let mut count = 0usize;
        loop {
            let mut params = page_params(&summary, view.name());
            if let Some(cursor) = &cursor {
                params["cursor"] = json!(cursor);
            }
            let actual = payload(bound(&mut session, "image/dependency-page", params));
            let expected: Value = serde_json::from_str(
                &image
                    .dependency_page(
                        image.image_digest(),
                        TARGET,
                        view,
                        &token,
                        cursor.as_deref(),
                        ImageDependencyPageOptions::new(1, 65536).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual["offset"], count);
            assert_eq!(actual["source_authority"], false);
            assert!(actual.get("chunk").is_none());
            let items = actual["items"].as_array().unwrap();
            assert!(items.len() <= 1);
            count += items.len();
            match actual["next_cursor"].as_str() {
                Some(next) => {
                    assert!(!items.is_empty());
                    assert_ne!(Some(next), cursor.as_deref());
                    cursor = Some(next.to_owned());
                }
                None => {
                    assert_eq!(actual["total_items"], count);
                    break;
                }
            }
        }
    }
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn readonly_discovery_clients_and_older_profile_isolation_agree() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    for name in ["image/dependency-summary", "image/dependency-page"] {
        let method = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == name)
            .unwrap();
        assert_eq!(method["capability"], "semantic_read");
        assert_eq!(method["query"], true);
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
                name,
                json!({"image_revision":old.image_revision(),"target":TARGET}),
            );
            let response: Value =
                serde_json::from_slice(&old.handle_frame(&request).unwrap()).unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
    assert!(schemas["unbundled_payload_schemas"]
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
        assert!(source.contains("image/dependency-summary"));
        assert!(source.contains("image/dependency-page"));
    }
    assert_eq!(
        bound(&mut session, "candidate/open", json!({}))["error"]["code"],
        -32601
    );
}

#[test]
fn wrong_bound_refs_and_stale_revisions_preserve_sources_and_valid_reads() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session();
    let summary = payload(bound(
        &mut session,
        "image/dependency-summary",
        json!({"target":TARGET}),
    ));
    let valid = page_params(&summary, "callers");
    let first = payload(bound(&mut session, "image/dependency-page", valid.clone()));
    let cursor = first["next_cursor"]
        .as_str()
        .expect("calculator add has multiple reverse callers");
    for (field, value) in [
        ("target", json!("calculator.subtract")),
        ("view", json!("sites")),
        ("handle", json!(format!("sha256:{}", "0".repeat(64)))),
    ] {
        let mut wrong = valid.clone();
        wrong[field] = value;
        assert_eq!(
            bound(&mut session, "image/dependency-page", wrong)["error"]["code"],
            -32000
        );
    }
    for (field, value) in [
        ("page_size", json!(2)),
        ("max_bytes", json!(65537)),
        ("cursor", json!("00:sha256:invalid")),
    ] {
        let mut wrong = valid.clone();
        wrong["cursor"] = json!(cursor);
        wrong[field] = value;
        assert_eq!(
            bound(&mut session, "image/dependency-page", wrong)["error"]["code"],
            -32000
        );
    }
    let mut stale = valid.clone();
    stale["image_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert_eq!(
        call(&mut session, "image/dependency-page", stale)["error"]["code"],
        -32000
    );
    for (field, value) in [
        ("page_size", json!(0)),
        ("max_bytes", json!(1023)),
        ("cursor", Value::Null),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        assert_eq!(
            bound(&mut session, "image/dependency-page", invalid)["error"]["code"],
            -32602
        );
    }
    assert_eq!(
        payload(bound(&mut session, "image/dependency-page", valid)),
        first
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn compact_navigation_batches_equal_sequential_bytes_in_input_order() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let image = fixture.image();
    let mut sequential = fixture.session();
    let mut parallel = fixture.session();
    let summary: Value = serde_json::from_str(
        &image
            .dependency_summary(image.image_digest(), TARGET)
            .unwrap(),
    )
    .unwrap();
    let mut params = page_params(&summary, "callers");
    params["image_revision"] = json!(image.image_digest());
    let requests = [
        frame(
            9,
            "image/dependency-summary",
            json!({"image_revision":image.image_digest(),"target":TARGET}),
        ),
        frame(3, "image/dependency-page", params.clone()),
        frame(
            2,
            "image/dependency-summary",
            json!({"image_revision":image.image_digest(),"target":"calculator.subtract"}),
        ),
        frame(8, "image/dependency-page", params),
    ];
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        let actual = parallel.handle_read_batch(&refs, workers).unwrap();
        assert_eq!(actual, expected);
        for (response, id) in actual.iter().zip([9, 3, 2, 8]) {
            let response: Value = serde_json::from_slice(response.as_ref().unwrap()).unwrap();
            assert_eq!(response["id"], id);
        }
    }
    for name in [
        "image/dependencies",
        "image/dependency-summary",
        "image/dependency-page",
    ] {
        assert!(parallel.parallel_read_methods().contains(&name));
    }
    for name in [
        "candidate/open",
        "candidate/build",
        "candidate/commit",
        "workspace/refresh",
    ] {
        assert!(!parallel.parallel_read_methods().contains(&name));
    }
    parallel.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}
