use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ContractsAndTestsFacts, ImageArtifactKind,
    InterfaceArtifactFacts, ProgramRootV3, SemanticWorkspaceRevision,
    CONTRACTS_AND_TESTS_FACTS_SCHEMA, MAX_IMAGE_ARTIFACT_BUILD_BYTES, MAX_PROGRAM_ROOT_V3_BYTES,
    PROGRAM_ROOT_V3_SCHEMA,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-program-root-v3-{label}-{}-{}",
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
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}

fn remint_v3(value: &mut Value) -> String {
    value
        .as_object_mut()
        .unwrap()
        .remove("program_root_v3_digest");
    let identity = canonical(value.clone());
    let mut digest = Sha256::new();
    digest.update(b"semaprax.program-root.digest.v3\0");
    digest.update((identity.len() as u64).to_le_bytes());
    digest.update(identity.as_bytes());
    value["program_root_v3_digest"] = Value::String(format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    ));
    canonical(value.clone())
}

#[test]
fn v3_retains_v1_v2_bytes_and_appends_exact_contract_test_facts() {
    let fixture = Fixture::new("exact");
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let default_workspace = snapshot.canonical_workspace_revision()?;
        let default_workspace_bytes = default_workspace.to_json().to_owned();
        let base_root = default_workspace.program_root()?;
        let base_root_bytes = base_root.to_json().to_owned();
        let lock = render_project_lock(snapshot)?;
        let association = base_root.associate_dependency_lock(
            snapshot,
            base_root.program_root_digest(),
            &lock,
        )?;
        let interface_facts = InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&super::program_root_v2::definition()],
        )?;
        let v2 = semaprax::project::ProgramRootV2::derive(
            &workspace,
            &base_root,
            &interface_facts,
            &association,
        )?;
        let v2_bytes = v2.to_json().to_owned();
        let facts = ContractsAndTestsFacts::derive(revision.clone(), revision.project_revision())?;
        let v3 = ProgramRootV3::derive(
            &workspace,
            &base_root,
            &interface_facts,
            &association,
            &facts,
        )?;

        assert_eq!(v3.segments().len(), 12);
        assert_eq!(v3.program_root_v2_digest(), v2.program_root_v2_digest());
        assert_eq!(v3.segments()[..11], v2.segments()[..]);
        for (retained, original) in v3.segments()[..11].iter().zip(v2.segments()) {
            assert_eq!(retained.to_json(), original.to_json());
        }
        let extension = &v3.segments()[11];
        assert_eq!(extension.kind(), "contracts_and_tests_facts");
        assert_eq!(extension.node_schema(), CONTRACTS_AND_TESTS_FACTS_SCHEMA);
        assert_eq!(extension.node_digest(), facts.facts_digest());
        assert_eq!(v3.relationships(), v2.relationships());
        assert!(v3.relationships().iter().all(|relationship| {
            relationship.binding() == "unbound" && relationship.digest().is_none()
        }));
        let value: Value = serde_json::from_str(v3.to_json()).unwrap();
        assert_eq!(value["schema"], PROGRAM_ROOT_V3_SCHEMA);
        assert!(value["nonclaims"]
            .as_array()
            .unwrap()
            .iter()
            .any(|claim| claim
                == "no_filesystem_network_execution_deployment_publication_or_commit_authority"));
        assert_eq!(
            ProgramRootV3::replay(
                &workspace,
                &base_root,
                &interface_facts,
                &association,
                &facts,
                v3.program_root_v3_digest(),
                v3.to_json().as_bytes(),
            )?,
            v3
        );
        assert_eq!(default_workspace.to_json(), default_workspace_bytes);
        assert_eq!(base_root.to_json(), base_root_bytes);
        assert_eq!(v2.to_json(), v2_bytes);
        Ok(())
    })
    .unwrap();
}

#[test]
fn v3_rejects_cross_subject_stale_reminted_mutated_and_over_bound_inputs() {
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    let core = second.0.join("src/core.spx");
    std::fs::write(
        &core,
        std::fs::read_to_string(&core)
            .unwrap()
            .replace("left + right", "left + right + 1"),
    )
    .unwrap();
    let mut other_facts = None;
    with_authenticated_project(&second.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        other_facts = Some(ContractsAndTestsFacts::derive(
            revision.clone(),
            revision.project_revision(),
        )?);
        Ok(())
    })
    .unwrap();

    with_authenticated_project(&first.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let base_workspace = snapshot.canonical_workspace_revision()?;
        let base_root = base_workspace.program_root()?;
        let lock = render_project_lock(snapshot)?;
        let association = base_root.associate_dependency_lock(
            snapshot,
            base_root.program_root_digest(),
            &lock,
        )?;
        let interface_facts = InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&super::program_root_v2::definition()],
        )?;
        let facts = ContractsAndTestsFacts::derive(revision.clone(), revision.project_revision())?;
        assert_code(
            ProgramRootV3::derive(
                &workspace,
                &base_root,
                &interface_facts,
                &association,
                other_facts.as_ref().unwrap(),
            ),
            "SPX-G581",
        );
        let v3 = ProgramRootV3::derive(
            &workspace,
            &base_root,
            &interface_facts,
            &association,
            &facts,
        )?;
        let stale = format!("sha256:{}", "0".repeat(64));
        assert_code(
            ProgramRootV3::replay(
                &workspace,
                &base_root,
                &interface_facts,
                &association,
                &facts,
                &stale,
                v3.to_json().as_bytes(),
            ),
            "SPX-G581",
        );

        let mut reordered: Value = serde_json::from_str(v3.to_json()).unwrap();
        reordered["segments"].as_array_mut().unwrap().swap(10, 11);
        let reordered = remint_v3(&mut reordered);
        let digest = serde_json::from_str::<Value>(&reordered).unwrap()["program_root_v3_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_code(
            ProgramRootV3::replay(
                &workspace,
                &base_root,
                &interface_facts,
                &association,
                &facts,
                &digest,
                reordered.as_bytes(),
            ),
            "SPX-G580",
        );

        let mut mutated: Value = serde_json::from_str(v3.to_json()).unwrap();
        mutated["segments"][11]["node_digest"] = Value::String(stale.clone());
        let mutated = remint_v3(&mut mutated);
        let digest = serde_json::from_str::<Value>(&mutated).unwrap()["program_root_v3_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_code(
            ProgramRootV3::replay(
                &workspace,
                &base_root,
                &interface_facts,
                &association,
                &facts,
                &digest,
                mutated.as_bytes(),
            ),
            "SPX-G580",
        );
        assert_code(
            ProgramRootV3::replay(
                &workspace,
                &base_root,
                &interface_facts,
                &association,
                &facts,
                v3.program_root_v3_digest(),
                &vec![b' '; MAX_PROGRAM_ROOT_V3_BYTES + 1],
            ),
            "SPX-G580",
        );
        Ok(())
    })
    .unwrap();
}
