//! Complete original receiving controls against narrow H006 transport.
use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module prefix.lookup; @id(\"prefix.main\") fn main() -> i64 { 0 }",
            "prefix-lookup.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

fn arena<'hir>(function: &'hir ResolvedFunction, keys: &[ExpressionId]) -> Arena<'hir> {
    let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
    let mut capacity =
        capacity::FactCapacity::with_payer(function, capacity::Payer::Selected(&mut physical))
            .unwrap();
    let mut arena = Arena::new(&mut capacity).unwrap();
    let mut budget = ReplayBudget::unobserved();
    for key in keys {
        arena.prepare_growth_work(function, &mut budget).unwrap();
        arena.reserve_next(&mut capacity).unwrap();
        arena
            .prepare_insert_work(function, key, &mut budget)
            .unwrap();
        assert!(arena.insert(key.clone(), None, &mut capacity).unwrap());
    }
    assert!(arena.balanced());
    arena
}

fn receiving_keys(count: usize) -> Vec<ExpressionId> {
    (0..count)
        .map(|i| {
            ExpressionId::from_owned(if i == 0 {
                String::new()
            } else {
                format!("opaque.{}.{i:06}\0tail", "λ\0識別子🙂".repeat(24))
            })
        })
        .collect()
}

fn receiving_queries(keys: &[ExpressionId]) -> Vec<ExpressionId> {
    let mut queries = keys.to_vec();
    for key in keys {
        queries.push(ExpressionId::from_owned(format!(
            "{}\0missing",
            key.as_str()
        )));
    }
    for text in ["", "\0", "α", "zzzz", "🙂", "opaque."] {
        queries.push(ExpressionId::from_owned(text.to_owned()));
    }
    queries
}

#[test]
fn receiving_comparison_keeps_every_full_key_result_and_original_work_event() {
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 15, 16, 17, 257] {
        for reverse in [false, true] {
            let mut keys = receiving_keys(count);
            if reverse {
                keys.reverse();
            }
            let arena = arena(function, &keys);
            let queries = receiving_queries(&keys);
            let physical = Box::new(physical_storage::PhysicalReservation::new(usize::MAX));
            crate::bounded_output::with_limit(usize::MAX, || {
                let mut legacy_budget = ReplayBudget::unobserved();
                let mut selected_budget = ReplayBudget::unobserved();
                selected_budget.payer = AllocationPayer::SelectedReservation;
                selected_budget.physical = Some(physical);
                let mut before_budget = ReplayBudget::new();
                let mut budget = ReplayBudget::new();
                let mut production_budget = ReplayBudget::new();
                for query in &queries {
                    let legacy_start = legacy_budget.remaining;
                    let selected_start = selected_budget.remaining;
                    let old_start = before_budget.remaining;
                    let start = budget.remaining;
                    let production_start = production_budget.remaining;
                    let before = find(&arena, function, query, &mut before_budget).unwrap();
                    let after = find_receiving(&arena, function, query, &mut budget).unwrap();
                    let production = arena.fact(function, query, &mut production_budget).unwrap();
                    let legacy = arena.fact(function, query, &mut legacy_budget).unwrap();
                    let selected = arena.fact(function, query, &mut selected_budget).unwrap();
                    match (before, after, production, legacy, selected) {
                        (Some(a), Some(b), Some(c), Some(d), Some(e)) => {
                            assert!(
                                std::ptr::eq(a, b)
                                    && std::ptr::eq(a, c)
                                    && std::ptr::eq(a, d)
                                    && std::ptr::eq(a, e)
                            );
                        }
                        (None, None, None, None, None) => {}
                        _ => panic!("lookup hit, miss or complete borrowed payload changed"),
                    }
                    let original_work = old_start - before_budget.remaining;
                    assert_eq!(legacy_start - legacy_budget.remaining, original_work);
                    assert_eq!(selected_start - selected_budget.remaining, original_work);
                    assert_eq!(start - budget.remaining, original_work + 8);
                    assert_eq!(
                        production_start - production_budget.remaining,
                        original_work + if count > 0 { 8 } else { 0 }
                    );
                }
            });
        }
    }
}

#[test]
fn receiving_state_reduces_actual_bills_and_preserves_all_physical_frontiers() {
    let program = program();
    let function = &program.functions[0];
    let keys = receiving_keys(257);
    let arena = arena(function, &keys);
    let queries = receiving_queries(&keys);
    let detached: Vec<_> = queries
        .iter()
        .map(|key| ExpressionId::from_owned(key.as_str().to_owned()))
        .collect();
    let observe = |receiving, available, floor, queries: &[ExpressionId]| {
        crate::bounded_output::with_limit_usage_refusal(available + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                let start = budget.remaining;
                let result = (|| {
                    for query in queries {
                        if receiving {
                            arena.fact(function, query, &mut budget)?;
                        } else {
                            find(&arena, function, query, &mut budget)?;
                        }
                    }
                    Ok::<_, Diagnostic>(())
                })();
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(budget
                        .reserve_bytes(function, 0, "receiving retry")
                        .is_err());
                }
                (ok, start - budget.remaining)
            })
        })
    };
    let (((ok, old_work), old), refused, old_bill, first) = observe(false, usize::MAX, 0, &queries);
    assert!(ok && !refused && first.is_none());
    let (((ok, work), current), refused, exact, first) = observe(true, usize::MAX, 0, &queries);
    assert!(ok && !refused && first.is_none());
    assert!(exact < old_bill);
    assert_eq!(work, old_work + 8 * queries.len());
    assert_eq!(current.allocated, old.allocated);
    for allocation in [old, current] {
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    // Sharing or relocating an immutable query's string cannot alter its bill.
    let (((ok, detached_work), allocation), refused, detached_bill, first) =
        observe(true, usize::MAX, 0, &detached);
    assert!(ok && !refused && first.is_none());
    assert_eq!((detached_work, detached_bill), (work, exact));
    assert!(!allocation.invalid && allocation.live == 0);
    for floor in [0, 37] {
        for available in [0, exact - 1, exact] {
            let (((ok, _), allocation), refused, spent, first) =
                observe(true, available, floor, &queries);
            assert_eq!(ok, available == exact);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            if let Some(first) = first {
                assert_eq!(first.floor, floor);
            }
            assert!(allocation.allocated <= spent && spent <= available);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if available == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn receiving_minimum_hit_funds_both_full_state_layouts_before_comparing() {
    let program = program();
    let function = &program.functions[0];
    let keys = receiving_keys(17);
    let arena = arena(function, &keys);
    let observe = |run| {
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            if run {
                assert!(arena
                    .fact(function, &keys[0], &mut budget)
                    .unwrap()
                    .is_some());
            }
        })
    };
    let ((), refused, setup, first) = observe(false);
    assert!(!refused && first.is_none());
    let ((), refused, paid, first) = observe(true);
    assert!(!refused && first.is_none());
    assert_eq!(
        paid - setup,
        LOOKUP_BYTES + 2 * std::mem::size_of::<ReceivingComparison<'_>>()
    );
}

#[test]
fn receiving_returns_complete_real_call_facts_without_a_projection_or_clone() {
    let owner = include_str!("../../../expression_facts.rs");
    let source = owner
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    let source = format!("module replay.fact_owner;{source}");
    let program =
        crate::hir::resolve(&crate::check(&source, "receiving-facts.spx").unwrap()).unwrap();
    let mut saw_call = false;
    for function in &program.functions {
        let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
        let mut preparation = ReplayBudget::unobserved();
        let arena = super::super::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical),
            &mut preparation,
        )
        .unwrap();
        crate::bounded_output::with_limit(1 << 26, || {
            let mut budget = ReplayBudget::new();
            for (key, fact) in arena.entries() {
                saw_call |= fact.is_some();
                let prior = arena
                    .find(function, key, &mut ReplayBudget::unobserved())
                    .unwrap()
                    .unwrap();
                let current = find_receiving(&arena, function, key, &mut budget)
                    .unwrap()
                    .unwrap();
                assert!(std::ptr::eq(prior, current) && std::ptr::eq(current, fact));
            }
        });
    }
    assert!(saw_call);
}
const LOOKUP_BYTES: usize = super::compact::LOOKUP_BYTES;
fn find_receiving<'a, 'hir>(
    arena: &'a Arena<'hir>,
    function: &ResolvedFunction,
    query: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a Option<CallFact<'hir>>>, Diagnostic> {
    super::compact::find_receiving(arena, function, query, budget)
        .map_err(crate::cleanup_plan::replay::physical_budget::ByteFailure::into_diagnostic)
}
