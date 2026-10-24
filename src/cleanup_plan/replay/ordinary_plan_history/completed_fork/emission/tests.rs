//! Source-authored controls; execution and shipping admission remain pending.
use super::super::tests::{fixture, inputs, seed};
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

fn call(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    match emit!(prefixes, suffixes, work)? {
        Emission::Complete(paths) => Ok(paths),
        Emission::Unsupported(_) => panic!("closed sequence fixture"),
    }
}
fn equal_paths(
    actual: &[ExprSkeletonPath],
    old: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> bool {
    if actual.len() != old.len() {
        return false;
    }
    for (actual, old) in actual.iter().zip(old) {
        if (
            &actual.owned_source,
            actual.failed,
            actual.residual,
            &actual.continuation,
        ) != (
            &old.owned_source,
            old.failed,
            old.residual,
            &old.continuation,
        ) {
            return false;
        }
        work.budget
            .reserve_bytes(
                work.function,
                2 * sequence::CURSOR_STORAGE + 2 * sequence::CURSOR_CALL,
                "complete history paired oracle cursors",
            )
            .unwrap();
        let mut left = sequence::Cursor::new();
        let mut right = sequence::Cursor::new();
        left.prepare(&actual.observations, work.function, work.budget)
            .unwrap();
        right
            .prepare(&old.observations, work.function, work.budget)
            .unwrap();
        loop {
            work.budget
                .reserve_bytes(
                    work.function,
                    2 * sequence::CURSOR_CALL,
                    "complete history paired oracle next",
                )
                .unwrap();
            let a = left.next(work.function, work.budget).unwrap();
            let b = right.next(work.function, work.budget).unwrap();
            if a != b {
                return false;
            }
            if a.is_none() {
                break;
            }
        }
    }
    true
}

#[test]
fn repeated_complete_forks_preserve_the_whole_immediate_predecessor() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        for len in [0, 1, 7, 127] {
            for count in [0, 1, 2, 5] {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let (mut old, suffixes) = inputs(&mut work, len, count);
                for _ in 0..3 {
                    old = super::super::before::sequence_skeleton_paths(old, &suffixes, &mut work)
                        .unwrap();
                }
                let (mut actual, suffixes) = inputs(&mut work, len, count);
                for _ in 0..3 {
                    actual = call(actual, &suffixes, &mut work).unwrap();
                }
                assert!(equal_paths(&actual, &old, &mut work));
                if len > 0 && count >= 2 {
                    assert!(actual.iter().any(|path| path.observations.prefix.is_some()));
                }
            }
        }
    });
}

#[test]
fn nested_suffix_contents_order_multiplicity_and_distinct_identity_are_preserved() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let (prefixes, suffixes) = inputs(&mut work, 3, 3);
        let nested = call(prefixes, &suffixes, &mut work).unwrap();
        let (prefixes, suffixes) = inputs(&mut work, 3, 3);
        let flat =
            super::super::before::sequence_skeleton_paths(prefixes, &suffixes, &mut work).unwrap();
        let a = seed(&mut work, 5, 1);
        let original = Rc::clone(&a.observations.items[0]);
        let actual = call(vec![a], &nested, &mut work).unwrap();
        let b = seed(&mut work, 5, 1);
        assert!(!Rc::ptr_eq(&original, &b.observations.items[0]));
        let mut old =
            super::super::before::sequence_skeleton_paths(vec![b], &flat, &mut work).unwrap();
        assert!(equal_paths(&actual, &old, &mut work));
        assert!(actual.iter().any(|path| matches!(
            path.observations.prefix.as_deref().map(|node| &node.kind),
            Some(ObservationDagKind::Cat(_))
        )));
        work.push_observation(
            &mut old[0],
            SkeletonObservation::Boolean {
                expression: function.body.id.clone(),
                value: false,
            },
            "complete history late mismatch",
        )
        .unwrap();
        assert!(!equal_paths(&actual, &old, &mut work));
    });
}

#[test]
fn unsupported_alternative_even_with_sequence_marker_keeps_original_owners() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        for selected in [false, true] {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let mut path = seed(&mut work, 2, 0);
            let owner = Rc::clone(&path.observations.items[0]);
            if selected {
                work.budget.admission = AdmissionMode::SelectedRetained;
            } else {
                let mut node = ObservationDag::legacy(ObservationDagKind::Alt(Vec::new()));
                node.ordinal = Some(sequence::SEQUENCE_MARKER);
                path.observations.prefix = Some(Rc::new(node));
            }
            let result = (|| -> Result<Emission, Diagnostic> {
                let work = &mut work;
                emit!(vec![path], &[], work)
            })()
            .unwrap();
            match result {
                Emission::Unsupported(paths) => {
                    assert!(Rc::ptr_eq(&owner, &paths[0].observations.items[0]))
                }
                Emission::Complete(_) => {
                    panic!("neither scope nor marker grants sequence authority")
                }
            }
        }
    });
}

#[test]
fn original_payer_exact_short_zero_floor_and_all_partial_owners_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let run = |limit, floor| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                assert!(set_active_floor(floor));
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let result = (|| {
                    let mut prefix = skeleton_work::empty_expr_path();
                    work.push_observation(
                        &mut prefix,
                        SkeletonObservation::Boolean {
                            expression: function.body.id.clone(),
                            value: true,
                        },
                        "complete emitter physical fixture",
                    )?;
                    let mut prefixes = Vec::new();
                    work.push_expr_path(&mut prefixes, prefix, "complete emitter physical input")?;
                    let suffixes = [
                        skeleton_work::empty_expr_path(),
                        skeleton_work::empty_expr_path(),
                    ];
                    let first = call(prefixes, &suffixes, &mut work)?;
                    let second = call(first, &suffixes, &mut work)?;
                    drop(second);
                    Ok::<(), Diagnostic>(())
                })();
                if result.is_err() {
                    // Even a zero reservation must authenticate the already-refused
                    // original lender; cleanup cannot replace the first refusal.
                    assert!(
                        work.budget
                            .reserve_bytes(function, 0, "complete emitter sticky zero")
                            .is_err()
                    );
                }
                drop(work);
                drop(budget);
                result.map_err(|error| {
                    let code = error.code;
                    drop(error);
                    code
                })
            })
        })
    };
    let ((result, heap), refused, paid, _) = run(usize::MAX, 0);
    result.unwrap();
    assert!(!refused && !heap.invalid && heap.live == 0);
    assert_eq!(heap.allocated, heap.freed);
    assert!(heap.allocated <= paid);
    for (limit, floor, success) in [
        (paid, 0, true),
        (paid - 1, 0, false),
        (0, 0, false),
        (paid, paid, false),
    ] {
        let ((result, heap), refused, _, first) = run(limit, floor);
        assert_eq!(result.is_ok(), success);
        assert!(!heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
        if !success {
            assert!(refused && first.is_some());
            assert_eq!(result.unwrap_err(), "SPX-H006");
        }
    }
}

#[test]
fn new_work_frontiers_keep_deterministic_six_field_diagnostics() {
    let program = fixture();
    let function = &program.functions[0];
    let attempt = |cut| {
        with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let (prefixes, suffixes) = inputs(&mut work, 3, 3);
            work.budget.remaining = cut;
            call(prefixes, &suffixes, &mut work).map(drop)
        })
        .0
    };
    for cut in 0..64 {
        match (attempt(cut), attempt(cut)) {
            (Ok(()), Ok(())) => {}
            (Err(a), Err(b)) => assert_eq!(
                (a.code, a.severity, a.message, a.path, a.span, a.help),
                (b.code, b.severity, b.message, b.path, b.span, b.help)
            ),
            _ => panic!("same physical producer and work cut changed outcome"),
        }
    }
}

#[test]
fn retained_fork_receivers_preserve_immediate_emitter_and_singleton_allocation() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        for len in [0, 1, 7, 127] {
            for count in [0, 1, 2, 5] {
                let mut old_budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);
                let (prefixes, suffixes) = inputs(&mut old_work, len, count);
                let old = with_immediate(|| call(prefixes, &suffixes, &mut old_work)).unwrap();
                let old_spent = old_work.budget.skeleton_spent;
                let mut new_budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut new_work = SkeletonWork::new(&program, function, &mut new_budget);
                let (prefixes, suffixes) = inputs(&mut new_work, len, count);
                let pointer = prefixes.as_ptr();
                let capacity = prefixes.capacity();
                let actual = call(prefixes, &suffixes, &mut new_work).unwrap();
                assert_eq!(old_spent, new_work.budget.skeleton_spent);
                if count == 1 {
                    assert_eq!(actual.as_ptr(), pointer);
                    assert_eq!(actual.capacity(), capacity);
                }
                assert!(equal_paths(&actual, &old, &mut new_work));
            }
        }
        // A real shared singleton, both stopped states and a flat empty suffix
        // all use the same cardinality proof; no empty input is manufactured.
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let (prefixes, suffixes) = inputs(&mut work, 7, 2);
        let shared = with_immediate(|| call(prefixes, &suffixes, &mut work)).unwrap();
        let (prefixes, _) = inputs(&mut work, 3, 0);
        let mut prefixes = prefixes;
        prefixes[1].failed = false;
        prefixes[1].residual = true;
        let pointer = prefixes.as_ptr();
        let actual = call(prefixes, &shared[..1], &mut work).unwrap();
        assert_eq!(actual.as_ptr(), pointer);
        let (mut prefixes, _) = inputs(&mut work, 3, 0);
        prefixes[1].failed = false;
        prefixes[1].residual = true;
        let old = with_immediate(|| call(prefixes, &shared[..1], &mut work)).unwrap();
        assert!(equal_paths(&actual, &old, &mut work));
    });
}

#[test]
fn retained_fork_immediate_work_cuts_and_all_diagnostic_fields_match() {
    let program = fixture();
    let function = &program.functions[0];
    // Cuts apply after the same complete paid setup. Both independent emitters
    // retain the held skeleton loan, including stopped and final-output events.
    for count in [0, 1, 2, 5] {
        let attempt = |cut, immediate| {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let (prefixes, suffixes) = inputs(&mut work, 7, count);
            work.budget.skeleton_remaining = cut;
            let before = work.budget.skeleton_spent;
            let result = if immediate {
                with_immediate(|| call(prefixes, &suffixes, &mut work))
            } else {
                call(prefixes, &suffixes, &mut work)
            }
            .map(drop);
            (result, work.budget.skeleton_spent - before)
        };
        for cut in 0..32 {
            let (old, old_work) = attempt(cut, true);
            let (new, new_work) = attempt(cut, false);
            assert_eq!(old_work, new_work);
            match (old, new) {
                (Ok(()), Ok(())) => {}
                (Err(a), Err(b)) => assert_eq!(
                    (a.code, a.severity, a.message, a.path, a.span, a.help),
                    (b.code, b.severity, b.message, b.path, b.span, b.help)
                ),
                _ => panic!("immediate held-work refusal frontier changed"),
            }
        }
    }
}

#[test]
fn retained_fork_keeps_the_entire_immediate_parent_and_old_producer() {
    let current = include_str!("../emission.rs");
    let before = retained::before_emission(current);
    let body = |s: &str| {
        let begin = s
            .find("pub(in crate::cleanup_plan::replay) fn emit_prepaid(")
            .unwrap();
        let end = s[begin..]
            .find("\n// The original caller's actual-payer pattern")
            .unwrap()
            + begin;
        s[begin..end].to_owned()
    };
    assert_eq!(body(current), body(&before));
    // Both the original public receiver invoice and the complete STEP inventory
    // stay compiled with the old body; no candidate discounts reach that arm.
    let banks = |s: &str| {
        let begin = s
            .find("pub(in crate::cleanup_plan::replay) const ENTRY:")
            .unwrap();
        let end = s.find("const FREEZE:").unwrap();
        s[begin..end].to_owned()
    };
    assert_eq!(banks(current), banks(&before));
}

#[test]
fn retained_fork_cold_empty_tiny_and_repeated_packets_disclose_actual_costs() {
    let program = fixture();
    let function = &program.functions[0];
    // Direct emitter boundary: deliberately includes cold/unsupported-empty
    // shapes that the shipping dispatcher can avoid. No app inference follows.
    for (empty, len, count, repeats) in [
        (true, 0, 0, 1),
        (false, 0, 1, 1),
        (false, 1, 2, 1),
        (false, 127, 1, 3),
        (false, 127, 5, 3),
    ] {
        let run = |immediate| {
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let result = (|| {
                        let (mut paths, suffixes) = inputs(&mut work, len, count);
                        if empty {
                            paths.clear();
                        }
                        for _ in 0..repeats {
                            paths = if immediate {
                                with_immediate(|| call(paths, &suffixes, &mut work))
                            } else {
                                call(paths, &suffixes, &mut work)
                            }?;
                        }
                        drop(paths);
                        drop(suffixes);
                        Ok::<_, Diagnostic>(())
                    })();
                    let work_used = work.budget.skeleton_spent;
                    drop(work);
                    drop(budget);
                    result.map(|()| work_used).map_err(|error| {
                        let code = error.code;
                        drop(error);
                        code
                    })
                })
            })
        };
        let ((old, old_heap), old_refused, old_paid, _) = run(true);
        let ((new, new_heap), new_refused, new_paid, _) = run(false);
        assert_eq!(old.unwrap(), new.unwrap());
        assert!(!old_refused && !new_refused);
        for (heap, paid) in [(old_heap, old_paid), (new_heap, new_paid)] {
            assert!(!heap.invalid && heap.live == 0);
            assert_eq!(heap.allocated, heap.freed);
            assert!(heap.allocated <= paid);
        }
        eprintln!("925 direct empty={} len={} suffixes={} repeats={} old_paid={} new_paid={} old_heap={:?} new_heap={:?}",
            empty,len,count,repeats,old_paid,new_paid,old_heap,new_heap);
    }
}
