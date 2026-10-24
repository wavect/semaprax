//! Whole original receiver and genuine owned-result allocation controls.
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::measure;

pub(super) fn program() -> ResolvedProgram {
    let owner = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/expression_facts.rs"
    ));
    let source = owner
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    crate::hir::resolve(
        &crate::check(
            &format!("module replay.fact_owner;{source}"),
            "transitive-receiving.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

fn observe<T: PartialEq + std::fmt::Debug>(
    work: usize,
    bytes: usize,
    expected: Option<&Result<T, Diagnostic>>,
    mut call: impl FnMut(&mut ReplayBudget) -> Result<T, Diagnostic>,
) -> (bool, usize, usize, bool, Option<ReservationRefusal>, usize) {
    let ((value, allocation), refused, debit, first) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work;
            let result = call(&mut budget);
            if let Some(expected) = expected {
                assert_eq!(&result, expected);
            }
            let success = result.is_ok();
            let used = work - budget.remaining;
            drop(result);
            if budget.refusal_spent {
                let remaining = budget.remaining;
                let bytes = crate::bounded_output::active_remaining();
                let retry = call(&mut budget);
                assert!(retry.is_err());
                assert_eq!(budget.remaining, remaining);
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                drop(retry);
            }
            drop(budget);
            (success, used)
        })
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    (
        value.0,
        value.1,
        debit,
        refused,
        first,
        allocation.allocated,
    )
}

fn full(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: bool,
) -> Result<(), Diagnostic> {
    let storage = validate_slots(function)?;
    let leaves = collect_leaves(function)?;
    let expected = crate::cleanup_plan::replay::validate_exits(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
    );
    let result = observe(MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected), |budget| {
        if current {
            super::super::remaining::validate_exits(
                program,
                function,
                &storage,
                &leaves,
                &BTreeSet::new(),
                budget,
            )
        } else {
            before_remaining::validate_exits(
                program,
                function,
                &storage,
                &leaves,
                &BTreeSet::new(),
                budget,
            )
        }
    });
    assert_eq!(result.0, expected.is_ok());
    assert!(!result.3);
    expected
}

#[test]
fn complete_transitive_receivers_preserve_old_order_payloads_and_drop() {
    let program = program();
    let mut exits = 0;
    let mut places = 0;
    for function in &program.functions {
        assert_eq!(
            full(&program, function, false),
            full(&program, function, true)
        );
        for exit in &function.cleanup_plan.exits {
            exits += 1;
            let expected = crate::cleanup_plan::replay::expected_exit_regions(function, exit);
            let new = observe(MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected), |budget| {
                expected_exit_regions(function, exit, budget)
            });
            assert_eq!(new.0, expected.is_ok());
            assert!(!new.3);
            let mut attacks = Vec::new();
            let mut bad = function.clone();
            bad.cleanup_plan.exits[exit.id.0 as usize].from = BlockId(u32::MAX);
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.blocks[exit.from.0 as usize].region = CleanupRegionId(u32::MAX);
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.exits[exit.id.0 as usize].continuation =
                ExitContinuation::Continue(EdgeId(u32::MAX));
            attacks.push(bad);
            if let ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } = &exit.continuation
            {
                let target = function.cleanup_plan.edges[edge.0 as usize].to;
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[target.0 as usize].region = CleanupRegionId(u32::MAX);
                attacks.push(bad);
            }
            for bad in attacks {
                let target = &bad.cleanup_plan.exits[exit.id.0 as usize];
                let expected = crate::cleanup_plan::replay::expected_exit_regions(&bad, target);
                let new = observe(MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected), |budget| {
                    expected_exit_regions(&bad, target, budget)
                });
                assert_eq!(new.0, expected.is_ok());
                assert!(!new.3);
                assert_eq!(full(&program, &bad, false), full(&program, &bad, true));
            }
        }
        let storage = validate_slots(function).unwrap();
        let leaves = collect_leaves(function).unwrap();
        for leaf in leaves.values() {
            places += 1;
            let mut cases = vec![leaf.place.clone()];
            let mut bad = leaf.place.clone();
            bad.projections
                .push(DeclarationId::new("unknown.λ\0🙂projection"));
            cases.push(bad);
            let mut bad = leaf.place.clone();
            bad.storage =
                StorageId::Temporary(ExpressionId::from_owned("unknown.λ\0🙂storage".into()));
            cases.push(bad);
            for place in cases {
                let expected = crate::cleanup_plan::replay::validate_place(
                    function, &place, &storage, &leaves,
                );
                let new = observe(MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected), |budget| {
                    validate_place(function, &place, &storage, &leaves, budget)
                });
                assert_eq!(new.0, expected.is_ok());
                assert!(!new.3);
            }
        }
    }
    assert!(exits > 0 && places > 0);
}

#[test]
fn paid_transitive_results_keep_exact_short_zero_work_and_bytes() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| !collect_leaves(f).unwrap().is_empty())
        .unwrap();
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let place = &leaves.values().next().unwrap().place;
    let expected = crate::cleanup_plan::replay::validate_place(function, place, &storage, &leaves);
    let full = observe(MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected), |budget| {
        validate_place(function, place, &storage, &leaves, budget)
    });
    assert!(full.0 && !full.3 && full.1 > 0 && full.2 > 0);
    assert!(full.5 <= full.2);
    for work in [0, full.1 - 1, full.1] {
        let value = observe(work, 1 << 26, None, |budget| {
            validate_place(function, place, &storage, &leaves, budget)
        });
        assert_eq!(value.0, work == full.1);
        assert!(!value.3);
    }
    for bytes in [0, full.2 - 1, full.2] {
        let value = observe(MAX_REPLAY_WORK_UNITS, bytes, None, |budget| {
            validate_place(function, place, &storage, &leaves, budget)
        });
        assert_eq!(value.0, bytes == full.2);
        assert_eq!(value.3, bytes != full.2);
        assert_eq!(value.4.is_some(), bytes != full.2);
    }
    let exit = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| crate::cleanup_plan::replay::expected_exit_regions(function, exit).is_ok())
        .unwrap();
    let expected = crate::cleanup_plan::replay::expected_exit_regions(function, exit);
    let full = observe(MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected), |budget| {
        expected_exit_regions(function, exit, budget)
    });
    assert!(full.0 && !full.3 && full.1 > 0 && full.2 > 0 && full.5 <= full.2);
    for work in [0, full.1 - 1, full.1] {
        let value = observe(work, 1 << 26, None, |budget| {
            expected_exit_regions(function, exit, budget)
        });
        assert_eq!(value.0, work == full.1);
        assert!(!value.3);
    }
    for bytes in [0, full.2 - 1, full.2] {
        let value = observe(MAX_REPLAY_WORK_UNITS, bytes, None, |budget| {
            expected_exit_regions(function, exit, budget)
        });
        assert_eq!(value.0, bytes == full.2);
        assert_eq!(value.3, bytes != full.2);
        assert_eq!(value.4.is_some(), bytes != full.2);
    }
}

#[test]
fn transitive_receiver_originals_and_sticky_auth_stay_complete() {
    let program = program();
    let function = &program.functions[0];
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    if let Some(leaf) = leaves.values().next() {
        let value = observe(0, 1 << 26, None, |budget| {
            validate_place(function, &leaf.place, &storage, &leaves, budget)
        });
        assert!(!value.0 && !value.3);
    }
    let old = ci_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__expression_facts__receiving__remaining.rs.before.txt"));
    let adapted = old.replace("use super::*;", "use super::super::*;\nuse crate::cleanup_plan::replay::{expected_exit_regions, validate_place};")
        .replace("pub(super) mod selected;", "use super::super::remaining::selected;")
        .replace("\n#[cfg(test)]\nmod tests;\n", "\n");
    assert_eq!(ci_str!("before_remaining.rs"), adapted);
    source_archives();
}

fn source_archives() {
    macro_rules! ci_str {
        ($path:expr) => {
            source_entry(file!(), $path, ::std::include_str!($path))
        };
    }

    use sha2::{Digest, Sha256};
    let rows: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/receiving-transitive-results-816/exact-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        ("src/cleanup_plan/replay/expression_facts/receiving.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt")), "631abde22c4c2ff48d35b75d1f1e5475819d6ee20ed4bb5c83c531b057675ecd"),
        ("src/cleanup_plan/replay/expression_facts/receiving/regions_entry.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/regions_entry.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__expression_facts__receiving__regions_entry.rs.before.txt")), "327237ebc01755438f25ff967fc327a3ffb4f9d6be2a2a54bad6a36803b7095b"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__expression_facts__receiving__remaining.rs.before.txt")), "ea688655dac741ee5795e16582e64a69cae3928d491d008d36bca4549c73df05"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__expression_facts__receiving__retained.rs.before.txt")), "350076d7fb340ac0c350a278b00f318762f1189418e12fc2abcc1c2e3c6adbe3"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__expression_facts__receiving__retained__exits.rs.before.txt")), "a314eae7257c5ba20dc57d1a9ee1f166d2919bb34ebf9fb4f46e287c8b64b40c"),
        ("src/cleanup_plan/replay/leaf_index.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/leaf_index.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__leaf_index.rs.before.txt")), "6d7f32f3ed12640d920d58b6001f1088d4ed8b8e846303e715065f19ca4a1b9e"),
        ("tests/hir_module_boundaries.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/tests__hir_module_boundaries.rs.before.txt")), "4c6763cc6a218441e08de18be3eeb8f54e0612e23879e7c7085462f79521d86e"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt")), "ffca787b9ed74a134e02ed5af5dcfe62a137815b59360a130d4c2c3149d6f807"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/receiving-transitive-results-816/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt")), "3aba68eb707236e99d2d93679c19c9e0a9b4849a8f7e94c5bf6ef60e7fc6548f"),
    ];
    assert_eq!(rows.as_array().unwrap().len(), sources.len());
    for (row, (path, current, archive, fixed)) in rows.as_array().unwrap().iter().zip(sources) {
        assert_eq!(row["source"].as_str().unwrap(), path);
        assert_eq!(row["sha256"].as_str().unwrap(), fixed);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            fixed
        );
        let mut result = exact_entry(path, current, "receiving-transitive-results-816");
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(result.matches(after).count(), 1);
            result = result.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(result, archive);
    }
}

fn source_entry(
    caller: &'static str,
    included: &'static str,
    current: &'static str,
) -> &'static str {
    let source = included
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .and_then(|s| s.strip_prefix('/'))
        .unwrap();
    let expected=match source {
 "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs"=>"e6d9241f9287e8e00d5c8413623383b53b46e5efbe6da50e576dcf6bbd642db4",
 "tests/hir_module_boundaries.rs"=>"055a8f7ddd8b9f5b2d12122154de053a0a6823b75c298580cd4770628c4e8851",
 "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"=>"d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0",
 "src/cleanup_plan/replay/live_oracle/source_custody/b.rs"=>"4d534b5433078d8d65ec9f09c7f763614a177cd743aa02465de64c023e509173",
 _=>return crate::source_reflows::source(caller,included,current),
 };
    Box::leak(
        crate::source_reflows_merge::main_e177_final_inputs::value_direct(
            source, expected, current,
        )
        .into_boxed_str(),
    )
}
fn exact_entry(path: &str, current: &str, group: &str) -> String {
    use sha2::{Digest, Sha256};
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/workspace-test-funding-20261024/entry-reflows.json"
    ));
    assert_eq!(data.len(), 1110944);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(data.as_bytes()))
        ),
        "d3aeae9e7f7aecd60a9329f17714dfb521e2c6d0f8038961bceefa5f319e1904"
    );
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    let Some(row) = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == path && row["group"] == group)
    else {
        return current.to_owned();
    };
    assert_eq!(current.len(), row["input_bytes"].as_u64().unwrap() as usize);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["input_sha256"].as_str().unwrap()
    );
    let mut stage = current.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let before = edit["before"].as_str().unwrap();
        assert_eq!(stage.matches(before).count(), 1);
        stage = stage.replacen(before, edit["after"].as_str().unwrap(), 1);
    }
    assert_eq!(stage.len(), row["output_bytes"].as_u64().unwrap() as usize);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(stage.as_bytes()))
        ),
        row["output_sha256"].as_str().unwrap()
    );
    stage
}

#[test]
fn complete_source_entry_reflows_refuse_unicode_drift_truncation_replay_and_foreign_inputs() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/workspace-test-funding-20261024/entry-reflows.json"
    )))
    .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 9);
    for row in rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == "receiving-transitive-results-816")
    {
        let path = row["path"].as_str().unwrap();
        let input = row["original_input"].as_str().unwrap();
        let output = row["original_output"].as_str().unwrap();
        assert_eq!(
            exact_entry(path, input, "receiving-transitive-results-816"),
            output
        );
        let mut short = input.to_owned();
        short.pop();
        let mut changed = input.as_bytes().to_vec();
        changed[0] = b'X';
        let changed = String::from_utf8(changed).unwrap();
        for foreign in [
            String::new(),
            short,
            changed,
            format!("{input}\n"),
            output.to_owned(),
            format!("{input}{input}"),
        ] {
            assert!(std::panic::catch_unwind(|| exact_entry(
                path,
                &foreign,
                "receiving-transitive-results-816"
            ))
            .is_err());
        }
    }
    assert_eq!(
        exact_entry(
            "unlisted",
            "unchanged λ\0",
            "receiving-transitive-results-816"
        ),
        "unchanged λ\0"
    );
}
