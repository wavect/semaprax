//! Checked Project admission for the V2 Everyday Agent sibling. This test
//! deliberately does not construct a provider, candidate, or repair session.

use std::path::{Path, PathBuf};

use semaprax::agent_lifecycle::iterative::compile_project_agent_lifecycle_v2;
use semaprax::agent_runtime_v2::OfflineRepairEnvelope;
use semaprax::project::{with_authenticated_project, CandidateTestPolicy, ProjectExecutionOptions};

const AGENT_ID: &str = "everyday.v2.agent";
const SOURCE_PATH: &str = "src/agent.spx";
const STEP_ID: &str = "everyday.v2.agent.type.step";

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/everyday-agent-v2-project")
}

#[test]
fn everyday_v2_agent_is_authenticated_and_compiles_as_a_linked_project_lifecycle() {
    let manifest = project_root().join("semaprax.toml");
    with_authenticated_project(&manifest, |snapshot| {
        let project = snapshot.retain_revision();
        let source = project
            .sources()
            .iter()
            .find(|source| source.path() == SOURCE_PATH)
            .expect("the V2 Agent source is retained by the Project");
        assert!(source.source().contains(r#"\"provider_id\":\"opencode\""#));
        assert!(source
            .source()
            .contains(r#"\"model_id\":\"muse-spark-1.3-contributor-free\""#));
        assert!(source
            .source()
            .contains(r#"\"required_locality\":\"remote_allowed\""#));
        assert!(!source.source().contains("fake.local"));

        let lifecycle =
            compile_project_agent_lifecycle_v2(&project, SOURCE_PATH, AGENT_ID, STEP_ID)?;
        let document: serde_json::Value = serde_json::from_str(lifecycle.canonical_json())
            .expect("linked lifecycle is canonical JSON");
        assert_eq!(document["schema"], "semaprax.agent-iterative-lifecycle.v3");
        assert_eq!(document["agent_id"], AGENT_ID);
        assert_eq!(document["step"]["type"], STEP_ID);
        assert!(document["linked_source"].is_object());
        assert!(lifecycle
            .proposal_schema()
            .schema()
            .digest()
            .starts_with("sha256:"));

        let baseline = project.execute_test(&ProjectExecutionOptions::default())?;
        assert!(
            !baseline.command_succeeded(),
            "the committed target is the intentionally failing repair subject"
        );

        let preview = OfflineRepairEnvelope::new(project.clone(), "everyday.v2.repair.target")?
            .preview(42, false)?;
        let policy = CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap();
        let candidate = preview
            .candidate()
            .execute_tests(preview.candidate().candidate_digest(), &policy)?;
        assert!(
            candidate.passed(),
            "the fixed target passes the immutable manifest test closure"
        );
        Ok(())
    })
    .unwrap();
}
