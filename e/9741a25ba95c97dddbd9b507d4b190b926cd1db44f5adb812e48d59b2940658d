//! Whole-source inverse and immutable shared dependency binding for the oracle.
use sha2::{Digest, Sha256};
#[path = "../../oracle_closure_before/custody.rs"]
mod oracle_closure_custody;
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
    let current = crate::bounded_output::census_before(
        "src/cleanup_plan/replay/path_join.rs",
        include_bytes!("../../path_join.rs"),
    );
    let current = std::str::from_utf8(&current).unwrap();
    let before = include_str!("../before_frontier/whole.rs.txt");
    let marker = "fn validate_control_group_with_budget";
    assert_eq!(
        current.split_once(marker).unwrap().1,
        before.split_once(marker).unwrap().1
    );
    for (path, expected) in rows {
        // Bound the changed continuation route against its compiled current
        // image before the complete inherited inverse/dependency checks.
        let mut actual = if path == "src/cleanup_plan/replay/continuation_storage.rs" {
            let physical = oracle_closure_custody::exact_compiled(&root.join(&path), include_str!("../../continuation_storage.rs"));
            crate::bounded_output::census_before(&path, physical.as_bytes())
        } else {
            crate::bounded_output::census_before(&path, &std::fs::read(root.join(&path)).unwrap())
        };
        // Finite inventory transport only: the main scalar classifier changes
        // the measured common lexical prefix. This is not old execution restoration.
        let encoded = include_str!("../before_frontier/custody/main_d227_transport.json");
        assert_eq!(oracle_closure_custody::exact_compiled(&root.join("src/cleanup_plan/replay/path_join/before_frontier/custody/main_d227_transport.json"), encoded).as_bytes(), encoded.as_bytes());
        assert_eq!(
            digest(encoded.as_bytes()),
            "a45ca685dd5d58087205b82584411860c1cc7d1ba2beb1ff2d64154043338eeb"
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
            actual = current_shared_input_before(&path, &actual);
            actual = retained_fork_source_before(&path, &actual);
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
                assert_eq!(
                    actual.as_slice(),
                    row["before_text"].as_str().unwrap().as_bytes()
                );
                assert_eq!(actual.len(), 55813);
                assert_eq!(
                    digest(&actual),
                    "87257e50e24a64d047eff17fd2ef9de7e1ed4c1049697222223947c62b28f514"
                );
            } else {
                assert_eq!(digest(&actual), expected);
            }
        }
        if packet.get(&path).is_none() {
            actual = current_shared_input_before(&path, &actual);
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
        let continuation_before = if matches!(path.as_str(), "src/cleanup_plan/replay/continuation_storage/tests.rs" | "src/cleanup_plan/replay/continuation_storage.rs")
        {
            let compiled = if path == "src/cleanup_plan/replay/continuation_storage.rs" { include_bytes!("../../continuation_storage.rs").as_slice() } else { include_bytes!("../../continuation_storage/tests.rs").as_slice() };
            assert_eq!(actual.as_slice(), compiled);
            if path == "src/cleanup_plan/replay/continuation_storage.rs" {
                actual = oracle_closure_custody::restore(&path, std::str::from_utf8(&actual).unwrap()).into_bytes();
            }
            let packet = include_bytes!("../../continuation_storage/reached_before/inverse.json");
            assert_eq!(
                digest(packet),
                "502ce5028fe9c180ff71c8e7f19ad6724f5a729244d828dadcc8ccdf1e59d129"
            );
            let rows: serde_json::Value = serde_json::from_slice(packet).unwrap();
            let row = rows
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["path"] == path)
                .unwrap();
            assert_eq!(actual.len() as u64, row["current_length"]);
            assert_eq!(digest(&actual), row["current_sha256"].as_str().unwrap());
            let mut before = actual.clone();
            for e in row["edits"].as_array().unwrap().iter().rev() {
                let start = e["start"].as_u64().unwrap() as usize;
                let end = e["end"].as_u64().unwrap() as usize;
                assert_eq!(
                    &before[start..end],
                    e["current"].as_str().unwrap().as_bytes()
                );
                before.splice(start..end, e["before"].as_str().unwrap().bytes());
            }
            let (fixture, length, sha): (&[u8], usize, &str) = if path == "src/cleanup_plan/replay/continuation_storage.rs" {
                (include_bytes!("../../continuation_storage/reached_before/continuation_storage.rs.txt"), 14296, "50e0fcee065b5222376f153597daa16ffc1044875e286c0bfc92ecf50e88a25f")
            } else { (include_bytes!("../../continuation_storage/reached_before/tests.rs.txt"), 27342, "ef071c7def7542ab0365cdd9566779a4b37270365bf6c147df7e87122debbdc2") };
            assert_eq!(before.as_slice(), fixture);
            assert_eq!(before.len(), length);
            assert_eq!(digest(&before), sha);
            Some(before)
        } else {
            None
        };
        let actual = if let Some(before)=continuation_before.as_ref() {before.as_slice()} else
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
                actual = retained_fork_source_before(&path, &actual);
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
            } else if path == "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs" {
                assert_eq!(actual.as_slice(), include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs" )).as_slice());
                assert_eq!(actual.len(), 19806);
                assert_eq!(digest(&actual), "6745c50ef06c4bab501efe623044c0044977a946df0aff8c95873a0c92de0fd1");
                let before = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/1d31e1bf8377e7a1d7dd1d5a8844faa974cefa6cf94bd3029283647f70d2a51c"));
                assert_eq!(before.len(), 15284);
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

// Acquire only these five complete physical epochs before the unchanged
// fixed946/main42 source contracts. No inventory target or old edit is repinned.
fn retained_fork_source_before(path: &str, current: &[u8]) -> Vec<u8> {
    if path == "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs" {
        let compiled = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs")).as_slice();
        let physical = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_eq!(physical.as_slice(), compiled);
        assert_eq!(current, compiled);
        assert_eq!(current.len(), 31154);
        assert_eq!(digest(current), "5e0da04b5e6373c30f7b31799f36a484477ccc0f9d79c98af4f91cd6a4d0e335");
        // Entire already-retained main42 input, before its unchanged old edits.
        let before = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/fc7a3fe35db9670affa2d479fea6e0a9eadf50afed410e729c7d956124d1c7f5"));
        assert_eq!(before.len(), 24273);
        assert_eq!(digest(before), "fc7a3fe35db9670affa2d479fea6e0a9eadf50afed410e729c7d956124d1c7f5");
        assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap(), physical);
        return before.to_vec();
    }
    let (compiled, length, expected, before_length, before_digest, edits):
        (&[u8], usize, &str, usize, &str, &[(usize, &str, &str)]) = match path {
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs" => (include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs")).as_slice(), 23389, "f2539cb9f7408df422dd81ddf360625e2a54fcc4ca9c330288e81147ffa894eb", 20575, "d7db6c7fc49140f9dabec3e647bf64b50571ee71a8478aae1c03acd92bba6c5f", &[
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs" => (include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs")).as_slice(), 11753, "7944d10f5515da0a4e9c4b6ed49882bb3e20ccf9cfc96fc4c20372370ebf955f", 11246, "1f55c14d5d7fe41614e045fc680d5df8a93d4e29e4698e47bdb7e39ce8203c68", &[
            (637, "    failed: bool,\n", ""),
            (1047, "in crate::clean", "s"),
            (1064, "_plan::", "e"),
            (1072, "eplay", ""),
            (1562, "size_of::<bool>() // initialized invocation-local sticky terminal state\n    + ", ""),
            (2004, "\n    + 3 * size_of::<bool>()", ""),
            (2033, " // sticky read, guard, and first terminal publication", ""),
            (4590, "", "!"),
            (4597, "fai", ""),
            (4603, "", "ger.current()"),
            (4691, "if !", ""),
            (4701, ".ledger.current() {", ""),
            (4733, "$sc", ".w"),
            (4737, "pe.failed = t", ""),
            (4751, "ue;", "k"),
            (4767, "r", ".budg"),
            (4770, "urn Err(Diagnostic::io(\"SPX-H006\", String::new()));", ""),
            (4830, "}\n", ""),
            (4836, "    match $scope.work.budget", ""),
            (4923, " {\n            Ok(()) => {},\n            Err(error) => {\n                $scope.failed = true", "?"),
            (5017, "\n                return Err(error);\n            }\n        }", ""),
            (5092, "in crate::clean", "s"),
            (5109, "_plan::", "e"),
            (5117, "eplay", ""),
            (5447, "e,\n        failed: fals", ""),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs" => (include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs")).as_slice(), 18414, "074d4939f7c6a2c278f482219eb7e66419b3dfe66929602c8ab6cc942c826165", 18010, "066f042491018f63adfa22bb4f0754bdbd9938f6941b5fc32a1c9c13b5f6ba41", &[
            (778, "e,\n        failed: fals", ""),
            (6214, ";\n        // Isolate the validation owner's Rc lifetime from the fixture's\n        // destruction scratch, which otherwise retains its own strong/weak owners.\n        work.release_observation_scratch_for_test()", ""),
            (8110, "        drop(scope);\n        // Classification also lends b to work's private query scratch. End\n        // that owner before testing exclusive access to the fixture's Rc.\n        drop(work);\n        assert!(Rc::get_mut(b.prefix.as_mut().unwrap()).is_some());", "        drop(scope);\n        assert!(Rc::get_mut(b.prefix.as_mut().unwrap()).is_some());"),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs" => (include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs")).as_slice(), 24967, "703b8e31e2c34896382513d8e4cff5d900d8c5990ccbdb96aad3dc363aed3e05", 24843, "d4757425643d96a03c1c6a05f31d7018cb5653c71a921bb736335e9102edf6f6", &[
            (15815, "", "cur"),
            (15817, "", "n"),
            (15818, "ained_epoch", ""),
            (15832, "crate::cleanup_plan::replay::skeleton_work::query_proof::before_emission(", ""),
            (15937, "));\n    let current = retained_epoch.as_str(", ""),
        ]),
        _ => return current.to_vec(),
    };
    let physical = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    assert_eq!(physical.as_slice(), compiled);
    assert_eq!(current, compiled);
    assert_eq!(current.len(), length);
    assert_eq!(digest(current), expected);
    let mut before = if path == "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs" {
        crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::before_emission(std::str::from_utf8(current).unwrap()).into_bytes()
    } else { current.to_vec() };
    for &(at, after, prior) in edits.iter().rev() {
        assert_eq!(before.get(at..at + after.len()), Some(after.as_bytes()));
        before.splice(at..at + after.len(), prior.bytes());
    }
    assert_eq!(before.len(), before_length);
    assert_eq!(digest(&before), before_digest);
    assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap(), physical);
    before
}
// Finite exact successor inputs; original42 packet and all946 targets stay whole.
fn current_shared_input_before(path: &str, current: &[u8]) -> Vec<u8> {
    let lookup_epoch = path == "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs";
    let current = if lookup_epoch {
        let physical = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        let compiled = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs")).as_slice();
        assert_eq!(physical.as_slice(), compiled);
        assert_eq!(current, compiled);
        // Complete fixture-owner funding precedes the unchanged prior lookup chain.
        assert_eq!(current.len(), 45009);
        assert_eq!(digest(current), "810201d22a70f872c298aca9941bf5347798889fc91284bc4cc3f0e7402debbd");
        let current = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/29e398dbdcd6a81735156bdf0d72a437d40e64a5786bb79a3add1924e626342a")).as_slice();
        // Bind the complete repaired physical lookup before acquiring the
        // entire authenticated mainb7 input for the unchanged older fronts.
        assert_eq!(current.len(), 44359);
        assert_eq!(digest(current), "29e398dbdcd6a81735156bdf0d72a437d40e64a5786bb79a3add1924e626342a");
        let current = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/8fb97de082a60f774add25568ba491fac867b7a82880ef8e601a31f068585394")).as_slice();
        assert_eq!(current.len(), 41729);
        assert_eq!(digest(current), "8fb97de082a60f774add25568ba491fac867b7a82880ef8e601a31f068585394");
        let prior = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/48ba251872ba4bf83424e95f78cbadcd11636432a03da7ec8e700e78425a353d")).as_slice();
        assert_eq!(prior.len(), 39177);
        assert_eq!(digest(prior), "48ba251872ba4bf83424e95f78cbadcd11636432a03da7ec8e700e78425a353d");
        assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap(), physical);
        prior
    } else { current };
    let (compiled, length, sha, before, before_length, before_sha):
        (&[u8], usize, &str, &[u8], usize, &str) = match path {
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs")).as_slice(),
            39177,
            "48ba251872ba4bf83424e95f78cbadcd11636432a03da7ec8e700e78425a353d",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/c2b55aa5990962e7cd33994ec25ff30217f7067770c92458f4d6fce8c20ecb6a")).as_slice(),
            20898,
            "c2b55aa5990962e7cd33994ec25ff30217f7067770c92458f4d6fce8c20ecb6a",
        ),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs")).as_slice(),
            40157,
            "96e28c2a55d4153e2c8195606c0d2b3236b873136f99092abe8158fb49f898c5",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/b3540b1d35fbe4ea954d6f5ee2cf38eeedc27c0c5635d79a123883d0e6d5481c")).as_slice(),
            39198,
            "b3540b1d35fbe4ea954d6f5ee2cf38eeedc27c0c5635d79a123883d0e6d5481c",
        ),
        "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/tests.rs")).as_slice(),
            36455,
            "8f1327c88ae23e83819566e674c5c3cd983aec05a4e7f4d9815ebb8b0c3c9f54",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/61478bcc50bb3c4a2d640abb3cf8a05cf2a42816d1af80a9a01f7974633c6ba2")).as_slice(),
            30961,
            "61478bcc50bb3c4a2d640abb3cf8a05cf2a42816d1af80a9a01f7974633c6ba2",
        ),
        "src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs")).as_slice(),
            17605,
            "a6a8ba1ef8cdd1793bf1a7d6ddce7ffa9dd7ebf1834ee14fbdf1df357a9efec1",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/fmt-a110/ph_EVlWF6lq-xy2YIfGpPharqGOy-vsL7RIM7R2B49k.txt")).as_slice(),
            11649,
            "a61fc4565585ea5abec72d9821f1a93e16aba863b2fafb0bed120ced1d81e3d9",
        ),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs")).as_slice(),
            42390,
            "64e89c8d153fa9a970434c45abe3dab0e58690103ad2c388a64168781f174f11",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/fmt-local-441-20261018/whole/3a690560fa0fed1ce01cffed9cefab0753a4660cc6774f9dd68375759790b00a.txt")).as_slice(),
            37623,
            "3a690560fa0fed1ce01cffed9cefab0753a4660cc6774f9dd68375759790b00a",
        ),
        "src/cleanup_plan/replay/factored/moderate_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/moderate_tests.rs")).as_slice(),
            19377,
            "4c32d91e8bc3d5567f19984cb289471e285a0ae14b42a9909ca8dba489aa8224",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/0e3c100eef6708ff77be409f9d9299ab1d1cce3cfc96c22c1677cebda56d543b")).as_slice(),
            12894,
            "0e3c100eef6708ff77be409f9d9299ab1d1cce3cfc96c22c1677cebda56d543b",
        ),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_hook.rs")).as_slice(),
            13225,
            "926a833bbcf34fd7c826dfb91498565b683d6d9c1118a1c23e94c8f446f099d4",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/a0cf6a6604b1909859e78ecfe95f780ba1600072ae6ca9e259d2943c7dbebfa7")).as_slice(),
            12521,
            "a0cf6a6604b1909859e78ecfe95f780ba1600072ae6ca9e259d2943c7dbebfa7",
        ),
        "src/cleanup_plan/replay/factored/strategy.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/strategy.rs")).as_slice(),
            16924,
            "ce09eb553cc2041e7dd56621a7371b401207f80eabd2ee68542d9b766a7fc7c9",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/15654f12e2893a35782117e9876fdc8745a6a69cde17608e1797310d7a093579")).as_slice(),
            16594,
            "15654f12e2893a35782117e9876fdc8745a6a69cde17608e1797310d7a093579",
        ),
        "src/cleanup_plan/replay/factored.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs")).as_slice(),
            55919,
            "a707ba025d94fa1dfcd81f44b1984ee945c3479e4e0bae6f5f5cda2dde010269",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4")).as_slice(),
            52726,
            "8249547ae7f6fa877064e583e65415f6b9e65931c22eaba8cbb848f687935dc4",
        ),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/final_affine_tests.rs")).as_slice(),
            22359,
            "09c98cb0c881bdc947d5926c48af3cd75af7f2643f3de69d4a95ad691b9e01ea",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/6ab4e3e2ac19b71c15e10acb82747ff91e2d7f2e631ed6f6da16c2e946bc3fc2")).as_slice(),
            22093,
            "6ab4e3e2ac19b71c15e10acb82747ff91e2d7f2e631ed6f6da16c2e946bc3fc2",
        ),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/shared_prefix_tests.rs")).as_slice(),
            44699,
            "553a7a669cf2a7606c9cb205f252027fb83b515cb9fc43de39fd9abe3589a152",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/db5bafe83cfc9c0f9ccd195df7b006a19fc7a1475f82dbb302c6c0f11d05dbaf")).as_slice(),
            44431,
            "db5bafe83cfc9c0f9ccd195df7b006a19fc7a1475f82dbb302c6c0f11d05dbaf",
        ),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs")).as_slice(),
            17977,
            "f483721064e9e81524fdf22c6b41e07d045edfab0d9f4d0627e35e8c470bc33e",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/5534698ffa877cfeff74d028d5cd23ef13e100c0a361a14245512f9530e23745")).as_slice(),
            10327,
            "5534698ffa877cfeff74d028d5cd23ef13e100c0a361a14245512f9530e23745",
        ),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs")).as_slice(),
            35081,
            "5d34472a0f59197c69d691fd9cc9b2dc90424ad09c74287b31ea42b2df263eb6",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/67b827f40e61c90022e48c1c234b9f3f667dd5dc0ab799c57ee249bfbbfae93d")).as_slice(),
            20122,
            "67b827f40e61c90022e48c1c234b9f3f667dd5dc0ab799c57ee249bfbbfae93d",
        ),
        "src/cleanup_plan/replay/path_join/selected/frontier_tests.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests.rs")).as_slice(),
            20580,
            "3a5463e01040f296252142a47869022af5ddb2877b83ee805806ef7e9e4784d7",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/0a5d4e362fa7fb3411325c93472d9f3ab90e04464d03a06a919d5e7bf40166c5")).as_slice(),
            20257,
            "0a5d4e362fa7fb3411325c93472d9f3ab90e04464d03a06a919d5e7bf40166c5",
        ),
        "src/cleanup_plan/replay/path_join/selected/tests/mod.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/mod.rs")).as_slice(),
            9062,
            "7b1f8e69bcb7e577260e3568267b54c717e2a18949c6bff8da0b5a37f6a13a46",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/2a24be44491fea179ef26ff63c8fc5ad33b575a0204bdaf07975a0e2267852a2")).as_slice(),
            8585,
            "2a24be44491fea179ef26ff63c8fc5ad33b575a0204bdaf07975a0e2267852a2",
        ),
        "src/cleanup_plan/replay/path_join/selected/tests/session.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/session.rs")).as_slice(),
            6270,
            "c2a7426a13a310ac42c5ea718713c4bed6d573ebd8acc49c16ba2eafe4595380",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/a716f34c0e96b8917650e3f70d8526a91aff74ac50b09fe7f349c0c8da045dfa")).as_slice(),
            6018,
            "a716f34c0e96b8917650e3f70d8526a91aff74ac50b09fe7f349c0c8da045dfa",
        ),
        "src/cleanup_plan/replay/path_join/selected/tests/whole.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/whole.rs")).as_slice(),
            8466,
            "7be8da3cfc5b711340bb346680d1d73aa8ff3a35cfad6398c86849839f81f539",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/d7d68f8b047ea1abc4f182b604478f459ecdd397c74ab532957cbd5532cf2d6e")).as_slice(),
            8152,
            "d7d68f8b047ea1abc4f182b604478f459ecdd397c74ab532957cbd5532cf2d6e",
        ),
        "src/cleanup_plan/replay/transition.rs" => (
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/transition.rs")).as_slice(),
            36569,
            "1f07539f75b8884294fe1cf86ffeaffd1509f736d19be503f94afeb0ba94c455",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/6f90748e5cae2c16e44eedb83232fc19c33fbe6e01abf574c7efb93be47be983")).as_slice(),
            35499,
            "6f90748e5cae2c16e44eedb83232fc19c33fbe6e01abf574c7efb93be47be983",
        ),
        _ => return current.to_vec(),
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = std::fs::read(root.join(path)).unwrap();
    assert_eq!(physical.as_slice(), compiled);
    if lookup_epoch {
        assert_eq!(current, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/48ba251872ba4bf83424e95f78cbadcd11636432a03da7ec8e700e78425a353d")).as_slice());
    } else {
        assert_eq!(current, compiled);
    }
    assert_eq!(current.len(), length);
    assert_eq!(digest(current), sha);
    assert_eq!(before.len(), before_length);
    assert_eq!(digest(before), before_sha);
    let result = before.to_vec();
    assert_eq!(std::fs::read(root.join(path)).unwrap(), physical);
    result
}
