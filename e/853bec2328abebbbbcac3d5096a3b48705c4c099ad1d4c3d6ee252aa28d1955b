//! Real inventory child refusal transfers the original report/ledger pair.
use super::*;
use crate::cleanup_plan::replay::selected_context::family::Reason;
use crate::cleanup_plan::replay::{inventory_coverage, selected_context::child::Permit};

fn exercise(program: &crate::hir::ResolvedProgram, observe_publish: bool) {
    let mut subject = members(program)[0].clone();
    subject.cleanup.schema = "hostile.selected.inventory";
    let mut preparation = prepared(&subject);
    let backing = preparation.report_backing();
    let ledger = std::ptr::from_ref(preparation.budget.physical.as_deref().unwrap());
    // A genuinely second prepaid receiver; no receipt is manufactured after
    // the first failure by retrying the already sticky ledger.
    preparation.budget.reserve_array::<crate::cleanup_plan::replay::selected_context::child::WrapperFormalReceiving<'_>>(
        &subject,1,"selected wrapper formal caller frontier").unwrap();
    preparation
        .budget
        .charge(&subject, 3, "selected wrapper formal caller frontier")
        .unwrap();
    preparation.budget.reserve_array::<crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>>(&subject,1,"selected family call receiving").unwrap();
    preparation
        .budget
        .charge(&subject, 3, "selected family call receiving")
        .unwrap();
    let later = Permit::family_receiver::<()>(&subject, &mut preparation.budget).unwrap();
    let error = inventory_coverage::paid::validate_inventory_coverage(
        program,
        &subject,
        &mut preparation.budget,
    )
    .unwrap_err();
    let crate::cleanup_plan::replay::expression_facts::errors::EngineError::Structural(receipt) =
        &error
    else {
        panic!("actual selected child must retain finite error data");
    };
    let (source, reason) = receipt.borrowed_family_receipt().unwrap();
    assert_eq!(
        reason,
        Reason::CleanupInventoryDisagreesWithIndependentlyRebuiltTypedHir
    );
    assert_eq!(source.as_str().as_ptr(), subject.body.id.as_str().as_ptr());
    let source_pointer = source.as_str().as_ptr();
    let charged = preparation.budget.physical.as_ref().unwrap().charged();
    let mut refusal = if observe_publish {
        let (refusal, observed) = measure(|| preparation.refuse_engine(error));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        refusal
    } else {
        preparation.refuse_engine(error)
    };
    assert_paid(&refusal, backing);
    assert!(std::ptr::eq(refusal.ledger_mut().unwrap(), ledger));
    assert_eq!(refusal.ledger_mut().unwrap().charged(), charged);
    let first = first_receipt(&mut refusal);
    let retained = refusal
        .structural_receipt()
        .unwrap()
        .borrowed_family_receipt()
        .unwrap();
    assert_eq!(retained.1, reason);
    assert_eq!(retained.0.as_str().as_ptr(), source_pointer);
    let preparation = refusal.into_preparation().unwrap();
    let mut refusal = preparation.refuse_engine(later.family(Reason::BaseSlot(usize::MAX)));
    assert_paid(&refusal, backing);
    assert_eq!(first_receipt(&mut refusal), first);
    assert_eq!(
        refusal
            .structural_receipt()
            .unwrap()
            .borrowed_family_receipt()
            .unwrap()
            .1,
        reason
    );
}
#[test]
fn actual_selected_child_keeps_exact_report_source_ledger_first_receipt_and_full_drop() {
    let program = program();
    exercise(&program, true);
    let (_, observed) = measure(|| exercise(&program, false));
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
