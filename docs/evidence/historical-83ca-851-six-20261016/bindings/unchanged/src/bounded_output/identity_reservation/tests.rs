use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn actual_builder_identity_report_setup_exact_short_sticky_and_pointer_drop() {
    assert_eq!(MESSAGE.len(), 48);
    let payload = size_of::<Diagnostic>() + 48;
    let request = size_of::<IdentityReservation>() + payload;
    let (((), observed), failed, used, first) = with_limit_usage_refusal(request, || {
        measure(|| {
            let mut owner = IdentityReservation::acquire().unwrap();
            let pointers = owner.pointers();
            let error = owner.reserve(1).unwrap_err();
            assert_eq!(owner.reserve(0), Err(error));
            assert_eq!(owner.reserve(1), Err(error));
            let report = owner.publish();
            assert_eq!(report.as_ptr(), pointers.0);
            assert_eq!(report[0].message.as_ptr(), pointers.1);
            assert_eq!(report[0].code, "SPX-H006");
            assert_eq!(report[0].message, MESSAGE);
            drop(report);
        })
    });
    assert!(failed);
    assert_eq!(used, request);
    assert_eq!(first.unwrap().requested, 1);
    assert_eq!(observed.allocations, 2);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, payload);
    assert_eq!(observed.freed, payload);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((error, observed), failed, used, first) = with_limit_usage_refusal(request - 1, || {
        measure(|| IdentityReservation::acquire().err().unwrap())
    });
    assert!(matches!(error, Refused::Budget(_)));
    assert!(failed);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, request);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.live, 0);
}

#[test]
fn identity_owner_cannot_spend_nested_or_restored_budget() {
    let request = size_of::<IdentityReservation>() + size_of::<Diagnostic>() + MESSAGE.len();
    let (mut escaped, failed, used) = with_limit_usage(request, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let ((refusal, inner_remaining), inner_failed, inner_used) =
            with_limit_usage(10, || (owner.reserve(1), active_remaining()));
        assert_eq!(refusal, Err(Refused::Detached));
        assert_eq!(inner_remaining, Some(0)); // parent has already spent its whole allowance
        assert!(!inner_failed);
        assert_eq!(inner_used, 0);
        assert!(matches!(owner.reserve(0), Err(Refused::Budget(_))));
        owner
    });
    assert!(failed);
    assert_eq!(used, request);
    assert_eq!(escaped.reserve(0), Err(Refused::Detached));
    drop(escaped);
    let (error, observed) = measure(|| IdentityReservation::acquire().err().unwrap());
    assert_eq!(error, Refused::NoOwner);
    assert_eq!(observed.allocations, 0);
}
