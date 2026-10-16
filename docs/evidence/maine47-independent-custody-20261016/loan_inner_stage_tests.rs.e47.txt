//! Bounded diagnostic coordinates only; no new account, text, or trace.
use super::*;
use crate::bounded_output::cfg_reservation::{CfgReservation, Refused};
use crate::test_allocation_observer::measure;
use std::mem::{align_of, size_of};
const INNER: [ReservationStage; 5] = [
    ReservationStage::LoanCandidateInventory,
    ReservationStage::LoanCfgDiscovery,
    ReservationStage::LoanCfgPoints,
    ReservationStage::LoanCfgEdges,
    ReservationStage::LoanCanonicalAppend,
];
#[test]
fn loan_inner_stages_keep_complete_old_enum_budget_receipt_guard_and_carrier_layouts() {
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
        CanonicalLoanStorage,

        RetainedModule,
        LoanIdentityInventory,
        RetainedLoanScratch,
        RetainedLoanSidecars,
    }
    let _old_variants = [
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
        BeforeStage::CanonicalLoanStorage,
        BeforeStage::RetainedModule,
        BeforeStage::LoanIdentityInventory,
        BeforeStage::RetainedLoanScratch,
        BeforeStage::RetainedLoanSidecars,
    ];
    #[derive(Clone, Copy)]
    struct BeforeReceipt {
        _requested: usize,
        _remaining: usize,
        _floor: usize,
        _stage: Option<BeforeStage>,
    }
    #[derive(Clone, Copy)]
    enum BeforeRefused {
        NoOwner,
        Budget(BeforeReceipt),
        Allocation,
        Capacity,
        Detached,
        Arithmetic,
    }
    let _old_refusals = [
        BeforeRefused::NoOwner,
        BeforeRefused::Allocation,
        BeforeRefused::Capacity,
        BeforeRefused::Detached,
        BeforeRefused::Arithmetic,
    ];
    let receipt = BeforeReceipt {
        _requested: 0,
        _remaining: 0,
        _floor: 0,
        _stage: None,
    };
    let BeforeRefused::Budget(read_receipt) = BeforeRefused::Budget(receipt) else {
        unreachable!()
    };
    assert_eq!(read_receipt._requested, 0);
    struct BeforeBudget {
        _initial: usize,
        _remaining: Cell<usize>,
        _floor: Cell<usize>,
        _overflowed: Cell<bool>,
        _first_refusal: Cell<Option<BeforeReceipt>>,
        _stage: Cell<Option<BeforeStage>>,
    }
    struct BeforeGuard {
        _held: Option<(Rc<BeforeBudget>, Option<BeforeStage>)>,
    }
    struct BeforePaid {
        _budget: Rc<BeforeBudget>,
        _text: String,
        _first: Cell<Option<BeforeRefused>>,
    }
    macro_rules! same {
        ($a:ty,$b:ty) => {
            assert_eq!(size_of::<$a>(), size_of::<$b>());
            assert_eq!(align_of::<$a>(), align_of::<$b>());
        };
    }
    same!(ReservationStage, BeforeStage);
    same!(Option<ReservationStage>, Option<BeforeStage>);
    same!(ReservationRefusal, BeforeReceipt);
    // The independent call-site diagnostic added one pointer after this
    // stage's original control. Retain the entire old budget comparison.
    #[allow(
        dead_code,
        reason = "independent full old budget plus the added call-site pointer"
    )]
    struct BudgetWithSite {
        before: BeforeBudget,
        site: Cell<Option<&'static std::panic::Location<'static>>>,
    }
    same!(super::canonical_stage_tests::HistoricalBudget, BeforeBudget);
    same!(Budget, BudgetWithSite);
    same!(ReservationStageGuard, BeforeGuard);
    same!(Refused, BeforeRefused);
    same!(Option<Refused>, Option<BeforeRefused>);
    same!(CfgReservation, BeforePaid);
}
#[test]
fn loan_inner_stage_scopes_have_zero_allocation_debit_and_preserve_outer_identity() {
    let (((), metrics), overflow, used, first) = with_limit_usage_refusal(3, || {
        measure(|| {
            let owner = active().unwrap();
            let _outer = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
            for stage in INNER {
                let guard = reservation_stage_guard(stage);
                assert!(Rc::ptr_eq(&owner, &active().unwrap()));
                assert_eq!(owner.stage.get(), Some(stage));
                drop(guard);
                assert_eq!(
                    owner.stage.get(),
                    Some(ReservationStage::CanonicalLoanStorage)
                );
            }
        })
    });
    assert!(!overflow);
    assert_eq!(used, 0);
    assert_eq!(first, None);
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, 0);
    assert_eq!(metrics.live, 0);
}
#[test]
fn loan_inner_first_refusal_stays_exact_across_later_scopes_nested_owner_and_unwind() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    for stage in INNER {
        stage_debits::take_completed();
        let (_, overflow, used, first) = with_limit_usage_refusal(3, || {
            let _trace = stage_debits::start(0, 0, 6);
            let _operation =
                stage_debits::operation_guard(stage_debits::Operation::CanonicalLoanStorage);
            let owner = active().unwrap();
            let _outer = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
            let escaped = reservation_stage_guard(stage);
            let (_, child_overflow, child_used, child_first) = with_limit_usage_refusal(1, || {
                assert_eq!(active().unwrap().stage.get(), None);
                let _child = reservation_stage_guard(ReservationStage::HirValidation);
                assert!(!reserve_active_required(2));
            });
            assert!(child_overflow);
            assert_eq!(child_used, 0);
            assert_eq!(
                child_first.unwrap().stage,
                Some(ReservationStage::HirValidation)
            );
            assert_eq!(owner.stage.get(), Some(stage));
            assert!(reserve_active_required(3));
            assert!(!reserve_active_required(1));
            let selected = owner.first_refusal.get().unwrap();
            assert_eq!(selected.stage, Some(stage));
            drop(escaped);
            assert_eq!(
                owner.stage.get(),
                Some(ReservationStage::CanonicalLoanStorage)
            );
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                let _inner = reservation_stage_guard(ReservationStage::LoanCfgEdges);
                std::panic::panic_any(11usize);
            }));
            assert!(unwind.is_err());
            assert_eq!(
                owner.stage.get(),
                Some(ReservationStage::CanonicalLoanStorage)
            );
            let (((), metrics), _, _, _) = with_limit_usage_refusal(0, || {
                measure(|| {
                    let _foreign =
                        reservation_stage_guard(ReservationStage::LoanCandidateInventory);
                    assert!(!reserve_active_required(9));
                })
            });
            assert!(!metrics.invalid);
            assert_eq!(metrics.allocated, 0);
            assert!(!reserve_active_required(99));
            assert_eq!(owner.first_refusal.get(), Some(selected));
        });
        assert!(overflow);
        assert_eq!(used, 3);
        assert_eq!(first.unwrap().stage, Some(stage));
        assert_eq!(first.unwrap().requested, 1);
        let observed = stage_debits::take_completed()
            .unwrap()
            .first_operation_refusal
            .unwrap();
        assert_eq!(
            observed.operation,
            Some(stage_debits::Operation::CanonicalLoanStorage)
        );
        assert_eq!(observed.stage, Some(stage));
        assert_eq!(
            (observed.requested, observed.remaining, observed.floor),
            (1, 0, 0)
        );
    }
}

#[test]
fn loan_allocation_class_fallback_preserves_all_inner_phases_and_unchanged_layouts() {
    let ((), refused, debit, first) = with_limit_usage_refusal(usize::MAX, || {
        let budget = active().unwrap();
        let (_, metrics) = measure(|| {
            for outer in [None, Some(ReservationStage::HirValidation)]
                .into_iter()
                .chain(INNER.into_iter().map(Some))
            {
                budget.stage.set(outer);
                for class in [
                    ReservationStage::LoanOrderedDirectoryNode,
                    ReservationStage::LoanVectorBacking,
                ] {
                    let guard = loan_allocation_stage_guard(class);
                    assert_eq!(budget.stage.get(), outer);
                    drop(guard);
                    assert_eq!(budget.stage.get(), outer);
                }
            }
            budget
                .stage
                .set(Some(ReservationStage::CanonicalLoanStorage));
            let guard = loan_allocation_stage_guard(ReservationStage::LoanVectorBacking);
            assert_eq!(
                budget.stage.get(),
                Some(ReservationStage::LoanVectorBacking)
            );
            drop(guard);
            assert_eq!(
                budget.stage.get(),
                Some(ReservationStage::CanonicalLoanStorage)
            );
        });
        assert_eq!(metrics.allocated, 0);
        assert_eq!(metrics.live, 0);
        assert!(!metrics.invalid);
        assert_eq!(size_of::<ReservationStage>(), 1);
        assert_eq!(size_of::<Option<ReservationStage>>(), 1);
    });
    assert!(!refused);
    assert_eq!(debit, 0);
    assert_eq!(first, None);
}
#[test]
fn loan_allocation_class_first_receipt_is_sticky_through_foreign_owner_and_unwind() {
    for class in [
        ReservationStage::LoanOrderedDirectoryNode,
        ReservationStage::LoanVectorBacking,
    ] {
        let ((), refused, debit, first) = with_limit_usage_refusal(0, || {
            let budget = active().unwrap();
            budget
                .stage
                .set(Some(ReservationStage::CanonicalLoanStorage));
            let guard = loan_allocation_stage_guard(class);
            let (_, metrics) = measure(|| {
                assert!(!reserve_active_required(1));
            });
            assert_eq!(metrics.allocated, 0);
            let selected = budget.first_refusal.get().unwrap();
            assert_eq!(
                selected,
                ReservationRefusal {
                    requested: 1,
                    remaining: 0,
                    floor: 0,
                    stage: Some(class)
                }
            );
            // The harness owns a distinct nested Budget allocation; guard
            // operations themselves remain zero-allocation and zero-debit.
            let ((), nested_refused, nested_debit, nested_first) =
                with_limit_usage_refusal(9, || {
                    let (_, metrics) = measure(|| {
                        let _nested =
                            loan_allocation_stage_guard(ReservationStage::LoanOrderedDirectoryNode);
                        assert!(reserve_active_required(0));
                        assert_eq!(budget.first_refusal.get(), Some(selected));
                    });
                    assert_eq!(metrics.allocated, 0);
                    assert_eq!(metrics.live, 0);
                });
            assert!(!nested_refused);
            assert_eq!(nested_debit, 0);
            assert_eq!(nested_first, None);
            let (_, metrics) = measure(|| {
                drop(guard);
                assert_eq!(
                    budget.stage.get(),
                    Some(ReservationStage::CanonicalLoanStorage)
                );
                assert!(!reserve_active_required(0));
                assert_eq!(budget.first_refusal.get(), Some(selected));
            });
            assert_eq!(metrics.allocated, 0);
            assert_eq!(metrics.live, 0);
            assert!(!metrics.invalid);
        });
        assert!(refused);
        assert_eq!(debit, 0);
        assert_eq!(first.unwrap().stage, Some(class));
    }
    let ((), refused, debit, first) = with_limit_usage_refusal(0, || {
        let budget = active().unwrap();
        budget
            .stage
            .set(Some(ReservationStage::CanonicalLoanStorage));
        // The panic payload is preexisting static text, outside the physical
        // guard observer; this checks RAII restoration, not panic allocation.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = loan_allocation_stage_guard(ReservationStage::LoanVectorBacking);
            panic!("class guard unwind");
        }));
        assert!(result.is_err());
        assert_eq!(
            budget.stage.get(),
            Some(ReservationStage::CanonicalLoanStorage)
        );
    });
    assert!(!refused);
    assert_eq!(debit, 0);
    assert_eq!(first, None);
}
