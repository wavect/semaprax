use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};

fn program() -> ResolvedProgram {
    let checked = crate::check(
        "module prefix_fusion; @id(\"main\") fn main()->i64 {0}",
        "prefix-fusion.spx",
    )
    .unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn seed(work: &mut SkeletonWork<'_, '_>, count: usize) -> Result<ExprSkeletonPath, Diagnostic> {
    let mut path = empty_expr_path();
    for index in 0..count {
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression: work.function.body.id.clone(),
                value: index % 2 == 0,
            },
            "fusion seed",
        )?;
    }
    Ok(path)
}
fn inputs(
    work: &mut SkeletonWork<'_, '_>,
    len: usize,
    suffix: usize,
    count: usize,
) -> Result<(Vec<ExprSkeletonPath>, Vec<ExprSkeletonPath>), Diagnostic> {
    let prefix = seed(work, len)?;
    let mut prefixes = Vec::new();
    work.push_expr_path(&mut prefixes, prefix, "fusion prefix")?;
    let mut suffixes = Vec::new();
    for index in 0..count {
        let mut path = seed(work, suffix)?;
        path.failed = index == 1;
        path.residual = index == 2;
        path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
        work.push_expr_path(&mut suffixes, path, "fusion suffix")?;
    }
    Ok((prefixes, suffixes))
}
fn sequence(
    old: bool,
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    if old {
        before::sequence_skeleton_paths(prefixes, suffixes, work)
    } else {
        super::super::super::sequence_skeleton_paths(prefixes, suffixes, work)
    }
}
fn same_dag(
    actual: &Option<std::rc::Rc<ObservationDag>>,
    expected: &Option<std::rc::Rc<ObservationDag>>,
) {
    match (actual, expected) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            assert_eq!(a.ordinal, b.ordinal);
            assert_eq!(a.pending_bound, b.pending_bound);
            match (&a.kind, &b.kind) {
                (ObservationDagKind::Seq(a, x), ObservationDagKind::Seq(b, y)) => {
                    same_dag(a, b);
                    assert_eq!(x, y);
                }
                (ObservationDagKind::Cat(x), ObservationDagKind::Cat(y))
                | (ObservationDagKind::Alt(x), ObservationDagKind::Alt(y)) => {
                    assert_eq!(x.len(), y.len());
                    for (a, b) in x.iter().zip(y) {
                        same_dag(&Some(a.clone()), &Some(b.clone()));
                    }
                }
                _ => panic!("complete DAG kind differs"),
            }
        }
        _ => panic!("complete DAG prefix differs"),
    }
}
fn same(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(a.observations.items, b.observations.items);
        same_dag(&a.observations.prefix, &b.observations.prefix);
        assert_eq!(
            a.observations.items.capacity(),
            b.observations.items.capacity()
        );
        assert_eq!(a.owned_source, b.owned_source);
        assert_eq!(a.failed, b.failed);
        assert_eq!(a.residual, b.residual);
        assert_eq!(a.continuation, b.continuation);
    }
}
#[test]
fn ordinary_prefix_fusion_complete_previous_fields_capacity_work_and_materializations() {
    let program = program();
    let function = &program.functions[0];
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        for len in [0, 1, CONTROLS / size_of::<Observation>() + 1, 1024] {
            for suffix in [0, 1, 17] {
                for count in [0, 1, 2, 4] {
                    let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let mut current = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let mut work = SkeletonWork::new(&program, function, &mut old);
                    let (prefixes, suffixes) = inputs(&mut work, len, suffix, count).unwrap();
                    let start = skeleton_materializations();
                    let expected = sequence(true, prefixes, &suffixes, &mut work).unwrap();
                    let old_events = skeleton_materializations() - start;
                    drop((suffixes, work));
                    let mut work = SkeletonWork::new(&program, function, &mut current);
                    let (prefixes, suffixes) = inputs(&mut work, len, suffix, count).unwrap();
                    let start = skeleton_materializations();
                    let actual = sequence(false, prefixes, &suffixes, &mut work).unwrap();
                    assert_eq!(skeleton_materializations() - start, old_events);
                    same(&actual, &expected);
                    drop((suffixes, work));
                    assert_eq!(current.skeleton_spent, old.skeleton_spent);
                    assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                    drop((actual, expected, current, old));
                }
            }
        }
    });
    assert!(!refused && first.is_none());
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    okay: bool,
    paths: usize,
    first_len: usize,
    first_capacity: usize,
    work: usize,
    events: usize,
    code: Option<&'static str>,
}
fn owner(
    program: &ResolvedProgram,
    old: bool,
    len: usize,
    suffix: usize,
    count: usize,
    bytes: usize,
) -> (Outcome, Metrics, bool, usize) {
    let ((outcome, observed), refused, paid, _) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            let function = &program.functions[0];
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let start = skeleton_materializations();
            let result = (|| {
                let mut work = SkeletonWork::with_index(program, function, &mut budget, None)?;
                let (prefixes, suffixes) = inputs(&mut work, len, suffix, count)?;
                sequence(old, prefixes, &suffixes, &mut work)
            })();
            let outcome = match result {
                Ok(paths) => Outcome {
                    okay: true,
                    paths: paths.len(),
                    first_len: paths
                        .first()
                        .map_or(0, |path| path.observations.items.len()),
                    first_capacity: paths
                        .first()
                        .map_or(0, |path| path.observations.items.capacity()),
                    work: budget.skeleton_spent,
                    events: skeleton_materializations() - start,
                    code: None,
                },
                Err(error) => Outcome {
                    okay: false,
                    paths: 0,
                    first_len: 0,
                    first_capacity: 0,
                    work: budget.skeleton_spent,
                    events: skeleton_materializations() - start,
                    code: Some(error.code),
                },
            };
            drop(budget);
            outcome
        })
    });
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    (outcome, observed, refused, paid)
}
#[test]
fn ordinary_prefix_fusion_whole_owner_removes_actual_backing_and_pays_controls() {
    let program = program();
    let len = CONTROLS / size_of::<Observation>() + 257;
    for count in [2, 4, 8] {
        let (old, old_alloc, old_refused, old_paid) =
            owner(&program, true, len, 1, count, usize::MAX);
        let (current, new_alloc, new_refused, new_paid) =
            owner(&program, false, len, 1, count, usize::MAX);
        assert_eq!(current, old);
        assert!(current.okay && !old_refused && !new_refused);
        let removed = len * size_of::<Observation>();
        assert_eq!(old_alloc.allocated - new_alloc.allocated, removed);
        assert_eq!(old_paid - new_paid, removed - CONTROLS);
        assert_eq!(
            old_alloc.allocations + old_alloc.reallocations,
            new_alloc.allocations + new_alloc.reallocations + 1
        );
        assert!(new_alloc.highwater <= old_alloc.highwater);
        let (exact, _, refused, paid) = owner(&program, false, len, 1, count, new_paid);
        assert_eq!(exact, current);
        assert!(!refused && paid == new_paid);
        let (short, _, refused, _) = owner(&program, false, len, 1, count, new_paid - 1);
        assert!(!short.okay && refused);
        // Removing a real allocation changes byte admission. Equal-budget first
        // refusal identity is deliberately NOT asserted against the old owner.
        let (old_short, _, refused, _) = owner(&program, true, len, 1, count, new_paid);
        assert!(!old_short.okay && refused);
    }
    // Tiny, empty-suffix and sole-move cases decline before every new fee.
    for (len, suffix, count) in [(0, 1, 2), (1, 1, 4), (1024, 0, 2), (1024, 1, 1)] {
        let (old, old_alloc, _, old_paid) = owner(&program, true, len, suffix, count, usize::MAX);
        let (new, new_alloc, _, new_paid) = owner(&program, false, len, suffix, count, usize::MAX);
        assert_eq!(old, new);
        assert_eq!(old_alloc.allocated, new_alloc.allocated);
        assert_eq!(new_paid - old_paid, 0);
    }
}
#[test]
fn ordinary_prefix_fusion_original_work_and_sticky_failure_remain_first() {
    let program = program();
    let function = &program.functions[0];
    for remaining in [0, 1] {
        let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            for old in [true, false] {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let prefix = seed(&mut work, 1024).unwrap();
                let suffix = seed(&mut work, 1).unwrap();
                work.budget.skeleton_remaining = remaining;
                let result = (|| {
                    work.charge(1, "skeleton prefix clone")?;
                    work.budget.reserve_bytes(
                        function,
                        final_output::FINAL_SPLIT_CONTROLS,
                        "HIR final-suffix split controls",
                    )?;
                    if old {
                        work.budget.reserve_clone(
                            function,
                            &prefix.observations,
                            "skeleton prefix clone",
                        )?;
                        note_skeleton_materialization();
                        work.extend_owned_observations(
                            prefix.observations.clone(),
                            &suffix.observations,
                            "skeleton suffix clone",
                        )
                    } else {
                        Ok(after_work!(work, prefix.observations, suffix.observations))
                    }
                })()
                .unwrap_err();
                assert!(result.message.contains(if remaining == 0 {
                    "skeleton prefix clone"
                } else {
                    "skeleton suffix clone"
                }));
            }
        });
        assert!(!refused);
    }
    for bytes in [0, 1] {
        let (result, _, refused, _) = owner(&program, false, 1024, 1, 4, bytes);
        assert!(!result.okay && refused);
    }
}
#[test]
fn ordinary_prefix_fusion_frozen_receiver_is_complete_and_original_calls_survive() {
    let frozen = include_str!("../../../../../docs/evidence/ordinary-prefix-fusion-20261017/preimages/src__cleanup_plan__replay.rs.before");
    let start = frozen.find("fn sequence_skeleton_paths(\n").unwrap();
    let end = frozen[start..]
        .find("\nuse path_summary::has_active_paths;")
        .unwrap()
        + start;
    let expected = format!("//! Complete frozen fade6f4df sequencing receiver; no fusion dispatch.\nuse super::super::super::*;\npub(super) {}", &frozen[start..end]);
    let before = include_str!("before.rs");
    assert_eq!(before, expected);
    assert!(before.contains("work.clone_observations(&prefix.observations"));
    assert!(before.contains("let observations = prefix.observations.clone();"));
    assert!(before.contains("FINAL_MOVE_CONTROLS"));
    assert!(before.contains("sequence_minimum::optimized::push!"));
    assert!(!before.contains("prefix_fusion::after_work!"));
}

#[test]
fn ordinary_prefix_fusion_expired_active_builder_and_dag_suffix_keep_old_route() {
    let program = program();
    let function = &program.functions[0];
    // Preserve an authenticated builder budget across expiration of its actual
    // active account. Neither a copied prefix nor a fusion probe can revive it.
    for old in [true, false] {
        let mut budget = ReplayBudget::unobserved();
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let prefix = seed(&mut work, 1024).unwrap();
        let suffix = seed(&mut work, 1).unwrap();
        let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            *work.budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            work.budget.prepare_builder(function).unwrap();
        });
        assert!(!refused);
        let events = skeleton_materializations();
        let error = (|| {
            work.charge(1, "skeleton prefix clone")?;
            if old {
                work.budget.reserve_clone(
                    function,
                    &prefix.observations,
                    "skeleton prefix clone",
                )?;
                note_skeleton_materialization();
                work.extend_owned_observations(
                    prefix.observations.clone(),
                    &suffix.observations,
                    "skeleton suffix clone",
                )
            } else {
                Ok(after_work!(work, prefix.observations, suffix.observations))
            }
        })()
        .unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(skeleton_materializations(), events);
    }
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut current = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut old);
        let (prefixes, mut suffixes) = inputs(&mut work, 1024, 1, 2).unwrap();
        suffixes[0].observations.prefix = Some(
            work.ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))
                .unwrap(),
        );
        let expected = sequence(true, prefixes, &suffixes, &mut work).unwrap();
        drop((suffixes, work));
        let mut work = SkeletonWork::new(&program, function, &mut current);
        let (prefixes, mut suffixes) = inputs(&mut work, 1024, 1, 2).unwrap();
        suffixes[0].observations.prefix = Some(
            work.ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))
                .unwrap(),
        );
        let actual = sequence(false, prefixes, &suffixes, &mut work).unwrap();
        same(&actual, &expected);
        drop((suffixes, work));
        assert_eq!(old.skeleton_spent, current.skeleton_spent);
        drop((actual, expected, old, current));
    });
    assert!(!refused);
}

#[test]
fn ordinary_prefix_fusion_independent_oracle_keeps_complete_original_account() {
    let program = program();
    let function = &program.functions[0];
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let run = |old| {
            let scope = allocation::OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, scope.lease());
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let (prefixes, suffixes) = inputs(&mut work, 1024, 1, 4).unwrap();
            let result = sequence(old, prefixes, &suffixes, &mut work).unwrap();
            drop((suffixes, work));
            let work = (budget.skeleton_spent, budget.skeleton_remaining);
            drop(budget);
            let receipt = scope.receipt();
            drop(scope);
            (result, work, receipt)
        };
        let (expected, old_work, old_receipt) = run(true);
        let (actual, new_work, new_receipt) = run(false);
        same(&actual, &expected);
        assert_eq!(new_work, old_work);
        assert_eq!(new_receipt, old_receipt);
        drop((actual, expected));
    });
    assert!(!refused);
}
