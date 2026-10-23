//! Independent complete tree receivers and physically paid online arena controls.
use super::*;
use crate::test_allocation_observer::measure;

fn build<'a>(
    function: &'a ResolvedFunction,
    physical: &mut PhysicalReservation,
    budget: &mut ReplayBudget,
) -> Result<super::super::arena::Arena<'a>, Diagnostic> {
    super::super::arena::build::<super::super::errors::Legacy>(
        function,
        capacity::Payer::Selected(physical),
        budget,
    )
}
fn same_lookup(
    old: &BTreeMap<ExpressionId, Option<CallFact<'_>>>,
    arena: &super::super::arena::Arena<'_>,
) {
    assert_eq!(old.len(), arena.len());
    for ((old_key, old), (key, current)) in old.iter().zip(arena.entries()) {
        assert_eq!(old_key, key);
        match (old, current) {
            (Some(old), Some(current)) => {
                assert_eq!(old.callee, current.callee);
                assert_eq!(old.instance, current.instance);
                assert_eq!(old.arguments, current.arguments);
                assert_eq!(old.type_arguments, current.type_arguments);
            }
            (None, None) => {}
            _ => panic!("complete fact variant differs"),
        }
    }
    assert!(arena.balanced());
}
#[test]
fn arena_full_tree_receiving_preserves_all_fields_and_first_diagnostic() {
    let program = program();
    for original in &program.functions {
        for hostile in [false, true] {
            let mut function = original.clone();
            if hostile {
                if let Some(edge) = function.cleanup_plan.edges.first_mut() {
                    edge.id.0 = u32::MAX;
                }
            }
            let old = expression_facts(&function).unwrap();
            let arena = build(
                &function,
                &mut PhysicalReservation::new(usize::MAX),
                &mut ReplayBudget::unobserved(),
            )
            .unwrap();
            same_lookup(&old, &arena);
            let facts = Facts::Arena(arena);
            let storage = validate_slots(&function).unwrap();
            let leaves = collect_leaves(&function).unwrap();
            let old_status = validate_status_sources(&function, &old);
            let new_status = receiving::validate_status_sources(
                &function,
                &facts,
                &mut ReplayBudget::unobserved(),
            );
            assert_eq!(old_status, new_status);
            let old_result = validate_blocks_and_edges(
                &program,
                &function,
                &old,
                &storage,
                &leaves,
                None,
                &mut ReplayBudget::unobserved(),
            );
            let current = receiving::validate_blocks_and_edges(
                &program,
                &function,
                &facts,
                &storage,
                &leaves,
                None,
                &mut ReplayBudget::unobserved(),
            );
            assert_eq!(old_result, current);
        }
    }
}
#[test]
fn arena_exact_backing_work_short_sticky_and_receiving_drop_are_independent() {
    let program = program();
    for function in &program.functions {
        let mut physical = PhysicalReservation::new(usize::MAX);
        let mut budget = ReplayBudget::unobserved();
        let initial = budget.remaining;
        let ((), metrics) = measure(|| {
            let facts = build(function, &mut physical, &mut budget).unwrap();
            let old = expression_facts(function).unwrap();
            // The independent old tree is dropped before observer accounting below.
            same_lookup(&old, &facts);
            drop(old);
            for (key, _) in facts.entries() {
                assert!(facts.fact(function, key, &mut budget).unwrap().is_some());
            }
            drop(facts);
        });
        let charged = physical.charged();
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        // Old-tree allocation belongs to the independent control, not arena payment.
        let mut constructor_physical = PhysicalReservation::new(usize::MAX);
        let ((), actual) = measure(|| {
            let facts = build(
                function,
                &mut constructor_physical,
                &mut ReplayBudget::unobserved(),
            )
            .unwrap();
            drop(facts);
        });
        assert!(actual.allocated <= constructor_physical.charged());
        assert!(actual.highwater <= constructor_physical.charged());
        assert_eq!(actual.allocated, actual.freed);
        assert_eq!(actual.live, 0);
        assert_eq!(charged, constructor_physical.charged());
        assert!(initial > budget.remaining);
        let mut exact = PhysicalReservation::new(charged);
        let facts = build(function, &mut exact, &mut ReplayBudget::unobserved()).unwrap();
        assert_eq!(exact.charged(), charged);
        drop(facts);
        for limit in [0, 1, charged - 1] {
            let mut short = PhysicalReservation::new(limit);
            assert!(build(function, &mut short, &mut ReplayBudget::unobserved()).is_err());
            assert!(short.reserve(0).is_err());
        }
        let mut full_budget = ReplayBudget::unobserved();
        let before = full_budget.remaining;
        drop(
            build(
                function,
                &mut PhysicalReservation::new(usize::MAX),
                &mut full_budget,
            )
            .unwrap(),
        );
        let work = before - full_budget.remaining;
        assert!(work > 0);
        let mut exact = ReplayBudget::unobserved();
        exact.remaining = work;
        drop(
            build(
                function,
                &mut PhysicalReservation::new(usize::MAX),
                &mut exact,
            )
            .unwrap(),
        );
        assert_eq!(exact.remaining, 0);
        let mut short = ReplayBudget::unobserved();
        short.remaining = work - 1;
        assert!(build(
            function,
            &mut PhysicalReservation::new(usize::MAX),
            &mut short
        )
        .is_err());
        // The saved unobserved compatibility route owns no sticky reservation.
        assert!(short
            .charge(function, work, "exhausted arena retry")
            .is_err());
    }
}
#[test]
fn arena_duplicate_precedes_later_descendant_and_keeps_complete_diagnostic() {
    let program = program();
    let mut function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "fact.read")
        .unwrap()
        .clone();
    let root = function.body.id.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let template = tail.as_ref().clone();
    let mut late = template.clone();
    for depth in 0..520 {
        let mut parent = template.clone();
        parent.id = ExpressionId::from_owned(format!("arena.later.depth.{depth}"));
        parent.kind = ResolvedExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(late),
        };
        late = parent;
    }
    tail.id = root;
    tail.kind = ResolvedExprKind::Unary {
        op: crate::ast::UnaryOp::Neg,
        value: Box::new(late),
    };
    let old = expression_facts(&function).err().unwrap();
    let current = build(
        &function,
        &mut PhysicalReservation::new(usize::MAX),
        &mut ReplayBudget::unobserved(),
    )
    .err()
    .unwrap();
    assert_eq!(old, current);
}
#[test]
fn arena_rotations_equal_text_keys_and_order_match_full_tree() {
    let program = program();
    let function = &program.functions[0];
    for reverse in [false, true] {
        let mut physical = PhysicalReservation::new(usize::MAX);
        let mut capacity =
            capacity::FactCapacity::with_payer(function, capacity::Payer::Selected(&mut physical))
                .unwrap();
        let mut arena = super::super::arena::Arena::new(&mut capacity).unwrap();
        let mut old = BTreeMap::new();
        let mut budget = ReplayBudget::unobserved();
        for at in 0..257 {
            let value = if reverse { 256 - at } else { at };
            let key = ExpressionId::from_owned(format!("λ.key.{value}"));
            arena.prepare_growth_work(function, &mut budget).unwrap();
            arena.reserve_next(&mut capacity).unwrap();
            arena
                .prepare_insert_work(function, &key, &mut budget)
                .unwrap();
            assert!(arena.insert(key.clone(), None, &mut capacity).unwrap());
            old.insert(key, None);
            assert!(arena.balanced());
        }
        same_lookup(&old, &arena);
        let key = ExpressionId::from_owned("λ.key.100".into());
        arena
            .prepare_insert_work(function, &key, &mut budget)
            .unwrap();
        assert!(!arena.insert(key, None, &mut capacity).unwrap());
        assert_eq!(arena.len(), 257);
    }
}

#[test]
fn complete_original_producers_receivers_and_full_arena_dispatch_are_source_locked() {
    const OLD: &str =
        include_str!("../../../../../docs/evidence/expression-fact-arena/original_facts.txt");
    const OLD_CAPACITY: &str =
        include_str!("../../../../../docs/evidence/expression-fact-arena/original_capacity.txt");
    const RECEIVERS: &str =
        include_str!("../../../../../docs/evidence/expression-fact-arena/original_receivers.txt");
    const CURRENT: &str = include_str!("../../expression_facts.rs");
    const CAPACITY: &str = include_str!("../capacity.rs");
    let capacity_before_types = capacity::type_cursor_tests::immediate_before(CAPACITY);
    const SAVED: &str = include_str!("../saved.rs");
    const REPLAY: &str = include_str!("../../../replay.rs");
    const PRODUCER: &str = include_str!("../arena/producer.rs");
    fn item<'a>(source: &'a str, name: &str) -> &'a str {
        let start = source.find(name).expect("complete owning definition");
        let tail = &source[start..];
        let end = tail.find("\n}\n").expect("complete column-zero owner");
        &tail[..end + 2]
    }
    for name in [
        "pub(super) fn expression_facts<'hir>",
        "pub(super) fn expression_facts_with_reservation<'hir>",
        "fn with_policy<'hir",
        "fn with_capacity<'hir",
        "pub(super) fn paid<'hir>",
        "fn collect_expression_facts<'a",
    ] {
        assert_eq!(item(OLD, name), item(CURRENT, name));
    }
    assert_eq!(
        item(OLD, "pub(super) fn with_budget<'hir>"),
        item(SAVED, "pub(super) fn with_budget<'hir>")
    );
    for receiver in RECEIVERS.split("\n\nfn ") {
        let receiver = receiver.strip_prefix("fn ").unwrap_or(receiver);
        assert!(
            REPLAY.contains(receiver),
            "complete original receiving clause changed"
        );
    }
    assert!(capacity_before_types.starts_with(OLD_CAPACITY.trim_end().strip_suffix('}').unwrap()));
    let old_dispatch = item(OLD, "fn collect_expression_facts<'a");
    let old_match = old_dispatch
        .split("let fact = match &current.kind {")
        .nth(1)
        .unwrap()
        .split("            };\n")
        .next()
        .unwrap();
    let new_match = PRODUCER
        .split("let fact = match &current.kind {")
        .nth(1)
        .unwrap()
        .split("            };\n")
        .next()
        .unwrap();
    assert_eq!(
        old_match, new_match,
        "entire operation dispatcher and payloads"
    );
}

#[test]
fn arena_actual_builder_and_complete_saved_builder_are_independent_through_drop() {
    let program = program();
    for function in &program.functions {
        let reference = expression_facts(function).unwrap();
        for original in [true, false] {
            let (((), metrics), refused, charged) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        if original {
                            let facts = saved::with_budget(function, &mut budget).unwrap();
                            same(&reference, &facts);
                            drop(facts);
                        } else {
                            let facts = with_budget(function, &mut budget).unwrap();
                            let Facts::Arena(arena) = facts else {
                                panic!("actual builder must use arena")
                            };
                            same_lookup(&reference, &arena);
                            for (key, _) in arena.entries_paid(function, &mut budget).unwrap() {
                                assert!(arena.fact(function, key, &mut budget).unwrap().is_some());
                            }
                            drop(arena);
                        }
                        drop(budget);
                    })
                });
            assert!(!refused && !metrics.invalid);
            assert!(metrics.allocated <= charged);
            assert!(metrics.highwater <= charged);
            assert_eq!(metrics.allocated, metrics.freed);
            assert_eq!(metrics.live, 0);
        }
    }
}
