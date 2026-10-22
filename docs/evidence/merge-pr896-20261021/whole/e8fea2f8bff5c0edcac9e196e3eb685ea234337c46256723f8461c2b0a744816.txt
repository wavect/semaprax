//! Real Session semantic failure retains the preacquired carrier and same ledger.
use super::*;
use crate::cleanup_plan::replay::{
    expression_facts::errors::EngineError,
    primitive_fixture,
    selected_context::child::Permit,
    structural_control::selected::reason::{Kind, Reason},
};
fn exercise(observe_publish: bool) {
    let mut program = tiny();
    let index = program
        .functions
        .iter()
        .position(|f| f.id.as_str() == "b")
        .unwrap();
    program.functions[index].cleanup_plan.entry = crate::cleanup_plan::BlockId(u32::MAX);
    let source = primitive_fixture::Context::new(&program);
    let subject = &program.functions[index];
    let mut preparation = prepared(subject);
    let backing = preparation.report_backing();
    let ledger = std::ptr::from_ref(preparation.budget.physical.as_deref().unwrap());
    preparation.budget.reserve_array::<crate::cleanup_plan::replay::selected_context::child::WrapperFormalReceiving<'_>>(
        subject,1,"selected wrapper formal caller frontier").unwrap();
    preparation
        .budget
        .charge(subject, 3, "selected wrapper formal caller frontier")
        .unwrap();
    preparation.budget.reserve_array::<crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>>(
        subject,1,"selected control call receiving").unwrap();
    preparation
        .budget
        .charge(subject, 3, "selected control call receiving")
        .unwrap();
    let later = Permit::control_receiver::<()>(subject, &mut preparation.budget).unwrap();
    let mut session = preparation
        .into_paid_session(&source, subject)
        .unwrap_or_else(|_| panic!("actual Session setup"));
    let error = session.validate_scalar_paid(subject).unwrap_err();
    let EngineError::Structural(receipt) = &error else {
        panic!("semantic unknown entry must be typed: {error:?}")
    };
    let (retained, reason) = receipt.borrowed_control_receipt().unwrap();
    assert!(matches!(
        reason,
        Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
    ));
    assert_eq!(
        retained.as_str().as_ptr(),
        subject.body.id.as_str().as_ptr()
    );
    let source_pointer = retained.as_str().as_ptr();
    assert!(matches!(
        session.validate_scalar_paid(subject),
        Err(EngineError::Failed)
    ));
    let preparation = session.into_failed_preparation();
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
        .borrowed_control_receipt()
        .unwrap();
    assert_eq!(retained.0.as_str().as_ptr(), source_pointer);
    assert!(matches!(
        retained.1,
        Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
    ));
    let preparation = refusal.into_preparation().unwrap();
    let mut refusal =
        preparation.refuse_engine(later.control(Reason::Finite(Kind::CleanupCfgContainsACycle)));
    assert_paid(&refusal, backing);
    assert_eq!(first_receipt(&mut refusal), first);
    let retained = refusal
        .structural_receipt()
        .unwrap()
        .borrowed_control_receipt()
        .unwrap();
    assert_eq!(retained.0.as_str().as_ptr(), source_pointer);
    assert!(matches!(
        retained.1,
        Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
    ));
}
#[test]
fn actual_paid_session_unknown_entry_keeps_source_carrier_ledger_sticky_failure_and_full_drop() {
    exercise(true);
    let (_, observed) = measure(|| exercise(false));
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
