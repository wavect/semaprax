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

fn inferred_fixture(label: &str, explicit: bool) -> Fixture {
    let fixture = Fixture::owned_vec(label, false);
    let arguments = if explicit { "<bool>" } else { "" };
    let source = format!(
        r#"
module fixture.app;
@id("inferred.pair") record Pair<T, U> {{
 @id("inferred.payload") payload: T,
 @id("inferred.marker") marker: U,
}}
@id("inferred.relay") fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ value }}
@id("inferred.main") fn main() -> i64 {{
 let input = [1u8];
 let value = relay{arguments}(Pair<Bytes, bool> {{ payload: bytes_copy(array_as_slice(input)), marker: true }});
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
fn inferred_generic_instance_program_roots_replay_their_own_source() {
    let explicit = inferred_fixture("generic-inference-explicit", true);
    let inferred = inferred_fixture("generic-inference-omitted", false);
    assert_inferred_roots(explicit, inferred);
}

#[test]
fn inferred_generic_instance_expression_vectors_replay_their_own_source() {
    let make = |label: &str, explicit: bool| {
        let fixture = inferred_fixture(label, explicit);
        let path = fixture.0.join("src/app.spx");
        let arguments = if explicit { "<bool, i64>" } else { "" };
        let source = format!(
            r#"
module fixture.app;
@id("inferred.pair") record Pair<T, U> {{
 @id("inferred.payload") payload: T,
 @id("inferred.marker") marker: U,
}}
@id("inferred.relay") fn relay<T, U>(value: own Pair<Bytes, T>, tag: U) -> Pair<Bytes, T> {{ value }}
@id("inferred.main") fn main() -> i64 {{
 let input = [1u8];
 let value = relay{arguments}(Pair<Bytes, bool> {{ payload: bytes_copy(array_as_slice(input)), marker: true }}, 1 + 2);
 match own value {{ Pair {{ payload, marker }} => if marker {{ 0 }} else {{ 1 }}, }}
}}
@id("fixture.public") fn published() -> i64 {{ 0 }}
"#
        );
        let parsed = semaprax::parse(&source, &path).unwrap();
        std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
        fixture
    };
    assert_inferred_roots(
        make("generic-inference-vector-explicit", true),
        make("generic-inference-vector-omitted", false),
    );
}

fn assert_inferred_roots(explicit: Fixture, inferred: Fixture) {
    let explicit_revision = explicit.revision();
    let inferred_revision = inferred.revision();
    let explicit_workspace = explicit_revision.canonical_workspace_revision().unwrap();
    let inferred_workspace = inferred_revision.canonical_workspace_revision().unwrap();
    let instances = |workspace: &SemanticWorkspaceRevision| {
        let node: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
        node["payload"]["generic_instance_closures"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|closure| {
                let graph: Value =
                    serde_json::from_str(closure["graph"].as_str().unwrap()).unwrap();
                graph["generic_instance_ownership"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|instance| {
                        for key in [
                            "template",
                            "type_arguments",
                            "execution_instance",
                            "parameters",
                            "result",
                            "cleanup_plan_schema",
                        ] {
                            assert!(!instance[key].is_null(), "missing checked fact {key}");
                        }
                        json!([
                            instance["template"],
                            instance["type_arguments"],
                            instance["execution_instance"],
                            instance["parameters"],
                            instance["result"],
                            instance["cleanup_plan_schema"],
                            instance["cleanup_inventory"],
                            instance["cleanup_plan"],
                        ])
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let inferred_instances = instances(&inferred_workspace);
    assert!(!inferred_instances.is_empty());
    assert_eq!(inferred_instances, instances(&explicit_workspace));
    assert_ne!(
        inferred_workspace.source_projection_digest(),
        explicit_workspace.source_projection_digest()
    );
    let inferred_root = inferred_workspace.program_root().unwrap();
    let explicit_root = explicit_workspace.program_root().unwrap();
    assert_eq!(
        ProgramRoot::replay(
            &inferred_workspace,
            inferred_root.program_root_digest(),
            inferred_root.to_json().as_bytes(),
        )
        .unwrap(),
        inferred_root
    );
    assert!(ProgramRoot::replay(
        &explicit_workspace,
        inferred_root.program_root_digest(),
        inferred_root.to_json().as_bytes(),
    )
    .is_err());
    assert!(ProgramRoot::replay(
        &inferred_workspace,
        explicit_root.program_root_digest(),
        explicit_root.to_json().as_bytes(),
    )
    .is_err());
}

#[test]
fn owned_box_bytes_workspace_binds_v5_prelude_and_replays_its_root() {
    let fixture = Fixture::owned_vec("owned-box-bytes-prelude", false);
    let source = r#"
module fixture.app;
@id("fixture.main") fn main() -> i64 {
    let input = [9u8];
    let owner = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    let bytes = box_into_inner<Bytes>(owner);
    if byte_len(bytes_as_slice(bytes)) == 1usize { 0 } else { 1 }
}
@id("fixture.public") fn published() -> i64 { 0 }
"#;
    let parsed = semaprax::parse(source, fixture.0.join("src/app.spx")).unwrap();
    std::fs::write(
        fixture.0.join("src/app.spx"),
        semaprax::format::canonical(&parsed),
    )
    .unwrap();

    // The later source selects scalar Box v4; it must never downgrade v5.
    let scalar = semaprax::parse(
        "module fixture.tests; @id(\"fixture.tests.main\") fn main()->i64 {box_into_inner<i64>(box_new<i64>(0))}",
        fixture.0.join("src/tests.spx"),
    ).unwrap();
    std::fs::write(
        fixture.0.join("src/tests.spx"),
        semaprax::format::canonical(&scalar),
    )
    .unwrap();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let semantic: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    assert_eq!(
        semantic["payload"]["prelude_digest"],
        framed(
            b"semaprax.semantic-workspace-revision.prelude.digest.v1\0",
            include_bytes!("../../fixtures/prelude-v5.contract"),
        )
    );
    let root = workspace.program_root().unwrap();
    assert_eq!(
        ProgramRoot::replay(
            &workspace,
            root.program_root_digest(),
            root.to_json().as_bytes(),
        )
        .unwrap(),
        root
    );
}

#[test]
fn owned_vec_bytes_workspace_binds_v6_prelude_and_replays_its_root() {
    let fixture = Fixture::owned_vec("owned-vec-bytes-prelude", false);
    let source = r#"
module fixture.app;
@id("fixture.main") fn main() -> i64 {
    let input = [9u8];
    let empty = vec_with_capacity<Bytes>(1usize);
    let values = vec_push<Bytes>(empty, bytes_copy(array_as_slice(input)));
    if vec_len<Bytes>(values) == 1usize { 0 } else { 1 }
}
@id("fixture.public") fn published() -> i64 { 0 }
"#;
    let parsed = semaprax::parse(source, fixture.0.join("src/app.spx")).unwrap();
    std::fs::write(
        fixture.0.join("src/app.spx"),
        semaprax::format::canonical(&parsed),
    )
    .unwrap();

    // The later source selects owned Box v5; it must never downgrade v6.
    let scalar = semaprax::parse(
        "module fixture.tests; @id(\"fixture.tests.main\") fn main()->i64 {let input=[1u8];let owner=box_new<Bytes>(bytes_copy(array_as_slice(input)));0}",
        fixture.0.join("src/tests.spx"),
    ).unwrap();
    std::fs::write(
        fixture.0.join("src/tests.spx"),
        semaprax::format::canonical(&scalar),
    )
    .unwrap();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let semantic: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    assert_eq!(
        semantic["payload"]["prelude_digest"],
        framed(
            b"semaprax.semantic-workspace-revision.prelude.digest.v1\0",
            include_bytes!("../../fixtures/prelude-v6.contract"),
        )
    );
    let root = workspace.program_root().unwrap();
    assert_eq!(
        ProgramRoot::replay(
            &workspace,
            root.program_root_digest(),
            root.to_json().as_bytes(),
        )
        .unwrap(),
        root
    );
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

#[test]
fn explicit_forwarding_program_root_binds_v35_symbolic_mapping() {
    let fixture = Fixture::owned_vec("explicit-forwarding-root", false);
    let text = r#"module fixture.app;
@id("fixture.first") fn first<A,B>(left:A,right:B)->A {left}
@id("fixture.swap") fn swap<A,B>(left:A,right:B)->B {first<B,A>(right,left)}
@id("fixture.main") fn main()->i64 {swap<i64,i64>(1,2)}
@id("fixture.public") fn published()->i64 {0}
"#;
    let path = fixture.0.join("src/app.spx");
    let parsed = semaprax::parse(text, &path).unwrap();
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
        assert_eq!(graph["schema"], "semaprax.graph.v35");
        assert_eq!(
            graph["generic_template_forwarding"][0]["forwarded_argument_mapping"][0]["source"]
                ["index"],
            1
        );
        assert_eq!(graph["revision"], closure["defining_revision"]);
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
    let canonical = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("// retained projection\n{canonical}")).unwrap();
    let commented = fixture.revision().canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.semantic_program().digest(),
        commented.semantic_program().digest()
    );
    assert_ne!(
        workspace.workspace_revision(),
        commented.workspace_revision()
    );
    assert!(ProgramRoot::replay(
        &commented,
        root.program_root_digest(),
        root.to_json().as_bytes()
    )
    .is_err());
}

#[test]
fn generic_collections_program_root_replays_private_owned_carriers() {
    let fixture = Fixture::owned_vec("generic-collections-root", false);
    let text = r#"module fixture.app;
@id("fixture.make") fn make<T>(value:T)->Box<T> {box_new<T>(value)}
@id("fixture.take") fn take<T>(value:own Box<T>)->T {box_into_inner<T>(value)}
@id("fixture.vector") fn vector<T>(value:T)->Vec<T> {vec_push<T>(vec_with_capacity<T>(1usize),value)}
@id("fixture.read") fn read<T>(value:own Vec<T>)->T {vec_get<T>(value,0usize)}
@id("fixture.main") fn main()->i64 {read<i64>(vector<i64>(take<i64>(make<i64>(7))))}
@id("fixture.public") fn published()->i64 {0}
"#;
    let path = fixture.0.join("src/app.spx");
    let parsed = semaprax::parse(text, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let node: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    assert!(!node["payload"]["generic_instance_closures"]
        .as_array()
        .unwrap()
        .is_empty());
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
    let public_owner = text.replace(
        "fn published()->i64 {0}",
        "fn published()->Box<i64> {make<i64>(7)}",
    );
    let parsed = semaprax::parse(&public_owner, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let errors = with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .err()
    .expect("scalar public boundary must reject Box");
    assert!(
        errors.iter().any(|error| error.code == "SPX-W115"
            && error.message == "Public Scalar Export Profile v1 does not admit generic function templates or instances"),
        "{errors:?}"
    );
}

#[test]
fn generic_authored_variant_program_root_replays_private_case_ownership() {
    let fixture = Fixture::owned_vec("generic-authored-variant-root", false);
    let text = r#"module fixture.app;
@id("fixture.choice") variant Choice<P,T>{@id("fixture.data") Data{@id("fixture.payload") payload:P,@id("fixture.marker") marker:T,},@id("fixture.empty") Empty{@id("fixture.empty.marker") marker:T,},}
@id("fixture.rebuild") fn rebuild<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{match own value{Choice::Data{payload,marker}=>Choice<Bytes,T>::Data{payload:payload,marker:marker},Choice::Empty{marker}=>Choice<Bytes,T>::Empty{marker:marker},}}
@id("fixture.make") fn make()->Choice<Bytes,bool>{let input=[9u8];Choice<Bytes,bool>::Data{payload:bytes_copy(array_as_slice(input)),marker:true}}
@id("fixture.consume") fn consume(value:own Choice<Bytes,bool>)->i64{match own value{Choice::Data{payload,marker}=>if marker{1}else{0},Choice::Empty{marker}=>if marker{1}else{0},}}
@id("fixture.main") fn main()->i64{consume(rebuild<bool>(make()))}
@id("fixture.public") fn published()->i64 {0}
"#;
    let path = fixture.0.join("src/app.spx");
    let parsed = semaprax::parse(text, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let node: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    assert!(!node["payload"]["generic_instance_closures"]
        .as_array()
        .unwrap()
        .is_empty());
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
    let public_owner = text.replace(
        "fn published()->i64 {0}",
        "fn published()->Choice<Bytes,bool> {make()}",
    );
    let parsed = semaprax::parse(&public_owner, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let errors = with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .err()
    .expect("scalar public boundary must reject authored variants");
    assert!(
        errors.iter().any(|error| error.code == "SPX-W115"
            && error.message == "Public Scalar Export Profile v1 does not admit authored resource, record, or variant declarations"),
        "{errors:?}"
    );
}
