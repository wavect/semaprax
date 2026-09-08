//! Internal callable bodies retain scalar Project signatures and exact roots.
use semaprax::project::{with_authenticated_project, ProgramRoot, ProjectRevision};

#[path = "function_values/closures.rs"]
mod closures;
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
        std::fs::write(root.join("semaprax.toml"),"schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"fixture.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"fixture.tests\"]\n\n[exports]\nweb = [\"fixture.public\"]\n").unwrap();
        Self { root, source }
    }
    fn capturing_closure() -> Self {
        let mut fixture = Self::new("increment");
        fixture.source = r#"
module fixture.app;
@id("fixture.make") fn make(offset:i64)->fn(i64)->i64 { fn(value:i64)->i64 { offset + value } }
@id("fixture.main") fn main()->i64 { let callback=make(40); callback(2) }
@id("fixture.public") fn published()->i64 { 0 }
"#
        .to_owned();
        let path = fixture.root.join("src/app.spx");
        let parsed = semaprax::parse(&fixture.source, &path).unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
        fixture
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

#[test]
fn internal_function_values_and_generic_instances_share_one_checked_closure() {
    let fixture = Fixture::new("decrement");
    let source = fixture
        .source
        .replace("callback(41)", "callback(identity<i64>(41))")
        + "\n@id(\"fixture.identity\") fn identity<T>(value:T)->T {value}\n";
    let path = fixture.root.join("src/app.spx");
    let checked = semaprax::check(&source, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&checked)).unwrap();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    let closures = payload["payload"]["checked_callable_closures"]
        .as_array()
        .unwrap();
    let entry = closures
        .iter()
        .find(|closure| closure["role"] == "entry")
        .unwrap();
    let graph: serde_json::Value = serde_json::from_str(entry["graph"].as_str().unwrap()).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v36");
    assert!(!revision.entry_program().function_instances.is_empty());
    let graph_text = entry["graph"].as_str().unwrap();
    assert!(graph_text.contains("fixture.identity"));
    assert!(graph_text.contains("candidate_targets"));
    assert_eq!(
        payload["schema"],
        "semaprax.semantic-workspace-revision.semantic-program.v3"
    );
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
}

#[test]
fn internal_function_values_uninstantiated_template_retains_v36_source_metadata() {
    let fixture = Fixture::new("increment");
    let source = r#"
module fixture.app;
@id("fixture.unused") fn unused<T>(values:own Vec<T>,callback:fn(T)->T)->T {
    vec_get<T>(values,0usize)
}
@id("fixture.main") fn main()->i64 {0}
@id("fixture.public") fn published()->i64 {0}
"#;
    let path = fixture.root.join("src/app.spx");
    let checked = semaprax::check(source, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&checked)).unwrap();
    let revision = fixture.revision();
    assert!(revision.entry_program().function_instances.is_empty());
    let workspace = revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().schema(),
        "semaprax.semantic-workspace-revision.semantic-program.v4"
    );
    assert!(workspace
        .semantic_program()
        .to_json()
        .contains("semaprax.graph.v36"));
    let semantic: serde_json::Value =
        serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    let source_closures = semantic["payload"]["checked_source_callable_closures"]
        .as_array()
        .unwrap();
    assert_eq!(source_closures.len(), 1);
    assert_eq!(source_closures[0]["path"], "src/app.spx");
    assert_eq!(
        source_closures[0]["omitted_callable_templates"],
        serde_json::json!(["fixture.unused"])
    );
    let source_graph: serde_json::Value =
        serde_json::from_str(source_closures[0]["graph"].as_str().unwrap()).unwrap();
    assert_eq!(source_graph["schema"], "semaprax.graph.v36");
    assert_eq!(
        semaprax::project::SemanticWorkspaceRevision::replay(
            &revision,
            workspace.workspace_revision(),
            workspace.to_json().as_bytes()
        )
        .unwrap(),
        workspace
    );
    std::fs::write(
        &path,
        format!(
            "// projection-only comment\n{}",
            semaprax::format::canonical(&checked)
        ),
    )
    .unwrap();
    let commented_revision = fixture.revision();
    let commented = commented_revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().digest(),
        commented.semantic_program().digest()
    );
    assert_eq!(workspace.semantic_digest(), commented.semantic_digest());
    assert_ne!(
        workspace.source_projection_digest(),
        commented.source_projection_digest()
    );
    assert_ne!(
        workspace.program_root().unwrap().program_root_digest(),
        commented.program_root().unwrap().program_root_digest()
    );
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
}

#[test]
fn unused_generic_closure_template_binds_graph_v37_semantic_program_v5_and_program_root() {
    let fixture = Fixture::new("increment");
    let source = r#"
module fixture.app;
@id("fixture.fill") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        let callback=fn(item:T)->T{replacement};
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        index<length
    }
    output
}
@id("fixture.main") fn main()->i64 {0}
@id("fixture.public") fn published()->i64 {0}
"#;
    let path = fixture.root.join("src/app.spx");
    let checked = semaprax::check(source, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&checked)).unwrap();
    let revision = fixture.revision();
    assert!(revision.entry_program().function_instances.is_empty());
    assert!(
        !semaprax::hir::closure::requires_closure_projection(revision.entry_program()),
        "an omitted template has no executable closure product"
    );
    let workspace = revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().schema(),
        "semaprax.semantic-workspace-revision.semantic-program.v5"
    );
    let semantic: serde_json::Value =
        serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    let source_closures = semantic["payload"]["checked_source_callable_closures"]
        .as_array()
        .unwrap();
    assert_eq!(source_closures.len(), 1);
    assert_eq!(
        source_closures[0]["omitted_callable_templates"],
        serde_json::json!(["fixture.fill"])
    );
    let graph: serde_json::Value =
        serde_json::from_str(source_closures[0]["graph"].as_str().unwrap()).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v37");
    let definitions = graph["template_closure_definitions"].as_array().unwrap();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["template"], "fixture.fill");
    assert_eq!(definitions[0]["signature"]["kind"], "function");
    assert_eq!(definitions[0]["body"]["kind"], "block");
    assert_eq!(definitions[0]["body"]["tail"]["kind"], "place");
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
    let forged = workspace.to_json().replacen(
        "\\\"kind\\\":\\\"closure\\\"",
        "\\\"kind\\\":\\\"function_reference\\\"",
        1,
    );
    assert_ne!(forged, workspace.to_json());
    assert!(semaprax::project::SemanticWorkspaceRevision::replay(
        &revision,
        workspace.workspace_revision(),
        forged.as_bytes()
    )
    .is_err());
    let changed_source = source.replace("fn(item:T)->T{replacement}", "fn(item:T)->T{item}");
    let changed = semaprax::check(&changed_source, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&changed)).unwrap();
    let changed_workspace = fixture.revision().canonical_workspace_revision().unwrap();
    assert_ne!(
        workspace.semantic_program().digest(),
        changed_workspace.semantic_program().digest(),
        "the checked symbolic closure body participates in SemanticProgram replay"
    );
    assert!(ProgramRoot::replay(
        &changed_workspace,
        root.program_root_digest(),
        root.to_json().as_bytes()
    )
    .is_err());
    std::fs::write(&path, semaprax::format::canonical(&checked)).unwrap();
    std::fs::write(
        &path,
        format!(
            "// source projection only\n{}",
            semaprax::format::canonical(&checked)
        ),
    )
    .unwrap();
    let commented = fixture.revision().canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().digest(),
        commented.semantic_program().digest()
    );
    assert_ne!(
        root.program_root_digest(),
        commented.program_root().unwrap().program_root_digest()
    );
}
