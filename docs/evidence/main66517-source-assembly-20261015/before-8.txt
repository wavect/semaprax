use crate::project::{ProjectFrontendCache, ProjectFrontendSource, ProjectManifest};

#[test]
fn unchanged_frontend_build_performs_no_hidden_source_reparse() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    let manifest =
        ProjectManifest::parse(&std::fs::read_to_string(root.join("semaprax.toml")).unwrap())
            .unwrap();
    let sources = manifest
        .sources()
        .iter()
        .map(|path| {
            ProjectFrontendSource::new(
                path.as_str(),
                &std::fs::read_to_string(root.join(path.as_str())).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for semantic in [false, true] {
        let mut cache = if semantic {
            ProjectFrontendCache::new_with_semantic_cache()
        } else {
            ProjectFrontendCache::new()
        };
        let cold = cache.build(&manifest, &sources).unwrap();
        let before = crate::TEST_PUBLIC_PARSE_CALLS.with(|calls| calls.get());
        crate::TEST_PUBLIC_PARSE_SITES.with(|sites| *sites.borrow_mut() = Some(Vec::new()));
        let warm = cache.build(&manifest, &sources).unwrap();
        let sites = crate::TEST_PUBLIC_PARSE_SITES.with(|sites| sites.borrow_mut().take().unwrap());
        let actual_calls = crate::TEST_PUBLIC_PARSE_CALLS.with(|calls| calls.get()) - before;
        let work: serde_json::Value = serde_json::from_str(warm.to_json()).unwrap();
        assert_eq!(work["work"]["modules_parsed"], 0);
        assert_eq!(
            actual_calls, 0,
            "cached Project construction must not reparse sources after frontend accounting: {sites:?}"
        );
        assert_eq!(
            cold.revision().project_revision(),
            warm.revision().project_revision()
        );
        assert_eq!(
            cold.revision().semantic_graph_digest(),
            warm.revision().semantic_graph_digest()
        );
    }
}

#[test]
fn retained_ast_revision_matches_source_replay_across_preludes() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files = vec![
        examples.join("lazy-iterator-adapters.spx"),
        examples.join("iterator-operations.spx"),
    ];
    for folder in [
        "calculator-project",
        "vector-stats-project",
        "agent-response-project",
    ] {
        files.extend(
            std::fs::read_dir(examples.join(folder).join("src"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "spx")),
        );
    }
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        let program = crate::parse(&source, &path).unwrap();
        let canonical = crate::format::canonical(&program);
        assert_eq!(
            crate::graph::revision_from_canonical_program(&canonical, &program),
            crate::graph::revision_from_canonical_source(&canonical),
            "{}",
            path.display()
        );
    }
}
