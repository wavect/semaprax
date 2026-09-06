//! Standalone ContractsAndTestsFacts v1 derivation and replay regressions.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ContractsAndTestsFacts, CONTRACTS_AND_TESTS_FACTS_SCHEMA,
    MAX_CONTRACTS_AND_TESTS_FACTS_BYTES,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const DIGEST_DOMAIN: &[u8] = b"semaprax.contracts-and-tests-facts.digest.v1\0";

fn remint(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(DIGEST_DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-contracts-tests-facts-{}-{}",
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
        std::fs::write(root.join("src/tests.spx"), TEST_SOURCE).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> std::sync::Arc<semaprax::project::ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn replace(&self, path: &str, before: &str, after: &str) {
        let path = self.0.join(path);
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains(before));
        std::fs::write(path, source.replacen(before, after, 1)).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TEST_SOURCE: &str = r#"module calculator.tests;
use function @id("calculator.add") from calculator.core as add;

@id("calculator.tests.main")
fn main() -> i64
    ensures result == 0
{
    0
}

@id("calculator.tests.zed")
fn test_zed() -> i64
    requires add(1, 1) == 2
    ensures result == 0
{
    0
}

@id("calculator.tests.alpha")
fn test_alpha() -> i64
{
    0
}

@id("calculator.tests.skipped")
fn test_skipped(value: i64) -> i64
{
    value
}

@id("calculator.tests.helper")
fn helper() -> i64
{
    0
}
"#;

#[test]
fn contracts_and_declared_tests_are_exact_ordered_hir_facts() {
    let revision = Fixture::new().revision();
    let facts =
        ContractsAndTestsFacts::derive(revision.clone(), revision.project_revision()).unwrap();
    let ids = facts
        .functions()
        .iter()
        .map(|fact| fact.stable_id())
        .collect::<Vec<_>>();
    assert!(ids
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes()));
    assert_eq!(
        facts
            .tests()
            .iter()
            .map(|test| (test.stable_id(), test.kind()))
            .collect::<Vec<_>>(),
        vec![
            ("calculator.tests.alpha", "named_test"),
            ("calculator.tests.main", "test_main"),
            ("calculator.tests.zed", "named_test"),
        ]
    );
    assert!(!facts
        .tests()
        .iter()
        .any(|test| test.name() == "test_skipped" || test.name() == "helper"));
    let divide = facts
        .functions()
        .iter()
        .find(|fact| fact.stable_id() == "calculator.divide")
        .unwrap();
    assert_eq!(divide.requires().len(), 1);
    assert_eq!(divide.requires()[0].phase(), "requires");
    assert_eq!(divide.requires()[0].index(), 0);
    assert!(divide.requires()[0].source_fact().contains("binary"));
    let zed = facts
        .functions()
        .iter()
        .find(|fact| fact.stable_id() == "calculator.tests.zed")
        .unwrap();
    assert_eq!(zed.requires()[0].index(), 0);
    assert_eq!(zed.ensures()[0].index(), 0);
    assert!(facts
        .functions()
        .iter()
        .any(|fact| fact.stable_id() == "calculator.tests.helper"
            && fact.requires().is_empty()
            && fact.ensures().is_empty()));

    let value: Value = serde_json::from_str(facts.to_json()).unwrap();
    assert_eq!(value["schema"], CONTRACTS_AND_TESTS_FACTS_SCHEMA);
    assert_eq!(value["coverage_claimed"], false);
    assert_eq!(value["execution_claimed"], false);
    assert_eq!(value["source_authority"], false);
    assert_eq!(
        ContractsAndTestsFacts::replay(
            revision.clone(),
            revision.project_revision(),
            facts.facts_digest(),
            facts.to_json().as_bytes()
        )
        .unwrap(),
        facts
    );
}

#[test]
fn malformed_stale_reminted_and_over_bound_facts_fail_closed() {
    let revision = Fixture::new().revision();
    let facts =
        ContractsAndTestsFacts::derive(revision.clone(), revision.project_revision()).unwrap();
    let mut stale = revision.project_revision().as_bytes().to_vec();
    stale[7] = if stale[7] == b'a' { b'b' } else { b'a' };
    let stale = String::from_utf8(stale).unwrap();
    assert_eq!(
        ContractsAndTestsFacts::derive(revision.clone(), &stale).unwrap_err()[0].code,
        "SPX-G575"
    );

    let mut noncanonical = facts.to_json().as_bytes().to_vec();
    noncanonical.push(b' ');
    assert_eq!(
        ContractsAndTestsFacts::replay(
            revision.clone(),
            revision.project_revision(),
            facts.facts_digest(),
            &noncanonical
        )
        .unwrap_err()[0]
            .code,
        "SPX-G574"
    );

    let mut bad_phase: Value = serde_json::from_str(facts.to_json()).unwrap();
    let divide = bad_phase["functions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["stable_id"] == "calculator.divide")
        .unwrap();
    divide["requires"][0]["phase"] = Value::String("ensures".to_owned());
    let mut bad_phase = serde_json::to_string(&bad_phase).unwrap();
    bad_phase.push('\n');
    assert_eq!(
        ContractsAndTestsFacts::replay(
            revision.clone(),
            revision.project_revision(),
            facts.facts_digest(),
            bad_phase.as_bytes()
        )
        .unwrap_err()[0]
            .code,
        "SPX-G574"
    );

    let mut reminted: Value = serde_json::from_str(facts.to_json()).unwrap();
    reminted["tests"][0]["name"] = Value::String("test_alphb".to_owned());
    let mut reminted = serde_json::to_string(&reminted).unwrap();
    reminted.push('\n');
    let reminted_digest = remint(reminted.as_bytes());
    assert_eq!(
        ContractsAndTestsFacts::replay(
            revision.clone(),
            revision.project_revision(),
            &reminted_digest,
            reminted.as_bytes()
        )
        .unwrap_err()[0]
            .code,
        "SPX-G575"
    );

    let over_bound = vec![b' '; MAX_CONTRACTS_AND_TESTS_FACTS_BYTES + 1];
    assert_eq!(
        ContractsAndTestsFacts::replay(
            revision,
            facts.project_revision(),
            facts.facts_digest(),
            &over_bound
        )
        .unwrap_err()[0]
            .code,
        "SPX-G574"
    );
}

#[test]
fn subject_binding_changes_digest_while_body_only_edits_preserve_inventory() {
    let fixture = Fixture::new();
    let original_revision = fixture.revision();
    let original = ContractsAndTestsFacts::derive(
        original_revision.clone(),
        original_revision.project_revision(),
    )
    .unwrap();

    fixture.replace("src/core.spx", "left + right", "left + right + 0");
    fixture.replace(
        "src/tests.spx",
        "fn test_alpha() -> i64\n{\n    0\n}",
        "fn test_alpha() -> i64\n{\n    if true { 0 } else { 1 }\n}",
    );
    let body_revision = fixture.revision();
    let body_only =
        ContractsAndTestsFacts::derive(body_revision.clone(), body_revision.project_revision())
            .unwrap();
    assert_ne!(original.project_revision(), body_only.project_revision());
    assert_ne!(original.facts_digest(), body_only.facts_digest());
    assert_eq!(original.functions(), body_only.functions());
    assert_eq!(original.tests(), body_only.tests());

    fixture.replace("src/core.spx", "requires right != 0", "requires right > 0");
    let contract_revision = fixture.revision();
    let contract_changed = ContractsAndTestsFacts::derive(
        contract_revision.clone(),
        contract_revision.project_revision(),
    )
    .unwrap();
    assert_ne!(body_only.facts_digest(), contract_changed.facts_digest());
    let before = body_only
        .functions()
        .iter()
        .find(|fact| fact.stable_id() == "calculator.divide")
        .unwrap();
    let after = contract_changed
        .functions()
        .iter()
        .find(|fact| fact.stable_id() == "calculator.divide")
        .unwrap();
    assert_ne!(before.requires(), after.requires());
}
