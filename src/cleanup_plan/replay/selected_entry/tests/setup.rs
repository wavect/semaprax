//! Initial setup returns an inline reason until a real outer report owner pays.
use super::*;
use crate::test_allocation_observer::measure;
fn required(function: &ResolvedFunction) -> usize {
    std::mem::size_of::<PhysicalReservation>()
        + 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>()
        + function.id.as_str().len()
        + 512
}
#[test]
fn inline_setup_zero_and_each_one_short_return_the_exact_sticky_ledger_without_allocating() {
    let program = program();
    let function = members(&program)[0];
    let reservation = std::mem::size_of::<PhysicalReservation>();
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    for (limit, charged, reason) in [
        (
            0,
            0,
            "cleanup replay physical reservation exceeds the bound",
        ),
        (
            reservation - 1,
            0,
            "cleanup replay physical reservation exceeds the bound",
        ),
        (
            reservation + cursor - 1,
            reservation,
            "cleanup replay clone cursor reservation refused",
        ),
        (
            required(function) - 1,
            reservation + cursor,
            "cleanup replay refusal text reservation refused",
        ),
    ] {
        let ((actual_charged, actual_reason), allocations) = measure(|| {
            let failure = match Preparation::try_new(function, PhysicalReservation::new(limit)) {
                Ok(_) => panic!("one-short setup cannot publish preparation"),
                Err(failure) => failure,
            };
            let (mut failed, selected) = failure.into_parts();
            let first = failed.reserve(0).unwrap_err();
            assert_eq!(failed.reserve(1).unwrap_err(), first);
            assert_eq!(failed.reserve(0).unwrap_err(), first);
            let charged = failed.charged();
            #[allow(
                clippy::drop_non_drop,
                reason = "End this borrowed proof at the measured lifetime boundary."
            )]
            drop(failed);
            (charged, selected.message())
        });
        assert_eq!((actual_charged, actual_reason), (charged, reason));
        assert!(!allocations.invalid);
        assert_eq!(
            (
                allocations.allocations,
                allocations.allocated,
                allocations.live
            ),
            (0, 0, 0)
        );
    }
}
#[test]
fn inline_setup_exact_preparation_preserves_first_prepaid_text_pointer_receipt_and_drop() {
    let program = program();
    let function = members(&program)[0];
    let bytes = required(function);
    let (_, allocations) = measure(|| {
        let mut preparation = match Preparation::try_new(function, PhysicalReservation::new(bytes))
        {
            Ok(preparation) => preparation,
            Err(_) => panic!("exact setup must complete"),
        };
        let pointer = preparation.budget.refusal_text.as_ref().unwrap().as_ptr();
        let physical = preparation.budget.physical.as_deref().unwrap();
        let ledger = physical as *const PhysicalReservation;
        assert_eq!(physical.charged(), bytes);
        let error = preparation
            .reserve_array::<u8>(function, 1, "setup owning test")
            .unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(error.message.as_ptr(), pointer);
        assert!(error.message.contains(function.id.as_str()));
        assert!(preparation.failed);
        let physical = preparation.budget.physical.as_deref_mut().unwrap();
        assert_eq!(
            physical as *mut PhysicalReservation as *const PhysicalReservation,
            ledger
        );
        assert_eq!(physical.charged(), bytes);
        let first = physical.reserve(0).unwrap_err();
        assert_eq!(physical.reserve(1).unwrap_err(), first);
        let retry = preparation.budget.budget_refusal(function, "retry", "");
        assert_eq!(retry.code, "SPX-H006");
        assert!(retry.message.is_empty());
        assert_eq!(
            preparation.budget.physical.as_deref().unwrap().charged(),
            bytes
        );
        drop(retry);
        drop(error);
        drop(preparation);
    });
    assert!(!allocations.invalid);
    assert_eq!(allocations.allocations, 2);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(
        allocations.allocated,
        std::mem::size_of::<PhysicalReservation>() + function.id.as_str().len() + 512
    );
    assert_eq!(allocations.live, 0);
    assert_eq!(allocations.allocated, allocations.freed);
}
