//! Internal callable bodies retain scalar Project signatures and exact roots.
use semaprax::project::{with_authenticated_project, ProgramRoot, ProjectRevision};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    source: String,
}
impl Fixture {
    fn new(target: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-function-value-workspace-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let source = format!(
            r#"
module fixture.app;
@id("fixture.increment") fn increment(value:i64)->i64{{value+1}}
@id("fixture.decrement") fn decrement(value:i64)->i64{{value-1}}
@id("fixture.main") fn main()->i64{{let callback=if true{{{target}}}else{{increment}};callback(41)}}
@id("fixture.public") fn published()->i64{{0}}
"#
        );
        for (path, text) in [
            ("src/app.spx", source.as_str()),
            (
                "src/tests.spx",
                "module fixture.tests; @id(\"fixture.tests.main\") fn main()->i64{0}",
            ),
        ] {
            let parsed = semaprax::parse(text, root.join(path)).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        std::fs::write(root.join("semaprax.toml"),"schema = \"semaprax.manifest.v1\"\n[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n[modules]\nentry = \"fixture.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"fixture.tests\"]\n[exports]\nweb = [\"fixture.public\"]\n").unwrap();
        Self { root, source }
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
fn internal_function_values_workspace_graph_and_program_root_replay_exact_source() {
    let fixture = Fixture::new("decrement");
    let parsed = semaprax::check(&fixture.source, fixture.root.join("src/app.spx")).unwrap();
    let graph = semaprax::graph::to_json(&parsed).unwrap();
    assert!(graph.contains("semaprax.graph.v36"));
    assert!(graph.contains("\"candidate_targets\":[\"fixture.decrement\",\"fixture.increment\"]"));
    semaprax::graph::verify_json(&parsed, &graph).unwrap();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        semaprax::project::SemanticWorkspaceRevision::replay(
            &revision,
            workspace.workspace_revision(),
            workspace.to_json().as_bytes()
        )
        .unwrap(),
        workspace
    );
    let semantic = workspace.semantic_program();
    assert_eq!(
        semantic.schema(),
        "semaprax.semantic-workspace-revision.semantic-program.v3"
    );
    let semantic_value: serde_json::Value = serde_json::from_str(semantic.to_json()).unwrap();
    let closures = semantic_value["payload"]["checked_callable_closures"]
        .as_array()
        .unwrap();
    let entry = closures
        .iter()
        .find(|closure| closure["role"] == "entry")
        .unwrap();
    let retained_graph = entry["graph"].as_str().unwrap();
    assert!(retained_graph.contains("semaprax.graph.v36"));
    assert!(retained_graph
        .contains("\"candidate_targets\":[\"fixture.decrement\",\"fixture.increment\"]"));
    assert!(semantic_value["payload"]
        .get("generic_instance_closures")
        .is_none());
    let root = workspace.program_root().unwrap();
    assert_eq!(
        ProgramRoot::replay(
            &workspace,
            root.program_root_digest(),
            root.to_json().as_bytes()
        )
        .unwrap(),
        root
    );
    assert_eq!(
        revision
            .canonical_workspace_revision()
            .unwrap()
            .program_root()
            .unwrap(),
        root
    );
    let linked = revision.entry_program();
    assert_eq!(
        semaprax::hir::function_value::target_universe(linked).len(),
        2
    );
    assert!(linked.functions.iter().all(|f| !matches!(
        f.return_type,
        semaprax::hir::ResolvedType::Function { .. }
    ) && f
        .params
        .iter()
        .all(|p| !matches!(p.ty, semaprax::hir::ResolvedType::Function { .. }))));
}
#[test]
fn internal_function_values_changed_target_rejects_cross_paired_roots() {
    let first = Fixture::new("decrement");
    let second = Fixture::new("increment");
    let first_revision = first.revision();
    let second_revision = second.revision();
    let first_workspace = first_revision.canonical_workspace_revision().unwrap();
    let second_workspace = second_revision.canonical_workspace_revision().unwrap();
    assert_ne!(
        first_workspace.source_projection_digest(),
        second_workspace.source_projection_digest()
    );
    assert_ne!(
        first_workspace.semantic_program().digest(),
        second_workspace.semantic_program().digest()
    );
    let first_root = first_workspace.program_root().unwrap();
    let second_root = second_workspace.program_root().unwrap();
    assert_ne!(
        first_root.program_root_digest(),
        second_root.program_root_digest()
    );
    assert!(ProgramRoot::replay(
        &second_workspace,
        first_root.program_root_digest(),
        first_root.to_json().as_bytes()
    )
    .is_err());
    assert!(ProgramRoot::replay(
        &first_workspace,
        second_root.program_root_digest(),
        second_root.to_json().as_bytes()
    )
    .is_err());
}
