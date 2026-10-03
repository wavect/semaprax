//! LAW-02: explicit law-source selection and exact retained Project binding.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::assurance_manifest::law_set::{derive_report, LawPolicy, LawSet};
use semaprax::project::{
    with_authenticated_project, ManifestLayout, ProjectCandidate, ProjectManifest, SemanticChange,
};
use semaprax::query::{self, QueryFilters};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const LAW: &str = "module calculator.laws;\n\n@id(\"calculator.divide.nonzero\")\nlaw contract \"calculator.divide\" requires (right: i64)\n    right != 0\n    evidence theorem_proved;\n";
const RELATIONAL: &str = "module calculator.laws;\n\n@id(\"calculator.order.total\")\nlaw relational (left: i64, right: i64)\n    left <= right || right < left\n    evidence smt_proved;\n";
const MANIFEST: &str = "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/core.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n";

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Fixture {
    fn new(law: &str, manifest: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-native-law-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/src");
        for name in ["app.spx", "core.spx", "tests.spx"] {
            std::fs::copy(example.join(name), root.join("src").join(name)).unwrap();
        }
        std::fs::write(root.join("src/contracts.spx"), law).unwrap();
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn law(&self) -> PathBuf {
        self.0.join("src/contracts.spx")
    }
}

#[test]
fn explicit_law_module_is_queryable_open_and_bound_to_exact_project_bytes() {
    let fixture = Fixture::new(LAW, MANIFEST);
    let parsed = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(parsed.layout(), ManifestLayout::TablesV2);
    assert_eq!(parsed.manifest_schema(), "semaprax.manifest.v2");
    assert_eq!(parsed.schema(), "semaprax.project.v1");
    assert_eq!(parsed.law_sources(), ["src/contracts.spx"]);
    assert_eq!(parsed.to_canonical_toml(), MANIFEST);

    let before = with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        assert_eq!(revision.law_modules().len(), 1);
        assert_eq!(
            revision.law_modules()[0].laws[0].law_id,
            "calculator.divide.nonzero"
        );
        let query = query::run_project(
            &revision,
            &QueryFilters {
                kinds: vec!["law".to_owned()],
                ..QueryFilters::default()
            },
        )?;
        let json = query::project_json(&query);
        assert!(json.contains("semaprax.project-query.v2"));
        assert!(json.contains("calculator.divide.nonzero"));
        assert!(json.contains("right != 0"));
        let graph: serde_json::Value = serde_json::from_str(revision.semantic_graph()).unwrap();
        assert_eq!(graph["schema"], "semaprax.project-semantic-graph.v6");
        assert_eq!(
            graph["law_modules"][0]["laws"][0]["law_id"],
            "calculator.divide.nonzero"
        );
        assert_eq!(
            graph["law_dependencies"][0]["declaration_id"],
            "calculator.divide"
        );
        let law_set = LawSet::derive(&revision, "law02.test", revision.law_modules().to_vec())?;
        let policy = LawPolicy::strict(law_set.clone())?;
        let report = derive_report(&revision, &law_set, &policy)?;
        assert!(report.contains("calculator.divide.nonzero"));
        assert!(report.contains("awaiting_evidence"));
        let root = revision.program_root()?;
        Ok((
            revision.project_revision().to_owned(),
            root.program_root_digest().to_owned(),
        ))
    })
    .unwrap();

    std::fs::write(
        fixture.law(),
        LAW.replace(
            "module calculator.laws;",
            "module calculator.laws;\n// authored note",
        ),
    )
    .unwrap();
    let after = with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let root = revision.program_root()?;
        Ok((
            revision.project_revision().to_owned(),
            root.program_root_digest().to_owned(),
        ))
    })
    .unwrap();
    assert_ne!(before.0, after.0);
    assert_ne!(before.1, after.1);
}

#[test]
fn hidden_missing_and_misassociated_laws_fail_closed() {
    let fixture = Fixture::new(LAW, MANIFEST);
    let hidden = MANIFEST.replace("law_sources = [\"src/contracts.spx\"]", "law_sources = []");
    std::fs::write(fixture.manifest(), hidden).unwrap();
    assert!(with_authenticated_project(&fixture.manifest(), |snapshot| snapshot.check()).is_err());

    std::fs::write(fixture.manifest(), MANIFEST).unwrap();
    let invalid = LAW.replace("calculator.divide\"", "calculator.absent\"");
    std::fs::write(fixture.law(), invalid).unwrap();
    let errors =
        with_authenticated_project(&fixture.manifest(), |snapshot| snapshot.check()).unwrap_err();
    assert_eq!(errors[0].code, "SPX-LW110");

    assert_eq!(
        ProjectManifest::parse(&MANIFEST.replace(
            "law_sources = [\"src/contracts.spx\"]",
            "law_sources = [\"src/absent.spx\"]",
        ))
        .unwrap_err()[0]
            .code,
        "SPX-J100"
    );
    assert_eq!(
        ProjectManifest::parse(&MANIFEST.replace("semaprax.manifest.v2", "semaprax.manifest.v1"))
            .unwrap_err()[0]
            .code,
        "SPX-J120"
    );
}

#[test]
fn candidate_preserves_selected_law_bytes_while_editing_executable_source() {
    let fixture = Fixture::new(LAW, MANIFEST);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let base = snapshot.retain_revision();
        let candidate = ProjectCandidate::open(base.clone(), base.project_revision())?;
        let change = SemanticChange::new(
            base.project_revision(),
            &serde_json::json!({"kind":"rename_declaration","target":"calculator.multiply","name":"product"}),
        )?;
        let updated = candidate.apply(candidate.candidate_digest(), &change)?;
        let law = updated
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == "src/contracts.spx")
            .unwrap();
        assert_eq!(law.source(), LAW);
        assert_eq!(updated.revision().law_modules(), base.law_modules());
        assert_ne!(updated.revision().project_revision(), base.project_revision());
        Ok(())
    })
    .unwrap();
}

#[test]
fn independent_scalar_relation_is_queryable_and_remains_open_without_proof() {
    let fixture = Fixture::new(RELATIONAL, MANIFEST);
    let parsed = semaprax::native_law_source::parse(RELATIONAL, "src/contracts.spx").unwrap();
    assert_eq!(semaprax::native_law_source::canonical(&parsed), RELATIONAL);
    let replay = semaprax::native_law_source::parse(
        &semaprax::native_law_source::canonical(&parsed),
        "src/contracts.spx",
    )
    .unwrap();
    assert_eq!(parsed, replay);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let query = query::run_project(
            &revision,
            &QueryFilters {
                kinds: vec!["law".to_owned()],
                ..QueryFilters::default()
            },
        )?;
        let json = query::project_json(&query);
        assert!(json.contains("calculator.order.total"));
        assert!(json.contains("left <= right || right < left"));
        let graph: serde_json::Value = serde_json::from_str(revision.semantic_graph()).unwrap();
        assert_eq!(
            graph["law_modules"][0]["laws"][0]["law_id"],
            "calculator.order.total"
        );
        assert_eq!(graph["law_dependencies"], serde_json::json!([]));
        let law_set = LawSet::derive(
            &revision,
            "law02.relational",
            revision.law_modules().to_vec(),
        )?;
        let policy = LawPolicy::strict(law_set.clone())?;
        let report = derive_report(&revision, &law_set, &policy)?;
        assert!(report.contains("scalar_relational_proposition_has_no_verified_proof_attachment"));
        assert!(report.contains("\"accepted\":false"));
        Ok(())
    })
    .unwrap();
    let query = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "query",
            fixture.manifest().to_str().unwrap(),
            "--kind",
            "law",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        query.status.success(),
        "{}",
        String::from_utf8_lossy(&query.stderr)
    );
    let cli_query: serde_json::Value = serde_json::from_slice(&query.stdout).unwrap();
    assert_eq!(cli_query["matches"][0]["id"], "calculator.order.total");
    assert!(cli_query["matches"][0]["signature"]
        .as_str()
        .unwrap()
        .contains("left <= right || right < left"));
    let graph = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["graph", fixture.manifest().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        graph.status.success(),
        "{}",
        String::from_utf8_lossy(&graph.stderr)
    );
    let cli_graph: serde_json::Value = serde_json::from_slice(&graph.stdout).unwrap();
    assert_eq!(
        cli_graph["law_modules"][0]["laws"][0]["law_id"],
        "calculator.order.total"
    );
    assert_eq!(
        cli_graph["law_modules"][0]["laws"][0]["selector"]["proposition"],
        "left <= right || right < left"
    );

    let false_law = RELATIONAL.replace("left <= right || right < left", "false");
    let false_fixture = Fixture::new(&false_law, MANIFEST);
    with_authenticated_project(&false_fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let law_set = LawSet::derive(&revision, "law02.false", revision.law_modules().to_vec())?;
        let policy = LawPolicy::strict(law_set.clone())?;
        let report = derive_report(&revision, &law_set, &policy)?;
        assert!(report.contains("\"accepted\":false"));
        assert!(report.contains("\"open\":1"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn relational_calls_and_undeclared_binders_are_refused() {
    for (bad, code) in [
        (
            RELATIONAL.replace("left <= right || right < left", "read(left)"),
            "SPX-LW110",
        ),
        (
            RELATIONAL.replace("left <= right || right < left", "third == left"),
            "SPX-LW110",
        ),
        (
            RELATIONAL.replace("left <= right || right < left", "forall(left)"),
            "SPX-LW110",
        ),
        (
            RELATIONAL.replace("left <= right || right < left", "io.read(left)"),
            "SPX-LW110",
        ),
        (
            format!("{RELATIONAL}\nproof law \"a-different-law\";"),
            "SPX-LW110",
        ),
    ] {
        assert_eq!(
            semaprax::native_law_source::parse(&bad, "src/contracts.spx")
                .unwrap_err()
                .code,
            code
        );
    }
    let declaration = RELATIONAL.split("\n\n").nth(1).unwrap();
    let duplicate = format!("{RELATIONAL}\n{declaration}");
    assert_eq!(
        semaprax::native_law_source::parse(&duplicate, "src/contracts.spx")
            .unwrap_err()
            .code,
        "SPX-LW110"
    );
}

#[test]
fn complete_repository_example_checks_and_exposes_both_laws() {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/native-law-project/semaprax.toml");
    with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        assert_eq!(revision.law_modules().len(), 1);
        assert_eq!(revision.law_modules()[0].laws.len(), 2);
        let graph: serde_json::Value = serde_json::from_str(revision.semantic_graph()).unwrap();
        assert_eq!(graph["schema"], "semaprax.project-semantic-graph.v6");
        assert_eq!(
            graph["law_modules"][0]["laws"][0]["law_id"],
            "native-law.add.right-nonnegative"
        );
        assert_eq!(
            graph["law_modules"][0]["laws"][1]["law_id"],
            "native-law.order.total"
        );
        Ok(())
    })
    .unwrap();
}
