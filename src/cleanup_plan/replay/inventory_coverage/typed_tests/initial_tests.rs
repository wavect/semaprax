//! Actual selected coverage owns admission before any raw inventory receiver.
use super::super::super::{Diagnostic, ReplayBudget};
use super::{flat_program, program};
use crate::test_allocation_observer::measure;

fn legacy(
    error: super::super::super::expression_facts::errors::EngineError,
    function: &crate::hir::ResolvedFunction,
) -> Diagnostic {
    use super::super::super::expression_facts::errors::EngineError;
    match error {
        EngineError::Owned(error) => error,
        EngineError::Structural(receipt) => receipt.legacy(function),
        other => panic!("unexpected inventory failure: {other:?}"),
    }
}

#[test]
fn initial_receiving_precedes_raw_entry_and_preserves_complete_inventory_through_drop() {
    for program in [program(), flat_program()] {
        for function in &program.functions {
            if !(function.id.as_str().starts_with("coverage.")
                || function.id.as_str().starts_with("flat."))
            {
                continue;
            }
            for old in [true, false] {
                let ((), observed) = measure(|| {
                    let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
                    let before = budget.physical.as_ref().unwrap().charged();
                    let remaining = budget.remaining;
                    crate::cleanup::reset_typed_inventory_initial_entries();
                    if old {
                        super::super::validate_inventory_coverage(&program, function, &mut budget)
                            .unwrap();
                    } else {
                        super::super::paid::validate_inventory_coverage(
                            &program,
                            function,
                            &mut budget,
                        )
                        .unwrap();
                    }
                    let (entries, physical, remaining_pointer, entry_charged) =
                        crate::cleanup::typed_inventory_initial_entries();
                    assert_eq!(entries, usize::from(!old));
                    if !old {
                        assert_eq!(
                            physical,
                            std::ptr::from_ref(budget.physical.as_deref().unwrap()) as usize
                        );
                        assert_eq!(
                            remaining_pointer,
                            std::ptr::from_ref(&budget.remaining) as usize
                        );
                        assert!(
                            entry_charged
                                >= before
                                    + std::mem::size_of::<
                                        crate::cleanup::TypedInventoryInitialReceiving,
                                    >()
                        );
                        assert!(budget.physical.as_ref().unwrap().charged() >= entry_charged);
                    }
                    assert!(budget.physical.as_ref().unwrap().charged() > before);
                    assert!(budget.remaining < remaining);
                    eprintln!(
                        "inventory initial receiving {} old={old} charged={} work={}",
                        function.id,
                        budget.physical.as_ref().unwrap().charged(),
                        remaining - budget.remaining
                    );
                    drop(budget);
                });
                assert!(!observed.invalid && observed.live == 0);
                assert_eq!(observed.allocated, observed.freed);
            }
            let mut forged = function.clone();
            // Change a real inventory field without changing the typed program.
            forged.cleanup.schema = if forged.cleanup.schema == "invalid" {
                "other-invalid"
            } else {
                "invalid"
            };
            let mut a = ReplayBudget::with_physical(&forged, usize::MAX).unwrap();
            let mut b = ReplayBudget::with_physical(&forged, usize::MAX).unwrap();
            let old =
                super::super::validate_inventory_coverage(&program, &forged, &mut a).unwrap_err();
            let current = legacy(
                super::super::paid::validate_inventory_coverage(&program, &forged, &mut b)
                    .unwrap_err(),
                &forged,
            );
            assert_eq!(current, old, "complete six-field inventory mismatch");
        }
    }
}

// This test-only adapter expands the SAME lexical preflight and complete raw
// call as shipping coverage. No forwarding helper is introduced in production.
fn initial_call(
    program: &dyn crate::cleanup::ReplayContext,
    function: &crate::hir::ResolvedFunction,
    budget: &mut ReplayBudget,
    permit: super::super::super::selected_context::child::Permit<'_>,
) -> Result<
    crate::cleanup::CleanupInventory,
    super::super::super::expression_facts::errors::EngineError,
> {
    super::super::paid::admit_initial!(budget, function, permit);
    let super::super::super::ReplayBudget {
        remaining,
        physical,
        ..
    } = budget;
    let physical = physical
        .as_deref_mut()
        .expect("same actual selected budget");
    match crate::cleanup::build_prepaid_typed_flat_record_inventory(
        program, function, physical, remaining,
    ) {
        Ok(inventory) => Ok(inventory),
        Err(failure) => Err(permit.refuse(
            crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Inventory(failure),
        )),
    }
}

#[test]
fn initial_admission_exact_short_zero_work_and_floor_keep_first_receipt() {
    use crate::bounded_output::{clear_active_floor, set_active_floor, with_limit_usage_refusal};
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "coverage.fill")
        .unwrap();
    let size = std::mem::size_of::<crate::cleanup::TypedInventoryInitialReceiving>();
    for headroom in [0, size - 1, size] {
        let limit = 1 << 20;
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let permit = super::super::super::selected_context::child::Permit::acquire::<()>(
            function,
            &mut budget,
        )
        .unwrap();
        let before = budget.physical.as_ref().unwrap().charged();
        // Stop after admitted raw entry at its first genuine internal work debit.
        budget.remaining = crate::cleanup::TYPED_INVENTORY_INITIAL_FIELDS;
        crate::cleanup::reset_typed_inventory_initial_entries();
        let (((), observed), refused, _, first) = with_limit_usage_refusal(limit, || {
            assert!(set_active_floor(limit - headroom));
            measure(|| {
                let result = initial_call(&program, function, &mut budget, permit);
                assert!(result.is_err());
                let entries = crate::cleanup::typed_inventory_initial_entries().0;
                assert_eq!(entries, usize::from(headroom == size));
                let charged = budget.physical.as_ref().unwrap().charged();
                if headroom < size {
                    assert_eq!(charged, before);
                } else {
                    assert!(charged >= before + size);
                }
                let first_physical = budget
                    .physical
                    .as_deref_mut()
                    .unwrap()
                    .reserve(0)
                    .unwrap_err();
                clear_active_floor();
                assert_eq!(
                    budget.physical.as_deref_mut().unwrap().reserve(0),
                    Err(first_physical)
                );
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                drop(result);
                assert_eq!(
                    budget.physical.as_ref().unwrap().charged(),
                    charged,
                    "result Drop cannot refund initial receiving"
                );
            })
        });
        assert!(refused && first.is_some());
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(
            observed.allocations, 0,
            "initial failure only moves already funded diagnostics/sealed data"
        );
        drop(budget);
    }
    for work in [0, 43, 44] {
        let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let permit = super::super::super::selected_context::child::Permit::acquire::<()>(
            function,
            &mut budget,
        )
        .unwrap();
        budget.remaining = work;
        let before = budget.physical.as_ref().unwrap().charged();
        crate::cleanup::reset_typed_inventory_initial_entries();
        let failure = initial_call(&program, function, &mut budget, permit).unwrap_err();
        assert_eq!(budget.remaining, 0);
        assert!(
            budget.physical.as_ref().unwrap().charged() >= before + size,
            "admission bytes remain spent on work failure"
        );
        assert_eq!(
            crate::cleanup::typed_inventory_initial_entries().0,
            usize::from(work == 44)
        );
        let current = legacy(failure, function);
        let expected =
            super::super::super::replay_error(function, "selected inventory work exhausted");
        assert_eq!(
            current, expected,
            "complete six-field selected first work refusal"
        );
        drop(current);
        drop(budget);
    }
}
