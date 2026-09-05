//! Exact pathless native-C/header evidence, authored and intentionally unrun.
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
            "spx-image-c-artifacts-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v1"
name = "c-artifacts"
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
@id("api.add") fn add(left:i64,right:i64)->i64 {left+right}
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
            let parsed = semaprax::parse(text, path).unwrap();
            std::fs::write(fixture.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        fixture
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
fn carrier(revision: &ProjectRevision) -> (String, Value, BTreeMap<String, Vec<u8>>) {
    // This API returns a String after complete canonical Project replay.
    let text = revision
        .build_c_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
        .unwrap();
    assert_eq!(
        text,
        revision
            .build_c_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap()
    );
    let value: Value = serde_json::from_str(&text).unwrap();
    let mut files = BTreeMap::new();
    for row in value["artifacts"].as_array().unwrap() {
        let hex = row["hex"].as_str().unwrap();
        assert_eq!(hex.len() % 2, 0);
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(row["sha256"], sha256(&bytes));
        assert_eq!(row["bytes"], bytes.len());
        assert!(files
            .insert(row["path"].as_str().unwrap().to_owned(), bytes)
            .is_none());
    }
    (text, value, files)
}
fn error<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("invalid C artifact query accepted");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}

#[test]
fn cross_file_headers_and_export_bindings_match_the_exact_whole_project_native_projection() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let report = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::C,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap();
    let projection: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(projection["kind"], "c");
    assert_eq!(projection["carrier_schema"], "semaprax.project-c-build.v1");
    let (text, build, files) = carrier(&revision);
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "c-header/src/core.spx.h",
            "c-header/src/core.spx.json",
            "c-header/src/flags.spx.h",
            "c-header/src/flags.spx.json",
            "native/entry.c"
        ]
    );
    assert_eq!(
        projection["carrier_envelope_sha256"],
        sha256(text.as_bytes())
    );
    assert_eq!(build["project_revision"], revision.project_revision());
    let native = std::str::from_utf8(&files["native/entry.c"]).unwrap();
    assert_eq!(
        native,
        semaprax::codegen::emit_hir_c(revision.public_api_program()).unwrap()
    );
    for (id, path) in [("api.add", "src/core.spx"), ("api.flag", "src/flags.spx")] {
        let export = projection["exports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|export| export["id"] == id)
            .unwrap();
        assert_eq!(export["admission"], "admitted");
        assert_eq!(export["native_artifact_path"], "native/entry.c");
        assert_eq!(export["source"]["path"], path);
        let header_path = format!("c-header/{path}.h");
        let envelope_path = format!("c-header/{path}.json");
        assert_eq!(export["header_artifact_path"], header_path);
        assert_eq!(export["header_envelope_path"], envelope_path);
        let envelope = std::str::from_utf8(&files[&envelope_path]).unwrap();
        let header = semaprax::c_header::verify_envelope(envelope).unwrap();
        assert_eq!(header.as_bytes(), files[&header_path]);
        let value: Value = serde_json::from_str(envelope).unwrap();
        assert_eq!(value["schema"], "semaprax.c-header.v1");
        assert_eq!(value["payload"]["source"]["path"], path);
        assert_eq!(value["payload"]["selection"]["admitted"], 1);
        assert_eq!(value["payload"]["selection"]["excluded"], 0);
        let declarations = value["payload"]["functions"].as_array().unwrap();
        assert_eq!(declarations.len(), 1);
        let declaration = &declarations[0];
        assert_eq!(declaration["stable_id"], id);
        assert_eq!(declaration["matches_native"], true);
        assert_eq!(export["symbol"], declaration["symbol"]);
        assert_eq!(export["signature"], declaration["signature"]);
        let signature = declaration["signature"].as_str().unwrap();
        assert_eq!(native.lines().filter(|line| *line == signature).count(), 1);
        assert_eq!(header.lines().filter(|line| *line == signature).count(), 1);
        assert!(signature.contains("spx_status_token"));
        assert!(signature.contains("spx_result_out"));
        assert!(!header.contains("api.hidden"));
    }
    for (source, bound) in revision
        .sources()
        .iter()
        .zip(projection["sources"].as_array().unwrap())
    {
        assert_eq!(bound["path"], source.path());
        assert_eq!(bound["source_digest"], source.source_digest());
        assert_eq!(bound["source_revision"], source.source_revision());
    }
    for row in projection["artifacts"].as_array().unwrap() {
        let file = &files[row["path"].as_str().unwrap()];
        assert_eq!(row["bytes"], file.len());
        assert_eq!(row["sha256"], sha256(file));
        assert!(row.get("hex").is_none());
    }
    for flag in [
        "source_authority",
        "artifact_materialization",
        "target_execution",
    ] {
        assert_eq!(projection[flag], false);
    }
    image
        .verify_artifact_projection(
            image.image_digest(),
            ImageArtifactKind::C,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            report.as_bytes(),
        )
        .unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn signature_change_preserves_export_ids_updates_real_prototypes_and_replays_artifact_delta() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let intent = json!({"kind":"change_function_signature","target":"api.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]});
    let changed = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap();
    let bytes = changed
        .artifact_delta(changed.candidate_digest(), ImageArtifactKind::C)
        .unwrap();
    let delta: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(delta["kind"], "c");
    assert_eq!(delta["comparison"]["artifact_bytes_equal"], false);
    let (_, _, before) = carrier(base.revision());
    let (_, _, after) = carrier(changed.revision());
    assert_ne!(before["native/entry.c"], after["native/entry.c"]);
    assert_eq!(
        before["c-header/src/flags.spx.h"],
        after["c-header/src/flags.spx.h"]
    );
    let old: Value = serde_json::from_slice(&before["c-header/src/core.spx.json"]).unwrap();
    let new: Value = serde_json::from_slice(&after["c-header/src/core.spx.json"]).unwrap();
    let old_signature = old["payload"]["functions"][0]["signature"]
        .as_str()
        .unwrap();
    let new_signature = new["payload"]["functions"][0]["signature"]
        .as_str()
        .unwrap();
    assert_eq!(
        new_signature.matches("int64_t").count(),
        old_signature.matches("int64_t").count() + 1
    );
    assert_eq!(new["payload"]["functions"][0]["stable_id"], "api.add");
    assert_eq!(
        new["payload"]["functions"][0]["symbol"],
        old["payload"]["functions"][0]["symbol"]
    );
    assert!(std::str::from_utf8(&after["native/entry.c"])
        .unwrap()
        .lines()
        .any(|line| line == new_signature));
    for row in delta["files"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let old = &before[path];
        let new = &after[path];
        assert_eq!(row["bytes_equal"], old == new);
        assert_eq!(row["base"]["sha256"], sha256(old));
        assert_eq!(row["candidate"]["sha256"], sha256(new));
    }
    assert_eq!(
        base.revision().manifest().web_exports(),
        changed.revision().manifest().web_exports()
    );
    changed
        .verify_artifact_delta(
            changed.candidate_digest(),
            ImageArtifactKind::C,
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
            .artifact_delta(restored.candidate_digest(), ImageArtifactKind::C)
            .unwrap(),
        bytes
    );
    let mut forged = delta.clone();
    forged["files"][0]["candidate"]["sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    forged.sort_all_objects();
    error(
        changed.verify_artifact_delta(
            changed.candidate_digest(),
            ImageArtifactKind::C,
            format!("{forged}\n").as_bytes(),
        ),
        "SPX-G333",
    );
    error(
        changed.artifact_delta(base.candidate_digest(), ImageArtifactKind::C),
        "SPX-G224",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn excluded_owned_signatures_are_explicit_and_never_gain_public_c_prototypes() {
    let fixture = Fixture::new();
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project");
    let paths = [
        "semaprax.toml",
        "src/app.spx",
        "src/frame.spx",
        "src/tests.spx",
    ];
    for path in paths {
        std::fs::copy(sample.join(path), fixture.0.join(path)).unwrap();
    }
    let before = paths.map(|path| std::fs::read(fixture.0.join(path)).unwrap());
    let revision = fixture.revision();
    let image = image(&revision);
    let (_, _, files) = carrier(&revision);
    assert_eq!(
        std::str::from_utf8(&files["native/entry.c"]).unwrap(),
        semaprax::codegen::emit_hir_c(revision.public_api_program()).unwrap()
    );
    let envelope = std::str::from_utf8(&files["c-header/src/frame.spx.json"]).unwrap();
    let header = semaprax::c_header::verify_envelope(envelope).unwrap();
    let parsed: Value = serde_json::from_str(envelope).unwrap();
    assert_eq!(parsed["payload"]["selection"]["admitted"], 0);
    assert_eq!(parsed["payload"]["selection"]["excluded"], 3);
    assert!(parsed["payload"]["functions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(header.as_bytes(), files["c-header/src/frame.spx.h"]);
    let projection: Value = serde_json::from_str(
        &image
            .artifact_projection(
                image.image_digest(),
                ImageArtifactKind::C,
                MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            )
            .unwrap(),
    )
    .unwrap();
    for export in projection["exports"].as_array().unwrap() {
        assert_eq!(export["admission"], "excluded");
        assert_eq!(export["reason"], "unsupported_parameter_mode");
        for field in ["symbol", "signature", "header_artifact_path"] {
            assert!(export[field].is_null());
        }
        assert_eq!(
            export["header_envelope_path"],
            "c-header/src/frame.spx.json"
        );
        assert!(parsed["payload"]["exclusions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["stable_id"] == export["id"] && row["reason"] == export["reason"]));
        assert_eq!(export["native_artifact_path"], "native/entry.c");
    }
    assert_eq!(
        before,
        paths.map(|path| std::fs::read(fixture.0.join(path)).unwrap())
    );
}

#[test]
fn c_projection_rejects_tampering_stale_images_and_invalid_bounds_without_materialization() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let bytes = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::C,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap();
    error(
        image.verify_artifact_projection(
            image.image_digest(),
            ImageArtifactKind::C,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            format!("{bytes}\n").as_bytes(),
        ),
        "SPX-G293",
    );
    error(
        image.artifact_projection(
            &format!("sha256:{}", "0".repeat(64)),
            ImageArtifactKind::C,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        ),
        "SPX-G221",
    );
    for bound in [1023, MAX_IMAGE_ARTIFACT_BUILD_BYTES + 1] {
        error(
            image.artifact_projection(image.image_digest(), ImageArtifactKind::C, bound),
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
