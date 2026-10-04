use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-patch-receipt-{}-{}",
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

fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
}

fn canonical(value: Value) -> Vec<u8> {
    let mut value = value;
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn receipt_digest(content: &Value) -> String {
    let bytes = canonical(content.clone());
    let mut hash = Sha256::new();
    hash.update(b"semaprax.patch-receipt.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

#[test]
fn receipt_is_compact_bound_and_independently_replay_verified() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let candidate = apply(
        &root,
        json!({"kind":"add_declaration","target":"calculator.add","declaration":{
            "id":"receipt.identity","name":"identity","parameters":[{"name":"value","type":"i64","mode":"value"}],
            "return_type":"i64","effects":[],"requires":[],"ensures":[],
            "body":{"kind":"place","name":"value"}}}),
    );
    let receipt = candidate
        .patch_receipt(candidate.candidate_digest())
        .unwrap();
    assert!(receipt.len() <= 8 * 1024);
    let value: Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(value["schema"], "semaprax.patch-receipt.v1");
    assert_eq!(
        value["content"]["binding"]["candidate_digest"],
        candidate.candidate_digest()
    );
    assert_eq!(
        value["content"]["declarations"]["directly_changed_count"],
        1
    );
    assert_eq!(
        value["content"]["declarations"]["directly_changed_preview"][0]["id"],
        "receipt.identity"
    );
    assert_eq!(value["content"]["effect_usage"]["status"], "not_applicable");
    let verification: Value = serde_json::from_str(
        &candidate
            .verify_patch_receipt(candidate.candidate_digest(), receipt.as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(verification["result"], "exact_recomputation");
}

#[test]
fn recomputed_outer_digest_cannot_make_a_tampered_receipt_verify() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let candidate = apply(
        &root,
        json!({"kind":"rename_declaration","target":"calculator.add","name":"sum"}),
    );
    let receipt = candidate
        .patch_receipt(candidate.candidate_digest())
        .unwrap();
    let mut value: Value = serde_json::from_str(&receipt).unwrap();
    value["content"]["declarations"]["directly_changed_preview"][0]["change"] = json!("removed");
    value["receipt_digest"] = json!(receipt_digest(&value["content"]));
    let tampered = canonical(value);
    let errors = candidate
        .verify_patch_receipt(candidate.candidate_digest(), &tampered)
        .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G984"));
}

#[test]
fn stale_selector_has_a_bound_refusal_without_a_result_candidate_or_successful_checks() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let stale = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let receipt = candidate.patch_receipt_refusal(stale).unwrap();
    let value: Value = serde_json::from_str(&receipt).unwrap();
    assert!(receipt.len() <= 8 * 1024);
    assert_eq!(
        value["content"]["attempt"]["status"],
        "refused_stale_candidate_selector"
    );
    assert_eq!(
        value["content"]["binding"]["requested_candidate_digest"],
        stale
    );
    assert!(value["content"]["binding"]["project_revision"].is_null());
    assert_eq!(value["content"]["checks"][0]["result"], "failed");
    assert!(value["content"]["checks"].as_array().unwrap()[1..]
        .iter()
        .all(|check| check["result"] == "not_run"));
    let verification: Value = serde_json::from_str(
        &candidate
            .verify_patch_receipt_refusal(stale, receipt.as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(verification["result"], "exact_refusal_recomputation");
}

#[test]
fn receipt_comparison_verifies_both_routes_and_refuses_incompatible_inputs() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = apply(
        &root,
        json!({"kind":"rename_declaration","target":"calculator.add","name":"sum"}),
    );
    let right = apply(
        &root,
        json!({"kind":"rename_declaration","target":"calculator.add","name":"plus"}),
    );
    let left_receipt = left.patch_receipt(left.candidate_digest()).unwrap();
    let right_receipt = right.patch_receipt(right.candidate_digest()).unwrap();
    let comparison: Value = serde_json::from_str(
        &left
            .compare_patch_receipts(
                left.candidate_digest(),
                left_receipt.as_bytes(),
                &right,
                right.candidate_digest(),
                right_receipt.as_bytes(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(comparison["result"], "comparable");
    assert_eq!(comparison["reasons"], json!([]));
    assert_eq!(
        comparison["comparison"]["effect_usage"]["left"]["status"],
        "not_applicable"
    );

    let stale = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    let refusal = right.patch_receipt_refusal(stale).unwrap();
    let incomparable: Value = serde_json::from_str(
        &left
            .compare_patch_receipts(
                left.candidate_digest(),
                left_receipt.as_bytes(),
                &right,
                stale,
                refusal.as_bytes(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(incomparable["result"], "not_comparable");
    assert!(incomparable["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("right_receipt_did_not_admit_a_candidate")));
    assert!(incomparable["comparison"].is_null());
}
