//! Candidate ABI-shaped delta evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-abi-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v9"
name = "abi-delta"
version = "1.0.0"
profile = "flat-owned-record-api.v1"
entry = "abi.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["abi.make"]
tests = ["abi.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module abi.core;
@id("abi.packet") record Packet { @id("abi.packet.payload") payload:Bytes, @id("abi.packet.value") value:i64, }
@id("abi.make") fn make(input:borrow Slice<u8>)->Packet {Packet {payload:bytes_copy(input),value:1}}
"#,
            ),
            (
                "src/app.spx",
                r#"module abi.app;
@id("abi.main") fn main()->i64 {0}
"#,
            ),
            (
                "src/tests.spx",
                r#"module abi.tests;
@id("abi.test") fn main()->i64 {0}
"#,
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(base: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
    )
    .unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("hostile ABI delta accepted");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}

#[test]
fn exported_record_shape_and_retained_targets_are_exact_and_replayable() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"add_record_field","target":"abi.packet","field":{"id":"abi.packet.tag","name":"tag","type":"bool","default":{"kind":"bool","value":false}}}),
    );
    let bytes = candidate.abi_delta(candidate.candidate_digest()).unwrap();
    let value: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(value["schema"], "semaprax.project-candidate-abi-delta.v1");
    for status in [
        "compatibility",
        "runtime",
        "deployment",
        "external_consumers",
    ] {
        assert_eq!(value[status], "not_assessed");
    }
    let function = value["facts"]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "abi.make")
        .unwrap();
    assert_eq!(function["classification"], "unchanged");
    let nominal = value["facts"]["public_nominals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["candidate"]["declaration_id"] == "abi.packet")
        .unwrap();
    assert_eq!(nominal["classification"], "changed");
    assert_eq!(
        nominal["candidate"]["shape"]["fields"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(nominal["candidate"]["checked_facts"]["layout_key"].is_string());
    assert_eq!(value["facts"]["targets"].as_array().unwrap().len(), 4);
    candidate
        .verify_abi_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(
        restored.abi_delta(restored.candidate_digest()).unwrap(),
        bytes
    );
}

#[test]
fn abi_delta_rejects_tampering_stale_selectors_and_oversized_evidence() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let bytes = candidate.abi_delta(candidate.candidate_digest()).unwrap();
    let mut tampered: Value = serde_json::from_str(&bytes).unwrap();
    tampered["compatibility"] = json!("compatible");
    tampered.sort_all_objects();
    let tampered = format!("{tampered}\n");
    code(
        candidate.verify_abi_delta(candidate.candidate_digest(), tampered.as_bytes()),
        "SPX-G524",
    );
    code(
        candidate
            .abi_delta("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
        "SPX-G224",
    );
    code(
        candidate.verify_abi_delta(
            candidate.candidate_digest(),
            &vec![b' '; semaprax::project::MAX_PROJECT_CANDIDATE_ABI_DELTA_BYTES + 1],
        ),
        "SPX-G523",
    );
}
