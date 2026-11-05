use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::rc::Rc;

fn program() -> ResolvedProgram {
    let source = "module branch_funded; @id(\"main\") fn main()->i64 { 1 }";
    let ast = crate::check(source, "branch-funded.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
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

#[allow(
    clippy::type_complexity,
    reason = "The independently compiled control retains its exact tuple layout and receiving types."
)]
fn observe(
    current: bool,
    values: &Vec<Rc<usize>>,
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
) -> (
    (
        (Result<usize, Failure>, usize, usize),
        crate::test_allocation_observer::Metrics,
    ),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(bytes, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            let before = skeleton_materializations();
            let result = (|| {
                let copy = if current {
                    budget.reserve_bytes(
                        function,
                        controls::<Rc<usize>, Rc<usize>>(),
                        "branch clone receiving controls",
                    )?;
                    with_observation(&mut budget, function, values, Rc::clone(&values[0]))?
                } else {
                    let mut copy = skeleton_clone(
                        &mut budget,
                        function,
                        values,
                        "cleanup-plan skeleton branch clone",
                    )?;
                    skeleton_push(
                        &mut budget,
                        function,
                        &mut copy,
                        Rc::clone(&values[0]),
                        "plan branch observation push",
                    )?;
                    copy
                };
                assert_eq!(copy.len(), values.len() + 1);
                assert!(copy[..values.len()]
                    .iter()
                    .zip(values)
                    .all(|(a, b)| Rc::ptr_eq(a, b)));
                assert!(Rc::ptr_eq(copy.last().unwrap(), &values[0]));
                let capacity = copy.capacity();
                drop(copy);
                Ok(capacity)
            })();
            let result = result.map_err(failure);
            let remaining = budget.skeleton_remaining;
            let materialized = skeleton_materializations() - before;
            drop(budget);
            (result, remaining, materialized)
        })
    })
}

#[test]
fn fused_clone_push_pays_real_receivers_and_keeps_exact_short_zero_drop() {
    let program = program();
    let function = &program.functions[0];
    let values = (0..512).map(Rc::new).collect::<Vec<_>>();
    let ((old, old_alloc), old_refused, old_paid, old_first) =
        observe(false, &values, function, usize::MAX, usize::MAX);
    let ((actual, allocation), refused, exact, first) =
        observe(true, &values, function, usize::MAX, usize::MAX);
    eprintln!("branch_clone old_paid={old_paid} current_paid={exact} old_allocation={old_alloc:?} current_allocation={allocation:?}");
    assert!(!old_refused && old_first.is_none() && !refused && first.is_none());
    assert_eq!(actual, old);
    assert!(
        !allocation.invalid && allocation.live == 0 && !old_alloc.invalid && old_alloc.live == 0
    );
    assert_eq!(allocation.allocated, allocation.freed);
    assert_eq!(old_alloc.allocated, old_alloc.freed);
    assert!(allocation.allocated <= exact && old_alloc.allocated <= old_paid);
    for limit in [0, exact - 1, exact] {
        let ((outcome, allocation), refused, paid, first) =
            observe(true, &values, function, usize::MAX, limit);
        assert_eq!(outcome.0.is_ok(), limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_some(), limit != exact);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
    assert!(values.iter().all(|value| Rc::strong_count(value) == 1));
}

#[test]
fn fused_clone_keeps_original_push_work_refusal_before_capacity_payment() {
    let program = program();
    let function = &program.functions[0];
    let values = vec![Rc::new(1usize)];
    for work in [0, 1, 2] {
        let ((old, old_alloc), old_refused, _, old_first) =
            observe(false, &values, function, work, usize::MAX);
        let ((actual, allocation), refused, paid, first) =
            observe(true, &values, function, work, usize::MAX);
        assert_eq!(actual, old); // Complete six-field Diagnostic, work and materializations.
        assert_eq!(refused, old_refused);
        assert_eq!(first.is_some(), old_first.is_some());
        assert!(
            !allocation.invalid
                && allocation.live == 0
                && !old_alloc.invalid
                && old_alloc.live == 0
        );
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(old_alloc.allocated, old_alloc.freed);
        if work == 1 {
            let ((bounded, bounded_alloc), bounded_refused, bounded_paid, bounded_first) =
                observe(true, &values, function, work, paid);
            assert_eq!(bounded, actual);
            assert_eq!(bounded_refused, refused);
            assert_eq!(bounded_first, first);
            assert_eq!(bounded_paid, paid);
            assert!(!bounded_alloc.invalid && bounded_alloc.live == 0);
            assert_eq!(bounded_alloc.allocated, bounded_alloc.freed);
            let failure = actual.0.unwrap_err();
            assert!(std::str::from_utf8(&failure.message.0[..failure.message.1])
                .unwrap()
                .contains("plan branch observation push"));
        }
    }
}
