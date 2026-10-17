//! Old local backing acquisition versus invocation-held backing, on one source.
//! Source and fixed observer/active-ledger setup are outside each whole lifetime.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const SOURCE: &str = r#"
module scratch.replay;
@id("g.choice") variant Choice { @id("g.a") A, @id("g.b") B, }
@id("g.positive") fn positive(value:i64)->bool requires value>=0 ensures result==true {value>=0}
@id("g.main") fn main()->i64 {
 let choice=Choice::A {}; let held="held"; let mut count=0;
 while count<2 {
  let number=match choice {
   Choice::A {} | Choice::B {} if string_len(string_concat("a","b"))==2 && positive(count) => 1,
   _ if { let temp="x"; string_len(temp)==99 } => 9,
   _ => 0,
  };
  count=count+number; 0
 } count+string_len(held)
}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(SOURCE, "scratch.spx").unwrap()).unwrap()
}
/// Saved old continuation acquisition, before any root evaluation/push. The
/// state machine itself is unchanged and is shared with the production runner.
fn old_expression<'hir>(
    program: &'hir ResolvedProgram,
    function: &'hir ResolvedFunction,
    expression: &'hir ResolvedExpr,
    work: &mut SkeletonWork<'hir, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    work.budget
        .reserve_array::<Frame<'hir>>(function, 516, "typed HIR continuation stack")?;
    let mut frames = Vec::with_capacity(516);
    expression_skeleton_run(program, function, expression, work, &mut frames)
}
// Saved small DAG expansion: sequence order and duplicate alternatives are
// semantic content, not Rc-address equality or Debug output.
fn observation_sequences(
    observations: &Observations,
) -> Vec<Vec<std::rc::Rc<SkeletonObservation>>> {
    fn expand(dag: &skeleton_work::ObservationDag) -> Vec<Vec<std::rc::Rc<SkeletonObservation>>> {
        match &dag.kind {
            skeleton_work::ObservationDagKind::Seq(prefix, items) => {
                let mut paths = prefix
                    .as_ref()
                    .map_or_else(|| vec![vec![]], |prefix| expand(prefix));
                for path in &mut paths {
                    path.extend_from_slice(items);
                }
                paths
            }
            skeleton_work::ObservationDagKind::Alt(parts) => {
                parts.iter().flat_map(|part| expand(part)).collect()
            }
            skeleton_work::ObservationDagKind::Cat(parts) => {
                let mut paths = vec![vec![]];
                for part in parts {
                    let suffixes = expand(part);
                    paths = paths
                        .into_iter()
                        .flat_map(|prefix| {
                            suffixes.iter().map(move |suffix| {
                                let mut path = prefix.clone();
                                path.extend_from_slice(suffix);
                                path
                            })
                        })
                        .collect();
                }
                paths
            }
        }
    }
    let (prefix, items) = observations.parts();
    let mut paths = prefix.map_or_else(|| vec![vec![]], |prefix| expand(prefix));
    for path in &mut paths {
        path.extend_from_slice(items);
    }
    paths
}
#[test]
fn same_program_roots_and_guard_reentry_keep_old_paths_work_and_first_errors() {
    let program = program();
    let mut storage = Storage::new(&program);
    let mut old_budget = ReplayBudget::unobserved();
    let mut new_budget = ReplayBudget::unobserved();
    reserve_program_skeleton_work(&program, program.functions.iter(), &mut old_budget).unwrap();
    reserve_program_skeleton_work(&program, program.functions.iter(), &mut new_budget).unwrap();
    for function in &program.functions {
        let mut old = SkeletonWork::new(&program, function, &mut old_budget);
        let mut current = SkeletonWork::new(&program, function, &mut new_budget);
        current.continuations = Some(&mut storage);
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            let before = old_expression(&program, function, expression, &mut old).unwrap();
            let after = expression_skeleton(&program, function, expression, &mut current).unwrap();
            assert_eq!(before.len(), after.len());
            for (old, new) in before.iter().zip(&after) {
                assert_eq!(
                    observation_sequences(&old.observations),
                    observation_sequences(&new.observations)
                );
                assert_eq!(old.owned_source, new.owned_source);
                assert_eq!(
                    (old.failed, old.residual, &old.continuation),
                    (new.failed, new.residual, &new.continuation)
                );
            }
        }
    }
    assert_eq!(old_budget.remaining, new_budget.remaining);
    assert_eq!(old_budget.skeleton_spent, new_budget.skeleton_spent);
    assert!(storage.frames.is_some());
    assert!(storage.frames.as_ref().unwrap().is_empty());
    let foreign = program.clone();
    assert!(!storage.matches(&foreign));

    // A late traversal refusal returns no surviving payloads. The same actual
    // HIR reaches both old and new receivers; their complete diagnostics agree.
    let function = &program.functions[1];
    let mut old_budget = ReplayBudget::with_skeleton_limit(1);
    let mut new_budget = ReplayBudget::with_skeleton_limit(1);
    let old = old_expression(
        &program,
        function,
        &function.body,
        &mut SkeletonWork::new(&program, function, &mut old_budget),
    )
    .err()
    .unwrap();
    let mut work = SkeletonWork::new(&program, function, &mut new_budget);
    work.continuations = Some(&mut storage);
    let new = expression_skeleton(&program, function, &function.body, &mut work)
        .err()
        .unwrap();
    assert_eq!(old, new);
    assert_eq!(work.budget.skeleton_remaining, 0);
    assert!(work
        .continuations
        .as_ref()
        .unwrap()
        .frames
        .as_ref()
        .unwrap()
        .is_empty());
    assert!(expression_skeleton(&program, function, &function.body, &mut work).is_err());
}
#[test]
fn repeated_roots_remove_only_real_frame_backing_and_settle_through_drop() {
    let program = program();
    let function = &program.functions[0];
    // This scalar function has exactly requires/body/ensures, no guard reentry.
    let roots = [&function.requires[0], &function.body, &function.ensures[0]];
    let observe = |reuse: bool| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut storage = Storage::new(&program);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                if reuse {
                    work.continuations = Some(&mut storage);
                }
                for expression in roots {
                    let paths = if reuse {
                        expression_skeleton(&program, function, expression, &mut work)
                    } else {
                        old_expression(&program, function, expression, &mut work)
                    }
                    .unwrap();
                    drop(paths);
                }
                drop(work);
                drop(storage);
                drop(budget);
            })
        })
    };
    let (((), old), old_refused, old_charge, _) = observe(false);
    let (((), current), refused, charge, _) = observe(true);
    assert!(!old_refused && !refused && !old.invalid && !current.invalid);
    let literal_removed = 2 * 516 * std::mem::size_of::<Frame<'_>>();
    assert_eq!(old.allocated - current.allocated, literal_removed);
    assert_eq!(old.allocations - current.allocations, 2);
    assert_eq!(old_charge - charge, literal_removed);
    assert_eq!((old.live, current.live), (0, 0));
    assert_eq!(old.allocated, old.freed);
    assert_eq!(current.allocated, current.freed);
}
#[test]
fn full_replay_verdict_and_same_payer_exact_short_preserve_current_owner() {
    let program = program();
    // Whole independent control retains the old per-root receiver; production
    // uses one storage across all functions and nested guards.
    let mut budget = ReplayBudget::unobserved();
    let index = ordinary_renewals::index(&program, &mut budget).unwrap();
    reserve_program_skeleton_work(&program, program.functions.iter(), &mut budget).unwrap();
    for function in &program.functions {
        validate_structure_with_budget(&program, function, &mut budget, Some(&index)).unwrap();
    }
    validate_program(&program).unwrap();

    let mut corrupt = program.clone();
    let main = corrupt
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "g.main")
        .unwrap();
    assert!(!main.cleanup_plan.status_sources.is_empty());
    main.cleanup_plan.status_sources.remove(0);
    let old_verdict = (|| {
        let mut budget = ReplayBudget::unobserved();
        let index = ordinary_renewals::index(&corrupt, &mut budget)?;
        reserve_program_skeleton_work(&corrupt, corrupt.functions.iter(), &mut budget)?;
        for function in &corrupt.functions {
            validate_structure_with_budget(&corrupt, function, &mut budget, Some(&index))?;
        }
        Ok::<(), Diagnostic>(())
    })();
    assert!(old_verdict.is_err());
    assert_eq!(old_verdict, validate_program(&corrupt));

    let function = &program.functions[0];
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut storage = Storage::new(&program);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                work.continuations = Some(&mut storage);
                let result = expression_skeleton(&program, function, &function.body, &mut work);
                if result.is_err() {
                    assert!(work.budget.reserve_bytes(function, 0, "retry").is_err());
                }
                drop(result);
                drop(work);
                drop(storage);
                drop(budget);
            })
        })
    };
    let (((), exact), refused, required, _) = observe(usize::MAX);
    assert!(!refused && !exact.invalid && exact.live == 0);
    let (((), paid), refused, charged, _) = observe(required);
    assert!(!refused && charged == required && !paid.invalid && paid.live == 0);
    let (((), short), refused, _, receipt) = observe(required - 1);
    assert!(refused && receipt.is_some() && !short.invalid && short.live == 0);
}

#[test]
fn lazy_first_and_nested_layouts_use_the_same_sticky_payer_through_drop() {
    let program = program();
    let function = &program.functions[0];
    let frame_bytes = 516 * std::mem::size_of::<Frame<'_>>();
    let observe = |limit, count| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                // Authentication is allocation-free; acquire the report backing
                // explicitly before measuring the continuation-only frontier.
                budget.prepare_builder(function).unwrap();
                let mut outer = None;
                let mut inner = None;
                let mut error = None;
                if count > 0 {
                    match fresh(function, &mut budget) {
                        Ok(buffer) => outer = Some(buffer),
                        Err(refusal) => error = Some(refusal),
                    }
                }
                if count > 1 && error.is_none() {
                    match fresh(function, &mut budget) {
                        Ok(buffer) => inner = Some(buffer),
                        Err(refusal) => error = Some(refusal),
                    }
                }
                if let Some(error) = &error {
                    assert!(error.message.contains("typed HIR continuation stack"));
                    assert!(budget.reserve_bytes(function, 0, "retry").is_err());
                }
                // Both backing owners coexist, as they do at actual guarded reentry.
                drop((inner, outer, error));
                drop(budget);
            })
        })
    };
    let (((), baseline), refused, setup, _) = observe(usize::MAX, 0);
    assert!(!refused && !baseline.invalid && baseline.live == 0);
    for count in [1, 2] {
        let exact = setup + count * frame_bytes;
        let (((), paid), refused, charged, _) = observe(exact, count);
        assert!(!refused && charged == exact && !paid.invalid);
        assert_eq!(paid.allocated - baseline.allocated, count * frame_bytes);
        assert_eq!(paid.live, 0);
        assert_eq!(paid.allocated, paid.freed);
        let (((), short), refused, _, receipt) = observe(exact - 1, count);
        assert!(refused && receipt.is_some() && !short.invalid);
        assert_eq!(
            short.allocated - baseline.allocated,
            (count - 1) * frame_bytes
        );
        assert_eq!(short.live, 0);
        assert_eq!(short.allocated, short.freed);
    }
}

mod nested_pool;

const HELD_TARGETS: [usize; 10] = [2, 4, 8, 16, 32, 64, 128, 256, 512, 516];
fn fill_held<'a>(frames: &mut Vec<Frame<'a>>, function: &'a ResolvedFunction) {
    while frames.len() < frames.capacity() {
        let frame = if frames.len().is_multiple_of(2) {
            Frame::Eval(&function.body)
        } else {
            Frame::UpcastPassthrough
        };
        frames.push(frame);
    }
}
fn held_order(frames: &[Frame<'_>], function: &ResolvedFunction) {
    for (index, frame) in frames.iter().enumerate() {
        match frame {
            Frame::Eval(expression) if index.is_multiple_of(2) => {
                assert!(std::ptr::eq(*expression, &function.body));
            }
            Frame::UpcastPassthrough if !index.is_multiple_of(2) => {}
            _ => panic!("a live continuation changed order or kind"),
        }
    }
}
#[test]
fn held_growth_preserves_every_live_frame_and_pays_every_physical_frontier() {
    let program = program();
    let function = &program.functions[0];
    let observe = |limit, floor, stop| {
        with_limit_usage_refusal(limit + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                let start = budget.remaining;
                let result = (|| {
                    let mut frames = fresh_receiving(function, &mut budget)?;
                    for target in HELD_TARGETS.into_iter().skip(1).take_while(|n| *n <= stop) {
                        fill_held(&mut frames, function);
                        let old_capacity = frames.capacity();
                        let pointer = frames.as_ptr();
                        let result = reserve_push(&mut frames, function, &mut budget);
                        if result.is_err() {
                            assert_eq!(frames.len(), old_capacity);
                            assert_eq!(frames.capacity(), old_capacity);
                            assert_eq!(frames.as_ptr(), pointer);
                            held_order(&frames, function);
                        }
                        result?;
                        assert_eq!(frames.len(), old_capacity);
                        assert_eq!(frames.capacity(), target);
                        held_order(&frames, function);
                    }
                    assert_eq!(frames.capacity(), stop);
                    drop(frames);
                    Ok::<_, Diagnostic>(())
                })();
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(budget.reserve_bytes(function, 0, "held retry").is_err());
                }
                (ok, start - budget.remaining)
            })
        })
    };
    for target in HELD_TARGETS {
        let (((ok, work), allocation), refused, exact, receipt) = observe(usize::MAX, 0, target);
        assert!(ok && !refused && receipt.is_none());
        let backing = HELD_TARGETS
            .into_iter()
            .take_while(|n| *n <= target)
            .sum::<usize>()
            * std::mem::size_of::<Frame<'_>>();
        // The refusal-text owner also exists through the final Drop.
        assert!(allocation.allocated >= backing && allocation.allocated <= exact);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        let growth_work = HELD_TARGETS
            .into_iter()
            .take_while(|n| *n < target)
            .map(|n| 8 + 2 * n)
            .sum::<usize>()
            + 8;
        assert_eq!(work, growth_work);
        for floor in [0, 37] {
            for available in [0, exact - 1, exact] {
                let (((ok, _), allocation), refused, used, first) =
                    observe(available, floor, target);
                assert_eq!(ok, available == exact);
                assert_eq!(refused, !ok);
                assert_eq!(first.is_some(), !ok);
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                assert!(allocation.allocated <= used && used <= available);
                if let Some(first) = first {
                    assert_eq!(first.floor, floor);
                    if available == exact - 1 {
                        assert_eq!(first.requested, target * std::mem::size_of::<Frame<'_>>());
                    }
                }
                if available == 0 {
                    assert_eq!(allocation.allocated, 0);
                }
            }
        }
    }
}
#[test]
fn held_growth_keeps_original_depth_refusal_and_never_relocates_after_work_failure() {
    let program = program();
    let function = &program.functions[0];
    crate::bounded_output::with_limit(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut frames = fresh_receiving(function, &mut budget).unwrap();
        fill_held(&mut frames, function);
        budget.remaining = 0;
        let pointer = frames.as_ptr();
        let capacity = frames.capacity();
        let (result, allocation) = measure(|| reserve_push(&mut frames, function, &mut budget));
        assert!(result.is_err());
        assert_eq!(allocation.allocated, 0);
        assert_eq!(frames.as_ptr(), pointer);
        assert_eq!(frames.capacity(), capacity);
        held_order(&frames, function);
        assert!(reserve_push(&mut frames, function, &mut budget).is_err());
    });
    let mut old_budget = ReplayBudget::unobserved();
    let mut old = fresh(function, &mut old_budget).unwrap();
    fill_held(&mut old, function);
    let before = reserve_push(&mut old, function, &mut old_budget).unwrap_err();
    crate::bounded_output::with_limit(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut frames = fresh_receiving(function, &mut budget).unwrap();
        for _ in HELD_TARGETS.into_iter().skip(1) {
            fill_held(&mut frames, function);
            reserve_push(&mut frames, function, &mut budget).unwrap();
        }
        fill_held(&mut frames, function);
        assert_eq!(frames.len(), FRAME_CAPACITY);
        let pointer = frames.as_ptr();
        let after = reserve_push(&mut frames, function, &mut budget).unwrap_err();
        assert_eq!(before, after);
        assert_eq!(frames.as_ptr(), pointer);
        held_order(&frames, function);
    });
}
#[test]
fn only_authenticated_program_storage_selects_small_reusable_backing() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut legacy = Storage::new(&program);
        let (lease, frames) = legacy.acquire(function, &mut budget).unwrap();
        assert_eq!(frames.capacity(), FRAME_CAPACITY);
        legacy.release(lease, frames);
        let mut held = Storage::for_program(&program, &mut budget).unwrap();
        let (outer, frames) = held.acquire(function, &mut budget).unwrap();
        assert_eq!(frames.capacity(), 2);
        let outer_pointer = frames.as_ptr();
        let (inner, nested) = held.acquire(function, &mut budget).unwrap();
        assert_eq!(nested.capacity(), 2);
        let nested_pointer = nested.as_ptr();
        assert_ne!(outer_pointer, nested_pointer);
        held.release(inner, nested);
        let (inner, nested) = held.acquire(function, &mut budget).unwrap();
        assert_eq!(nested.as_ptr(), nested_pointer);
        held.release(inner, nested);
        held.release(outer, frames);
        let (outer, frames) = held.acquire(function, &mut budget).unwrap();
        assert_eq!(frames.as_ptr(), outer_pointer);
        held.release(outer, frames);
        crate::bounded_output::with_limit(usize::MAX, || {
            assert!(held.acquire(function, &mut budget).is_err());
        });
        // Restoring the original invocation cannot clear the sticky refusal.
        assert!(held.acquire(function, &mut budget).is_err());
        assert!(held.frames.as_ref().unwrap().is_empty());
    });
}

#[test]
fn shallow_receiving_removes_the_full_fixed_request_without_an_allowance_change() {
    let program = program();
    let function = &program.functions[0];
    let observe = |held| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let frames = if held {
                    fresh_receiving(function, &mut budget)
                } else {
                    fresh(function, &mut budget)
                }
                .unwrap();
                let capacity = frames.capacity();
                drop(frames);
                drop(budget);
                capacity
            })
        })
    };
    let ((old_capacity, old), old_refused, old_charge, old_first) = observe(false);
    let ((capacity, current), refused, charge, first) = observe(true);
    assert_eq!((old_capacity, capacity), (FRAME_CAPACITY, 2));
    assert!(!old_refused && !refused && old_first.is_none() && first.is_none());
    assert!(charge < old_charge);
    assert_eq!(
        old.allocated - current.allocated,
        (FRAME_CAPACITY - 2) * std::mem::size_of::<Frame<'_>>()
    );
    for allocation in [old, current] {
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn checked_roots_and_reentrant_guards_keep_complete_old_paths_with_small_paid_backing() {
    let program = program();
    crate::bounded_output::with_limit(usize::MAX, || {
        let mut old_budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut storage = Storage::for_program(&program, &mut budget).unwrap();
        for function in &program.functions {
            let mut old = SkeletonWork::new(&program, function, &mut old_budget);
            let mut current = SkeletonWork::new(&program, function, &mut budget);
            current.continuations = Some(&mut storage);
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(function.ensures.iter())
            {
                let before = old_expression(&program, function, expression, &mut old).unwrap();
                let after =
                    expression_skeleton(&program, function, expression, &mut current).unwrap();
                assert_eq!(before.len(), after.len());
                for (before, after) in before.iter().zip(&after) {
                    assert_eq!(
                        observation_sequences(&before.observations),
                        observation_sequences(&after.observations)
                    );
                    assert_eq!(before.owned_source, after.owned_source);
                    assert_eq!(
                        (before.failed, before.residual, &before.continuation),
                        (after.failed, after.residual, &after.continuation)
                    );
                }
            }
        }
        assert!(storage.frames.as_ref().unwrap().is_empty());
        assert!(storage.frames.as_ref().unwrap().capacity() < FRAME_CAPACITY);
        let nested = storage
            .nested
            .as_ref()
            .expect("genuine guard reentry must run");
        assert_eq!(nested.depth, 0);
        assert!(nested.slots.iter().all(|slot| {
            let frames = slot.as_ref().unwrap();
            frames.is_empty() && frames.capacity() < FRAME_CAPACITY
        }));
        // Relocation uses the ordinary work account; every original skeleton
        // push/event still consumes exactly the original skeleton account.
        assert_eq!(old_budget.skeleton_spent, budget.skeleton_spent);
        assert_eq!(old_budget.skeleton_remaining, budget.skeleton_remaining);
    });
}
