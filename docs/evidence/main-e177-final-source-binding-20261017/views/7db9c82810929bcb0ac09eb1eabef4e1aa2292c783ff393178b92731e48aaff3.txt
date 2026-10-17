use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::rc::Rc;

fn program() -> ResolvedProgram {
    let mut source = String::from("module direct879; @id(\"f\") fn f(a:bool,b:bool)->i64 {");
    for i in 0..96 {
        source.push_str(&format!("let n{i}=({i}+1)*({i}+2);"));
    }
    source.push_str(
        "let v=if a {1}else{2};if b {v+1}else{v+2}} @id(\"main\") fn main()->i64 {f(true,false)}",
    );
    let parsed = crate::check(&source, "direct879.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let checked = crate::check(&canonical, "direct879.spx").unwrap();
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

#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<usize, Failure>,
    work: usize,
    ordinary: usize,
    events: usize,
}
fn observe(
    function: &ResolvedFunction,
    values: &Vec<Rc<usize>>,
    target: BlockId,
    limit: usize,
    floor: usize,
    work: usize,
) -> (
    (Outcome, crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    let (value, refused, paid, receipt) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            assert!(crate::bounded_output::set_active_floor(floor));
            let begin = skeleton_materializations();
            let result = (|| {
                // The same earlier receiving payment as the real two-edge caller.
                // No call argument, Result<Vec<_>> or query is formed above it.
                budget.reserve_bytes(
                    function,
                    super::inline_controls::<Rc<usize>, Rc<usize>>(),
                    "branch clone receiving controls",
                )?;
                let copy = copy_prepaid(
                    &mut budget,
                    function,
                    values,
                    Rc::clone(&values[0]),
                    "cleanup-plan skeleton branch clone",
                    &function.cleanup_plan,
                    target,
                )?;
                assert_eq!(copy.len(), values.len() + 1);
                assert!(
                    copy[..values.len()]
                        .iter()
                        .zip(values)
                        .all(|(a, b)| Rc::ptr_eq(a, b))
                );
                assert!(Rc::ptr_eq(copy.last().unwrap(), &values[0]));
                let capacity = copy.capacity();
                drop(copy);
                Ok(capacity)
            })()
            .map_err(failure);
            let ordinary = budget.remaining;
            let work = budget.skeleton_remaining;
            let events = skeleton_materializations() - begin;
            if result.is_err() {
                let bytes = crate::bounded_output::active_remaining();
                let first = failure(
                    budget
                        .reserve_bytes(function, 0, "direct879 sticky")
                        .unwrap_err(),
                );
                let second = failure(
                    budget
                        .reserve_bytes(function, 0, "direct879 sticky")
                        .unwrap_err(),
                );
                assert_eq!(first, second); // all six fields of both sticky retries
                assert_eq!(first, failure(Diagnostic::io("SPX-H006", String::new())));
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                assert_eq!(budget.remaining, ordinary);
                assert_eq!(budget.skeleton_remaining, work);
            }
            crate::bounded_output::clear_active_floor();
            drop(budget);
            Outcome {
                result,
                work,
                ordinary,
                events,
            }
        })
    });
    assert!(!value.1.invalid);
    assert_eq!(value.1.live, 0);
    assert_eq!(value.1.allocated, value.1.freed);
    assert!(value.1.allocated <= paid);
    (value, refused, paid, receipt)
}

// Independent real-budget oracle. It never calls the new query/clone producer
// or allocates its Vec; its supplied expected capacity is computed before the
// ledger. Every first failure is compared in all six fields with this route.
fn oracle(
    function: &ResolvedFunction,
    values: &Vec<Rc<usize>>,
    capacity: usize,
    limit: usize,
    floor: usize,
    work: usize,
) -> Outcome {
    let (outcome, _, _, _) = with_limit_usage_refusal(limit, || {
        let mut budget = ReplayBudget::with_skeleton_limit(work);
        assert!(crate::bounded_output::set_active_floor(floor));
        let mut events = 0;
        let result = (|| {
            budget.reserve_bytes(
                function,
                super::inline_controls::<Rc<usize>, Rc<usize>>(),
                "branch clone receiving controls",
            )?;
            budget.charge_skeleton(function, 1, "cleanup-plan skeleton branch clone")?;
            budget.reserve_clone(function, values, "cleanup-plan skeleton branch clone")?;
            events += 1;
            budget.charge_skeleton(function, 1, "plan branch observation push")?;
            events += 1;
            budget.reserve_array::<Rc<usize>>(
                function,
                capacity - values.len(),
                "plan branch observation push",
            )?;
            budget.reserve_bytes(function, 0, "plan branch observation push")?;
            Ok(capacity)
        })()
        .map_err(failure);
        crate::bounded_output::clear_active_floor();
        Outcome {
            result,
            work: budget.skeleton_remaining,
            ordinary: budget.remaining,
            events,
        }
    });
    outcome
}

#[test]
fn direct_generic_entry_prepays_result_and_arguments_with_exact_short_floor_sticky_drop() {
    assert_eq!(
        super::inline_controls::<Rc<usize>, Rc<usize>>(),
        super::super::inline_controls::<Rc<usize>>() + copy_controls::<Rc<usize>, Rc<usize>>()
    );
    let program = program();
    let function = &program.functions[0];
    let target=function.cleanup_plan.blocks.iter().find(|block|matches!(block.terminator,
        CleanupTerminator::Exit(exit) if matches!(function.cleanup_plan.exits[exit.0 as usize].continuation,
            ExitContinuation::CommitResult {..}|ExitContinuation::ReturnUnit|ExitContinuation::ReturnFailure {..})))
        .unwrap().id;
    for width in [42, 64, 128] {
        let values = (0..width).map(Rc::new).collect::<Vec<_>>();
        let ((full, _), refused, exact, _) = observe(function, &values, target, usize::MAX, 0, 2);
        assert!(!refused);
        assert_eq!(
            full.result,
            Ok(bound(&function.cleanup_plan, target, width)
                .unwrap()
                .required
                .min(width * 2))
        );
        let expected = full.result.as_ref().copied().unwrap();
        assert_eq!(full, oracle(function, &values, expected, usize::MAX, 0, 2));
        assert_eq!(full.work, 0);
        assert_eq!(full.events, 2);
        for limit in [0, exact - 1, exact] {
            let first = observe(function, &values, target, limit, 0, 2);
            let again = observe(function, &values, target, limit, 0, 2);
            assert_eq!(first.0.0, again.0.0);
            assert_eq!(first.0.0, oracle(function, &values, expected, limit, 0, 2));
            assert_eq!(first.1, again.1);
            assert_eq!(first.2, again.2);
            assert_eq!(first.0.0.result.is_ok(), limit == exact);
        }
        let mut limit = 0;
        let mut complete = false;
        // Walk actual producer receipts rather than guessed byte boundaries.
        // Each receiver gets short/exact-admission/floor and full sticky/Drop checks.
        for _ in 0..10_000 {
            let step = observe(function, &values, target, limit, 0, 2);
            if step.0.0.result.is_ok() {
                complete = true;
                break;
            }
            assert!(step.1);
            let request = step.3.unwrap();
            let end = step.2.checked_add(request.requested).unwrap();
            assert!(end > limit);
            let short = observe(function, &values, target, end - 1, 0, 2);
            let repeat = observe(function, &values, target, end - 1, 0, 2);
            assert_eq!(short.0.0, repeat.0.0);
            assert_eq!(
                short.0.0,
                oracle(function, &values, expected, end - 1, 0, 2)
            );
            assert_eq!(short.3, repeat.3);
            assert!(short.0.0.result.is_err());
            let floored = observe(function, &values, target, end, 1, 2);
            assert!(floored.1 && floored.0.0.result.is_err());
            assert_eq!(floored.0.0, oracle(function, &values, expected, end, 1, 2));
            let admitted = observe(function, &values, target, end, 0, 2);
            assert!(admitted.2 >= end);
            limit = end;
        }
        assert!(
            complete,
            "actual receipt staircase must reach the direct producer"
        );
        let ((floor, _), refused, _, _) = observe(function, &values, target, exact, 1, 2);
        assert!(refused && floor.result.is_err());
        assert_eq!(floor, oracle(function, &values, expected, exact, 1, 2));
        for work in [0, 1] {
            let first = observe(function, &values, target, usize::MAX, 0, work);
            let again = observe(function, &values, target, usize::MAX, 0, work);
            assert_eq!(first.0.0, again.0.0);
            assert!(first.0.0.result.is_err());
            assert_eq!(first.0.0.events, work);
            assert_eq!(
                first.0.0,
                oracle(function, &values, expected, usize::MAX, 0, work)
            );
        }
    }
}

#[test]
fn current_continuations_keep_full_predecessor_paths_and_both_work_accounts() {
    let program = program();
    let (((), metrics), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let begin = skeleton_materializations();
                let expected = before::plan_skeleton_paths(function, &mut old).unwrap();
                let events = skeleton_materializations() - begin;
                let begin = skeleton_materializations();
                let actual =
                    super::super::super::plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(skeleton_materializations() - begin, events);
                assert_eq!(old.remaining, current.remaining);
                assert_eq!(old.skeleton_remaining, current.skeleton_remaining);
                drop((actual, expected));
                drop((old, current));
            }
        })
    });
    assert!(!refused);
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid);
}

#[test]
fn current_query_rejects_missing_cycles_overflow_and_foreign_authority() {
    let program = program();
    let function = &program.functions[0];
    let plan = &function.cleanup_plan;
    assert!(bound(plan, BlockId(u32::MAX), 64).is_none());
    let terminal=plan.blocks.iter().find(|block|matches!(block.terminator,
        CleanupTerminator::Exit(exit) if matches!(plan.exits[exit.0 as usize].continuation,
            ExitContinuation::CommitResult {..}|ExitContinuation::ReturnUnit|ExitContinuation::ReturnFailure {..})))
        .unwrap().id;
    assert!(bound(plan, terminal, usize::MAX).is_none());
    assert!(bound(plan, terminal, 0).is_none());
    let mut cycle = plan.clone();
    cycle.edges[0].to = BlockId(0);
    cycle.blocks[0].terminator = CleanupTerminator::Goto(EdgeId(0));
    assert!(bound(&cycle, BlockId(0), 64).is_none());
    let mut further = plan.clone();
    further.edges[0].to = BlockId(1);
    further.blocks[0].terminator = CleanupTerminator::Branch(vec![EdgeId(0), EdgeId(0)]);
    further.blocks[1].terminator = CleanupTerminator::Branch(vec![EdgeId(0), EdgeId(0)]);
    assert!(bound(&further, BlockId(0), 64).is_none());
    let values = Vec::with_capacity(64);
    let mut budget = ReplayBudget::unobserved();
    let result = probe(TakeCandidate {
        values: &values,
        remaining_edges: 0,
        edge_count: 3,
        budget: &budget,
        plan,
        target: terminal,
    });
    assert!(!result.take && result.copy_capacity.is_none());
    budget.payer = AllocationPayer::ActiveBuilder;
    budget.admission = allocation::AdmissionMode::SelectedRetained;
    let result = probe(TakeCandidate {
        values: &values,
        remaining_edges: 0,
        edge_count: 3,
        budget: &budget,
        plan,
        target: terminal,
    });
    assert!(!result.take && result.copy_capacity.is_none());
}

#[test]
fn continue_return_break_and_latch_have_real_bounded_continuation_controls() {
    let program = program();
    let original = &program.functions[0];
    let terminal=original.cleanup_plan.blocks.iter().find(|block| matches!(block.terminator,
        CleanupTerminator::Exit(exit) if matches!(original.cleanup_plan.exits[exit.0 as usize].continuation,
            ExitContinuation::CommitResult {..}|ExitContinuation::ReturnUnit|ExitContinuation::ReturnFailure {..}))).unwrap().id;
    let (exit, target) = original
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::FunctionReturn { target, .. } => Some((exit.id, target.clone())),
            _ => None,
        })
        .unwrap();
    let start = original
        .cleanup_plan
        .blocks
        .iter()
        .find(|block| matches!(block.terminator,CleanupTerminator::Exit(id) if id==exit))
        .unwrap()
        .id;
    for (continuation, marker) in [
        (ExitContinuation::Continue(EdgeId(0)), 0),
        (
            ExitContinuation::FunctionReturn {
                target: target.clone(),
                edge: EdgeId(0),
                source: match &original.cleanup_plan.exits[exit.0 as usize].continuation {
                    ExitContinuation::FunctionReturn { source, .. } => source.clone(),
                    _ => unreachable!(),
                },
            },
            1,
        ),
        (
            ExitContinuation::LoopBreak {
                target: target.clone(),
                edge: EdgeId(0),
            },
            1,
        ),
    ] {
        let mut function = original.clone();
        let plan = &mut function.cleanup_plan;
        plan.entry = start;
        plan.edges[0].to = terminal;
        plan.exits[exit.0 as usize].continuation = continuation;
        let expected = 65
            + plan.blocks[start.0 as usize].transitions.len()
            + plan.blocks[terminal.0 as usize].transitions.len()
            + marker;
        assert_eq!(bound(plan, start, 64).unwrap().required, expected);
        let values = (0..64).map(Rc::new).collect::<Vec<_>>();
        let ((paid_copy, _), refused, _, _) = observe(&function, &values, start, usize::MAX, 0, 2);
        assert!(!refused);
        assert_eq!(paid_copy.result, Ok(expected.min(128)));
        assert_eq!((paid_copy.work, paid_copy.events), (0, 2));
        let (((), metrics), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let expected = before::plan_skeleton_paths(&function, &mut old).unwrap();
                let count = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual =
                    super::super::super::plan_skeleton_paths(&function, &mut current).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(skeleton_materializations() - before, count);
                assert_eq!(old.remaining, current.remaining);
                assert_eq!(old.skeleton_remaining, current.skeleton_remaining);
                drop((actual, expected));
                drop((current, old));
            })
        });
        assert!(!refused && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid);
    }
    let mut latch = original.cleanup_plan.clone();
    latch.exits[exit.0 as usize].continuation = ExitContinuation::LoopLatch { target };
    assert!(bound(&latch, start, 64).is_none());
}
