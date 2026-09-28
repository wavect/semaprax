//! Checked Agent metadata projections; no Agent runtime/provider execution.
#[path = "agent_execution_fixture.rs"]
mod fixture;
use semaprax::project::{with_authenticated_project, ProgramRoot, ProjectRevision};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);
impl Workspace {
    fn new(source: &str, spare: Option<&str>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-agent-execution-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let value = Self(root.canonicalize().unwrap());
        value.write("src/app.spx", source);
        value.write(
            "src/tests.spx",
            "module fixture.tests; @id(\"fixture.tests.main\") fn main()->i64 {0}",
        );
        if let Some(spare) = spare {
            value.write("src/spare.spx", spare);
        }
        let sources = if spare.is_some() {
            "[\"src/app.spx\",\"src/spare.spx\",\"src/tests.spx\"]"
        } else {
            "[\"src/app.spx\",\"src/tests.spx\"]"
        };
        std::fs::write(value.0.join("semaprax.toml"), format!(
            "schema = \"semaprax.manifest.v1\"\n[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n[modules]\nentry = \"fixture.app\"\nsources = {sources}\ntests = [\"fixture.tests\"]\n[exports]\nweb = [\"fixture.public\"]\n")).unwrap();
        value
    }
    fn write(&self, path: &str, source: &str) {
        let program = semaprax::parse(source, self.0.join(path)).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn workspace_v4_preserves_each_dynamic_base_and_exact_source_agent_rows() {
    for version in 1..=3 {
        let source = fixture::source(true, true);
        let source = if version == 1 {
            source
        } else {
            fixture::protocol(&source, version == 3)
        };
        let workspace = Workspace::new(&source, None);
        let revision = workspace.revision();
        let graph: Value = serde_json::from_str(revision.semantic_graph()).unwrap();
        assert_eq!(graph["schema"], "semaprax.workspace-graph.v4");
        let section = &graph["agent_execution"];
        assert_eq!(
            section["base_schema"],
            format!("semaprax.workspace-graph.v{version}")
        );
        assert_eq!(section["authority"], "none");
        let rows = section["agents"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["module"], "fixture.app");
        assert_eq!(rows[0]["path"], "src/app.spx");
        let parsed = semaprax::check(&source, "src/app.spx").unwrap();
        let canonical = semaprax::format::canonical(&parsed);
        let repeated = semaprax::check(&canonical, "src/app.spx").unwrap();
        assert_eq!(semaprax::format::canonical(&repeated), canonical);
        let source_graph: Value =
            serde_json::from_str(&semaprax::graph::to_json(&repeated).unwrap()).unwrap();
        assert_eq!(
            rows[0]["agent"],
            source_graph["agent_execution"]["agents"][0]["agent"]
        );
        assert_eq!(
            rows[0]["operations"],
            source_graph["agent_execution"]["agents"][0]["operations"]
        );
        assert_eq!(
            rows[0]["model_wait"],
            source_graph["agent_execution"]["agents"][0]["model_wait"]
        );
        assert_eq!(rows[0]["operations"].as_array().unwrap().len(), 4);
        if version >= 2 {
            assert!(graph.get("session_protocols").is_some());
        }
        if version == 3 {
            assert!(graph.get("session_protocol_follows").is_some());
        }
    }
}

#[test]
fn legacy_source_retains_its_base_schema_without_execution_facts() {
    let source = fixture::source(false, false);
    let workspace = Workspace::new(&source, None);
    let first = workspace.revision();
    let second = workspace.revision();
    assert_eq!(first.semantic_graph(), second.semantic_graph());
    let graph: Value = serde_json::from_str(first.semantic_graph()).unwrap();
    assert_eq!(graph["schema"], "semaprax.workspace-graph.v1");
    assert!(graph.get("agent_execution").is_none());
    let source_graph: Value = serde_json::from_str(
        &semaprax::graph::to_json(&semaprax::check(&source, "src/app.spx").unwrap()).unwrap(),
    )
    .unwrap();
    assert!(source_graph.get("agent_execution").is_none());
    assert_eq!(first.entry_program().agents.len(), 1);
}

#[test]
fn unreachable_opted_in_module_does_not_require_unlinked_agent_functions() {
    let source = "module fixture.app; @id(\"fixture.main\") fn main()->i64 {0} @id(\"fixture.public\") fn published()->i64 {0}";
    let spare = fixture::source(true, true).replace("fixture.", "spare.");
    let workspace = Workspace::new(source, Some(&spare));
    let revision = workspace.revision();
    assert!(revision.entry_program().agents.is_empty());
    assert!(!revision
        .entry_program()
        .functions
        .iter()
        .any(|function| function.id.as_str().starts_with("spare.")));
    let graph: Value = serde_json::from_str(revision.semantic_graph()).unwrap();
    assert!(graph.get("agent_execution").is_none());
    assert_eq!(graph["schema"], "semaprax.workspace-graph.v1");
}

#[test]
fn program_root_binds_helper_and_body_metadata_but_semantic_program_ignores_comments() {
    let source = fixture::source(true, true);
    let workspace = Workspace::new(&source, None);
    let original = workspace.revision().canonical_workspace_revision().unwrap();
    let root = original.program_root().unwrap();
    for changed in [
        source.replace(
            "propose = \"fixture.wait\"",
            "propose = \"fixture.wait.other\"",
        ),
        source.replace("value + 1", "value + 2"),
    ] {
        assert_ne!(changed, source);
        workspace.write("src/app.spx", &changed);
        let changed = workspace.revision().canonical_workspace_revision().unwrap();
        assert_ne!(
            original.semantic_program().digest(),
            changed.semantic_program().digest()
        );
        assert!(ProgramRoot::replay(
            &changed,
            root.program_root_digest(),
            root.to_json().as_bytes()
        )
        .is_err());
    }
    workspace.write("src/app.spx", &source);
    let path = workspace.0.join("src/app.spx");
    let canonical = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, format!("// source-only comment\n{canonical}")).unwrap();
    let commented = workspace.revision().canonical_workspace_revision().unwrap();
    assert_eq!(
        original.semantic_program().digest(),
        commented.semantic_program().digest()
    );
    assert_ne!(
        root.program_root_digest(),
        commented.program_root().unwrap().program_root_digest()
    );
}

#[test]
fn web_observe_subset_omits_opted_in_metadata_and_keeps_legacy_inventory() {
    for embedded in [false, true] {
        let workspace = Workspace::new(&fixture::source(embedded, embedded), None);
        let path = workspace.0.join("semaprax.toml");
        let manifest = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            manifest.replace("fixture.public", "fixture.agent.fn.observe"),
        )
        .unwrap();
        let revision = workspace.revision();
        let web = revision.public_api_program();
        assert!(web
            .functions
            .iter()
            .any(|function| function.id.as_str() == "fixture.agent.fn.observe"));
        assert!(!web
            .functions
            .iter()
            .any(|function| function.id.as_str() == "fixture.agent.fn.initialize"));
        assert_eq!(web.agents.len(), usize::from(!embedded));
        assert_eq!(revision.entry_program().agents.len(), 1);
    }
}

#[test]
fn unreachable_agent_duplicate_identity_is_checked_before_selection() {
    let source = fixture::source(true, true);
    let spare = fixture::source(true, true)
        .replace("fixture.", "spare.")
        .replace("@id(\"spare.agent\")", "@id(\"fixture.agent\")");
    let workspace = Workspace::new(&source, Some(&spare));
    let errors =
        with_authenticated_project(&workspace.0.join("semaprax.toml"), |_| Ok(())).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G559"),
        "{errors:?}"
    );
}
