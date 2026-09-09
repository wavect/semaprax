//! Owned iterator semantics are retained by the same workspace and ProgramRoot.
use super::*;

const SOURCE: &str = r#"module fixture.app;
@id("fixture.consume") fn consume(value: own Bytes) -> usize {
    let view = bytes_as_slice(value); byte_len(view)
}
@id("fixture.main") fn main() -> i64 {
    let payload = [1u8];
    let values = vec_push<Bytes>(vec_with_capacity<Bytes>(1usize), bytes_copy(array_as_slice(payload)));
    let mut total = 0usize;
    for own item in vec_into_iter<Bytes>(values) { total = total + consume(item); 0 }
    if total == 1usize { 0 } else { 1 }
}
@id("fixture.public") fn published() -> i64 { 0 }
"#;

fn fixture(label: &str, source: &str) -> Fixture {
    let fixture = Fixture::owned_vec(label, false);
    if source.contains("fixture.consume") {
        let manifest = std::fs::read_to_string(fixture.manifest()).unwrap();
        std::fs::write(
            fixture.manifest(),
            manifest.replace(
                "version = \"0.1.0\"",
                "version = \"0.1.0\"\nprofile = \"owned-data-api.v1\"",
            ),
        )
        .unwrap();
    }
    let path = fixture.0.join("src/app.spx");
    let parsed = semaprax::parse(source, &path).unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&canonical, &path).unwrap()),
        canonical
    );
    std::fs::write(path, canonical).unwrap();
    fixture
}

fn verify_root(
    fixture: &Fixture,
) -> (Arc<ProjectRevision>, SemanticWorkspaceRevision, ProgramRoot) {
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let semantic: Value = serde_json::from_str(workspace.semantic_program().to_json()).unwrap();
    assert_eq!(
        semantic["payload"]["prelude_digest"],
        framed(
            b"semaprax.semantic-workspace-revision.prelude.digest.v1\0",
            include_bytes!("../../fixtures/prelude-v8.contract"),
        )
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
    (revision, workspace, root)
}

#[test]
fn owned_iterator_workspace_binds_payload_cleanup_and_rejects_source_drift() {
    let fixture = fixture("owned-iterator-root", SOURCE);
    let source = semaprax::parse(SOURCE, fixture.0.join("src/app.spx")).unwrap();
    let graph: Value = serde_json::from_str(&semaprax::graph::to_json(&source).unwrap()).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v45");
    assert_eq!(
        graph["owned_iterator_payloads"]["yield_owners"],
        json!(["core.iter-step.yield.item", "core.iter-step.yield.rest"])
    );
    assert_eq!(
        graph["owned_iterator_payloads"]["cleanup_schema"],
        "semaprax.cleanup-plan.v13"
    );
    let (_, before, root) = verify_root(&fixture);
    let changed = SOURCE.replace("[1u8]", "[2u8]");
    let path = fixture.0.join("src/app.spx");
    std::fs::write(
        &path,
        semaprax::format::canonical(&semaprax::parse(&changed, &path).unwrap()),
    )
    .unwrap();
    let (_, after, next_root) = verify_root(&fixture);
    assert_ne!(
        before.semantic_program().digest(),
        after.semantic_program().digest()
    );
    assert!(ProgramRoot::replay(
        &after,
        root.program_root_digest(),
        root.to_json().as_bytes()
    )
    .is_err());
    assert!(ProgramRoot::replay(
        &before,
        next_root.program_root_digest(),
        next_root.to_json().as_bytes()
    )
    .is_err());
}

#[test]
fn owned_iterator_local_done_retains_payload_prelude_without_vec_operations() {
    let source = r#"module fixture.app;
@id("fixture.main") fn main() -> i64 {
    let step = IterStep<Bytes>::Done {};
    match own step { IterStep::Done {} => 0, IterStep::Yield {item,rest} => 1, }
}
@id("fixture.public") fn published() -> i64 { 0 }
"#;
    verify_root(&fixture("owned-iterator-done-root", source));
}
