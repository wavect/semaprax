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
        let old = super::super::before::sequence_skeleton_paths(vec![b], &flat, &mut work).unwrap();
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
            let result =
                (|| -> Result<Emission, Diagnostic> { emit!(vec![path], &[], work) })().unwrap();
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
