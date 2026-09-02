//! Bidirectional actual-merge previews: authored regression sources, unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
            "spx-merge-preview-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "merge-preview"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "preview.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["preview.public"]
tests = ["preview.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module preview.core;
@id("preview.add") fn add(left:i64,right:i64)->i64 {left+right}
@id("preview.subtract") fn subtract(left:i64,right:i64)->i64 {left-right}
@id("preview.public") fn public_value(value:i64)->i64 {add(value,1)}
"#,
            ),
            (
                "src/app.spx",
                r#"module preview.app;
use function @id("preview.public") from preview.core as public_value;
@id("preview.main") fn main()->i64 {public_value(41)}
"#,
            ),
            (
                "src/tests.spx",
                r#"module preview.tests;
use function @id("preview.public") from preview.core as public_value;
@id("preview.test") fn main()->i64 {if public_value(41)==42 {0}else{1}}
"#,
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(fixture.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        fixture
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
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
fn apply(base: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
    )
    .unwrap()
}
fn body(target: &str, value: i64) -> Value {
    json!({"kind":"replace_function_body","target":target,"body":{"kind":"i64","value":value}})
}
fn rename(target: &str, name: &str) -> Value {
    json!({"kind":"rename_declaration","target":target,"name":name})
}
fn signature(name: &str) -> Value {
    json!({"kind":"change_function_signature","target":"preview.add","append_parameters":[{"name":name,"type":"i64","argument":{"kind":"i64","value":0}}]})
}
fn record(id: &str, name: &str) -> Value {
    json!({"kind":"add_declaration","target":"preview.public","declaration":{"kind":"record","id":id,"name":name,"fields":[{"id":format!("{id}.value"),"name":"value","type":"i64"}]}})
}
fn sources(candidate: &ProjectCandidate) -> Vec<(String, String)> {
    candidate
        .revision()
        .sources()
        .iter()
        .map(|source| (source.path().to_owned(), source.source().to_owned()))
        .collect()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejected selection");
    assert!(errors.iter().any(|row| row.code == expected), "{errors:?}");
}
fn preview(left: &ProjectCandidate, right: &ProjectCandidate) -> (String, Value) {
    let raw = left
        .merge_preview(left.candidate_digest(), right, right.candidate_digest())
        .unwrap();
    assert!(raw.len() <= semaprax::project::MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES);
    let report: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(report.as_object().unwrap().len(), 12);
    assert_eq!(
        report["schema"],
        semaprax::project::PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA
    );
    assert_eq!(
        report["base_revision"],
        left.base_revision().project_revision()
    );
    assert_eq!(report["left_candidate_revision"], left.candidate_digest());
    assert_eq!(report["right_candidate_revision"], right.candidate_digest());
    assert_eq!(report["tests"], "not_run");
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["candidate_retained"], false);
    assert_eq!(
        report["validation"],
        "ordinary_merge_with_full_candidate_admission"
    );
    assert_eq!(
        report["nonclaims"],
        json!([
            "not_behavioral_equivalence",
            "not_runtime_or_test_execution",
            "not_external_consumer_compatibility",
            "not_permission_to_publish_or_retain_candidates",
            "directional_rejection_may_be_a_conservative_or_capacity_limit"
        ])
    );
    assert_eq!(
        raw,
        left.merge_preview(left.candidate_digest(), right, right.candidate_digest())
            .unwrap()
    );
    (raw, report)
}
fn accepted(row: &Value, actual: &semaprax::project::ProjectCandidateRebase) {
    assert_eq!(row.as_object().unwrap().len(), 6);
    assert_eq!(row["status"], "accepted");
    let candidate = actual.candidate();
    assert_eq!(
        row["result_project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        row["result_candidate_revision"],
        candidate.candidate_digest()
    );
    let merge: Value = serde_json::from_str(actual.to_json()).unwrap();
    assert_eq!(row["shared_history_prefix"], merge["shared_history_prefix"]);
    assert_eq!(
        row["source_file_count"],
        candidate.revision().sources().len()
    );
    assert_eq!(
        row["source_bytes"],
        candidate
            .revision()
            .sources()
            .iter()
            .map(|source| source.source().len())
            .sum::<usize>()
    );
}
fn rejected(row: &Value, errors: &[Diagnostic]) {
    assert_eq!(row.as_object().unwrap().len(), 3);
    assert_eq!(row["status"], "rejected");
    assert_eq!(
        row["interpretation"],
        "merge_rejected_not_proof_of_incompatibility"
    );
    assert_eq!(
        row["diagnostics"],
        Value::Array(
            errors
                .iter()
                .map(|error| json!({"code":error.code,"message":error.message}))
                .collect()
        )
    );
}

#[test]
fn unrelated_body_and_display_edits_preview_both_actual_orders_without_retaining_or_changing_parents(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = apply(&base, body("preview.add", 7));
    let right = apply(&base, rename("preview.subtract", "difference"));
    let before = (left.to_json().to_owned(), right.to_json().to_owned());
    let left_then_right = right
        .merge(right.candidate_digest(), &left, left.candidate_digest())
        .unwrap();
    let right_then_left = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert_eq!(
        sources(left_then_right.candidate()),
        sources(right_then_left.candidate())
    );
    assert_ne!(
        left_then_right.candidate().candidate_digest(),
        right_then_left.candidate().candidate_digest(),
        "equal canonical source must not erase distinct ordered histories"
    );
    let (raw, report) = preview(&left, &right);
    accepted(&report["left_then_right"], &left_then_right);
    accepted(&report["right_then_left"], &right_then_left);
    assert_eq!(report["same_source"], true);
    let (_, reverse) = preview(&right, &left);
    assert_eq!(reverse["left_then_right"], report["right_then_left"]);
    assert_eq!(reverse["right_then_left"], report["left_then_right"]);
    let receipt: Value = serde_json::from_str(
        &left
            .verify_merge_preview(
                left.candidate_digest(),
                &right,
                right.candidate_digest(),
                raw.as_bytes(),
            )
            .unwrap(),
    )
    .unwrap();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-candidate-merge-preview.v1\0");
    hash.update((raw.len() as u64).to_le_bytes());
    hash.update(raw.as_bytes());
    assert_eq!(
        receipt,
        json!({"schema":"semaprax.project-candidate-merge-preview-verification.v1",
        "result":"exact_source_history_recomputation","base_revision":base.base_revision().project_revision(),
        "left_candidate_revision":left.candidate_digest(),"right_candidate_revision":right.candidate_digest(),
        "report_digest":format!("sha256:{:x}", semaprax::digest_hex::LowerHex(hash.finalize())),"tests":"not_run","source_authority":false,"candidate_retained":false})
    );
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        left.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(
        restored
            .merge_preview(
                restored.candidate_digest(),
                &right,
                right.candidate_digest()
            )
            .unwrap(),
        raw
    );
    assert_eq!(left.to_json(), before.0);
    assert_eq!(right.to_json(), before.1);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn independent_type_additions_are_accepted_but_actual_canonical_declaration_order_is_not_erased() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = apply(&base, record("preview.alpha", "Alpha"));
    let right = apply(&base, record("preview.beta", "Beta"));
    let lr = right
        .merge(right.candidate_digest(), &left, left.candidate_digest())
        .unwrap();
    let rl = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert_ne!(
        sources(lr.candidate()),
        sources(rl.candidate()),
        "fixture must prove actual source order differs before asserting the preview"
    );
    let (_, report) = preview(&left, &right);
    accepted(&report["left_then_right"], &lr);
    accepted(&report["right_then_left"], &rl);
    assert_eq!(report["same_source"], false);
    for (candidate, names) in [
        (lr.candidate(), ["Alpha", "Beta"]),
        (rl.candidate(), ["Beta", "Alpha"]),
    ] {
        let source = candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == "src/core.spx")
            .unwrap();
        let parsed = semaprax::parse(source.source(), source.path()).unwrap();
        assert_eq!(
            parsed
                .types
                .iter()
                .map(|ty| ty.name.as_str())
                .collect::<Vec<_>>(),
            names
        );
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn competing_signature_rejections_are_actual_directional_diagnostics_not_an_incompatibility_proof()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = apply(&base, signature("left_extra"));
    let right = apply(&base, signature("right_extra"));
    let lr = right
        .merge(right.candidate_digest(), &left, left.candidate_digest())
        .err()
        .unwrap();
    let rl = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .err()
        .unwrap();
    assert!(lr.iter().any(|error| error.code == "SPX-G235"));
    assert!(rl.iter().any(|error| error.code == "SPX-G235"));
    let (_, report) = preview(&left, &right);
    rejected(&report["left_then_right"], &lr);
    rejected(&report["right_then_left"], &rl);
    assert!(report["same_source"].is_null());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn exact_shared_history_prefix_and_self_preview_do_not_duplicate_changes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let prefix = apply(&base, rename("preview.public", "published_value"));
    let left = apply(&prefix, body("preview.add", 7));
    let right = apply(&prefix, body("preview.subtract", 9));
    let (_, report) = preview(&left, &right);
    assert_eq!(report["left_then_right"]["shared_history_prefix"], 1);
    assert_eq!(report["right_then_left"]["shared_history_prefix"], 1);
    for (key, actual) in [
        (
            "left_then_right",
            right
                .merge(right.candidate_digest(), &left, left.candidate_digest())
                .unwrap(),
        ),
        (
            "right_then_left",
            left.merge(left.candidate_digest(), &right, right.candidate_digest())
                .unwrap(),
        ),
    ] {
        accepted(&report[key], &actual);
        let history: Value = serde_json::from_str(actual.candidate().to_json()).unwrap();
        assert_eq!(history["changes"].as_array().unwrap().len(), 3);
    }
    let (_, same) = preview(&left, &left);
    assert_eq!(same["same_source"], true);
    assert_eq!(
        same["left_then_right"]["result_candidate_revision"],
        left.candidate_digest()
    );
    assert_eq!(same["left_then_right"]["shared_history_prefix"], 2);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn selectors_common_base_and_exact_preview_verification_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let left = apply(&base, body("preview.add", 7));
    let right = apply(&base, body("preview.subtract", 9));
    let stale = format!("sha256:{}", "0".repeat(64));
    code(
        left.merge_preview(&stale, &right, right.candidate_digest()),
        "SPX-G224",
    );
    code(
        left.merge_preview(left.candidate_digest(), &right, &stale),
        "SPX-G224",
    );
    code(
        left.merge_preview("not-a-digest", &right, right.candidate_digest()),
        "SPX-G222",
    );
    let independent = ProjectCandidate::open(
        Arc::clone(right.revision()),
        right.revision().project_revision(),
    )
    .unwrap();
    code(
        left.merge_preview(
            left.candidate_digest(),
            &independent,
            independent.candidate_digest(),
        ),
        "SPX-G235",
    );
    let (raw, mut report) = preview(&left, &right);
    report["same_source"] = json!(false);
    code(
        left.verify_merge_preview(
            left.candidate_digest(),
            &right,
            right.candidate_digest(),
            report.to_string().as_bytes(),
        ),
        "SPX-G235",
    );
    code(
        left.verify_merge_preview(
            left.candidate_digest(),
            &right,
            right.candidate_digest(),
            format!("{raw} ").as_bytes(),
        ),
        "SPX-G235",
    );
    code(
        left.verify_merge_preview(
            left.candidate_digest(),
            &right,
            right.candidate_digest(),
            &vec![b' '; semaprax::project::MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES + 1],
        ),
        "SPX-G226",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn actual_combined_history_capacity_is_reported_for_each_order_without_publishing_a_result() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut left = apply(&base, body("preview.add", 1));
    let mut right = apply(&base, body("preview.subtract", 1));
    for n in 2..=17 {
        left = apply(&left, body("preview.add", n));
    }
    for n in 2..=16 {
        right = apply(&right, body("preview.subtract", n));
    }
    let (_, report) = preview(&left, &right);
    for row in [&report["left_then_right"], &report["right_then_left"]] {
        assert_eq!(row["status"], "rejected");
        assert!(row["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "SPX-G234"));
    }
    assert!(report["same_source"].is_null());
    assert_eq!(fixture.bytes(), disk);
}
