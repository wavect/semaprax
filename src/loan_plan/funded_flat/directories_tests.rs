//! Whole pre-change borrowed BTree receiver, independently from the older
//! public planner. Inputs/Budget are outside; owner, discovery, construction,
//! result receiving and Drop are inside each actual allocator observation.
use super::*;
use crate::loan_plan::build_plan;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};

fn observe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expected: &LoanPlan,
    old: bool,
    limit: usize,
) -> (
    (bool, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let owner = match CfgReservation::acquire() {
                Ok(owner) => owner,
                Err(_) => return false,
            };
            let result = if old {
                funded_flat_legacy::build(program, function, &owner)
            } else {
                build(program, function, &owner)
            };
            match result {
                Ok(plan) => {
                    assert_eq!(&plan, expected);
                    drop(plan);
                    assert_eq!(owner.first(), None);
                    drop(owner);
                    true
                }
                Err(PlannerError::Physical(first)) => {
                    assert_eq!(owner.first(), Some(first));
                    assert_eq!(owner.reserve(0), Err(first));
                    assert_eq!(owner.reserve(1), Err(first));
                    let pointer = owner.text_pointer();
                    let report = owner.diagnostic();
                    assert_eq!(report.message.as_ptr(), pointer);
                    drop(report);
                    false
                }
                Err(PlannerError::Semantic(error)) => panic!("positive receiver: {error:?}"),
            }
        })
    })
}

fn whole(program: &ResolvedProgram, function: &ResolvedFunction) {
    let expected = saved_planner::build_plan(program, function).unwrap();
    assert_eq!(build_plan(program, function).unwrap(), expected);
    for old in [true, false] {
        let ((success, actual), refused, exact, first) =
            observe(program, function, &expected, old, usize::MAX);
        assert!(success);
        assert!(!refused);
        assert_eq!(first, None);
        assert!(actual.allocated <= exact);
        assert!(actual.highwater <= exact);
        assert_eq!(actual.freed, actual.allocated);
        assert_eq!(actual.live, 0);
        assert!(!actual.invalid);
        let ((success, repeated), refused, used, first) =
            observe(program, function, &expected, old, exact);
        assert!(success);
        assert!(!refused);
        assert_eq!(used, exact);
        assert_eq!(first, None);
        assert_eq!(repeated, actual);
        let ((success, short), refused, used, first) =
            observe(program, function, &expected, old, exact - 1);
        assert!(!success);
        assert!(refused);
        assert!(first.is_some());
        assert!(used <= exact - 1);
        assert!(short.allocated <= used);
        assert_eq!(short.freed, short.allocated);
        assert_eq!(short.live, 0);
        assert!(!short.invalid);
        // Both measured receivers are reported. Per-node AVL cost may exceed
        // BTree allocation; no relative savings or normalized debit is assumed.
        eprintln!("paid-plan BTree={old} actual={actual:?} debit={exact}");
    }
}

#[test]
fn complete_old_btree_receiver_constructor_drop_exact_short_and_original_applications() {
    for count in [1, 3, 17] {
        let (mut program, index) = boundary_tests::fixture_with_uses(count, true);
        assert!(
            !saved_planner::build_plan(&program, &program.functions[index])
                .unwrap()
                .loans
                .is_empty()
        );
        whole(&program, &program.functions[index]);
        // Attached inventory never supplies allocation or canonical authority.
        program.functions[index].loan_plan.loans.clear();
        program.functions[index].loan_plan.endpoints.reverse();
        program.functions[index].loan_plan.edges.reverse();
        whole(&program, &program.functions[index]);
    }
    for source in [
        include_str!("../../../tests/language/while_loops/record_borrow_renewal.spx"),
        include_str!("../../../tests/fixtures/nested-collection-records.spx"),
        include_str!("../../../tests/fixtures/nested-collection-string-record.spx"),
        include_str!("../../../tests/fixtures/owned-nested-outcome.spx"),
    ] {
        let ast = crate::check(source, "paid-directory-application.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        for function in &program.functions {
            whole(&program, function);
        }
    }
}

#[test]
fn complete_old_btree_plan_and_checked_work_first_errors_keep_exact_priority() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        old_work.allocation = Some(&owner);
        let expected = paid_plan_legacy::build(&program, function, &mut old_work).unwrap();
        let complete_work = old_work.used;
        for limit in [0, 1, 3, 9, complete_work - 1, complete_work] {
            let mut old_work = WorkCounter::new(limit);
            old_work.allocation = Some(&owner);
            let old = paid_plan_legacy::build(&program, function, &mut old_work);
            let mut current_work = WorkCounter::new(limit);
            current_work.allocation = Some(&owner);
            let current = paid_plan::build(&program, function, &mut current_work);
            assert_eq!(current, old);
            assert_eq!(current_work.used, old_work.used);
            if limit == complete_work {
                assert_eq!(current.unwrap(), expected);
            } else {
                assert_eq!(
                    current.unwrap_err().message,
                    "loan analysis exceeds 1,000,000 checked work"
                );
            }
            assert_eq!(owner.first(), None);
        }
    });
    assert!(!refused);
    let mut forged = function.clone();
    let ResolvedExprKind::Block { statements, .. } = &forged.body.kind else {
        panic!("original boundary block");
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("original own root");
    };
    forged.body.id = value.id.clone();
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        for limit in [0, MAX_LOAN_PLAN_WORK_V1] {
            let mut old_work = WorkCounter::new(limit);
            old_work.allocation = Some(&owner);
            let old = paid_plan_legacy::build(&program, &forged, &mut old_work).unwrap_err();
            let mut current_work = WorkCounter::new(limit);
            current_work.allocation = Some(&owner);
            assert_eq!(
                paid_plan::build(&program, &forged, &mut current_work).unwrap_err(),
                old
            );
            assert_eq!(current_work.used, old_work.used);
            assert_eq!(owner.first(), None);
        }
    });
    assert!(!refused);
}
