use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = include_str!("../../../expression_facts.rs")
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    crate::hir::resolve(
        &crate::check(
            &format!("module replay.fact_owner;{source}"),
            "status-fact-reuse.spx",
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
    let statuses = if current {
        validate_status_sources_retained(function, facts, budget)?
    } else {
        status_before::validate_status_sources_retained(function, facts, budget)?
    };
    validate_blocks_and_edges_retained(
        program, function, facts, storage, leaves, None, budget, &statuses,
    )
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
        super::super::super::arena::build::<errors::Legacy>(
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
        super::super::super::arena::build::<errors::Legacy>(
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

// Independently price the actual removed lookups on the same immutable arena.
// Bootstrap the unchanged old receiver first; no fixture-gap numeric is fitted.
fn removed_lookup_cost(function: &ResolvedFunction) -> (usize, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let ((cost, allocation), refused, _, _) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            drop(
                status_before::validate_status_sources_retained(function, &facts, &mut budget)
                    .unwrap(),
            );
            let work = budget.remaining;
            let bytes = crate::bounded_output::active_remaining().unwrap();
            for source in &function.cleanup_plan.status_sources {
                if let StatusProducer::PropagatedCall { callee } = &source.producer {
                    let Some(Some(call)) = facts
                        .fact(function, &source.id.expression, &mut budget)
                        .unwrap()
                    else {
                        panic!("complete old receiver authenticated this exact call");
                    };
                    assert_eq!(call.callee.as_ref(), callee);
                }
            }
            let cost = (
                work - budget.remaining,
                bytes - crate::bounded_output::active_remaining().unwrap(),
            );
            drop(budget);
            cost
        })
    });
    assert!(!refused && !allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    cost
}

#[test]
fn complete_status_receivers_preserve_call_payload_six_fields_priority_and_drop() {
    let program = program();
    let mut saved = false;
    let mut attacked_call = false;
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
        // Keep the complete earlier component's exact equations independent.
        // The actual production receiver above still runs with every original
        // hostile/physical/Drop control; prefix work is a separate algorithm.
        let component = component_before_prefix::observe(
            &program,
            function,
            true,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            Some(&expected),
        );
        assert!(component.0 && !component.3);
        assert!(old.5 <= old.2 && new.5 <= new.2 && component.5 <= component.2);
        {
            let new = component;
            let calls = function
                .cleanup_plan
                .status_sources
                .iter()
                .filter(|source| matches!(source.producer, StatusProducer::PropagatedCall { .. }))
                .count();
            let receiving_bytes = function
                .cleanup_plan
                .status_sources
                .len()
                .checked_mul(std::mem::size_of::<&Option<CallFact<'_>>>())
                .unwrap()
                .checked_add(
                    calls
                        .checked_mul(std::mem::size_of::<&CallFact<'_>>())
                        .unwrap(),
                )
                .unwrap();
            let assignments = function
                .cleanup_plan
                .status_sources
                .len()
                .checked_add(calls)
                .unwrap();
            let removed = removed_lookup_cost(function);
            assert_eq!(
                old.1.checked_add(assignments).unwrap(),
                new.1.checked_add(removed.0).unwrap()
            );
            assert_eq!(
                old.2.checked_add(receiving_bytes).unwrap(),
                new.2.checked_add(removed.1).unwrap()
            );
            if function
                .cleanup_plan
                .status_sources
                .iter()
                .any(|s| matches!(s.producer, StatusProducer::PropagatedCall { .. }))
            {
                assert!(new.1 < old.1 && new.2 < old.2);
                assert!(new.5 <= old.5); // Receiving control is fully paid and allocation-free.
                saved = true;
            }
        }
        let mut attacks = Vec::new();
        if let Some(source) = function.cleanup_plan.status_sources.first() {
            let mut bad = function.clone();
            bad.cleanup_plan.status_sources.push(source.clone());
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.status_sources[0].id.expression =
                ExpressionId::from_owned("foreign.first.status".into());
            attacks.push(bad);
            // Unknown current source wins before a later duplicate/callee failure.
            let mut bad = function.clone();
            bad.cleanup_plan.status_sources[0].id.expression =
                ExpressionId::from_owned("foreign.first.status".into());
            bad.cleanup_plan.status_sources.push(source.clone());
            attacks.push(bad);
        }
        for (at, source) in function.cleanup_plan.status_sources.iter().enumerate() {
            if let StatusProducer::PropagatedCall { .. } = &source.producer {
                attacked_call = true;
                let wrong = program
                    .functions
                    .iter()
                    .find(|other| match &source.producer {
                        StatusProducer::PropagatedCall { callee } => other.id != *callee,
                        _ => false,
                    })
                    .unwrap()
                    .id
                    .clone();
                let mut bad = function.clone();
                bad.cleanup_plan.status_sources[at].producer = StatusProducer::PropagatedCall {
                    callee: wrong.clone(),
                };
                attacks.push(bad);
                // Duplicate insertion wins over the duplicate's malformed callee.
                let mut bad = function.clone();
                let mut duplicate = source.clone();
                duplicate.producer = StatusProducer::PropagatedCall {
                    callee: wrong.clone(),
                };
                bad.cleanup_plan.status_sources.push(duplicate);
                attacks.push(bad);
                // Existing non-call expression is distinct from unknown expression.
                let mut bad = function.clone();
                bad.cleanup_plan.status_sources[at].id.expression = function.body.id.clone();
                attacks.push(bad);
                let mut bad = function.clone();
                bad.cleanup_plan.status_sources[at].id.lane = StatusLane::ContractFalse;
                attacks.push(bad);
                break;
            }
        }
        for hostile in attacks {
            let expected = original(&program, &hostile);
            assert!(expected.is_err());
            for current in [false, true] {
                let result = observe(
                    &program,
                    &hostile,
                    current,
                    MAX_REPLAY_WORK_UNITS,
                    1 << 26,
                    Some(&expected),
                );
                assert!(!result.0 && !result.3);
            }
        }
    }
    assert!(saved && attacked_call);
}

#[test]
fn actual_status_borrow_receivers_keep_own_exact_short_zero_work_and_bytes() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| {
            f.cleanup_plan
                .status_sources
                .iter()
                .any(|s| matches!(s.producer, StatusProducer::PropagatedCall { .. }))
        })
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
            if result.0 {
                assert_eq!(result.1, full.1);
            }
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
            if result.0 {
                assert_eq!(result.2, full.2);
            }
        }
    }
}

#[test]
fn complete_before_status_fact_receiver_and_full_source_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/status-fact-borrow-reuse/retained.rs.before.txt"
    ));
    let rows: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/status-fact-borrow-reuse/exact-source-inverses.json"
    )))
    .unwrap();
    let fixed = [
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs", "c6069082ff7abb15cff8c832bc0a6086f8ef5c57103c450448ffc0494aa5eeee"),
        ("tests/hir_module_boundaries.rs", "83dc4425c79ca5720aa4d98d61561bdae8690701a0d7fb8daa0fbdfd0076f259"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "485f5b066ebe1e2edb3a42c9bcdfeb6d6f310d2691736eaf63777a02ac803676"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "44edd8afa0f32ce7d479b54e369ed7ae1220eb0bfdd057edb915808090b29c18"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "d1c619e451e181d4f1a9832f87f3055e4231f64b79099fcb4f15c4dbef592d5e"),
    ];
    let sources = [
        ("tests/hir_module_boundaries.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/retained.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/status-fact-borrow-reuse/retained_tests.rs.before.txt"))),
    ];
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed
            .iter()
            .find(|(path, _)| *path == row["source"].as_str().unwrap())
            .unwrap()
            .1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let (_, current, archive) = sources
            .iter()
            .find(|(path, _, _)| *path == row["source"].as_str().unwrap())
            .unwrap();
        let mut source = (*current).to_owned();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            fixed_hash
        );
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, *archive);
    }
    let start = old
        .find("pub(in crate::cleanup_plan::replay) fn validate_status_sources_retained")
        .unwrap();
    let end = old.find("\n// The retained key proof").unwrap();
    assert_eq!(ci_str!("status_before.rs"), format!(
        "//! Complete pre-reuse status receiver, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}
