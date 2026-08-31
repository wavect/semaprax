//! Exact production descriptor/provider, not a substitute admission wrapper.
use semaprax::project::{self, PublicApiSubject};

const SELECTED: [&str; 8] = [
    "stage.block",
    "stage.conditional",
    "stage.direct",
    "stage.multiple",
    "stage.nested",
    "stage.place",
    "stage.projected",
    "stage.temporary",
];

pub(super) fn provider() -> String {
    let parsed = semaprax::check(include_str!("source.spx"), "bytes-staging.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&reparsed), canonical);
    assert_eq!(
        semaprax::graph::to_json(&parsed).unwrap(),
        semaprax::graph::to_json(&reparsed).unwrap()
    );
    let program = semaprax::hir::resolve(&parsed).unwrap();
    semaprax::hir::validate(&program).unwrap();
    let selected = SELECTED.map(str::to_owned);
    let fact = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let subject = PublicApiSubject {
        project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: fact,
        workspace_revision: fact,
        project_graph_digest: fact,
    };
    let descriptor = project::derive_public_api_descriptor(&program, &selected, subject).unwrap();
    let bytes = descriptor.canonical_bytes();
    let digest = descriptor.digest();
    assert_eq!(
        project::replay_public_api_descriptor(&program, &selected, subject, &bytes, &digest)
            .unwrap(),
        descriptor
    );
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        document["exports"].as_array().unwrap().len(),
        SELECTED.len()
    );
    let emitted = semaprax::codegen::emit_native_owned_data_provider(
        &program, &selected, subject, &bytes, &digest,
    )
    .unwrap();
    assert_eq!(emitted.descriptor(), bytes);
    assert_eq!(emitted.descriptor_digest(), digest);
    emitted.source().to_owned()
}
