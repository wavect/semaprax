use super::super::*;
use super::before_transfer_current as before;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::rc::Rc;

// The same complete corpus as the preserved inline-fusion observer, plus a
// larger flat history. Source checking/canonicalization stays outside probes.
fn program() -> ResolvedProgram {
    let mut source = String::from("module branch_inline; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}");
    source.push_str(" @id(\"long\") fn long(a:bool)->i64 {");
    for index in 0..33 {
        source.push_str(&format!("let s{index}=\"flat\";"));
    }
    source.push_str("if a {1}else{2}}");
    let checked = crate::check(&source, "branch-inline.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "branch-inline.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    let mut bytes = [0; 1024];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    let result = Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    };
    drop(value);
    result
}

fn plan(
    old: bool,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    if old {
        before::plan_skeleton_paths(function, budget)
    } else {
        super::super::plan_skeleton_paths(function, budget)
    }
}

fn inputs(
    work: &mut SkeletonWork<'_, '_>,
    suffix_count: usize,
    tail: usize,
) -> Result<(Vec<ExprSkeletonPath>, Vec<ExprSkeletonPath>, usize), Diagnostic> {
    let mut prefixes = Vec::new();
    for state in 0..4 {
        let mut path = empty_expr_path();
        for _ in 0..tail {
            work.push_observation(
                &mut path,
                SkeletonObservation::Boolean {
                    expression: work.function.body.id.clone(),
                    value: true,
                },
                "final-output seed",
            )?;
        }
        path.failed = state == 2;
        path.residual = state == 3;
        path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
        work.push_expr_path(&mut prefixes, path, "final-output prefix")?;
    }
    let pointer = prefixes[0].observations.items.as_ptr() as usize;
    let mut suffixes = Vec::new();
    for _ in 0..suffix_count {
        work.push_expr_path(&mut suffixes, empty_expr_path(), "final-output suffix")?;
    }
    Ok((prefixes, suffixes, pointer))
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
        super::super::sequence_skeleton_paths(prefixes, suffixes, work)
    }
}
fn same_paths(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.observations, expected.observations);
        assert_eq!(actual.owned_source, expected.owned_source);
        assert_eq!(actual.failed, expected.failed);
        assert_eq!(actual.residual, expected.residual);
        assert_eq!(actual.continuation, expected.continuation);
    }
}

#[test]
fn final_output_keeps_complete_hir_plan_multiplicity_work_and_failures() {
    let program = program();
    let function = &program.functions[0];
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        for suffix_count in [0, 1, 2] {
            for tail in [0, 16] {
                let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut current = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut old_work = SkeletonWork::new(&program, function, &mut old);
                let (prefixes, suffixes, _) = inputs(&mut old_work, suffix_count, tail).unwrap();
                let materializations = skeleton_materializations();
                let expected = sequence(true, prefixes, &suffixes, &mut old_work).unwrap();
                let old_materializations = skeleton_materializations() - materializations;
                drop((suffixes, old_work));
                let mut work = SkeletonWork::new(&program, function, &mut current);
                let (prefixes, suffixes, pointer) = inputs(&mut work, suffix_count, tail).unwrap();
                let marker = prefixes[0].observations.items.first().cloned();
                let materializations = skeleton_materializations();
                let actual = sequence(false, prefixes, &suffixes, &mut work).unwrap();
                assert_eq!(
                    skeleton_materializations() - materializations,
                    old_materializations
                );
                same_paths(&actual, &expected);
                if suffix_count == 1 && tail != 0 {
                    assert_eq!(actual[0].observations.items.as_ptr() as usize, pointer);
                    assert!(Rc::ptr_eq(
                        &actual[0].observations.items[0],
                        marker.as_ref().unwrap()
                    ));
                }
                drop((marker, suffixes, work));
                assert_eq!(current.skeleton_spent, old.skeleton_spent);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                drop((actual, expected, current, old));
            }
        }
        for function in &program.functions {
            let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut current = ReplayBudget::with_skeleton_limit(usize::MAX);
            let expected = plan(true, function, &mut old).unwrap();
            let actual = plan(false, function, &mut current).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(current.skeleton_spent, old.skeleton_spent);
            assert_eq!(current.remaining, old.remaining);
            let spent = current.skeleton_spent;
            assert!(spent > 1);
            drop((actual, expected, current, old));
            for work in [0, 1, spent - 1, spent] {
                let run = |old| {
                    let mut budget = ReplayBudget::with_skeleton_limit(work);
                    let materializations = skeleton_materializations();
                    let outcome = plan(old, function, &mut budget)
                        .map(|paths| {
                            let len = paths.len();
                            drop(paths);
                            len
                        })
                        .map_err(failure);
                    (
                        outcome,
                        budget.skeleton_remaining,
                        budget.skeleton_spent,
                        skeleton_materializations() - materializations,
                    )
                };
                assert_eq!(run(false), run(true));
            }
        }
        let mut hostile = program.functions[0].clone();
        let edge = hostile
            .cleanup_plan
            .edges
            .iter_mut()
            .find(|edge| !matches!(edge.condition, EdgeCondition::Always))
            .unwrap();
        edge.condition = EdgeCondition::Always;
        let run = |old| {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            failure(plan(old, &hostile, &mut budget).unwrap_err())
        };
        assert_eq!(run(false), run(true)); // every Diagnostic field, no code-only match
    });
    assert!(!refused && first.is_none());
}

#[test]
fn final_output_whole_owners_report_capacities_exact_short_zero_and_drop() {
    let program = program();
    let function = &program.functions[0];
    // This shipping gate complements, rather than replaces, the historical
    // inline-fusion allocation-equality claim. Every original corpus function
    // is measured on both complete independent enumerators.
    for function in &program.functions {
        let observe = |old, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    super::take_moves();
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let outcome = plan(old, function, &mut budget)
                        .map(|paths| {
                            let capacities = paths
                                .iter()
                                .map(|path| path.observations.items.capacity())
                                .sum::<usize>();
                            let len = paths.len();
                            drop(paths);
                            (len, capacities)
                        })
                        .map_err(failure);
                    let spent = budget.skeleton_spent;
                    drop(budget);
                    (outcome, spent, super::take_moves())
                })
            })
        };
        let ((old, old_alloc), old_refused, old_paid, old_first) = observe(true, usize::MAX);
        let ((actual, alloc), refused, paid, first) = observe(false, usize::MAX);
        eprintln!("final_output plan old={old:?} current={actual:?} old_paid={old_paid} current_paid={paid} old_backing={old_alloc:?} current_backing={alloc:?}");
        assert!(!old_refused && old_first.is_none() && !refused && first.is_none());
        assert_eq!(actual.0.as_ref().unwrap().0, old.0.as_ref().unwrap().0);
        assert_eq!(actual.1, old.1);
        assert_eq!(old.2, (0, 0, 0));
        // Each actual final move removes precisely the old fused 2*len buffer.
        // All first-sibling, observation, queue and output allocations survive.
        assert_eq!(
            old_alloc.allocated.checked_sub(alloc.allocated),
            Some(actual.2 .1)
        );
        assert_eq!(old_alloc.allocations - alloc.allocations, actual.2 .0);
        assert_eq!(old_alloc.reallocations, alloc.reallocations);
        if function.name == "long" {
            assert!(actual.2 .0 > 0 && actual.2 .1 > 0);
        }
        for metrics in [old_alloc, alloc] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        assert!(old_alloc.allocated <= old_paid && alloc.allocated <= paid);
        for limit in [0, paid - 1, paid] {
            let ((outcome, allocation), refused, _, first) = observe(false, limit);
            assert_eq!(outcome.0.is_ok(), limit == paid);
            assert_eq!(refused, limit != paid);
            assert_eq!(first.is_some(), limit != paid);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
        }
    }
    let observe = |old, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let result = (|| {
                    let mut work = SkeletonWork::with_index(&program, function, &mut budget, None)?;
                    let (prefixes, suffixes, pointer) = inputs(&mut work, 1, 128)?;
                    let input_capacity = prefixes
                        .iter()
                        .map(|path| path.observations.items.capacity())
                        .sum::<usize>();
                    let paths = sequence(old, prefixes, &suffixes, &mut work)?;
                    let capacity = paths
                        .iter()
                        .map(|path| path.observations.items.capacity())
                        .sum::<usize>();
                    if !old {
                        assert_eq!(paths[0].observations.items.as_ptr() as usize, pointer);
                    }
                    let len = paths.len();
                    drop(paths);
                    Ok::<_, Diagnostic>((len, input_capacity, capacity))
                })()
                .map_err(failure);
                let spent = budget.skeleton_spent;
                drop(budget);
                (result, spent)
            })
        })
    };
    let ((old, old_alloc), old_refused, old_paid, old_first) = observe(true, usize::MAX);
    let ((actual, allocation), refused, paid, first) = observe(false, usize::MAX);
    eprintln!("final_output HIR old={old:?} current={actual:?} old_paid={old_paid} current_paid={paid} old_backing={old_alloc:?} current_backing={allocation:?}");
    assert!(!old_refused && old_first.is_none() && !refused && first.is_none());
    assert_eq!(actual.0, old.0);
    assert_eq!(actual.1, old.1);
    for metrics in [old_alloc, allocation] {
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    // HIR seed backing is inside both probes; this is not a warmed shared input.
    assert!(allocation.allocated <= paid && old_alloc.allocated <= old_paid);
    for limit in [0, paid - 1, paid] {
        let ((result, metrics), refused, _, first) = observe(false, limit);
        assert_eq!(result.0.is_ok(), limit == paid);
        assert_eq!(refused, limit != paid);
        assert_eq!(first.is_some(), limit != paid);
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    assert_complete_old_sources();
}

// Source-only ownership checks run after every physical scope.
fn assert_complete_old_sources() {
    super::tests::assert_complete_old_sources();
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                include_str!("before_transfer_current.rs").as_bytes()
            ))
        ),
        "0f262dc78bccc47841ddf525edcb38b3c90a557fd96667bf9e82c3831145d2b0"
    );
}
