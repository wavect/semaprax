//! Select the retained schema receiver within the real ordinary replay caller.
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, with_phase_trace};
use std::cell::Cell;

thread_local! {
    static PREVIOUS: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn previous_selected() -> bool {
    PREVIOUS.with(Cell::get)
}

fn with_previous<T>(previous: bool, run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            PREVIOUS.with(|state| state.set(self.0));
        }
    }
    let _restore = Restore(PREVIOUS.with(|state| state.replace(previous)));
    run()
}

fn receive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> (Result<(), Diagnostic>, [usize; 3]) {
    let mut budget = ReplayBudget::new();
    assert!(budget.pays_builder());
    let result = reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)
        .and_then(|_| validate_structure_with_budget(program, function, &mut budget, None));
    (
        result,
        [
            budget.remaining,
            budget.skeleton_remaining,
            budget.skeleton_spent,
        ],
    )
}

#[test]
fn complete_ordinary_receiver_reaches_all_nine_selectors_and_drops() {
    let checked = crate::check(
        "module cursor.renewal; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}",
        "cursor-renewal.spx",
    ).unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();

    let invoke = |previous, function, cap, floor| {
        with_previous(previous, || {
            with_limit_usage_refusal(cap, || {
                assert!(set_active_floor(floor));
                receive(&program, function)
            })
        })
    };
    let mut changed = function.clone();
    changed.cleanup_plan.schema = "forged.schema";
    for subject in [function, &changed] {
        let (old, refused, _, first) = invoke(true, subject, usize::MAX, 0);
        assert!(!refused && first.is_none());
        let (new, refused, _, first) = invoke(false, subject, usize::MAX, 0);
        assert!(!refused && first.is_none());
        assert_eq!(new, old);
        assert_eq!(new.0.is_ok(), std::ptr::eq(subject, function));
    }

    for previous in [true, false] {
        let (complete, refused, paid, first) = invoke(previous, function, usize::MAX, 0);
        assert!(complete.0.is_ok() && !refused && first.is_none());
        let (exact, refused, exact_paid, first) = invoke(previous, function, paid + 17, 17);
        assert_eq!(exact, complete);
        assert_eq!(exact_paid, paid);
        assert!(!refused && first.is_none());
        let (short, refused, _, first) = invoke(previous, function, paid + 16, 17);
        assert!(short.0.is_err() && refused && first.is_some());
        let (zero, refused, _, first) = invoke(previous, function, 0, 0);
        assert!(zero.0.is_err() && refused && first.is_some());

        let (((success, heap), refused, observed_paid, first), trace) = with_phase_trace(|| {
            with_previous(previous, || {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| receive(&program, function).0.is_ok())
                })
            })
        });
        assert!(success && !refused && first.is_none());
        assert_eq!(observed_paid, paid);
        assert!(!heap.invalid && !trace.overflow);
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
        if previous {
            assert!(trace.function_peak("schema-previous").is_some());
        } else {
            assert!(trace.function_peak("schema-current").is_some());
            let selectors = [
                "schema-record-update",
                "schema-byte-renewal",
                "schema-text-renewal",
                "schema-vector-renewal",
                "schema-owned-iterator",
                "schema-record-iterator",
                "schema-iterator-default",
                "schema-iterator-record",
                "schema-iterator-loop",
            ];
            let allocations = selectors.map(|selector| {
                let (entry, peak, exit) = trace
                    .function_peak(selector)
                    .unwrap_or_else(|| panic!("ordinary caller skipped {selector}"));
                assert!(peak >= entry.live && peak >= exit.live);
                exit.allocated.checked_sub(entry.allocated).unwrap()
            });
            assert!(allocations[..5].iter().all(|&bytes| bytes > 0));
            assert_eq!(
                allocations[5], 0,
                "record scan reuses the owned scan's buffer"
            );
            assert!(allocations[6..8].iter().all(|&bytes| bytes > 0));
            assert_eq!(
                allocations[8], 0,
                "iterator-loop scan reuses the same buffer"
            );
        }
    }
}

#[test]
fn selector_walk_refuses_a_foreign_scope_and_remains_sticky_on_restore() {
    use crate::bounded_output::{active_refused, active_remaining};
    let [program, ..] = paid_type_facts::tests::fixtures();
    let function = &program.functions[0];
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut payer = paid_type_facts::Payer::capture(function, &mut budget).unwrap();
        let mut walk = iterator_presence::Walk::new(function, &mut payer).unwrap();
        let outer_remaining = active_remaining();
        let (inner, foreign_refused, foreign_paid, foreign_first) =
            with_limit_usage_refusal(usize::MAX, || walk.uses_owned_iterator());
        assert!(inner.is_err());
        assert!(!foreign_refused && foreign_paid == 0 && foreign_first.is_none());
        assert!(active_refused());
        assert_eq!(active_remaining(), outer_remaining);
        assert!(walk.contains_iterator_loop().is_err());
        assert_eq!(active_remaining(), outer_remaining);
    });
    assert!(refused);
    assert!(first.is_some());
}
