//! Complete immediate-old BTree receiver vs current numeric arena receiver.
use super::super::{Canonicalizer, Rc, ReplayBudget, before_ordered};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};

fn fixture() -> super::ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}

#[test]
fn ordered_complete_old_closures_successors_ids_signatures_and_weighted_overflow_match() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let mut old = before_ordered::Canonicalizer::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut a = ReplayBudget::unobserved();
    let mut b = ReplayBudget::unobserved();
    for root in roots[..6].iter().chain(roots[..6].iter()) {
        super::assert_closure_result(
            &current.closure(&function, *root, &mut b),
            &old.closure(&function, *root, &mut a),
        );
        assert_eq!(a.remaining, b.remaining);
        assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
    }
    // Unit singleton (borrowed), weighted singleton and general multiple input
    // states, repeated numeric keys and both multiply/add overflow frontiers.
    for state in [
        vec![],
        vec![(sink, 1)],
        vec![(roots[0], 1)],
        vec![(roots[4], 3)],
        vec![(roots[4], 2), (roots[5], 3)],
        vec![(roots[0], 1), (roots[0], 2)],
        vec![(roots[3], u64::MAX)],
        vec![(sink, u64::MAX), (sink, 1)],
        vec![(roots[0], u64::MAX), (roots[0], 1)],
        vec![(roots[3], 0), (roots[1], 7)],
    ] {
        let shared = Rc::new(state.clone());
        assert_eq!(
            current.successors(&function, &shared, &mut b),
            old.successors(&function, &shared, &mut a),
            "complete weighted target multiplicities and accepted weight",
        );
        assert_eq!(a.remaining, b.remaining);
        assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
        assert_eq!(
            current.id(&function, state.clone(), &mut b),
            old.id(&function, state, &mut a),
        );
        assert_eq!(a.remaining, b.remaining);
        assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
        assert_eq!(&current.ids, old.signatures());
        assert_eq!(&current.states, old.states());
    }
    assert_eq!(
        current.id(&function, vec![(roots[6], 1)], &mut b),
        old.id(&function, vec![(roots[6], 1)], &mut a),
        "complete cyclic diagnostic",
    );
    assert_eq!(a.remaining, b.remaining);
}

#[test]
fn ordered_complete_old_each_exposed_work_frontier_preserves_first_diagnostic_and_overflow() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    for state in [
        vec![(roots[4], 1)],
        vec![(roots[4], 2), (roots[5], 3)],
        vec![(roots[3], u64::MAX)],
        vec![(sink, u64::MAX), (sink, 1)],
        vec![(roots[6], 1)],
    ] {
        let mut old = before_ordered::Canonicalizer::new(&automaton, sink);
        let mut probe = ReplayBudget::unobserved();
        let initial = probe.remaining;
        drop(old.id(&function, state.clone(), &mut probe));
        let spent = initial - probe.remaining;
        for work in 0..=spent {
            let mut old = before_ordered::Canonicalizer::new(&automaton, sink);
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut a = ReplayBudget::unobserved();
            let mut b = ReplayBudget::unobserved();
            a.remaining = work;
            b.remaining = work;
            assert_eq!(
                current.id(&function, state.clone(), &mut b),
                old.id(&function, state.clone(), &mut a),
                "work={work}, input={state:?}",
            );
            assert_eq!(a.remaining, b.remaining);
            assert_eq!(a.skeleton_remaining, b.skeleton_remaining);
            assert!(
                current.frames.is_empty() && current.open.is_empty() && current.cursor.is_empty()
            );
        }
    }
}

#[test]
fn ordered_full_receiver_actual_reservation_allocation_peak_and_every_exit_drop() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let observe = |old: bool, bytes, state: &[(usize, u64)]| {
        with_limit_usage_refusal(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut old_receiver = before_ordered::Canonicalizer::new(&automaton, sink);
                let mut current = Canonicalizer::new(&automaton, sink);
                let result = (|| {
                    budget.reserve_array::<(usize, u64)>(
                        &function,
                        state.len(),
                        "ordered full input",
                    )?;
                    let mut input = Vec::with_capacity(state.len());
                    input.extend_from_slice(state);
                    if old {
                        old_receiver.id(&function, input, &mut budget)
                    } else {
                        current.id(&function, input, &mut budget)
                    }
                })();
                let ok = match result {
                    Ok(_) => true,
                    Err(error) => {
                        assert_eq!(error.code, "SPX-H006");
                        drop(error);
                        false
                    }
                };
                let at = current_metrics().allocated;
                drop(old_receiver);
                drop(current);
                drop(budget);
                assert_eq!(
                    current_metrics().allocated,
                    at,
                    "all result/error Drop paths allocate nothing"
                );
                ok
            })
        })
    };
    for state in [
        vec![(roots[4], 1)],
        vec![(roots[4], 2), (roots[5], 3)],
        vec![(roots[3], u64::MAX)],
        vec![(roots[6], 1)],
    ] {
        for old in [true, false] {
            let full = observe(old, usize::MAX, &state);
            assert!(full.0.0 == (state[0].0 != roots[6]) && !full.1 && full.3.is_none());
            assert!(!full.0.1.invalid && full.0.1.live == 0);
            assert_eq!(full.0.1.allocated, full.0.1.freed);
            assert!(full.0.1.allocated <= full.2 && full.0.1.highwater <= full.2);
            eprintln!(
                "ordered full old={old} input={state:?} reservation={} allocation={:?}",
                full.2, full.0.1
            );
            // Cycle diagnostics have semantic failure independently of bytes;
            // their unchanged result was compared above, including all work cuts.
            if state[0].0 == roots[6] {
                continue;
            }
            for bytes in [0, full.2 - 1, full.2] {
                let actual = observe(old, bytes, &state);
                assert_eq!(actual.0.0, bytes == full.2);
                assert_eq!(actual.1, bytes < full.2);
                assert_eq!(actual.3.is_some(), bytes < full.2);
                assert!(!actual.0.1.invalid && actual.0.1.live == 0);
                assert_eq!(actual.0.1.allocated, actual.0.1.freed);
                if bytes == 0 {
                    assert_eq!(actual.0.1.allocated, 0);
                }
            }
        }
    }
}
