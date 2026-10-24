use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::cleanup_plan::replay::expression_facts::{arena, expression_facts};
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module replay.retained_exit;
@id("exit.read") fn read(input:borrow Bytes)->usize {
 let view=bytes_as_slice(input); let range=byte_range(view,0usize,1usize); byte_len(range)
}
@id("exit.main") fn main()->i64 {
 let data=bytes_zeroed(1usize); if read(data)==1usize {42}else{0}
}"#,
            "retained-exit.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn receive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    current: bool,
) -> Result<(), Diagnostic> {
    let statuses = validate_status_sources_retained(function, facts, budget)?;
    if current {
        validate_exits_retained(
            program,
            function,
            storage,
            leaves,
            &BTreeSet::new(),
            budget,
            &statuses,
        )
    } else {
        super::super::super::remaining::validate_exits(
            program,
            function,
            storage,
            leaves,
            &BTreeSet::new(),
            budget,
        )
    }
}

// HIR/facts are existing immutable inputs. Every receiver allocation, result,
// retained token and budget drops while the actual observer is still active.
fn observe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: bool,
    work_limit: usize,
    byte_limit: usize,
    expected: Option<&Result<(), Diagnostic>>,
) -> (bool, usize, usize, bool, Option<ReservationRefusal>, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let ((value, allocation), refused, debit, first) = with_limit_usage_refusal(byte_limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work_limit;
            let result = receive(
                program,
                function,
                &facts,
                &storage,
                &leaves,
                &mut budget,
                current,
            );
            if let Some(expected) = expected {
                assert_eq!(&result, expected);
            }
            let success = result.is_ok();
            if !success && work_limit < MAX_REPLAY_WORK_UNITS && byte_limit == 1 << 26 {
                assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
                assert_eq!(budget.remaining, 0);
            }
            let used = work_limit - budget.remaining;
            drop(result);
            if !success && (work_limit < MAX_REPLAY_WORK_UNITS || byte_limit < 1 << 26) {
                let remaining = budget.remaining;
                let bytes = crate::bounded_output::active_remaining();
                let retry = receive(
                    program,
                    function,
                    &facts,
                    &storage,
                    &leaves,
                    &mut budget,
                    current,
                );
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

fn original(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut budget,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    receive(
        program,
        function,
        &facts,
        &storage,
        &leaves,
        &mut budget,
        false,
    )
}

// The removed operation is executed independently after the same actual status
// producer/bootstrap, while its retained token remains live through full Drop.
fn removed_directory_cost(function: &ResolvedFunction) -> (usize, usize, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let ((cost, allocation), refused, _, first) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let held = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
            let work = budget.remaining;
            let bytes = crate::bounded_output::active_remaining().unwrap();
            let before = crate::test_allocation_observer::current_metrics();
            let mut directory =
                Directory::new(function, &mut budget, "cleanup exit status directory").unwrap();
            for source in &function.cleanup_plan.status_sources {
                directory.insert(function, &source.id, &mut budget).unwrap();
            }
            let after = crate::test_allocation_observer::current_metrics();
            let cost = (
                work - budget.remaining,
                bytes - crate::bounded_output::active_remaining().unwrap(),
                after.allocated - before.allocated,
            );
            drop(directory);
            drop(held);
            drop(budget);
            cost
        })
    });
    assert!(!refused && first.is_none() && !allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    cost
}

fn exact_accounting(
    function: &ResolvedFunction,
    old: &(bool, usize, usize, bool, Option<ReservationRefusal>, usize),
    new: &(bool, usize, usize, bool, Option<ReservationRefusal>, usize),
) {
    eprintln!("exit-status-cost function={} statuses={} old={old:?} current={new:?} borrow=({BORROW_UNITS},{BORROW_BYTES})",
        function.id, function.cleanup_plan.status_sources.len());
    assert!(old.0 && new.0 && !old.3 && !new.3);
    assert!(old.4.is_none() && new.4.is_none());
    let removed = removed_directory_cost(function);
    eprintln!(
        "exit-status-removed function={} cost={removed:?}",
        function.id
    );
    assert_eq!(
        old.1.checked_add(BORROW_UNITS).unwrap(),
        new.1.checked_add(removed.0).unwrap()
    );
    assert_eq!(
        old.2.checked_add(BORROW_BYTES).unwrap(),
        new.2.checked_add(removed.1).unwrap()
    );
    assert_eq!(old.5, new.5.checked_add(removed.2).unwrap());
    match function.cleanup_plan.status_sources.len() {
        0 => {
            assert_eq!(removed, (0, 0, 0));
            assert_eq!(new.1, old.1 + BORROW_UNITS);
            assert_eq!(new.2, old.2 + BORROW_BYTES);
            assert_eq!(new.5, old.5);
        }
        1 => {
            // The actual old first insertion performs relocation1 + VISIT3;
            // it has no key comparison, rotation or parent rebalance.
            assert_eq!(removed.0, 4);
            assert_eq!(BORROW_UNITS, 7);
            assert_eq!(new.1, old.1 + 3);
            // Pin the actual a93 64-bit receipt, not a portable Node layout
            // lemma. All hosts retain the independent additive equations above.
            #[cfg(target_pointer_width = "64")]
            {
                assert_eq!(BORROW_BYTES, 49);
                assert_eq!(removed.1, 48);
                assert_eq!(removed.2, 48);
                assert_eq!(new.2, old.2 + 1);
                assert_eq!(old.5, new.5 + 48);
            }
        }
        _ => {}
    }
}

// A separate real public caller with three ordinary borrowed reads. The old
// receiver is unchanged and builds its original duplicate status directory.
fn representative() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module replay.retained_exit_many;
@id("many.read") fn read(input:borrow Bytes)->usize {
 let view=bytes_as_slice(input); let range=byte_range(view,0usize,1usize); byte_len(range)
}
@id("many.main") fn main()->i64 {
 let data=bytes_zeroed(1usize);
 if read(data)==1usize {if read(data)==1usize {if read(data)==1usize {42}else{0}}else{0}}else{0}
}"#,
            "retained-exit-many.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn complete_exit_status_receivers_keep_six_fields_order_and_drop() {
    complete_source_archives();
    let program = program();
    let mut saw_status = false;
    for function in &program.functions {
        let expected = original(&program, function);
        assert!(expected.is_ok());
        let old = observe(
            &program,
            function,
            false,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            Some(&expected),
        );
        let new = observe(
            &program,
            function,
            true,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            Some(&expected),
        );
        assert!(old.0 && new.0 && !old.3 && !new.3);
        exact_accounting(function, &old, &new);
        if function.cleanup_plan.exits.is_empty() {
            continue;
        }
        let mut attacks = Vec::new();
        let mut bad = function.clone();
        bad.cleanup_plan.exits[0].id = crate::cleanup_plan::ExitTargetId(u32::MAX);
        attacks.push(bad);
        let mut bad = function.clone();
        bad.cleanup_plan.exits[0].from = BlockId(u32::MAX);
        attacks.push(bad);
        let mut bad = function.clone();
        bad.cleanup_plan.exits[0]
            .leaves_regions
            .push(super::super::CleanupRegionId(u32::MAX));
        attacks.push(bad);
        let mut bad = function.clone();
        bad.cleanup_plan.exits[0].continuation = ExitContinuation::ReturnUnit;
        attacks.push(bad);
        if let Some(source) = function.cleanup_plan.status_sources.first() {
            saw_status = true;
            let mut missing = source.id.clone();
            missing.expression = ExpressionId::from_owned("unknown.λ\0🙂status".into());
            let mut bad = function.clone();
            bad.cleanup_plan.exits[0].continuation = ExitContinuation::ReturnFailure {
                source: missing.clone(),
            };
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.exits[0].id = crate::cleanup_plan::ExitTargetId(u32::MAX);
            bad.cleanup_plan.exits[0].continuation =
                ExitContinuation::ReturnFailure { source: missing };
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.status_sources.push(source.clone());
            attacks.push(bad);
        }
        for exit in &function.cleanup_plan.exits {
            if let Some(first) = exit.finalize_in_order.first() {
                let at = exit.id.0 as usize;
                let mut bad = function.clone();
                bad.cleanup_plan.exits[at]
                    .finalize_in_order
                    .push(first.clone());
                attacks.push(bad);
                let mut bad = function.clone();
                bad.cleanup_plan.exits[at].finalize_in_order[0].guard_flag =
                    LivenessFlagId(u32::MAX);
                attacks.push(bad);
            }
        }
        for hostile in attacks {
            let expected = original(&program, &hostile);
            // ReturnUnit may be legitimate for a unit-returning source function.
            for current in [false, true] {
                let result = observe(
                    &program,
                    &hostile,
                    current,
                    MAX_REPLAY_WORK_UNITS,
                    1 << 26,
                    Some(&expected),
                );
                assert_eq!(result.0, expected.is_ok());
                assert!(!result.3);
            }
        }
    }
    assert!(saw_status);
    let program = representative();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "many.main")
        .unwrap();
    assert!(function.cleanup_plan.status_sources.len() >= 2);
    let expected = original(&program, function);
    assert!(expected.is_ok());
    let old = observe(
        &program,
        function,
        false,
        MAX_REPLAY_WORK_UNITS,
        1 << 26,
        Some(&expected),
    );
    let new = observe(
        &program,
        function,
        true,
        MAX_REPLAY_WORK_UNITS,
        1 << 26,
        Some(&expected),
    );
    exact_accounting(function, &old, &new);
    assert!(old.0 && new.0 && !old.3 && !new.3);
    assert!(new.1 < old.1 && new.2 < old.2 && new.5 < old.5);
}

#[test]
fn exit_status_full_receivers_keep_exact_short_zero_work_bytes_and_tree_fallback() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| !f.cleanup_plan.status_sources.is_empty())
        .unwrap();
    for current in [false, true] {
        let full = observe(
            &program,
            function,
            current,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            None,
        );
        assert!(full.0 && !full.3 && full.1 > 0 && full.2 > 0);
        for limit in [0, full.1 - 1, full.1] {
            let result = observe(&program, function, current, limit, 1 << 26, None);
            assert_eq!(result.0, limit == full.1);
            assert!(!result.3);
        }
        for limit in [0, full.2 - 1, full.2] {
            let result = observe(
                &program,
                function,
                current,
                MAX_REPLAY_WORK_UNITS,
                limit,
                None,
            );
            assert_eq!(result.0, limit == full.2);
            assert_eq!(result.3, limit != full.2);
            assert_eq!(result.4.is_some(), limit != full.2);
        }
    }
    let facts = Facts::Tree(expression_facts(function).unwrap());
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let expected = original(&program, function);
    let mut costs = Vec::new();
    for current in [false, true] {
        let ((work, allocation), refused, bytes, first) = with_limit_usage_refusal(1 << 26, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let statuses =
                    validate_status_sources_retained(function, &facts, &mut budget).unwrap();
                assert!(statuses.ids.is_none());
                let result = if current {
                    validate_exits_retained(
                        &program,
                        function,
                        &storage,
                        &leaves,
                        &BTreeSet::new(),
                        &mut budget,
                        &statuses,
                    )
                } else {
                    super::super::super::remaining::validate_exits(
                        &program,
                        function,
                        &storage,
                        &leaves,
                        &BTreeSet::new(),
                        &mut budget,
                    )
                };
                assert_eq!(result, expected);
                let work = MAX_REPLAY_WORK_UNITS - budget.remaining;
                drop(result);
                drop(statuses);
                drop(budget);
                work
            })
        });
        assert!(!refused && first.is_none() && !allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        costs.push((work, bytes));
    }
    assert_eq!(costs[1].0, costs[0].0 + BORROW_UNITS);
    // The selector runs once before borrowing and again in the complete tree
    // fallback. Its full receiving bank remains paid in addition to the loan.
    assert_eq!(
        costs[1].1,
        costs[0].1 + BORROW_BYTES + helpers::EXIT_SELECTOR_BYTES
    );
}

#[test]
fn exit_status_authentication_precedes_debit_and_foreign_active_stays_sticky() {
    let program = program();
    let function = &program.functions[0];
    let foreign = function.clone();
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut other = ReplayBudget::new();
            let statuses = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
            for (subject, payer) in [(&foreign, &mut budget), (function, &mut other)] {
                let bytes = crate::bounded_output::active_remaining();
                let work = payer.remaining;
                let result = validate_exits_retained(
                    &program,
                    subject,
                    &storage,
                    &leaves,
                    &BTreeSet::new(),
                    payer,
                    &statuses,
                );
                assert_eq!(
                    result,
                    Err(replay_error(
                        subject,
                        "exit status directory belongs to another function or budget"
                    ))
                );
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                assert_eq!(payer.remaining, work);
                drop(result);
            }
            let (_, nested_refused, nested_bytes, nested_first) =
                with_limit_usage_refusal(1 << 26, || {
                    let result = validate_exits_retained(
                        &program,
                        function,
                        &storage,
                        &leaves,
                        &BTreeSet::new(),
                        &mut budget,
                        &statuses,
                    );
                    assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
                    assert!(result
                        .as_ref()
                        .unwrap_err()
                        .message
                        .contains("invocation owner changed"));
                    drop(result);
                });
            assert!(!nested_refused && nested_first.is_none());
            assert_eq!(nested_bytes, 0);
            let bytes = crate::bounded_output::active_remaining();
            let restored = validate_exits_retained(
                &program,
                function,
                &storage,
                &leaves,
                &BTreeSet::new(),
                &mut budget,
                &statuses,
            );
            assert!(restored.is_err());
            assert_eq!(budget.remaining, 0);
            assert_eq!(crate::bounded_output::active_remaining(), bytes);
            drop(restored);
            drop(statuses);
            drop(other);
            drop(budget);
        })
    });
    assert!(refused && first.is_some());
    assert_eq!(first.unwrap().requested, 0);
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

fn complete_source_archives() {
    macro_rules! ci_str {
        ($path:expr) => {
            source_entry(file!(), $path, ::std::include_str!($path))
        };
    }

    use sha2::{Digest, Sha256};
    let rows: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/exit-status-directory-reuse/exact-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        ("src/cleanup_plan/replay.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay.rs.before.txt")), "2a4db37f10435075809a43d112073b686710d55f4f5f43e0f553686a63504796"),
        ("src/cleanup_plan/replay/expression_facts/receiving.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt")), "2e8e01a0d509c00cc39953fa8358d7533c862ea1367c9eba6fe7fcb8c0179762"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__retained.rs.before.txt")), "5460420073056042a13515edc037e2c6737da39286c91105b3feae4f942aeab2"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__remaining.rs.before.txt")), "ba562e69301e6283bff9bec4517444ccea0fc2094f56959214cbbf78e3915b1b"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__remaining__selected.rs.before.txt")), "446459444ef20047d571618c4c85c4cb1d8151f52bb1b587221c795bff0b52b9"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/a.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__live_oracle__source_custody__a.rs.before.txt")), "e539844c397f769ad20cdf13324de0dfd598ed053e88ad57bdb18ee766c7a951"),
        ("src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__expression_facts__receiving__remaining__tests.rs.before.txt")), "ac2685e11c2cd09f9caee373b6b5744b5544b5680032cc07fbead0650ceef474"),
        ("tests/hir_module_boundaries.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/tests__hir_module_boundaries.rs.before.txt")), "ef30bafb7e74ece87a8e4ccc2a056906a5775e3689139e538c74db8d8c4d4b0d"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt")), "4cc35f98927317e03348e7cb2508fec59d99b697def2b35f620fcac0060735a6"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-status-directory-reuse/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt")), "a349da31a1efabca38017cca4b1b7305a694ff7bd5f923e8eddad84d9e82053d"),
    ];
    assert_eq!(rows.as_array().unwrap().len(), sources.len());
    for (row, (path, current, old, fixed)) in rows.as_array().unwrap().iter().zip(sources) {
        assert_eq!(row["source"].as_str().unwrap(), path);
        assert_eq!(row["sha256"].as_str().unwrap(), fixed);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
            ),
            fixed
        );
        let mut restored = exact_entry(path, current, "exit-status-directory-reuse");
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, old);
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
        .filter(|row| row["group"] == "exit-status-directory-reuse")
    {
        let path = row["path"].as_str().unwrap();
        let input = row["original_input"].as_str().unwrap();
        let output = row["original_output"].as_str().unwrap();
        assert_eq!(
            exact_entry(path, input, "exit-status-directory-reuse"),
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
                "exit-status-directory-reuse"
            ))
            .is_err());
        }
    }
    assert_eq!(
        exact_entry("unlisted", "unchanged λ\0", "exit-status-directory-reuse"),
        "unchanged λ\0"
    );
}
