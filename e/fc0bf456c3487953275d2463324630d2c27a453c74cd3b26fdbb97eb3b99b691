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
                assert!(copy[..values.len()]
                    .iter()
                    .zip(values)
                    .all(|(a, b)| Rc::ptr_eq(a, b)));
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
            assert_eq!(first.0 .0, again.0 .0);
            assert_eq!(first.0 .0, oracle(function, &values, expected, limit, 0, 2));
            assert_eq!(first.1, again.1);
            assert_eq!(first.2, again.2);
            assert_eq!(first.0 .0.result.is_ok(), limit == exact);
        }
        let mut limit = 0;
        let mut complete = false;
        // Walk actual producer receipts rather than guessed byte boundaries.
        // Each receiver gets short/exact-admission/floor and full sticky/Drop checks.
        for _ in 0..10_000 {
            let step = observe(function, &values, target, limit, 0, 2);
            if step.0 .0.result.is_ok() {
                complete = true;
                break;
            }
            assert!(step.1);
            let request = step.3.unwrap();
            let end = step.2.checked_add(request.requested).unwrap();
            assert!(end > limit);
            let short = observe(function, &values, target, end - 1, 0, 2);
            let repeat = observe(function, &values, target, end - 1, 0, 2);
            assert_eq!(short.0 .0, repeat.0 .0);
            assert_eq!(
                short.0 .0,
                oracle(function, &values, expected, end - 1, 0, 2)
            );
            assert_eq!(short.3, repeat.3);
            assert!(short.0 .0.result.is_err());
            let floored = observe(function, &values, target, end, 1, 2);
            assert!(floored.1 && floored.0 .0.result.is_err());
            assert_eq!(floored.0 .0, oracle(function, &values, expected, end, 1, 2));
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
            assert_eq!(first.0 .0, again.0 .0);
            assert!(first.0 .0.result.is_err());
            assert_eq!(first.0 .0.events, work);
            assert_eq!(
                first.0 .0,
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
    // Keep the original tail-expression fixture as a negative: it has no
    // explicit return edge and cannot supply a FunctionReturn control target.
    let tail_program = program();
    assert!(tail_program.functions[0]
        .cleanup_plan
        .exits
        .iter()
        .all(|exit| !matches!(exit.continuation, ExitContinuation::FunctionReturn { .. })));
    drop(tail_program);
    // Author a real return, then use its resolver-owned source and target.
    // The same 96 statements and both original branches remain in the body.
    let mut source = String::from("module direct879; @id(\"f\") fn f(a:bool,b:bool)->i64 {");
    for i in 0..96 {
        source.push_str(&format!("let n{i}=({i}+1)*({i}+2);"));
    }
    source.push_str(
        "if a { return 1; };let v=if a {1}else{2};if b {v+1}else{v+2}} @id(\"main\") fn main()->i64 {f(true,false)}",
    );
    // A loop-free return deliberately remains on the acyclic compatibility
    // route. Keep the complete prior authored fixture as a live negative.
    let compatibility = crate::check(&source, "direct879.spx").unwrap();
    let compatibility_canonical = crate::format::canonical(&compatibility);
    let compatibility_checked = crate::check(&compatibility_canonical, "direct879.spx").unwrap();
    assert_eq!(
        crate::format::canonical(&compatibility_checked),
        compatibility_canonical
    );
    let compatibility = crate::hir::resolve(&compatibility_checked).unwrap();
    assert!(!crate::hir::lexical_exit::requires(&compatibility.functions[0]).unwrap());
    assert!(compatibility.functions[0]
        .cleanup_plan
        .exits
        .iter()
        .all(|exit| !matches!(exit.continuation, ExitContinuation::FunctionReturn { .. })));
    drop(compatibility);
    // The actual return is INSIDE the authored one-iteration loop, so its
    // resolver-owned edge is lexical. The same 96 statements and both original
    // branches remain; the complete loop-free source above stays a negative.
    source = source.replacen("if a { return 1; };",
        "let mut lexical_once=0;while lexical_once<1 {lexical_once=lexical_once+1;if a { return 1; };0}", 1);
    let parsed = crate::check(&source, "direct879.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let checked = crate::check(&canonical, "direct879.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let program = crate::hir::resolve(&checked).unwrap();
    let original = &program.functions[0];
    assert!(crate::hir::lexical_exit::requires(original).unwrap());
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

// Complete current and immediate-old enumerators, with all original input held
// outside the measured ledger. The old function is an exact whole-body copy.
fn tiny_plan_program() -> ResolvedProgram {
    let source = "module branch_inline; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}";
    let checked = crate::check(source, "branch-inline.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "branch-inline.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}

fn observe_small_plan(
    old: bool,
    function: &ResolvedFunction,
    limit: usize,
    floor: usize,
    work: usize,
) -> (
    (Outcome, crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    let answer = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            assert!(crate::bounded_output::set_active_floor(floor));
            let begin = skeleton_materializations();
            let result = (|| {
                let paths = if old {
                    before::plan_skeleton_paths_before_small_copy(function, &mut budget)?
                } else {
                    super::super::super::plan_skeleton_paths(function, &mut budget)?
                };
                let count = paths.len();
                drop(paths);
                Ok(count)
            })()
            .map_err(failure);
            let ordinary = budget.remaining;
            let work = budget.skeleton_remaining;
            let events = skeleton_materializations() - begin;
            if result.is_err() {
                let bytes = crate::bounded_output::active_remaining();
                let first = failure(
                    budget
                        .reserve_bytes(function, 0, "small copy sticky")
                        .unwrap_err(),
                );
                let second = failure(
                    budget
                        .reserve_bytes(function, 0, "small copy sticky")
                        .unwrap_err(),
                );
                assert_eq!(first, second);
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
    assert!(!answer.0 .1.invalid);
    assert_eq!(answer.0 .1.live, 0);
    assert_eq!(answer.0 .1.allocated, answer.0 .1.freed);
    assert!(answer.0 .1.allocated <= answer.2);
    answer
}

#[test]
fn small_copy_keeps_complete_immediate_paths_and_original_work_refusals() {
    for program in [tiny_plan_program(), program()] {
        for function in &program.functions {
            let (spent, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let begin = skeleton_materializations();
                let expected =
                    before::plan_skeleton_paths_before_small_copy(function, &mut old).unwrap();
                let events = skeleton_materializations() - begin;
                let begin = skeleton_materializations();
                let actual =
                    super::super::super::plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(skeleton_materializations() - begin, events);
                assert_eq!(old.remaining, current.remaining);
                assert_eq!(old.skeleton_remaining, current.skeleton_remaining);
                assert_eq!(old.skeleton_spent, current.skeleton_spent);
                let spent = old.skeleton_spent;
                drop((actual, expected, old, current));
                spent
            });
            assert!(!refused && first.is_none());
            assert!(spent > 1);
            for work in [0, 1, spent - 1, spent] {
                let old = observe_small_plan(true, function, usize::MAX, 0, work);
                let actual = observe_small_plan(false, function, usize::MAX, 0, work);
                assert_eq!(actual.0 .0, old.0 .0);
                assert_eq!(actual.1, old.1);
                assert_eq!(actual.0 .0.result.is_ok(), work == spent);
            }
        }
    }
}

#[test]
fn small_copy_whole_caller_keeps_exact_short_floor_sticky_and_complete_drop() {
    let program = tiny_plan_program();
    let function = &program.functions[0];
    let old = observe_small_plan(true, function, usize::MAX, 0, MAX_REPLAY_WORK_UNITS);
    take_small_copies();
    let current = observe_small_plan(false, function, usize::MAX, 0, MAX_REPLAY_WORK_UNITS);
    assert!(take_small_copies() > 0);
    assert!(old.0 .0.result.is_ok() && current.0 .0.result.is_ok());
    assert_eq!(current.0 .0, old.0 .0);
    assert!(!old.1 && !current.1 && old.3.is_none() && current.3.is_none());
    assert!(current.2 < old.2);
    let exact = current.2;
    assert!(exact <= 64 * 1024 * 1024);
    for limit in [0, exact - 1, exact] {
        let step = observe_small_plan(false, function, limit, 0, MAX_REPLAY_WORK_UNITS);
        let again = observe_small_plan(false, function, limit, 0, MAX_REPLAY_WORK_UNITS);
        assert_eq!(step.0 .0, again.0 .0);
        assert_eq!(step.1, again.1);
        assert_eq!(step.2, again.2);
        assert_eq!(step.3, again.3);
        assert_eq!(step.0 .0.result.is_ok(), limit == exact);
    }
    let mut limit = 0;
    let mut completed = false;
    for _ in 0..20_000 {
        let step = observe_small_plan(false, function, limit, 0, MAX_REPLAY_WORK_UNITS);
        if step.0 .0.result.is_ok() {
            completed = true;
            break;
        }
        assert!(step.1);
        let request = step.3.unwrap();
        let end = step.2.checked_add(request.requested).unwrap();
        assert!(end > limit);
        let short = observe_small_plan(false, function, end - 1, 0, MAX_REPLAY_WORK_UNITS);
        let again = observe_small_plan(false, function, end - 1, 0, MAX_REPLAY_WORK_UNITS);
        assert_eq!(short.0 .0, again.0 .0);
        assert_eq!(short.3, again.3);
        assert!(short.1 && short.0 .0.result.is_err());
        let floored = observe_small_plan(false, function, end, 1, MAX_REPLAY_WORK_UNITS);
        assert!(floored.1 && floored.0 .0.result.is_err());
        assert_eq!(short.0 .0.result, floored.0 .0.result);
        let admitted = observe_small_plan(false, function, end, 0, MAX_REPLAY_WORK_UNITS);
        assert!(admitted.2 >= end);
        limit = end;
    }
    assert!(
        completed,
        "complete caller receipt staircase reaches the producer"
    );
}

#[test]
fn small_copy_cutoff_uses_actual_full_receivers_without_a_backing_credit() {
    let old = inline_controls::<Rc<SkeletonObservation>, SkeletonObservation>();
    let inline = super::super::inline_controls::<Rc<SkeletonObservation>>();
    assert_eq!(small_inline_controls::<Rc<SkeletonObservation>>(), inline);
    assert_eq!(
        PLAN_SMALL_SELECTION_CONTROLS,
        super::super::super::final_output::PLAN_SELECTION_CONTROLS + SMALL_SELECTION_CONTROLS
    );
    assert!(old > inline + SMALL_SELECTION_CONTROLS);
    let word = size_of::<Rc<SkeletonObservation>>();
    for width in 0..SMALL_COPY_LIMIT {
        let most_extra_backing = width.saturating_sub(1) * word;
        assert!(inline + SMALL_SELECTION_CONTROLS + most_extra_backing < old);
    }
    assert!(inline + SMALL_SELECTION_CONTROLS + (SMALL_COPY_LIMIT - 1) * word >= old);
}

#[test]
fn compact_branch_payments_keep_complete_first_refusals_floors_work_and_backing() {
    let program = program();
    let function = &program.functions[0];
    let values = (0..8).map(Rc::new).collect::<Vec<_>>();
    for limit in [0, 1, 511, 512, 1024, 8192] {
        for floor in [0, 128] {
            for request in [0, 1, 8, 128, usize::MAX] {
                for operation in 0..4 {
                    let run = |compact| {
                        with_limit_usage_refusal(limit, || {
                            measure(|| {
                                let mut budget = ReplayBudget::with_skeleton_limit(500);
                                assert_eq!(
                                    crate::bounded_output::set_active_floor(floor),
                                    floor <= limit
                                );
                                let result = match (compact, operation) {
                                    (false, 0) => budget.reserve_bytes(
                                        function,
                                        request,
                                        "paired byte payment",
                                    ),
                                    (true, 0) => budget
                                        .reserve_compact_bytes(
                                            function,
                                            request,
                                            "paired byte payment",
                                        )
                                        .map_err(physical_budget::ByteFailure::into_diagnostic),
                                    (false, 1) => budget.reserve_array::<usize>(
                                        function,
                                        request,
                                        "paired array payment",
                                    ),
                                    (true, 1) => budget
                                        .reserve_compact_array::<usize>(
                                            function,
                                            request,
                                            "paired array payment",
                                        )
                                        .map_err(physical_budget::ByteFailure::into_diagnostic),
                                    (false, 2) => budget.reserve_clone(
                                        function,
                                        &values,
                                        "paired clone payment",
                                    ),
                                    (true, 2) => budget
                                        .reserve_compact_clone(
                                            function,
                                            &values,
                                            "paired clone payment",
                                        )
                                        .map_err(physical_budget::ByteFailure::into_diagnostic),
                                    (false, _) => budget.charge_skeleton(
                                        function,
                                        request,
                                        "paired skeleton work",
                                    ),
                                    (true, _) => budget
                                        .charge_compact_skeleton(
                                            function,
                                            request,
                                            "paired skeleton work",
                                        )
                                        .map_err(physical_budget::ByteFailure::into_diagnostic),
                                }
                                .map_err(failure);
                                let repeated = if result.is_err() {
                                    Some(
                                        if compact {
                                            budget
                                                .reserve_compact_bytes(
                                                    function,
                                                    0,
                                                    "paired sticky authentication",
                                                )
                                                .map_err(
                                                    physical_budget::ByteFailure::into_diagnostic,
                                                )
                                        } else {
                                            budget.reserve_bytes(
                                                function,
                                                0,
                                                "paired sticky authentication",
                                            )
                                        }
                                        .map_err(failure),
                                    )
                                } else {
                                    None
                                };
                                let state = (
                                    budget.remaining,
                                    budget.skeleton_remaining,
                                    budget.skeleton_spent,
                                    budget.refusal_spent,
                                    budget.builder_clone_cursor_paid,
                                );
                                crate::bounded_output::clear_active_floor();
                                drop(budget);
                                (result, repeated, state)
                            })
                        })
                    };
                    let original = run(false);
                    let compact = run(true);
                    assert_eq!(
                        compact, original,
                        "operation={operation} request={request} limit={limit} floor={floor}"
                    );
                    assert!(!compact.0 .1.invalid);
                    assert_eq!(compact.0 .1.live, 0);
                    assert_eq!(compact.0 .1.allocated, compact.0 .1.freed);
                    assert!(compact.0 .1.allocated <= compact.2);
                }
            }
        }
    }
}
