//! Two closed source acquisitions; neither retargets an original V13 endpoint.
use serde_json::Value;

#[path = "../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod physical_reader;

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

fn v14_source_before_before_v20(source: &str, supplied: &str) -> String {
    if source == OWNERS[2] {
        // Complete repaired physical source first; the immutable controller
        // routes below still consume their entire Git424e input view.
        let compiled = include_str!("../main_e177_overlay_compiled.rs");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(supplied, compiled);
        assert_eq!(
            std::fs::read(root.join(source)).unwrap(),
            compiled.as_bytes()
        );
        let joint_compiled_input = joint_compiled_source_before(supplied);
        let supplied = joint_compiled_input.as_deref().unwrap_or(supplied);
        // Acquire the entire pre-observer compiled input before its unchanged history.
        let observer_compiled_input;
        let supplied = if super::hash(supplied.as_bytes()) == "616ca00e0b195931261dc80ea32312e84092863620883bb747605f1ec23e9caf" {
            assert_eq!(supplied.len(), 157253);
            assert_eq!(physical_reader::read(&root.join(source), 157253,
                "616ca00e0b195931261dc80ea32312e84092863620883bb747605f1ec23e9caf", 9_753_094).unwrap(), compiled);
            observer_compiled_input = physical_reader::read(
                &root.join("e/84242ca46276900f23cc4442ae9aa64fe47f81076fd0511e1d5a979998675b11"), 156891, "84242ca46276900f23cc4442ae9aa64fe47f81076fd0511e1d5a979998675b11", 9_753_094).unwrap();
            observer_compiled_input.as_str()
        } else { supplied };
        // Acquire the complete reviewed pre-repair compiled input before its unchanged history.
        let repair_compiled_input;
        let supplied = if super::hash(supplied.as_bytes()) == "84242ca46276900f23cc4442ae9aa64fe47f81076fd0511e1d5a979998675b11" {
            assert_eq!(supplied.len(), 156891);
            if super::hash(compiled.as_bytes()) == "616ca00e0b195931261dc80ea32312e84092863620883bb747605f1ec23e9caf" {
                assert_eq!(physical_reader::read(&root.join("e/84242ca46276900f23cc4442ae9aa64fe47f81076fd0511e1d5a979998675b11"), 156891,
                    "84242ca46276900f23cc4442ae9aa64fe47f81076fd0511e1d5a979998675b11", 9_753_094).unwrap(), supplied);
            } else {
            assert_eq!(physical_reader::read(&root.join(source), 156891,
                "84242ca46276900f23cc4442ae9aa64fe47f81076fd0511e1d5a979998675b11", 9_753_094).unwrap(), compiled);
            }
            repair_compiled_input = physical_reader::read(
                &root.join("e/777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3"),
                154097, "777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3", 9_753_094).unwrap();
            repair_compiled_input.as_str()
        } else {
            supplied
        };
        // Acquire complete mainb7 source before the unchanged loader history.
        let mainb7_compiled_input;
        let supplied = if super::hash(supplied.as_bytes()) == "777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3" {
            assert_eq!(supplied.len(), 154097);
            let mut restored = supplied.to_owned();
            for &(start, end, after, before) in [
                (45382, 45937, " /* The new test-wire helper has complete current physical and compiled channels before its exact admission is removed for the old proof. */ let helper = \"tests/workspace/semantic_graph/host_pins.rs\"; let physical = std::fs::read_to_string( std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(helper), ).unwrap(); assert_eq!(super::compiled(helper), Some(physical.as_str())); let expected = format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(physical.as_bytes()))); assert_eq!(super::super::value_direct(helper, &expected, &physical), physical);", ""),
                (118072, 118944, "    #[rustfmt::skip] #[test] fn remaining_source_view_requires_the_complete_current_compiled_channel() { use sha2::{Digest, Sha256}; let source = \"src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs\"; let raw = include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \"/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs\" )); assert_eq!(super::compiled(source), Some(raw)); let old = crate::source_reflows_merge::main_e177_final_inputs::value_direct( source, \"e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4\", raw, ); assert_eq!( old, include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \"/e/e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4\" )) ); assert_eq!( format!( \"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes())) ), \"e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4\" ); } ", ""),
                (131340, 131685, "hostpin_arm = \"        \\\"tests/workspace/semantic_graph/host_pins.rs\\\" => Some(include_str!(\\\"../../tests/workspace/semantic_graph/host_pins.rs\\\")),\\n\"; assert_eq!(lookup.matches(hostpin_arm).count(), 1, \"exact incoming host-pin lookup arm\"); let before_hostpins = lookup.replacen(hostpin_arm, \"\", 1); let lookup = before_hostpins.as_str(); let ", ""),
            ].iter().rev() {
                assert!(start <= end && end <= restored.len());
                assert!(restored.is_char_boundary(start) && restored.is_char_boundary(end));
                assert_eq!(&restored[start..end], after);
                restored.replace_range(start..end, before);
            }
            assert_eq!(restored.len(), 152325);
            assert_eq!(super::hash(restored.as_bytes()), "7e88be48fafcc0a596fef3b48630452f45ae9441ab98cd1733224644e2151be6");
            mainb7_compiled_input = restored;
            mainb7_compiled_input.as_str()
        } else {
            supplied
        };
        // Exact main8/constructor registration source phase; prior epochs remain complete.
        // Complete actual loader-fixture source precedes its immutable pre-V15 input view.
        let v15_loader_input;
        let supplied = if super::hash(supplied.as_bytes())
            == "7e88be48fafcc0a596fef3b48630452f45ae9441ab98cd1733224644e2151be6"
        {
            assert_eq!(supplied.len(), 152325);
            let post = "let current_loader = LOADER.replacen(\" length=10905\\n\", &format!(\" length={}\\n\", CODECS.len()), 1).replacen(\"a0f906f11a8c7205a2124e5cd9c1909fa1a53a12731a0775d3e360d844bab794\", &digest(CODECS), 1);";
            let prior = "let current_loader = LOADER.replacen(\" length=10905\\n\", \" length=23065\\n\", 1).replacen(\"a0f906f11a8c7205a2124e5cd9c1909fa1a53a12731a0775d3e360d844bab794\", \"9cae8c3b1db0d54de8ce9d66639dab5de6c16c9e5772d1503b922482b1b2a1fb\", 1);";
            assert_eq!(supplied.matches(post).count(), 1);
            let restored = supplied.replacen(post, prior, 1);
            assert_eq!(restored.len(), 152355);
            assert_eq!(
                super::hash(restored.as_bytes()),
                "3f0857f8d893c058137794c6a1aa533dfe6092746ad0cbdea1bef020c12d60cf"
            );
            v15_loader_input = restored;
            v15_loader_input.as_str()
        } else {
            supplied
        };
        assert_eq!(supplied.len(), 152355);
        assert_eq!(
            super::hash(supplied.as_bytes()),
            "3f0857f8d893c058137794c6a1aa533dfe6092746ad0cbdea1bef020c12d60cf"
        );
        let mut main8_scope_epoch = supplied.to_owned();
        let main8_scope_edits: &[(usize, &str, &str)] = &[
            (129739, "    #[rustfmt::skip] pub(super) fn parent_for_current_scope(parent: &str, lookup: &str) -> String {\n", "    pub(super) fn parent_for_current_scope(parent: &str, lookup: &str) -> String {\n"),
            (129873, "        assert_eq!(lookup, LOOKUP); let main8_903_arms = \"        \\\"src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/retained_scratch.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/retained_scratch.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/uncached_peak_before.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/uncached_peak_before.rs\\\")),\\n        \\\"src/hir.rs\\\" => Some(include_str!(\\\"../../src/hir.rs\\\")),\\n        \\\"src/hir/function_directory.rs\\\" => Some(include_str!(\\\"../../src/hir/function_directory.rs\\\")),\\n        \\\"src/loan_plan/owned_capacity.rs\\\" => Some(include_str!(\\\"../../src/loan_plan/owned_capacity.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/held_projection_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/held_projection_original.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/prelude_forecast_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/prelude_forecast_original.rs\\\")),\\n        \\\"src/workspace_graph/type_proof_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/type_proof_tests.rs\\\")),\\n        \\\"src/workspace_graph/tests/builder_limits.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/tests/builder_limits.rs\\\")),\\n        \\\"src/bounded_output/stage_debits.rs\\\" => Some(include_str!(\\\"../../src/bounded_output/stage_debits.rs\\\")),\\n        \\\"src/cleanup_plan/renewal_profile/child_refusal.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/renewal_profile/child_refusal.rs\\\")),\\n        \\\"src/cleanup_plan/replay/branch_clone/before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/branch_clone/before.rs\\\")),\\n        \\\"src/cleanup_plan/replay/compact_stage.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/compact_stage.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/oracle_order/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/oracle_order/tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_array/funded_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_array/funded_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_array/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_array/tests.rs\\\")),\\n        \\\"src/source_reflows_merge/maina3_custody.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/maina3_custody.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/compact_nodes/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/compact_nodes/tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/selected_context/child.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/selected_context/child.rs\\\")),\\n        \\\"src/workspace_graph/diagnostics.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/diagnostics.rs\\\")),\\n        \\\"src/workspace_graph/diagnostics/borrowed_help_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/diagnostics/borrowed_help_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(main8_903_arms).count(), 1, \"exact main8 and constructor-retention lookup suffix\"); let prior_main8_903_lookup = lookup.replacen(main8_903_arms, \"\", 1); assert_eq!(prior_main8_903_lookup.len(), 76063); assert_eq!(digest(&prior_main8_903_lookup), \"c621ebfa81b46f511fda7446d73f2b6a3dc5a6c73d5b3a08f426cf01c1e437cb\"); let lookup = prior_main8_903_lookup.as_str(); let next_leaf_arm_0 = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_0).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_0 = lookup.replacen(next_leaf_arm_0, \"\", 1); let lookup = prior_leaf_lookup_0.as_str(); let next_leaf_arm_1 = \"        \\\"tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\" => Some(include_str!(\\\"../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_1).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_1 = lookup.replacen(next_leaf_arm_1, \"\", 1); let lookup = prior_leaf_lookup_1.as_str(); let main3d81_arms = \"        \\\"src/cleanup_plan/replay/transition.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/transition.rs\\\")),\\n        \\\"src/workspace_graph/initial_report/build.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/initial_report/build.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, \"exact incoming3d81 lookup arms and complete oracle\"); let main3d81_lookup = lookup.replacen(main3d81_arms, \"\", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = \"        \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(field_local_arms).count(), 1, \"exact four field-local lookup arms\"); let field_prior_lookup = lookup.replacen(field_local_arms, \"\", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\")),\\n\"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, \"exact new retained-fork lookup arm\"); let prior_lookup = lookup.replacen(retained_leaf_arm, \"\", 1); let lookup = prior_lookup.as_str();\n", "        assert_eq!(lookup, LOOKUP); let next_leaf_arm_0 = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_0).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_0 = lookup.replacen(next_leaf_arm_0, \"\", 1); let lookup = prior_leaf_lookup_0.as_str(); let next_leaf_arm_1 = \"        \\\"tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\" => Some(include_str!(\\\"../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_1).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_1 = lookup.replacen(next_leaf_arm_1, \"\", 1); let lookup = prior_leaf_lookup_1.as_str(); let main3d81_arms = \"        \\\"src/cleanup_plan/replay/transition.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/transition.rs\\\")),\\n        \\\"src/workspace_graph/initial_report/build.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/initial_report/build.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, \"exact incoming3d81 lookup arms and complete oracle\"); let main3d81_lookup = lookup.replacen(main3d81_arms, \"\", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = \"        \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(field_local_arms).count(), 1, \"exact four field-local lookup arms\"); let field_prior_lookup = lookup.replacen(field_local_arms, \"\", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\")),\\n\"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, \"exact new retained-fork lookup arm\"); let prior_lookup = lookup.replacen(retained_leaf_arm, \"\", 1); let lookup = prior_lookup.as_str();\n"),
        ];
        for &(at, after, before) in main8_scope_edits.iter().rev() {
            assert_eq!(main8_scope_epoch.get(at..at + after.len()), Some(after));
            main8_scope_epoch.replace_range(at..at + after.len(), before);
        }
        assert_eq!(main8_scope_epoch.len(), 145546);
        assert_eq!(
            super::hash(main8_scope_epoch.as_bytes()),
            "213369b5e6e3332c392149132b04409d6c7f1100988a27c0cd13b36e6fdad93d"
        );
        let supplied = main8_scope_epoch.as_str();
        // Exact new two-child source epoch before the entire old INPUT.
        assert_eq!(supplied.len(), 145546);
        assert_eq!(
            super::hash(supplied.as_bytes()),
            "213369b5e6e3332c392149132b04409d6c7f1100988a27c0cd13b36e6fdad93d"
        );
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
        assert_eq!(
            super::hash(supplied.as_bytes()),
            "84f65e1495528a262750473e8ea36acd2d921e009e115fa5387bf802c81fd0a1"
        );
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
        assert_eq!(
            super::hash(restored.as_bytes()),
            "48386e45517906a53af81c0f58cf7c9c49a61d4abbc3ba4bee132c7f3e48130a"
        );
        assert_eq!(
            std::fs::read(root.join(source)).unwrap(),
            compiled.as_bytes()
        );
        return restored;
    }
    if source == "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs" {
        assert_eq!(
            supplied,
            include_str!("../main_e177_overlay_labels_v12/tests.rs")
        );
        // Acquire the entire physical V16 source before the unchanged V15 inverse.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let compiled = supplied;
        let v16_labels_input;
        let supplied = if super::hash(supplied.as_bytes())
            == "0421597ed21e1938cc215e2eea2ac2461b6d3324527df9833e1769688f1e65f2"
        {
            assert_eq!(supplied.len(), 9793);
            assert_eq!(
                physical_reader::read(&root.join(source), 9793,
                    "0421597ed21e1938cc215e2eea2ac2461b6d3324527df9833e1769688f1e65f2", 9_753_094).unwrap(),
                compiled
            );
            let post = "    if page[\"schema\"] == super::super::SCHEMA_V16 {\n        super::super::verify_current_v16(&page);\n        return;\n    }\n";
            assert_eq!(supplied.matches(post).count(), 1);
            let restored = supplied.replacen(post, "", 1);
            assert_eq!(restored.len(), 9670);
            assert_eq!(super::hash(restored.as_bytes()),
                "e1f1e7883e7b9930c68b345102fef5efb7caf4825164d75df5d130ac9b6872d9");
            v16_labels_input = restored;
            v16_labels_input.as_str()
        } else {
            supplied
        };
        // Acquire the complete V15 test source before its one exact historical dispatch inverse.
        let v15_labels_input;
        let supplied = if super::hash(supplied.as_bytes())
            == "e1f1e7883e7b9930c68b345102fef5efb7caf4825164d75df5d130ac9b6872d9"
        {
            assert_eq!(supplied.len(), 9670);
            assert_eq!(
                std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source)).unwrap(),
                compiled.as_bytes()
            );
            let post = "    if page[\"schema\"] == super::super::SCHEMA_V15 {\n        super::super::verify_current_v15(&page);\n        return;\n    }\n";
            assert_eq!(supplied.matches(post).count(), 1);
            let restored = supplied.replacen(post, "", 1);
            assert_eq!(restored.len(), 9547);
            assert_eq!(
                super::hash(restored.as_bytes()),
                "652294002414f8f91534668666c28bec3954d1ce250cffc7fd0f3522a1356a8e"
            );
            v15_labels_input = restored;
            v15_labels_input.as_str()
        } else {
            supplied
        };
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
        if compiled.len() == 9793 {
            assert_eq!(physical_reader::read(&root.join(source), 9793,
                "0421597ed21e1938cc215e2eea2ac2461b6d3324527df9833e1769688f1e65f2", 9_753_094).unwrap(), compiled);
        }
        return restored;
    }
    let Some(index) = V14_FRONT_SOURCES.iter().position(|path| *path == source) else {
        return supplied.to_owned();
    };
    let compiled = v20_previous_compiled(index);
    assert_eq!(supplied, compiled, "complete physical V14 caller source");
    let packet: Value = serde_json::from_str(include_str!("controller_transport.json")).unwrap();
    let front = &packet["v14_source_fronts"];
    let physical = &packet["v16_physical_front"];
    assert_eq!(physical["schema"], "semaprax.v16-physical-source-fronts.v1");
    assert_eq!(physical["source_kind"], "COMPLETE_PHYSICAL_V16_TO_UNCHANGED_V15_INPUT");
    assert_eq!(physical["sources"], serde_json::json!(V14_FRONT_SOURCES));
    let physical_rows = physical["records"].as_array().unwrap();
    assert_eq!(physical_rows.len(), V14_FRONT_SOURCES.len());
    let physical_names: Vec<_> = physical_rows.iter()
        .map(|row| row["source"].as_str().unwrap()).collect();
    assert_eq!(physical_names, V14_FRONT_SOURCES);
    let physical_row = &physical_rows[index];
    let physical_length = usize::try_from(physical_row["length"].as_u64().unwrap()).unwrap();
    let physical_sha = physical_row["sha256"].as_str().unwrap();
    assert!(physical_length <= 9_753_094);
    assert_eq!(supplied.len(), physical_length);
    assert_eq!(super::hash(supplied.as_bytes()), physical_sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(physical_reader::read(&root.join(v20_previous_archive(index)), physical_length,
        physical_sha, 9_753_094).unwrap(), compiled);
    let mut v16_input = supplied.to_owned();
    for edit in physical_row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        let before = edit["before"].as_str().unwrap();
        assert!(start <= end && end <= v16_input.len());
        assert!(v16_input.is_char_boundary(start) && v16_input.is_char_boundary(end));
        assert_eq!(&v16_input[start..end], edit["after"].as_str().unwrap());
        let restored_length = v16_input.len().checked_sub(end - start).unwrap()
            .checked_add(before.len()).unwrap();
        assert!(restored_length <= 9_753_094);
        v16_input.replace_range(start..end, before);
    }
    assert_eq!(v16_input.len() as u64, physical_row["target"]["length"].as_u64().unwrap());
    assert_eq!(super::hash(v16_input.as_bytes()), physical_row["target"]["sha256"].as_str().unwrap());
    // Retain the complete V16-to-V15 replay above. The current V14 front
    // independently authenticates the physical source, whereas older packets
    // acquired the unchanged V15 input. Select only the exact recorded stage.
    let front_row = &front["records"][index];
    let supplied = if front_row["length"] == physical_row["length"]
        && front_row["sha256"] == physical_row["sha256"]
    {
        supplied
    } else {
        assert_eq!(physical_row["target"]["length"], front_row["length"]);
        assert_eq!(physical_row["target"]["sha256"], front_row["sha256"]);
        v16_input.as_str()
    };
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
    assert_eq!(physical_reader::read(&root.join(v20_previous_archive(index)), physical_length,
        physical_sha, 9_753_094).unwrap(), compiled);
    restored
}

// Exact next-joint compiled-source ingress to the complete retained mainb7 image.
fn joint_compiled_source_before(supplied: &str) -> Option<String> {
    const SOURCE: &str = "src/source_reflows_merge/main_e177_overlay_compiled.rs";
    const CURRENT: &str = include_str!("../main_e177_overlay_compiled.rs");
    const PRIOR: &str = include_str!("../../../e/d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679");
    const SHA: &str = "783a685712dfb5be8792f0915156d61d7ab6b136765215565aa05f22eda5d37d";
    if supplied.len() != 183704 || super::hash(supplied.as_bytes()) != SHA { return None; }
    assert_eq!(supplied, CURRENT, "whole current compiled source before named predecessor");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = physical_reader::read(&root.join(SOURCE), CURRENT.len(), SHA, 9_753_094).unwrap();
    assert_eq!(physical, CURRENT);
    assert_eq!(physical_reader::read(&root.join("e/d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679"), PRIOR.len(), "d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679", 9_753_094).unwrap(), PRIOR);
    let mut prior = supplied.to_owned();
    let edits: &[(usize, usize, &str, &str)] = &[
        (102, 102, "", "\n"),
        (184, 184, "", "\n"),
        (664, 664, "", "\n"),
        (1009, 1009, "", "\n"),
        (131493, 135338, "         assert_eq!(parent,ROOT);assert_eq!(lookup,LOOKUP);         let next_root=std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"));         let next_actual=super::main48_historical_reader::read(&next_root.join(\"src/source_reflows_merge/main_e177_overlay_lookup.rs\"), 96793,\"65bdc9a6c3a63378911550dd87d5ec8a7bc35576812dcc4eecd572f645a4d9bf\",2_097_152).unwrap();assert_eq!(next_actual,LOOKUP);         let next_parent=super::main48_historical_reader::read(&next_root.join(\"src/source_reflows_merge/main_e177_overlay_compiled.rs\"),ROOT.len(),&digest(ROOT),2_097_152).unwrap();assert_eq!(next_parent,ROOT);         let mut next_before=lookup.to_owned();for arm in [\"        \\\"src/cleanup_plan/replay/trailing_active.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/trailing_active.rs\\\")),\\n\",\"        \\\"src/cleanup_plan/replay/trailing_active/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/trailing_active/tests.rs\\\")),\\n\"] {assert_eq!(next_before.matches(arm).count(),1);next_before=next_before.replacen(arm,\"\",1);}         const NEXT_BEFORE:&str=include_str!(\"../../e/28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\");assert_eq!(next_before,NEXT_BEFORE);assert_eq!(super::main48_historical_reader::read(&next_root.join(\"e/28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\"),96525,\"28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\",2_097_152).unwrap(),NEXT_BEFORE);         let next_result=(|| {const LOOKUP:&str=include_str!(\"../../e/28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\");let lookup=next_before.as_str();\n         assert_eq!(parent, ROOT); assert_eq!(lookup, LOOKUP);         let registration_root = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"));         let registration_actual = super::main48_historical_reader::read(&registration_root.join(\"e/28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\"), 96525, \"28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\", 2_097_152).unwrap();         assert_eq!(registration_actual, LOOKUP);         let registration_parent = super::main48_historical_reader::read(&registration_root.join(\"src/source_reflows_merge/main_e177_overlay_compiled.rs\"), ROOT.len(), &digest(ROOT), 2_097_152).unwrap(); assert_eq!(registration_parent, ROOT);         let mut registration_prior = lookup.to_owned();         for arm in [\"        \\\"src/source_reflows/ci_inputs_current_epoch.rs\\\" => Some(include_str!(\\\"../../src/source_reflows/ci_inputs_current_epoch.rs\\\")),\\n\", \"        \\\"src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_before_protocol/owner.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_before_protocol/owner.rs\\\")),\\n\", \"        \\\"src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_before_protocol/held_invocation_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_before_protocol/held_invocation_tests.rs\\\")),\\n\"] { assert_eq!(registration_prior.matches(arm).count(), 1); registration_prior = registration_prior.replacen(arm, \"\", 1); }         const REGISTRATION_BEFORE: &str = include_str!(\"../../e/758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\");         assert_eq!(registration_prior, REGISTRATION_BEFORE);         assert_eq!(super::main48_historical_reader::read(&registration_root.join(\"e/758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\"), 95867, \"758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\", 2_097_152).unwrap(), REGISTRATION_BEFORE);         let registration_result = (|| { const LOOKUP: &str = include_str!(\"../../e/758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\"); let lookup = registration_prior.as_str(); \n", ""),
        (135558, 137088, "e/758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\"), 95867, \"758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\", 2_097_152).expect(\"complete actual current joint compiled lookup\"); assert_eq!(joint_actual_lookup, LOOKUP); assert_eq!((lookup.len(), digest(lookup)), (95867, \"758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\".to_owned())); let mut v19_lookup = lookup.to_owned(); for arm in [\"        \\\"src/source_reflows_merge/main_e177_overlay_compiled_successor_v19.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/main_e177_overlay_compiled_successor_v19.rs\\\")),\\n\",\"        \\\"src/source_reflows_merge/main_e177_overlay_compiled_successor_v19/tests.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/main_e177_overlay_compiled_successor_v19/tests.rs\\\")),\\n\"] { assert_eq!(v19_lookup.matches(arm).count(), 1, \"exact V19 compiled lookup arm\"); v19_lookup = v19_lookup.replacen(arm, \"\", 1); } assert_eq!((v19_lookup.len(), digest(&v19_lookup)), (95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\".to_owned())); assert_eq!(v19_lookup, include_str!(\"../../e/bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\"), \"ENTIRE named pre-V19 compiled lookup\"); assert_eq!(super::main48_historical_reader::read(&joint_root.join(\"e/bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\"), 95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\", 2_097_152).unwrap(), v19_lookup); let lookup = v19_lookup.as_str(", "src/source_reflows_merge/main_e177_overlay_lookup.rs\"), 95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\", 2_097_152).expect(\"complete actual current joint compiled lookup\"); assert_eq!(joint_actual_lookup, LOOKUP); assert_eq!((lookup.len(), digest(lookup)), (95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\".to_owned())"),
        (168131, 169314, "e/758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\"), 95867, \"758584ac732d901b9467614e2b4c672b83f320429419888b15648ff1b2db124b\", 2_097_152).unwrap(), joint_actual_lookup); restored.replacen(BRIDGE, \"\", 1)\n             })();         assert_eq!(super::main48_historical_reader::read(&registration_root.join(\"e/28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\"), 96525, \"28cda8999aae299ae9ab1af5f1529428d6cc77b97b801c069fbde0690e559581\", 2_097_152).unwrap(), registration_actual);         assert_eq!(super::main48_historical_reader::read(&registration_root.join(\"src/source_reflows_merge/main_e177_overlay_compiled.rs\"), ROOT.len(), &digest(ROOT), 2_097_152).unwrap(), registration_parent);         registration_result \n         })();assert_eq!(super::main48_historical_reader::read(&next_root.join(\"src/source_reflows_merge/main_e177_overlay_lookup.rs\"),96793,\"65bdc9a6c3a63378911550dd87d5ec8a7bc35576812dcc4eecd572f645a4d9bf\",2_097_152).unwrap(),next_actual);assert_eq!(super::main48_historical_reader::read(&next_root.join(\"src/source_reflows_merge/main_e177_overlay_compiled.rs\"),ROOT.len(),&digest(ROOT),2_097_152).unwrap(),next_parent);next_result\n", "src/source_reflows_merge/main_e177_overlay_lookup.rs\"), 95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\", 2_097_152).unwrap(), joint_actual_lookup); restored.replacen(BRIDGE, \"\", 1)\n    ")
    ];
    for &(start, end, after, before) in edits.iter().rev() {
        assert!(start <= end && end <= prior.len());
        assert!(prior.is_char_boundary(start) && prior.is_char_boundary(end));
        assert_eq!(&prior[start..end], after);
        prior.replace_range(start..end, before);
    }
    assert_eq!(prior, PRIOR, "entire immediate prior compiled source");
    let result = joint_compiled_source_before_current1042(&prior);
    assert!(result.is_some(), "complete original joint route still executes");
    assert_eq!(physical_reader::read(&root.join(SOURCE), CURRENT.len(), SHA, 9_753_094).unwrap(), physical);
    assert_eq!(physical_reader::read(&root.join("e/d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679"), PRIOR.len(), "d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679", 9_753_094).unwrap(), PRIOR);
    result
}

fn joint_compiled_source_before_current1042(supplied: &str) -> Option<String> {
    const SOURCE: &str = "e/d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679";
    const CURRENT: &str = include_str!("../../../e/d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679");
    const BEFORE: &str = include_str!("../../../e/777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3");
    const SHA: &str = "d89a458c6d798b37f185929e312aecdb3f0ddc1eff908127a8d3677050773679";
    if supplied.len() != 177730 || super::hash(supplied.as_bytes()) != SHA { return None; }
    assert_eq!(supplied, CURRENT, "complete supplied joint compiled source");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = physical_reader::read(&root.join(SOURCE), supplied.len(), SHA, 9_753_094).unwrap();
    assert_eq!(physical, CURRENT, "whole physical and compiled joint source before historical inverse");
    let mut prior = supplied.to_owned();
    let edits: &[(usize, usize, &str, &str)] = &[
        (10251, 10418, "            \"src/workspace_graph/expected_projection/borrowed_imports/tests.rs\",\n            \"src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs\",\n", ""),
        (28625, 28792, "            \"src/workspace_graph/expected_projection/borrowed_imports/tests.rs\",\n            \"src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs\",\n", ""),
        (119360, 120200, "#[path = \"../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs\"] mod main48_historical_reader; #[cfg(test)] mod census_split { use sha2::{Digest, Sha256}; const ROOT: &str = include_str!(\"main_e177_overlay_compiled.rs\"); const LOOKUP: &str = include_str!(\"main_e177_overlay_lookup.rs\"); const BEFORE: &str = include_str!(\"main_e177_overlay_compiled.before.rs.txt\"); const PYTHON: &str = include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py\" )); const CODECS: &str = include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py\" )); const PYTHON_BEFORE: &str = include_str!(concat!( env!(\"CARGO_MANIFEST_DIR\"), \"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.before.py.txt\" ));\n", "#[cfg(test)]\nmod census_split {\n    use sha2::{Digest, Sha256};\n    const ROOT: &str = include_str!(\"main_e177_overlay_compiled.rs\");\n    const LOOKUP: &str = include_str!(\"main_e177_overlay_lookup.rs\");\n    const BEFORE: &str = include_str!(\"main_e177_overlay_compiled.before.rs.txt\");\n    const PYTHON: &str = include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py\"\n    ));\n    const CODECS: &str = include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py\"\n    ));\n    const PYTHON_BEFORE: &str = include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.before.py.txt\"\n    ));\n"),
        (121661, 122524, "    const KEYS: &[&str] = &[ \"src/cleanup_plan/replay/skeleton_work/query_proof.rs\", \"src/cleanup_plan/replay/skeleton_work/query_proof/before.rs\", \"src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs\", \"src/bounded_output/exclusive_debits/path_census.rs\", \"src/bounded_output/request_reservation.rs\", \"src/source_reflows_merge/main_e177_overlay_lookup.rs\", \"src/hir/inspection/paid_nul.rs\", \"src/hir/inspection/paid_nul/pending.rs\", \"src/hir/inspection/paid_nul/tests.rs\", \"src/hir/inspection/paid_nul/reuse_before.rs\", \"src/hir/inspection/paid_nul/reuse_before/pending.rs\", \"src/hir/inspection/paid_nul/type_scratch.rs\", \"src/cleanup_plan/replay/continuation_storage.rs\", \"src/cleanup_plan/replay/continuation_storage/tests.rs\", \"src/cleanup_plan/replay/continuation_storage/reached.rs\", \"src/cleanup_plan/replay/continuation_storage/reached_before.rs\",\n", "    const KEYS: &[&str] = &[\n        \"src/cleanup_plan/replay/skeleton_work/query_proof.rs\",\n        \"src/cleanup_plan/replay/skeleton_work/query_proof/before.rs\",\n        \"src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs\",\n        \"src/bounded_output/exclusive_debits/path_census.rs\",\n        \"src/bounded_output/request_reservation.rs\",\n        \"src/source_reflows_merge/main_e177_overlay_lookup.rs\",\n            \"src/hir/inspection/paid_nul.rs\",\n        \"src/hir/inspection/paid_nul/pending.rs\",\n        \"src/hir/inspection/paid_nul/tests.rs\",\n        \"src/hir/inspection/paid_nul/reuse_before.rs\",\n        \"src/hir/inspection/paid_nul/reuse_before/pending.rs\",\n        \"src/hir/inspection/paid_nul/type_scratch.rs\",\n        \"src/cleanup_plan/replay/continuation_storage.rs\",\n        \"src/cleanup_plan/replay/continuation_storage/tests.rs\",\n        \"src/cleanup_plan/replay/continuation_storage/reached.rs\",\n        \"src/cleanup_plan/replay/continuation_storage/reached_before.rs\",\n"),
        (131532, 161844, "        assert_eq!(lookup, LOOKUP); let joint_root = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")); let joint_actual_lookup = super::main48_historical_reader::read(&joint_root.join(\"src/source_reflows_merge/main_e177_overlay_lookup.rs\"), 95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\", 2_097_152).expect(\"complete actual current joint compiled lookup\"); assert_eq!(joint_actual_lookup, LOOKUP); assert_eq!((lookup.len(), digest(lookup)), (95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\".to_owned())); let mut joint_lookup = lookup.to_owned(); for arm in [\"        \\\"src/workspace_graph/expected_projection/borrowed_imports.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports.rs\\\")),\\n\",\"        \\\"src/source_reflows_merge/final_ready_inputs/merged_projection.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/final_ready_inputs/merged_projection.rs\\\")),\\n\",\"        \\\"src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation.rs\\\")),\\n\",\"        \\\"src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_tests.rs\\\")),\\n\",\"        \\\"src/cleanup_plan/replay/expression_facts/receiving/retained/held_status_invocation.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/retained/held_status_invocation.rs\\\")),\\n\",\"        \\\"src/cleanup_plan/replay/fact_preflight/before_status_receivers.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/fact_preflight/before_status_receivers.rs\\\")),\\n\"] { assert_eq!(joint_lookup.matches(arm).count(), 1); joint_lookup = joint_lookup.replacen(arm, \"\", 1); } assert_eq!((joint_lookup.len(), digest(&joint_lookup)), (94357, \"87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\".to_owned())); assert_eq!(joint_lookup, include_str!(\"../../e/87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\"), \"ENTIRE named pre-registration lookup\"); assert_eq!(super::main48_historical_reader::read(&joint_root.join(\"e/87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\"), 94357, \"87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\", 2_097_152).unwrap(), joint_lookup); let lookup = joint_lookup.as_str(); let main_ac791_arm = \"        \\\"src/workspace_graph/expected_projection/borrowed_imports_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports_original.rs\\\")),\\n\"; assert_eq!(lookup.len(), 94357); assert_eq!(digest(lookup), \"87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\"); let actual_main_ac791_lookup = super::main48_historical_reader::read(&std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"e/87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\"), 94357, \"87e1e8f1d4986faba675ed8a1465f1b575b5655b6f2343bb8bfb5d59a371ca83\", 2_097_152).expect(\"complete named pre-registration compiled lookup physical image\"); assert_eq!(actual_main_ac791_lookup, lookup); assert_eq!(lookup.matches(main_ac791_arm).count(), 1, \"exact new main borrowed original lookup arm\"); let before_main_ac791_admission = lookup.replacen(main_ac791_arm, \"\", 1); assert_eq!(before_main_ac791_admission.len(), 94177); assert_eq!(digest(&before_main_ac791_admission), \"4205124f1f409cdd227d4fb14c3a931bce34d527c937d8c10383943066c96954\"); let lookup = before_main_ac791_admission.as_str(); assert_eq!(lookup.len(), 94177); assert_eq!(digest(lookup), \"4205124f1f409cdd227d4fb14c3a931bce34d527c937d8c10383943066c96954\"); let mut before_main48_admission = lookup.to_owned(); for arm in [\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/tests.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/full.rs\\\")),\\n\"] { assert_eq!(before_main48_admission.matches(arm).count(), 1, \"exact main48 two compiled lookup arms\"); before_main48_admission = before_main48_admission.replacen(arm, \"\", 1); } assert_eq!(before_main48_admission.len(), 93819); assert_eq!(digest(&before_main48_admission), \"35c97c45831981c91aea6d9253aebf59eee044081d7c336abf4c244274a0ae2f\"); let lookup = before_main48_admission.as_str(); assert_eq!(lookup.len(), 93819); assert_eq!(digest(lookup), \"35c97c45831981c91aea6d9253aebf59eee044081d7c336abf4c244274a0ae2f\"); let mut before_direct_place_admission = lookup.to_owned(); for arm in [\"        \\\"src/hir/validation/scope_checkpoint/marks/direct_before.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/scope_checkpoint/marks/direct_before.rs\\\")),\\n\",\"        \\\"src/hir/validation/scope_checkpoint/marks/direct_tests.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/scope_checkpoint/marks/direct_tests.rs\\\")),\\n\"] { assert_eq!(before_direct_place_admission.matches(arm).count(), 1, \"exact direct Place compiled lookup arms\"); before_direct_place_admission = before_direct_place_admission.replacen(arm, \"\", 1); } assert_eq!(before_direct_place_admission.len(), 93501); assert_eq!(digest(&before_direct_place_admission), \"6d4c6200aa557eae5b04014a5597fdc1bd188a676609a81cf2313180e49c3b3d\"); let lookup = before_direct_place_admission.as_str(); assert_eq!(lookup.len(), 93501); assert_eq!(digest(lookup), \"6d4c6200aa557eae5b04014a5597fdc1bd188a676609a81cf2313180e49c3b3d\"); let mut before_main328_admission = lookup.to_owned(); for arm in [\"        \\\"src/source_reflows_merge/constructor_stage.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/constructor_stage.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/defaults/field_directory.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/defaults/field_directory.rs\\\")),\\n\",\"        \\\"src/vec_ops/wrappers.rs\\\" => Some(include_str!(\\\"../../src/vec_ops/wrappers.rs\\\")),\\n\",\"        \\\"src/box_ops/wrappers.rs\\\" => Some(include_str!(\\\"../../src/box_ops/wrappers.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/defaults.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/defaults.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/import_stub.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/import_stub.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs\\\")),\\n\",\"        \\\"src/ast/program_source/decl_slices/alias_tests.rs\\\" => Some(include_str!(\\\"../../src/ast/program_source/decl_slices/alias_tests.rs\\\")),\\n\",\"        \\\"src/ast/program_source/decl_slices/before_alias.rs\\\" => Some(include_str!(\\\"../../src/ast/program_source/decl_slices/before_alias.rs\\\")),\\n\",\"        \\\"src/ast/program_source/function_alias.rs\\\" => Some(include_str!(\\\"../../src/ast/program_source/function_alias.rs\\\")),\\n\",\"        \\\"src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs\\\")),\\n\",\"        \\\"src/hir/identity_resolution/nominal.rs\\\" => Some(include_str!(\\\"../../src/hir/identity_resolution/nominal.rs\\\")),\\n\"] { assert_eq!(before_main328_admission.matches(arm).count(), 1, \"exact incoming fifteen compiled lookup arms\"); before_main328_admission = before_main328_admission.replacen(arm, \"\", 1); } assert_eq!(before_main328_admission.len(), 91159); assert_eq!(digest(&before_main328_admission), \"228b873c91c1283c148742795dc45bf8b4464a49d0e574b02046762336b3e30d\"); let lookup = before_main328_admission.as_str(); assert_eq!(lookup.len(), 91159); assert_eq!(digest(lookup), \"228b873c91c1283c148742795dc45bf8b4464a49d0e574b02046762336b3e30d\"); let closed_roots_test_arm = \"        \\\"src/workspace_graph/expected_projection/call_carriers/closed_roots_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/call_carriers/closed_roots_tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(closed_roots_test_arm).count(), 1, \"exact cfg(test) closed-roots extraction arm\"); let before_closed_roots_extraction = lookup.replacen(closed_roots_test_arm, \"\", 1); let lookup = before_closed_roots_extraction.as_str(); assert_eq!(lookup.len(), 90965); assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(lookup.as_bytes()))), \"64f9913b90f4f7d68ff9bad77634f1814085d47b39456587f3946ed590028b78\"); let admission1011_arms = \"        \\\"src/source_reflows_merge/main_e177_overlay_names_v11/main_e177_overlay_pins_v10.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/main_e177_overlay_names_v11/main_e177_overlay_pins_v10.rs\\\")),\\n        \\\"src/source_reflows_merge/main_e177_overlay_incoming064.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/main_e177_overlay_incoming064.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs\\\")),\\n        \\\"src/cleanup_plan/replay/projection_backing.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/projection_backing.rs\\\")),\\n        \\\"src/cleanup_plan/replay/projection_backing/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/projection_backing/tests.rs\\\")),\\n        \\\"src/hir/canonical_loan_storage/caller/stream.rs\\\" => Some(include_str!(\\\"../../src/hir/canonical_loan_storage/caller/stream.rs\\\")),\\n        \\\"src/hir/canonical_loan_storage/caller/stream/tests.rs\\\" => Some(include_str!(\\\"../../src/hir/canonical_loan_storage/caller/stream/tests.rs\\\")),\\n        \\\"src/hir/validation/lexical_exit/lexical_join.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/lexical_exit/lexical_join.rs\\\")),\\n        \\\"src/loan_plan/canonical_sink.rs\\\" => Some(include_str!(\\\"../../src/loan_plan/canonical_sink.rs\\\")),\\n        \\\"src/loan_plan/paid_materialize/immediate_before.rs\\\" => Some(include_str!(\\\"../../src/loan_plan/paid_materialize/immediate_before.rs\\\")),\\n        \\\"src/loan_plan/paid_materialize/owned_inputs_tests.rs\\\" => Some(include_str!(\\\"../../src/loan_plan/paid_materialize/owned_inputs_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/call_carriers/closed_roots.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/call_carriers/closed_roots.rs\\\")),\\n        \\\"src/source_reflows_merge/main_e177_overlay_digests_v13/typed_refs_v18.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/main_e177_overlay_digests_v13/typed_refs_v18.rs\\\")),\\n        \\\"src/hir/validation/scope_checkpoint/merge/moved_direct.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/scope_checkpoint/merge/moved_direct.rs\\\")),\\n        \\\"src/hir/validation/scope_checkpoint/merge/moved_direct/before.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/scope_checkpoint/merge/moved_direct/before.rs\\\")),\\n        \\\"src/hir/validation/scope_checkpoint/merge/moved_direct/tests.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/scope_checkpoint/merge/moved_direct/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(admission1011_arms).count(), 1, \"exact eighteen compiled logical additions\"); let admission1011_before = lookup.replacen(admission1011_arms, \"\", 1); assert_eq!(admission1011_before.len(), 88007); assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(admission1011_before.as_bytes()))), \"767833956590d53f846bbe706da3c9a79cfdf040f2bdb2a3deb64397fb41f287\"); let admission1011_archive = super::super::held_4a6eb(&std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"e/767833956590d53f846bbe706da3c9a79cfdf040f2bdb2a3deb64397fb41f287\"), 88007, \"767833956590d53f846bbe706da3c9a79cfdf040f2bdb2a3deb64397fb41f287\").unwrap(); assert_eq!(admission1011_before, admission1011_archive); let lookup = admission1011_before.as_str(); let observer_arm = \"        \\\"src/workspace_graph/forecast_observation.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/forecast_observation.rs\\\")),\\n\"; assert_eq!(lookup.matches(observer_arm).count(), 1, \"exact newly audited forecast observer lookup arm\"); let before_observer = lookup.replacen(observer_arm, \"\", 1); let lookup = before_observer.as_str(); let hostpin_arm = \"        \\\"tests/workspace/semantic_graph/host_pins.rs\\\" => Some(include_str!(\\\"../../tests/workspace/semantic_graph/host_pins.rs\\\")),\\n\"; assert_eq!(lookup.matches(hostpin_arm).count(), 1, \"exact incoming host-pin lookup arm\"); let before_hostpins = lookup.replacen(hostpin_arm, \"\", 1); let lookup = before_hostpins.as_str(); let main8_903_arms = \"        \\\"src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/retained_scratch.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/retained_scratch.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/uncached_peak_before.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/uncached_peak_before.rs\\\")),\\n        \\\"src/hir.rs\\\" => Some(include_str!(\\\"../../src/hir.rs\\\")),\\n        \\\"src/hir/function_directory.rs\\\" => Some(include_str!(\\\"../../src/hir/function_directory.rs\\\")),\\n        \\\"src/loan_plan/owned_capacity.rs\\\" => Some(include_str!(\\\"../../src/loan_plan/owned_capacity.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/held_projection_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/held_projection_original.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/prelude_forecast_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/prelude_forecast_original.rs\\\")),\\n        \\\"src/workspace_graph/type_proof_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/type_proof_tests.rs\\\")),\\n        \\\"src/workspace_graph/tests/builder_limits.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/tests/builder_limits.rs\\\")),\\n        \\\"src/bounded_output/stage_debits.rs\\\" => Some(include_str!(\\\"../../src/bounded_output/stage_debits.rs\\\")),\\n        \\\"src/cleanup_plan/renewal_profile/child_refusal.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/renewal_profile/child_refusal.rs\\\")),\\n        \\\"src/cleanup_plan/replay/branch_clone/before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/branch_clone/before.rs\\\")),\\n        \\\"src/cleanup_plan/replay/compact_stage.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/compact_stage.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/oracle_order/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/oracle_order/tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_array/funded_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_array/funded_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_array/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_array/tests.rs\\\")),\\n        \\\"src/source_reflows_merge/maina3_custody.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/maina3_custody.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/compact_nodes/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/compact_nodes/tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/selected_context/child.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/selected_context/child.rs\\\")),\\n        \\\"src/workspace_graph/diagnostics.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/diagnostics.rs\\\")),\\n        \\\"src/workspace_graph/diagnostics/borrowed_help_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/diagnostics/borrowed_help_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(main8_903_arms).count(), 1, \"exact main8 and constructor-retention lookup suffix\"); let prior_main8_903_lookup = lookup.replacen(main8_903_arms, \"\", 1); assert_eq!((prior_main8_903_lookup.len(), digest(&prior_main8_903_lookup)), (81547, \"a5fd7203e231da2cf53ebb4dab2c3b8cdbaf32fced49e07c7eaec9da6a2836b7\".to_owned()), \"complete joint admission input before independent old lookup phase\"); let mut prior_main8_903_lookup = prior_main8_903_lookup; for joint_admission in [\"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/neutral.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/neutral.rs\\\")),\\n        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/neutral/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/neutral/tests.rs\\\")),\\n        \\\"src/hir/validation/lexical_exit/arm_context_tests.rs\\\" => Some(include_str!(\\\"../../src/hir/validation/lexical_exit/arm_context_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/before_local_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/before_local_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/before_local_uncached.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/before_local_uncached.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/constructor_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/constructor_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/arena/prefix/compact.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/prefix/compact.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/arena/prefix/compact_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/prefix/compact_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/compact.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/compact.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/compact_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/compact_tests.rs\\\")),\\n\", \"        \\\"src/cleanup_plan/replay/expression_facts/arena/invocation.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/invocation.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/arena/invocation_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/invocation_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_invocation.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_invocation.rs\\\")),\\n        \\\"src/cleanup_plan/replay/fact_preflight.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/fact_preflight.rs\\\")),\\n        \\\"src/source_front_pool.rs\\\" => Some(include_str!(\\\"../../src/source_front_pool.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/call_sites.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/call_sites.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs\\\")),\\n\"] { assert_eq!(prior_main8_903_lookup.matches(joint_admission).count(), 1, \"exact two registered joint admission blocks\"); prior_main8_903_lookup = prior_main8_903_lookup.replacen(joint_admission, \"\", 1); } assert_eq!(prior_main8_903_lookup.len(), 78389); assert_eq!(digest(&prior_main8_903_lookup), \"fc334e8e33719e8c8725de541564f7a03e9d5925cd613f3953f2505a01dffeb4\"); let closure926_930_arms = \"        \\\"src/cleanup_plan/replay/continuation_storage/oracle_reached.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/continuation_storage/oracle_reached.rs\\\")),\\n        \\\"src/cleanup_plan/replay/continuation_storage/oracle_reached_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/continuation_storage/oracle_reached_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/oracle_order.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/oracle_order.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/oracle_order/before_flat_identity.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/oracle_order/before_flat_identity.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/oracle_order/flat_identity_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/oracle_order/flat_identity_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/oracle_closure_before/custody.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/oracle_closure_before/custody.rs\\\")),\\n        \\\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_countdown_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_countdown_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads/before_countdown.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/certified_heads/before_countdown.rs\\\")),\\n        \\\"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_branch_frames.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_branch_frames.rs\\\")),\\n        \\\"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/branch_frames.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/branch_frames.rs\\\")),\\n        \\\"src/source_reflows/ci_inputs.rs\\\" => Some(include_str!(\\\"../../src/source_reflows/ci_inputs.rs\\\")),\\n        \\\"src/workflow_profile/plan_comparison.rs\\\" => Some(include_str!(\\\"../../src/workflow_profile/plan_comparison.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/identity_prebound_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/identity_prebound_tests.rs\\\")),\\n\"; assert_eq!(prior_main8_903_lookup.matches(closure926_930_arms).count(), 1, \"exact thirteen closure926-930 lookup arms\"); let prior_main8_903_lookup = prior_main8_903_lookup.replacen(closure926_930_arms, \"\", 1); assert_eq!(prior_main8_903_lookup.len(), 76063); assert_eq!(digest(&prior_main8_903_lookup), \"c621ebfa81b46f511fda7446d73f2b6a3dc5a6c73d5b3a08f426cf01c1e437cb\"); let lookup = prior_main8_903_lookup.as_str(); let next_leaf_arm_0 = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_0).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_0 = lookup.replacen(next_leaf_arm_0, \"\", 1); let lookup = prior_leaf_lookup_0.as_str(); let next_leaf_arm_1 = \"        \\\"tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\" => Some(include_str!(\\\"../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_1).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_1 = lookup.replacen(next_leaf_arm_1, \"\", 1); let lookup = prior_leaf_lookup_1.as_str(); let main3d81_arms = \"        \\\"src/cleanup_plan/replay/transition.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/transition.rs\\\")),\\n        \\\"src/workspace_graph/initial_report/build.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/initial_report/build.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, \"exact incoming3d81 lookup arms and complete oracle\"); let main3d81_lookup = lookup.replacen(main3d81_arms, \"\", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = \"        \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(field_local_arms).count(), 1, \"exact four field-local lookup arms\"); let field_prior_lookup = lookup.replacen(field_local_arms, \"\", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\")),\\n\"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, \"exact new retained-fork lookup arm\"); let prior_lookup = lookup.replacen(retained_leaf_arm, \"\", 1); let lookup = prior_lookup.as_str();\n", "        assert_eq!(lookup, LOOKUP); let hostpin_arm = \"        \\\"tests/workspace/semantic_graph/host_pins.rs\\\" => Some(include_str!(\\\"../../tests/workspace/semantic_graph/host_pins.rs\\\")),\\n\"; assert_eq!(lookup.matches(hostpin_arm).count(), 1, \"exact incoming host-pin lookup arm\"); let before_hostpins = lookup.replacen(hostpin_arm, \"\", 1); let lookup = before_hostpins.as_str(); let main8_903_arms = \"        \\\"src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/retained_scratch.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/retained_scratch.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/uncached_peak_before.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/uncached_peak_before.rs\\\")),\\n        \\\"src/hir.rs\\\" => Some(include_str!(\\\"../../src/hir.rs\\\")),\\n        \\\"src/hir/function_directory.rs\\\" => Some(include_str!(\\\"../../src/hir/function_directory.rs\\\")),\\n        \\\"src/loan_plan/owned_capacity.rs\\\" => Some(include_str!(\\\"../../src/loan_plan/owned_capacity.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/held_projection_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/held_projection_original.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/prelude_forecast_original.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/prelude_forecast_original.rs\\\")),\\n        \\\"src/workspace_graph/type_proof_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/type_proof_tests.rs\\\")),\\n        \\\"src/workspace_graph/tests/builder_limits.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/tests/builder_limits.rs\\\")),\\n        \\\"src/bounded_output/stage_debits.rs\\\" => Some(include_str!(\\\"../../src/bounded_output/stage_debits.rs\\\")),\\n        \\\"src/cleanup_plan/renewal_profile/child_refusal.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/renewal_profile/child_refusal.rs\\\")),\\n        \\\"src/cleanup_plan/replay/branch_clone/before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/branch_clone/before.rs\\\")),\\n        \\\"src/cleanup_plan/replay/compact_stage.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/compact_stage.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/unit_weight_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/oracle_order/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/oracle_order/tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_array/funded_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_array/funded_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_array/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_array/tests.rs\\\")),\\n        \\\"src/source_reflows_merge/maina3_custody.rs\\\" => Some(include_str!(\\\"../../src/source_reflows_merge/maina3_custody.rs\\\")),\\n        \\\"src/cleanup_plan/replay/factored/compact_nodes/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/factored/compact_nodes/tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/selected_context/child.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/selected_context/child.rs\\\")),\\n        \\\"src/workspace_graph/diagnostics.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/diagnostics.rs\\\")),\\n        \\\"src/workspace_graph/diagnostics/borrowed_help_tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/diagnostics/borrowed_help_tests.rs\\\")),\\n        \\\"src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(main8_903_arms).count(), 1, \"exact main8 and constructor-retention lookup suffix\"); let prior_main8_903_lookup = lookup.replacen(main8_903_arms, \"\", 1); assert_eq!(prior_main8_903_lookup.len(), 76063); assert_eq!(digest(&prior_main8_903_lookup), \"c621ebfa81b46f511fda7446d73f2b6a3dc5a6c73d5b3a08f426cf01c1e437cb\"); let lookup = prior_main8_903_lookup.as_str(); let next_leaf_arm_0 = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_0).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_0 = lookup.replacen(next_leaf_arm_0, \"\", 1); let lookup = prior_leaf_lookup_0.as_str(); let next_leaf_arm_1 = \"        \\\"tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\" => Some(include_str!(\\\"../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\\")),\\n\"; assert_eq!(lookup.matches(next_leaf_arm_1).count(), 1, \"exact newly registered leaf lookup arm\"); let prior_leaf_lookup_1 = lookup.replacen(next_leaf_arm_1, \"\", 1); let lookup = prior_leaf_lookup_1.as_str(); let main3d81_arms = \"        \\\"src/cleanup_plan/replay/transition.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/transition.rs\\\")),\\n        \\\"src/workspace_graph/initial_report/build.rs\\\" => Some(include_str!(\\\"../../src/workspace_graph/initial_report/build.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main_single_groups/receiver_tests.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/main3d81_before.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/main3d81_before.rs\\\")),\\n\"; assert_eq!(lookup.matches(main3d81_arms).count(), 1, \"exact incoming3d81 lookup arms and complete oracle\"); let main3d81_lookup = lookup.replacen(main3d81_arms, \"\", 1); let lookup = main3d81_lookup.as_str(); let field_local_arms = \"        \\\"src/cleanup_plan/replay/path_join/field_local/mod.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\\")),\\n        \\\"src/cleanup_plan/replay/path_join/field_local/tests.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\\")),\\n\"; assert_eq!(lookup.matches(field_local_arms).count(), 1, \"exact four field-local lookup arms\"); let field_prior_lookup = lookup.replacen(field_local_arms, \"\", 1); let lookup = field_prior_lookup.as_str(); let retained_leaf_arm = \"        \\\"src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\" => Some(include_str!(\\\"../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\\")),\\n\"; assert_eq!(lookup.matches(retained_leaf_arm).count(), 1, \"exact new retained-fork lookup arm\"); let prior_lookup = lookup.replacen(retained_leaf_arm, \"\", 1); let lookup = prior_lookup.as_str();\n"),
        (163054, 163336, "        assert_eq!(super::main48_historical_reader::read(&joint_root.join(\"src/source_reflows_merge/main_e177_overlay_lookup.rs\"), 95495, \"bdbfde2197219ad237e2137853b8a54da87e20092e334c7831114a6841cbb934\", 2_097_152).unwrap(), joint_actual_lookup); restored.replacen(BRIDGE, \"\", 1)\n", "        restored.replacen(BRIDGE, \"\", 1)\n"),
        (174705, 177728, "\n    #[rustfmt::skip]\n    fn incoming48_wire_before(root: &str) -> String {\n        const INCOMING: &str = include_str!(\"../../e/066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f\");\n        const TARGET: &str = include_str!(\"../../e/b66377d59526b82e1020058f9b8514419902bbeaf3bfa019a721b8ae087b6f3f\");\n        let path = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"e/066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f\");\n        let physical = super::main48_historical_reader::read(&path, 73910, \"066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f\", 2_097_152).expect(\"whole incoming48 root archive\");\n        assert_eq!(root, INCOMING); assert_eq!(physical, INCOMING); assert_eq!(root.len(), 73910); assert_eq!(digest(root), \"066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f\");\n        // Undo only the complete new wire dispatch and its test view; all old inverses follow.\n        let mut v11_root = root.to_owned();\n        let helper = \"// Closed policy membership only; current source/archive bytes are never retained.\\npub(super) fn declares_source(source: &str) -> bool {\\n    REPLACEMENTS.contains(&source) || ADDITIONS.contains(&source)\\n}\\n\\n\";\n        assert_eq!(v11_root.matches(helper).count(), 1);\n        v11_root = v11_root.replacen(helper, \"\", 1);\n        let added = \"    if page[\\\"schema\\\"] == fragments_v3::SCHEMA_V11 {\\n        let expanded = fragments_v3::expand_v11(&before, policy, page)?;\\n        return compose_values_v2(before, policy, &expanded);\\n    }\\n\";\n        assert_eq!(v11_root.matches(added).count(), 1);\n        v11_root = v11_root.replacen(added, \"\", 1);\n        let added = \"        // Decode only the new wire envelope before the unchanged full predecessor checks.\\n        let page = if page[\\\"schema\\\"] == fragments_v3::SCHEMA_V11 {\\n            fragments_v3::expand_v11(&before, &policy, &page).unwrap()\\n        } else {\\n            (*page).clone()\\n        };\\n\";\n        assert_eq!(v11_root.matches(added).count(), 1);\n        v11_root = v11_root.replacen(added, \"\", 1);\n        let root = v11_root.as_str();\n        assert_eq!(root, TARGET, \"ENTIRE named before-main48-wire stage\"); assert_eq!(root.len(), 73227); assert_eq!(digest(root), \"b66377d59526b82e1020058f9b8514419902bbeaf3bfa019a721b8ae087b6f3f\");\n        assert_eq!(super::main48_historical_reader::read(&path, 73910, \"066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f\", 2_097_152).unwrap(), INCOMING, \"late incoming archive reread\");\n        v11_root\n    }\n    #[rustfmt::skip] #[test]\n    fn incoming48_wire_inverse_preserves_the_entire_authenticated_named_stage() {\n        let incoming = include_str!(\"../../e/066e37a105113138b4fa62a0154ff94f54078da735e05fd924b3596954d8107f\"); assert_eq!(incoming48_wire_before(incoming), include_str!(\"../../e/b66377d59526b82e1020058f9b8514419902bbeaf3bfa019a721b8ae087b6f3f\"));\n        assert!(std::panic::catch_unwind(|| incoming48_wire_before(&format!(\"{incoming} \"))).is_err());\n    }\n", ""),
    ];
    for &(start, end, after, before) in edits.iter().rev() {
        assert!(start <= end && end <= prior.len());
        assert!(prior.is_char_boundary(start) && prior.is_char_boundary(end));
        assert_eq!(&prior[start..end], after);
        prior.replace_range(start..end, before);
    }
    assert_eq!((prior.len(), super::hash(prior.as_bytes())), (154097, "777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3".to_owned()));
    assert_eq!(prior, BEFORE, "entire independently retained historical compiled input");
    assert_eq!(physical_reader::read(&root.join("e/777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3"), 154097, "777709dffedae86cb1ee7750967b21640d5be4296c92474f99861c34615287e3", 9_753_094).unwrap(), BEFORE);
    assert_eq!(physical_reader::read(&root.join(SOURCE), CURRENT.len(), SHA, 9_753_094).unwrap(), physical, "late whole current compiled source");
    Some(prior)
}

// Closed current ingress; old V16 and V14 packets stay byte-exact below it.
fn v20_previous_archive(index:usize)->&'static str { match index {
    0 => "e/db75ca979432426a1c536505a3f1276cc983db7d37070be39bd3a6367471acd7",
    1 => "e/cb545f91a3c5354918f1695a3225b6d1ee00fd9f28bfb5ed54930b27bf38179e",
    2 => "e/f03a36e0ca253e8d181491526aae3e29d57093b3f5225a31b1ebe65aab8d0bb6",
    3 => "e/f96fdfc3f5fd5a2a6afac6b29170ba79f6100feb77459b415390f67c2841f339",
    _=>unreachable!(),
}}
fn v20_previous_compiled(index:usize)->&'static str { match index {
    0 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/db75ca979432426a1c536505a3f1276cc983db7d37070be39bd3a6367471acd7")),
    1 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/cb545f91a3c5354918f1695a3225b6d1ee00fd9f28bfb5ed54930b27bf38179e")),
    2 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/f03a36e0ca253e8d181491526aae3e29d57093b3f5225a31b1ebe65aab8d0bb6")),
    3 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/f96fdfc3f5fd5a2a6afac6b29170ba79f6100feb77459b415390f67c2841f339")),
    _=>unreachable!(),
}}
fn v20_current_compiled(index:usize)->&'static str { match index {
    0 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py")),
    1 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_reflows_merge/main_e177_overlay_v2.rs")),
    2 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py")),
    3 => include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_reflows_merge/main_e177_overlay_fragments_v3.rs")),
    _=>unreachable!(),
}}
fn v20_ingress_before_using(index:usize,supplied:&str,record:&Value,read:&impl Fn(&std::path::Path,usize,&str)->String)->String {
    v20_ingress_before_format_using(index,supplied,record,"semaprax.v20-shared-source-ingress.v1",read)
}
fn v20_ingress_before_format_using(index:usize,supplied:&str,record:&Value,format:&str,read:&impl Fn(&std::path::Path,usize,&str)->String)->String {
    let compact=match format {"semaprax.v20-shared-source-ingress.v1"=>false,"semaprax.v20-shared-source-ingress.v2"=>true,_=>panic!("closed source ingress format")};
    let keys=|value:&Value,names:&[&str]|value.as_object().is_some_and(|o|o.len()==names.len()&&names.iter().all(|name|o.contains_key(*name)));
    assert!(keys(record,&["source","current","previous","edits"]));assert_eq!(record["source"],V14_FRONT_SOURCES[index]);
    let compiled=v20_current_compiled(index);assert_eq!(compiled,supplied,"whole actual current source before history");
    let prior=v20_previous_compiled(index);let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let length=compiled.len();let sha=super::hash(compiled.as_bytes());let archive=format!("e/{sha}");
    let previous_sha=super::hash(prior.as_bytes());
    for (descriptor,n,h,path) in [(&record["current"],length,sha.as_str(),archive.as_str()),(&record["previous"],prior.len(),previous_sha.as_str(),v20_previous_archive(index))] {
        assert!(keys(descriptor,&["length","sha256"]));assert!(n<=9_753_094);
        assert_eq!(descriptor["length"].as_u64(),Some(n as u64));assert_eq!(descriptor["sha256"],h);assert_eq!(path,format!("e/{h}"));
    }
    let acquire=||{
        assert_eq!(read(&root.join(V14_FRONT_SOURCES[index]),length,&sha),compiled);
        assert_eq!(read(&root.join(&archive),length,&sha),compiled);
        assert_eq!(read(&root.join(v20_previous_archive(index)),prior.len(),&previous_sha),prior);
    };
    acquire();let mut restored=supplied.to_owned();
    for edit in record["edits"].as_array().unwrap().iter().rev(){
        let (start,end,after,before)=if compact {
            let parts=edit.as_array().unwrap();assert_eq!(parts.len(),3,"closed slice ingress edit");
            let start=usize::try_from(parts[0].as_u64().unwrap()).unwrap();let end=usize::try_from(parts[1].as_u64().unwrap()).unwrap();
            assert!(start<=end&&end<=compiled.len()&&compiled.is_char_boundary(start)&&compiled.is_char_boundary(end));
            (start,end,&compiled[start..end],parts[2].as_str().unwrap())
        }else{
            let (start,end,after,before)=if let Some(parts)=edit.as_array(){
                assert_eq!(parts.len(),4,"closed compact ingress edit");(&parts[0],&parts[1],&parts[2],&parts[3])
            }else{assert!(keys(edit,&["start","end","after","before"]));(&edit["start"],&edit["end"],&edit["after"],&edit["before"])};
            (usize::try_from(start.as_u64().unwrap()).unwrap(),usize::try_from(end.as_u64().unwrap()).unwrap(),after.as_str().unwrap(),before.as_str().unwrap())
        };
        assert!(start<=end&&end<=restored.len()&&restored.is_char_boundary(start)&&restored.is_char_boundary(end));assert_eq!(&restored[start..end],after);
        assert!(restored.len().checked_sub(end-start).unwrap().checked_add(before.len()).unwrap()<=9_753_094);restored.replace_range(start..end,before);
    }
    assert_eq!(restored,prior,"ENTIRE pre-V20 physical stage");acquire();restored
}
fn v20_ingress_before(index:usize,supplied:&str,record:&Value)->String {
    v20_ingress_before_using(index,supplied,record,&|p,n,h|physical_reader::read(p,n,h,9_753_094).unwrap())
}
fn v14_source_before_using(source:&str,supplied:&str,read:&impl Fn(&std::path::Path,usize,&str)->String)->String {
    let Some(index)=V14_FRONT_SOURCES.iter().position(|s|*s==source)else{return v14_source_before_before_v20(source,supplied);};
    let packet:Value=serde_json::from_str(include_str!("controller_transport.json")).unwrap();let ingress=&packet["v20_shared_ingress"];
    assert!(ingress.as_object().is_some_and(|o|o.len()==4));let format=ingress["schema"].as_str().unwrap();assert!(matches!(format,"semaprax.v20-shared-source-ingress.v1"|"semaprax.v20-shared-source-ingress.v2"));assert_eq!(ingress["source_kind"],"ACTUAL_CURRENT_TO_ENTIRE_PRE_V20_PHYSICAL_STAGE");assert_eq!(ingress["sources"],serde_json::json!(V14_FRONT_SOURCES));
    let records=ingress["records"].as_array().unwrap();assert_eq!(records.len(),V14_FRONT_SOURCES.len());
    assert_eq!(records.iter().map(|r|r["source"].as_str().unwrap()).collect::<Vec<_>>(),V14_FRONT_SOURCES);
    let before=v20_ingress_before_format_using(index,supplied,&records[index],format,read);let result=v14_source_before_before_v20(source,&before);
    assert_eq!(v20_ingress_before_format_using(index,supplied,&records[index],format,read),before);result
}
fn v14_source_before(source:&str,supplied:&str)->String {
    v14_source_before_using(source,supplied,&|p,n,h|physical_reader::read(p,n,h,9_753_094).unwrap())
}
#[cfg(test)]
mod v20_ingress_tests {
    use super::*;
    #[test]
    fn current_three_channels_restore_whole_pre_v20_and_all_old_targets_refuse_drift() {
        let packet:Value=serde_json::from_str(include_str!("controller_transport.json")).unwrap();
        for index in 0..V14_FRONT_SOURCES.len(){
            let current=v20_current_compiled(index);let prior=v20_previous_compiled(index);
            let compact_record=&packet["v20_shared_ingress"]["records"][index];let mut expanded=compact_record.clone();
            if packet["v20_shared_ingress"]["schema"]=="semaprax.v20-shared-source-ingress.v2" {
                for edit in expanded["edits"].as_array_mut().unwrap(){let parts=edit.as_array().unwrap();assert_eq!(parts.len(),3);let start=usize::try_from(parts[0].as_u64().unwrap()).unwrap();let end=usize::try_from(parts[1].as_u64().unwrap()).unwrap();*edit=serde_json::json!([start,end,&current[start..end],parts[2]]);}
            }
            let record=&expanded;
            assert_eq!(v20_ingress_before(index,current,record),prior);
            assert_eq!(v14_source_before(V14_FRONT_SOURCES[index],current),v14_source_before_before_v20(V14_FRONT_SOURCES[index],prior));
            let mut drift=current.to_owned();drift.push(' ');assert!(std::panic::catch_unwind(||v20_ingress_before(index,&drift,record)).is_err());
            for field in ["current","previous"]{let mut bad=record.clone();bad[field]["sha256"]="0".repeat(64).into();assert!(std::panic::catch_unwind(||v20_ingress_before(index,current,&bad)).is_err());}
            let mut bad=record.clone();bad["extra"]=true.into();assert!(std::panic::catch_unwind(||v20_ingress_before(index,current,&bad)).is_err());
            let first=&record["edits"][0];let tuple=if let Some(parts)=first.as_array(){parts.clone()}else{vec![first["start"].clone(),first["end"].clone(),first["after"].clone(),first["before"].clone()]};
            let mut compact=record.clone();compact["edits"][0]=serde_json::json!(tuple);assert_eq!(v20_ingress_before(index,current,&compact),prior);
            let mut object=record.clone();object["edits"][0]=serde_json::json!({"start":tuple[0],"end":tuple[1],"after":tuple[2],"before":tuple[3]});assert_eq!(v20_ingress_before(index,current,&object),prior);
            for edit in [serde_json::json!([0,0,""]),serde_json::json!([]),serde_json::json!([0,0,"","",false]),serde_json::json!([true,0,"",""]),serde_json::json!([-1,0,"",""]),serde_json::json!([0.5,0,"",""]),serde_json::json!([0,0,false,""]),serde_json::json!([0,0,"",false]),serde_json::json!({"start":0,"end":0,"after":"","before":"","extra":true})]{
                let mut bad=record.clone();bad["edits"][0]=edit;assert!(std::panic::catch_unwind(||v20_ingress_before(index,current,&bad)).is_err());
            }
            // The new version derives slices only after all ordinary channels authenticate.
            let mut slice_record=record.clone();for edit in slice_record["edits"].as_array_mut().unwrap(){let p=edit.as_array().unwrap();*edit=serde_json::json!([p[0],p[1],p[3]]);}
            let decode=|r:&Value,supplied:&str|v20_ingress_before_format_using(index,supplied,r,"semaprax.v20-shared-source-ingress.v2",&|p,n,h|physical_reader::read(p,n,h,9_753_094).unwrap());
            assert_eq!(decode(&slice_record,current),prior);
            assert!(std::panic::catch_unwind(||decode(&slice_record,&drift)).is_err());
            for field in ["current","previous"]{let mut bad=slice_record.clone();bad[field]["sha256"]="0".repeat(64).into();assert!(std::panic::catch_unwind(||decode(&bad,current)).is_err());}
            for edit in [serde_json::json!([]),serde_json::json!([0,0,"",""]),serde_json::json!([true,0,""]),serde_json::json!([-1,0,""]),serde_json::json!([0.5,0,""]),serde_json::json!([0,0,false]),serde_json::json!([1,0,""]),serde_json::json!([0,current.len()+1,""]),serde_json::json!({"start":0,"end":0,"before":""})]{let mut bad=slice_record.clone();bad["edits"][0]=edit;assert!(std::panic::catch_unwind(||decode(&bad,current)).is_err());}
            if let Some((offset,ch))=current.char_indices().find(|(_,ch)|ch.len_utf8()>1){let mut bad=slice_record.clone();bad["edits"][0]=serde_json::json!([offset+1,offset+ch.len_utf8(),""]);assert!(std::panic::catch_unwind(||decode(&bad,current)).is_err());}
            let reads=std::cell::Cell::new(0);assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||v20_ingress_before_format_using(index,current,&slice_record,"unknown",&|_,_,_|{reads.set(reads.get()+1);String::new()}))).is_err());assert_eq!(reads.get(),0);
            // Exercise each actual channel through the unchanged ordinary held
            // reader, with corrupt/truncated fixture bytes at early AND late
            // acquisition. The immutable repository images are never edited.
            let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let current_sha=super::super::hash(current.as_bytes());
            let channels=[(root.join(V14_FRONT_SOURCES[index]),current),(root.join(format!("e/{current_sha}")),current),(root.join(v20_previous_archive(index)),prior)];
            for (channel,bytes) in channels {for frontier in 0..4 {for truncated in [false,true] {
                static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
                let sequence=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
                let path=std::env::temp_dir().join(format!("semaprax-v20-source-ingress-{}-{sequence}",std::process::id()));
                let mut bad=bytes.as_bytes().to_vec();if truncated{bad.pop();}else{bad[0]^=1;}
                let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
                use std::io::Write;file.write_all(&bad).unwrap();drop(file);
                struct Remove(std::path::PathBuf);impl Drop for Remove{fn drop(&mut self){std::fs::remove_file(&self.0).unwrap();}}
                let remove=Remove(path);let encounters=std::cell::Cell::new(0usize);
                let read=|p:&std::path::Path,n:usize,h:&str|{
                    let selected=if p==channel.as_path() {let count=encounters.get();encounters.set(count+1);count==frontier}else{false};
                    physical_reader::read(if selected{remove.0.as_path()}else{p},n,h,9_753_094).unwrap()
                };
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||v14_source_before_using(V14_FRONT_SOURCES[index],current,&read))).is_err());
                assert_eq!(encounters.get(),frontier+1,"selected freshness frontier reached");
            }}}

        }
    }
}
