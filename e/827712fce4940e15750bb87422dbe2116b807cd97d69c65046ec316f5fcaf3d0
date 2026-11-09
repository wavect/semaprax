//! Two closed source acquisitions; neither retargets an original V13 endpoint.
use serde_json::Value;

const OWNERS: [&str; 3] = [
    "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
    "src/source_reflows_merge/main_e177_overlay_v2.rs",
    "src/source_reflows_merge/main_e177_overlay_compiled.rs",
];
const TARGET: &str = "ca1b14fcf0f108f4c085b11a0b370faa33cde96c";

#[derive(Clone, Copy)]
enum Route {
    Original,
    Caller,
}

pub(super) fn original(source: &str, supplied: &str) -> String {
    let acquired = v14_source_before(source, supplied);
    if OWNERS.contains(&source) {
        restore(source, &acquired, Route::Original)
    } else {
        acquired
    }
}

pub(super) fn caller(source: &str, supplied: &str) -> Option<String> {
    OWNERS
        .contains(&source)
        .then(|| restore(source, &v14_source_before(source, supplied), Route::Caller))
}

fn restore(source: &str, supplied: &str, route: Route) -> String {
    // The caller first authenticates its complete actual include_str! source.
    let packet: Value = serde_json::from_str(include_str!("controller_transport.json")).unwrap();
    assert_eq!(packet["schema"], "semaprax.v13-controller-transport.v1");
    assert_eq!(packet["target_commit"], TARGET);
    let records = packet["records"].as_array().unwrap();
    assert_eq!(records.len(), OWNERS.len());
    let names: Vec<_> = records
        .iter()
        .map(|record| record["source"].as_str().unwrap())
        .collect();
    assert_eq!(names, OWNERS);
    let record = &records[OWNERS.iter().position(|owner| *owner == source).unwrap()];
    assert_eq!(supplied.len() as u64, record["length"].as_u64().unwrap());
    assert_eq!(
        super::hash(supplied.as_bytes()),
        record["sha256"].as_str().unwrap()
    );
    let original: Value = serde_json::from_str(include_str!("inverse.json")).unwrap();
    let rows: Vec<_> = original
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["source"] == source)
        .collect();
    assert_eq!(rows.len(), 1);
    let endpoint = match route {
        Route::Original => &record["original"],
        Route::Caller => &record["caller"],
    };
    let mut restored = supplied.to_owned();
    let edits = match route {
        Route::Original => &endpoint["edits"],
        Route::Caller => {
            assert_eq!(endpoint["edits_from_original"], source);
            if source == OWNERS[2] {
                // One complete named historical input, never a generic trim.
                let front = &endpoint["input_front"];
                assert_eq!(
                    front["source_kind"],
                    "COMPLETE_GIT424E_COMPILED_TO_ACCEPTED5906_CALLER_INPUT"
                );
                assert_eq!(front["length"], 145335);
                assert_eq!(
                    front["sha256"],
                    "48386e45517906a53af81c0f58cf7c9c49a61d4abbc3ba4bee132c7f3e48130a"
                );
                assert_eq!(front["length"], record["length"]);
                assert_eq!(front["sha256"], record["sha256"]);
                assert_eq!(front["target"]["length"], 141321);
                assert_eq!(
                    front["target"]["sha256"],
                    "44abde8164a14329bc67feaeefc90459846f4198ab25eb863b1fb8c0af50cbfc"
                );
                assert_eq!(endpoint["length"], 140514);
                assert_eq!(
                    endpoint["sha256"],
                    "10e2595b3b4fb89f84dee525c0a2023fa2c15a9bfad5a49dc48042bd928b5097"
                );
                let prefix = front["edits"].as_array().unwrap();
                assert_eq!(prefix.len(), 4);
                for edit in prefix {
                    let after = edit["after"].as_str().unwrap();
                    assert!(!after.is_empty());
                    assert_eq!(edit["count"].as_u64(), Some(1));
                    assert_eq!(restored.matches(after).count(), 1);
                    restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                assert_eq!(
                    restored.len() as u64,
                    front["target"]["length"].as_u64().unwrap()
                );
                assert_eq!(
                    super::hash(restored.as_bytes()),
                    front["target"]["sha256"].as_str().unwrap()
                );
            }
            &rows[0]["edits"]
        }
    };
    for edit in edits.as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert!(!after.is_empty());
        assert_eq!(edit["count"].as_u64(), Some(1));
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(restored.len() as u64, endpoint["length"].as_u64().unwrap());
    assert_eq!(
        super::hash(restored.as_bytes()),
        endpoint["sha256"].as_str().unwrap()
    );
    if let Route::Original = route {
        // Full ca1b inputs precede the independently replayed seven original
        // inverses. Their whole lower endpoints and records stay unchanged.
        assert_eq!(endpoint["length"], rows[0]["length"]);
        assert_eq!(endpoint["sha256"], rows[0]["sha256"]);
    }
    // Parent/root Caller retain their merged metadata. Compiled Caller first
    // acquires its complete named accepted5906 input, then the immutable old7
    // instrumentation inverse; the full actual merged source stays bound above.
    restored
}

const V14_FRONT_SOURCES: [&str; 4] = [
    "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
    "src/source_reflows_merge/main_e177_overlay_v2.rs",
    "docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py",
    "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs",
];

fn v14_source_before(source: &str, supplied: &str) -> String {
    if source == OWNERS[2] {
        // Complete repaired physical source first; the immutable controller
        // routes below still consume their entire Git424e input view.
        let compiled = include_str!("../main_e177_overlay_compiled.rs");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(supplied, compiled);
        assert_eq!(std::fs::read(root.join(source)).unwrap(), compiled.as_bytes());
        // Exact new two-child source epoch before the entire old INPUT.
        assert_eq!(supplied.len(), 145546);
        assert_eq!(super::hash(supplied.as_bytes()), "213369b5e6e3332c392149132b04409d6c7f1100988a27c0cd13b36e6fdad93d");
        let mut compiled_epoch = supplied.to_owned();
        let epoch_edits: &[(usize, &str, &str)] = &[
            (129856, "        assert_eq!(lookup, LOOKUP); let next_leaf_arm_0 = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_0).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_0 = lookup.replacen(next_leaf_arm_0, \"\", 1); let lookup = prior_leaf_lookup_0.as_str(); let next_leaf_arm_1 = \"        \\\"tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\" => Some(include_str!(\\\"../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_1).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_1 = lookup.replacen(next_leaf_arm_1, \"\", 1); let lookup = prior_leaf_lookup_1.as_str(); let main3d81_arms = \"        \\\"src/cleanup_plan/replay/transition.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/transition.rs\\\")),\\n        \\\"src/workspace_graph/initial_report/build.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/initial_report/build.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, \"exact incoming3d81 lookup arms and complete oracle\"); let main3d81_lookup = lookup.replacen(main3d81_arms, \"\", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = \"        \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(field_local_arms).count(), 1, \"exact four field-local lookup arms\"); let field_prior_lookup = lookup.replacen(field_local_arms, \"\", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\")),\\n\"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, \"exact new retained-fork lookup arm\"); let prior_lookup = lookup.replacen(retained_leaf_arm, \"\", 1); let lookup = prior_lookup.as_str();\n", "        assert_eq!(lookup, LOOKUP); let main3d81_arms = \"        \\\"src/cleanup_plan/replay/transition.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/transition.rs\\\")),\\n        \\\"src/workspace_graph/initial_report/build.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/initial_report/build.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, \"exact incoming3d81 lookup arms and complete oracle\"); let main3d81_lookup = lookup.replacen(main3d81_arms, \"\", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = \"        \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(field_local_arms).count(), 1, \"exact four field-local lookup arms\"); let field_prior_lookup = lookup.replacen(field_local_arms, \"\", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\")),\\n\"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, \"exact new retained-fork lookup arm\"); let prior_lookup = lookup.replacen(retained_leaf_arm, \"\", 1); let lookup = prior_lookup.as_str();\n"),
        ];
        for &(at, after, before) in epoch_edits.iter().rev() {
            assert_eq!(compiled_epoch.get(at..at + after.len()), Some(after));
            compiled_epoch.replace_range(at..at + after.len(), before);
        }
        let supplied = compiled_epoch.as_str();
        assert_eq!(supplied.len(), 144680);
        assert_eq!(super::hash(supplied.as_bytes()), "84f65e1495528a262750473e8ea36acd2d921e009e115fa5387bf802c81fd0a1");
        let mut restored = supplied.to_owned();
        for &(start, end, after, before) in &[
            (143860, 144167, "        let current_loader = LOADER.replacen(\" length=10905\\n\", \" length=23065\\n\", 1).replacen(\"a0f906f11a8c7205a2124e5cd9c1909fa1a53a12731a0775d3e360d844bab794\", \"9cae8c3b1db0d54de8ce9d66639dab5de6c16c9e5772d1503b922482b1b2a1fb\", 1); assert_eq!(PYTHON.matches(&current_loader).count(), 1); for parent in [\n", "        let current_loader = LOADER.replacen(\" length=10905\\n\", \" length=16207\\n\", 1).replacen(\"a0f906f11a8c7205a2124e5cd9c1909fa1a53a12731a0775d3e360d844bab794\", \"36caf4ad467086e1b1be44917472751d359088fe01e3930f4042d69dacb537f8\", 1); assert_eq!(PYTHON.matches(&current_loader).count(), 1); for parent in [\n"),
            (136916, 137567, "        let financed_scope = \"        for source in [\\\"src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs\\\", \\\"src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs\\\", \\\"src/cleanup_plan/replay_tests/factored.rs\\\", \\\"src/cleanup_plan/replay/factored/moderate_tests.rs\\\", \\\"src/cleanup_plan/replay/factored/paired.rs\\\", \\\"src/cleanup_plan/replay/factored/paired/tests.rs\\\", \\\"src/workspace_graph/tests.rs\\\", \\\"src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs\\\", \\\"src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs\\\", \\\"src/cleanup_plan/replay/workspace_oracle.rs\\\", \";\n", "        let financed_scope = \"        for source in [\\\"src/cleanup_plan/replay/path_join/before_singleton.rs\\\", \\\"src/cleanup_plan/replay/transition.rs\\\", \\\"src/workspace_graph/initial_report/build.rs\\\", \\\"docs/AGENT-SKILL-BUNDLE-V1.json\\\", \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\", \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\", \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\", \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\", \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\", \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\", \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\", \\\"src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs\\\", \\\"src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs\\\", \\\"src/cleanup_plan/replay_tests/factored.rs\\\", \\\"src/cleanup_plan/replay/factored/moderate_tests.rs\\\", \\\"src/cleanup_plan/replay/factored/paired.rs\\\", \\\"src/cleanup_plan/replay/factored/paired/tests.rs\\\", \\\"src/workspace_graph/tests.rs\\\", \\\"src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs\\\", \\\"src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs\\\", \\\"src/cleanup_plan/replay/workspace_oracle.rs\\\", \";\n"),
        ] {
            assert!(restored.is_char_boundary(start) && restored.is_char_boundary(end));
            assert_eq!(&restored[start..end], after);
            restored.replace_range(start..end, before);
        }
        assert_eq!(restored.len(), 145335);
        assert_eq!(super::hash(restored.as_bytes()), "48386e45517906a53af81c0f58cf7c9c49a61d4abbc3ba4bee132c7f3e48130a");
        assert_eq!(std::fs::read(root.join(source)).unwrap(), compiled.as_bytes());
        return restored;
    }
    if source == "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs" {
        assert_eq!(
            supplied,
            include_str!("../main_e177_overlay_labels_v12/tests.rs")
        );
        assert_eq!(supplied.len(), 9547);
        assert_eq!(
            super::hash(supplied.as_bytes()),
            "652294002414f8f91534668666c28bec3954d1ce250cffc7fd0f3522a1356a8e"
        );
        let post = "4 {\n        super::super::verify_current_v14(&page);\n        return;\n    }\n    if page[\"schema\"] == super::super::SCHEMA_V1";
        let prior = "";
        assert_eq!(supplied.matches(post).count(), 1);
        let restored = supplied.replacen(post, prior, 1);
        assert_eq!(restored.len(), 9424);
        assert_eq!(
            super::hash(restored.as_bytes()),
            "6b316023a1d5f99cec3300b78bd84cd93077e1a806c774ab349aaea6356920e6"
        );
        return restored;
    }
    let Some(index) = V14_FRONT_SOURCES.iter().position(|path| *path == source) else {
        return supplied.to_owned();
    };
    let compiled = match index {
        0 => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py"
        )),
        1 => include_str!("../main_e177_overlay_v2.rs"),
        2 => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py"
        )),
        3 => include_str!("../main_e177_overlay_fragments_v3.rs"),
        _ => unreachable!(),
    };
    assert_eq!(supplied, compiled, "complete physical V14 caller source");
    let packet: Value = serde_json::from_str(include_str!("controller_transport.json")).unwrap();
    let front = &packet["v14_source_fronts"];
    assert_eq!(front["schema"], "semaprax.v14-physical-source-fronts.v1");
    assert_eq!(
        front["source_kind"],
        "PHYSICAL_V14_SOURCE_TO_COMPLETE_PRE_V14_INPUT_VIEW"
    );
    assert_eq!(front["sources"], serde_json::json!(V14_FRONT_SOURCES));
    let rows = front["records"].as_array().unwrap();
    assert_eq!(rows.len(), V14_FRONT_SOURCES.len());
    let names: Vec<_> = rows
        .iter()
        .map(|row| row["source"].as_str().unwrap())
        .collect();
    assert_eq!(names, V14_FRONT_SOURCES);
    let row = &rows[index];
    let target_kind = if index < 2 {
        let controllers = packet["records"].as_array().unwrap();
        let record = controllers
            .iter()
            .find(|record| record["source"] == source)
            .unwrap();
        assert_eq!(
            record["input_view_kind"],
            "PHYSICAL_V14_ACQUIRED_THEN_PRE_V14_BODY_WITH_CURRENT_POLICY_SEALS"
        );
        "PROJECTED_PRE_V14_BODY_WITH_CURRENT_POLICY_SEALS"
    } else {
        "COMPLETE_GIT424E_SOURCE_BEFORE"
    };
    assert_eq!(row["target"]["source_kind"], target_kind);
    assert_eq!(supplied.len() as u64, row["length"].as_u64().unwrap());
    assert!(supplied.len() <= 9_753_094); // Existing source domain, unchanged.
    assert_eq!(
        super::hash(supplied.as_bytes()),
        row["sha256"].as_str().unwrap()
    );
    let mut restored = supplied.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(start <= end && end <= restored.len());
        assert!(restored.is_char_boundary(start) && restored.is_char_boundary(end));
        assert_eq!(&restored[start..end], edit["after"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        assert!(restored.len() <= 9_753_094);
    }
    assert_eq!(
        restored.len() as u64,
        row["target"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        super::hash(restored.as_bytes()),
        row["target"]["sha256"].as_str().unwrap()
    );
    restored
}
