use super::*;

fn fixture(label: &str, marker: bool) -> Fixture {
    let fixture = Fixture::owned_vec(label, false);
    let source = format!(
        r#"
module fixture.app;
@id("fixture.pair") record Pair<T, U> {{
    @id("fixture.payload") payload: T,
    @id("fixture.marker") marker: U,
}}
@id("fixture.relay") fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ value }}
@id("fixture.main") fn main() -> i64 {{
    let input = [1u8];
    let value = relay<bool>(Pair<Bytes, bool> {{ payload: bytes_copy(array_as_slice(input)), marker: {marker} }});
    match own value {{ Pair {{ payload, marker }} => if marker {{ 0 }} else {{ 1 }}, }}
}}
@id("fixture.public") fn published() -> i64 {{ 0 }}
"#
    );
    let parsed = semaprax::parse(&source, fixture.0.join("src/app.spx")).unwrap();
    std::fs::write(
        fixture.0.join("src/app.spx"),
        semaprax::format::canonical(&parsed),
    )
    .unwrap();
    fixture
}

#[test]
fn generic_instance_program_root_binds_checked_ownership_and_rejects_cross_pairs() {
    let first = fixture("generic-first", true);
    let second = fixture("generic-second", false);
    let revision = first.revision();
    let other_revision = second.revision();
    let legacy = (
        revision.project_revision().to_owned(),
        revision.semantic_graph().to_owned(),
    );
    let workspace = revision.canonical_workspace_revision().unwrap();
    let other = other_revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().schema(),
        SemanticProgram::SCHEMA_V2
    );
    let node: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    let closures = node["payload"]["generic_instance_closures"]
        .as_array()
        .unwrap();
    assert!(!closures.is_empty());
    for closure in closures {
        assert_eq!(closure["defining_revision"], revision.project_revision());
        let graph: Value = serde_json::from_str(closure["graph"].as_str().unwrap()).unwrap();
        assert_eq!(graph["schema"], "semaprax.graph.v34");
        assert!(graph["generic_instance_ownership"]
            .as_array()
            .is_some_and(|v| !v.is_empty()));
    }
    let root = workspace.program_root().unwrap();
    let other_root = other.program_root().unwrap();
    let segment = root
        .segments()
        .iter()
        .find(|segment| segment.kind() == "semantic_program")
        .unwrap();
    assert_eq!(segment.node_schema(), SemanticProgram::SCHEMA_V2);
    assert_eq!(segment.node_digest(), workspace.semantic_program().digest());
    assert_eq!(
        segment.node_bytes(),
        workspace.semantic_program().to_json().len()
    );
    assert_ne!(segment.node_digest(), other.semantic_program().digest());
    assert_eq!(
        ProgramRoot::replay(
            &workspace,
            root.program_root_digest(),
            root.to_json().as_bytes()
        )
        .unwrap(),
        root
    );
    assert!(ProgramRoot::replay(
        &other,
        root.program_root_digest(),
        root.to_json().as_bytes()
    )
    .is_err());
    assert!(ProgramRoot::replay(
        &workspace,
        other_root.program_root_digest(),
        other_root.to_json().as_bytes()
    )
    .is_err());
    // Remint an otherwise canonical root around another valid semantic node.
    // Even a self-consistent digest cannot substitute retained checked meaning.
    let mut reminted: Value = serde_json::from_str(root.to_json()).unwrap();
    let other_wire: Value = serde_json::from_str(other_root.to_json()).unwrap();
    reminted["segments"][1] = other_wire["segments"][1].clone();
    reminted.as_object_mut().unwrap().remove("program_root");
    let reminted_digest = framed(
        b"semaprax.program-root.digest.v1\0",
        canonical(reminted.clone()).as_bytes(),
    );
    reminted["program_root"] = json!(reminted_digest);
    assert!(
        ProgramRoot::replay(&workspace, &reminted_digest, canonical(reminted).as_bytes()).is_err()
    );
    assert_eq!(
        SemanticWorkspaceRevision::replay(
            &revision,
            workspace.workspace_revision(),
            workspace.to_json().as_bytes()
        )
        .unwrap(),
        workspace
    );
    let mut forged: Value = serde_json::from_str(workspace.to_json()).unwrap();
    let graph_slot = &mut forged["nodes"]["semantic_program"]["value"]["payload"]
        ["generic_instance_closures"][0]["graph"];
    let original = graph_slot.as_str().unwrap();
    let changed = original.replacen(
        "\"ownership_mode\":\"own\"",
        "\"ownership_mode\":\"value\"",
        1,
    );
    assert_ne!(original, changed);
    *graph_slot = json!(changed);
    let node_bytes = canonical(forged["nodes"]["semantic_program"]["value"].clone());
    forged["nodes"]["semantic_program"]["digest"] = json!(framed(
        b"semaprax.semantic-workspace-revision.semantic-program.digest.v2\0",
        node_bytes.as_bytes()
    ));
    let semantic = sequence(
        b"semaprax.semantic-workspace-revision.semantic.digest.v1\0",
        [
            "semantic_program",
            "stable_identity_index",
            "contracts_and_tests",
            "agent_definitions",
            "authority_policies",
            "target_profiles",
        ]
        .map(|key| forged["nodes"][key]["digest"].as_str().unwrap()),
    );
    forged["digests"]["semantic"] = json!(semantic);
    let reminted_workspace = sequence(
        b"semaprax.semantic-workspace-revision.digest.v1\0",
        [
            "semantic",
            "source_projection",
            "manifest",
            "dependency_lock",
        ]
        .map(|key| forged["digests"][key].as_str().unwrap()),
    );
    forged["workspace_revision"] = json!(reminted_workspace);
    assert!(SemanticWorkspaceRevision::replay(
        &revision,
        &reminted_workspace,
        canonical(forged).as_bytes()
    )
    .is_err());
    assert_eq!(
        legacy,
        (
            revision.project_revision().to_owned(),
            revision.semantic_graph().to_owned()
        )
    );
}
