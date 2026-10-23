//! Complete immediately preceding PLAN enumerator against actual compact calls.
use super::super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};
use std::mem::size_of;
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

#[derive(Debug)]
struct Run {
    result: Result<Vec<SkeletonPath>, Failure>,
    remaining: usize,
    spent: usize,
    paid: usize,
    refused: bool,
    heap: Metrics,
    moves: (usize, usize, usize),
    materializations: usize,
}
fn run(compact: bool, f: &ResolvedFunction, work: usize, bytes: usize, floor: usize) -> Run {
    let ((result, remaining, spent, heap, moves, materializations), refused, paid, _) =
        with_limit_usage_refusal(bytes, || {
            let ((result, remaining, spent, moves, materializations), heap) = measure(|| {
                super::take_moves();
                let before = skeleton_materializations();
                let mut b = ReplayBudget::with_skeleton_limit(work);
                assert!(crate::bounded_output::set_active_floor(floor));
                let result = if compact {
                    plan_skeleton_paths(f, &mut b)
                } else {
                    super::before_plan_compact::plan_skeleton_paths(f, &mut b)
                }
                .map_err(failure);
                (
                    result,
                    b.skeleton_remaining,
                    b.skeleton_spent,
                    super::take_moves(),
                    skeleton_materializations() - before,
                )
            });
            (result, remaining, spent, heap, moves, materializations)
        });
    Run {
        result,
        remaining,
        spent,
        paid,
        refused,
        heap,
        moves,
        materializations,
    }
}
fn same(old: &Run, new: &Run) {
    assert_eq!(old.result, new.result);
    assert_eq!(old.remaining, new.remaining);
    assert_eq!(old.spent, new.spent);
    assert_eq!(old.materializations, new.materializations);
    assert_eq!(old.moves, new.moves);
    assert_eq!(old.heap, new.heap);
}
// Independent breadth-first walk over the actual checked acyclic plan. This
// counts dispatches, never infers a count from paid bytes or changes admission.
fn selections(f: &ResolvedFunction) -> usize {
    let mut todo = VecDeque::from([f.cleanup_plan.entry]);
    let mut count = 0;
    let mut visits = 0;
    while let Some(id) = todo.pop_front() {
        visits += 1;
        assert!(visits < 100_000, "fixture must remain acyclic and bounded");
        count += usize::from(matches!(
            &f.cleanup_plan.blocks[id.0 as usize].terminator,
            CleanupTerminator::Branch(_)
        ));
        todo.extend(block_successors(f, id));
    }
    count
}
#[test]
fn complete_plan_paths_work_heap_and_exact_compact_result_delta_match_predecessor() {
    let program = program();
    let unit =
        size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), physical_budget::ByteFailure>>();
    assert_eq!(
        super::PLAN_MOVE_CONTROLS - super::compact::PLAN_MOVE_CONTROLS,
        5 * unit
    );
    assert_eq!(
        branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>()
            - branch_clone::plan_compact_roles::<std::rc::Rc<SkeletonObservation>>(),
        unit
    );
    assert_eq!(
        branch_clone::direct::compact_roles::<std::rc::Rc<SkeletonObservation>, SkeletonObservation>(
        ) - branch_clone::direct::plan_compact_roles::<
            std::rc::Rc<SkeletonObservation>,
            SkeletonObservation,
        >(),
        unit
    );
    let mut observed_moves = 0;
    for f in &program.functions {
        let old = run(false, f, usize::MAX, 64 * 1024 * 1024, 0);
        let new = run(true, f, usize::MAX, 64 * 1024 * 1024, 0);
        same(&old, &new);
        assert!(new.result.is_ok() && !new.refused && !old.refused);
        assert_eq!(
            new.paid + (selections(f) + 6 * new.moves.0) * unit,
            old.paid
        );
        observed_moves += new.moves.0;
    }
    assert!(
        observed_moves > 0,
        "actual final-sibling producer must execute"
    );
}
#[test]
fn every_plan_work_cut_complete_errors_exact_short_zero_and_floor_keep_sticky_owners() {
    let program = program();
    for f in &program.functions {
        let full = run(true, f, usize::MAX, 64 * 1024 * 1024, 0);
        for work in 0..=full.spent {
            same(
                &run(false, f, work, 64 * 1024 * 1024, 0),
                &run(true, f, work, 64 * 1024 * 1024, 0),
            );
        }
        for compact in [false, true] {
            let full = run(compact, f, usize::MAX, 64 * 1024 * 1024, 0);
            let exact = run(compact, f, usize::MAX, full.paid, 0);
            same(&full, &exact);
            assert!(!exact.refused);
            for (bytes, floor) in [(0, 0), (full.paid - 1, 0), (full.paid, 1)] {
                let short = run(compact, f, usize::MAX, bytes, floor);
                assert!(short.result.is_err() && short.refused);
                assert!(
                    !short.heap.invalid
                        && short.heap.live == 0
                        && short.heap.allocated == short.heap.freed
                );
            }
        }
    }
    let mut hostile = program.functions[0].clone();
    hostile
        .cleanup_plan
        .edges
        .iter_mut()
        .find(|e| !matches!(e.condition, EdgeCondition::Always))
        .unwrap()
        .condition = EdgeCondition::Always;
    same(
        &run(false, &hostile, usize::MAX, 64 * 1024 * 1024, 0),
        &run(true, &hostile, usize::MAX, 64 * 1024 * 1024, 0),
    );
}
