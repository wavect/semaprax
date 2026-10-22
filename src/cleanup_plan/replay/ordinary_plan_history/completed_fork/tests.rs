//! Authored prerequisite controls; no shipping/application qualification claim.
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

fn call(
    prefixes: Vec<ExprSkeletonPath>,
    forks: &[&[ExprSkeletonPath]],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Outcome, Diagnostic> {
    produce!(prefixes, forks, work)
}
pub(super) fn fixture() -> ResolvedProgram {
    super::super::tests::program()
}
pub(super) fn seed(work: &mut SkeletonWork<'_, '_>, len: usize, offset: usize) -> ExprSkeletonPath {
    let mut path = skeleton_work::empty_expr_path();
    for index in 0..len {
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression: work.function.body.id.clone(),
                value: (index + offset) % 2 == 0,
            },
            "completed fork fixture observation",
        )
        .unwrap();
    }
    path
}
pub(super) fn inputs(
    work: &mut SkeletonWork<'_, '_>,
    len: usize,
    count: usize,
) -> (Vec<ExprSkeletonPath>, Vec<ExprSkeletonPath>) {
    let mut prefixes = Vec::new();
    for index in 0..2 {
        let mut path = seed(work, len, index);
        path.failed = index == 1;
        work.push_expr_path(&mut prefixes, path, "completed fork fixture prefix")
            .unwrap();
    }
    let mut suffixes = Vec::new();
    for index in 0..count {
        let mut path = seed(work, index % 3, index);
        path.failed = index == 2;
        path.residual = index == 1;
        work.push_expr_path(&mut suffixes, path, "completed fork fixture suffix")
            .unwrap();
    }
    (prefixes, suffixes)
}
fn complete(outcome: Outcome) -> Histories {
    match outcome {
        Outcome::Complete(set) => set,
        Outcome::Unsupported(_) => panic!("closed fixture"),
    }
}
fn assert_projection(set: &Histories, flat: &[ExprSkeletonPath], work: &mut SkeletonWork<'_, '_>) {
    assert_eq!(set.paths.len(), flat.len());
    for (path, old) in set.paths.iter().zip(flat) {
        assert_eq!(path.tail.failed, old.failed);
        assert_eq!(path.tail.residual, old.residual);
        assert_eq!(path.tail.owned_source, old.owned_source);
        assert_eq!(path.tail.continuation, old.continuation);
        work.budget
            .reserve_bytes(
                work.function,
                4 * size_of::<content::Cursor<'static>>() + PATH,
                "test complete projection cursor",
            )
            .unwrap();
        let mut cursor = content::prepare(set, path, work).unwrap();
        assert!(old.observations.prefix.is_none());
        for value in &old.observations.items {
            assert_eq!(
                content::next(&mut cursor, work).unwrap(),
                Some(value.as_ref())
            );
        }
        assert_eq!(content::next(&mut cursor, work).unwrap(), None);
    }
}
#[test]
fn complete_immediate_sequence_source_and_every_prior_projection_survive() {
    use sha2::{Digest, Sha256};
    let saved = include_str!("sequence.before.txt");
    assert_eq!(include_str!("before.rs").strip_prefix("//! Entire immediate predecessor sequence; visibility and namespace only.\nuse super::super::super::*;\n\n").unwrap(), saved.replacen("fn sequence_skeleton_paths(", "pub(super) fn sequence_skeleton_paths(", 1));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(saved.as_bytes()))
        ),
        "cff2df44e82c02e10be3996d60aa894c1419b657408d09e9a348b4fac5ee7b7d"
    );
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        for len in [0, 1, 7, 127] {
            for count in [0, 1, 2, 5] {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut old_work = SkeletonWork::new(&program, function, &mut budget);
                let (prefixes, suffixes) = inputs(&mut old_work, len, count);
                let first =
                    before::sequence_skeleton_paths(prefixes, &suffixes, &mut old_work).unwrap();
                let expected =
                    before::sequence_skeleton_paths(first, &suffixes, &mut old_work).unwrap();
                drop(old_work);
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let (prefixes, suffixes) = inputs(&mut work, len, count);
                let set = complete(call(prefixes, &[&suffixes, &suffixes], &mut work).unwrap());
                assert_projection(&set, &expected, &mut work);
                // Every prefix's observation handles exist only once per real
                // completed fork, while path multiplicity is unchanged.
                if count >= 2 && len > 0 {
                    assert!(set.chunks.len() < set.paths.len());
                }
            }
        }
    });
}
fn flat_paths(paths: Vec<SkeletonPath>) -> Vec<ExprSkeletonPath> {
    paths
        .into_iter()
        .map(|path| {
            assert_ne!(path.terminal, SkeletonTerminal::LoopLatch);
            ExprSkeletonPath {
                observations: path.observations,
                failed: path.terminal == SkeletonTerminal::Failure,
                residual: false,
                owned_source: None,
                continuation: lexical_skeleton::Continuation::Normal,
            }
        })
        .collect()
}
#[test]
fn independently_built_real_hir_and_plan_keep_full_lexicographic_content_and_multiplicity() {
    let program = fixture();
    with_limit_usage_refusal(usize::MAX, || {
        for function in &program.functions {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let hir = hir_skeleton_paths(&program, function, &mut budget).unwrap();
            let plan = super::super::before::produce(function, &mut budget).unwrap();
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let left = complete(call(flat_paths(hir), &[], &mut work).unwrap());
            let mut right = complete(call(flat_paths(plan), &[], &mut work).unwrap());
            right.paths.reverse();
            assert!(content::equivalent(&left, &right, &mut work).unwrap());
            if !right.paths.is_empty() {
                right.paths.pop();
                assert!(!content::equivalent(&left, &right, &mut work).unwrap());
            }
        }
    });
}
#[test]
fn equal_bytes_distinct_owners_chunk_boundaries_and_forged_links_do_not_grant_equality() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let (p, s) = inputs(&mut work, 7, 3);
        let left = complete(call(p, &[&s, &s], &mut work).unwrap());
        let (p, s) = inputs(&mut work, 7, 3);
        let mut right = complete(call(p, &[&s, &s], &mut work).unwrap());
        assert!(!Rc::ptr_eq(
            &left.chunks[0].items[0],
            &right.chunks[0].items[0]
        ));
        assert!(content::equivalent(&left, &right, &mut work).unwrap());
        right.chunks[0].items[0] = Rc::new(SkeletonObservation::Boolean {
            expression: function.body.id.clone(),
            value: false,
        });
        assert!(!content::equivalent(&left, &right, &mut work).unwrap());
        right.chunks[0].previous = Some(0);
        assert_eq!(
            content::equivalent(&left, &right, &mut work)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    });
}
#[test]
fn original_payer_exact_short_zero_floor_and_full_partial_drop_are_retained() {
    let program = fixture();
    let function = &program.functions[0];
    let run = |limit, floor| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                assert!(set_active_floor(floor));
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                // All input construction, candidate and final Drop remain
                // inside the same original payer and allocation observer.
                let result = (|| {
                    let mut prefix = skeleton_work::empty_expr_path();
                    work.push_observation(
                        &mut prefix,
                        SkeletonObservation::Boolean {
                            expression: function.body.id.clone(),
                            value: true,
                        },
                        "physical fixture observation",
                    )?;
                    let mut prefixes = Vec::new();
                    work.push_expr_path(&mut prefixes, prefix, "physical fixture prefix")?;
                    let suffixes = [
                        skeleton_work::empty_expr_path(),
                        skeleton_work::empty_expr_path(),
                    ];
                    let result = call(prefixes, &[&suffixes], &mut work)?;
                    drop(result);
                    Ok::<(), Diagnostic>(())
                })();
                drop(work);
                drop(budget);
                // Drop every actual failure Diagnostic inside the same observer.
                result.map_err(|error| {
                    let code = error.code;
                    drop(error);
                    code
                })
            })
        })
    };
    let ((result, metrics), refused, paid, _) = run(usize::MAX, 0);
    result.unwrap();
    assert!(!refused && !metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid);
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
fn unsupported_dags_and_selected_contexts_keep_the_complete_original_input() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        for selected in [false, true] {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let mut path = seed(&mut work, 3, 0);
            if selected {
                work.budget.admission = AdmissionMode::SelectedRetained;
            } else {
                path.observations.prefix = Some(Rc::new(skeleton_work::ObservationDag::legacy(
                    skeleton_work::ObservationDagKind::Alt(Vec::new()),
                )));
            }
            let original = Rc::clone(&path.observations.items[0]);
            let prefixes = vec![path];
            let unsupported = call(prefixes, &[], &mut work).unwrap();
            match unsupported {
                Outcome::Unsupported(paths) => {
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0].observations.items.len(), 3);
                    assert!(Rc::ptr_eq(&original, &paths[0].observations.items[0]));
                    assert_eq!(paths[0].observations.prefix.is_none(), selected);
                }
                Outcome::Complete(_) => panic!("unsupported input acquired no history authority"),
            }
        }
    });
}

#[test]
fn every_new_work_refusal_is_deterministic_in_all_six_diagnostic_fields() {
    let program = fixture();
    let function = &program.functions[0];
    let attempt = |cut| {
        with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let (prefixes, suffixes) = inputs(&mut work, 3, 3);
            work.budget.remaining = cut;
            call(prefixes, &[&suffixes, &suffixes], &mut work).map(|outcome| drop(outcome))
        })
        .0
    };
    for cut in [0, 1, 2, 4, 8, 16, 32, 64] {
        let left = attempt(cut);
        let right = attempt(cut);
        match (left, right) {
            (Ok(()), Ok(())) => {}
            (Err(a), Err(b)) => {
                assert_eq!(
                    (a.code, a.severity, a.message, a.path, a.span, a.help),
                    (b.code, b.severity, b.message, b.path, b.span, b.help)
                );
            }
            _ => panic!("work cut changed outcome"),
        }
    }
}
