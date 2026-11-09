//! Whole prior compact producer and actual paid invocation-local frame.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};

fn program() -> ResolvedProgram {
    let source = crate::check(
        "module flat_frame; @id(\"f\") fn f(a:bool)->i64 {if a {1}else{2}} @id(\"main\") fn main()->i64 {f(true)}",
        "flat-frame.spx",
    ).unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn paths(program: &ResolvedProgram, count: usize, dag_at: Option<usize>) -> Vec<SkeletonPath> {
    let history = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(program, &program.functions[0], &mut budget);
        node(&mut work, ObservationDagKind::Seq(None, Vec::new())).unwrap()
    })
    .0;
    (0..count)
        .map(|index| {
            let mut path = empty_expr_path();
            if dag_at == Some(index) {
                path.observations.prefix = Some(history.clone());
            }
            SkeletonPath {
                observations: path.observations,
                terminal: SkeletonTerminal::Success,
            }
        })
        .collect()
}
#[derive(Debug)]
struct Run {
    result: Result<bool, String>,
    remaining: usize,
    skeleton_remaining: usize,
    skeleton_spent: usize,
    refusal_spent: bool,
    refused: bool,
    paid: usize,
    heap: Metrics,
    sticky: Option<String>,
}
fn run(
    current: bool,
    paths: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
    floor: usize,
    lent: bool,
) -> Run {
    let (
        (result, remaining, skeleton_remaining, skeleton_spent, refusal_spent, heap, sticky),
        refused,
        paid,
        _,
    ) = with_limit_usage_refusal(bytes, || {
        let mut budget = ReplayBudget::with_skeleton_limit(work);
        budget.lent_skeleton = lent;
        assert!(crate::bounded_output::set_active_floor(floor));
        let (result, heap) = measure(|| {
            budget.reserve_bytes(function, CALL, "test flat probe complete call")?;
            if current {
                all_flat(paths, function, &mut budget).map_err(ByteFailure::into_diagnostic)
            } else {
                super::super::before_flat_probe::all_flat(paths, function, &mut budget)
                    .map_err(ByteFailure::into_diagnostic)
            }
        });
        let result = result.map_err(|error| error.json());
        let remaining = budget.remaining;
        let skeleton_remaining = budget.skeleton_remaining;
        let skeleton_spent = budget.skeleton_spent;
        let refusal_spent = budget.refusal_spent;
        let sticky = if crate::bounded_output::active_refused() || refusal_spent {
            Some(
                budget
                    .reserve_compact_bytes(function, 0, "post-probe sticky control")
                    .expect_err("refused invocation cannot revive")
                    .into_diagnostic()
                    .json(),
            )
        } else {
            None
        };
        (
            result,
            remaining,
            skeleton_remaining,
            skeleton_spent,
            refusal_spent,
            heap,
            sticky,
        )
    });
    Run {
        result,
        remaining,
        skeleton_remaining,
        skeleton_spent,
        refusal_spent,
        refused,
        paid,
        heap,
        sticky,
    }
}
fn same(old: &Run, new: &Run) {
    assert_eq!(old.result, new.result);
    assert_eq!(old.remaining, new.remaining);
    assert_eq!(old.skeleton_remaining, new.skeleton_remaining);
    assert_eq!(old.skeleton_spent, new.skeleton_spent);
    assert_eq!(old.refusal_spent, new.refusal_spent);
    assert_eq!(old.refused, new.refused);
    assert_eq!(old.heap, new.heap);
    assert_eq!(old.sticky, new.sticky);
}
#[test]
fn reusable_frame_matches_whole_prior_probe_at_every_work_cut_and_lent_entry() {
    let program = program();
    let function = &program.functions[0];
    for count in [0usize, 1, 2, 7, 32, 129] {
        for dag in [None, Some(0), Some(count / 2), count.checked_sub(1)] {
            let paths = paths(&program, count, dag);
            for lent in [false, true] {
                for work in (0..count + 3).chain([usize::MAX]) {
                    let old = run(false, &paths, function, work, usize::MAX, 0, lent);
                    let new = run(true, &paths, function, work, usize::MAX, 0, lent);
                    same(&old, &new);
                }
            }
        }
    }
}
#[test]
fn reusable_frame_exact_reservation_floor_zero_and_sticky_ownership_are_preserved() {
    let program = program();
    let function = &program.functions[0];
    let old_frame = 2 * PAYMENT
        + 2 * size_of::<&mut std::slice::Iter<'static, SkeletonPath>>()
        + 2 * size_of::<Option<&SkeletonPath>>()
        + 2 * size_of::<&SkeletonPath>()
        + 2 * size_of::<&Option<NodeRc>>()
        + 2 * size_of::<bool>();
    for count in [0usize, 1, 2, 7, 32, 129] {
        for dag in [None, Some(0), Some(count / 2), count.checked_sub(1)] {
            let paths = paths(&program, count, dag);
            let old = run(false, &paths, function, usize::MAX, usize::MAX, 0, false);
            let new = run(true, &paths, function, usize::MAX, usize::MAX, 0, false);
            same(&old, &new);
            let visits = if let Some(at) = dag.filter(|at| *at < count) {
                at + 1
            } else {
                count + 1
            };
            assert_eq!(old.paid + FRAME, new.paid + visits * old_frame);
            for floor in [0, 1, 64] {
                let exact = run(
                    true,
                    &paths,
                    function,
                    usize::MAX,
                    new.paid + floor,
                    floor,
                    false,
                );
                assert_eq!(exact.result, new.result);
                assert_eq!(exact.paid, new.paid);
                assert!(!exact.refused);
                assert_eq!(exact.heap, new.heap);
                let short = run(
                    true,
                    &paths,
                    function,
                    usize::MAX,
                    new.paid + floor - 1,
                    floor,
                    false,
                );
                assert!(short.result.is_err());
                assert!(short.refused);
                assert!(short.sticky.is_some());
            }
            let zero = run(true, &paths, function, usize::MAX, 0, 0, false);
            assert!(zero.result.is_err());
            assert!(zero.refused);
            assert!(zero.sticky.is_some());
        }
    }
}
