use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::cell::Cell;

thread_local! { static ORIGINAL: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(Cell::get)
}
fn with_original<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL.with(|flag| flag.replace(true)));
    run()
}
fn program() -> ResolvedProgram {
    // This is the complete previously admitted boxed-state receiver fixture.
    let source = include_str!("../receiver_tests.rs");
    let source = source.split_once("const SOURCE: &str = r#\"").unwrap().1;
    let source = source.split_once("\"#;").unwrap().0;
    let checked = crate::check(source, "single-state-groups.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "single-state-groups.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn replay(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    validate_path_states(program, function, &storage, &leaves, budget, None, None)
}
#[test]
fn whole_replay_preserves_success_work_and_the_older_independent_receiver() {
    let program = program();
    with_limit_usage_refusal(usize::MAX, || {
        for function in &program.functions {
            let plan = function.cleanup_plan.clone();
            let mut original = ReplayBudget::new();
            let mut current = ReplayBudget::new();
            with_original(|| replay(&program, function, &mut original)).unwrap();
            replay(&program, function, &mut current).unwrap();
            assert_eq!(current.remaining, original.remaining);
            assert_eq!(current.skeleton_remaining, original.skeleton_remaining);
            let mut independent = ReplayBudget::new();
            let storage = validate_slots_paid(function, &mut independent).unwrap();
            let leaves = collect_leaves_paid(function, &mut independent).unwrap();
            super::super::before::validate_path_states::<Diagnostic>(
                &program,
                function,
                &storage,
                &leaves,
                &mut independent,
                None,
                None,
            )
            .unwrap();
            assert_eq!(current.remaining, independent.remaining);
            assert_eq!(current.skeleton_remaining, independent.skeleton_remaining);
            assert_eq!(function.cleanup_plan, plan);
        }
    });
}
#[test]
fn the_complete_group_receiver_keeps_hostile_transition_diagnostics_and_work() {
    let program = program();
    let mut forged = program
        .functions
        .iter()
        .find(|f| f.name == "f")
        .unwrap()
        .clone();
    let block = forged
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .any(|t| matches!(t, CleanupTransition::Initialize { .. }))
        })
        .unwrap();
    let at = block
        .transitions
        .iter()
        .position(|t| matches!(t, CleanupTransition::Initialize { .. }))
        .unwrap();
    block
        .transitions
        .insert(at + 1, block.transitions[at].clone());
    let plan = forged.cleanup_plan.clone();
    with_limit_usage_refusal(usize::MAX, || {
        let mut old = ReplayBudget::new();
        let mut current = ReplayBudget::new();
        let expected = with_original(|| replay(&program, &forged, &mut old)).unwrap_err();
        let actual = replay(&program, &forged, &mut current).unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(current.remaining, old.remaining);
        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
        assert_eq!(forged.cleanup_plan, plan);
    });
}
#[test]
fn full_replay_removes_backing_with_zero_exact_short_limits_and_complete_drop() {
    let program = program();
    let function = program.functions.iter().find(|f| f.name == "f").unwrap();
    let observe = |original, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = if original {
                    with_original(|| replay(&program, function, &mut budget))
                } else {
                    replay(&program, function, &mut budget)
                };
                let outcome = (result.is_ok(), budget.remaining, budget.skeleton_remaining);
                drop(result);
                drop(budget);
                outcome
            })
        })
    };
    let ((old, old_allocation), refused, old_paid, first) = observe(true, usize::MAX);
    assert!(old.0 && !refused && first.is_none());
    assert!(!old_allocation.invalid && old_allocation.live == 0);
    assert_eq!(old_allocation.allocated, old_allocation.freed);
    assert!(old_allocation.allocated <= old_paid);
    let ((current, allocation), refused, exact, first) = observe(false, usize::MAX);
    assert_eq!(current, old);
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact);
    assert!(exact < old_paid);
    assert!(allocation.allocated < old_allocation.allocated);
    for limit in [0, exact - 1, exact] {
        let ((outcome, allocation), refused, paid, first) = observe(false, limit);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        assert_eq!(outcome.0, limit == exact);
        if limit == exact {
            assert_eq!(outcome, current);
            assert!(!refused && first.is_none());
            assert_eq!(paid, exact);
        } else {
            assert!(refused && first.is_some());
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
