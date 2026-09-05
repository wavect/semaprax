//! Source-interface and generated-artifact fact bundle regressions.
//! Authored for the workspace harness; not executed during implementation.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, InterfaceArtifactFacts, ProjectSemanticImage,
    INTERFACE_ARTIFACT_FACTS_SCHEMA, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-interface-artifact-facts-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
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
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> std::sync::Arc<semaprax::project::ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
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
fn exact_interface_and_artifact_facts_replay_without_changing_legacy_roots() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let canonical_before = revision.canonical_workspace_revision().unwrap();
    let canonical_bytes = canonical_before.to_json().as_bytes().to_vec();
    let canonical_digest = canonical_before.workspace_revision().to_owned();
    let program_root_before = canonical_before.program_root().unwrap();
    let program_root_bytes = program_root_before.to_json().as_bytes().to_vec();
    let program_root_digest = program_root_before.program_root_digest().to_owned();

    let facts = InterfaceArtifactFacts::derive(
        revision.clone(),
        revision.project_revision(),
        &[ImageArtifactKind::Web],
        MAX_IMAGE_ARTIFACT_BUILD_BYTES,
    )
    .unwrap();
    let descriptor = revision.scalar_wit_interface_v1().unwrap();
    let interface = facts.source_interface().unwrap();
    assert_eq!(interface.kind(), "scalar_wit");
    assert_eq!(interface.schema(), descriptor.schema());
    assert_eq!(interface.canonical_bytes(), descriptor.canonical_bytes());
    assert_eq!(interface.descriptor_digest(), descriptor.digest());

    let image =
        ProjectSemanticImage::derive(revision.clone(), revision.project_revision()).unwrap();
    let expected_report = image
        .artifact_projection(
            image.image_digest(),
            ImageArtifactKind::Web,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap();
    assert_eq!(facts.image_revision(), image.image_digest());
    assert_eq!(facts.artifact_projections().len(), 1);
    assert_eq!(facts.artifact_projections()[0].report(), expected_report);

    let value: Value = serde_json::from_str(facts.to_json()).unwrap();
    assert_eq!(value["schema"], INTERFACE_ARTIFACT_FACTS_SCHEMA);
    assert_eq!(value["project_revision"], revision.project_revision());
    assert_eq!(value["source_authority"], false);
    assert_eq!(value["artifact_materialization"], false);
    assert_eq!(value["target_execution"], false);
    assert!(facts.to_json().ends_with('\n'));

    let replayed = InterfaceArtifactFacts::replay(
        revision.clone(),
        revision.project_revision(),
        &[ImageArtifactKind::Web],
        MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        facts.digest(),
        facts.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed, facts);

    let canonical_after = revision.canonical_workspace_revision().unwrap();
    let program_root_after = canonical_after.program_root().unwrap();
    assert_eq!(canonical_after.workspace_revision(), canonical_digest);
    assert_eq!(canonical_after.to_json().as_bytes(), canonical_bytes);
    assert_eq!(
        program_root_after.program_root_digest(),
        program_root_digest
    );
    assert_eq!(program_root_after.to_json().as_bytes(), program_root_bytes);
}

#[test]
fn stale_malformed_and_noncanonical_fact_inputs_fail_closed() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let kinds = [ImageArtifactKind::Web];
    let facts = InterfaceArtifactFacts::derive(
        revision.clone(),
        revision.project_revision(),
        &kinds,
        MAX_IMAGE_ARTIFACT_BUILD_BYTES,
    )
    .unwrap();

    let mut stale_project = revision.project_revision().as_bytes().to_vec();
    stale_project[7] = if stale_project[7] == b'a' { b'b' } else { b'a' };
    let stale_project = String::from_utf8(stale_project).unwrap();
    assert_eq!(
        InterfaceArtifactFacts::derive(
            revision.clone(),
            &stale_project,
            &kinds,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap_err()[0]
            .code,
        "SPX-G553"
    );
    assert_eq!(
        InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Npm, ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap_err()[0]
            .code,
        "SPX-G552"
    );
    assert_eq!(
        InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web, ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )
        .unwrap_err()[0]
            .code,
        "SPX-G552"
    );

    let mut noncanonical = facts.to_json().as_bytes().to_vec();
    noncanonical.push(b' ');
    assert_eq!(
        InterfaceArtifactFacts::replay(
            revision.clone(),
            revision.project_revision(),
            &kinds,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            facts.digest(),
            &noncanonical,
        )
        .unwrap_err()[0]
            .code,
        "SPX-G552"
    );

    let mut mutated: Value = serde_json::from_str(facts.to_json()).unwrap();
    mutated["artifact_projections"][0]["report"] = Value::String("{}".to_owned());
    let mut reminted = serde_json::to_string(&mutated).unwrap();
    reminted.push('\n');
    assert_eq!(
        InterfaceArtifactFacts::replay(
            revision.clone(),
            revision.project_revision(),
            &kinds,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            facts.digest(),
            reminted.as_bytes(),
        )
        .unwrap_err()[0]
            .code,
        "SPX-G553"
    );

    let uppercase_digest = facts.digest().to_ascii_uppercase();
    assert_eq!(
        InterfaceArtifactFacts::replay(
            revision,
            &stale_project,
            &kinds,
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            &uppercase_digest,
            facts.to_json().as_bytes(),
        )
        .unwrap_err()[0]
            .code,
        "SPX-G552"
    );
}
