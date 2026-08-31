//! Pathless artifact-delta evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageArtifactKind, ProjectCandidate, ProjectRevision,
    SemanticChange, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    module: &'static str,
}
impl Fixture {
    fn new(owned: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-artifact-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let module = if owned {
            "src/frame.spx"
        } else {
            "src/core.spx"
        };
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join(if owned {
            "examples/frame-payload-project"
        } else {
            "examples/calculator-project"
        });
        for path in ["semaprax.toml", "src/app.spx", module, "src/tests.spx"] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self { root, module }
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        ["semaprax.toml", "src/app.spx", self.module, "src/tests.spx"]
            .iter()
            .map(|p| std::fs::read(self.root.join(p)).unwrap())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn apply(base: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
    )
    .unwrap()
}
fn report(candidate: &ProjectCandidate, kind: ImageArtifactKind) -> Value {
    serde_json::from_str(
        &candidate
            .artifact_delta(candidate.candidate_digest(), kind)
            .unwrap(),
    )
    .unwrap()
}
fn carrier(revision: &ProjectRevision, kind: ImageArtifactKind) -> Value {
    match kind {
        ImageArtifactKind::Web => {
            let build = revision
                .build_web_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
                .unwrap();
            build.verify().unwrap();
            serde_json::from_str(build.envelope()).unwrap()
        }
        ImageArtifactKind::Npm => {
            let build = revision
                .build_npm_inline(MAX_IMAGE_ARTIFACT_BUILD_BYTES)
                .unwrap();
            build.verify().unwrap();
            serde_json::from_str(build.envelope()).unwrap()
        }
        ImageArtifactKind::OpenApi => {
            panic!("OpenAPI has dedicated source-bound artifact evidence; this helper decodes Web/npm carriers")
        }
    }
}
fn files(carrier: &Value, kind: ImageArtifactKind) -> BTreeMap<String, Vec<u8>> {
    carrier["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row[if kind == ImageArtifactKind::Web {
                "content_hex"
            } else {
                "hex"
            }]
            .as_str()
            .unwrap();
            assert_eq!(hex.len() % 2, 0);
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(row["sha256"], sha256(&bytes));
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
fn assert_actual_files(candidate: &ProjectCandidate, kind: ImageArtifactKind, value: &Value) {
    let before = files(&carrier(candidate.base_revision(), kind), kind);
    let after = files(&carrier(candidate.revision(), kind), kind);
    let paths = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let rows = value["files"].as_array().unwrap();
    assert_eq!(rows.len(), paths.len());
    for (row, path) in rows.iter().zip(paths) {
        assert_eq!(row["path"], path);
        let expected_equal = before.get(&path) == after.get(&path);
        assert_eq!(row["bytes_equal"], expected_equal);
        for (side, files) in [("base", &before), ("candidate", &after)] {
            if let Some(bytes) = files.get(&path) {
                assert_eq!(row[side]["bytes"], bytes.len());
                assert_eq!(row[side]["sha256"], sha256(bytes));
                assert_eq!(row[side]["path"], path);
            } else {
                assert!(row[side].is_null());
            }
        }
        if expected_equal {
            assert_eq!(row["change"], "unchanged");
        }
    }
    assert_eq!(value["comparison"]["artifact_bytes_equal"], before == after);
    for (side, revision) in [
        ("base", candidate.base_revision()),
        ("candidate", candidate.revision()),
    ] {
        assert_eq!(value[side]["project_revision"], revision.project_revision());
        assert_eq!(
            value[side]["project_graph_digest"],
            revision.semantic_graph_digest()
        );
        let bindings = value[side]["sources"].as_array().unwrap();
        assert_eq!(bindings.len(), revision.sources().len());
        for (binding, source) in bindings.iter().zip(revision.sources()) {
            assert_eq!(binding["path"], source.path());
            assert_eq!(binding["source_revision"], source.source_revision());
            assert_eq!(binding["source_digest"], source.source_digest());
        }
    }
}
fn replay(candidate: &ProjectCandidate, kind: ImageArtifactKind) {
    let bytes = candidate
        .artifact_delta(candidate.candidate_digest(), kind)
        .unwrap();
    candidate
        .verify_artifact_delta(candidate.candidate_digest(), kind, bytes.as_bytes())
        .unwrap();
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(
        restored
            .artifact_delta(restored.candidate_digest(), kind)
            .unwrap(),
        bytes
    );
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("hostile artifact delta accepted");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    format!("{value}\n")
}
fn fact_digest(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = canonical(value.clone()).into_bytes();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.candidate-artifact-delta.fact.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[test]
fn unchanged_web_candidate_retains_all_exact_files_exports_and_nonpublication_flags() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let value = report(&base, ImageArtifactKind::Web);
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-artifact-delta.v1"
    );
    assert_eq!(value["kind"], "web");
    assert_eq!(value["comparison"]["carrier_equal"], true);
    assert_eq!(value["comparison"]["artifact_bytes_equal"], true);
    assert_eq!(value["inventory"]["changed_files"], 0);
    assert_eq!(value["inventory"]["base_files"], 7);
    assert_eq!(value["max_build_bytes"], MAX_IMAGE_ARTIFACT_BUILD_BYTES);
    for key in [
        "artifact_materialization",
        "target_execution",
        "source_authority",
    ] {
        assert_eq!(value[key], false);
    }
    assert_eq!(value["outside_projection"], json!(["rust", "c", "openapi"]));
    let exports = value["exports"].as_array().unwrap();
    assert_eq!(
        exports.len(),
        base.revision().manifest().web_exports().len()
    );
    for (export, id) in exports.iter().zip(base.revision().manifest().web_exports()) {
        assert_eq!(export["id"], *id);
        assert_eq!(export["exact_equal"], true);
        assert_eq!(export["change"], "unchanged");
    }
    assert_actual_files(&base, ImageArtifactKind::Web, &value);
    replay(&base, ImageArtifactKind::Web);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn scalar_signature_changes_bind_exact_web_abi_and_file_bytes_without_per_export_artifact_attribution(
) {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
    );
    let value = report(&candidate, ImageArtifactKind::Web);
    assert_actual_files(&candidate, ImageArtifactKind::Web, &value);
    assert_eq!(value["comparison"]["carrier_equal"], false);
    assert_eq!(
        value["inventory"]["base_exports"],
        value["inventory"]["candidate_exports"]
    );
    for (revision, count) in [(base.revision(), 2), (candidate.revision(), 3)] {
        let files = files(
            &carrier(revision, ImageArtifactKind::Web),
            ImageArtifactKind::Web,
        );
        let metadata: Value =
            serde_json::from_slice(files.get("semaprax.scalar-exports.json").unwrap()).unwrap();
        let exported = metadata["scalar_abi"]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["stable_id"] == "calculator.add")
            .unwrap();
        assert_eq!(exported["parameters"].as_array().unwrap().len(), count);
    }
    let export = value["exports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "calculator.add")
        .unwrap();
    assert_eq!(export["base"]["id"], "calculator.add");
    assert_eq!(export["candidate"]["id"], "calculator.add");
    replay(&candidate, ImageArtifactKind::Web);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_npm_descriptor_preserves_stable_export_identity_through_checked_signature_change() {
    let fixture = Fixture::new(true);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"change_function_signature","target":"frame.payload","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
    );
    let value = report(&candidate, ImageArtifactKind::Npm);
    assert_actual_files(&candidate, ImageArtifactKind::Npm, &value);
    assert_eq!(value["inventory"]["base_files"], 6);
    assert_eq!(value["inventory"]["candidate_files"], 6);
    for (revision, count) in [(base.revision(), 1), (candidate.revision(), 2)] {
        let files = files(
            &carrier(revision, ImageArtifactKind::Npm),
            ImageArtifactKind::Npm,
        );
        let metadata: Value =
            serde_json::from_slice(files.get("semaprax.api.json").unwrap()).unwrap();
        let descriptor = revision.public_api_descriptor().unwrap();
        assert_eq!(
            metadata["descriptor"].as_str().unwrap().as_bytes(),
            descriptor.canonical_bytes()
        );
        assert_eq!(metadata["descriptor_digest"], descriptor.digest());
        let descriptor: Value =
            serde_json::from_str(metadata["descriptor"].as_str().unwrap()).unwrap();
        let export = descriptor["exports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["stable_id"] == "frame.payload")
            .unwrap();
        assert_eq!(export["typescript_name"], "frame.payload");
        assert_eq!(export["parameters"].as_array().unwrap().len(), count);
    }
    let export = value["exports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "frame.payload")
        .unwrap();
    assert_eq!(export["base"]["source"]["path"], "src/frame.spx");
    assert_eq!(export["candidate"]["source"]["path"], "src/frame.spx");
    replay(&candidate, ImageArtifactKind::Npm);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn source_changes_are_not_automatically_reported_as_changes_to_every_artifact_file() {
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let original = fixture.candidate();
    let prepared = apply(
        &original,
        json!({"kind":"add_declaration","target":"calculator.add","declaration":{"id":"artifact.internal","name":"internal_value","parameters":[],"return_type":"i64","effects":[],"requires":[],"ensures":[],"body":{"kind":"i64","value":1}}}),
    );
    let base = ProjectCandidate::open(
        Arc::clone(prepared.revision()),
        prepared.revision().project_revision(),
    )
    .unwrap();
    let candidate = apply(
        &base,
        json!({"kind":"replace_function_body","target":"artifact.internal","body":{"kind":"i64","value":2}}),
    );
    let value = report(&candidate, ImageArtifactKind::Web);
    assert_actual_files(&candidate, ImageArtifactKind::Web, &value);
    assert_eq!(value["comparison"]["source_bindings_equal"], false);
    assert_eq!(value["comparison"]["carrier_equal"], false);
    let package = value["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "package.json")
        .unwrap();
    assert_eq!(package["bytes_equal"], true);
    assert_eq!(package["change"], "unchanged");
    replay(&candidate, ImageArtifactKind::Web);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn reminted_file_facts_wrong_kind_noncanonical_stale_and_unsupported_profile_requests_fail_closed()
{
    let fixture = Fixture::new(false);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i64","value":42}}),
    );
    let bytes = candidate
        .artifact_delta(candidate.candidate_digest(), ImageArtifactKind::Web)
        .unwrap();
    let mut tampered: Value = serde_json::from_str(&bytes).unwrap();
    tampered["candidate"]["artifacts"][0]["sha256"] = json!(sha256(b"forged"));
    tampered["comparison"]["candidate_digest"] = json!(fact_digest(&tampered["candidate"]));
    code(
        candidate.verify_artifact_delta(
            candidate.candidate_digest(),
            ImageArtifactKind::Web,
            canonical(tampered).as_bytes(),
        ),
        "SPX-G333",
    );
    let mut wrongkind: Value = serde_json::from_str(&bytes).unwrap();
    wrongkind["kind"] = json!("npm");
    code(
        candidate.verify_artifact_delta(
            candidate.candidate_digest(),
            ImageArtifactKind::Web,
            canonical(wrongkind).as_bytes(),
        ),
        "SPX-G333",
    );
    code(
        candidate.verify_artifact_delta(
            candidate.candidate_digest(),
            ImageArtifactKind::Web,
            format!("{bytes} ").as_bytes(),
        ),
        "SPX-G333",
    );
    assert!(candidate
        .artifact_delta(base.candidate_digest(), ImageArtifactKind::Web)
        .is_err());
    assert_eq!(
        candidate
            .artifact_delta(candidate.candidate_digest(), ImageArtifactKind::Web)
            .unwrap(),
        bytes
    );
    assert_eq!(fixture.bytes(), disk);
    let owned = Fixture::new(true);
    let disk = owned.bytes();
    let candidate = owned.candidate();
    code(
        candidate.artifact_delta(candidate.candidate_digest(), ImageArtifactKind::Web),
        "SPX-W120",
    );
    assert_eq!(owned.bytes(), disk);
}
