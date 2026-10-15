//! Pending executable controls for the real inline producer and complete old receiver.
use super::super::{before_inline_weighted, Canonicalizer, Diagnostic, Label, Rc, ReplayBudget, ResolvedFunction};
use super::SmallWeighted;
use crate::cleanup_plan::replay::factored::{Automaton, SkeletonTerminal};
use crate::test_allocation_observer::{measure, Metrics};
use std::cmp::Ordering;
use std::collections::BTreeMap;

fn fixture() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE).1
}
fn graph(function: &ResolvedFunction, width: usize, repeats: usize) -> (Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    for _ in 0..width {
        let child = graph.end(function, &mut budget, sink, SkeletonTerminal::Failure).unwrap();
        for _ in 0..repeats {
            graph.movement(function, &mut budget, root, Label::End(SkeletonTerminal::Success), child).unwrap();
        }
    }
    (graph, sink, root)
}

#[test]
fn inline_weighted_content_eq_ord_and_state_keys_ignore_one_many_shape() {
    let one = Rc::new(SmallWeighted::One([(7, 3)]));
    let many = Rc::new(SmallWeighted::Many(vec![(7, 3)]));
    assert_eq!(one, many);
    assert_eq!(one.cmp(&many), Ordering::Equal);
    for entries in [vec![], vec![(6, 9)], vec![(7, 2)], vec![(7, 4)], vec![(7, 3), (8, 1)]] {
        let expected = [(7, 3)].as_slice().cmp(entries.as_slice());
        let other = SmallWeighted::Many(entries);
        assert_eq!(one.as_ref().cmp(&other), expected);
        assert_eq!(many.as_ref().cmp(&other), expected);
        assert_eq!(other.cmp(one.as_ref()), expected.reverse());
    }
    let mut states = BTreeMap::new();
    assert_eq!(states.insert(one, 9), None);
    assert_eq!(states.insert(many, 12), Some(9));
    assert_eq!(states.len(), 1);
}

#[test]
fn inline_weighted_immediate_complete_receiver_preserves_verdict_work_ids_and_every_state() {
    let function = fixture();
    for width in 0..=4 {
        for repeats in [1, 2, 4] {
            let (graph, sink, root) = graph(&function, width, repeats);
            let mut old = before_inline_weighted::Canonicalizer::new(&graph, sink);
            let mut current = Canonicalizer::new(&graph, sink);
            let mut a = ReplayBudget::unobserved();
            let mut b = ReplayBudget::unobserved();
            for input in [vec![], vec![(root, 1)], vec![(root, 3)], vec![(root, 1)], vec![(root, u64::MAX)], vec![(sink, u64::MAX), (sink, 1)]] {
                assert_eq!(current.id(&function, input.clone(), &mut b), old.id(&function, input, &mut a));
                assert_eq!((b.remaining, b.skeleton_remaining), (a.remaining, a.skeleton_remaining));
                assert_eq!(&current.ids, old.signatures());
                super::assert_states(&current.states, old.states());
                assert!(current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
            }
        }
    }
}

#[test]
fn inline_weighted_real_successors_accumulate_promote_and_keep_one_representation_only_for_one_target() {
    let function = fixture();
    for width in 0..=4 {
        for repeats in [1, 2, 4] {
            let (graph, sink, root) = graph(&function, width, repeats);
            let mut a = ReplayBudget::unobserved();
            let mut b = ReplayBudget::unobserved();
            let mut old = before_inline_weighted::Canonicalizer::new(&graph, sink);
            let mut current = Canonicalizer::new(&graph, sink);
            let input = Rc::new(SmallWeighted::Many(vec![(root, 1)]));
            let old_input = Rc::new(vec![(root, 1)]);
            let actual = current.successors(&function, &input, &mut b);
            let expected = old.successors(&function, &old_input, &mut a);
            super::assert_successors(&actual, &expected);
            assert_eq!((a.remaining, a.skeleton_remaining), (b.remaining, b.skeleton_remaining));
            let (_, targets) = actual.unwrap().unwrap();
            if width == 0 { assert!(targets.is_empty()); }
            else {
                assert_eq!(targets.len(), 1);
                assert_eq!(targets[0].1.len(), width);
                assert!(targets[0].1.iter().all(|(_, weight)| *weight == repeats as u64));
                assert_eq!(matches!(&*targets[0].1, SmallWeighted::One(_)), width == 1);
                assert!(targets[0].1.windows(2).all(|pair| pair[0].0 < pair[1].0));
            }
        }
    }
}

#[test]
fn inline_weighted_every_work_cut_cycle_and_multiply_or_add_overflow_keep_complete_old_diagnostics() {
    let function = fixture();
    let (mut graph, sink, root) = graph(&function, 2, 2);
    let mut build = ReplayBudget::unobserved();
    let cycle = graph.node(&function, &mut build).unwrap();
    graph.movement(&function, &mut build, cycle, Label::End(SkeletonTerminal::Success), cycle).unwrap();
    for input in [vec![(root, 1)], vec![(root, u64::MAX)], vec![(sink, u64::MAX), (sink, 1)], vec![(cycle, 1)]] {
        let mut probe = ReplayBudget::unobserved();
        let before = probe.remaining;
        drop(before_inline_weighted::Canonicalizer::new(&graph, sink).id(&function, input.clone(), &mut probe));
        let used = before - probe.remaining;
        for units in 0..=used {
            let mut a = ReplayBudget::unobserved(); let mut b = ReplayBudget::unobserved();
            a.remaining = units; b.remaining = units;
            let mut old = before_inline_weighted::Canonicalizer::new(&graph, sink);
            let mut current = Canonicalizer::new(&graph, sink);
            assert_eq!(current.id(&function, input.clone(), &mut b), old.id(&function, input.clone(), &mut a));
            assert_eq!((a.remaining, a.skeleton_remaining), (b.remaining, b.skeleton_remaining));
            assert!(current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
        }
    }
}

fn probe(function: &ResolvedFunction, graph: &Automaton, sink: usize, root: usize, limit: usize, old: bool) -> (Result<usize, ()>, Metrics) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    let admission = budget.physical.as_ref().unwrap().charged();
    measure(|| {
        let result = (|| {
            budget.reserve_array::<(usize, u64)>(function, 1, "inline weighted probe root")?;
            let id = if old {
                before_inline_weighted::Canonicalizer::new(graph, sink).id(function, vec![(root, 1)], &mut budget)?
            } else {
                Canonicalizer::new(graph, sink).id(function, vec![(root, 1)], &mut budget)?
            };
            assert!(id.is_some());
            Ok(budget.physical.as_ref().unwrap().charged() - admission)
        })();
        // Do not return a potentially heap-owning diagnostic past the observer.
        result.map_err(|error: Diagnostic| { assert_eq!(error.code, "SPX-H006"); drop(error); })
    })
}

#[test]
fn inline_weighted_real_allocations_debit_peak_and_drop_include_many_header_and_singleton_backing() {
    let function = fixture();
    for width in 0..=4 {
        let (graph, sink, root) = graph(&function, width, 2);
        let (old, old_metrics) = probe(&function, &graph, sink, root, 1 << 24, true);
        let (current, current_metrics) = probe(&function, &graph, sink, root, 1 << 24, false);
        let old = old.unwrap(); let current = current.unwrap();
        for (paid, metrics) in [(old, old_metrics), (current, current_metrics)] {
            assert!(!metrics.invalid && metrics.allocated <= paid && metrics.highwater <= paid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        if width == 1 { assert!(current_metrics.allocations < old_metrics.allocations); }
        eprintln!("inline-weighted width={width} old={old}/{old_metrics:?} current={current}/{current_metrics:?}");
        let admission = ReplayBudget::with_physical(&function, 1 << 24).unwrap().physical.as_ref().unwrap().charged();
        for bytes in [admission, admission + current - 1, admission + current] {
            let (result, metrics) = probe(&function, &graph, sink, root, bytes, false);
            assert_eq!(result.is_ok(), bytes == admission + current);
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            drop(result);
        }
    }
}

#[test]
fn inline_weighted_last_shared_owner_keeps_backing_live_until_drop_and_preserves_copy_work() {
    let function = fixture(); let (graph, sink, root) = graph(&function, 1, 1);
    let mut budget = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let admission = budget.physical.as_ref().unwrap().charged();
    let ((), metrics) = measure(|| {
        budget.reserve_array::<(usize, u64)>(&function, 1, "inline last owner root").unwrap();
        let input = super::super::own_weighted(&function, &mut budget, vec![(root, 1)]).unwrap();
        let mut current = Canonicalizer::new(&graph, sink);
        let (_, targets) = current.successors(&function, &input, &mut budget).unwrap().unwrap();
        let work = budget.remaining;
        let state = super::super::share_weighted(&function, &mut budget, &targets[0].1).unwrap();
        assert_eq!(work - budget.remaining, state.len() + 1);
        assert!(matches!(&*state, SmallWeighted::One(_)));
        assert!(Rc::ptr_eq(&state, &targets[0].1));
        drop(targets); drop(current); drop(input);
        assert!(crate::test_allocation_observer::current_metrics().live > 0);
        drop(state);
    });
    let paid = budget.physical.as_ref().unwrap().charged() - admission;
    assert!(!metrics.invalid && metrics.allocated <= paid && metrics.highwater <= paid);
    assert_eq!(metrics.live, 0); assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn inline_weighted_moved_many_exact_enum_controls_and_actual_rc_layout_precede_construction() {
    use std::mem::size_of;
    let function = fixture();
    let vector = size_of::<(usize, u64)>();
    let controls = 2 * size_of::<SmallWeighted>()
        + size_of::<Result<super::super::SharedWeighted, Diagnostic>>()
        + size_of::<Result<(), Diagnostic>>();
    let header = super::super::physical_clone::rc_backing_bytes::<SmallWeighted>();
    let admission = ReplayBudget::with_physical(&function, 1 << 20).unwrap().physical.as_ref().unwrap().charged();
    for available in [vector, vector + controls - 1, vector + controls + header - 1, vector + controls + header] {
        let mut budget = ReplayBudget::with_physical(&function, admission + available).unwrap();
        let ((), allocation) = measure(|| {
            budget.reserve_array::<(usize, u64)>(&function, 1, "inline enum exact root").unwrap();
            let output = super::super::own_weighted(&function, &mut budget, vec![(9, 7)]);
            assert_eq!(output.is_ok(), available == vector + controls + header);
            if let Ok(state) = output {
                assert!(matches!(&*state, SmallWeighted::Many(_)));
                assert_eq!(state.as_slice(), &[(9, 7)]);
                drop(state);
            } else { drop(output); }
        });
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        let expected = vector + if available == vector + controls + header { header } else { 0 };
        assert_eq!(allocation.allocated, expected, "only actually funded Vec/Rc layouts allocate");
        assert_eq!(allocation.highwater, expected);
        assert_eq!(budget.physical.as_ref().unwrap().charged() - admission,
            vector + if available >= vector + controls { controls } else { 0 }
                + if available == vector + controls + header { header } else { 0 });
    }
}

#[test]
fn inline_weighted_promoted_many_overflow_precedes_later_cyclic_label() {
    let function = fixture();
    let (mut graph, sink, root) = graph(&function, 2, 1);
    let first = graph.nodes[root].moves[0].1;
    let mut build = ReplayBudget::unobserved();
    graph.movement(&function, &mut build, root, Label::End(SkeletonTerminal::Success), first).unwrap();
    graph.movement(&function, &mut build, root, Label::End(SkeletonTerminal::Failure), root).unwrap();
    let mut a = ReplayBudget::unobserved(); let mut b = ReplayBudget::unobserved();
    let mut old = before_inline_weighted::Canonicalizer::new(&graph, sink);
    let mut current = Canonicalizer::new(&graph, sink);
    let expected = old.id(&function, vec![(root, u64::MAX)], &mut a);
    let actual = current.id(&function, vec![(root, u64::MAX)], &mut b);
    assert_eq!(actual, expected);
    assert_eq!(actual.unwrap(), None);
    assert_eq!((a.remaining, a.skeleton_remaining), (b.remaining, b.skeleton_remaining));
    super::assert_states(&current.states, old.states());
    assert!(current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty());
}
