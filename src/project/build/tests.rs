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

#[test]
fn native_law_selection_refuses_hidden_sources_on_owned_cold_and_cached_routes() {
    const LAW: &str = "module calculator.laws;\n\n@id(\"calculator.divide.nonzero\")\nlaw contract \"calculator.divide\" requires (right: i64)\n    right != 0\n    evidence theorem_proved;\n";
    const MANIFEST: &str = "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"calculator\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/core.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n";
    let selected = ProjectManifest::parse(MANIFEST).unwrap();
    let hidden_text = MANIFEST.replace("law_sources = [\"src/contracts.spx\"]", "law_sources = []");
    assert_ne!(hidden_text, MANIFEST);
    let hidden = ProjectManifest::parse(&hidden_text).unwrap();
    let sources = [
        (
            "src/app.spx",
            include_str!("../../../examples/calculator-project/src/app.spx"),
        ),
        ("src/contracts.spx", LAW),
        (
            "src/core.spx",
            include_str!("../../../examples/calculator-project/src/core.spx"),
        ),
        (
            "src/tests.spx",
            include_str!("../../../examples/calculator-project/src/tests.spx"),
        ),
    ];
    let owned = || {
        sources
            .iter()
            .map(
                |(path, source)| crate::semantic_workspace::SemanticWorkspaceSource {
                    path: (*path).to_owned(),
                    source: (*source).to_owned(),
                },
            )
            .collect()
    };
    let errors = super::build_owned(&hidden, owned())
        .err()
        .expect("owned hidden law must refuse");
    assert_eq!(errors[0].code, "SPX-LW110");
    let frontend = sources
        .iter()
        .map(|(path, source)| ProjectFrontendSource::new(path, source).unwrap())
        .collect::<Vec<_>>();
    for semantic in [false, true] {
        let mut cache = if semantic {
            ProjectFrontendCache::new_with_semantic_cache()
        } else {
            ProjectFrontendCache::new()
        };
        let errors = cache
            .build(&hidden, &frontend)
            .err()
            .expect("cold hidden law must refuse");
        assert_eq!(errors[0].code, "SPX-LW110");
        let before = cache.build(&selected, &frontend).unwrap();
        assert_eq!(before.revision().law_modules().len(), 1);
        let errors = cache
            .build(&hidden, &frontend)
            .err()
            .expect("cached hidden law must refuse");
        assert_eq!(errors[0].code, "SPX-LW110");
        let after = cache.build(&selected, &frontend).unwrap();
        assert_eq!(after.revision().law_modules().len(), 1);
        assert_eq!(
            before.revision().project_revision(),
            after.revision().project_revision()
        );
        assert_eq!(
            before.revision().semantic_graph_digest(),
            after.revision().semantic_graph_digest()
        );
    }
}
