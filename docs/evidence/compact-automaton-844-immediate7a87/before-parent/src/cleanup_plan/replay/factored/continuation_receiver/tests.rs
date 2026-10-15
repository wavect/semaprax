use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
const SOURCE: &str = "module receiving; @id(\"f\") fn f()->i64 {let x=if 1==1 {2}else{3};x} @id(\"main\") fn main()->i64 {0}";
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "continuation-receiving.spx").unwrap()).unwrap()
}
#[test]
fn whole_original_and_current_comparison_pairs_execute_with_paid_last_drop() {
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            // Full original pair stays inside the same owning allocation window.
            for function in &program.functions {
                super::super::cross_check(&program, function);
            }
            let mut budget = ReplayBudget::new();
            let mut storage =
                continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
            let mut pointer = None;
            for function in &program.functions {
                oracle_hook::cross_check_with_storage(
                    &program,
                    function,
                    &mut budget,
                    Some(&mut storage),
                )
                .unwrap();
                let frames = storage.frames.as_ref().unwrap();
                assert!(frames.is_empty());
                if let Some(previous) = pointer {
                    assert_eq!(frames.as_ptr(), previous);
                }
                pointer = Some(frames.as_ptr());
            }
            drop(storage);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}
#[test]
fn current_factored_storage_preserves_old_automata_work_and_full_hostile_diagnostic() {
    // Preserve the malformed inherited panic separately from the stable diagnostic.
    // Both complete algorithms still run, including their unwind and last Drop.
    let mut malformed = program();
    malformed.functions[0].cleanup_plan.blocks.clear();
    // The default harness hook may initialize retained output/backtrace caches.
    // Replace only that test instrumentation outside the complete compiler
    // observation. Both actual panic payloads, unwinds and final Drop stay in it.
    struct RestoreHook<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for RestoreHook<F> {
        fn drop(&mut self) {
            self.0.take().expect("one hook restoration")();
        }
    }
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let restore_hook = RestoreHook(Some(|| std::panic::set_hook(previous_hook)));
    // Catch the entire observation too: restoring a hook during active unwind
    // would panic. Restore normally first, then resume any assertion failure.
    let measured = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let function = &malformed.functions[0];
                let mut old_budget = ReplayBudget::with_skeleton_limit(0);
                let mut budget = ReplayBudget::with_skeleton_limit(0);
                let mut storage =
                    continuation_storage::Storage::for_program(&malformed, &mut budget).unwrap();
                let old = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_comparison(&malformed, function, &mut old_budget, None, None)
                }))
                .err()
                .expect("complete old receiver retains its malformed-block panic");
                let current = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_comparison(&malformed, function, &mut budget, None, Some(&mut storage))
                }))
                .err()
                .expect("current receiver retains the same malformed-block panic");
                fn message(payload: &(dyn std::any::Any + Send)) -> Option<&str> {
                    payload
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| payload.downcast_ref::<&'static str>().copied())
                }
                assert_eq!(
                    message(&*old),
                    Some("index out of bounds: the len is 0 but the index is 0")
                );
                assert_eq!(message(&*current), message(&*old));
                drop(old);
                drop(current);
                drop(storage);
                drop(old_budget);
                drop(budget);
            })
        })
    }));
    drop(restore_hook);
    let (((), allocation), refused, _, first) = match measured {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    eprintln!(
        "malformed old/current compiler panic observation: {allocation:?}; invalid_reason={:?}",
        crate::test_allocation_observer::invalid_reason()
    );
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    // Rust panic payload/hook allocations are observed too, but have no compiler
    // debit claim. The valid compiler cases below retain the full funding bound.
    for hostile in [false, true] {
        let mut program = program();
        if hostile {
            let plan = &mut program.functions[0].cleanup_plan;
            let edge = plan
                .blocks
                .iter()
                .find_map(|block| {
                    if let CleanupTerminator::Branch(edges) = &block.terminator {
                        edges.first().copied()
                    } else {
                        None
                    }
                })
                .expect("fixture has a real branch edge");
            plan.edges[edge.0 as usize].condition = EdgeCondition::Always;
        }
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let function = &program.functions[0];
                let mut old_budget = ReplayBudget::with_skeleton_limit(0);
                let mut budget = ReplayBudget::with_skeleton_limit(0);
                let mut storage =
                    continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
                let old = build_comparison(&program, function, &mut old_budget, None, None);
                let current =
                    build_comparison(&program, function, &mut budget, None, Some(&mut storage));
                assert_eq!(
                    (
                        old_budget.remaining,
                        old_budget.skeleton_spent,
                        old_budget.skeleton_remaining
                    ),
                    (
                        budget.remaining,
                        budget.skeleton_spent,
                        budget.skeleton_remaining
                    )
                );
                match (old, current) {
                    (Ok(old), Ok(current)) => {
                        assert!(!hostile);
                        assert_eq!(
                            (old.sink, old.hir_root, old.plan_root),
                            (current.sink, current.hir_root, current.plan_root)
                        );
                        assert_eq!(old.automaton.nodes.len(), current.automaton.nodes.len());
                        for (a, b) in old.automaton.nodes.iter().zip(&current.automaton.nodes) {
                            assert_eq!(a.moves, b.moves);
                            assert_eq!(a.epsilon, b.epsilon);
                        }
                    }
                    (Err(old), Err(current)) => {
                        assert!(hostile);
                        assert_eq!(old, current);
                        assert_eq!(old.code, "SPX-H006");
                        assert!(old
                            .message
                            .ends_with("branch skeleton contains an unconditional edge"));
                    }
                    _ => panic!("complete old/current verdict mismatch"),
                }
                drop(storage);
                drop(old_budget);
                drop(budget);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}
#[test]
fn paid_factored_receiving_exact_short_zero_keeps_first_failure_and_whole_drop() {
    let program = program();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::with_skeleton_limit(0);
                let mut storage =
                    match continuation_storage::Storage::for_program(&program, &mut budget) {
                        Ok(value) => value,
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            drop(error);
                            return false;
                        }
                    };
                let result =
                    build_comparison(&program, function, &mut budget, None, Some(&mut storage));
                let success = result.is_ok();
                if let Err(error) = &result {
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.path, None);
                    assert_eq!(error.span, None);
                    assert_eq!(error.help, None);
                    let at = current_metrics().allocated;
                    let retry = budget.reserve_bytes(function, 0, "retry").unwrap_err();
                    assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                    drop(retry);
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(result);
                drop(storage);
                drop(budget);
                success
            })
        })
    };
    let ((success, allocation), refused, exact, first) = observe(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact);
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = observe(limit);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        if limit == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(paid, exact);
        } else {
            assert!(!success && refused && first.is_some());
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
#[test]
fn foreign_then_restored_invocation_cannot_acquire_or_reuse_held_frames() {
    let program = program();
    for cached in [false, true] {
        let expected = Diagnostic::io("SPX-H006", format!("cleanup plan for function `{}` failed independent replay: typed-HIR continuation storage belongs to another invocation", program.functions[0].id));
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::new();
                let mut storage =
                    continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
                if cached {
                    let (lease, frames) = storage.acquire(function, &mut budget).unwrap();
                    storage.release(lease, frames);
                }
                let pointer = storage.frames.as_ref().unwrap().as_ptr();
                let (_, nested_refused, nested_used, nested_first) =
                    with_limit_usage_refusal(usize::MAX, || {
                        let at = current_metrics().allocated;
                        let error = storage
                            .authenticate(&program, function, &mut budget)
                            .unwrap_err();
                        assert_eq!(error, expected);
                        drop(error);
                        assert_eq!(current_metrics().allocated, at);
                    });
                assert!(!nested_refused && nested_used == 0 && nested_first.is_none());
                let at = current_metrics().allocated;
                let error = storage
                    .authenticate(&program, function, &mut budget)
                    .unwrap_err();
                assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
                drop(error);
                assert_eq!(current_metrics().allocated, at);
                assert_eq!(storage.frames.as_ref().unwrap().as_ptr(), pointer);
                drop(storage);
                drop(budget);
            })
        });
        assert!(refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}
