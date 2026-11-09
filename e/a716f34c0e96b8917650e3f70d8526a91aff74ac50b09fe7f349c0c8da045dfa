use super::*;
use crate::cleanup_plan::replay::selected_entry::{Acquired, ReportForm};
const FORM: ReportForm = ReportForm::new("SPX-T781", "selected path paid report control");
#[test]
fn live_initialize_paid_session_retains_original_report_source_and_first_receipt() {
    for text in [SCALAR, OWNED_LEXICAL] {
        let mut program = program(text);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "family.bytes")
            .unwrap();
        let block = function
            .cleanup_plan
            .blocks
            .iter_mut()
            .find(|b| {
                b.transitions
                    .iter()
                    .any(|t| matches!(t, CleanupTransition::Initialize { .. }))
            })
            .unwrap();
        let index = block
            .transitions
            .iter()
            .position(|t| matches!(t, CleanupTransition::Initialize { .. }))
            .unwrap();
        block
            .transitions
            .insert(index + 1, block.transitions[index].clone());
        let source = primitive_fixture::Context::new(&program);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "family.bytes")
            .unwrap();
        let old = owner(&source, function, true).unwrap_err();
        if text == SCALAR {
            assert_eq!(
                old,
                replay_error(
                    function,
                    "selected scalar helper body contains non-Copy storage"
                )
            );
            assert_eq!(owner(&source, function, false), Err(old.clone()));
        }
        let (_, observed) = measure(|| {
            let acquired = Acquired::try_acquire(PhysicalReservation::new(LIMIT), FORM)
                .unwrap_or_else(|_| panic!("ample report carrier"));
            let backing = acquired.backing();
            let prepared = acquired
                .prepare(function)
                .unwrap_or_else(|_| panic!("ample preparation"));
            let mut session = prepared
                .into_paid_session(&source, function)
                .unwrap_or_else(|_| panic!("held source constructor"));
            let error = session.validate_paid(function).unwrap_err();
            let Error::Structural(receipt) = &error else {
                panic!("path semantic must remain typed: {error:?}")
            };
            if text == SCALAR {
                assert!(
                    receipt.borrowed_control_receipt().is_none(),
                    "original scalar refusal precedes owned path state"
                );
            } else {
                let (
                    held,
                    structural_control::selected::reason::Reason::Path(Reason::Finite(kind)),
                ) = receipt.borrowed_control_receipt().unwrap()
                else {
                    panic!("nested path reason")
                };
                assert_eq!(
                    *kind,
                    Kind::InitializeTransitionInitializesAnEmptyOrLiveCleanupPlace
                );
                assert_eq!(held.as_str().as_ptr(), function.body.id.as_str().as_ptr());
            }
            // A nested observer is invalid. Read the same active whole-
            // lifetime counters around these two allocation-free operations.
            let later_before = crate::test_allocation_observer::current_metrics();
            let later = session.validate_paid(function);
            let later_observed = crate::test_allocation_observer::current_metrics();
            assert!(matches!(later, Err(Error::Failed)));
            assert_eq!(
                (
                    later_observed.allocations - later_before.allocations,
                    later_observed.reallocations - later_before.reallocations,
                ),
                (0, 0)
            );
            let publish_before = crate::test_allocation_observer::current_metrics();
            let mut refusal = session.into_failed_preparation().refuse_engine(error);
            let publish = crate::test_allocation_observer::current_metrics();
            assert_eq!(
                (
                    publish.allocations - publish_before.allocations,
                    publish.reallocations - publish_before.reallocations,
                ),
                (0, 0)
            );
            let report = refusal.report().unwrap();
            assert_eq!((report.len(), report.capacity()), (1, 1));
            assert_eq!(report.as_ptr(), backing.0);
            assert_eq!(report[0].message.as_ptr(), backing.1);
            assert_eq!(
                (report[0].code, report[0].message.as_ref()),
                (FORM.code(), FORM.message())
            );
            assert!(
                refusal.receipt().is_none(),
                "no owned semantic Diagnostic transport"
            );
            assert!(refusal.structural_receipt().is_some());
            let ledger = refusal.ledger_mut().unwrap();
            let charged = ledger.charged();
            let first = ledger.reserve(0).unwrap_err();
            assert_eq!(ledger.reserve(1).unwrap_err(), first);
            assert_eq!(ledger.charged(), charged);
            // Full detailed legacy observation is separately exercised by owner().
            if text == OWNED_LEXICAL {
                assert!(old
                    .message
                    .contains("initialize transition initializes an empty or live cleanup place"));
            } else {
                // Full six-field equality was checked before this physical
                // observation; do not allocate an expected Diagnostic here.
                assert!(old
                    .message
                    .contains("selected scalar helper body contains non-Copy storage"));
            }
        });
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
