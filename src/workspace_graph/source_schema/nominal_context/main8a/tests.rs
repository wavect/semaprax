use super::*;

fn fixture_source(name: &str) -> &'static str {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/workspace/canonical_revision/owned_iterators.rs"
    ));
    source
        .split_once(&format!("const {name}: &str = r#\""))
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0
}
fn build() -> WorkspaceGraphBuild {
    let sources = [
        ("src/app.spx", fixture_source("IMPORTED_RECORD_SOURCE")),
        ("src/types.spx", fixture_source("IMPORTED_RECORD_PROVIDER")),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&crate::parse(source, Path::new(path)).unwrap()),
    })
    .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .unwrap()
        .0
}

#[test]
fn imported_record_context_keeps_v67_and_the_exact_local_emitted_view() {
    let build = build();
    let caller = build
        .hir
        .modules
        .iter()
        .find(|m| m.path == "src/app.spx")
        .unwrap();
    let types = Types::new(caller, &build.hir.modules, &build.hir.declarations).unwrap();
    assert_eq!(types.len(), caller.types.len());
    for (actual, expected) in types.iter().zip(caller.types.iter()) {
        assert!(std::ptr::eq(actual, expected));
    }
    assert!(!types.iter().any(|item| item.id.as_str() == "fixture.line"));
    let imported = types
        .find_in_context(&mut |item| item.id.as_str() == "fixture.line")
        .unwrap();
    assert_eq!(imported.name, "Line");
    // The frozen projection still fails closed when nominal context is absent.
    assert_eq!(
        graph::graph_schema_from_parts_and_instances(
            &caller.interfaces,
            &caller.types,
            &caller.functions,
            &caller.function_templates,
            &caller.function_instances,
        )
        .unwrap(),
        "semaprax.graph.v74"
    );
    assert_eq!(
        schema(caller, &build.hir.modules, &build.hir.declarations).unwrap(),
        "semaprax.graph.v67"
    );
    assert_eq!(
        build.source_graph_schemas().unwrap()["src/app.spx"],
        "semaprax.graph.v67"
    );
}

#[test]
fn missing_foreign_and_duplicate_nominal_context_preserve_conservative_refusal() {
    for mutation in 0..6 {
        let mut build = build();
        let caller = build
            .hir
            .modules
            .iter()
            .position(|m| m.path == "src/app.spx")
            .unwrap();
        match mutation {
            0 => {
                build.hir.declarations.remove("fixture.line");
            }
            1 => {
                build.hir.declarations.get_mut("fixture.line").unwrap().path =
                    Some("foreign.spx".into());
            }
            2 => {
                build.hir.declarations.get_mut("fixture.line").unwrap().kind =
                    hir::DeclarationKind::Variant;
            }
            3 => {
                build
                    .hir
                    .declarations
                    .get_mut("fixture.line")
                    .unwrap()
                    .owner = Some("foreign".into());
            }
            4 => {
                build
                    .hir
                    .declarations
                    .get_mut("fixture.line")
                    .unwrap()
                    .origin = hir::IdentityOrigin::CompilerOwned;
            }
            _ => {
                let item = build
                    .hir
                    .modules
                    .iter()
                    .flat_map(|m| m.types.iter())
                    .find(|t| t.id.as_str() == "fixture.line")
                    .unwrap()
                    .clone();
                build.hir.modules[caller].types = vec![item].into();
            }
        }
        let caller = &build.hir.modules[caller];
        let types = Types::new(caller, &build.hir.modules, &build.hir.declarations).unwrap();
        assert!(types
            .find_in_context(&mut |item| item.id.as_str() == "fixture.line")
            .is_none());
        // No weakened missing-type rule: unknown nominal types still select
        // the conservative profile rather than claiming frozen compatibility.
        assert_eq!(
            graph::graph_schema_from_parts_and_instances(
                &caller.interfaces,
                &types,
                &caller.functions,
                &caller.function_templates,
                &caller.function_instances,
            )
            .unwrap(),
            "semaprax.graph.v74"
        );
    }
}
