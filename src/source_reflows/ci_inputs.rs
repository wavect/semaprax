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
// These two complete predecessor stages belong to this fixed CI inventory.
// Authenticate the actual compiled/physical/current row before selecting the
// already-declared whole view; every original indexed inverse still runs.
fn historical_entry<'a>(source: &str, expected: &str, current: &'a str) -> Cow<'a, str> {
    match (source, expected) {
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
    let probe_before = crate::source_reflows_merge::before_phase_probes(row["source"].as_str().unwrap(), actual);
    let current = probe_before.as_ref();
    // Restore the exact pre-Main44 adapter input before its immutable inverses.
    assert_eq!(current.len(), 326351);
    assert_eq!(digest(current), "c4eb6d914f71ea5ea5ec3cee2710b4c18fee4c45b2c9019e8b48b5fb46653bbb");
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
    row["inverse"].as_array_mut().unwrap().extend(main44.as_array().unwrap().iter().cloned());
    row["current_length"] = serde_json::json!(current.len());
    row["current_sha256"] = serde_json::json!(digest(current));
    // Authenticate subsequent complete source inputs before replaying the same
    // immutable CI bodies; each inverse binds every changed literal and offset.
    {
        let row = &mut rows[5];
        let current = compiled(row["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(current.len(), 29371);
        assert_eq!(digest(current), "9dffc2821b02990bea08f9434b911c9dca98e52af5be26013bc1702916118742");
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
        row["inverse"].as_array_mut().unwrap().extend(edits.as_array().unwrap().iter().cloned());
        row["current_length"] = serde_json::json!(current.len());
        row["current_sha256"] = serde_json::json!(digest(current));
    }
    {
        let row = &mut rows[8];
        let current = compiled(row["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(current.len(), 59526);
        assert_eq!(digest(current), "96f77aa63800b15888434dde208ea6ed3f9bf3294297bcc371b8dfacfc9c7cf3");
        let edits: serde_json::Value = serde_json::from_str("[[5156,\"mod selected_function;\\n\",\"\"]]").unwrap();
        let mut checkpoint = current.to_owned();
        for edit in edits.as_array().unwrap().iter().rev() {
            let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
            let after = edit[1].as_str().unwrap();
            assert_eq!(&checkpoint[at..at + after.len()], after);
            checkpoint.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
        }
        assert_eq!(checkpoint.len() as u64, row["current_length"]);
        assert_eq!(digest(&checkpoint), row["current_sha256"]);
        row["inverse"].as_array_mut().unwrap().extend(edits.as_array().unwrap().iter().cloned());
        row["current_length"] = serde_json::json!(current.len());
        row["current_sha256"] = serde_json::json!(digest(current));
    }
    // Compose diagnostic-only inverses last, so they execute first on the
    // complete compiled/fresh current bytes before every existing CI inverse.
    let probes: serde_json::Value = serde_json::from_str("[{\"source\":\"src/cleanup_plan/validate.rs\",\"length\":4475,\"sha256\":\"75ac0859815d3a47a94b81493c04a8ae89516f4939fe79a8da7f94d141e41c1b\",\"inverse\":[[530,\"    #[cfg(test)]\\n    let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\\n        \\\"cleanup_plan::validate::canonical_rebuild\\\",\\n    );\\n\",\"\"],[2065,\"    #[cfg(test)]\\n    drop(_allocation_phase);\\n\",\"\"]]},{\"source\":\"src/cleanup_plan/replay.rs\",\"length\":326426,\"sha256\":\"5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d\",\"inverse\":[[2773,\"\",\"\\n\"],[2865,\"\",\"\\n\"],[4765,\"\",\"\\n\"],[6116,\"\",\"\\n\"],[6445,\"\",\"\\n\"],[6511,\"\",\"\\n\"],[113510,\"    #[cfg(test)]\\n    let _allocation_phase =\\n        crate::test_allocation_observer::function_phase_guard(\\\"cleanup_plan::replay::hir_paths\\\");\\n\",\"\"],[278824,\"    #[cfg(test)]\\n    let _allocation_phase =\\n        crate::test_allocation_observer::function_phase_guard(\\\"cleanup_plan::replay::plan_paths\\\");\\n\",\"\"],[576,\"use final_output::compact as compact_path;\\n\",\"\"],[122062,\"    let minimum = compact_path::count!(\\n\",\"    let minimum = path_array::count!(\\n\"],[122331,\"    final_output::compact::hir_selection!(work);\\n\",\"    work.budget.reserve_bytes(\\n        work.function,\\n        final_output::HIR_SELECTION_CONTROLS,\\n        \\\"HIR single-suffix selection controls\\\",\\n    )?;\\n\"],[122852,\"            compact_path::push!(\\n\",\"            path_array::push!(\\n\"],[123324,\"                final_output::compact::hir_move!(work);\\n                final_output::compact::hir_work!(work);\\n\",\"                work.budget.reserve_bytes(\\n                    work.function,\\n                    final_output::HIR_MOVE_CONTROLS,\\n                    \\\"HIR single-suffix move controls\\\",\\n                )?;\\n                work.charge(1, \\\"skeleton prefix clone\\\")?;\\n\"],[123938,\"            compact_path::push!(\\n\",\"            path_array::push!(\\n\"],[127075,\"            compact_path::push!(\\n\",\"            path_array::push!(\\n\"],[129471,\"        compact_path::push!(\\n\",\"        path_array::push!(\\n\"]]},{\"source\":\"src/cleanup_plan/replay/factored.rs\",\"length\":49022,\"sha256\":\"3e3835bc69e3029d4fbfb03639dab27573e3bdd3d8c02ea286a0cc53ea94ea02\",\"inverse\":[[41290,\"    let enumeration_phase = crate::test_allocation_observer::function_phase_guard(\\n        \\\"cleanup_plan::crosscheck::enumerated_hir_plan\\\",\\n    );\\n\",\"\"],[41845,\"    drop(enumeration_phase);\\n\",\"\"],[42067,\"        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\\n            \\\"cleanup_plan::crosscheck::factored_compare\\\",\\n        );\\n\",\"\"],[14022,\"    let Some(hir) = canonical.id_with_compact_receiving(\\n        function,\\n        vec![(hir_root, 1)],\\n        budget,\\n        &mut receiving,\\n    )?\\n\",\"    let Some(hir) =\\n        canonical.id_with_receiving(function, vec![(hir_root, 1)], budget, &mut receiving)?\\n\"],[14504,\"    let Some(plan) = canonical.id_with_compact_receiving(\\n        function,\\n        vec![(plan_root, 1)],\\n        budget,\\n        &mut receiving,\\n    )?\\n\",\"    let Some(plan) =\\n        canonical.id_with_receiving(function, vec![(plan_root, 1)], budget, &mut receiving)?\\n\"]]},{\"source\":\"src/cleanup_plan/replay/factored/continuation_receiver.rs\",\"length\":2919,\"sha256\":\"f8720b0acb29fe38df16c4c0c2f9baf69c220f99db81b8637b766c9f02529b4a\",\"inverse\":[[803,\"    let enumeration_phase = crate::test_allocation_observer::function_phase_guard(\\n        \\\"cleanup_plan::crosscheck::enumerated_hir_plan\\\",\\n    );\\n\",\"\"],[1358,\"    drop(enumeration_phase);\\n\",\"\"],[1580,\"        let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\\n            \\\"cleanup_plan::crosscheck::factored_compare\\\",\\n        );\\n\",\"\"]]}]").unwrap();
    let mut probes = probes;
    // Preserve the complete original probe entry, then compose the two exact
    // current early-release splices ahead of every immutable indexed inverse.
    {
        let probe = &mut probes[2];
        assert_eq!(probe["source"], "src/cleanup_plan/replay/factored.rs");
        assert_eq!(probe["length"], 49022);
        assert_eq!(probe["sha256"], "3e3835bc69e3029d4fbfb03639dab27573e3bdd3d8c02ea286a0cc53ea94ea02");
        let current = compiled(probe["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(current.len(), 49837);
        assert_eq!(digest(current), "ffca82dfababa42ad8f21b57a51c42d313a769c17dc14115e1a2ac60a103e5c8");
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
        probe["inverse"].as_array_mut().unwrap().extend(extra.as_array().unwrap().iter().cloned());
        probe["length"] = serde_json::json!(current.len());
        probe["sha256"] = serde_json::json!(digest(current));
    }
    for probe in probes.as_array().unwrap() {
        let row = rows.as_array_mut().unwrap().iter_mut().find(|row| row["source"] == probe["source"]).unwrap();
        let actual = compiled(row["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(actual.len() as u64, probe["length"]);
        assert_eq!(digest(actual), probe["sha256"]);
        let prior = crate::source_reflows_merge::before_phase_probes(row["source"].as_str().unwrap(), actual);
        assert_eq!(prior.len() as u64, row["current_length"]);
        assert_eq!(digest(&prior), row["current_sha256"]);
        row["inverse"].as_array_mut().unwrap().extend(probe["inverse"].as_array().unwrap().iter().cloned());
        row["current_length"] = probe["length"].clone();
        row["current_sha256"] = probe["sha256"].clone();
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
    let entered = historical_entry(source, row["current_sha256"].as_str().unwrap(), current);
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
    reader::controls();
}
