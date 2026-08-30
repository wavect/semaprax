//! Source-bound projection regressions; authored, not executed locally.
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectSemanticImage,
    IMAGE_ARTIFACT_PROJECTION_SCHEMA, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-image-targets-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(file), root.join(file)).unwrap();
        }
        Self(root)
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
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
#[test]
fn target_membership_uses_real_role_programs_without_runtime_claims() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let report: Value = serde_json::from_str(
        &image
            .target_admission(image.image_digest(), "calculator.add")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(report["target_execution"], false);
    assert_eq!(report["projections"].as_array().unwrap().len(), 4);
    for projection in report["projections"].as_array().unwrap() {
        assert_eq!(projection["scope"], "complete_linked_role_closure");
        if projection["role"] == "entry" {
            assert_eq!(projection["selected_function_in_closure"], true);
        }
    }
    assert_eq!(
        image
            .target_admission(image.image_digest(), "missing")
            .unwrap_err()[0]
            .code,
        "SPX-G290"
    );
}
#[test]
fn web_artifacts_bind_actual_carrier_exports_and_reject_report_mutations() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let original = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let report = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::Web,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap();
    let value: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(value["schema"], IMAGE_ARTIFACT_PROJECTION_SCHEMA);
    assert_eq!(value["artifacts"].as_array().unwrap().len(), 7);
    assert_eq!(value["artifact_materialization"], false);
    let build = image
        .revision()
        .build_web_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
        .unwrap();
    assert_eq!(value["carrier_payload_digest"], build.payload_digest());
    let carrier: Value = serde_json::from_str(build.envelope()).unwrap();
    for (compact, full) in value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .zip(carrier["artifacts"].as_array().unwrap())
    {
        assert_eq!(compact["path"], full["path"]);
        assert_eq!(compact["sha256"], full["sha256"]);
        assert_eq!(compact["bytes"], full["bytes"]);
        assert!(compact.get("content_hex").is_none());
    }
    assert!(value["exports"]
        .as_array()
        .unwrap()
        .iter()
        .any(|export| export["id"] == "calculator.add"));
    image
        .verify_artifact_projection(
            image.image_digest(),
            ImageArtifactKind::Web,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            report.as_bytes(),
        )
        .unwrap();
    let mut mutated = report.into_bytes();
    mutated.push(b'\n');
    assert_eq!(
        image
            .verify_artifact_projection(
                image.image_digest(),
                ImageArtifactKind::Web,
                MAX_IMAGE_ARTIFACT_BUILD_BYTES,
                &mutated
            )
            .unwrap_err()[0]
            .code,
        "SPX-G293"
    );
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        original
    );
    assert_eq!(
        image
            .artifact_projection(
                image.image_digest(),
                ImageArtifactKind::Web,
                MAX_IMAGE_ARTIFACT_BUILD_BYTES + 1
            )
            .unwrap_err()[0]
            .code,
        "SPX-G291"
    );
}

#[test]
fn pathless_candidate_build_requires_a_host_grant() {
    use semaprax::image_transport::{VNextPolicy, VNextSession};
    fn request(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
        params["image_revision"] = serde_json::json!(session.image_revision());
        let frame =
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
        serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
    }
    let fixture = Fixture::new();
    let manifest = fixture.0.join("semaprax.toml").canonicalize().unwrap();
    let mut blocked = VNextSession::open(
        &manifest,
        VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        request(&mut blocked, "candidate/build", serde_json::json!({}))["error"]["code"],
        -32601
    );
    let mut allowed = VNextSession::open(
        &manifest,
        VNextPolicy {
            candidate_prepare: true,
            build_enabled: true,
            ..Default::default()
        },
    )
    .unwrap();
    let opened = request(&mut allowed, "candidate/open", serde_json::json!({}));
    let candidate = opened["result"]["payload"]["candidate_revision"]
        .as_str()
        .unwrap();
    let built = request(
        &mut allowed,
        "candidate/build",
        serde_json::json!({"candidate_revision":candidate,"kind":"web"}),
    );
    assert!(built.get("error").is_none(), "{built}");
    assert_eq!(
        built["result"]["payload"]["report_schema"],
        IMAGE_ARTIFACT_PROJECTION_SCHEMA
    );
    assert_eq!(
        built["result"]["payload"]["artifact_materialization"],
        false
    );
    assert_eq!(built["result"]["payload"]["target_execution"], false);
}
