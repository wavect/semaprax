use std::path::Path;

use semaprax::{format, graph, hir, parse, verify};

#[test]
fn every_committed_example_is_canonical_and_verified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut paths = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "spx"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());

    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let program = parse(&source, &path).unwrap_or_else(|error| panic!("{error}"));
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics.is_empty(),
            "{} produced diagnostics: {diagnostics:#?}",
            path.display()
        );
        hir::resolve(&program).unwrap_or_else(|diagnostics| {
            panic!("{} did not resolve: {diagnostics:#?}", path.display())
        });
        assert_eq!(
            format::canonical(&program),
            source,
            "{} is not canonical",
            path.display()
        );
    }
}

#[test]
fn contract_graph_matches_exact_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("examples/meaning.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(
        format!("{}\n", graph::to_json(&program).unwrap()),
        include_str!("snapshots/meaning.graph.json")
    );
}

#[test]
fn meaning_revision_matches_the_domain_separated_sha256_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("examples/meaning.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();

    assert_eq!(
        graph::revision(&program),
        "sha256:ab81beaac718d4cdaf2c79740391a7280e593dea9178275af7a1bc177f3a44c3"
    );
}
