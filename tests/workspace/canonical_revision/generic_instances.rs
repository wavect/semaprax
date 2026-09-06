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
    let mut semantic_source = node["payload"].clone();
    semantic_source
        .as_object_mut()
        .unwrap()
        .remove("generic_instance_closures");
    let defining_subject = canonical(json!({
        "semantic_source": semantic_source,
        "manifest": revision.manifest().to_canonical_toml(),
    }));
    let defining_revision = framed(
        b"semaprax.generic-instance-program-revision.v1\0",
        defining_subject.as_bytes(),
    );
    let closures = node["payload"]["generic_instance_closures"]
        .as_array()
        .unwrap();
    assert!(!closures.is_empty());
    for closure in closures {
        assert_eq!(
            closure["defining_revision_kind"],
            "normalized_project_semantics"
        );
        assert_eq!(closure["defining_revision"], defining_revision);
        let graph: Value = serde_json::from_str(closure["graph"].as_str().unwrap()).unwrap();
        assert_eq!(graph["schema"], "semaprax.graph.v34");
        assert_eq!(graph["revision"], defining_revision);
        for instance in graph["generic_instance_ownership"].as_array().unwrap() {
            assert_eq!(instance["source_revision"], defining_revision);
        }
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

#[test]
fn generic_instance_semantic_identity_ignores_comments_but_root_replay_binds_source() {
    let first = fixture("generic-comment-base", true);
    let commented = fixture("generic-comment-projection", true);
    let path = commented.0.join("src/app.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("// projection-only generic ownership comment\n{source}"),
    )
    .unwrap();

    let revision = first.revision();
    let commented_revision = commented.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let other = commented_revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().schema(),
        SemanticProgram::SCHEMA_V2
    );
    assert_eq!(
        other.semantic_program().schema(),
        SemanticProgram::SCHEMA_V2
    );
    assert_ne!(
        revision.project_revision(),
        commented_revision.project_revision()
    );
    assert_eq!(workspace.semantic_digest(), other.semantic_digest());
    assert_eq!(
        workspace.semantic_program().digest(),
        other.semantic_program().digest()
    );

    let instance_identities = |workspace: &SemanticWorkspaceRevision| {
        let node: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
        let mut identities = BTreeMap::new();
        for closure in node["payload"]["generic_instance_closures"]
            .as_array()
            .unwrap()
        {
            let graph: Value = serde_json::from_str(closure["graph"].as_str().unwrap()).unwrap();
            for instance in graph["generic_instance_ownership"].as_array().unwrap() {
                let key = (
                    closure["role"].as_str().unwrap().to_owned(),
                    instance["template"].as_str().unwrap().to_owned(),
                    instance["type_arguments"].to_string(),
                );
                let identity = instance["concrete_instance"].as_str().unwrap().to_owned();
                assert!(identities.insert(key, identity).is_none());
            }
        }
        assert!(!identities.is_empty());
        identities
    };
    assert_eq!(instance_identities(&workspace), instance_identities(&other));
    assert_ne!(
        workspace.source_projection_digest(),
        other.source_projection_digest()
    );
    assert_ne!(workspace.workspace_revision(), other.workspace_revision());
    let root = workspace.program_root().unwrap();
    let other_root = other.program_root().unwrap();
    assert_ne!(root.program_root_digest(), other_root.program_root_digest());
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
        ProgramRoot::replay(
            &other,
            other_root.program_root_digest(),
            other_root.to_json().as_bytes()
        )
        .unwrap(),
        other_root
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
    assert!(SemanticWorkspaceRevision::replay(
        &commented_revision,
        workspace.workspace_revision(),
        workspace.to_json().as_bytes()
    )
    .is_err());
    assert!(SemanticWorkspaceRevision::replay(
        &revision,
        other.workspace_revision(),
        other.to_json().as_bytes()
    )
    .is_err());
}

#[test]
fn generic_result_program_root_replays_checked_variant_closures() {
    for error in [
        "i64", "i32", "u8", "usize", "char", "f32", "f64", "bool", "Bytes",
    ] {
        let fixture = Fixture::owned_vec(&format!("generic-result-{error}"), false);
        let text = format!(
            r#"
module fixture.app;
@id("fixture.propagate") fn propagate<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> {{
    let payload = value?;
    Result<Bytes, E>::Ok {{ value: payload }}
}}
@id("fixture.consume") fn consume(value: own Result<Bytes, {error}>) -> i64 {{
    match own value {{
        Result::Ok {{ value: payload }} => 0,
        Result::Err {{ error: error }} => 1,
    }}
}}
@id("fixture.main") fn main() -> i64 {{
    let input = [1u8];
    consume(propagate<{error}>(Result<Bytes, {error}>::Ok {{ value: bytes_copy(array_as_slice(input)) }}))
}}
@id("fixture.public") fn published() -> i64 {{ 0 }}
"#
        );
        let path = fixture.0.join("src/app.spx");
        let parsed = semaprax::parse(&text, &path).unwrap();
        std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
        let revision = fixture.revision();
        let workspace = revision.canonical_workspace_revision().unwrap();
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
            let graph: Value = serde_json::from_str(closure["graph"].as_str().unwrap()).unwrap();
            assert_eq!(graph["schema"], "semaprax.graph.v34");
            let instances = graph["generic_instance_ownership"].as_array().unwrap();
            assert_eq!(instances.len(), 1);
            assert_eq!(
                instances[0]["cleanup_plan_schema"],
                "semaprax.cleanup-plan.v6"
            );
            assert!(instances[0]["result"]["concrete_record_identity"].is_null());
            assert_eq!(
                instances[0]["source_revision"],
                closure["defining_revision"]
            );
        }
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
            SemanticWorkspaceRevision::replay(
                &revision,
                workspace.workspace_revision(),
                workspace.to_json().as_bytes()
            )
            .unwrap(),
            workspace
        );
        let mut forged: Value = serde_json::from_str(root.to_json()).unwrap();
        let semantic = forged["segments"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|segment| segment["kind"] == "semantic_program")
            .unwrap();
        semantic["node_digest"] = json!("0".repeat(64));
        assert!(ProgramRoot::replay(
            &workspace,
            root.program_root_digest(),
            canonical(forged).as_bytes()
        )
        .is_err());
        let manifest = std::fs::read_to_string(fixture.manifest()).unwrap();
        std::fs::write(
            fixture.manifest(),
            manifest.replace("fixture.public", "fixture.consume"),
        )
        .unwrap();
        assert!(
            with_authenticated_project(&fixture.manifest(), |snapshot| Ok(
                snapshot.retain_revision()
            ))
            .is_err(),
            "private owned Result admission must not widen the scalar public ABI"
        );
    }
}
