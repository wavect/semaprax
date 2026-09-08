//! Workspace replay coverage for private Copy-scalar closure snapshots.

use super::*;

#[test]
fn capturing_closures_bind_semantic_program_v5_graph_v37_and_exact_roots() {
    let fixture = Fixture::capturing_closure();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let semantic = workspace.semantic_program();
    assert_eq!(
        semantic.schema(),
        "semaprax.semantic-workspace-revision.semantic-program.v5"
    );
    let semantic_json: serde_json::Value = serde_json::from_str(semantic.to_json()).unwrap();
    let closures = semantic_json["payload"]["checked_callable_closures"]
        .as_array()
        .unwrap();
    let entry = closures
        .iter()
        .find(|closure| closure["role"] == "entry")
        .unwrap();
    let graph = entry["graph"].as_str().unwrap();
    assert!(graph.contains("semaprax.graph.v37"), "{graph}");
    assert!(graph.contains("\"closure_definitions\""), "{graph}");
    assert!(graph.contains("\"kind\":\"closure\""), "{graph}");

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
    let source = std::fs::read_to_string(fixture.root.join("src/app.spx")).unwrap();
    std::fs::write(
        fixture.root.join("src/app.spx"),
        format!("// source projection only\n{source}"),
    )
    .unwrap();
    let commented_revision = fixture.revision();
    let commented = commented_revision.canonical_workspace_revision().unwrap();
    assert_eq!(semantic.digest(), commented.semantic_program().digest());
    assert_ne!(
        workspace.source_projection_digest(),
        commented.source_projection_digest()
    );
    assert_ne!(
        root.program_root_digest(),
        commented.program_root().unwrap().program_root_digest()
    );
    assert!(ProgramRoot::replay(
        &commented,
        root.program_root_digest(),
        root.to_json().as_bytes()
    )
    .is_err());
    let commented_root = commented.program_root().unwrap();
    assert_eq!(
        ProgramRoot::replay(
            &commented,
            commented_root.program_root_digest(),
            commented_root.to_json().as_bytes()
        )
        .unwrap(),
        commented_root
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
}

#[test]
fn capturing_closures_private_signature_does_not_become_public_export() {
    let fixture = Fixture::capturing_closure();
    let manifest = fixture.root.join("semaprax.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("web = [\"fixture.public\"]", "web = [\"fixture.make\"]");
    std::fs::write(&manifest, text).unwrap();
    let errors = with_authenticated_project(&manifest, |_| Ok(())).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.code == "SPX-W115" || error.code == "SPX-G174"),
        "{errors:?}"
    );
}

#[test]
fn capturing_closures_private_signature_cannot_cross_module_import() {
    let fixture = Fixture::capturing_closure();
    let source = r#"
module fixture.tests;
use function @id("fixture.make") from fixture.app as imported_make;
@id("fixture.tests.main") fn main()->i64 { let callback=imported_make(40); callback(2) }
"#;
    let path = fixture.root.join("src/tests.spx");
    let parsed = semaprax::parse(source, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let errors =
        with_authenticated_project(&fixture.root.join("semaprax.toml"), |_| Ok(())).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G172"),
        "{errors:?}"
    );
}
