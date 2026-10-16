use super::*;
use crate::bounded_output::cfg_reservation::{CfgReservation, Refused};
use crate::test_allocation_observer::measure;
use std::mem::{align_of, size_of};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn canonical_stage_preserves_actual_budget_receipt_guard_and_paid_carrier_layouts() {
    // Frozen pre-change field types, independent of the current declarations.
    // The old and new stage enums both fit the same one-byte/niche layout.
    #[derive(Clone, Copy)]
    enum BeforeStage {
        ResolvedCore,
        CompactValidation,
        UncachedSyntheticPeak,
        AuthoredModuleValidation,
        FrontendLookup,
        SelectiveResolution,
        FrontendRetention,
        ImportedVecInstances,
        CorePrebound,
        ExpectedEdges,
        ResolvedCallEdges,
        StubSignatures,
        DeclarationFacts,
        RetainedFacts,
        CoreDirectories,
        EdgeProof,
        SyntheticModule,
        CachedValidation,
        FreshResolution,
        HirLowering,
        HirLoanPlans,
        HirInstanceLoanPlans,
        HirCleanupInventories,
        HirInstanceCleanupInventories,
        HirCleanupPlans,
        HirYieldProfiles,
        HirValidation,
        RetainedModule,
        LoanIdentityInventory,
        RetainedLoanScratch,
        RetainedLoanSidecars,
    }
    let _all_old_stages = [
        BeforeStage::ResolvedCore,
        BeforeStage::CompactValidation,
        BeforeStage::UncachedSyntheticPeak,
        BeforeStage::AuthoredModuleValidation,
        BeforeStage::FrontendLookup,
        BeforeStage::SelectiveResolution,
        BeforeStage::FrontendRetention,
        BeforeStage::ImportedVecInstances,
        BeforeStage::CorePrebound,
        BeforeStage::ExpectedEdges,
        BeforeStage::ResolvedCallEdges,
        BeforeStage::StubSignatures,
        BeforeStage::DeclarationFacts,
        BeforeStage::RetainedFacts,
        BeforeStage::CoreDirectories,
        BeforeStage::EdgeProof,
        BeforeStage::SyntheticModule,
        BeforeStage::CachedValidation,
        BeforeStage::FreshResolution,
        BeforeStage::HirLowering,
        BeforeStage::HirLoanPlans,
        BeforeStage::HirInstanceLoanPlans,
        BeforeStage::HirCleanupInventories,
        BeforeStage::HirInstanceCleanupInventories,
        BeforeStage::HirCleanupPlans,
        BeforeStage::HirYieldProfiles,
        BeforeStage::HirValidation,
        BeforeStage::RetainedModule,
        BeforeStage::LoanIdentityInventory,
        BeforeStage::RetainedLoanScratch,
        BeforeStage::RetainedLoanSidecars,
    ];
    struct BeforeBudget {
        _initial: usize,
        _remaining: Cell<usize>,
        _floor: Cell<usize>,
        _overflowed: Cell<bool>,
        _first_refusal: Cell<Option<BeforeReceipt>>,
        _stage: Cell<Option<BeforeStage>>,
    }
    #[derive(Clone, Copy)]
    struct BeforeReceipt {
        _requested: usize,
        _remaining: usize,
        _floor: usize,
        _stage: Option<BeforeStage>,
    }
    struct BeforeGuard {
        _held: Option<(Rc<BeforeBudget>, Option<BeforeStage>)>,
    }
    #[derive(Clone, Copy)]
    enum BeforeRefused {
        NoOwner,
        Budget { _receipt: BeforeReceipt },
        Allocation,
        Capacity,
        Detached,
        Arithmetic,
    }
    let _all_old_reasons = [
        BeforeRefused::NoOwner,
        BeforeRefused::Budget {
            _receipt: BeforeReceipt {
                _requested: 0,
                _remaining: 0,
                _floor: 0,
                _stage: None,
            },
        },
        BeforeRefused::Allocation,
        BeforeRefused::Capacity,
        BeforeRefused::Detached,
        BeforeRefused::Arithmetic,
    ];
    struct BeforePaidCarrier {
        _budget: Rc<BeforeBudget>,
        _text: String,
        _first: Cell<Option<BeforeRefused>>,
    }
    assert_eq!(size_of::<ReservationStage>(), 1);
    assert_eq!(size_of::<Option<ReservationStage>>(), 1);
    macro_rules! same_layout {
        ($actual:ty, $before:ty) => {
            assert_eq!(size_of::<$actual>(), size_of::<$before>());
            assert_eq!(align_of::<$actual>(), align_of::<$before>());
        };
    }
    same_layout!(ReservationStage, BeforeStage);
    same_layout!(Option<ReservationStage>, Option<BeforeStage>);
    same_layout!(Refused, BeforeRefused);
    same_layout!(Option<Refused>, Option<BeforeRefused>);
    same_layout!(Budget, BeforeBudget);
    same_layout!(ReservationRefusal, BeforeReceipt);
    same_layout!(ReservationStageGuard, BeforeGuard);
    same_layout!(CfgReservation, BeforePaidCarrier);
}

#[test]
fn canonical_stage_entry_exit_has_no_allocation_or_debit_and_keeps_outer_identity() {
    let (((), metrics), overflow, used, first) = with_limit_usage_refusal(3, || {
        measure(|| {
            let budget = active().unwrap();
            let count = Rc::strong_count(&budget);
            let _outer = reservation_stage_guard(ReservationStage::FreshResolution);
            {
                let _inner = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
                assert!(Rc::ptr_eq(&active().unwrap(), &budget));
                assert_eq!(Rc::strong_count(&budget), count + 2);
                assert_eq!(
                    budget.stage.get(),
                    Some(ReservationStage::CanonicalLoanStorage)
                );
                assert!(reserve_active_required(3));
            }
            assert_eq!(Rc::strong_count(&budget), count + 1);
            assert_eq!(budget.stage.get(), Some(ReservationStage::FreshResolution));
        })
    });
    assert_eq!((overflow, used, first), (false, 3, None));
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 0);
    assert_eq!(metrics.reallocations, 0);
    assert_eq!(
        (
            metrics.allocated,
            metrics.freed,
            metrics.highwater,
            metrics.live
        ),
        (0, 0, 0, 0)
    );
    let (_, metrics) = measure(|| {
        drop(reservation_stage_guard(
            ReservationStage::CanonicalLoanStorage,
        ))
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 0);
    assert!(active().is_none());
}

#[test]
fn canonical_stage_nested_foreign_detached_unwind_floor_and_sticky_receipts_stay_local() {
    let (escaped, overflow, used, first) = with_limit_usage_refusal(5, || {
        let _outer = reservation_stage_guard(ReservationStage::FreshResolution);
        let parent = active().unwrap();
        let escaped = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
        let child = with_limit_usage_refusal(2, || {
            assert_eq!(active().unwrap().stage.get(), None);
            let _child = reservation_stage_guard(ReservationStage::HirValidation);
            assert!(reserve_active_required(1));
            // Parent guard belongs to parent even while the child is active.
            drop(escaped);
            assert_eq!(
                active().unwrap().stage.get(),
                Some(ReservationStage::HirValidation)
            );
            assert_eq!(parent.stage.get(), Some(ReservationStage::FreshResolution));
        });
        assert_eq!((child.1, child.2, child.3), (false, 1, None));
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _stage = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
            panic!("stage restoration witness");
        }));
        assert!(caught.is_err());
        assert_eq!(parent.stage.get(), Some(ReservationStage::FreshResolution));
        parent.floor.set(2);
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _stage = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
            assert!(!reserve_active_required(3));
            panic!("first sticky stage survives unwind");
        }));
        assert!(caught.is_err());
        assert!(!reserve_active_required(0));
        assert!(!reserve_active_required(1));
        reservation_stage_guard(ReservationStage::CanonicalLoanStorage)
    });
    assert!(overflow);
    assert_eq!(used, 1);
    assert_eq!(
        first,
        Some(ReservationRefusal {
            requested: 3,
            remaining: 4,
            floor: 2,
            stage: Some(ReservationStage::CanonicalLoanStorage),
        })
    );
    // Dropping a detached guard while another budget is current restores only
    // its original owner, never the foreign active budget or its first receipt.
    let (_, overflow, used, first) = with_limit_usage_refusal(1, || {
        let _foreign = reservation_stage_guard(ReservationStage::HirValidation);
        drop(escaped);
        assert_eq!(
            active().unwrap().stage.get(),
            Some(ReservationStage::HirValidation)
        );
        assert!(!reserve_active_required(2));
    });
    assert!(overflow);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().stage, Some(ReservationStage::HirValidation));
    assert!(active().is_none());
}
