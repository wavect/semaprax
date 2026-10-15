use super::*;
use crate::bounded_output::{self, ReservationStage};
use crate::test_allocation_observer::measure;

#[test]
fn actual_prepare_stage_keeps_full_old_storage_diagnostics_numeric_receipts_and_drop() {
    let program = super::super::tests::fixture();
    let seed = ConstructorSeed::authenticate(&program, None).unwrap();
    // This fixture's final current function is the loan-free monomorphic
    // identity instance. Its complete original planner returns no append data.
    // Raw sealing is an identity move, so the final positive reservation lies
    // in candidate inventory; later append authentication requests are zero.
    let final_function = program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .last()
        .unwrap();
    let final_plan = crate::loan_plan::build_plan(&program, final_function).unwrap();
    assert_eq!(final_plan.schema, crate::loan_plan::LOAN_PLAN_SCHEMA_V1);
    assert!(
        final_plan.loans.is_empty()
            && final_plan.endpoints.is_empty()
            && final_plan.edges.is_empty()
    );
    drop(final_plan);
    let (expected, deferred) = owned_before::prepare_original(&seed, &program).unwrap();
    drop(deferred);
    let run = |saved, limit| {
        bounded_output::with_limit_usage_refusal(limit, || {
            bounded_output::with_reservation_stage(ReservationStage::FreshResolution, || {
                measure(|| {
                    let result = if saved {
                        owned_before::prepare_original(&seed, &program)
                    } else {
                        prepare_with_comparison(&seed, &program)
                    };
                    let diagnostic = match result {
                        Ok((storage, deferred)) => {
                            assert_eq!(storage.ids, expected.ids);
                            assert_eq!(storage.liveness, expected.liveness);
                            assert_eq!(storage.origins, expected.origins);
                            drop(deferred);
                            drop(storage);
                            None
                        }
                        Err(error) => {
                            assert!(!bounded_output::reserve_active_required(0));
                            let retry = if saved {
                                owned_before::prepare_original(&seed, &program)
                            } else {
                                prepare_with_comparison(&seed, &program)
                            }
                            .err()
                            .unwrap();
                            assert_eq!(retry.code, "SPX-H006");
                            assert!(retry.message.is_empty());
                            drop(retry);
                            Some(error)
                        }
                    };
                    // Retain only fixed diagnostic equality coordinates outside
                    // the observer; owned error backing is freed inside it.
                    let failed = diagnostic.is_some();
                    if let Some(error) = diagnostic {
                        let expected = if limit == 0 {
                            ""
                        } else {
                            "canonical loan CFG storage reservation refused"
                        };
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                        assert_eq!(error.message, expected);
                        assert!(
                            error.path.is_none() && error.span.is_none() && error.help.is_none()
                        );
                        drop(error);
                    }
                    failed
                })
            })
        })
    };
    let (_, overflow, exact, first) = run(false, usize::MAX);
    assert!(!overflow && first.is_none());
    for limit in [usize::MAX, exact, exact - 1, 0] {
        let old = run(true, limit);
        let current = run(false, limit);
        assert_eq!(current.0, old.0);
        assert_eq!((current.1, current.2), (old.1, old.2));
        let mut normalized = current.3;
        if let Some(receipt) = &mut normalized {
            if limit == 0 {
                assert_eq!(receipt.stage, Some(ReservationStage::CanonicalLoanStorage));
                // The independently compiled old outer receiver has no
                // canonical-stage scope around its initial acquisition.
                assert_eq!(
                    old.3.unwrap().stage,
                    Some(ReservationStage::FreshResolution)
                );
                receipt.stage = Some(ReservationStage::FreshResolution);
            } else {
                assert_eq!(limit, exact - 1);
                assert_eq!(
                    receipt.stage,
                    Some(ReservationStage::LoanCandidateInventory)
                );
                // Both receivers invoke that same real paid candidate producer;
                // preserve its now-more-precise coordinate rather than erasing it.
                assert_eq!(
                    old.3.unwrap().stage,
                    Some(ReservationStage::LoanCandidateInventory)
                );
            }
        }
        assert_eq!(normalized, old.3);
        let metrics = current.0 .1;
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
        assert!(metrics.allocated <= current.2 && metrics.highwater <= current.2);
    }
}

#[test]
fn actual_shared_seal_stage_preserves_exact_short_zero_numeric_receipts_and_complete_drop() {
    let program = super::super::tests::fixture();
    let seed = ConstructorSeed::authenticate(&program, None).unwrap();
    let (expected, deferred) = owned_before::prepare_original(&seed, &program).unwrap();
    drop(deferred);
    let run = |saved, limit| {
        bounded_output::with_limit_usage_refusal(limit, || {
            bounded_output::with_reservation_stage(ReservationStage::FreshResolution, || {
                measure(|| {
                    let result = if saved {
                        sealed_before_stage::prepare_before_stage(&seed, &program, shared::seal)
                            .map(|(storage, _)| storage)
                    } else {
                        prepare_shared(&seed)
                    };
                    match result {
                        Ok(storage) => {
                            assert_eq!(storage.ids, expected.ids);
                            assert_eq!(storage.liveness, expected.liveness);
                            assert_eq!(storage.origins, expected.origins);
                            assert!(storage.is_current(&program));
                            drop(storage);
                            true
                        }
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                            assert_eq!(
                                error.message,
                                if limit == 0 {
                                    ""
                                } else {
                                    "canonical loan CFG storage reservation refused"
                                }
                            );
                            assert!(
                                error.path.is_none()
                                    && error.span.is_none()
                                    && error.help.is_none()
                            );
                            drop(error);
                            let before = crate::test_allocation_observer::current_metrics();
                            let retry = if saved {
                                sealed_before_stage::prepare_before_stage(
                                    &seed,
                                    &program,
                                    shared::seal,
                                )
                                .map(|(storage, _)| storage)
                            } else {
                                prepare_shared(&seed)
                            }
                            .err()
                            .unwrap();
                            assert_eq!(retry.code, "SPX-H006");
                            assert!(retry.message.is_empty());
                            drop(retry);
                            assert!(!bounded_output::reserve_active_required(0));
                            assert_eq!(crate::test_allocation_observer::current_metrics(), before);
                            false
                        }
                    }
                })
            })
        })
    };
    let ((ok, initial), refused, exact, first) = run(false, usize::MAX);
    assert!(ok && !refused && first.is_none() && !initial.invalid);
    let seal_request = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<Storage<'_>>())
        .unwrap()
        .0
        .pad_to_align()
        .size();
    for limit in [usize::MAX, exact, exact - 1, 0] {
        let old = run(true, limit);
        let current = run(false, limit);
        assert_eq!(current.0, old.0);
        assert_eq!((current.1, current.2), (old.1, old.2));
        let mut first = current.3;
        if let Some(receipt) = &mut first {
            assert_eq!(receipt.stage, Some(ReservationStage::CanonicalLoanStorage));
            if limit == exact - 1 {
                assert_eq!(
                    receipt.requested, seal_request,
                    "one-short must reach actual shared sealing"
                );
                assert_eq!(receipt.remaining, seal_request - 1);
                assert_eq!(receipt.floor, 0);
            }
            receipt.stage = Some(ReservationStage::FreshResolution);
        }
        assert_eq!(first, old.3);
        let metrics = current.0 .1;
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
        assert!(metrics.allocated <= current.2 && metrics.highwater <= current.2);
    }
    let original = ci_str!("sealed_before_stage.rs")
        .strip_prefix("//! Complete 4ec shared-sealing receiver before production stage attribution.\nuse super::*;\n").unwrap()
        .replacen("pub(super) fn prepare_before_stage<'a, T>(", "fn prepare_sealed<'a, T>(", 1);
    let archive = ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__hir__canonical_loan_storage__caller.rs.sealed-before.txt"));
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
        ),
        "86872491c35cb785ed81356bd181eaad9c337127754ee0b2e20ab6c7d10d29f8"
    );
    assert_eq!(original, archive);
}
