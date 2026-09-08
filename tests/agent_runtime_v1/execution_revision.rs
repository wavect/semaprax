use super::agent_lifecycle_v1::proposal;
use super::source_agent_lifecycle::source_module;
use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::{
    compile_source_agent_lifecycle, FixtureRead, LifecycleBudget, LifecycleStatus, LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::execution_revision::{bind_execution_revision, ProgramRootRef};
use semaprax::project::with_authenticated_project;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "spx-execution-roots-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
        let source = source_module(
            "fixture.agent.fn.reduce",
            r#"
@id("fixture.export.payload")
record ExportPayload { @id("fixture.export.payload.bytes") bytes: Bytes, }
@id("fixture.export.build")
fn build(input: borrow Slice<u8>) -> ExportPayload { ExportPayload { bytes: bytes_copy(input) } }
"#,
        );
        let source = source
            .replace(
                "    @id(\"fixture.agent.type.observation.tag\")\n    tag: Bytes,\n",
                "",
            )
            .replace("    let tag = [79u8, 66u8];\n", "")
            .replace("        tag: bytes_copy(array_as_slice(tag)),\n", "");
        let source = semaprax::format::canonical(&semaprax::parse(&source, "src/app.spx").unwrap());
        std::fs::write(path.join("src/app.spx"), source).unwrap();
        std::fs::write(
            path.join("src/tests.spx"),
            "module fixture.tests;\n\n@id(\"fixture.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        std::fs::write(path.join("semaprax.toml"), "schema = \"semaprax.project.v11\"\nname = \"fixture\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"fixture.agent.lifecycle\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"fixture.export.build\"]\ntests = [\"fixture.tests\"]\n").unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn execution_roots_bind_retained_source_and_actual_run() {
    let fixture = Fixture::new();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let foreign_fixture = Fixture::new();
        let foreign_path = foreign_fixture.0.join("src/app.spx");
        let foreign_source = std::fs::read_to_string(&foreign_path)
            .unwrap()
            .replace("fn main() -> i64\n{\n    0", "fn main() -> i64\n{\n    1");
        assert_ne!(
            foreign_source,
            std::fs::read_to_string(&foreign_path).unwrap()
        );
        std::fs::write(&foreign_path, foreign_source).unwrap();
        let foreign =
            with_authenticated_project(&foreign_fixture.0.join("semaprax.toml"), |other| {
                other.retain_revision().program_root()
            })?;
        assert_ne!(foreign.program_root_digest(), root.program_root_digest());
        let source = &project.sources()[0];
        let lifecycle =
            compile_source_agent_lifecycle(source.source(), source.path(), "fixture.agent")?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let proposed = proposal(
            lifecycle.proposal_schema().schema().digest(),
            "5",
            false,
            "1",
        );
        let bind = |path: &str, expected: &str, budget: i64| {
            bind_execution_revision(
                project.clone(),
                ProgramRootRef::V1(&root),
                expected,
                path,
                "fixture.agent",
                &deployment,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget,
                },
                &proposed,
                LifecycleBudget::default(),
            )
        };
        let first = bind("src/app.spx", root.program_root_digest(), 12)?;
        let same = bind("src/app.spx", root.program_root_digest(), 12)?;
        assert_eq!(first.execution_revision(), same.execution_revision());
        let different = bind("src/app.spx", root.program_root_digest(), 13)?;
        assert_ne!(first.instance_root(), different.instance_root());
        assert!(!first.instance_root().canonical_json().contains("alpha"));
        assert_eq!(
            bind("src/missing.spx", root.program_root_digest(), 12)
                .err()
                .unwrap()[0]
                .code,
            "SPX-G583"
        );
        assert_eq!(
            bind("src/app.spx", "sha256:stale", 12).err().unwrap()[0].code,
            "SPX-G583"
        );
        let foreign_error = bind_execution_revision(
            project.clone(),
            ProgramRootRef::V1(&foreign),
            foreign.program_root_digest(),
            "src/app.spx",
            "fixture.agent",
            &deployment,
            LifecycleTask {
                objective: b"alpha".to_vec(),
                budget: 12,
            },
            &proposed,
            LifecycleBudget::default(),
        )
        .err()
        .unwrap();
        assert_eq!(foreign_error[0].code, "SPX-G583");
        assert!(foreign_error[0].message.contains("source-owned segment"));
        let workspace = project.canonical_workspace_revision()?;
        let lock = semaprax::project::render_project_lock(snapshot)?;
        let association =
            root.associate_dependency_lock(snapshot, root.program_root_digest(), &lock)?;
        let interface = semaprax::project::InterfaceArtifactFacts::derive(
            project.clone(),
            project.project_revision(),
            &[semaprax::project::ImageArtifactKind::Npm],
            semaprax::project::MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let contracts = semaprax::project::ContractsAndTestsFacts::derive(
            project.clone(),
            project.project_revision(),
        )?;
        let v2 =
            semaprax::project::ProgramRootV2::derive(&workspace, &root, &interface, &association)?;
        let v3 = semaprax::project::ProgramRootV3::derive(
            &workspace,
            &root,
            &interface,
            &association,
            &contracts,
        )?;
        for program in [ProgramRootRef::V2(&v2), ProgramRootRef::V3(&v3)] {
            let bound = bind_execution_revision(
                project.clone(),
                program,
                program.digest(),
                "src/app.spx",
                "fixture.agent",
                &deployment,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget: 12,
                },
                &proposed,
                LifecycleBudget::default(),
            )?;
            assert_ne!(bound.execution_revision(), first.execution_revision());
            let mut operation = FixtureRead::new(b"observed".to_vec());
            assert_eq!(
                bound
                    .run(&mut operation, &AgentCancellation::new())?
                    .run()
                    .status(),
                LifecycleStatus::Completed
            );
            assert_eq!(operation.calls(), 1);
        }
        let revision = first.execution_revision().clone();
        let mut read = FixtureRead::new(b"observed".to_vec());
        let evidence = first.run(&mut read, &AgentCancellation::new())?;
        assert_eq!(evidence.run().status(), LifecycleStatus::Completed);
        assert_eq!(read.calls(), 1);
        assert_eq!(evidence.execution_revision(), &revision);
        assert!(evidence
            .evidence_root()
            .canonical_json()
            .contains(evidence.run().evidence_digest()));
        Ok(())
    })
    .unwrap();
}

#[path = "execution_revision/iterative.rs"]
mod iterative;
