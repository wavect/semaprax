//! Exact-revision function-reference evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageFacet, ProjectSemanticImage,
    IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA, IMAGE_FUNCTION_REFERENCE_SCHEMA,
    MAX_IMAGE_FUNCTION_REFERENCE_BYTES, MAX_IMAGE_FUNCTION_REFERENCE_RESOLUTION_BYTES,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
const REFERENCE_NONCLAIMS: [&str; 5] = [
    "integrity_and_staleness_binding_not_capability_or_secret",
    "exact_revision_only_no_automatic_migration",
    "no_hir_graph_source_or_handle_facts_trusted_from_reference",
    "no_source_execution_candidate_retention_or_publication_authority",
    "no_persistent_server_state_or_general_session_recovery",
];
const RESOLUTION_NONCLAIMS: [&str; 5] = [
    "resolved_only_against_exact_current_image_and_source_provenance",
    "function_summary_and_facet_handle_freshly_derived_not_trusted_from_reference",
    "no_cursor_persistence_or_automatic_migration",
    "no_source_execution_candidate_retention_or_publication_authority",
    "no_ranking_or_general_session_recovery",
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-function-reference-{}-{}",
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
    fn change_add_body(&self) {
        let path = self.0.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap();
        let changed = source.replacen("left + right", "left + right + 0", 1);
        assert_ne!(changed, source);
        let parsed = semaprax::parse(&changed, "src/core.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn parse(text: &str) -> Value {
    assert!(!text.ends_with('\n'));
    serde_json::from_str(text).unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("invalid function reference accepted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected),
        "{diagnostics:?}"
    );
}
fn digest(payload: &Value) -> String {
    let mut unsigned = payload.as_object().unwrap().clone();
    unsigned.remove("reference_revision");
    let bytes = serde_json::to_vec(&unsigned).unwrap();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.image-function-reference.payload.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}
fn canonical_reference(mut value: Value) -> String {
    value["reference_revision"] = Value::Null;
    let revision = digest(&value);
    value["reference_revision"] = json!(revision);
    serde_json::to_string(&value).unwrap()
}
fn facet_handle(summary: &Value, facet: ImageFacet) -> String {
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["facet"] == facet.name())
        .unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn exact_reference_digest_provenance_and_fresh_resolution_are_closed_and_authority_free() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    let image_json = image.to_json().to_owned();
    let reference_text = image
        .export_function_reference(image.image_digest(), TARGET, None)
        .unwrap();
    assert!(reference_text.len() <= MAX_IMAGE_FUNCTION_REFERENCE_BYTES);
    let reference = parse(&reference_text);
    assert_eq!(reference.as_object().unwrap().len(), 14);
    assert_eq!(reference["schema"], IMAGE_FUNCTION_REFERENCE_SCHEMA);
    assert_eq!(reference["reference_revision"], digest(&reference));
    assert_eq!(reference["image_revision"], image.image_digest());
    assert_eq!(
        reference["project_revision"],
        image.revision().project_revision()
    );
    assert_eq!(
        reference["workspace_revision"],
        image.revision().workspace_revision()
    );
    assert_eq!(
        reference["project_graph_digest"],
        image.revision().semantic_graph_digest()
    );
    assert_eq!(reference["target_kind"], "function");
    assert_eq!(reference["target"], TARGET);
    assert_eq!(reference["facet"], Value::Null);
    assert_eq!(reference["source"].as_object().unwrap().len(), 4);
    assert_eq!(reference["source"]["path"], "src/core.spx");
    assert_eq!(reference["source"]["module"], "calculator.core");
    for fact in ["source_revision", "source_digest"] {
        assert!(reference["source"][fact]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }
    for field in ["source_authority", "execution", "publication_authority"] {
        assert_eq!(reference[field], false);
    }
    assert_eq!(reference["nonclaims"], json!(REFERENCE_NONCLAIMS));

    let resolved_text = image
        .resolve_function_reference(image.image_digest(), reference_text.as_bytes())
        .unwrap();
    assert!(resolved_text.len() <= MAX_IMAGE_FUNCTION_REFERENCE_RESOLUTION_BYTES);
    let resolved = parse(&resolved_text);
    assert_eq!(resolved.as_object().unwrap().len(), 14);
    assert_eq!(
        resolved["schema"],
        IMAGE_FUNCTION_REFERENCE_RESOLUTION_SCHEMA
    );
    for fact in [
        "reference_revision",
        "image_revision",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
        "target",
        "facet",
    ] {
        assert_eq!(resolved[fact], reference[fact]);
    }
    let summary: Value = serde_json::from_str(
        &image
            .function_summary(image.image_digest(), TARGET)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(resolved["function_summary"], summary);
    assert_eq!(resolved["facet_handle"], Value::Null);
    for field in ["source_authority", "execution", "publication_authority"] {
        assert_eq!(resolved[field], false);
    }
    assert_eq!(resolved["nonclaims"], json!(RESOLUTION_NONCLAIMS));
    assert_eq!(image.to_json(), image_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn independently_rebuilt_identical_image_resolves_the_same_function_and_facet() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert_ne!(first.0, second.0);
    assert_eq!(first.bytes(), second.bytes());
    let left = first.image();
    let right = second.image();
    assert_eq!(left.image_digest(), right.image_digest());
    for facet in [None, Some(ImageFacet::Signature), Some(ImageFacet::Cleanup)] {
        let reference = left
            .export_function_reference(left.image_digest(), TARGET, facet)
            .unwrap();
        assert_eq!(
            right
                .export_function_reference(right.image_digest(), TARGET, facet)
                .unwrap(),
            reference
        );
        let local = left
            .resolve_function_reference(left.image_digest(), reference.as_bytes())
            .unwrap();
        let rebuilt = right
            .resolve_function_reference(right.image_digest(), reference.as_bytes())
            .unwrap();
        assert_eq!(rebuilt, local);
        let value = parse(&rebuilt);
        assert_eq!(value["facet"], json!(facet.map(ImageFacet::name)));
        let expected = facet
            .map(|selected| facet_handle(&value["function_summary"], selected))
            .map(Value::String)
            .unwrap_or(Value::Null);
        assert_eq!(value["facet_handle"], expected);
    }
}

#[test]
fn stale_tampered_extra_unknown_facet_and_missing_target_references_fail_closed() {
    let fixture = Fixture::new();
    let original = fixture.image();
    let reference = original
        .export_function_reference(original.image_digest(), TARGET, Some(ImageFacet::Callers))
        .unwrap();
    let valid = parse(&reference);

    let changed_fixture = Fixture::new();
    changed_fixture.change_add_body();
    let changed = changed_fixture.image();
    assert_ne!(changed.image_digest(), original.image_digest());
    code(
        changed.resolve_function_reference(changed.image_digest(), reference.as_bytes()),
        "SPX-G363",
    );

    for (field, value) in [
        ("target", json!("calculator.missing")),
        ("facet", json!("future-facet")),
        (
            "image_revision",
            json!(format!("sha256:{}", "0".repeat(64))),
        ),
        ("target_kind", json!("record")),
    ] {
        let mut altered = valid.clone();
        altered[field] = value;
        let bytes = canonical_reference(altered);
        code(
            original.resolve_function_reference(original.image_digest(), bytes.as_bytes()),
            "SPX-G363",
        );
    }

    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("target");
    let missing = canonical_reference(missing);
    code(
        original.resolve_function_reference(original.image_digest(), missing.as_bytes()),
        "SPX-G363",
    );
    let mut extra = valid.clone();
    extra["unexpected"] = json!(true);
    let extra = canonical_reference(extra);
    code(
        original.resolve_function_reference(original.image_digest(), extra.as_bytes()),
        "SPX-G363",
    );
    let tampered = reference.replacen(TARGET, "calculator.subtract", 1);
    code(
        original.resolve_function_reference(original.image_digest(), tampered.as_bytes()),
        "SPX-G363",
    );
    code(
        original.resolve_function_reference(original.image_digest(), b"{}"),
        "SPX-G363",
    );
    code(
        original.resolve_function_reference(
            original.image_digest(),
            &vec![b' '; MAX_IMAGE_FUNCTION_REFERENCE_BYTES + 1],
        ),
        "SPX-G364",
    );
    assert!(original
        .resolve_function_reference(original.image_digest(), reference.as_bytes())
        .is_ok());
}

#[test]
fn export_rejects_missing_targets_and_stale_expected_images_without_side_effects() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let image = fixture.image();
    code(
        image.export_function_reference(image.image_digest(), "calculator.missing", None),
        "SPX-G227",
    );
    code(
        image.export_function_reference(
            &format!("sha256:{}", "0".repeat(64)),
            TARGET,
            Some(ImageFacet::Signature),
        ),
        "SPX-G221",
    );
    assert_eq!(fixture.bytes(), disk);
}
