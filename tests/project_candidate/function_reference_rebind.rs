//! Candidate-bound exact function-reference rebind evidence, authored/unrun.

use semaprax::project::{
    with_authenticated_project, ImageFacet, ProjectCandidate, ProjectSemanticImage, SemanticChange,
    PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_SCHEMA,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-reference-rebind-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn open(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn changed(candidate: &ProjectCandidate) -> ProjectCandidate {
    let intent = json!({
        "kind":"change_function_signature",
        "target":"calculator.add",
        "append_parameters":[{
            "name":"offset",
            "type":"i64",
            "argument":{"kind":"i64","value":0}
        }]
    });
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn exact_base_reference_rebinds_to_the_candidate_image_and_requires_replay() {
    let fixture = Fixture::new();
    let base = fixture.open();
    let base_image = ProjectSemanticImage::derive(
        Arc::clone(base.revision()),
        base.revision().project_revision(),
    )
    .unwrap();
    let reference = base_image
        .export_function_reference(
            base_image.image_digest(),
            "calculator.add",
            Some(ImageFacet::Signature),
        )
        .unwrap();
    let candidate = changed(&base);
    let text = candidate
        .rebind_function_reference(candidate.candidate_digest(), reference.as_bytes())
        .unwrap();
    let report = parse(&text);

    assert_eq!(
        report["schema"],
        PROJECT_CANDIDATE_FUNCTION_REFERENCE_REBIND_SCHEMA
    );
    assert_eq!(report["candidate_revision"], candidate.candidate_digest());
    assert_eq!(report["base_image_revision"], base_image.image_digest());
    assert_eq!(report["rebind"]["accepted"], true);
    assert_eq!(
        report["rebind"]["status"],
        "rebound_to_changed_source_explicit_function"
    );
    assert_eq!(report["rebind"]["normal_destination_replay_required"], true);
    for field in ["source_authority", "execution", "publication_authority"] {
        assert_eq!(report[field], false);
    }

    let candidate_image = ProjectSemanticImage::derive(
        Arc::clone(candidate.revision()),
        candidate.revision().project_revision(),
    )
    .unwrap();
    assert_eq!(report["image_revision"], candidate_image.image_digest());
    let rebound = report["rebind"]["rebound_reference"].as_str().unwrap();
    candidate_image
        .resolve_function_reference(candidate_image.image_digest(), rebound.as_bytes())
        .unwrap();
    assert!(base_image
        .resolve_function_reference(base_image.image_digest(), rebound.as_bytes())
        .is_err());
    assert!(candidate
        .rebind_function_reference(base.candidate_digest(), reference.as_bytes())
        .is_err());
}

#[test]
fn empty_candidate_reports_the_closed_identical_image_rejection() {
    let fixture = Fixture::new();
    let candidate = fixture.open();
    let image = ProjectSemanticImage::derive(
        Arc::clone(candidate.revision()),
        candidate.revision().project_revision(),
    )
    .unwrap();
    let reference = image
        .export_function_reference(image.image_digest(), "calculator.add", None)
        .unwrap();
    let report = parse(
        &candidate
            .rebind_function_reference(candidate.candidate_digest(), reference.as_bytes())
            .unwrap(),
    );
    assert_eq!(report["rebind"]["accepted"], false);
    assert_eq!(report["rebind"]["status"], "rejected");
    assert_eq!(
        report["rebind"]["rejection"]["reason"],
        "source_and_destination_images_are_identical"
    );
    assert!(report["rebind"]["rebound_reference"].is_null());
}

#[test]
fn tampered_reference_fails_before_candidate_wrapper_publication() {
    let fixture = Fixture::new();
    let base = fixture.open();
    let image = ProjectSemanticImage::derive(
        Arc::clone(base.revision()),
        base.revision().project_revision(),
    )
    .unwrap();
    let mut reference = image
        .export_function_reference(image.image_digest(), "calculator.add", None)
        .unwrap()
        .into_bytes();
    reference.push(b' ');
    let candidate = changed(&base);
    let diagnostics = candidate
        .rebind_function_reference(candidate.candidate_digest(), &reference)
        .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G363"));
}

#[test]
fn reference_from_a_different_base_fails_ordinary_source_resolution() {
    let fixture = Fixture::new();
    let base = fixture.open();
    let candidate = changed(&base);

    let other = Fixture::new();
    let core = other.0.join("src/core.spx");
    let source = std::fs::read_to_string(&core).unwrap();
    // Canonical formatting carries no comments, so move the foreign base with
    // an authored body edit that leaves `calculator.add` untouched.
    let foreign_source = source.replace("    value < 0\n", "    value < 1\n");
    assert_ne!(foreign_source, source);
    std::fs::write(&core, &foreign_source).unwrap();
    let foreign = other.open();
    let foreign_image = ProjectSemanticImage::derive(
        Arc::clone(foreign.revision()),
        foreign.revision().project_revision(),
    )
    .unwrap();
    let reference = foreign_image
        .export_function_reference(foreign_image.image_digest(), "calculator.add", None)
        .unwrap();

    let diagnostics = candidate
        .rebind_function_reference(candidate.candidate_digest(), reference.as_bytes())
        .unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G363"));
}
