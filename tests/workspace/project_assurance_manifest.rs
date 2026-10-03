//! Project assurance-manifest regressions.
//!
//! The report is an authenticated project projection, so these tests keep the
//! fixture deliberately small but exercise the provider source as well as the
//! entry, public-API, and test roots selected by the manifest.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::architecture_claims::{ArchitectureClaim, ArchitectureClaimSet};
use semaprax::assurance_manifest::project::{
    derive, generate, generate_from_snapshot, verify_against_revision, ProjectAssuranceOptions,
};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::with_authenticated_project;
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-project-assurance-manifest-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn source(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wire(document: &str) -> Value {
    serde_json::from_str(document).unwrap()
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = match result {
        Err(errors) => errors,
        Ok(_) => panic!("expected {code}"),
    };
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

fn held_claims() -> ArchitectureClaimSet {
    ArchitectureClaimSet::new(vec![ArchitectureClaim::forbid_reaches(
        "calculator-is-negative-does-not-reach-divide",
        "calculator.is-negative",
        "calculator.divide",
    )
    .unwrap()])
    .unwrap()
}

#[test]
fn project_manifest_is_deterministic_and_covers_provider_obligations_and_root() {
    let fixture = Fixture::new("coverage");
    let options = ProjectAssuranceOptions::default();
    let first = generate(&fixture.manifest(), &options).unwrap();
    let second = generate(&fixture.manifest(), &options).unwrap();
    assert_eq!(first, second);

    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        assert_eq!(derive(&revision, &options)?, first);
        verify_against_revision(&first, &revision, &options)?;
        Ok(())
    })
    .unwrap();
    let expected_root = with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot
            .retain_revision()
            .canonical_workspace_revision()?
            .program_root()?
            .program_root()
            .to_owned())
    })
    .unwrap();

    let payload = &wire(&first)["payload"];
    assert_eq!(
        wire(&first)["schema"],
        "semaprax.project-assurance-manifest.v1"
    );
    assert_eq!(
        payload["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| source["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["src/app.spx", "src/core.spx", "src/tests.spx"],
    );
    assert_eq!(payload["program_root"], expected_root.as_str());
    let obligations = payload["obligations"].as_array().unwrap();
    assert!(obligations
        .iter()
        .any(|obligation| obligation["source_path"] == "src/core.spx"));
    assert!(obligations
        .iter()
        .all(|obligation| obligation["id"].is_string()));
    assert!(obligations
        .windows(2)
        .all(|pair| pair[0]["id"].as_str() <= pair[1]["id"].as_str()));
}

#[test]
fn held_architecture_claim_is_recorded_but_violated_and_unevaluable_claims_refuse() {
    let held = Fixture::new("held-claim");
    let options = ProjectAssuranceOptions::default().with_claims(held_claims());
    let document = generate(&held.manifest(), &options).unwrap();
    let payload = &wire(&document)["payload"];
    assert!(payload["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|obligation| {
            obligation["kind"] == "architecture_law"
                && obligation["declaration_id"] == "calculator.is-negative"
                && obligation["classification"] == "compiler_proved"
        }));
    assert_eq!(
        payload["architecture_claims"]["claims"][0]["status"],
        "held"
    );

    let violated = Fixture::new("violated-claim");
    let claims = ArchitectureClaimSet::new(vec![ArchitectureClaim::forbid_reaches(
        "calculator-main-does-not-reach-add",
        "calculator.app.main",
        "calculator.add",
    )
    .unwrap()])
    .unwrap();
    let options = ProjectAssuranceOptions::default().with_claims(claims);
    assert_code(generate(&violated.manifest(), &options), "SPX-Z101");

    let unevaluable = Fixture::new("unevaluable-claim");
    // Reuse the admitted private-callable Project shape from function_values.rs.
    let manifest = r#"schema = "semaprax.manifest.v1"

[package]
name = "fixture"
version = "0.1.0"

[modules]
entry = "fixture.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["fixture.tests"]

[exports]
web = ["fixture.public"]
"#;
    std::fs::write(unevaluable.manifest(), manifest).unwrap();
    for (path, source) in [
        (
            "src/app.spx",
            r#"module fixture.app;
@id("fixture.increment") fn increment(value:i64)->i64 { value + 1 }
@id("fixture.decrement") fn decrement(value:i64)->i64 { value - 1 }
@id("fixture.main") fn main()->i64 { let callback=if true{decrement}else{increment}; callback(41) }
@id("fixture.public") fn published()->i64 { 0 }
"#,
        ),
        (
            "src/tests.spx",
            "module fixture.tests; @id(\"fixture.tests.main\") fn main()->i64{0}",
        ),
    ] {
        let parsed = semaprax::parse(source, path).unwrap();
        std::fs::write(
            unevaluable.source(path),
            semaprax::format::canonical(&parsed),
        )
        .unwrap();
    }
    let claims = ArchitectureClaimSet::new(vec![ArchitectureClaim::forbid_reaches(
        "no-dynamic-path",
        "fixture.main",
        "fixture.public",
    )
    .unwrap()])
    .unwrap();
    let options = ProjectAssuranceOptions::default().with_claims(claims);
    assert_code(generate(&unevaluable.manifest(), &options), "SPX-Z101");
}

#[test]
fn replay_rejects_cross_revision_and_tampered_documents() {
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    std::fs::write(
        second.source("src/core.spx"),
        std::fs::read_to_string(second.source("src/core.spx"))
            .unwrap()
            .replace("left + right", "left + right + 1"),
    )
    .unwrap();
    let options = ProjectAssuranceOptions::default();
    let document = generate(&first.manifest(), &options).unwrap();
    let tampered = document.replacen("\"payload_digest\"", "\"tampered_payload_digest\"", 1);

    with_authenticated_project(&first.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        assert_code(
            verify_against_revision(&tampered, &revision, &options),
            "SPX-Z104",
        );
        Ok(())
    })
    .unwrap();
    with_authenticated_project(&second.manifest(), |snapshot| {
        assert_code(
            verify_against_revision(&document, &snapshot.retain_revision(), &options),
            "SPX-Z104",
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn snapshot_source_drift_and_obligation_or_output_budgets_fail_closed() {
    let fixture = Fixture::new("drift");
    let options = ProjectAssuranceOptions::default();
    let drift = with_authenticated_project(&fixture.manifest(), |snapshot| {
        std::fs::write(
            fixture.source("src/core.spx"),
            std::fs::read_to_string(fixture.source("src/core.spx"))
                .unwrap()
                .replace("left + right", "left + right + 1"),
        )
        .unwrap();
        generate_from_snapshot(snapshot, &options)
    });
    assert!(drift.is_err());

    let manifest_drift = Fixture::new("manifest-drift");
    let drift = with_authenticated_project(&manifest_drift.manifest(), |snapshot| {
        std::fs::write(
            manifest_drift.manifest(),
            std::fs::read_to_string(manifest_drift.manifest())
                .unwrap()
                .replace("name = \"calculator\"", "name = \"calculator-drift\""),
        )
        .unwrap();
        generate_from_snapshot(snapshot, &options)
    });
    assert!(drift.is_err());

    let bounded = Fixture::new("budgets");
    assert_eq!(
        ProjectAssuranceOptions::new(2_048, 0).unwrap_err().code,
        "SPX-Z101"
    );
    let one_obligation = ProjectAssuranceOptions::new(262_144, 1).unwrap();
    assert_code(generate(&bounded.manifest(), &one_obligation), "SPX-Z102");
    let tight_output = ProjectAssuranceOptions::new(2_048, 65_536).unwrap();
    assert_code(generate(&bounded.manifest(), &tight_output), "SPX-Z102");

    // Issue #271, second half: the refusal must say WHICH budget was
    // exceeded, by how much, and how to raise it. `SPX-Z101` beside it has
    // always stated its valid range ("max_bytes must be between ..."); there
    // is no reason a runtime exhaustion should tell a caller less than an
    // option rejection does. Before this, the message was a bare "exceeds its
    // obligation or output byte budget" and a user who hit it on a freshly
    // scaffolded project had nothing to act on.
    let obligation_error = generate(&bounded.manifest(), &one_obligation).unwrap_err();
    assert!(
        obligation_error[0].message.contains("max_obligations is 1"),
        "the obligation refusal must name the bound it hit: {}",
        obligation_error[0].message
    );
    let byte_error = generate(&bounded.manifest(), &tight_output).unwrap_err();
    assert!(
        byte_error[0].message.contains("max_bytes is 2048"),
        "the byte refusal must name the budget it hit: {}",
        byte_error[0].message
    );
    for error in [&obligation_error, &byte_error] {
        assert!(
            error[0].help.as_deref().is_some_and(
                |help| help.contains("--max-bytes") && help.contains("--max-obligations")
            ),
            "the refusal must name the flags that raise the budget: {:?}",
            error[0].help
        );
    }
}

#[path = "law_set.rs"]
mod law_set;
