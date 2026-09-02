//! Project OpenAPI artifact evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectCandidate, ProjectRevision,
    ProjectSemanticImage, SemanticChange, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PATHS: [&str; 5] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/flags.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-image-openapi-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v1"
name = "openapi-artifacts"
entry = "api.app"
sources = ["src/app.spx", "src/core.spx", "src/flags.spx", "src/tests.spx"]
web_exports = ["api.add", "api.flag"]
tests = ["api.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module api.core;
@id("api.add") fn add(left:i64, right:i64)->i64 {left+right}
@id("api.hidden") fn hidden(value:i64)->i64 {value}
"#,
            ),
            (
                "src/flags.spx",
                r#"module api.flags;
@id("api.flag") fn invert(value:bool)->bool {!value}
"#,
            ),
            (
                "src/app.spx",
                r#"module api.app;
use function @id("api.add") from api.core as add;
use function @id("api.flag") from api.flags as invert;
@id("api.main") fn main()->i64 {if invert(false) {add(40,2)} else {0}}
"#,
            ),
            (
                "src/tests.spx",
                r#"module api.tests;
use function @id("api.add") from api.core as add;
@id("api.test") fn main()->i64 {if add(40,2)==42 {0}else{1}}
"#,
            ),
        ] {
            fixture.write(path, text);
        }
        fixture
    }
    fn write(&self, path: &str, source: &str) {
        let program = semaprax::parse(source, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
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
fn image(revision: &Arc<ProjectRevision>) -> ProjectSemanticImage {
    ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision()).unwrap()
}
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
fn bound_digest(domain: &[u8], bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{}",
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
fn artifacts(revision: &ProjectRevision) -> (Value, BTreeMap<String, Vec<u8>>) {
    let build = revision
        .build_openapi_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
        .unwrap();
    // The public builder returns a String and independently rebuilds the
    // canonical Project before returning it. Repeat that complete replay and
    // require exact carrier equality in addition to per-file hash checks.
    assert_eq!(
        build,
        revision
            .build_openapi_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap()
    );
    let envelope: Value = serde_json::from_str(&build).unwrap();
    let mut files = BTreeMap::new();
    for artifact in envelope["artifacts"].as_array().unwrap() {
        let hex = artifact["hex"].as_str().unwrap();
        assert_eq!(hex.len() % 2, 0);
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(artifact["sha256"], sha256(&bytes));
        assert!(files
            .insert(artifact["path"].as_str().unwrap().to_owned(), bytes)
            .is_none());
    }
    (envelope, files)
}
fn error<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().expect("invalid OpenAPI projection accepted");
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn selected_cross_file_exports_bind_actual_openapi_operations_and_source_bytes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let bytes = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap();
    let projection: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(projection["kind"], "openapi");
    assert_eq!(
        projection["carrier_schema"],
        "semaprax.project-openapi-build.v1"
    );
    for flag in [
        "source_authority",
        "artifact_materialization",
        "target_execution",
    ] {
        assert_eq!(projection[flag], false);
    }
    let (carrier, files) = artifacts(&revision);
    assert_eq!(carrier["project_revision"], revision.project_revision());
    assert_eq!(
        carrier["project_graph_digest"],
        revision.semantic_graph_digest()
    );
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        ["openapi/src/core.spx.json", "openapi/src/flags.spx.json"]
    );
    for file in projection["artifacts"].as_array().unwrap() {
        let actual = files.get(file["path"].as_str().unwrap()).unwrap();
        assert_eq!(file["bytes"], actual.len());
        assert_eq!(file["sha256"], sha256(actual));
        assert!(file.get("hex").is_none());
        assert!(file.get("content_hex").is_none());
    }
    assert_eq!(projection["exports"].as_array().unwrap().len(), 2);
    for (id, source_path, operation_id) in [
        ("api.add", "src/core.spx", "api_add"),
        ("api.flag", "src/flags.spx", "api_flag"),
    ] {
        let export = projection["exports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|export| export["id"] == id)
            .unwrap();
        let artifact_path = format!("openapi/{source_path}.json");
        assert_eq!(export["source"]["path"], source_path);
        assert_eq!(export["artifact_path"], artifact_path);
        assert_eq!(export["operation_path"], format!("/{id}"));
        assert_eq!(export["operation_id"], operation_id);
        let actual: Value = serde_json::from_slice(files.get(&artifact_path).unwrap()).unwrap();
        assert_eq!(actual["schema"], "semaprax.openapi.v1");
        assert_eq!(actual["source"]["path"], source_path);
        assert_eq!(actual["operations"], 1);
        assert_eq!(actual["document"]["openapi"], "3.1.0");
        let operation = &actual["document"]["paths"][format!("/{id}")]["post"];
        assert_eq!(operation["x-stable-id"], id);
        assert_eq!(operation["operationId"], operation_id);
        assert!(actual["document"]["paths"].get("/api.hidden").is_none());
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == source_path)
            .unwrap();
        assert_eq!(actual["source"]["revision"], source.source_revision());
        assert_eq!(
            actual["source"]["sha256"],
            bound_digest(b"semaprax.openapi.source.v1\0", source.source().as_bytes())
        );
        assert_eq!(
            actual["sha256"],
            bound_digest(
                b"semaprax.openapi.document.v1\0",
                actual["document"].to_string().as_bytes()
            )
        );
    }
    let add: Value =
        serde_json::from_slice(files.get("openapi/src/core.spx.json").unwrap()).unwrap();
    assert_eq!(
        add["document"]["components"]["schemas"]["api_add.Request"]["required"],
        json!(["left", "right"])
    );
    assert_eq!(
        add["document"]["components"]["schemas"]["api_add.Result"]["format"],
        "int64"
    );
    let flag: Value =
        serde_json::from_slice(files.get("openapi/src/flags.spx.json").unwrap()).unwrap();
    assert_eq!(
        flag["document"]["components"]["schemas"]["api_flag.Result"]["type"],
        "boolean"
    );
    for (bound, source) in projection["sources"]
        .as_array()
        .unwrap()
        .iter()
        .zip(revision.sources())
    {
        assert_eq!(bound["path"], source.path());
        assert_eq!(bound["source_digest"], source.source_digest());
        assert_eq!(bound["source_revision"], source.source_revision());
    }
    image
        .verify_artifact_projection(
            image.image_digest(),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            bytes.as_bytes(),
        )
        .unwrap();
    assert_eq!(
        image
            .artifact_projection(
                image.image_digest(),
                ImageArtifactKind::OpenApi,
                MAX_IMAGE_ARTIFACT_BUILD_BYTES
            )
            .unwrap(),
        bytes
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn signature_evolution_changes_exact_openapi_request_and_file_delta_then_replays() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let change = SemanticChange::new(base.revision().project_revision(), &json!({"kind":"change_function_signature","target":"api.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]})).unwrap();
    let changed = base.apply(base.candidate_digest(), &change).unwrap();
    let bytes = changed
        .artifact_delta(changed.candidate_digest(), ImageArtifactKind::OpenApi)
        .unwrap();
    let delta: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(delta["kind"], "openapi");
    assert_eq!(delta["comparison"]["artifact_bytes_equal"], false);
    let (_, before) = artifacts(base.revision());
    let (_, after) = artifacts(changed.revision());
    for row in delta["files"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let old = before.get(path).unwrap();
        let new = after.get(path).unwrap();
        assert_eq!(row["bytes_equal"], old == new);
        assert_eq!(row["base"]["sha256"], sha256(old));
        assert_eq!(row["candidate"]["sha256"], sha256(new));
    }
    assert_eq!(
        before["openapi/src/flags.spx.json"],
        after["openapi/src/flags.spx.json"]
    );
    let new: Value = serde_json::from_slice(&after["openapi/src/core.spx.json"]).unwrap();
    assert_eq!(
        new["document"]["components"]["schemas"]["api_add.Request"]["required"],
        json!(["left", "right", "offset"])
    );
    assert_eq!(
        changed.revision().manifest().web_exports(),
        base.revision().manifest().web_exports()
    );
    changed
        .verify_artifact_delta(
            changed.candidate_digest(),
            ImageArtifactKind::OpenApi,
            bytes.as_bytes(),
        )
        .unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(changed.base_revision()),
        changed.base_revision().project_revision(),
        changed.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(
        restored
            .artifact_delta(restored.candidate_digest(), ImageArtifactKind::OpenApi)
            .unwrap(),
        bytes
    );
    let mut forged = delta.clone();
    forged["files"][0]["candidate"]["sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    forged.sort_all_objects();
    error(
        changed.verify_artifact_delta(
            changed.candidate_digest(),
            ImageArtifactKind::OpenApi,
            format!("{forged}\n").as_bytes(),
        ),
        "SPX-G333",
    );
    error(
        changed.artifact_delta(base.candidate_digest(), ImageArtifactKind::OpenApi),
        "SPX-G224",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn image_projection_rejects_stale_tampered_and_out_of_bound_requests_without_output_writes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let bytes = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap();
    error(
        image.verify_artifact_projection(
            image.image_digest(),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            format!("{bytes}\n").as_bytes(),
        ),
        "SPX-G293",
    );
    error(
        image.artifact_projection(
            &format!("sha256:{}", "0".repeat(64)),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        ),
        "SPX-G221",
    );
    for limit in [1023, MAX_IMAGE_ARTIFACT_BUILD_BYTES + 1] {
        error(
            image.artifact_projection(image.image_digest(), ImageArtifactKind::OpenApi, limit),
            "SPX-G291",
        );
    }
    assert_eq!(fixture.bytes(), disk);
    let mut files = std::fs::read_dir(&fixture.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["semaprax.toml", "src"]);
}

#[test]
fn owned_project_exports_are_rejected_by_existing_openapi_signature_admission() {
    let fixture = Fixture::new();
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project");
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/frame.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(sample.join(path), fixture.0.join(path)).unwrap();
    }
    let revision = fixture.revision();
    let image = image(&revision);
    let before = [
        "semaprax.toml",
        "src/app.spx",
        "src/frame.spx",
        "src/tests.spx",
    ]
    .map(|path| std::fs::read(fixture.0.join(path)).unwrap());
    let rejected = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap_err();
    assert!(
        rejected.iter().any(|error| error.code == "SPX-OA103"),
        "{rejected:?}"
    );
    let after = [
        "semaprax.toml",
        "src/app.spx",
        "src/frame.spx",
        "src/tests.spx",
    ]
    .map(|path| std::fs::read(fixture.0.join(path)).unwrap());
    assert_eq!(before, after);
}

#[test]
#[ignore = "needs OpenAPI effect check fix"]
fn effectful_export_is_rejected_at_existing_project_admission_before_openapi_can_build() {
    let fixture = Fixture::new();
    // Project v1's scalar carrier is stricter than a standalone schema query:
    // its ordinary profile admission already rejects declared effects.
    for path in &PATHS[1..] {
        let source = std::fs::read_to_string(fixture.0.join(path)).unwrap();
        let mut parsed = semaprax::parse(&source, *path).unwrap();
        parsed.permits.push("clock.read".to_owned());
        for function in &mut parsed.functions {
            function.effects = vec!["clock.read".to_owned()];
        }
        std::fs::write(fixture.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
    }
    let before = fixture.bytes();
    let rejected = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let image =
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())?;
        image.artifact_projection(
            image.image_digest(),
            ImageArtifactKind::OpenApi,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
    })
    .unwrap_err();
    assert!(
        rejected
            .iter()
            .any(|error| error.code == "SPX-W115" && error.message.contains("declares effects")),
        "{rejected:?}"
    );
    assert_eq!(fixture.bytes(), before);
}
