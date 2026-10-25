//! Complete pre-batch route remains the independent diagnostic/control oracle.
use super::super::super::{
    expression_facts::errors::EngineError, Diagnostic, ReplayBudget, ResolvedFunction,
};
use super::{flat_program, program};
use crate::test_allocation_observer::measure;

fn legacy(error: EngineError, function: &ResolvedFunction) -> Diagnostic {
    match error {
        EngineError::Owned(error) => error,
        EngineError::Structural(receipt) => receipt.legacy(function),
        other => panic!("unexpected selected inventory error: {other:?}"),
    }
}
fn run(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) -> (Result<(), Diagnostic>, usize, usize) {
    let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    let work = budget.remaining;
    let ledger = std::ptr::from_ref(budget.physical.as_deref().unwrap());
    let result = if old {
        super::super::paid_before::validate_inventory_coverage(program, function, &mut budget)
    } else {
        super::super::paid::validate_inventory_coverage(program, function, &mut budget)
    };
    assert!(std::ptr::eq(ledger, budget.physical.as_deref().unwrap()));
    // Detailed formatting is explicitly outside the paid production route.
    let result = result.map_err(|error| legacy(error, function));
    (
        result,
        budget.physical.as_ref().unwrap().charged(),
        work - budget.remaining,
    )
}
#[test]
fn complete_selected_inventory_old_route_keeps_success_all_fields_and_full_drop() {
    for program in [program(), flat_program()] {
        for function in &program.functions {
            if !(function.id.as_str().starts_with("coverage.")
                || function.id.as_str().starts_with("flat."))
            {
                continue;
            }
            let ((old, old_bytes, old_work), old_drop) = measure(|| run(&program, function, true));
            let ((current, bytes, work), current_drop) = measure(|| run(&program, function, false));
            assert_eq!(old, Ok(()), "genuine original fixture: {}", function.id);
            assert_eq!(current, old, "all fields: {}", function.id);
            assert!(
                bytes >= old_bytes,
                "new full roles receive no old-fee credit"
            );
            assert!(work >= old_work, "old work is retained");
            eprintln!("selected-family {} old-bytes={old_bytes} new-bytes={bytes} old-work={old_work} new-work={work}", function.id);
            for observed in [old_drop, current_drop] {
                assert!(!observed.invalid && observed.live == 0);
                assert_eq!(observed.allocated, observed.freed);
            }
        }
    }
}
#[test]
fn complete_selected_inventory_hostile_base_entry_and_supplemental_match_six_fields() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "coverage.main")
        .unwrap();
    let base = function.cleanup.slots.len();
    assert!(base > 0 && function.cleanup_plan.slots.len() > base);
    for case in 0..6 {
        let mut forged = function.clone();
        match case {
            0 => forged.cleanup.schema = "hostile.inventory",
            1 => forged.cleanup_plan.slots.clear(),
            2 => forged.cleanup_plan.slots[0].storage_index ^= 1,
            3 => forged.cleanup_plan.slots[base].storage_index ^= 1,
            4 => {
                forged.cleanup_plan.slots.pop();
            }
            5 => {
                forged.cleanup_plan.slots[base].ty = crate::hir::ResolvedType::I64;
            }
            _ => unreachable!(),
        }
        let old = run(&program, &forged, true).0.unwrap_err();
        let current = run(&program, &forged, false).0.unwrap_err();
        assert_eq!(
            current, old,
            "complete original diagnostic fields for hostile case {case}"
        );
    }
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "coverage.fill")
        .unwrap();
    assert!(!function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .is_empty());
    let mut forged = function.clone();
    forged
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .clear();
    assert_eq!(
        run(&program, &forged, false).0.unwrap_err(),
        run(&program, &forged, true).0.unwrap_err()
    );
}
#[test]
fn selected_family_zero_and_one_short_formation_retains_first_failure_without_allocating() {
    use super::super::super::selected_context::child::Permit;
    use crate::bounded_output::{clear_active_floor, set_active_floor, with_limit_usage_refusal};
    let program = program();
    let function = &program.functions[0];
    let bytes =
        std::mem::size_of::<super::super::super::selected_context::child::FamilyReceiving<'_>>();
    for headroom in [0, bytes - 1] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let charged = budget.physical.as_ref().unwrap().charged();
        let (((), observed), refused, _, first) = with_limit_usage_refusal(1 << 20, || {
            assert!(set_active_floor((1 << 20) - headroom));
            measure(|| {
                let error = match Permit::family_receiver::<()>(function, &mut budget) {
                    Ok(_) => panic!("finite receiving cannot fit"),
                    Err(error) => error,
                };
                assert!(
                    matches!(error, EngineError::Owned(_)),
                    "genuine prepaid budget error only"
                );
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                let original = budget
                    .physical
                    .as_deref_mut()
                    .unwrap()
                    .reserve(0)
                    .unwrap_err();
                clear_active_floor();
                assert_eq!(
                    budget.physical.as_deref_mut().unwrap().reserve(0),
                    Err(original)
                );
                drop(error);
            })
        });
        assert!(refused && first.is_some());
        assert_eq!(
            (observed.allocations, observed.reallocations, observed.live),
            (0, 0, 0)
        );
        assert!(!observed.invalid);
    }
    for work in [0, 2] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        budget.remaining = work;
        let (_, observed) = measure(|| {
            assert!(Permit::family_receiver::<()>(function, &mut budget).is_err());
            assert_eq!(budget.remaining, 0);
            assert!(Permit::family_receiver::<()>(function, &mut budget).is_err());
        });
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
    }
}

#[test]
fn selected_family_exact_full_receiving_floor_and_work_have_no_unpriced_success() {
    use super::super::super::{
        selected_context::{
            self,
            child::{BeforeFamilyReceiving as FamilyReceiving, FamilyCallReceiving, Permit},
        },
        semantic_refusal,
    };
    use crate::bounded_output::{clear_active_floor, set_active_floor, with_limit_usage_refusal};
    use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
    // Complete unchanged Permit::acquire<()> fields, independent of the new
    // family layer. No old carrier or header is credited against a new role.
    type Original<'a> = (
        Permit<'a>,
        Refusal,
        Option<Refusal>,
        semantic_refusal::Receipt,
        EngineError,
        Option<EngineError>,
        Result<(), Refusal>,
        Result<(), EngineError>,
        Result<Permit<'a>, EngineError>,
        Result<selected_context::Context<'a>, EngineError>,
        Result<(), EngineError>,
    );
    let family = std::mem::size_of::<FamilyReceiving<'_>>();
    let caller = std::mem::size_of::<FamilyCallReceiving<'_>>();
    let total = caller + family + caller + std::mem::size_of::<Original<'_>>();
    let program = program();
    let function = &program.functions[0];
    for headroom in [caller + family, total - 1, total] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let charged = budget.physical.as_ref().unwrap().charged();
        let (((), observed), refused, _, _) = with_limit_usage_refusal(1 << 20, || {
            assert!(set_active_floor((1 << 20) - headroom));
            measure(|| {
                let result = (|| {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                        budget,
                        function,
                        FamilyCallReceiving<'_>,
                        3,
                        "selected family call receiving",
                        Permit::family_receiver_before_formals::<()>(function, &mut budget)
                    )
                })();
                assert_eq!(result.is_ok(), headroom == total);
                let expected = if headroom == total {
                    total
                } else if headroom < caller + family + caller {
                    caller + family
                } else {
                    caller + family + caller
                };
                assert_eq!(
                    budget.physical.as_ref().unwrap().charged() - charged,
                    expected
                );
                drop(result);
                clear_active_floor();
            })
        });
        assert_eq!(refused, headroom != total);
        assert_eq!(
            (observed.allocations, observed.reallocations, observed.live),
            (0, 0, 0)
        );
        assert!(!observed.invalid);
    }
    for work in [0, 2, 3, 11, 12, 14, 15, 25, 26] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        budget.remaining = work;
        let (succeeded, observed) = measure(|| {
            let result = (|| {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    FamilyCallReceiving<'_>,
                    3,
                    "selected family call receiving",
                    Permit::family_receiver_before_formals::<()>(function, &mut budget)
                )
            })();
            result.is_ok()
        });
        assert_eq!(succeeded, work == 26);
        assert_eq!(budget.remaining, 0);
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    }
}

#[test]
fn current_selected_family_wrapper_formals_have_exact_short_zero_work_and_drop() {
    use super::super::super::{
        selected_context::{
            self,
            child::{FamilyCallReceiving, FamilyReceiving, Permit, WrapperFormalReceiving},
        },
        semantic_refusal,
    };
    use crate::bounded_output::{clear_active_floor, set_active_floor, with_limit_usage_refusal};
    use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
    // Complete unchanged Permit::acquire<()> fields, independent of the new
    // family layer. No old carrier or header is credited against a new role.
    type Original<'a> = (
        Permit<'a>,
        Refusal,
        Option<Refusal>,
        semantic_refusal::Receipt,
        EngineError,
        Option<EngineError>,
        Result<(), Refusal>,
        Result<(), EngineError>,
        Result<Permit<'a>, EngineError>,
        Result<selected_context::Context<'a>, EngineError>,
        Result<(), EngineError>,
    );
    let family = std::mem::size_of::<FamilyReceiving<'_>>();
    let source = std::mem::size_of::<&crate::hir::ExpressionId>();
    let caller = std::mem::size_of::<FamilyCallReceiving<'_>>();
    let formal = std::mem::size_of::<WrapperFormalReceiving<'_>>();
    let total = caller + formal + family + source + caller + std::mem::size_of::<Original<'_>>();
    let program = program();
    let function = &program.functions[0];
    for headroom in [caller + formal + family, total - 1, total] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let charged = budget.physical.as_ref().unwrap().charged();
        let (((), observed), refused, _, _) = with_limit_usage_refusal(1 << 20, || {
            assert!(set_active_floor((1 << 20) - headroom));
            measure(|| {
                let result = (|| {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
                        budget,
                        function,
                        FamilyCallReceiving<'_>,
                        3,
                        "selected family call receiving",
                        Permit::family_receiver::<()>(function, &mut budget)
                    )
                })();
                assert_eq!(result.is_ok(), headroom == total);
                let expected = if headroom == total {
                    total
                } else if headroom == caller + formal + family {
                    caller + formal + family
                } else {
                    caller + formal + family + source + caller
                };
                assert_eq!(
                    budget.physical.as_ref().unwrap().charged() - charged,
                    expected
                );
                drop(result);
                clear_active_floor();
            })
        });
        assert_eq!(refused, headroom != total);
        assert_eq!(
            (observed.allocations, observed.reallocations, observed.live),
            (0, 0, 0)
        );
        assert!(!observed.invalid);
    }
    for work in [0, 2, 3, 5, 6, 15, 16, 18, 19, 29, 30] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        budget.remaining = work;
        let (succeeded, observed) = measure(|| {
            let result = (|| {
                crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
                    budget,
                    function,
                    FamilyCallReceiving<'_>,
                    3,
                    "selected family call receiving",
                    Permit::family_receiver::<()>(function, &mut budget)
                )
            })();
            result.is_ok()
        });
        assert_eq!(succeeded, work == 30);
        assert_eq!(budget.remaining, 0);
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    }
}
