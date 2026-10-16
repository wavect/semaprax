//! Complete039b receiver and unchanged strict width0..4 owners remain live.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}
fn graph(function: &ResolvedFunction) -> (Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let leaf = graph.node(function, &mut budget).unwrap();
    (graph, sink, leaf)
}
fn same_maps(current: &Canonicalizer<'_>, old: &before_terminal_leaf::Canonicalizer<'_>) {
    assert!(current.ids.iter().eq(old.signature_entries()));
    assert!(current.states.iter().eq(old.state_entries()));
}

#[test]
fn cold_terminal_keeps_complete_old_maps_and_every_individual_work_refusal() {
    let function = fixture();
    let (graph, sink, leaf) = graph(&function);
    let mut old_budget = ReplayBudget::unobserved();
    let initial = old_budget.remaining;
    let mut old = before_terminal_leaf::Canonicalizer::new(&graph, sink);
    let expected = old.id(&function, vec![(leaf, 1)], &mut old_budget);
    assert!(expected.as_ref().unwrap().is_some());
    let work = initial - old_budget.remaining;
    assert_eq!(work, 9);
    for remaining in 0..=work {
        let mut old_budget = ReplayBudget::unobserved();old_budget.remaining = remaining;
        let mut budget = ReplayBudget::unobserved();budget.remaining = remaining;
        let mut old = before_terminal_leaf::Canonicalizer::new(&graph, sink);
        let mut current = Canonicalizer::new(&graph, sink);
        let expected = old.id(&function, vec![(leaf, 1)], &mut old_budget);
        let actual = current.id(&function, vec![(leaf, 1)], &mut budget);
        assert_eq!(actual, expected, "all six fields, work={remaining}");
        assert_eq!(budget.remaining, old_budget.remaining);
        assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        same_maps(&current, &old);
        if actual.is_ok() {
            assert!(old.scratch_was_acquired());
            assert!(!current.scratch_paid && current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
            assert_eq!(current.closures.state(&leaf), 1);
            assert_eq!(current.closures.payload_chunks(), 0);
        }
    }
}

#[test]
fn production_cold_dispatch_preserves_old_held_receiver_then_warm_and_general_roots() {
    let function = fixture();
    let (mut graph, sink, leaf) = graph(&function);
    let mut build = ReplayBudget::unobserved();
    let general = graph.node(&function, &mut build).unwrap();
    graph.movement(&function, &mut build, general, Label::End(super::super::SkeletonTerminal::Success), sink).unwrap();
    let mut old_budget = ReplayBudget::unobserved();let mut budget = ReplayBudget::unobserved();
    let mut old = before_terminal_leaf::Canonicalizer::new(&graph, sink);
    let mut current = Canonicalizer::new(&graph, sink);
    let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(&function, &old_budget);
    for root in [leaf, leaf, general, sink] {
        assert_eq!(current.id_for_comparison(&function, vec![(root, 1)], &mut budget),
            old.id_with_receiving(&function, vec![(root, 1)], &mut old_budget, &mut receiving));
        assert_eq!(budget.remaining, old_budget.remaining);
        assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        same_maps(&current, &old);
    }
}

#[test]
fn nonunit_sink_and_epsilon_shapes_keep_complete_general_kernel() {
    let function = fixture();
    let (mut graph, sink, leaf) = graph(&function);
    let mut build = ReplayBudget::unobserved();let epsilon = graph.node(&function, &mut build).unwrap();
    graph.epsilon(&function, &mut build, epsilon, leaf).unwrap();
    for (root, weight) in [(leaf, 0), (leaf, 2), (leaf, u64::MAX), (sink, 1), (epsilon, 1)] {
        let mut old_budget = ReplayBudget::unobserved();let mut budget = ReplayBudget::unobserved();
        let mut old = before_terminal_leaf::Canonicalizer::new(&graph, sink);
        let mut current = Canonicalizer::new(&graph, sink);
        assert_eq!(current.id(&function, vec![(root, weight)], &mut budget), old.id(&function, vec![(root, weight)], &mut old_budget));
        assert!(current.scratch_paid);
        assert_eq!(budget.remaining, old_budget.remaining);same_maps(&current, &old);
    }
}

#[test]
fn cold_terminal_foreign_and_restored_owners_reject_before_touching_either_work_account() {
    let function = fixture();let (graph, sink, leaf) = graph(&function);
    let mut original = ReplayBudget::unobserved();let mut foreign = ReplayBudget::unobserved();
    let mut receiver = Canonicalizer::new(&graph, sink);
    receiver.id_for_comparison(&function, vec![(leaf, 1)], &mut original).unwrap();
    let work = original.remaining;let other_work = foreign.remaining;
    let expected = Diagnostic::io("SPX-H006", String::new());
    assert_eq!(receiver.id_for_comparison(&function, vec![(leaf, 1)], &mut foreign), Err(expected.clone()));
    assert_eq!(foreign.remaining, other_work);assert_eq!(original.remaining, work);
    assert_eq!(receiver.id_for_comparison(&function, vec![(leaf, 1)], &mut original), Err(expected));
    assert_eq!(original.remaining, work);
}

fn observe(function: &ResolvedFunction, graph: &Automaton, sink: usize, leaf: usize, limit: usize, old: bool) -> (Result<usize, ()>, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget, Err(error) => { assert_eq!(error.code, "SPX-H006");return Err(()); }
        };
        let result = (|| {
            budget.reserve_array::<(usize, u64)>(function, 1, "cold leaf root")?;
            let result = if old {
                before_terminal_leaf::Canonicalizer::new(graph, sink).id(function, vec![(leaf, 1)], &mut budget)?
            } else {
                Canonicalizer::new(graph, sink).id(function, vec![(leaf, 1)], &mut budget)?
            };
            assert!(result.is_some());Ok::<_,Diagnostic>(budget.physical.as_ref().unwrap().charged())
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        let result = result.map_err(|error| { assert_eq!(error.code, "SPX-H006"); });
        drop(budget);assert_eq!(current_metrics().allocated, stopped);result
    })
}
#[test]
fn cold_terminal_actual_reservations_backing_peak_every_cut_and_final_drop() {
    let function = fixture();let (graph, sink, leaf) = graph(&function);
    let (before, old) = observe(&function, &graph, sink, leaf, 1 << 24, true);
    let (current, new) = observe(&function, &graph, sink, leaf, 1 << 24, false);
    let exact = current.unwrap();
    eprintln!("cold-terminal039b/current paid={before:?}/{exact} allocated/peak={old:?}/{new:?}");
    assert!(exact < before.unwrap(), "real absent wide destination/frame/cursor/empty payload, no fee credit");
    assert!(!old.invalid && old.live == 0 && !new.invalid && new.live == 0);
    assert_eq!(old.allocated, old.freed);assert_eq!(new.allocated, new.freed);
    for limit in 0..=exact {
        let (result, metrics) = observe(&function, &graph, sink, leaf, limit, false);
        assert_eq!(result.is_ok(), limit == exact);
        assert!(!metrics.invalid && metrics.live == 0);assert_eq!(metrics.allocated, metrics.freed);
    }
}

#[test]
fn empty_bitmap_promotion_keeps_all_empty_bits_and_refuses_before_value_construction() {
    let function = fixture();let mut budget = ReplayBudget::unobserved();
    let mut memo = empty_memo::Memo::<usize>::new();memo.authenticate(&function, &mut budget).unwrap();
    for key in 0..8 { memo.insert_empty_bound(&function, key, &mut budget).unwrap(); }
    assert_eq!(memo.payload_chunks(), 0);
    memo.insert_bound(&function, 9, &mut budget, || 42).unwrap();
    assert_eq!(memo.payload_chunks(), 1);assert_eq!(memo.get(&9), Some(&42));
    for key in 0..8 { assert_eq!(memo.state(&key), 1);assert!(memo.get(&key).is_none()); }
    memo.insert_bound(&function, 3, &mut budget, || 17).unwrap();
    assert_eq!(memo.payload_chunks(), 2);assert_eq!(memo.get(&3), Some(&17));
    for key in [0,1,2,4,5,6,7] { assert_eq!(memo.state(&key), 1); }
    // Sparse directory cost is unfavorable and must remain genuinely paid.
    memo.insert_empty_bound(&function, 512, &mut budget).unwrap();
    assert_eq!(memo.state(&512), 1);assert_eq!(memo.payload_chunks(), 2);
    let mut old_budget = ReplayBudget::unobserved();
    let mut old = before_terminal_leaf_empty_memo::Memo::<usize>::new();
    old.authenticate(&function, &mut old_budget).unwrap();
    for key in 0..8 { old.insert_empty_bound(&function, key, &mut old_budget).unwrap(); }
    old.insert_bound(&function, 9, &mut old_budget, || 42).unwrap();
    old.insert_bound(&function, 3, &mut old_budget, || 17).unwrap();
    old.insert_empty_bound(&function, 512, &mut old_budget).unwrap();
    for key in 0..=512 { assert_eq!(memo.state(&key), old.state(&key));assert_eq!(memo.get(&key), old.get(&key)); }
    assert_eq!(budget.remaining, old_budget.remaining);
    for limit in [0, 1, 255, 1024, 4096] {
        let made = std::cell::Cell::new(false);
        let Ok(mut budget) = ReplayBudget::with_physical(&function, limit) else { continue; };
        let mut memo = empty_memo::Memo::<usize>::new();
        if memo.authenticate(&function, &mut budget).is_err() { continue; }
        if memo.insert_empty_bound(&function, 0, &mut budget).is_err() { continue; }
        let result = memo.insert_bound(&function, 1, &mut budget, || { made.set(true); 42 });
        if result.is_err() {
            assert!(!made.get(), "promotion reserves every payload before make");
            memo.reject_after_error();
            let remaining = budget.remaining;
            let expected = Diagnostic::io("SPX-H006", String::new());
            assert_eq!(memo.insert(&function, 2, &mut budget, || 9), Err(expected));
            assert_eq!(budget.remaining, remaining);
        } else {
            assert!(made.get());assert_eq!(memo.state(&0), 1);assert_eq!(memo.get(&1), Some(&42));
        }
    }
}
