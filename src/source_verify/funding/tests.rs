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
