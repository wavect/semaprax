use super::WORK_MESSAGE;
use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::diagnostic::Diagnostic;
use crate::hir::prelude_bundle::{FixedFailure, FixedSchemaPayer, Payer, WorkReceipt};
use crate::test_allocation_observer::measure;
use std::mem::size_of;

fn counter_carriers() -> usize {
    size_of::<FixedSchemaPayer<'_, Refused>>()
        + size_of::<Result<FixedSchemaPayer<'_, Refused>, Refused>>()
        + size_of::<WorkReceipt>()
}

#[test]
fn actual_typed_work_refusal_moves_original_paid_buffers_and_all_six_fields_through_drop() {
    let expected = [Diagnostic::io("SPX-H006", WORK_MESSAGE)];
    let setup = IdentityReservation::setup_bytes().unwrap();
    let required = setup + counter_carriers();
    for make_byte_owner_sticky_after_work in [false, true] {
        let ((work_first, physical), byte_failed, used, byte_first) =
            with_limit_usage_refusal(required, || {
                measure(|| {
                    let mut owner = IdentityReservation::acquire().unwrap();
                    let pointers = owner.pointers();
                    let work_first = {
                        let mut reserve = |n| owner.reserve(n);
                        let mut fixed = FixedSchemaPayer::acquire(&mut reserve).unwrap();
                        let limit = fixed.receipt().limit;
                        let first = match fixed.work(limit.checked_add(1).unwrap()) {
                            Err(FixedFailure::Work(first)) => first,
                            other => panic!("expected distinct fixed work refusal: {other:?}"),
                        };
                        assert_eq!(first.requested, limit + 1);
                        assert_eq!(first.remaining, limit);
                        assert_eq!(fixed.work(0), Err(FixedFailure::Work(first)));
                        assert_eq!(fixed.bytes(0), Err(FixedFailure::Work(first)));
                        assert_eq!(fixed.receipt().first, Some(first));
                        first
                    };
                    if make_byte_owner_sticky_after_work {
                        let byte_error = owner.reserve(1).unwrap_err();
                        assert_eq!(owner.reserve(0), Err(byte_error));
                    }
                    let report = owner.publish_base_prelude_work();
                    assert_eq!(report.as_ptr(), pointers.0);
                    assert_eq!(report[0].message.as_ptr(), pointers.1);
                    assert_eq!(report[0].message_owned_capacity(), 48);
                    assert_eq!(report.as_slice(), expected.as_slice());
                    drop(report);
                    work_first
                })
            });
        assert!(work_first.requested > work_first.remaining);
        assert_eq!(byte_failed, make_byte_owner_sticky_after_work);
        assert_eq!(used, required);
        assert_eq!(
            byte_first,
            make_byte_owner_sticky_after_work.then_some(ReservationRefusal {
                requested: 1,
                remaining: 0,
                floor: 0,
                stage: None,
            })
        );
        // Exactly the original report slot and original 48-byte text exist.
        // No new allocation or realloc occurs during work refusal/publication.
        assert_eq!(physical.allocations, 2);
        assert_eq!(physical.reallocations, 0);
        assert_eq!(physical.allocated, size_of::<Diagnostic>() + 48);
        assert_eq!(physical.allocated, physical.freed);
        assert_eq!(physical.live, 0);
        assert!(!physical.invalid);
    }
}

#[test]
fn actual_counter_setup_byte_refusal_stays_primary_without_work_or_publication() {
    let setup = IdentityReservation::setup_bytes().unwrap();
    let counter = counter_carriers();
    for limit in [0, setup + counter - 1] {
        let (((error, request, remaining), physical), failed, used, first) =
            with_limit_usage_refusal(limit, || {
                measure(|| match IdentityReservation::acquire() {
                    Err(error) => (error, setup, limit),
                    Ok(mut owner) => {
                        let error = {
                            let mut reserve = |n| owner.reserve(n);
                            FixedSchemaPayer::acquire(&mut reserve).err().unwrap()
                        };
                        assert_eq!(owner.reserve(0), Err(error));
                        drop(owner);
                        (error, counter, limit - setup)
                    }
                })
            });
        let expected = ReservationRefusal {
            requested: request,
            remaining,
            floor: 0,
            stage: None,
        };
        assert!(failed);
        assert_eq!(error, Refused::Budget(expected));
        assert_eq!(first, Some(expected));
        assert_eq!(used, if limit == 0 { 0 } else { setup });
        assert_eq!(physical.allocations, if limit == 0 { 0 } else { 2 });
        assert_eq!(physical.reallocations, 0);
        assert_eq!(physical.allocated, physical.freed);
        assert_eq!(physical.live, 0);
        assert!(!physical.invalid);
    }
}

#[test]
fn foreign_then_restored_byte_owner_keeps_exact_first_inline_error_without_requery() {
    let required = IdentityReservation::setup_bytes().unwrap() + counter_carriers();
    let ((work_first, callback_after, observed), failed, used, first) =
        with_limit_usage_refusal(required, || {
            let mut owner = IdentityReservation::acquire().unwrap();
            let (work_first, observed) = {
                let mut reserve = |n| owner.reserve(n);
                let mut fixed = FixedSchemaPayer::acquire(&mut reserve).unwrap();
                let ((first, observed), nested_failed, nested_used, nested_first) =
                    with_limit_usage_refusal(0, || {
                        measure(|| {
                            assert_eq!(fixed.work(1), Err(FixedFailure::Bytes(Refused::Detached)));
                            assert_eq!(fixed.bytes(0), Err(FixedFailure::Bytes(Refused::Detached)));
                            assert_eq!(
                                fixed.work(usize::MAX),
                                Err(FixedFailure::Bytes(Refused::Detached))
                            );
                            assert_eq!(
                                fixed.first_failure(),
                                Some(FixedFailure::Bytes(Refused::Detached))
                            );
                            fixed.first_failure().unwrap()
                        })
                    });
                assert!(!nested_failed);
                assert_eq!(nested_used, 0);
                assert_eq!(nested_first, None);
                // The original byte scope is restored. Callback requery would
                // now say Budget, but this counter preserves the earlier Detached.
                assert_eq!(fixed.work(0), Err(FixedFailure::Bytes(Refused::Detached)));
                assert_eq!(fixed.bytes(1), Err(FixedFailure::Bytes(Refused::Detached)));
                assert_eq!(
                    fixed.first_failure(),
                    Some(FixedFailure::Bytes(Refused::Detached))
                );
                assert_eq!(fixed.receipt().used, 0);
                assert_eq!(fixed.receipt().first, None);
                (first, observed)
            };
            let callback_after = owner.reserve(0).unwrap_err();
            drop(owner);
            (work_first, callback_after, observed)
        });
    let receipt = ReservationRefusal {
        requested: 0,
        remaining: 0,
        floor: 0,
        stage: None,
    };
    assert_eq!(work_first, FixedFailure::Bytes(Refused::Detached));
    assert_eq!(callback_after, Refused::Budget(receipt));
    assert!(failed);
    assert_eq!(used, required);
    assert_eq!(first, Some(receipt));
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}
