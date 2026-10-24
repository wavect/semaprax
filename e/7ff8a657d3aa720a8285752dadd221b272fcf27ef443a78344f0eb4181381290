//! Whole-source inverse and immutable shared dependency binding for the oracle.
use sha2::{Digest, Sha256};
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
#[test]
fn complete_immediate_old_driver_and_each_helper_restore_their_whole_preimages() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        digest(include_bytes!("../before_frontier/custody/inverse.json")),
        "86a572cc73c1b4981a710e5d7fba725ef9723f62586a30c38fb2bc4b9f0fe510"
    );
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("../before_frontier/custody/inverse.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let mut actual =
            std::fs::read_to_string(root.join(row["compiled"].as_str().unwrap())).unwrap();
        let original = std::fs::read_to_string(root.join(row["before"].as_str().unwrap())).unwrap();
        assert_eq!(digest(original.as_bytes()), row["sha256"].as_str().unwrap());
        for change in row["inverse"].as_array().unwrap().iter().rev() {
            let at = change[0].as_u64().unwrap() as usize;
            let current = change[1].as_str().unwrap();
            let before = change[2].as_str().unwrap();
            assert_eq!(actual.get(at..at + current.len()), Some(current));
            actual.replace_range(at..at + current.len(), before);
        }
        assert_eq!(actual, original, "whole inverse for {}", row["compiled"]);
    }
    assert_eq!(
        include_str!("../before_frontier/whole.rs.txt"),
        include_str!("../before_frontier/custody/mod.rs.txt")
    );
}
#[test]
fn shared_replay_dependencies_and_unchanged_driver_helpers_remain_exactly_source_bound() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        digest(include_bytes!(
            "../before_frontier/custody/shared_readset.json"
        )),
        "a6004b3b67c5af61e98e225ae1716e3aa5dd9c08d01f14afa02d1ba73f62c4f7"
    );
    let rows: Vec<(String, String)> = serde_json::from_str(include_str!(
        "../before_frontier/custody/shared_readset.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 946);
    assert!(rows
        .iter()
        .all(|(path, _)| path != "src/cleanup_plan/replay/path_join.rs"));
    // The changed driver is held in full by whole.rs.txt and its separately
    // compiled inverse above. Its shared helper bodies and selected module
    // declaration remain exactly the immediate-old suffix, while all selected
    // implementation files retain their own unchanged readset bindings.
    let current = include_str!("../../path_join.rs");
    let before = include_str!("../before_frontier/whole.rs.txt");
    let marker = "fn validate_control_group_with_budget";
    assert_eq!(
        current.split_once(marker).unwrap().1,
        before.split_once(marker).unwrap().1
    );
    for (path, expected) in rows {
        let mut actual =
            crate::bounded_output::census_before(&path, &std::fs::read(root.join(&path)).unwrap());
        // Finite inventory transport only: the main scalar classifier changes
        // the measured common lexical prefix. This is not old execution restoration.
        let encoded = include_str!("../before_frontier/custody/main_d227_transport.json");
        assert_eq!(std::fs::read(root.join("src/cleanup_plan/replay/path_join/before_frontier/custody/main_d227_transport.json")).unwrap(), encoded.as_bytes());
        assert_eq!(
            digest(encoded.as_bytes()),
            "bb3054bcefde53fb800ca082cd2a9de658ec40a38b6da5c51c4ad7c43af3090f"
        );
        let packet: serde_json::Value = serde_json::from_str(encoded).unwrap();
        assert_eq!(packet.as_object().unwrap().len(), 42);
        if let Some(row) = packet.get(&path) {
            let compiled = match path.as_str() {
                "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/branch_clone/inline_tests.rs" => include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/branch_clone/inline_tests.rs"
                ))
                .as_slice(),
                "src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs" => {
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs"
                    ))
                    .as_slice()
                }
                "src/cleanup_plan/replay/branch_clone/production_capacity.rs" => {
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/src/cleanup_plan/replay/branch_clone/production_capacity.rs"
                    ))
                    .as_slice()
                }
                "src/cleanup_plan/replay/branch_clone/tests.rs" => include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/branch_clone/tests.rs"
                ))
                .as_slice(),
                "src/cleanup_plan/replay/branch_clone.rs" => include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/branch_clone.rs"
                ))
                .as_slice(),
                "src/cleanup_plan/replay/nested_shape.rs" => include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/nested_shape.rs"
                ))
                .as_slice(),
                "src/cleanup_plan/replay.rs" => include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay.rs"
                ))
                .as_slice(),
                "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs")).as_slice(),
                "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs")).as_slice(),
                "src/cleanup_plan/replay/factored.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs")).as_slice(),
                "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs")).as_slice(),
                "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs")).as_slice(),
                "src/cleanup_plan/replay/factored/continuation_receiver.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/continuation_receiver.rs")).as_slice(),
                "src/cleanup_plan/replay/skeleton_work.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work.rs")).as_slice(),
                "src/cleanup_plan/replay/ordinary_receiving/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_receiving/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs")).as_slice(),
                "src/cleanup_plan/replay/factored/continuation_receiver/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/continuation_receiver/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs")).as_slice(),
                "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs")).as_slice(),
                "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/arena/prefix/source_main8a.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix/source_main8a.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/arena/prefix/source_maina3.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix/source_maina3.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/capacity.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/capacity.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exits/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/return_operands/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/tests/arena.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/tests/arena.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_proof/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/expression_facts/receiving/tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/tests.rs")).as_slice(),
                "src/cleanup_plan/replay/path_join/selected/tests/mod.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/mod.rs")).as_slice(),
                "src/cleanup_plan/replay/path_join/selected/tests/renewal.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/renewal.rs")).as_slice(),
                "src/cleanup_plan/replay/path_join/selected/tests/session.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/session.rs")).as_slice(),
                "src/cleanup_plan/replay/path_join/selected/tests/strings.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/strings.rs")).as_slice(),
                "src/cleanup_plan/replay/path_join/selected/tests/whole.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/whole.rs")).as_slice(),
                "src/cleanup_plan/replay/physical_budget/authentication.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_budget/authentication.rs")).as_slice(),
                _ => panic!("unadmitted main transport path"),
            };
            assert_eq!(actual.as_slice(), compiled);
            assert_eq!(actual.len(), row["length"].as_u64().unwrap() as usize);
            assert_eq!(digest(&actual), row["sha256"].as_str().unwrap());
            for edit in row["edits"].as_array().unwrap().iter().rev() {
                let start = edit["start"].as_u64().unwrap() as usize;
                let end = edit["end"].as_u64().unwrap() as usize;
                assert_eq!(
                    &actual[start..end],
                    edit["current"].as_str().unwrap().as_bytes()
                );
                actual.splice(start..end, edit["before"].as_str().unwrap().bytes());
            }
            assert_eq!(
                actual.len(),
                row["before_length"].as_u64().unwrap() as usize
            );
            assert_eq!(digest(&actual), row["before_sha256"].as_str().unwrap());
            if path == "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs" {
                // This one endpoint is the complete643f intermediate. The
                // original ordinary-labels stage below still proves fixed946.
                assert_eq!(actual.as_slice(), row["before_text"].as_str().unwrap().as_bytes());
                assert_eq!(actual.len(), 55813);
                assert_eq!(digest(&actual), "87257e50e24a64d047eff17fd2ef9de7e1ed4c1049697222223947c62b28f514");
            } else {
                assert_eq!(digest(&actual), expected);
            }
        }
        if matches!(
            path.as_str(),
            "src/cleanup_plan/replay/selected_owner.rs"
                | "src/cleanup_plan/replay/selected_owner/paid.rs"
        ) {
            // Both complete drivers share this external authentication prefix.
            // Its new funding wrapper executes the exact bound original owner
            // search; the fixed old source targets below are not repinned.
            actual = crate::cleanup_plan::replay::selected_owner::ordinary::custody(&path, &actual);
        }
        let actual =
            if path == "src/cleanup_plan/replay/factored/strategy/compact_receiving/tests.rs" {
                // Finite test-only transport: bind the complete current test bytes,
                // then invert the whole replacement to the complete pinned old
                // fixture. No production dependency receives this inverse.
                assert_eq!(
                    digest(&actual),
                    "130785078147912db8f850bd5f5ff7ea09c847ce31a8706ddf566662542147ee"
                );
                let before = include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/e/a7e37ebe47e810939282e3e6abfb44db27b658418d91154131679d0759847efe"
                ));
                assert_eq!(before.len(), 13592);
                assert_eq!(digest(before), expected);
                before.as_slice()
            } else if path == "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs" {
                // Exact whole-source transport for OPT892; the complete
                // immediate-old dependency remains the fixed 946-row target.
                assert_eq!(actual.len(), 10932);
                assert_eq!(digest(&actual), "792be2624414510de96336103ccc964f39ff7c474581aa90521be539b8e21d2d");
                let before = include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/e/4392abffc59dee7596e750e1523f0e2b0f4c23a5f35c67caa475de5785c4f568"
                ));
                assert_eq!(before.len(), 7942);
                assert_eq!(digest(before), expected);
                before.as_slice()
            } else if path == "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs" {
                // Exact whole-source transport for OPT892; the complete
                // immediate-old dependency remains the fixed 946-row target.
                assert_eq!(actual.len(), 24843);
                assert_eq!(digest(&actual), "d4757425643d96a03c1c6a05f31d7018cb5653c71a921bb736335e9102edf6f6");
                let before = include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/e/2890943d20f5ac052889f50340039fb6c4467ff5d686334796438a5b242bdf55"
                ));
                assert_eq!(before.len(), 15818);
                assert_eq!(digest(before), expected);
                before.as_slice()
            } else if path == "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" {
                // Bind every current physical byte before the immutable 946 target.
                assert_eq!(actual.as_slice(), include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")).as_slice());
                assert_eq!(actual.len(), 37215);
                assert_eq!(digest(&actual), "24a7b0034088ef2db45d969ef4320a931e5f724b14732dfdf56073a818cd0081");
                let before = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/912a91d979144205750367d55e7aa9b74a4b3f4a1afc8472c010cf26af2677e8"));
                assert_eq!(before.len(), 37088);
                assert_eq!(digest(before), expected);
                before.as_slice()
            } else if path == "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs" {
                // Bind every current physical byte before the immutable 946 target.
                assert_eq!(actual.as_slice(), include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs")).as_slice());
                assert_eq!(actual.len(), 86537);
                assert_eq!(digest(&actual), "08c7336a5435b02aed14820e57b7ec6c48ddc9b311363d09c05b3447c0ad6246");
                let before = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/fa232cdadb3ff7ea321caa04554d8dc2c237928b976af5890f6ed555c276b1ec"));
                assert_eq!(before.len(), 86698);
                assert_eq!(digest(before), expected);
                before.as_slice()
            } else if path == "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs" {
                // Bind every current physical byte before the immutable 946 target.
                assert_eq!(actual.as_slice(), packet[&path]["before_text"].as_str().unwrap().as_bytes());
                assert_eq!(actual.len(), 55813);
                assert_eq!(digest(&actual), "87257e50e24a64d047eff17fd2ef9de7e1ed4c1049697222223947c62b28f514");
                let before = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/b6471558eaf00c76a99472843a9d96a58a11fe9e5779f50fdccbfc6f0011aa51"));
                assert_eq!(before.len(), 55814);
                assert_eq!(digest(before), expected);
                before.as_slice()
            } else {
                actual.as_slice()
            };
        assert_eq!(
            digest(actual),
            expected,
            "immediate-old shared dependency {path}"
        );
    }
    assert!(
        std::mem::size_of::<super::Incoming<super::PathState>>()
            >= std::mem::size_of::<std::collections::BTreeSet<Box<super::PathState>>>()
    );
}
