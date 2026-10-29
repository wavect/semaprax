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
        )
        // d70e4af22 changed this row after its pre-allocation whole view was
        // retained in the overlay; recover that exact view, then run every
        // original indexed inverse unchanged.
        | (
            "src/cleanup_plan/replay/factored/oracle_hook.rs",
            "a0cf6a6604b1909859e78ecfe95f780ba1600072ae6ca9e259d2943c7dbebfa7",
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
    let current = historical_entry(
        row["source"].as_str().unwrap(),
        successor["sha256"].as_str().unwrap(),
        compiled(row["source"].as_str().unwrap()).unwrap().1,
    );
    let current = current.as_ref();
    assert_eq!(current.len() as u64, successor["length"]);
    assert_eq!(digest(current), successor["sha256"]);
    let mut checkpoint = current.to_owned();
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
    row["current_length"] = successor["length"].clone();
    row["current_sha256"] = successor["sha256"].clone();
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
