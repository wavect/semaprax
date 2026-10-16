use super::super::*;
use super::before_transfer_current as before;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

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

fn inputs(
    work: &mut SkeletonWork<'_, '_>,
    suffix_count: usize,
    tail: usize,
) -> Result<(Vec<ExprSkeletonPath>, Vec<ExprSkeletonPath>, usize), Diagnostic> {
    let mut prefixes = Vec::new();
    for state in 0..1 {
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
fn final_fanout_keeps_complete_paths_and_reports_small_wide_transfer_costs() {
    let program = program();
    let function = &program.functions[0];
    for tail in [0, 8, super::FINAL_MINIMUM_ITEMS + 3, 512] {
        for fanout in [0, 1, 2, 3, 9] {
            let observe = |old| {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        let (prefixes, suffixes, _) = inputs(&mut work, fanout, tail).unwrap();
                        let materializations = skeleton_materializations();
                        let paths = sequence(old, prefixes, &suffixes, &mut work).unwrap();
                        let logical = (
                            work.budget.skeleton_spent,
                            skeleton_materializations() - materializations,
                        );
                        let capacities = paths
                            .iter()
                            .map(|p| p.observations.items.capacity())
                            .sum::<usize>();
                        let witness = paths.iter().fold(
                            (0usize, 0usize, 0usize),
                            |(n, failed, residual), p| {
                                (
                                    n + p.observations.items.len(),
                                    failed + usize::from(p.failed),
                                    residual + usize::from(p.residual),
                                )
                            },
                        );
                        drop((paths, suffixes));
                        drop(work);
                        drop(budget);
                        (witness, logical, capacities)
                    })
                })
            };
            let ((expected, old_allocation), old_refused, old_paid, old_first) = observe(true);
            let ((actual, allocation), refused, paid, first) = observe(false);
            assert!(!old_refused && !refused && old_first.is_none() && first.is_none());
            assert_eq!(actual.0, expected.0);
            assert_eq!(actual.1, expected.1);
            assert!(!allocation.invalid && allocation.live == 0);
            assert!(!old_allocation.invalid && old_allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert_eq!(old_allocation.allocated, old_allocation.freed);
            eprintln!("final fanout={fanout} tail={tail} old/current reserved={old_paid}/{paid} heap={old_allocation:?}/{allocation:?} capacities={}/{}", expected.2, actual.2);
            // The complete observation values, including Rc identity semantics,
            // are compared separately, outside the measured summary allocation.
            let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let mut old_work = SkeletonWork::new(&program, function, &mut old);
            let mut work = SkeletonWork::new(&program, function, &mut current);
            let (prefixes, suffixes, _) = inputs(&mut old_work, fanout, tail).unwrap();
            let expected = sequence(true, prefixes, &suffixes, &mut old_work).unwrap();
            let (prefixes, suffixes, pointer) = inputs(&mut work, fanout, tail).unwrap();
            let actual = sequence(false, prefixes, &suffixes, &mut work).unwrap();
            if fanout > 1 && tail >= super::FINAL_MINIMUM_ITEMS {
                assert_eq!(
                    actual.last().unwrap().observations.items.as_ptr() as usize,
                    pointer,
                    "actual final output keeps the original paid flat backing"
                );
            }
            same_paths(&actual, &expected);
            drop((actual, expected, suffixes));
            drop((old_work, work));
            drop((old, current));
        }
    }
    // These genuine stopped prefixes retain all fields and publish exactly
    // once; the active-only cost corpus above makes no mixed-state claim.
    for (failed, residual) in [(true, false), (false, true)] {
        let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut old_work = SkeletonWork::new(&program, function, &mut old);
        let mut work = SkeletonWork::new(&program, function, &mut current);
        let (mut prefixes, suffixes, _) = inputs(&mut old_work, 3, 16).unwrap();
        prefixes[0].failed = failed;
        prefixes[0].residual = residual;
        let expected = sequence(true, prefixes, &suffixes, &mut old_work).unwrap();
        let (mut prefixes, suffixes, _) = inputs(&mut work, 3, 16).unwrap();
        prefixes[0].failed = failed;
        prefixes[0].residual = residual;
        let actual = sequence(false, prefixes, &suffixes, &mut work).unwrap();
        same_paths(&actual, &expected);
        assert_eq!(actual.len(), 1);
        assert_eq!(work.budget.skeleton_spent, old_work.budget.skeleton_spent);
        drop((actual, expected, suffixes));
        drop((old_work, work));
        drop((old, current));
    }
}

#[test]
fn final_fanout_original_work_wins_competing_split_and_header_floors() {
    use crate::bounded_output::{
        active_remaining, set_active_floor, with_limit_usage_refusal_site,
    };
    let program = program();
    let function = &program.functions[0];
    const LIMIT: usize = 1 << 24;
    // The immutable caller and seeds are independently constructed before the
    // observation account. No setup scratch is mislabeled receiving savings.
    let run = |old, work_limit, floor_offset: Option<usize>| {
        let mut caller = ReplayBudget::unobserved();
        let mut work = SkeletonWork::new(&program, function, &mut caller);
        let (prefixes, suffixes, _) = inputs(&mut work, 3, super::FINAL_MINIMUM_ITEMS + 3).unwrap();
        let observed = with_limit_usage_refusal_site(LIMIT, || {
            measure(|| {
                *work.budget = ReplayBudget::new();
                work.budget.prepare_builder(function).unwrap();
                work.budget.reserve_skeleton(function, work_limit).unwrap();
                super::FINAL_FRONTIERS.with(|slot| slot.set([None, None]));
                let initial = active_remaining().unwrap();
                if let Some(offset) = floor_offset {
                    assert!(set_active_floor(initial - offset));
                }
                let outcome = sequence(old, prefixes, &suffixes, &mut work).map_err(failure);
                let frontiers = super::FINAL_FRONTIERS.with(|slot| slot.get());
                let spent = work.budget.skeleton_spent;
                let retry = if outcome.is_err() {
                    Some(
                        work.budget
                            .reserve_bytes(function, 0, "fanout sticky retry")
                            .map_err(failure),
                    )
                } else {
                    None
                };
                let outcome = outcome.map(|paths| {
                    let len = paths.len();
                    drop(paths);
                    len
                });
                // Paid result/budget and every partial path are dropped in scope.
                *work.budget = ReplayBudget::unobserved();
                (outcome, retry, initial, frontiers, spent)
            })
        });
        drop(suffixes);
        drop(work);
        drop(caller);
        observed
    };
    let (((outcome, _, initial, frontiers, _), allocation), refused, _, first, _) =
        run(false, MAX_REPLAY_WORK_UNITS, None);
    assert!(outcome.is_ok() && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    for at in 0..2 {
        let (remaining, spent) = frontiers[at].expect("both actual multi-suffix boundaries");
        let offset = initial - remaining;
        let (((outcome, retry, _, reached, actual_spent), allocation), _, _, _, _) =
            run(false, spent, Some(offset));
        let actual = outcome.unwrap_err();
        // The independent complete old receiver reaches the same original work
        // failure with an ample physical account. No expected text is guessed.
        let (((expected, _, _, _, _), old_allocation), _, _, _, _) = run(true, spent, None);
        let expected = expected.unwrap_err();
        assert!(!old_allocation.invalid && old_allocation.live == 0);
        assert_eq!(old_allocation.allocated, old_allocation.freed);
        assert_eq!(
            actual, expected,
            "all six Diagnostic fields at frontier {at}"
        );
        assert!(reached[at].is_some());
        assert_eq!(actual_spent, spent);
        assert!(retry.unwrap().is_err(), "selected failure remains sticky");
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}
