//! LAW-02: explicit law-source selection and exact retained Project binding.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::assurance_manifest::law_set::{derive_report, LawPolicy, LawSet};
use semaprax::project::{with_authenticated_project, ManifestLayout, ProjectManifest};
use semaprax::query::{self, QueryFilters};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const LAW: &str = "module calculator.laws;\n\n@id(\"calculator.divide.nonzero\")\nlaw contract \"calculator.divide\" requires (right: i64)\n    right != 0\n    evidence theorem_proved;\n";
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
