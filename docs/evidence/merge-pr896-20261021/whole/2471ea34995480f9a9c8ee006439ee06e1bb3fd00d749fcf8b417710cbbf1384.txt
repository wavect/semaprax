use super::*;
#[test]
fn ordinary_byte_adapter_preserves_compatibility() {
    let mut funding = SourceFunding::ordinary();
    assert_eq!(funding.reserve(usize::MAX), Ok(()));
    assert_eq!(funding.specialized_presentation(), Ok(()));
}

#[test]
fn real_byte_owner_keeps_first_refusal_even_under_another_budget() {
    let bytes = IdentityReservation::setup_bytes().unwrap() + SourceFunding::setup_bytes();
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let mut funding = SourceFunding::funded(&mut owner).unwrap();
        let first = funding.specialized_presentation().unwrap_err();
        assert!(matches!(first, SourceStop::Reservation(Refused::Budget(_))));
        let (second, foreign_refused, used) =
            crate::bounded_output::with_limit_usage(usize::MAX, || funding.reserve(0));
        assert_eq!(second, Err(first));
        assert!(!foreign_refused);
        assert_eq!(used, 0);
        assert_eq!(funding.reserve(usize::MAX), Err(first));
    });
    assert!(refused);
}

#[test]
fn sealed_abort_handoff_allocates_nothing_after_the_first_refusal() {
    let prefix = vec![
        Diagnostic::error("SPX-T900", "prefix", crate::ast::Span::default())
            .at_path("prefix.spx")
            .with_help("preserve help"),
    ];
    let expected = prefix.clone();
    let bytes = IdentityReservation::setup_bytes().unwrap() + SourceFunding::setup_bytes();
    let (abort, refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let mut funding = SourceFunding::funded(&mut owner).unwrap();
        let stop = funding.specialized_presentation().unwrap_err();
        let (abort, allocations) =
            crate::test_allocation_observer::measure(|| funding.abort(prefix, stop));
        assert_eq!(allocations.allocated, 0);
        assert_eq!(allocations.freed, 0);
        assert!(!allocations.invalid);
        abort
    });
    assert!(refused);
    assert_eq!(abort.prefix(), expected);
    assert_eq!(
        std::mem::size_of::<SourceAbort>(),
        std::mem::size_of::<Vec<Diagnostic>>()
    );
}

#[test]
fn storage_frontier_entry_is_prepaid_at_exact_and_one_short_without_drop_debt() {
    let identity = IdentityReservation::setup_bytes().unwrap();
    let exact = identity + SourceFunding::setup_bytes();
    let old_entry = exact - SourceFunding::STORAGE_DECLARATION_RECEIVING_BYTES;
    assert!(old_entry < exact - 1);
    for (limit, admitted) in [(old_entry, false), (exact - 1, false), (exact, true)] {
        let ((success, refused, used), physical) = crate::test_allocation_observer::measure(|| {
            crate::bounded_output::with_limit_usage(limit, || {
                let mut owner = IdentityReservation::acquire().unwrap();
                let success = match SourceFunding::funded(&mut owner) {
                    Ok(mut funding) => {
                        // Entry paid every named scan/finish slot before this
                        // later operation can observe the exhausted account.
                        let first = funding.reserve(1).unwrap_err();
                        assert_eq!(funding.reserve(0), Err(first));
                        true
                    }
                    Err(Refused::Budget(_)) => false,
                    Err(other) => panic!("unexpected source entry refusal: {other:?}"),
                };
                success
            })
        });
        assert_eq!(success, admitted);
        assert!(refused);
        assert!(used <= limit);
        if admitted {
            assert_eq!(used, exact);
        }
        assert!(!physical.invalid);
        assert_eq!(physical.live, 0);
        assert_eq!(physical.allocated, physical.freed);
    }
}
