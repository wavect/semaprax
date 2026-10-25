//! Complete current source authentication before immutable legacy CI audits.
//! This test-only view owns no compiler, budget or profiling authority.
use sha2::{Digest, Sha256};
use std::borrow::Cow;
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
fn compiled(source: &str) -> Option<(usize, &'static str)> {
    Some(match source {
        "src/cleanup_plan/replay.rs" => (
            0,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay.rs"
            )),
        ),
        "src/cleanup_plan/replay/allocation.rs" => (
            1,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/allocation.rs"
            )),
        ),
        "src/cleanup_plan/replay/factored.rs" => (
            2,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/factored.rs"
            )),
        ),
        "src/cleanup_plan/replay/factored/continuation_receiver.rs" => (
            3,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/factored/continuation_receiver.rs"
            )),
        ),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => (
            4,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/factored/oracle_hook.rs"
            )),
        ),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs" => (
            5,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"
            )),
        ),
        "src/cleanup_plan/replay/ordinary_plan_history/producer.rs" => (
            6,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/ordinary_plan_history/producer.rs"
            )),
        ),
        "src/cleanup_plan/validate.rs" => (
            7,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/validate.rs"
            )),
        ),
        "src/hir.rs" => (
            8,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir.rs")),
        ),
        "src/loop_calls/byte_writer/source_index.rs" => (
            9,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/loop_calls/byte_writer/source_index.rs"
            )),
        ),
        "src/source_verify/declaration/functions.rs" => (
            10,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/source_verify/declaration/functions.rs"
            )),
        ),
        "src/source_verify/declaration/functions/public_signature_before.rs" => (
            11,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/source_verify/declaration/functions/public_signature_before.rs"
            )),
        ),
        "src/source_verify/iterative/enter.rs" => (
            12,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/source_verify/iterative/enter.rs"
            )),
        ),
        "src/source_verify/iterative/enter/enter_before_components.rs" => (
            13,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/source_verify/iterative/enter/enter_before_components.rs"
            )),
        ),
        "src/cleanup_plan/replay/expression_facts.rs" => (
            14,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/expression_facts.rs"
            )),
        ),
        "src/prelude.rs" => (
            15,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/prelude.rs")),
        ),
        "src/hir/validation/function_scope/lifecycle.rs" => (
            16,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/hir/validation/function_scope/lifecycle.rs"
            )),
        ),
        _ => return None,
    })
}
// Authenticate the actual compiled/physical/current row before selecting the
fn historical_entry<'a>(source: &str, expected: &str, current: &'a str) -> Cow<'a, str> {
    match (source, expected) {
        ("src/cleanup_plan/replay/factored/oracle_hook.rs", "a0cf6a6604b1909859e78ecfe95f780ba1600072ae6ca9e259d2943c7dbebfa7") => { assert_eq!((current.len(), digest(current)), (13225, "926a833bbcf34fd7c826dfb91498565b683d6d9c1118a1c23e94c8f446f099d4".to_owned())); let before = reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("e/a0cf6a6604b1909859e78ecfe95f780ba1600072ae6ca9e259d2943c7dbebfa7"), 12521, expected, 2_097_152).unwrap(); assert_eq!(digest(&before), expected); Cow::Owned(before) },
        (
            "src/cleanup_plan/replay.rs",
            "cc999fb9034687101cb5d34db54ed4ea58289ac6d9f9f7fae9f53e693231f74a",
        )
        | (
            "src/cleanup_plan/replay/factored.rs",
            "d3808da9519009c1a7b223a91a635bc94cdeb80a2c8bc40b22bcfc8b1f2f918c",
        ) => Cow::Owned(
            crate::source_reflows_merge::main_e177_final_inputs::value_direct(
                source, expected, current,
            ),
        ),
        _ => Cow::Borrowed(current),
    }
}
fn rows() -> serde_json::Value {
    const DATA: &str = include_str!("../../docs/evidence/ci-inputs-20261021/inputs.json");
    assert_eq!(DATA.len(), 79496);
    assert_eq!(
        digest(DATA),
        "e9d79f36b84d073716d0fecd680661d37f3effdf58f42d5e413174b37183f080"
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/evidence/ci-inputs-20261021/inputs.json");
    assert_eq!(
        reader::read(&path, DATA.len(), &digest(DATA), 2_097_152).unwrap(),
        DATA
    );
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 14);
    const EXTRA: &str = include_str!("../../docs/evidence/legacy-live-inputs-20261021/inputs.json");
    assert_eq!(EXTRA.len(), 1370);
    assert_eq!(
        digest(EXTRA),
        "c4afab464d3d50afa676c506750cb3a5977fd362e55c6228a57d333a9d30c771"
    );
    assert_eq!(
        reader::read(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/evidence/legacy-live-inputs-20261021/inputs.json"),
            EXTRA.len(),
            &digest(EXTRA),
            2_097_152
        )
        .unwrap(),
        EXTRA
    );
    let extra: serde_json::Value = serde_json::from_str(EXTRA).unwrap();
    assert_eq!(extra.as_array().unwrap().len(), 3);
    let mut rows = rows;
    rows.as_array_mut()
        .unwrap()
        .extend(extra.as_array().unwrap().iter().cloned());
    let successor = reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/compact-byte-errors-20261021/replay-successor.json"),
        5646,
        "2788ad869b721f06ff00d2b7cb36b8e23dd9d86bd4f500ef7ee9dd4c33f1314c",
        2_097_152,
    )
    .unwrap();
    let successor: serde_json::Value = serde_json::from_str(&successor).unwrap();
    let row = &mut rows[0];
    assert_eq!(row["source"], successor["source"]);
    assert_eq!(row["current_length"], successor["checkpoint_length"]);
    assert_eq!(row["current_sha256"], successor["checkpoint_sha256"]);
    let actual = compiled(row["source"].as_str().unwrap()).unwrap().1;
    let probe_before =
        crate::source_reflows_merge::before_phase_probes(row["source"].as_str().unwrap(), actual);
    let current = probe_before.as_ref();
    // Restore the exact pre-Main44 adapter input before its immutable inverses.
    assert_eq!(current.len(), 326351);
    assert_eq!(
        digest(current),
        "c4eb6d914f71ea5ea5ec3cee2710b4c18fee4c45b2c9019e8b48b5fb46653bbb"
    );
    let main44: serde_json::Value = serde_json::from_str("[[366,\"#[cfg(test)]\\npub(crate) use path_array_main44::current_source as main44_current_source;\\n\",\"\"],[2719,\"\",\"\\n\"]]").unwrap();

    let mut checkpoint = current.to_owned();
    for edit in main44.as_array().unwrap().iter().rev() {
        let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
        let after = edit[1].as_str().unwrap();
        assert_eq!(&checkpoint[at..at + after.len()], after);
        checkpoint.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
    }
    assert_eq!(checkpoint.len() as u64, successor["length"]);
    assert_eq!(digest(&checkpoint), successor["sha256"]);
    // The new main admission independently selects this same complete predecessor.
    let entered = historical_entry(
        row["source"].as_str().unwrap(),
        successor["sha256"].as_str().unwrap(),
        actual,
    );
    assert_eq!(entered.as_ref(), checkpoint);
    for edit in successor["indexed_inverse"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
    {
        let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
        let after = edit[1].as_str().unwrap();
        assert_eq!(&checkpoint[at..at + after.len()], after);
        checkpoint.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
    }
    assert_eq!(checkpoint.len() as u64, row["current_length"]);
    assert_eq!(digest(&checkpoint), row["current_sha256"]);
    row["inverse"].as_array_mut().unwrap().extend(
        successor["indexed_inverse"]
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    row["inverse"]
        .as_array_mut()
        .unwrap()
        .extend(main44.as_array().unwrap().iter().cloned());
    row["current_length"] = serde_json::json!(current.len());
    row["current_sha256"] = serde_json::json!(digest(current));
    // Authenticate subsequent complete source inputs before replaying the same
    // immutable CI bodies; each inverse binds every changed literal and offset.
    {
        let row = &mut rows[5];
        let actual_current = compiled(row["source"].as_str().unwrap()).unwrap().1;
        let (closure_epoch, acquisition) =
            crate::source_reflows_merge::Main44Fixture::closure_epoch(
                row["source"].as_str().unwrap(),
                "9dffc2821b02990bea08f9434b911c9dca98e52af5be26013bc1702916118742",
                actual_current,
            );
        let current = closure_epoch.as_ref();
        assert_eq!(current.len(), 29371);
        assert_eq!(
            digest(current),
            "9dffc2821b02990bea08f9434b911c9dca98e52af5be26013bc1702916118742"
        );
        let edits: serde_json::Value = serde_json::from_str("[[1911,\"\\n#[cfg(test)]\\n#[rustfmt::skip]\\nmod before_loaded_segments;\\n#[cfg(test)]\\n#[rustfmt::skip]\\nmod main_loaded_segments;\\n#[cfg(test)]\\n#[rustfmt::skip]\\nmod paired_segments_before;\\n\",\"\"],[13567,\"        _ => lexical_cursor::PRIMITIVE_FRAMES + paired_value::INDEPENDENT_PRIMITIVES,\\n\",\"        _ => lexical_cursor::PRIMITIVE_FRAMES,\\n\"],[13798,\"    #[cfg(test)]\\n    if owner.len >= 2 { census.independent_frames(); }\\n\",\"\"],[21245,\"                            certified_boundary!(owner, left_links, right_links, function, budget, census, 'certificate, 'paired);\\n\",\"                            certified_boundary!(owner, left_links, right_links, function, budget, census, 'certificate);\\n\"]]").unwrap();
        let mut checkpoint = current.to_owned();
        for edit in edits.as_array().unwrap().iter().rev() {
            let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
            let after = edit[1].as_str().unwrap();
            assert_eq!(&checkpoint[at..at + after.len()], after);
            checkpoint.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
        }
        assert_eq!(checkpoint.len() as u64, row["current_length"]);
        assert_eq!(digest(&checkpoint), row["current_sha256"]);
        row["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(edits.as_array().unwrap().iter().cloned());
        row["inverse"].as_array_mut().unwrap().extend(
            acquisition
                .iter()
                .map(|&(at, after, before)| serde_json::json!([at, after, before])),
        );
        row["current_length"] = serde_json::json!(actual_current.len());
        row["current_sha256"] = serde_json::json!(digest(actual_current));
    }
    {
        let row = &mut rows[8];
        let current = compiled(row["source"].as_str().unwrap()).unwrap().1;
        let current = constructor903_before(row["source"].as_str().unwrap(), current).unwrap_or_else(|| current.to_owned()); let current = current.as_str();
        assert_eq!(current.len(), 59526);
        assert_eq!(
            digest(current),
            "96f77aa63800b15888434dde208ea6ed3f9bf3294297bcc371b8dfacfc9c7cf3"
        );
        let edits: serde_json::Value =
            serde_json::from_str("[[5156,\"mod selected_function;\\n\",\"\"]]").unwrap();
        let mut checkpoint = current.to_owned();
        for edit in edits.as_array().unwrap().iter().rev() {
            let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
            let after = edit[1].as_str().unwrap();
            assert_eq!(&checkpoint[at..at + after.len()], after);
            checkpoint.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
        }
        assert_eq!(checkpoint.len() as u64, row["current_length"]);
        assert_eq!(digest(&checkpoint), row["current_sha256"]);
        row["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(edits.as_array().unwrap().iter().cloned());
        row["current_length"] = serde_json::json!(current.len());
        row["current_sha256"] = serde_json::json!(digest(current));
    }
    // Compose diagnostic-only inverses last, so they execute first on the
    // complete compiled/fresh current bytes before every existing CI inverse.
    let probes: serde_json::Value = serde_json::from_str("[{\"source\":\"src/cleanup_plan/validate.rs\",\"length\":4475,\"sha256\":\"75ac0859815d3a47a94b81493c04a8ae89516f4939fe79a8da7f94d141e41c1b\",\"inverse\":[[530,\"    #[cfg(test)]\\n    let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\\n        \\\"cleanup_plan::validate::canonical_rebuild\\\",\\n    );\\n\",\"\"],[2065,\"    #[cfg(test)]\\n    drop(_allocation_phase);\\n\",\"\"]]},{\"source\":\"src/cleanup_plan/replay.rs\",\"length\":326426,\"sha256\":\"5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d\",\"inverse\":[[2773,\"\",\"\\n\"],[2865,\"\",\"\\n\"],[4765,\"\",\"\\n\"],[6116,\"\",\"\\n\"],[6445,\"\",\"\\n\"],[6511,\"\",\"\\n\"],[113510,\"    #[cfg(test)]\\n    let _allocation_phase =\\n        crate::test_allocation_observer::function_phase_guard(\\\"cleanup_plan::replay::hir_paths\\\");\\n\",\"\"],[278824,\"    #[cfg(test)]\\n    let _allocation_phase =\\n        crate::test_allocation_observer::function_phase_guard(\\\"cleanup_plan::replay::plan_paths\\\");\\n\",\"\"],[576,\"use final_output::compact as compact_path;\\n\",\"\"],[122062,\"    let minimum = compact_path::count!(\\n\",\"    let minimum = path_array::count!(\\n\"],[122331,\"    final_output::compact::hir_selection!(work);\\n\",\"    work.budget.reserve_bytes(\\n        work.function,\\n        final_output::HIR_SELECTION_CONTROLS,\\n        \\\"HIR single-suffix selection controls\\\",\\n    )?;\\n\"],[122852,\"            compact_path::push!(\\n\",\"            path_array::push!(\\n\"],[123324,\"                final_output::compact::hir_move!(work);\\n                final_output::compact::hir_work!(work);\\n\",\"                work.budget.reserve_bytes(\\n                    work.function,\\n                    final_output::HIR_MOVE_CONTROLS,\\n                    \\\"HIR single-suffix move controls\\\",\\n                )?;\\n                work.charge(1, \\\"skeleton prefix clone\\\")?;\\n\"],[123938,\"            compact_path::push!(\\n\",\"            path_array::push!(\\n\"],[127075,\"            compact_path::push!(\\n\",\"            path_array::push!(\\n\"],[129471,\"        compact_path::push!(\\n\",\"        path_array::push!(\\n\"]]},{\"source\":\"src/cleanup_plan/replay/factored.rs\",\"length\":49022,\"sha256\":\"3e3835bc69e3029d4fbfb03639dab27573e3bdd3d8c02ea286a0cc53ea94ea02\",\"inverse\":[[41290,\"    let enumeration_phase = crate::test_allocation_observer::function_phase_guard(\\n        \\\"cleanup_plan::crosscheck::enumerated_hir_plan\\\",\\n    );\\n\",\"\"],[41845,\"    drop(enumeration_phase);\\n\",\"\"],[42067,\"        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\\n            \\\"cleanup_plan::crosscheck::factored_compare\\\",\\n        );\\n\",\"\"],[14022,\"    let Some(hir) = canonical.id_with_compact_receiving(\\n        function,\\n        vec![(hir_root, 1)],\\n        budget,\\n        &mut receiving,\\n    )?\\n\",\"    let Some(hir) =\\n        canonical.id_with_receiving(function, vec![(hir_root, 1)], budget, &mut receiving)?\\n\"],[14504,\"    let Some(plan) = canonical.id_with_compact_receiving(\\n        function,\\n        vec![(plan_root, 1)],\\n        budget,\\n        &mut receiving,\\n    )?\\n\",\"    let Some(plan) =\\n        canonical.id_with_receiving(function, vec![(plan_root, 1)], budget, &mut receiving)?\\n\"]]},{\"source\":\"src/cleanup_plan/replay/factored/continuation_receiver.rs\",\"length\":2919,\"sha256\":\"f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a\",\"inverse\":[[803,\"    let enumeration_phase = crate::test_allocation_observer::function_phase_guard(\\n        \\\"cleanup_plan::crosscheck::enumerated_hir_plan\\\",\\n    );\\n\",\"\"],[1358,\"    drop(enumeration_phase);\\n\",\"\"],[1580,\"        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\\n            \\\"cleanup_plan::crosscheck::factored_compare\\\",\\n        );\\n\",\"\"]]}]").unwrap();
    let mut probes = probes;
    // Acquire the complete current replay image before the unchanged old probe.
    {
        let probe = &mut probes[1];
        assert_eq!(probe["source"], "src/cleanup_plan/replay.rs");
        assert_eq!(probe["length"], 326426);
        assert_eq!(
            probe["sha256"],
            "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
        );
        let original_probe = probe.clone();
        let actual = compiled("src/cleanup_plan/replay.rs").unwrap().1;
        assert_eq!(actual.len(), 327551);
        assert_eq!(
            digest(actual),
            "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5"
        );
        assert_eq!(
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cleanup_plan/replay.rs")
            )
            .unwrap(),
            actual.as_bytes()
        );
        let extra: serde_json::Value = serde_json::from_str("[[1097,\"mod plan_prefix;\\n\",\"\"],[108404,\"#[rustfmt::skip]\\n\",\"\"],[109364,\"    ordinary_plan_history::completed_fork::shipping::finish!(function, budget, expected);\\n\",\"\"],[112083,\"        ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } if { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } => !plan_prefix::equal_paths(function, budget, &mut expected, &mut actual)?,\\n\",\"\"],[116424,\"        budget.reserve_bytes(function, skeleton_work::query_proof::HEADER, \\\"skeleton query neutral slot receivers\\\")?;\\n        SkeletonWork { queries: skeleton_work::query_proof::Slot::new(function, budget),\\n\",\"        SkeletonWork {\\n\"],[122436,\"    let prefixes = ordinary_plan_history::completed_fork::emission::dispatch!(\\n        prefixes, suffixes, work\\n    );\\n\",\"\"],[122660,\"    let share_prefix = work.prefix_tail_sharing(&suffixes)?;\\n\",\"    let share_prefix = work.shares_prefix_tail(&suffixes)?;\\n\"],[123169,\"        sequence_vector::reuse!(prefixes, suffixes, work, token share_prefix, minimum, precharged);\\n\",\"        sequence_vector::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged);\\n\"],[123805,\"        if !precharged {\\n            skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);\\n\",\"        if !precharged && share_prefix {\\n            work.share_prefix_tail(&mut prefix.observations)?;\\n\"],[269073,\"    let share_prefix = work.prefix_tail_sharing(&suffixes)?;\\n\",\"    let share_prefix = work.shares_prefix_tail(&suffixes)?;\\n\"],[269543,\"        skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);\\n\\n\",\"        if share_prefix {\\n            work.share_prefix_tail(&mut prefix.observations)?;\\n        }\\n\"],[269792,\"            let observations = ordinary_plan_history::completed_fork::emission::extend_call!(\\n                observations, suffix, work\\n\",\"            let observations = work.extend_owned_observations(\\n                observations,\\n                &suffix.observations,\\n                \\\"call suffix clone\\\",\\n\"],[279166,\"#[rustfmt::skip]\\n\",\"\"],[279316,\"    budget.reserve_bytes(function, plan_prefix::QUERY_CONTROLS, \\\"PLAN prefix eligibility receivers\\\")?;\\n    if matches!(budget.payer, AllocationPayer::LegacyUnobserved) && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } { return plan_prefix::build(function, budget); }\\n\",\"\"],[289554,\"                compact_path::plan_selection!(budget, function);\\n\",\"                budget.reserve_bytes(\\n                    function,\\n                    branch_clone::direct::PLAN_SMALL_SELECTION_CONTROLS,\\n                    \\\"plan final-output selection controls\\\",\\n                )?;\\n\"],[289948,\"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\",\"                        budget.reserve_bytes(\\n\"],[290070,\"                            branch_clone::direct::plan_compact_roles::<\\n\",\"                            branch_clone::direct::compact_roles::<\\n\"],[290357,\"                        ));\\n\",\"                        )?;\\n\"],[291020,\"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\",\"                        budget.reserve_bytes(\\n\"],[291142,\"                            branch_clone::plan_compact_roles::<std::rc::Rc<SkeletonObservation>>(),\\n\",\"                            branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>(),\\n\"],[291305,\"                        ));\\n\",\"                        )?;\\n\"],[291645,\"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\",\"                        budget.reserve_bytes(\\n\"],[291767,\"                            branch_clone::direct::plan_compact_roles::<\\n\",\"                            branch_clone::direct::compact_roles::<\\n\"],[292054,\"                        ));\\n\",\"                        )?;\\n\"],[292911,\"                    compact_path::plan_move!(budget, function);\\n                    compact_path::plan_work!(\\n                        budget,\\n\",\"                    budget.reserve_bytes(\\n\"],[293087,\"                        \\\"cleanup-plan skeleton branch clone\\\"\\n                    );\\n\",\"                        final_output::PLAN_MOVE_CONTROLS,\\n                        \\\"plan final-output move controls\\\",\\n                    )?;\\n                    budget.charge_skeleton(function, 1, \\\"cleanup-plan skeleton branch clone\\\")?;\\n\"],[293480,\"                    compact_path::plan_work!(budget, function, \\\"plan branch observation push\\\");\\n\",\"                    budget.charge_skeleton(function, 1, \\\"plan branch observation push\\\")?;\\n\"],[293629,\"                    compact_path::plan_auth!(budget, function);\\n                    compact_path::plan_conversion!(budget, function, observation);\\n\",\"                    budget.reserve_bytes(function, 0, \\\"plan branch observation push\\\")?;\\n                    budget.reserve_bytes(\\n                        function,\\n                        physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(\\n                            &observation,\\n                        ),\\n                        \\\"plan branch observation push\\\",\\n                    )?;\\n\"],[297849,\"                        branch_clone::short_plan::produce!(\\n                            budget,\\n                            function,\\n                            observations,\\n                            observation,\\n                            edge,\\n                            queue\\n                        );\\n\",\"\"],[324946,\"#[cfg(test)]\\nmod workspace_oracle;\\n\",\"\\n\"],[325059,\"#[cfg(test)]\\npub(crate) use workspace_oracle::with_financed_workspace_test_oracles;\\n\",\"\\n\"],[325167,\"\",\"\\n\"]]").unwrap();
        let mut prior = actual.to_owned();
        for edit in extra.as_array().unwrap().iter().rev() {
            let at = edit[0].as_u64().unwrap() as usize;
            let after = edit[1].as_str().unwrap();
            assert_eq!(prior.get(at..at + after.len()), Some(after));
            prior.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
        }
        let fixture = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
        ));
        assert_eq!(prior.as_bytes(), fixture.as_slice());
        assert_eq!(prior.len(), 326426);
        assert_eq!(
            digest(&prior),
            "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
        );
        probe["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(extra.as_array().unwrap().iter().cloned());
        probe["length"] = serde_json::json!(actual.len());
        probe["sha256"] = serde_json::json!(digest(actual));
        probe["retained_prior_record"] = original_probe;
    }
    // Preserve the complete original probe entry, then compose the two exact
    // current early-release splices ahead of every immutable indexed inverse.
    {
        let probe = &mut probes[2];
        assert_eq!(probe["source"], "src/cleanup_plan/replay/factored.rs");
        assert_eq!(probe["length"], 49022);
        assert_eq!(
            probe["sha256"],
            "3e3835bc69e3029d4fbfb03639dab27573e3bdd3d8c02ea286a0cc53ea94ea02"
        );
        let actual_current = compiled(probe["source"].as_str().unwrap()).unwrap().1;
        let (closure_epoch, acquisition) =
            crate::source_reflows_merge::Main44Fixture::closure_epoch(
                probe["source"].as_str().unwrap(),
                "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8",
                actual_current,
            );
        let current = closure_epoch.as_ref();
        assert_eq!(current.len(), 49837);
        assert_eq!(
            digest(current),
            "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8"
        );
        let extra: serde_json::Value = serde_json::from_str("[[2039,\"#[cfg(test)]\\nmod early_release;\\n\",\"\"],[18392,\"        // Automaton labels now own every retained observation. Each explicit\\n        // release pays its argument/formal and reservation-result transport.\\n        #[cfg(test)]\\n        early_release::frontier(0, budget);\\n        budget.reserve_array::<(dag_memo::Memo, dag_memo::Memo, Result<(), Diagnostic>)>(\\n            function,\\n            1,\\n            \\\"factored DAG memo release transport\\\",\\n        )?;\\n        drop(memo);\\n        #[cfg(test)]\\n        early_release::frontier(1, budget);\\n        budget.reserve_array::<(Vec<SkeletonPath>, Vec<SkeletonPath>, Result<(), Diagnostic>)>(\\n            function,\\n            1,\\n            \\\"factored HIR paths release transport\\\",\\n        )?;\\n        drop(hir_paths);\\n        #[cfg(test)]\\n        early_release::frontier(2, budget);\\n\",\"\"]]").unwrap();
        let mut restored = current.to_owned();
        for edit in extra.as_array().unwrap().iter().rev() {
            let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
            let after = edit[1].as_str().unwrap();
            assert_eq!(&restored[at..at + after.len()], after);
            restored.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
        }
        assert_eq!(restored.len() as u64, probe["length"]);
        assert_eq!(digest(&restored), probe["sha256"]);
        probe["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(extra.as_array().unwrap().iter().cloned());
        probe["inverse"].as_array_mut().unwrap().extend(
            acquisition
                .iter()
                .map(|&(at, after, before)| serde_json::json!([at, after, before])),
        );
        probe["length"] = serde_json::json!(actual_current.len());
        probe["sha256"] = serde_json::json!(digest(actual_current));
    }
    {
        let probe = &mut probes[3];
        let prior_record = probe.clone();
        assert_eq!(
            probe["source"],
            "src/cleanup_plan/replay/factored/continuation_receiver.rs"
        );
        assert_eq!(probe["length"], 2919);
        assert_eq!(
            probe["sha256"],
            "f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a"
        );
        let actual = compiled(probe["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(actual.len(), 3211);
        assert_eq!(
            digest(actual),
            "57fb8fa2a03008f6088a2891972a8c3ff17d4eea9eec508809e898eb85a5e60b"
        );
        let extra: serde_json::Value = serde_json::from_str("[[1293,\"if matches!(enumerating.payer, AllocationPayer::LegacyUnobserved)\\n            && !ordinary_plan_history::completed_fork::emission::before_selected()\\n        {\\n            plan_prefix::equal_paths(function, &mut enumerating, &mut hir, &mut plan)\\n        } else {\\n            \",\"\"],[1579,\"    \",\"\"],[1612,\"    \",\"\"],[1631,\"\\n        }\",\"\"]]").unwrap();
        let mut prior = actual.to_owned();
        for edit in extra.as_array().unwrap().iter().rev() {
            let at = edit[0].as_u64().unwrap() as usize;
            let after = edit[1].as_str().unwrap();
            assert_eq!(prior.get(at..at + after.len()), Some(after));
            prior.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
        }
        let fixture = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a"
        ));
        assert_eq!(prior.as_bytes(), fixture.as_slice());
        assert_eq!(prior.len(), 2919);
        assert_eq!(
            digest(&prior),
            "f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a"
        );
        probe["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(extra.as_array().unwrap().iter().cloned());
        probe["length"] = serde_json::json!(actual.len());
        probe["sha256"] = serde_json::json!(digest(actual));
        probe["retained_prior_record"] = prior_record;
    }
    for probe in probes.as_array().unwrap() {
        let row = rows
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["source"] == probe["source"])
            .unwrap();
        let actual = compiled(row["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(actual.len() as u64, probe["length"]);
        assert_eq!(digest(actual), probe["sha256"]);
        let prior = crate::source_reflows_merge::before_phase_probes(
            row["source"].as_str().unwrap(),
            actual,
        );
        assert_eq!(prior.len() as u64, row["current_length"]);
        assert_eq!(digest(&prior), row["current_sha256"]);
        row["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(probe["inverse"].as_array().unwrap().iter().cloned());
        row["current_length"] = probe["length"].clone();
        row["current_sha256"] = probe["sha256"].clone();
    }
    // Extend only the existing lifecycle row; its immutable input and body
    // remain the predecessor of this exact private visibility repair.
    {
        let row = &mut rows[16];
        assert_eq!(
            row["source"],
            "src/hir/validation/function_scope/lifecycle.rs"
        );
        let current = compiled(row["source"].as_str().unwrap()).unwrap().1;
        let before = lifecycle_visibility_entry(row["source"].as_str().unwrap(), current).unwrap();
        assert_eq!(before.len() as u64, row["current_length"]);
        assert_eq!(digest(&before), row["current_sha256"]);
        let extra: serde_json::Value = serde_json::from_str("[[1296,\"pub(in crate::hir::validation) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {\\n\",\"pub(super) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {\\n\"],[3932,\"pub(in crate::hir::validation) mod completed;\\n\",\"pub(super) mod completed;\\n\"]]").unwrap();
        row["inverse"]
            .as_array_mut()
            .unwrap()
            .extend(extra.as_array().unwrap().iter().cloned());
        row["current_length"] = serde_json::json!(current.len());
        row["current_sha256"] = serde_json::json!(digest(current));
    }
    rows
}
pub(super) fn prior<'a>(source: &str, supplied: &'a str) -> Cow<'a, str> {
    let Some((index, current)) = compiled(source) else {
        return Cow::Borrowed(supplied);
    };
    let rows = rows();
    let row = &rows[index];
    assert_eq!(row["source"], source);
    assert_eq!(
        supplied, current,
        "complete compiled current CI history input"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fresh = reader::read(
        &root.join(source),
        current.len(),
        &digest(current),
        2_097_152,
    )
    .unwrap();
    assert_eq!(fresh, current, "complete fresh physical CI history input");
    let source903 = constructor903_before(source, current);
    let entered = historical_entry(source, row["current_sha256"].as_str().unwrap(), source903.as_deref().unwrap_or(current));
    assert_eq!(entered.len() as u64, row["current_length"]);
    assert_eq!(digest(&entered), row["current_sha256"]);
    let mut before = entered.into_owned();
    for edit in row["inverse"].as_array().unwrap().iter().rev() {
        let at = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap();
        assert_eq!(&before[at..at + after.len()], after);
        before.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
    }
    assert_eq!(before.len() as u64, row["before_length"]);
    assert_eq!(digest(&before), row["before_sha256"]);
    Cow::Owned(before)
}
pub(super) fn file_expected(
    path: &std::path::Path,
    length: usize,
    expected: &str,
) -> Option<String> {
    let source = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).ok()?;
    if !source
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    let source = source.to_str()?;
    let (index, current) = compiled(source)?;
    let rows = rows();
    let row = &rows[index];
    if row["before_length"].as_u64() != Some(length as u64) || row["before_sha256"] != expected {
        return None;
    }
    Some(prior(source, current).into_owned())
}
// Adapt an exact raw current input only when its consumer names this epoch.
// Already-projected inputs keep the original consumer's unchanged guards.
pub(super) fn for_expected<'a>(source: &str, expected: &str, supplied: &'a str) -> Cow<'a, str> {
    let Some((index, current)) = compiled(source) else {
        return Cow::Borrowed(supplied);
    };
    if supplied != current || rows()[index]["before_sha256"] != expected {
        return Cow::Borrowed(supplied);
    }
    prior(source, supplied)
}
// Registered sources keep their existing complete-view acquisition route.
// These nine raw inputs have no row in that immutable source membership.
#[allow(
    dead_code,
    reason = "This closed reader also compiles in namespaces that only use exact expected-input adapters."
)]
pub(super) fn for_ci<'a>(source: &str, supplied: &'a str) -> Cow<'a, str> {
    match source {
        "src/cleanup_plan/replay/allocation.rs"
        | "src/cleanup_plan/replay/ordinary_plan_history/producer.rs"
        | "src/cleanup_plan/validate.rs" => prior(source, supplied),
        "src/hir.rs" => prior(source, supplied),
        "src/loop_calls/byte_writer/source_index.rs" => prior(source, supplied),
        "src/source_verify/declaration/functions.rs" => prior(source, supplied),
        "src/source_verify/declaration/functions/public_signature_before.rs" => {
            prior(source, supplied)
        }
        "src/source_verify/iterative/enter.rs" => prior(source, supplied),
        "src/source_verify/iterative/enter/enter_before_components.rs" => prior(source, supplied),
        _ => Cow::Borrowed(supplied),
    }
}
#[test]
fn fourteen_complete_ci_input_inverses_keep_independent_git_bodies_and_refuse_stale_or_changed_source(
) {
    let cases = [
        ("src/cleanup_plan/replay.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/e688f57ac691a478b7c751ad69975d3ba447ffdc6ac8f71b018a5d438e8f3d0c.txt")),
        ("src/cleanup_plan/replay/allocation.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/0ad98be164b6f5f2340e57f769f5dc6b2bdc33eee75df00d77abe5352eed98da.txt")),
        ("src/cleanup_plan/replay/factored.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/045b2a3e7c1872ead79cfdf897f7337764d555d0c1db9c9ebb280f7e6365ca39.txt")),
        ("src/cleanup_plan/replay/factored/continuation_receiver.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/c6256e04224731110b5e1906169ec5e55139147f18433700825e18c982007794.txt")),
        ("src/cleanup_plan/replay/factored/oracle_hook.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/8cb40146286217b0816abb9f576b9d72805b3a38c0e48d5b71e51fedead76800.txt")),
        ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/47bbb8f39825c8551a781ea89da0e9bf4a2257a2920ded4b44f3933dc78a8285.txt")),
        ("src/cleanup_plan/replay/ordinary_plan_history/producer.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/39b563ae65f3cfbb62f944e84ac133435106412a0f6dddd3090a82fcdc00cdc2.txt")),
        ("src/cleanup_plan/validate.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/ca6b3e750717273c50887a6aa23aa72bc2f0c6a1d93af04228b77b63b2e16545.txt")),
        ("src/hir.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/e3aaf1c870be500fa794ec24d393f8dc94fcd996990abf388222ce079f4e0b04.txt")),
        ("src/loop_calls/byte_writer/source_index.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/6b5d4943365b08b9418a95c28e1601e3240d9d43b1c9c686530f4568c515756d.txt")),
        ("src/source_verify/declaration/functions.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/9813f3255640ce6fd02167491582b4a7b6b141a0b472cf561de6e52abb25a79a.txt")),
        ("src/source_verify/declaration/functions/public_signature_before.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/05cb069082838883c82b5b638e7acdbedd1a3ffc089894528c3b5105839217f8.txt")),
        ("src/source_verify/iterative/enter.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/4d6e8b68b9de792aa588a9a6cc4815308dcb318cc1735a527b9b9da5506f7661.txt")),
        ("src/source_verify/iterative/enter/enter_before_components.rs", include_str!("../../docs/evidence/ci-inputs-20261021/before/b2ba677d2dce0ce94b974200ff62e8eb590cf39e65e1a1b1bdca99ff05394f47.txt")),
        ("src/cleanup_plan/replay/expression_facts.rs", include_str!("../../docs/evidence/legacy-live-inputs-20261021/before/053698df5f2f88f5e6fde6c2d3a0c47f7eb683cc34dca0365f4be56682da4ae5.txt")),
        ("src/prelude.rs", include_str!("../../docs/evidence/legacy-live-inputs-20261021/before/32a5461db85f2832bb788ed32efd1df82eb8f48ffe44c51c533ceb20a596b478.txt")),
        ("src/hir/validation/function_scope/lifecycle.rs", include_str!("../../docs/evidence/legacy-live-inputs-20261021/before/4359a3991e559f31cbb1ad650886ed3433270cbf17ce535169b3ff04fe64027a.txt")),
    ];
    for (source, before) in cases {
        let (_, current) = compiled(source).unwrap();
        assert_eq!(prior(source, current), before);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
        assert_eq!(
            file_expected(&path, before.len(), &digest(before)).as_deref(),
            Some(before)
        );
        assert!(file_expected(&path, before.len() + 1, &digest(before)).is_none());
        assert!(file_expected(&path, before.len(), &"0".repeat(64)).is_none());
        assert_eq!(for_expected(source, &digest(before), current), before);
        assert_eq!(for_expected(source, &"0".repeat(64), current), current);
        assert_eq!(for_expected(source, &digest(before), before), before);
        for hostile in [
            format!("{current}\n"),
            current[..current.len() - 1].to_owned(),
            before.to_owned(),
        ] {
            assert!(std::panic::catch_unwind(|| prior(source, &hostile)).is_err());
        }
    }
    assert_eq!(prior("foreign.rs", "unlisted input"), "unlisted input");
    for (source, current) in [
        (
            "src/hir/validation/function_scope/lifecycle.rs",
            include_str!("../hir/validation/function_scope/lifecycle.rs"),
        ),
        (
            "src/hir/validation/function_scope/lifecycle/completed.rs",
            include_str!("../hir/validation/function_scope/lifecycle/completed.rs"),
        ),
    ] {
        let before = lifecycle_visibility_entry(source, current).unwrap();
        assert_ne!(before, current);
        for bad in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            before,
        ] {
            assert!(std::panic::catch_unwind(|| lifecycle_visibility_entry(source, &bad)).is_err());
        }
    }
    assert!(lifecycle_visibility_entry("foreign.rs", "unlisted input").is_none());
    for (source, current) in [
        (
            "src/hir/validation/scope_checkpoint/marks.rs",
            include_str!("../hir/validation/scope_checkpoint/marks.rs"),
        ),
        (
            "src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs",
            include_str!("../cleanup_plan/replay/ordinary_plan_history/frontiers.rs"),
        ),
        (
            "src/hir/validation/frame_reuse/tests.rs",
            include_str!("../hir/validation/frame_reuse/tests.rs"),
        ),
    ] {
        let before = batch_913_entry(source, current).unwrap();
        assert_ne!(before, current);
        for bad in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            before,
        ] {
            assert!(std::panic::catch_unwind(|| batch_913_entry(source, &bad)).is_err());
        }
    }
    assert!(batch_913_entry("foreign.rs", "unlisted input").is_none());
    for (source, current) in [
        (
            "src/cleanup_plan/replay/path_join/selected.rs",
            include_str!("../cleanup_plan/replay/path_join/selected.rs"),
        ),
        (
            "src/cleanup_plan/replay/path_join/selected/walk.rs",
            include_str!("../cleanup_plan/replay/path_join/selected/walk.rs"),
        ),
        (
            "src/cleanup_plan/replay/path_join/selected/queries.rs",
            include_str!("../cleanup_plan/replay/path_join/selected/queries.rs"),
        ),
        (
            "src/cleanup_plan/replay/path_join/selected/receiving.rs",
            include_str!("../cleanup_plan/replay/path_join/selected/receiving.rs"),
        ),
        (
            "src/cleanup_plan/replay/path_join/selected/tests/mod.rs",
            include_str!("../cleanup_plan/replay/path_join/selected/tests/mod.rs"),
        ),
    ] {
        let before = compact_frontier_entry(source, current).unwrap();
        assert_ne!(before, current);
        for bad in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            before,
        ] {
            assert!(std::panic::catch_unwind(|| compact_frontier_entry(source, &bad)).is_err());
        }
    }
    assert!(compact_frontier_entry("foreign.rs", "unlisted input").is_none());
    reader::controls();
}

// This controller alone consumes the pre-profile intrinsic-dispatch source.
// The actual compiler and every other source reader keep the current iterator.
pub(super) fn intrinsic_profile_entry(
    caller: &str,
    source: &str,
    supplied: &str,
) -> Option<String> {
    const PAID: &str = "src/hir/validation/function_lookup/paid_custody.rs";
    if caller == PAID || caller.ends_with("/src/hir/validation/function_lookup/paid_custody.rs") {
        return paid_constructor_entry(source, supplied);
    }
    const CALLER: &str = "src/hir/validation/constructor_seed/intrinsic_tests.rs";
    if caller != CALLER
        && !caller.ends_with(concat!(
            "/",
            "src/hir/validation/constructor_seed/intrinsic_tests.rs"
        ))
    {
        return None;
    }
    let compiled = match source {
        "src/list_ops.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/list_ops.rs")),
        "src/iterator_ops.rs" => {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/iterator_ops.rs"))
        }
        _ => return None,
    };
    const DATA: &str =
        include_str!("../../docs/evidence/intrinsic-profile-entry-20261022/inputs.json");
    assert_eq!(DATA.len(), 1156);
    assert_eq!(
        digest(DATA),
        "f7d25b201e0a48dd3675539096dbe998e92b1fbc0ee611fe7f7f3e73f7679a14"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("docs/evidence/intrinsic-profile-entry-20261022/inputs.json"),
            DATA.len(),
            &digest(DATA),
            2_097_152
        )
        .unwrap(),
        DATA
    );
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
        .unwrap();
    assert_eq!(
        supplied, compiled,
        "complete actual intrinsic source channel"
    );
    assert_eq!(compiled.len() as u64, row["current_length"]);
    assert_eq!(digest(compiled), row["current_sha256"]);
    assert_eq!(
        reader::read(
            &root.join(source),
            compiled.len(),
            row["current_sha256"].as_str().unwrap(),
            2_097_152
        )
        .unwrap(),
        compiled
    );
    let after = row["after"].as_str().unwrap();
    assert_eq!(compiled.matches(after).count(), 1);
    let previous = compiled.replacen(after, row["before"].as_str().unwrap(), 1);
    assert_eq!(previous.len() as u64, row["before_length"]);
    assert_eq!(digest(&previous), row["before_sha256"]);
    assert_eq!(
        reader::read(
            &root.join(row["archive"].as_str().unwrap()),
            previous.len(),
            row["before_sha256"].as_str().unwrap(),
            2_097_152
        )
        .unwrap(),
        previous
    );
    Some(previous)
}

#[test]
fn intrinsic_profile_entry_keeps_exact_controller_scope_and_refuses_complete_source_drift() {
    let caller = "src/hir/validation/constructor_seed/intrinsic_tests.rs";
    for (source, current) in [
        ("src/list_ops.rs", include_str!("../list_ops.rs")),
        ("src/iterator_ops.rs", include_str!("../iterator_ops.rs")),
    ] {
        let before = intrinsic_profile_entry(caller, source, current).unwrap();
        assert_ne!(before, current);
        assert!(intrinsic_profile_entry("src/another_controller.rs", source, current).is_none());
        for changed in [
            format!("{current}\n"),
            current[..current.len() - 1].to_owned(),
            before,
        ] {
            assert!(
                std::panic::catch_unwind(|| intrinsic_profile_entry(caller, source, &changed))
                    .is_err()
            );
        }
    }
    assert!(intrinsic_profile_entry(caller, "src/hir.rs", "foreign source").is_none());
}

fn paid_constructor_entry(source: &str, supplied: &str) -> Option<String> {
    if source == "src/stdin_stream_ops.rs" {
        return paid_stdin_entry(supplied);
    }
    if source != "src/hir/function_directory/caller.rs" {
        return None;
    }
    const CURRENT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/hir/function_directory/caller.rs"
    ));
    assert_eq!(supplied, CURRENT, "complete actual paid-constructor source");
    assert_eq!(CURRENT.len(), 4600);
    assert_eq!(
        digest(CURRENT),
        "63c8121f756943b8be3cfa53a0c4862459c7af14c3ef3124b75468546a42af35"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join(source),
            CURRENT.len(),
            &digest(CURRENT),
            2_097_152
        )
        .unwrap(),
        CURRENT
    );
    const AFTER: &str = "    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n        crate::bounded_output::exclusive_debits::Operation::FunctionDirectory,\n    );\n";
    assert_eq!(CURRENT.matches(AFTER).count(), 1);
    let before = CURRENT.replacen(AFTER, "", 1);
    assert_eq!(before.len(), 4373);
    assert_eq!(
        digest(&before),
        "217896ad2e11c8bcbdfde236ec29a9cc1c00da142c9c18968a6015b79451127a"
    );
    assert_eq!(reader::read(&root.join("docs/evidence/paid-constructor-entry-20261022/217896ad2e11c8bcbdfde236ec29a9cc1c00da142c9c18968a6015b79451127a.txt"), before.len(), &digest(&before), 2_097_152).unwrap(), before);
    Some(before)
}

fn paid_stdin_entry(supplied: &str) -> Option<String> {
    const CURRENT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/stdin_stream_ops.rs"
    ));
    assert_eq!(supplied, CURRENT, "complete actual paid stdin source");
    assert_eq!(CURRENT.len(), 17324);
    assert_eq!(
        digest(CURRENT),
        "5c216d1345d20face39f0e07ac9554c2d659b0a171fcf0cbb33dd60684961836"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/stdin_stream_ops.rs"),
            CURRENT.len(),
            &digest(CURRENT),
            2_097_152
        )
        .unwrap(),
        CURRENT
    );
    let mut before = CURRENT.to_owned();
    assert_eq!(before.matches(".function_refs()\n                .find_named_payload(operation.as_str(), ast_forward_signature)\n                .is_some()").count(), 1);
    before = before.replacen(".function_refs()\n                .find_named_payload(operation.as_str(), ast_forward_signature)\n                .is_some()", ".functions()\n                .iter()\n                .any(|function| function.name == *operation && ast_forward_signature(function))", 1);
    assert_eq!(before.matches("program\n        .functions()\n        .profile_iter()\n        .any(|function| function_uses(function.payload()))").count(), 1);
    before = before.replacen("program\n        .functions()\n        .profile_iter()\n        .any(|function| function_uses(function.payload()))", "program.functions().iter().any(function_uses)", 1);
    assert_eq!(before.len(), 17268);
    assert_eq!(
        digest(&before),
        "bc1e107b8756db7b323000b0d282b731db08b06b29b21ca9a3e86bb6366bef97"
    );
    assert_eq!(reader::read(&root.join("docs/evidence/paid-constructor-entry-20261022/bc1e107b8756db7b323000b0d282b731db08b06b29b21ca9a3e86bb6366bef97.txt"), before.len(), &digest(&before), 2_097_152).unwrap(), before);
    Some(before)
}

#[test]
fn paid_constructor_entries_refuse_foreign_controllers_and_changed_whole_sources() {
    let caller = "src/hir/validation/function_lookup/paid_custody.rs";
    for (source, current) in [
        (
            "src/hir/function_directory/caller.rs",
            include_str!("../hir/function_directory/caller.rs"),
        ),
        (
            "src/stdin_stream_ops.rs",
            include_str!("../stdin_stream_ops.rs"),
        ),
    ] {
        let before = intrinsic_profile_entry(caller, source, current).unwrap();
        assert_ne!(before, current);
        assert!(intrinsic_profile_entry("src/foreign.rs", source, current).is_none());
        for changed in [
            format!("{current}\n"),
            current[..current.len() - 1].to_owned(),
            before,
        ] {
            assert!(
                std::panic::catch_unwind(|| intrinsic_profile_entry(caller, source, &changed))
                    .is_err()
            );
        }
    }
    assert!(intrinsic_profile_entry(caller, "src/hir.rs", "foreign").is_none());
}

// The relocated validator receiver shares these private helpers. Every older
// source consumer enters through the exact six visibility-only reversals.
pub(super) fn lifecycle_visibility_entry(source: &str, supplied: &str) -> Option<String> {
    let (compiled, length, expected, prior_length, prior_sha, edits):
        (&str, usize, &str, usize, &str, &[(usize, &str, &str)]) = match source {
        "src/hir/validation/function_scope/lifecycle.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle.rs")), 3978, "8095a604e5320261b71d1681fd53fe4ba2a2e7dac7f2a7a13c0e505563e815df", 3938, "d043c680becb6e6385559f4d0d8e86e5eff79f5dbdca72cfbe97b058cc18d8bd", &[
            (1296, "pub(in crate::hir::validation) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {\n", "pub(super) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {\n"),
            (3932, "pub(in crate::hir::validation) mod completed;\n", "pub(super) mod completed;\n"),
        ]),
        "src/hir/validation/function_scope/lifecycle/completed.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/lifecycle/completed.rs")), 3672, "6444aa05d9e16fffa13f1d53018cad6793f32d5560adc09fe28309e20ef5fca9", 3736, "5ae07ffcc78eb7ac8c15abd3d8ff9e1bbf1f72d8307760ade92cb7d443135bfe", &[
            (287, "pub(in crate::hir::validation) struct Collector<'a> {\n", "pub(in crate::hir::validation::function_scope) struct Collector<'a> {\n"),
            (561, "    pub(in crate::hir::validation) fn new(program: &'a ResolvedProgram) -> Self {\n", "    pub(in crate::hir::validation::function_scope) fn new(program: &'a ResolvedProgram) -> Self {\n"),
            (1861, "    pub(in crate::hir::validation) fn extend(\n", "    pub(in crate::hir::validation::function_scope) fn extend(\n"),
            (3476, "    pub(in crate::hir::validation) fn iter(&self) -> impl Iterator<Item = &String> {\n", "    pub(in crate::hir::validation::function_scope) fn iter(&self) -> impl Iterator<Item = &String> {\n"),
        ]),
        _ => return None,
    };
    assert_eq!(
        supplied, compiled,
        "complete current lifecycle visibility source"
    );
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), expected);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(&root.join(source), length, expected, 2_097_152).unwrap(),
        compiled
    );
    let mut before = compiled.to_owned();
    for &(at, current, previous) in edits.iter().rev() {
        assert_eq!(&before[at..at + current.len()], current);
        before.replace_range(at..at + current.len(), previous);
    }
    assert_eq!(before.len(), prior_length);
    assert_eq!(digest(&before), prior_sha);
    Some(before)
}

// Exact #913 and cfg(test) repair ingress; every immutable predecessor stays whole.
pub(super) fn batch_913_entry(source: &str, supplied: &str) -> Option<String> {
    let (compiled, length, expected, prior_length, prior_sha, edits):
        (&str, usize, &str, usize, &str, &[(&str, &str)]) = match source {
        "src/hir/validation/scope_checkpoint/marks.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/scope_checkpoint/marks.rs")), 14549, "2965bb2d4a43a3ca060b21db1a7ad11ca0af3a7e5061e48832366a833bca3c0e", 10578, "08b3d1a43078fcc73d6ba8514a093d5bdd4b7a5b00378bfd20a8a9a67b75e464", &[
            ("        // The original walker does nothing for these exact root shapes. This\n        // is only a source-move pass: full expression validation already has\n        // its own owner, and every propagating root retains the old driver.\n        // Fund the additional required-reservation argument/formal and bool\n        // return/receiving, plus the borrowed match scrutinee, before formation.\n        // The existing outbound Result and static refusal constructor retain\n        // their roles; no forwarding Result/helper or owned selector is added.\n        {\n            #[cfg(test)]\n            let _operation = crate::bounded_output::stage_debits::operation_guard(\n                crate::bounded_output::stage_debits::Operation::ScopeCheckpoint,\n            );\n            #[cfg(feature = \"unstable-workflow-profiling\")]\n            let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n                crate::bounded_output::exclusive_debits::Operation::ScopeCheckpoint,\n            );\n            if !crate::bounded_output::reserve_active_required(INERT_ROOT_CONTROLS) {\n                // Same allocation-free diagnostic and complete outbound Result\n                // as the original first Storage payer. It is selected before\n                // touching scope, including a pre-existing sticky refusal.\n                return Err(Diagnostic::io_static(\n                    \"SPX-H006\",\n                    \"ownership scope checkpoint exceeds the active validation budget\",\n                ));\n            }\n        }\n        match &expression.kind {\n            ResolvedExprKind::Closure { .. }\n            | ResolvedExprKind::FunctionReference { .. }\n            | ResolvedExprKind::Int(_)\n            | ResolvedExprKind::Int32(_)\n            | ResolvedExprKind::Char(_)\n            | ResolvedExprKind::Uint8(_)\n            | ResolvedExprKind::Usize(_)\n            | ResolvedExprKind::Float32(_)\n            | ResolvedExprKind::Float64(_)\n            | ResolvedExprKind::Bool(_)\n            | ResolvedExprKind::String(_)\n            | ResolvedExprKind::BorrowedUtf8Literal(_)\n            | ResolvedExprKind::ArrayU8(_)\n            | ResolvedExprKind::RepeatArrayU8 { .. }\n            | ResolvedExprKind::BorrowPlace { .. }\n            | ResolvedExprKind::ByteRange { .. }\n            | ResolvedExprKind::Call { .. }\n            | ResolvedExprKind::LiteralFormat { .. }\n            | ResolvedExprKind::VecFieldRead { .. }\n            | ResolvedExprKind::RowLoan { .. }\n            | ResolvedExprKind::NativeRustImportCall(_)\n            | ResolvedExprKind::HostCommandCall(_)\n            | ResolvedExprKind::Unary { .. }\n            | ResolvedExprKind::Binary { .. }\n            | ResolvedExprKind::ConstructRecord { .. }\n            | ResolvedExprKind::ConstructVariant { .. }\n            | ResolvedExprKind::Try { .. }\n            | ResolvedExprKind::TryOption { .. }\n            | ResolvedExprKind::Yield { .. }\n            | ResolvedExprKind::UpdateRecord { .. } => return Ok(()),\n            ResolvedExprKind::LexicalExit { .. }\n            | ResolvedExprKind::Invoke { .. }\n            | ResolvedExprKind::Place(_)\n            | ResolvedExprKind::Block { .. }\n            | ResolvedExprKind::VecConstructor { .. }\n            | ResolvedExprKind::If { .. }\n            | ResolvedExprKind::Match { .. }\n            | ResolvedExprKind::Project { .. }\n            | ResolvedExprKind::Upcast { .. } => {}\n        }\n", ""),
            ("        #[cfg(test)]\n        if tests::before_selected() {\n            return self.mark_value_sources_moved_before_inert_root(expression, scope);\n        }\n", ""),
            ("\n// One real primitive site, no new fallible wrapper or helper call. The bool\n// branch reads its original primitive result; match creates no bool predicate.\nconst INERT_ROOT_CONTROLS: usize = 2 * std::mem::size_of::<usize>()\n    + 3 * std::mem::size_of::<bool>()\n    + std::mem::size_of::<&ResolvedExprKind>();\n\n#[cfg(test)]\nmod before;\n#[cfg(test)]\nmod tests;\n", ""),
        ]),
        "src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/frontiers.rs")), 10608, "c82fb33f537bed7561a0267298de31c1577b00e3d2e7b6db1ec70b52b9deec7d", 10327, "4d9b686be048b16ae777c5990227c1afb97a166f583757dbddc11989b5ac1f01", &[
            ("        assert_eq!(\n            format!(\n                \"{:x}\",\n                crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))\n            ),\n            \"a176917b2f70a4ddff56230d92de0abd6575bfc3e6869b08612d5fff767499a1\"\n        );\n", "        assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), \"a176917b2f70a4ddff56230d92de0abd6575bfc3e6869b08612d5fff767499a1\");\n"),
            ("        assert_eq!(\n            restored\n                .matches(\"use super::*;\\nuse crate::bounded_output::with_limit_usage_refusal;\\n\")\n                .count(),\n            1\n        );\n", "        assert_eq!(restored.matches(\"use super::*;\\nuse crate::bounded_output::with_limit_usage_refusal;\\n\").count(), 1);\n"),
            ("    use crate::test_allocation_observer::{measure, Metrics};\n", "    use crate::test_allocation_observer::{Metrics, measure};\n"),
            ("    assert_eq!(\n        TRACE.with(|trace| trace\n            .borrow()\n            .as_ref()\n            .expect(\"active trace lease\")\n            .events\n            .len()),\n        4096\n    );\n", "    assert_eq!(TRACE.with(|trace| trace.borrow().events.len()), 4096);\n"),
            ("    let (empty, lost) =\n        super::comparison_owner::pair_scope_census::finish_empty_for_frontier_test();\n", "    let (empty, lost) = super::comparison_owner::pair_scope_census::finish_empty_for_frontier_test();\n"),
        ]),
        "src/hir/validation/frame_reuse/tests.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/tests.rs")), 13930, "11c3145c9b06cbde60f592bcf4def6403ea5b30e7f442381becd1c4324999de8", 13594, "2d874d35d5139662abf9a10e0fe45abbd666136349eae390220f134da2c8ebc9", &[
            ("            root.id = ExpressionId::from_owned(\"forged.trailing.identity\".to_owned());\n", "            root.id = ExpressionId::new(\"forged.trailing.identity\".to_owned());\n"),
            ("                    tail.id = ExpressionId::from_owned(\"forged.tail\".to_owned());\n", "                    tail.id = ExpressionId::new(\"forged.tail\".to_owned());\n"),
        ]),
        "src/hir/validation/lexical_exit.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/lexical_exit.rs")), 47750, "5610b10bf134638d5a30dbee84058895d40263d46ac07057f154d47ff8487c0e", 47202, "ed95841d0c0bb977e8120046a0182ff2dcef8731f443d0d1c17af2233ce15bc4", &[
            ("                                for error in &errors {\n                                    eprintln!(\"OPT904_DIAGNOSTIC phase=hir::resolve code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help);\n                                }\n", ""),
            ("                            eprintln!(\"OPT904_DIAGNOSTIC phase=hir::validate code={:?} severity={:?} message={:?} path={:?} span={:?} help={:?}\", error.code, error.severity, error.message, error.path, error.span, error.help);\n", ""),
        ]),
        "src/hir/validation/frame_reuse/source_inverse.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/frame_reuse/source_inverse.rs")), 10055, "1b9b6134ab855b3396f602949c7a681efe6bf840189712f326b22917a803ec82", 9708, "4432050d6538ee4469137dc7f147256591d256c569f1e49a0727e6a223fe7f38", &[
            ("    let observed;\n    let current = if source == \"src/hir/validation/lexical_exit.rs\" {\n        observed = crate::source_reflows::batch_913_entry(source, current)\n            .expect(\"complete diagnostic observer ingress\");\n        observed.as_str()\n    } else {\n        current\n    };\n", ""),
            ("        let predecessor = if matches!(path, \"src/hir/validation/frame_reuse/tests.rs\" | \"src/hir/validation/frame_reuse/source_inverse.rs\") {", "        let predecessor = if path == \"src/hir/validation/frame_reuse/tests.rs\" {"),
        ]),
        _ => return None,
    };
    let physical_compiled = compiled;
    let (closure_epoch, _) =
        crate::source_reflows_merge::Main44Fixture::closure_epoch(source, expected, compiled);
    let compiled = closure_epoch.as_ref();
    assert_eq!(supplied, physical_compiled, "complete current batch source");
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), expected);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &if source == "src/hir/validation/lexical_exit.rs" {
                root.join("e").join(expected)
            } else {
                root.join(source)
            },
            length,
            expected,
            2_097_152,
        )
        .unwrap(),
        compiled
    );
    if source == "src/hir/validation/scope_checkpoint/marks.rs" {
        for (path, child, child_length, child_sha) in [
            (
                "src/hir/validation/scope_checkpoint/marks/before.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/hir/validation/scope_checkpoint/marks/before.rs"
                )),
                10603,
                "429e331bf13d326e4ea5e26185f0d4d6945c2ad13dbadc5c457da65f4867140c",
            ),
            (
                "src/hir/validation/scope_checkpoint/marks/tests.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/hir/validation/scope_checkpoint/marks/tests.rs"
                )),
                20658,
                "7f07e20bdc0b2797f0c2f8abc2af9c5427fef9472bdf91fa87b5cfbb30f51994",
            ),
        ] {
            assert_eq!(child.len(), child_length);
            assert_eq!(digest(child), child_sha);
            assert_eq!(
                reader::read(&root.join(path), child_length, child_sha, 2_097_152).unwrap(),
                child
            );
        }
    }
    let mut before = compiled.to_owned();
    if source == "src/hir/validation/frame_reuse/tests.rs" {
        // Complete current executable-entry fixture transport, then the exact
        // unchanged #913 identifier inverse below. Neither old endpoint moves.
        assert_eq!(before.matches("    source.push_str(\"@id(\\\"reuse.main\\\") fn main()->i64 {f31(1)}\\n\");\n    program(&source)\n}\nfn decimal_program() -> ResolvedProgram {\n    // Preserve the entire library; executable resolution also needs an entry.\n    let source = format!(\n        \"{}\\n@id(\\\"reuse.decimal.main\\\") fn main()->i64 {{let value=add(\\\"1\\\",\\\"2\\\");string_len(value)}}\\n\",\n        include_str!(\"../../../../std/int-decimal/src/decimal.spx\")\n    );\n").count(), 1);
        before = before.replacen("    source.push_str(\"@id(\\\"reuse.main\\\") fn main()->i64 {f31(1)}\\n\");\n    program(&source)\n}\nfn decimal_program() -> ResolvedProgram {\n    // Preserve the entire library; executable resolution also needs an entry.\n    let source = format!(\n        \"{}\\n@id(\\\"reuse.decimal.main\\\") fn main()->i64 {{let value=add(\\\"1\\\",\\\"2\\\");string_len(value)}}\\n\",\n        include_str!(\"../../../../std/int-decimal/src/decimal.spx\")\n    );\n", "", 1);
        assert_eq!(before.matches("        decimal_program(),\n").count(), 1);
        before = before.replacen(
            "        decimal_program(),\n",
            "        program(include_str!(\"../../../../std/int-decimal/src/decimal.spx\")),\n",
            1,
        );
        assert_eq!(
            before
                .matches("        (\"decimal\", decimal_program(), true),\n")
                .count(),
            1
        );
        before = before.replacen("        (\"decimal\", decimal_program(), true),\n", "        (\"decimal\", program(include_str!(\"../../../../std/int-decimal/src/decimal.spx\")), true),\n", 1);
        const MIDDLE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb"
        ));
        assert_eq!(MIDDLE.len(), 13608);
        assert_eq!(
            digest(MIDDLE),
            "a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb"
        );
        assert_eq!(
            reader::read(
                &root.join("e/a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb"),
                13608,
                "a6968006a29a11d4fdcb2adc75c34c0142c2e51166636c77b1b285b5ffdbe6bb",
                2_097_152
            )
            .unwrap(),
            MIDDLE
        );
        assert_eq!(
            before, MIDDLE,
            "ENTIRE unchanged pre-fixture current source"
        );
    }
    for &(after, prior) in edits {
        assert_eq!(before.matches(after).count(), 1);
        before = before.replacen(after, prior, 1);
    }
    assert_eq!(before.len(), prior_length);
    assert_eq!(digest(&before), prior_sha);
    Some(before)
}

// Enter the immutable selected-path predecessor only through complete live images.
pub(super) fn compact_frontier_entry(source: &str, supplied: &str) -> Option<String> {
    let compiled = match source {
        "src/cleanup_plan/replay/path_join/selected.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/walk.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/walk.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/queries.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/queries.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/receiving.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/receiving.rs"
        )),
        "src/cleanup_plan/replay/path_join/selected/tests/mod.rs" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/path_join/selected/tests/mod.rs"
        )),
        _ => return None,
    };
    assert_eq!(
        supplied, compiled,
        "complete current compact frontier source"
    );
    let encoded = include_str!("compact_frontier_inverse.json");
    assert_eq!(encoded.len(), 30712);
    assert_eq!(
        digest(encoded),
        "f3c2c90ae071a1e777dbb007ef1ac80adccdfc140892177cabd4b022e0778645"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/compact_frontier_inverse.json"),
            encoded.len(),
            &digest(encoded),
            2_097_152
        )
        .unwrap(),
        encoded
    );
    let binding: serde_json::Value = serde_json::from_str(encoded).unwrap();
    assert_eq!(
        binding["schema"],
        "semaprax.compact-frontier-whole-source-ingress.v1"
    );
    for (path, child) in [
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/finish.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/finish.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/mod.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/mod.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/queries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/queries.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/receiving.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/receiving.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/before_frontier/walk.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/before_frontier/walk.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/boxed_flow.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/boxed_flow.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/full_callers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/full_callers.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before/ordinary.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/lexical_before/ordinary.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/frontier_tests/scalar_before.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier_tests/scalar_before.rs"))),
        ("src/cleanup_plan/replay/cfg_bounds/tests/old.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/cfg_bounds/tests/old.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/whole.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/whole.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/renewal.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/renewal.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/session.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/session.rs"))),
        ("src/cleanup_plan/replay/path_join/selected/tests/strings.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/tests/strings.rs"))),
    ] {
        let pin = &binding["children"][path];
        let length = usize::try_from(pin["length"].as_u64().unwrap()).unwrap();
        let expected = pin["sha256"].as_str().unwrap();
        assert_eq!(child.len(), length);
        assert_eq!(digest(child), expected);
        assert_eq!(reader::read(&root.join(path), length, expected, 2_097_152).unwrap(), child);
    }
    let row = &binding["rows"][source];
    let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
    let expected = row["current_sha256"].as_str().unwrap();
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), expected);
    assert_eq!(
        reader::read(&root.join(source), length, expected, 2_097_152).unwrap(),
        compiled
    );
    let mut before = compiled.as_bytes().to_vec();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(
            before.get(start..end).unwrap(),
            edit["current"].as_str().unwrap().as_bytes()
        );
        before.splice(
            start..end,
            edit["before"].as_str().unwrap().as_bytes().iter().copied(),
        );
    }
    let before = String::from_utf8(before).unwrap();
    assert_eq!(before.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(digest(&before), row["before_sha256"].as_str().unwrap());
    Some(before)
}

pub(super) fn next_custody_before(owner: &str, source: &str, supplied: &str) -> String {
    if (owner, source) == ("modular916", "src/workspace_graph/expected_projection/held_projection.rs") && supplied.len() == 23069 && digest(supplied) == "19dd7ed0772d873ba5d7854b3c8c16c298009c2c0454d12b538524cbc7b56e29" { let complete = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/held_projection.rs")); let prior = next_custody_before("production7834", source, complete); assert_eq!(supplied, prior, "exact named production7834 output at modular916 ingress"); return prior; }
    if (owner, source) == ("modular916", "src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs") && supplied.len() == 14772 && digest(supplied) == "fdbdd20cb63d8bf8f32e8244e96be2526df769614f1ed325c9b515d7fd67e722" { let complete = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs")); let prior = next_custody_before("production7834", source, complete); assert_eq!(supplied, prior, "exact named production7834 output at modular916 ingress"); return prior; }
    if owner == "constructor903" { return constructor903_before(source, supplied).unwrap_or_else(|| supplied.to_owned()); }
    const PACKET: &str = include_str!("next_custody_inverse.json");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_inverse.json"),
            PACKET.len(),
            &digest(PACKET),
            2_097_152
        )
        .unwrap(),
        PACKET
    );
    let packet: serde_json::Value = serde_json::from_str(PACKET).unwrap();
    let owners = packet["owners"].as_array().unwrap();
    for (index, row) in owners.iter().enumerate() {
        assert!(
            !owners[..index]
                .iter()
                .any(|prior| prior["owner"] == row["owner"] && prior["source"] == row["source"]),
            "duplicate finite source pair"
        );
    }
    let row = packet["owners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["owner"] == owner && row["source"] == source);
    let Some(row) = row else {
        return constructor903_before(source, supplied).unwrap_or_else(|| supplied.to_owned());
    };
    let compiled = match source {
        "docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md")),
        "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py")),
        "src/bounded_output/classifier_string.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/classifier_string.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/producer_joint_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/producer_joint_tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/dag_storage/flat_fanout.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/dag_storage/flat_fanout.rs")),
        "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_fragments_v3.rs")),
        "src/source_reflows_merge/main_e177_overlay_pins_v10.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_pins_v10.rs")),
        "src/source_reflows_merge/main_e177_overlay_v2.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/main_e177_overlay_v2.rs")),
        "src/workspace_graph/expected_projection/call_carriers_inverse.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/call_carriers_inverse.rs")),
        "src/workspace_graph/expected_projection/held_projection.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/held_projection.rs")),
        "src/workspace_graph/expected_projection/private_layout.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout.rs")),
        "src/workspace_graph/expected_projection/private_layout/names.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names.rs")),
        "src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs")),
        "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/controls_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reuse_tests.rs")),
        "src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/shared_local_callees/reused_controls.rs")),
        "tests/workspace/semantic_graph/source_inventory.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),
        _ => unreachable!("closed finite source entry"),
    };
    // Authenticate the actual compiled/physical channel before any finite epoch projection.
    let physical = reader::read(
        &root.join(source),
        compiled.len(),
        &digest(compiled),
        2_097_152,
    )
    .unwrap();
    assert_eq!(
        physical, compiled,
        "complete actual compiled source channel"
    );
    let constructor = constructor903_before(source, compiled); let acquired = constructor.filter(|before| supplied == compiled || supplied == before.as_str())
        .or_else(|| (supplied.len() as u64 == row["current_length"].as_u64().unwrap() && digest(supplied) == row["current_sha256"].as_str().unwrap()).then(|| supplied.to_owned()))
        .or_else(|| next_emission_before(owner, source, supplied, compiled))
        .or_else(|| next_custody_front(owner, source, supplied, compiled, row));
    if acquired.is_none() {
        assert_eq!(supplied, compiled, "complete compiled source channel");
    }
    let compiled = acquired.as_deref().unwrap_or(compiled);
    assert_eq!(compiled.len() as u64, row["current_length"]);
    assert_eq!(digest(compiled), row["current_sha256"]);
    let mut prior_end = 0;
    for edit in row["edits"].as_array().unwrap() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(prior_end <= start && start <= end && end <= compiled.len());
        assert!(compiled.is_char_boundary(start) && compiled.is_char_boundary(end));
        prior_end = end;
    }
    let mut restored = compiled.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(&restored[start..end], edit["after"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    assert_eq!(restored, row["before_text"].as_str().unwrap());
    assert_eq!(restored.len() as u64, row["before_length"]);
    assert_eq!(digest(&restored), row["before_sha256"]);
    restored
}

// Only these four exact current fronts may precede the immutable26 input epochs.
fn next_custody_front(
    owner: &str,
    source: &str,
    supplied: &str,
    actual: &str,
    old: &serde_json::Value,
) -> Option<String> {
    const FRONT: &str = include_str!("next_custody_fronts.json");
    const PAIRS: [(&str, &str); 4] = [
        (
            "prelude286",
            "tests/workspace/semantic_graph/source_inventory.rs",
        ),
        (
            "v12-root",
            "src/source_reflows_merge/main_e177_overlay_v2.rs",
        ),
        (
            "v12-python",
            "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
        ),
        (
            "main295-v12-fragments",
            "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs",
        ),
    ];
    let Some(index) = PAIRS.iter().position(|pair| *pair == (owner, source)) else {
        return None;
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_fronts.json"),
            FRONT.len(),
            &digest(FRONT),
            2_097_152
        )
        .unwrap(),
        FRONT
    );
    let packet: serde_json::Value = serde_json::from_str(FRONT).unwrap();
    assert_eq!(packet["schema"], "semaprax.next-custody-four-fronts.v1");
    assert_eq!(
        packet["original_packet"]["length"],
        include_str!("next_custody_inverse.json").len()
    );
    assert_eq!(
        packet["original_packet"]["sha256"],
        digest(include_str!("next_custody_inverse.json"))
    );
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), PAIRS.len());
    for (row, pair) in rows.iter().zip(PAIRS) {
        assert_eq!(row["owner"], pair.0);
        assert_eq!(row["source"], pair.1);
    }
    let row = &rows[index];
    assert_eq!(actual.len() as u64, row["length"]);
    assert_eq!(digest(actual), row["sha256"]);
    fn project(actual: &str, stage: &serde_json::Value) -> String {
        let mut previous = 0;
        for edit in stage["edits"].as_array().unwrap() {
            let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
            assert!(previous <= start && start <= end && end <= actual.len());
            assert!(actual.is_char_boundary(start) && actual.is_char_boundary(end));
            assert_eq!(&actual[start..end], edit["after"].as_str().unwrap());
            previous = end;
        }
        let mut result = actual.to_owned();
        for edit in stage["edits"].as_array().unwrap().iter().rev() {
            let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
            result.replace_range(start..end, edit["before"].as_str().unwrap());
        }
        assert_eq!(result.len() as u64, stage["length"]);
        assert_eq!(digest(&result), stage["sha256"]);
        result
    }
    let middle = (!row["middle"].is_null()).then(|| project(actual, &row["middle"]));
    assert!(
        supplied == actual || middle.as_deref() == Some(supplied),
        "only the same actual source or its exact bound V13 intermediate"
    );
    assert_eq!(row["target"]["length"], old["current_length"]);
    assert_eq!(row["target"]["sha256"], old["current_sha256"]);
    let restored = project(actual, &row["target"]);
    assert_eq!(
        reader::read(&root.join(source), actual.len(), &digest(actual), 2_097_152).unwrap(),
        actual
    );
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_fronts.json"),
            FRONT.len(),
            &digest(FRONT),
            2_097_152
        )
        .unwrap(),
        FRONT
    );
    let original = include_str!("next_custody_inverse.json");
    assert_eq!(
        reader::read(
            &root.join("src/source_reflows/next_custody_inverse.json"),
            original.len(),
            &digest(original),
            2_097_152
        )
        .unwrap(),
        original
    );
    Some(restored)
}

// Only the two complete OPT925 test successors may acquire their fixed b23
// source epochs before the entire unchanged original26 inverse is applied.
fn next_emission_before(owner: &str, source: &str, supplied: &str, actual: &str) -> Option<String> {
    if owner != "cfgtest-repair" {
        return None;
    }
    // Authenticate the exact runtime-repaired raw source before its unchanged925 input view.
    let raw_actual=actual; let raw_supplied=supplied;
    let prefix: Option<(usize,&str,&[(usize,&str,&str)])> = match source {
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => Some((17977,"f483721064e9e81524fdf22c6b41e07d045edfab0d9f4d0627e35e8c470bc33e",&[(6817,"","                let mut work = SkeletonWork::new(&program, function, &mut budget);\n"),(6852,"                    let mut work = SkeletonWork::with_index(&program, function, &mut budget, None)?;\n",""),(8179,"                        budget\n","                        work.budget\n"),(8377,"","                drop(work);\n")])),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => Some((35081,"5d34472a0f59197c69d691fd9cc9b2dc90424ad09c74287b31ea42b2df263eb6",&[(1549,"    let raw_source = include_str!(\"../../../../replay.rs\");\n    assert_eq!(std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"src/cleanup_plan/replay.rs\")).unwrap(), raw_source.as_bytes());\n    let digest = |bytes: &[u8]| { use sha2::{Digest, Sha256}; format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(bytes))) };\n    assert_eq!(raw_source.len(), 327551);\n    assert_eq!(digest(raw_source.as_bytes()), \"751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5\");\n    let mut source = raw_source.to_owned();\n    let prefix_edits: &[(usize, &str, &str)] = &[\n        (1097, \"mod plan_prefix;\\n\", \"\"),\n        (108404, \"#[rustfmt::skip]\\n\", \"\"),\n        (112083, \"        ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } if { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } => !plan_prefix::equal_paths(function, budget, &mut expected, &mut actual)?,\\n\", \"\"),\n        (116424, \"        budget.reserve_bytes(function, skeleton_work::query_proof::HEADER, \\\"skeleton query neutral slot receivers\\\")?;\\n        SkeletonWork { queries: skeleton_work::query_proof::Slot::new(function, budget),\\n\", \"        SkeletonWork {\\n\"),\n        (279166, \"#[rustfmt::skip]\\n\", \"\"),\n        (279316, \"    budget.reserve_bytes(function, plan_prefix::QUERY_CONTROLS, \\\"PLAN prefix eligibility receivers\\\")?;\\n    if matches!(budget.payer, AllocationPayer::LegacyUnobserved) && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } { return plan_prefix::build(function, budget); }\\n\", \"\"),\n        (289554, \"                compact_path::plan_selection!(budget, function);\\n\", \"                budget.reserve_bytes(\\n                    function,\\n                    branch_clone::direct::PLAN_SMALL_SELECTION_CONTROLS,\\n                    \\\"plan final-output selection controls\\\",\\n                )?;\\n\"),\n        (289948, \"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\", \"                        budget.reserve_bytes(\\n\"),\n        (290070, \"                            branch_clone::direct::plan_compact_roles::<\\n\", \"                            branch_clone::direct::compact_roles::<\\n\"),\n        (290357, \"                        ));\\n\", \"                        )?;\\n\"),\n        (291020, \"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\", \"                        budget.reserve_bytes(\\n\"),\n        (291142, \"                            branch_clone::plan_compact_roles::<std::rc::Rc<SkeletonObservation>>(),\\n\", \"                            branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>(),\\n\"),\n        (291305, \"                        ));\\n\", \"                        )?;\\n\"),\n        (291645, \"                        physical_budget::compact_try!(budget.reserve_compact_bytes(\\n\", \"                        budget.reserve_bytes(\\n\"),\n        (291767, \"                            branch_clone::direct::plan_compact_roles::<\\n\", \"                            branch_clone::direct::compact_roles::<\\n\"),\n        (292054, \"                        ));\\n\", \"                        )?;\\n\"),\n        (292911, \"                    compact_path::plan_move!(budget, function);\\n                    compact_path::plan_work!(\\n                        budget,\\n\", \"                    budget.reserve_bytes(\\n\"),\n        (293087, \"                        \\\"cleanup-plan skeleton branch clone\\\"\\n                    );\\n\", \"                        final_output::PLAN_MOVE_CONTROLS,\\n                        \\\"plan final-output move controls\\\",\\n                    )?;\\n                    budget.charge_skeleton(function, 1, \\\"cleanup-plan skeleton branch clone\\\")?;\\n\"),\n        (293480, \"                    compact_path::plan_work!(budget, function, \\\"plan branch observation push\\\");\\n\", \"                    budget.charge_skeleton(function, 1, \\\"plan branch observation push\\\")?;\\n\"),\n        (293629, \"                    compact_path::plan_auth!(budget, function);\\n                    compact_path::plan_conversion!(budget, function, observation);\\n\", \"                    budget.reserve_bytes(function, 0, \\\"plan branch observation push\\\")?;\\n                    budget.reserve_bytes(\\n                        function,\\n                        physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(\\n                            &observation,\\n                        ),\\n                        \\\"plan branch observation push\\\",\\n                    )?;\\n\"),\n        (297849, \"                        branch_clone::short_plan::produce!(\\n                            budget,\\n                            function,\\n                            observations,\\n                            observation,\\n                            edge,\\n                            queue\\n                        );\\n\", \"\"),\n        (324946, \"#[cfg(test)]\\nmod workspace_oracle;\\n\", \"\\n\"),\n        (325059, \"#[cfg(test)]\\npub(crate) use workspace_oracle::with_financed_workspace_test_oracles;\\n\", \"\\n\"),\n        (325167, \"\", \"\\n\"),\n    ];\n    for &(at, current, previous) in prefix_edits.iter().rev() {\n        assert_eq!(source.get(at..at + current.len()), Some(current));\n        source.replace_range(at..at + current.len(), previous);\n    }\n    assert_eq!(source.len(), 326623);\n    assert_eq!(digest(source.as_bytes()), \"027903ce7f76fd0a9883e2736adfa1214bdb1e687303d939c0e40751c6130320\");\n    assert_eq!(std::fs::read(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"src/cleanup_plan/replay.rs\")).unwrap(), raw_source.as_bytes());\n","    let mut source = include_str!(\"../../../../replay.rs\").to_owned();\n"),(30433,"\n// Exact pre-sticky validation participates in this full caller boundary.\nfn joint_frozen_immediate(\n    program: &ResolvedProgram,\n    function: &ResolvedFunction,\n    budget: &mut ReplayBudget,\n) -> Result<(), Diagnostic> {\n    let mut expected = with_frozen_immediate(|| hir_skeleton_paths(program, function, budget))?;\n    budget.reserve_bytes(\n        function,\n        sequence::ENTRY_CALL,\n        \"whole HIR complete content sort\",\n    )?;\n    sequence::sort(&mut expected, function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::PRODUCE_CALL,\n        \"whole independent PLAN producer\",\n    )?;\n    let produced = view::produce(function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::COMPARE_CALL,\n        \"whole independent history comparison\",\n    )?;\n    view::compare(produced, &expected, budget)\n}\n\n#[test]\nfn retained_fork_true_frozen_immediate_whole_net_and_cold_costs_are_disclosed() {\n    let mut improvement = false;\n    for program in [fixture(), nested_call_program()] {\n        for function in &program.functions {\n            let run = |immediate, limit, floor| {\n                with_limit_usage_refusal(limit, || {\n                    measure(|| {\n                        assert!(set_active_floor(floor));\n                        let mut budget = ReplayBudget::new();\n                        let result = reserve_program_skeleton_work(\n                            &program,\n                            std::iter::once(function),\n                            &mut budget,\n                        )\n                        .and_then(|_reserved| {\n                            if immediate {\n                                joint_frozen_immediate(&program, function, &mut budget)\n                            } else {\n                                joint(&program, function, &mut budget)\n                            }\n                        });\n                        let work = (\n                            budget.skeleton_spent,\n                            budget.skeleton_remaining,\n                            budget.remaining,\n                        );\n                        if result.is_err() {\n                            assert!(budget\n                                .reserve_bytes(function, 0, \"retained whole fork sticky zero\")\n                                .is_err());\n                        }\n                        drop(budget);\n                        result.map(|()| work).map_err(|error| {\n                            let code = error.code;\n                            drop(error);\n                            code\n                        })\n                    })\n                })\n            };\n            let ((before, old_heap), old_refused, old_paid, _) = run(true, usize::MAX, 0);\n            let ((after, new_heap), new_refused, new_paid, _) = run(false, usize::MAX, 0);\n            let old_work = before.unwrap();\n            let new_work = after.unwrap();\n            assert_eq!(old_work, new_work);\n            assert!(!old_refused && !new_refused);\n            for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {\n                assert!(!heap.invalid && heap.live == 0);\n                assert_eq!(heap.allocated, heap.freed);\n                assert!(heap.allocated <= paid);\n            }\n            improvement |= new_paid < old_paid;\n            eprintln!(\"925 true_immediate_whole function={} old_paid={} new_paid={} old_heap={:?} new_heap={:?} work={:?}\",\n                function.id.as_str(), old_paid, new_paid, old_heap, new_heap, new_work);\n            // Exact/short/floor/zero are independently determined for each full\n            // producer. A changed net invoice never becomes a cap increase.\n            for (immediate, paid) in [(true, old_paid), (false, new_paid)] {\n                for (limit, floor, success) in [\n                    (paid, 0, true),\n                    (paid - 1, 0, false),\n                    (0, 0, false),\n                    (paid, paid, false),\n                ] {\n                    let ((result, heap), refused, _, first) = run(immediate, limit, floor);\n                    assert_eq!(result.is_ok(), success);\n                    assert!(!heap.invalid && heap.live == 0);\n                    assert_eq!(heap.allocated, heap.freed);\n                    if !success {\n                        assert!(refused && first.is_some());\n                        assert_eq!(result.unwrap_err(), \"SPX-H006\");\n                    }\n                }\n            }\n        }\n    }\n    assert!(\n        improvement,\n        \"complete composed receiver must earn a positive net invoice\"\n    );\n}\n","")])),
        _ => None,
    };
    let acquired=prefix.map(|(length,sha,edits)| { assert_eq!(raw_supplied,raw_actual); assert_eq!(raw_actual.len(),length); assert_eq!(digest(raw_actual),sha); let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")); assert_eq!(reader::read(&root.join(source),length,sha,2_097_152).unwrap(),raw_actual); let mut prior=raw_actual.to_owned(); for &(at,post,pre)in edits.iter().rev(){assert_eq!(prior.get(at..at+post.len()),Some(post));prior.replace_range(at..at+post.len(),pre);} prior });
    let actual=acquired.as_deref().unwrap_or(actual); let supplied=if acquired.is_some(){actual}else{supplied};
    let (length, sha, target, target_sha, edits): (usize, &str, &str, &str, &[(usize, &str, &str)]) = match source {
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/tests.rs" => (17992, "86eb3c62bd63b7b04a30adc265bde874e9faf1910cdd538d9d9e6c0265180fa0", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/5534698ffa877cfeff74d028d5cd23ef13e100c0a361a14245512f9530e23745")), "5534698ffa877cfeff74d028d5cd23ef13e100c0a361a14245512f9530e23745", &[(10327, "\n#[test]\nfn retained_fork_receivers_preserve_immediate_emitter_and_singleton_allocation() {\n    let program = fixture();\n    let function = &program.functions[0];\n    with_limit_usage_refusal(usize::MAX, || {\n        for len in [0, 1, 7, 127] {\n            for count in [0, 1, 2, 5] {\n                let mut old_budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n                let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);\n                let (prefixes, suffixes) = inputs(&mut old_work, len, count);\n                let old = with_immediate(|| call(prefixes, &suffixes, &mut old_work)).unwrap();\n                let old_spent = old_work.budget.skeleton_spent;\n                let mut new_budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n                let mut new_work = SkeletonWork::new(&program, function, &mut new_budget);\n                let (prefixes, suffixes) = inputs(&mut new_work, len, count);\n                let pointer = prefixes.as_ptr();\n                let capacity = prefixes.capacity();\n                let actual = call(prefixes, &suffixes, &mut new_work).unwrap();\n                assert_eq!(old_spent, new_work.budget.skeleton_spent);\n                if count == 1 {\n                    assert_eq!(actual.as_ptr(), pointer);\n                    assert_eq!(actual.capacity(), capacity);\n                }\n                assert!(equal_paths(&actual, &old, &mut new_work));\n            }\n        }\n        // A real shared singleton, both stopped states and a flat empty suffix\n        // all use the same cardinality proof; no empty input is manufactured.\n        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n        let mut work = SkeletonWork::new(&program, function, &mut budget);\n        let (prefixes, suffixes) = inputs(&mut work, 7, 2);\n        let shared = with_immediate(|| call(prefixes, &suffixes, &mut work)).unwrap();\n        let (prefixes, _) = inputs(&mut work, 3, 0);\n        let mut prefixes = prefixes;\n        prefixes[1].failed = false;\n        prefixes[1].residual = true;\n        let pointer = prefixes.as_ptr();\n        let actual = call(prefixes, &shared[..1], &mut work).unwrap();\n        assert_eq!(actual.as_ptr(), pointer);\n        let (mut prefixes, _) = inputs(&mut work, 3, 0);\n        prefixes[1].failed = false;\n        prefixes[1].residual = true;\n        let old = with_immediate(|| call(prefixes, &shared[..1], &mut work)).unwrap();\n        assert!(equal_paths(&actual, &old, &mut work));\n    });\n}\n\n#[test]\nfn retained_fork_immediate_work_cuts_and_all_diagnostic_fields_match() {\n    let program = fixture();\n    let function = &program.functions[0];\n    // Cuts apply after the same complete paid setup. Both independent emitters\n    // retain the held skeleton loan, including stopped and final-output events.\n    for count in [0, 1, 2, 5] {\n        let attempt = |cut, immediate| {\n            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n            let mut work = SkeletonWork::new(&program, function, &mut budget);\n            let (prefixes, suffixes) = inputs(&mut work, 7, count);\n            work.budget.skeleton_remaining = cut;\n            let before = work.budget.skeleton_spent;\n            let result = if immediate {\n                with_immediate(|| call(prefixes, &suffixes, &mut work))\n            } else {\n                call(prefixes, &suffixes, &mut work)\n            }\n            .map(drop);\n            (result, work.budget.skeleton_spent - before)\n        };\n        for cut in 0..32 {\n            let (old, old_work) = attempt(cut, true);\n            let (new, new_work) = attempt(cut, false);\n            assert_eq!(old_work, new_work);\n            match (old, new) {\n                (Ok(()), Ok(())) => {}\n                (Err(a), Err(b)) => assert_eq!(\n                    (a.code, a.severity, a.message, a.path, a.span, a.help),\n                    (b.code, b.severity, b.message, b.path, b.span, b.help)\n                ),\n                _ => panic!(\"immediate held-work refusal frontier changed\"),\n            }\n        }\n    }\n}\n\n#[test]\nfn retained_fork_keeps_the_entire_immediate_parent_and_old_producer() {\n    let current = include_str!(\"../emission.rs\");\n    let before = retained::before_emission(current);\n    let body = |s: &str| {\n        let begin = s\n            .find(\"pub(in crate::cleanup_plan::replay) fn emit_prepaid(\")\n            .unwrap();\n        let end = s[begin..]\n            .find(\"\\n// The original caller's actual-payer pattern\")\n            .unwrap()\n            + begin;\n        s[begin..end].to_owned()\n    };\n    assert_eq!(body(current), body(&before));\n    // Both the original public receiver invoice and the complete STEP inventory\n    // stay compiled with the old body; no candidate discounts reach that arm.\n    let banks = |s: &str| {\n        let begin = s\n            .find(\"pub(in crate::cleanup_plan::replay) const ENTRY:\")\n            .unwrap();\n        let end = s.find(\"const FREEZE:\").unwrap();\n        s[begin..end].to_owned()\n    };\n    assert_eq!(banks(current), banks(&before));\n}\n\n#[test]\nfn retained_fork_cold_empty_tiny_and_repeated_packets_disclose_actual_costs() {\n    let program = fixture();\n    let function = &program.functions[0];\n    // Direct emitter boundary: deliberately includes cold/unsupported-empty\n    // shapes that the shipping dispatcher can avoid. No app inference follows.\n    for (empty, len, count, repeats) in [\n        (true, 0, 0, 1),\n        (false, 0, 1, 1),\n        (false, 1, 2, 1),\n        (false, 127, 1, 3),\n        (false, 127, 5, 3),\n    ] {\n        let run = |immediate| {\n            with_limit_usage_refusal(usize::MAX, || {\n                measure(|| {\n                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);\n                    let mut work = SkeletonWork::new(&program, function, &mut budget);\n                    let result = (|| {\n                        let (mut paths, suffixes) = inputs(&mut work, len, count);\n                        if empty {\n                            paths.clear();\n                        }\n                        for _ in 0..repeats {\n                            paths = if immediate {\n                                with_immediate(|| call(paths, &suffixes, &mut work))\n                            } else {\n                                call(paths, &suffixes, &mut work)\n                            }?;\n                        }\n                        drop(paths);\n                        drop(suffixes);\n                        Ok::<_, Diagnostic>(())\n                    })();\n                    let work_used = work.budget.skeleton_spent;\n                    drop(work);\n                    drop(budget);\n                    result.map(|()| work_used).map_err(|error| {\n                        let code = error.code;\n                        drop(error);\n                        code\n                    })\n                })\n            })\n        };\n        let ((old, old_heap), old_refused, old_paid, _) = run(true);\n        let ((new, new_heap), new_refused, new_paid, _) = run(false);\n        assert_eq!(old.unwrap(), new.unwrap());\n        assert!(!old_refused && !new_refused);\n        for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {\n            assert!(!heap.invalid && heap.live == 0);\n            assert_eq!(heap.allocated, heap.freed);\n            assert!(heap.allocated <= paid);\n        }\n        eprintln!(\"925 direct empty={} len={} suffixes={} repeats={} old_paid={} new_paid={} old_heap={:?} new_heap={:?}\",\n            empty,len,count,repeats,old_paid,new_paid,old_heap,new_heap);\n    }\n}\n", "")]),
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/whole_tests.rs" => (24809, "40926dadafd6738359bd52c3a7ba3fb1f9dbc27beebbf19b4a4120aaf9cb405b", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/67b827f40e61c90022e48c1c234b9f3f667dd5dc0ab799c57ee249bfbbfae93d")), "67b827f40e61c90022e48c1c234b9f3f667dd5dc0ab799c57ee249bfbbfae93d", &[(20122, "\n// Complete immediate-b23 producer; the sort, independent PLAN producer,\n// comparison and owner destruction are identical to joint's whole boundary.\nfn joint_immediate(\n    program: &ResolvedProgram,\n    function: &ResolvedFunction,\n    budget: &mut ReplayBudget,\n) -> Result<(), Diagnostic> {\n    let mut expected = with_immediate(|| hir_skeleton_paths(program, function, budget))?;\n    budget.reserve_bytes(\n        function,\n        sequence::ENTRY_CALL,\n        \"whole HIR complete content sort\",\n    )?;\n    sequence::sort(&mut expected, function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::PRODUCE_CALL,\n        \"whole independent PLAN producer\",\n    )?;\n    let produced = view::produce(function, budget)?;\n    budget.reserve_bytes(\n        function,\n        view::COMPARE_CALL,\n        \"whole independent history comparison\",\n    )?;\n    view::compare(produced, &expected, budget)\n}\n\n#[test]\nfn retained_fork_whole_hir_sort_plan_net_invoice_and_cold_tiny_are_disclosed() {\n    let mut improvement = false;\n    for program in [fixture(), nested_call_program()] {\n        for function in &program.functions {\n            let run = |immediate, limit, floor| {\n                with_limit_usage_refusal(limit, || {\n                    measure(|| {\n                        assert!(set_active_floor(floor));\n                        let mut budget = ReplayBudget::new();\n                        let result = reserve_program_skeleton_work(\n                            &program,\n                            std::iter::once(function),\n                            &mut budget,\n                        )\n                        .and_then(|_reserved| {\n                            if immediate {\n                                joint_immediate(&program, function, &mut budget)\n                            } else {\n                                joint(&program, function, &mut budget)\n                            }\n                        });\n                        let work = (\n                            budget.skeleton_spent,\n                            budget.skeleton_remaining,\n                            budget.remaining,\n                        );\n                        if result.is_err() {\n                            assert!(budget\n                                .reserve_bytes(function, 0, \"retained whole fork sticky zero\")\n                                .is_err());\n                        }\n                        drop(budget);\n                        result.map(|()| work).map_err(|error| {\n                            let code = error.code;\n                            drop(error);\n                            code\n                        })\n                    })\n                })\n            };\n            let ((before, old_heap), old_refused, old_paid, _) = run(true, usize::MAX, 0);\n            let ((after, new_heap), new_refused, new_paid, _) = run(false, usize::MAX, 0);\n            let old_work = before.unwrap();\n            let new_work = after.unwrap();\n            assert_eq!(old_work, new_work);\n            assert!(!old_refused && !new_refused);\n            for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {\n                assert!(!heap.invalid && heap.live == 0);\n                assert_eq!(heap.allocated, heap.freed);\n                assert!(heap.allocated <= paid);\n            }\n            improvement |= new_paid < old_paid;\n            eprintln!(\"925 whole function={} old_paid={} new_paid={} old_heap={:?} new_heap={:?} work={:?}\",\n                function.id.as_str(), old_paid, new_paid, old_heap, new_heap, new_work);\n            // Exact/short/floor/zero are independently determined for each full\n            // producer. A changed net invoice never becomes a cap increase.\n            for (immediate, paid) in [(true, old_paid), (false, new_paid)] {\n                for (limit, floor, success) in [\n                    (paid, 0, true),\n                    (paid - 1, 0, false),\n                    (0, 0, false),\n                    (paid, paid, false),\n                ] {\n                    let ((result, heap), refused, _, first) = run(immediate, limit, floor);\n                    assert_eq!(result.is_ok(), success);\n                    assert!(!heap.invalid && heap.live == 0);\n                    assert_eq!(heap.allocated, heap.freed);\n                    if !success {\n                        assert!(refused && first.is_some());\n                        assert_eq!(result.unwrap_err(), \"SPX-H006\");\n                    }\n                }\n            }\n        }\n    }\n    assert!(\n        improvement,\n        \"complete composed receiver must earn a positive net invoice\"\n    );\n}\n", "")]),
        _ => return None,
    };
    assert_eq!(supplied, actual, "complete actual OPT925 source channel");
    assert_eq!(actual.len(), length);
    assert_eq!(digest(actual), sha);
    assert_eq!(digest(target), target_sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(
            &root.join(format!("e/{target_sha}")),
            target.len(),
            target_sha,
            2_097_152
        )
        .unwrap(),
        target
    );
    let mut restored = actual.to_owned();
    for (at, post, prior) in edits.iter().rev() {
        let end = at.checked_add(post.len()).unwrap();
        assert!(*at <= end && end <= restored.len());
        assert!(restored.is_char_boundary(*at) && restored.is_char_boundary(end));
        assert_eq!(&restored[*at..end], *post);
        restored.replace_range(*at..end, prior);
    }
    assert_eq!(restored, target, "entire fixed b23 test source");
    if acquired.is_some(){assert_eq!(reader::read(&root.join(source),raw_actual.len(),&digest(raw_actual),2_097_152).unwrap(),raw_actual);}
    Some(restored)
}

// Exact full physical source and named pre-903 view; every old route stays strict.
#[rustfmt::skip]
fn constructor903_before(source: &str, supplied: &str) -> Option<String> {
    const FRONT: &str = include_str!("next_custody_fronts.json");
    let packet: serde_json::Value = serde_json::from_str(FRONT).unwrap();
    let Some(row) = packet["constructor_retention_903"]["records"].get(source) else { return None; };
    assert_eq!(packet["constructor_retention_903"]["schema"], "semaprax.constructor-retention-903-source-fronts.v1");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let acquire = |path: &str, length: usize, sha: &str| reader::read(&root.join(path), length, sha, 2_097_152).unwrap();
    assert_eq!(acquire("src/source_reflows/next_custody_fronts.json", FRONT.len(), &digest(FRONT)), FRONT);
    let actual = acquire(source, row["current"]["length"].as_u64().unwrap().try_into().unwrap(), row["current"]["sha256"].as_str().unwrap());
    let mut previous = 0; for e in row["edits"].as_array().unwrap() { let a: usize = e["start"].as_u64().unwrap().try_into().unwrap(); let b: usize = e["end"].as_u64().unwrap().try_into().unwrap(); assert!(previous <= a && a <= b && b <= actual.len() && actual.is_char_boundary(a) && actual.is_char_boundary(b)); assert_eq!(&actual[a..b], e["after"].as_str().unwrap()); previous = b; }
    let mut restored = actual.clone(); for e in row["edits"].as_array().unwrap().iter().rev() { restored.replace_range::<std::ops::Range<usize>>(e["start"].as_u64().unwrap().try_into().unwrap()..e["end"].as_u64().unwrap().try_into().unwrap(), e["before"].as_str().unwrap()); }
    assert_eq!(restored, row["before_text"].as_str().unwrap()); assert_eq!(restored.len() as u64, row["target"]["length"]); assert_eq!(digest(&restored), row["target"]["sha256"]);
    assert!(supplied == actual || supplied == restored, "only complete physical or exact named pre-903 input view");
    assert_eq!(acquire(source, actual.len(), &digest(&actual)), actual); assert_eq!(acquire("src/source_reflows/next_custody_fronts.json", FRONT.len(), &digest(FRONT)), FRONT); Some(restored)
}
