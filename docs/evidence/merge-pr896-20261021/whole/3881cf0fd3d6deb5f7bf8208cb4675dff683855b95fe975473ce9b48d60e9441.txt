//! Caller-owned paid report carrier on the selected ledger; no admission.
use super::*;
use crate::cleanup_plan::replay::physical_storage::PhysicalRefusal;
use crate::test_allocation_observer::measure;

const FORM: ReportForm = ReportForm::new("SPX-T781", "paid selected replay report owning control");

fn carrier(form: ReportForm) -> usize {
    // Independent literal carrier request: one Diagnostic slot plus text.
    std::mem::size_of::<Diagnostic>() + form.message().len()
}
fn setup(function: &ResolvedFunction) -> [usize; 3] {
    [
        std::mem::size_of::<PhysicalReservation>(),
        514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>(),
        function.id.as_str().len() + 512,
    ]
}
fn first_receipt(refusal: &mut PaidRefusal) -> PhysicalRefusal {
    let ledger = refusal.ledger_mut().expect("retained ledger");
    let first = ledger.reserve(0).unwrap_err();
    assert_eq!(ledger.reserve(1).unwrap_err(), first);
    assert_eq!(ledger.reserve(0).unwrap_err(), first);
    first
}
fn assert_paid(refusal: &PaidRefusal, backing: (*const Diagnostic, *const u8)) {
    assert!(refusal.is_paid());
    let report = refusal.report().expect("paid report");
    assert_eq!((report.len(), report.capacity()), (1, 1));
    assert_eq!(report.as_ptr(), backing.0, "original Diagnostic slot");
    let first = &report[0];
    assert_eq!(first.code, FORM.code());
    assert_eq!(first.message, FORM.message());
    assert_eq!(first.message_owned_capacity(), FORM.message().len());
    assert_eq!(first.message.as_ptr(), backing.1, "original prepaid text");
    assert!(first.path.is_none() && first.span.is_none() && first.help.is_none());
}
fn acquired(limit: usize) -> Acquired {
    match Acquired::try_acquire(PhysicalReservation::new(limit), FORM) {
        Ok(acquired) => acquired,
        Err(_) => panic!("carrier is acquired before preparation"),
    }
}
fn prepared(function: &ResolvedFunction) -> Preparation {
    match acquired(1 << 26).prepare(function) {
        Ok(prepared) => prepared,
        Err(_) => panic!("ample setup"),
    }
}
/// The selected budget's prepaid refusal text: its pointer and capacity.
fn prepaid_text(preparation: &Preparation) -> (*const u8, usize) {
    let text = preparation.budget.refusal_text.as_ref().expect("unspent");
    (text.as_ptr(), text.capacity())
}
fn tiny() -> ResolvedProgram {
    let source = crate::check(
        "module paid.index; @id(\"a\") fn a()->i64 {1} @id(\"b\") fn main()->i64 {2}",
        "paid.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}

#[test]
fn paid_report_zero_and_one_short_return_the_exact_rejected_ledger_without_allocating() {
    let slot = std::mem::size_of::<Diagnostic>();
    assert_eq!(FORM.paid_bytes(), carrier(FORM));
    for (limit, charged, reason) in [
        (0, 0, physical_budget::SetupReason::ReportCarrier),
        (slot - 1, 0, physical_budget::SetupReason::ReportCarrier),
        (
            carrier(FORM) - 1,
            slot,
            physical_budget::SetupReason::ReportText,
        ),
    ] {
        let (mut refusal, allocations) =
            measure(
                || match Acquired::try_acquire(PhysicalReservation::new(limit), FORM) {
                    Ok(_) => panic!("one-short carrier cannot be acquired"),
                    Err(refusal) => refusal,
                },
            );
        assert!(!allocations.invalid);
        assert_eq!(
            (
                allocations.allocations,
                allocations.allocated,
                allocations.live
            ),
            (0, 0, 0),
            "refused carrier allocates no Diagnostic, Vec or text"
        );
        assert!(!refusal.is_paid());
        assert!(refusal.report().is_none() && refusal.refused().is_none());
        assert_eq!(refusal.setup_reason(), Some(reason));
        assert_eq!(first_receipt(&mut refusal), PhysicalRefusal::LocalLimit);
        assert_eq!(refusal.ledger_mut().unwrap().charged(), charged);
        // The typed unpaid boundary is reached without any allocation.
        let (unpaid, typed) = measure(|| refusal.into_published());
        assert_eq!(typed.allocations, 0, "no diagnostic is rebuilt here");
        let unpaid = unpaid.expect_err("an unpaid carrier never claims funding");
        assert_eq!(unpaid.reason(), UnpaidReason::Carrier(reason));
        // Only the named compatibility boundary allocates its unfunded literal.
        let (legacy, allocations) = measure(|| unpaid.into_unfunded());
        assert_eq!(
            (legacy[0].code, legacy[0].message.as_ref()),
            (FORM.code(), FORM.message())
        );
        assert_eq!(allocations.allocations, 2);
    }
}

#[test]
fn paid_report_setup_refusal_publishes_original_backing_and_keeps_setup_ledger() {
    let program = program();
    let function = members(&program)[0];
    let [reservation, cursor, text] = setup(function);
    let paid = carrier(FORM);
    for (limit, charged, reason) in [
        (paid, paid, physical_budget::SetupReason::Reservation),
        (
            paid + reservation + cursor - 1,
            paid + reservation,
            physical_budget::SetupReason::CloneCursor,
        ),
        (
            paid + reservation + cursor + text - 1,
            paid + reservation + cursor,
            physical_budget::SetupReason::TextReservation,
        ),
    ] {
        let (_, allocations) = measure(|| {
            let acquired = acquired(limit);
            assert_eq!(acquired.charged(), paid);
            let backing = acquired.backing();
            let mut refusal = match acquired.prepare(function) {
                Ok(_) => panic!("one-short setup cannot prepare"),
                Err(refusal) => refusal,
            };
            assert_paid(&refusal, backing);
            assert_eq!(refusal.setup_reason(), Some(reason));
            let refused = refusal.refused().unwrap();
            assert_eq!(
                (refused.cause(), refused.detail()),
                (Cause::Setup, reason.message())
            );
            assert_eq!(first_receipt(&mut refusal), PhysicalRefusal::LocalLimit);
            assert_eq!(refusal.ledger_mut().unwrap().charged(), charged);
            assert!(refusal.into_preparation().is_none());
        });
        assert!(!allocations.invalid);
        assert_eq!(allocations.allocations, 2, "carrier slot and text only");
        assert_eq!(allocations.reallocations, 0);
        assert_eq!(allocations.allocated, paid);
        assert_eq!((allocations.live, allocations.freed), (0, paid));
    }
}

#[test]
fn paid_report_constructor_refusal_keeps_the_exact_ledger_and_first_receipt() {
    let program = program();
    let function = members(&program)[0];
    let acquired = acquired(1 << 26);
    let backing = acquired.backing();
    let mut preparation = match acquired.prepare(function) {
        Ok(prepared) => prepared,
        Err(_) => panic!("ample setup"),
    };
    let ledger = preparation.budget.physical.as_deref().unwrap() as *const PhysicalReservation;
    let charged = preparation.budget.physical.as_deref().unwrap().charged();
    let own = Refused::constructor("owning constructor refusal control");
    let (error, allocations) = measure(|| {
        preparation
            .construct(function, |physical, _| -> Result<(), Refused> {
                assert_eq!(physical.reserve(usize::MAX), Err(PhysicalRefusal::Overflow));
                Err(own)
            })
            .unwrap_err()
    });
    assert_eq!(allocations.allocations, 0, "a constructor refusal is Copy");
    assert_eq!(error, own);
    assert!(preparation.receipt().is_none(), "no Diagnostic was made");
    let mut refusal = preparation.refuse(error);
    assert_paid(&refusal, backing);
    assert_eq!(refusal.setup_reason(), None);
    assert_eq!(refusal.refused(), Some(own));
    let retained = refusal.ledger_mut().unwrap();
    assert!(std::ptr::eq(&*retained, ledger), "same ledger");
    assert_eq!(retained.charged(), charged);
    // reject() on transfer cannot replace the constructor's first receipt.
    assert_eq!(first_receipt(&mut refusal), PhysicalRefusal::Overflow);
    // A paid report carries no admission: the owner stays failed.
    let mut preparation = refusal.into_preparation().unwrap();
    assert!(preparation.failed);
    assert!(preparation.probe_zero(function).is_err());
    assert_eq!(
        preparation.try_charge(function, 0, "restore"),
        Err(Refused::FAILED)
    );
    assert!(preparation.charge(function, 0, "restore").is_err());
    let context = primitive_fixture::Context::new(&program);
    assert!(Session::try_from_preparation(&context, function, preparation).is_err());
}

#[test]
fn paid_report_budget_refusal_moves_the_prepaid_text_once_without_allocating() {
    let program = program();
    let function = members(&program)[0];
    let mut preparation = prepared(function);
    let (text, capacity) = prepaid_text(&preparation);
    assert_eq!(capacity, function.id.as_str().len() + 512);
    preparation.budget.remaining = 3;
    let (refused, allocations) =
        measure(|| preparation.try_charge(function, 4, "owning exhaustion"));
    assert_eq!(
        (allocations.allocations, allocations.reallocations),
        (0, 0),
        "the first refusal moves the prepaid String, it formats nothing new"
    );
    let refused = refused.unwrap_err();
    assert_eq!(
        (refused.cause(), refused.detail()),
        (Cause::Budget, "owning exhaustion")
    );
    let receipt = preparation.receipt().expect("first paid receipt");
    assert_eq!(receipt.code, "SPX-H006");
    assert_eq!(
        (receipt.message.as_ptr(), receipt.message_owned_capacity()),
        (text, capacity)
    );
    assert!(receipt.message.starts_with("cleanup plan for function `"));
    // Later refusals are inline and leave the first receipt in place.
    let (later, allocations) = measure(|| {
        (
            preparation.try_charge(function, 0, "later"),
            preparation.try_reserve_array::<u8>(function, 1, "later"),
        )
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(later, (Err(Refused::FAILED), Err(Refused::FAILED)));
    let mut refusal = preparation.refuse(refused);
    assert_eq!(refusal.refused(), Some(refused));
    let receipt = refusal.receipt().expect("retained with the failed owner");
    assert_eq!(receipt.message.as_ptr(), text, "same prepaid text pointer");
    assert_eq!(first_receipt(&mut refusal), PhysicalRefusal::LocalLimit);
}

#[test]
fn paid_report_constructor_exhaustion_spends_the_prepaid_receipt_and_fails_closed() {
    let program = program();
    let function = members(&program)[0];
    let mut preparation = prepared(function);
    let (text, _) = prepaid_text(&preparation);
    preparation.budget.remaining = 2;
    let (refused, allocations) = measure(|| {
        preparation.construct(function, |_, debit| {
            debit(2)?;
            // A constructor that ignores its refused debit still fails closed.
            let _ = debit(1);
            Ok(())
        })
    });
    assert_eq!(allocations.allocations, 0, "no constructor Diagnostic");
    let refused = refused.unwrap_err();
    assert_eq!(refused.cause(), Cause::Budget);
    assert_eq!(preparation.budget.remaining, 0, "sticky, no refund");
    let receipt = preparation.receipt().expect("prepaid budget receipt");
    assert_eq!(receipt.message.as_ptr(), text);
    let refusal = preparation.refuse(refused);
    assert!(refusal.is_paid());
    assert_eq!(refusal.refused(), Some(refused));
}

#[test]
fn paid_report_session_refusal_transfers_carrier_and_retains_first_receipt_through_drop() {
    let mut program = tiny();
    program.functions[1].id = program.functions[0].id.clone();
    let context = primitive_fixture::Context::new(&program);
    let anchor = &program.functions[0];
    let (_, allocations) = measure(|| {
        let acquired = acquired(1 << 26);
        let backing = acquired.backing();
        let preparation = match acquired.prepare(anchor) {
            Ok(prepared) => prepared,
            Err(_) => panic!("ample setup"),
        };
        let ledger = preparation.budget.physical.as_deref().unwrap() as *const PhysicalReservation;
        let (preparation, first) =
            match Session::try_from_preparation(&context, anchor, preparation) {
                Ok(_) => panic!("duplicate identities must refuse"),
                Err(refused) => refused,
            };
        assert_eq!(first.code, "SPX-H006");
        let first_text = (first.message.as_ptr(), first.message_owned_capacity());
        let charged = preparation.budget.physical.as_deref().unwrap().charged();
        // Observation cannot nest: the pointer identity below proves a move.
        let mut refusal = preparation.refuse_session(first);
        assert_paid(&refusal, backing);
        assert_eq!(refusal.refused().unwrap().cause(), Cause::Session);
        // The public form stays the carrier's literal; the Session's own
        // Diagnostic is the retained first receipt, not the report text.
        let receipt = refusal.receipt().expect("retained Session receipt");
        assert_eq!(
            (receipt.message.as_ptr(), receipt.message_owned_capacity()),
            first_text,
            "the Session Diagnostic moved; nothing was cloned or formatted"
        );
        let retained = refusal.ledger_mut().unwrap();
        assert!(std::ptr::eq(&*retained, ledger), "same ledger");
        assert_eq!(retained.charged(), charged);
        assert_eq!(first_receipt(&mut refusal), PhysicalRefusal::LocalLimit);
        let preparation = refusal.into_preparation().unwrap();
        assert!(Session::try_from_preparation(&context, anchor, preparation).is_err());
    });
    assert!(!allocations.invalid);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(
        allocations.live, 0,
        "carrier, scratch and ledger owner dropped"
    );
    assert_eq!(allocations.allocated, allocations.freed);
}

#[test]
fn paid_finish_returns_the_same_ledger_and_a_failed_finish_allocates_nothing() {
    let program = program();
    let context = primitive_fixture::Context::new(&program);
    let [first, second] = members(&program);
    let preparation = prepared(first);
    let mut session = match Session::try_from_preparation(&context, first, preparation) {
        Ok(session) => session,
        Err(_) => panic!("ample session"),
    };
    session.validate(first).unwrap();
    session.validate(second).unwrap();
    let ledger = session.budget.physical.as_deref().unwrap() as *const PhysicalReservation;
    let charged = session.budget.physical.as_deref().unwrap().charged();
    // The carrier's own debit stays charged: success refunds nothing.
    assert!(charged >= carrier(FORM));
    let (finished, allocations) = measure(|| session.finish_paid());
    assert_eq!(allocations.allocations, 0);
    let ledger_box = match finished {
        Ok(ledger_box) => ledger_box,
        Err(_) => panic!("validated session finishes"),
    };
    assert!(std::ptr::eq(&*ledger_box, ledger), "same boxed ledger");
    assert_eq!(ledger_box.charged(), charged, "no refund or reset");
    drop(ledger_box);

    // A failed Session never reaches the legacy `finish` Diagnostic.
    let preparation = prepared(first);
    let mut session = match Session::try_from_preparation(&context, first, preparation) {
        Ok(session) => session,
        Err(_) => panic!("ample session"),
    };
    session.failed = true;
    let (refusal, allocations) = measure(|| session.finish_paid());
    assert_eq!(allocations.allocations, 0, "no replay_error is formatted");
    let Err(refusal) = refusal else {
        panic!("a failed Session cannot finish")
    };
    assert!(refusal.is_paid());
    assert_eq!(refusal.refused(), Some(Refused::FAILED));
}

#[test]
fn paid_report_remap_publishes_only_its_prepaid_form() {
    let other = ReportForm::new("SPX-T782", "a different outward form");
    let first_acquired = acquired(1 << 20);
    let backing = first_acquired.backing();
    let mut refusal = first_acquired.refuse_unprepared();
    assert_eq!(first_receipt(&mut refusal), PhysicalRefusal::LocalLimit);
    let (published, allocations) = measure(|| refusal.into_report(FORM));
    assert_eq!(allocations.allocations, 0, "paid form moves out unchanged");
    let published = published.unwrap();
    assert_eq!(published.as_ptr(), backing.0);
    let acquired = acquired(1 << 20);
    let (mismatched, allocations) = measure(|| acquired.refuse_unprepared().into_report(other));
    assert_eq!(
        allocations.allocations, 0,
        "fails closed without rebuilding"
    );
    let unpaid = mismatched.expect_err("a carrier for another form is never relabelled");
    assert_eq!(unpaid.reason(), UnpaidReason::FormMismatch);
    let legacy = unpaid.into_unfunded();
    assert_eq!(
        (legacy[0].code, legacy[0].message.as_ref()),
        (other.code(), other.message()),
        "only the named unfunded boundary allocates the requested literal"
    );
}

#[test]
fn paid_stage_moves_keep_each_report_with_its_original_ledger_without_pairing_arguments() {
    let program = program();
    let context = primitive_fixture::Context::new(&program);
    let first = members(&program)[0];
    let other = ReportForm::new("SPX-T782", "a second independently paid report");
    let ((), allocations) = measure(|| {
        let acquired_a = acquired(1 << 26);
        let acquired_b = match Acquired::try_acquire(PhysicalReservation::new(1 << 26), other) {
            Ok(owner) => owner,
            Err(_) => panic!("ample second carrier"),
        };
        let backing_a = acquired_a.backing();
        let backing_b = acquired_b.backing();
        let mut preparation_a = acquired_a
            .prepare(first)
            .unwrap_or_else(|_| panic!("first setup"));
        let mut preparation_b = acquired_b
            .prepare(first)
            .unwrap_or_else(|_| panic!("second setup"));
        let ledger_a =
            preparation_a.budget.physical.as_deref().unwrap() as *const PhysicalReservation;
        let ledger_b =
            preparation_b.budget.physical.as_deref().unwrap() as *const PhysicalReservation;
        assert_ne!(ledger_a, ledger_b);
        // Moving/swapping a whole stage also moves its report. No API accepts
        // a report, physical ledger or preparation from the other invocation.
        std::mem::swap(&mut preparation_a, &mut preparation_b);
        assert_eq!(preparation_a.report_backing(), backing_b);
        assert_eq!(preparation_b.report_backing(), backing_a);
        let mut session_b = preparation_a
            .into_paid_session(&context, first)
            .unwrap_or_else(|_| panic!("second session"));
        let mut session_a = preparation_b
            .into_paid_session(&context, first)
            .unwrap_or_else(|_| panic!("first session"));
        session_a.validate(first).unwrap();
        session_b.validate(first).unwrap();
        session_a.failed = true;
        session_b.failed = true;
        let mut refused_a = session_a.finish_paid().err().expect("first paid refusal");
        let mut refused_b = session_b.finish_paid().err().expect("second paid refusal");
        assert_paid(&refused_a, backing_a);
        let report_b = refused_b.report().unwrap();
        assert_eq!(report_b.as_ptr(), backing_b.0);
        assert_eq!(report_b[0].message.as_ptr(), backing_b.1);
        assert_eq!(
            (report_b[0].code, report_b[0].message.as_ref()),
            (other.code(), other.message())
        );
        assert!(std::ptr::eq(&*refused_a.ledger_mut().unwrap(), ledger_a));
        assert!(std::ptr::eq(&*refused_b.ledger_mut().unwrap(), ledger_b));
        assert_eq!(first_receipt(&mut refused_a), PhysicalRefusal::LocalLimit);
        assert_eq!(first_receipt(&mut refused_b), PhysicalRefusal::LocalLimit);
        drop((refused_a, refused_b));
    });
    assert!(!allocations.invalid);
    assert_eq!(allocations.live, 0);
    assert_eq!(allocations.allocated, allocations.freed);
}

#[path = "paid/selected_family.rs"]
mod selected_family;

#[path = "paid/selected_control.rs"]
mod selected_control;
