use super::*;

#[test]
fn refusal_evidence_keeps_first_sticky_request_and_exact_debits() {
    let (_, overflowed, used, refusal) = with_limit_usage_refusal(10, || {
        assert!(reserve_active_required(3));
        assert!(!reserve_active_required(8));
        assert!(reserve_active_required(2));
        assert!(!reserve_active_required(99));
    });
    assert!(overflowed);
    assert_eq!(used, 5);
    assert_eq!(
        refusal,
        Some(ReservationRefusal {
            requested: 8,
            remaining: 7,
            floor: 0
        })
    );
    let (admitted, overflowed, used, refusal) =
        with_limit_usage_refusal(3, || reserve_active_required(3));
    assert!(admitted);
    assert!(!overflowed);
    assert_eq!(used, 3);
    assert_eq!(refusal, None);
}

#[test]
fn refusal_evidence_distinguishes_optional_floor_and_sticky_required_floor() {
    let (_, overflowed, used, refusal) = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(4));
        assert!(!reserve_active(7));
        assert_eq!(active().unwrap().first_refusal.get(), None);
        assert!(!reserve_active_required(7));
        clear_active_floor();
        assert!(reserve_active_required(10));
    });
    assert!(overflowed);
    assert_eq!(used, 10);
    assert_eq!(
        refusal,
        Some(ReservationRefusal {
            requested: 7,
            remaining: 10,
            floor: 4
        })
    );
}

#[test]
fn refusal_evidence_is_phase_local_and_nested_restore_keeps_parent_identity() {
    let (child, overflowed, used, refusal) = with_limit_usage_refusal(10, || {
        assert!(reserve_active_required(2));
        let child = with_limit_usage_refusal(3, || {
            assert!(reserve_active_required(1));
            assert!(!reserve_active_required(3));
        });
        assert_eq!(active_remaining(), Some(7));
        assert!(!reserve_active_required(8));
        child
    });
    assert!(child.1);
    assert_eq!(child.2, 1);
    assert_eq!(
        child.3,
        Some(ReservationRefusal {
            requested: 3,
            remaining: 2,
            floor: 0
        })
    );
    assert!(overflowed);
    assert_eq!(used, 3);
    assert_eq!(
        refusal,
        Some(ReservationRefusal {
            requested: 8,
            remaining: 7,
            floor: 0
        })
    );
    assert!(active().is_none());
    let (_, overflowed, used, refusal) = with_limit_usage_refusal(0, || {});
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(refusal, None);
}

#[test]
fn refusal_evidence_records_parent_floor_restoration_not_child_evidence() {
    let (child, overflowed, used, refusal) = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(7));
        let child = with_limit_usage_refusal(4, || assert!(reserve_active_required(4)));
        assert_eq!(active_remaining(), Some(6));
        child
    });
    assert!(!child.1);
    assert_eq!(child.2, 4);
    assert_eq!(child.3, None);
    assert!(overflowed);
    assert_eq!(used, 4);
    assert_eq!(
        refusal,
        Some(ReservationRefusal {
            requested: 4,
            remaining: 10,
            floor: 7
        })
    );
}
