use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::hir::prelude_bundle::extension_work::{ExtensionPayer, VECTOR_WORK};
use crate::test_allocation_observer::{current_metrics, measure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stop(usize);

#[test]
fn all_selected_prefixes_keep_full_old_index_wire_and_reuse_only_selected_vec_payload() {
    let program = crate::parse(
        "module extension.controls; @id(\"app.main\") fn main()->i64 {0}",
        "extension.spx",
    )
    .unwrap();
    let source = crate::prelude::declarations();
    let ids = [
        crate::prelude::VEC_ID,
        crate::prelude::BOX_ID,
        crate::iterator_ops::ITER_ID,
        crate::iterator_ops::STEP_ID,
        crate::list_ops::LIST_ID,
        crate::list_ops::STEP_ID,
        crate::stdin_stream_ops::READER_ID,
        crate::map_ops::MAP_ID,
        crate::map_ops::SET_ID,
    ];
    for slot in 0..9 {
        assert_eq!(
            source[slot + 2].name,
            crate::hir::prelude_bundle::extension_model::NAMES[slot]
        );
        assert_eq!(source[slot + 2].stable_id, ids[slot]);
        assert_eq!(
            matches!(
                source[slot + 2].kind,
                crate::ast::TypeDeclarationKind::Variant { .. }
            ),
            matches!(slot, 3 | 5)
        );
        if !matches!(slot, 3 | 5) {
            assert!(
                matches!(&source[slot + 2].kind, crate::ast::TypeDeclarationKind::Record { fields } if fields.is_empty())
            );
        }
    }
    for count in [2, 3, 4, 6, 8, 9, 11] {
        let prelude = &crate::prelude::declarations()[..count];
        let old =
            crate::hir::DeclarationIndex::nominal_original_with_prelude(&program, prelude).unwrap();
        let ((first, second, receipt), refused, _, first_error) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut invocation = Invocation::new();
                let mut reserve = |n| {
                    if reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(Stop(1))
                    }
                };
                let (base, vector) = invocation.ensure_selected(prelude, &mut reserve).unwrap();
                assert_eq!(vector.is_some(), count > 2);
                let first = crate::hir::DeclarationIndex::from_verified_with_nominal_extensions(
                    &program,
                    prelude,
                    Some(base),
                    vector,
                    &mut reserve,
                )
                .unwrap();
                let receipt = invocation.extension_receipt();
                let (base, vector) = invocation.ensure_selected(prelude, &mut reserve).unwrap();
                let second = crate::hir::DeclarationIndex::from_verified_with_nominal_extensions(
                    &program,
                    prelude,
                    Some(base),
                    vector,
                    &mut reserve,
                )
                .unwrap();
                assert_eq!(invocation.extension_receipt(), receipt);
                for slot in 0..9 {
                    let actual = invocation.extension_receipts()[slot];
                    if slot < count - 2 {
                        let expected = crate::hir::prelude_bundle::extension_model::WORK[slot];
                        let actual = actual.unwrap();
                        assert_eq!(
                            (actual.limit, actual.used, actual.remaining, actual.first),
                            (expected, expected, 0, None)
                        );
                        let name = crate::hir::prelude_bundle::extension_model::NAMES[slot];
                        let id = first.type_id(name).unwrap();
                        let left = first.parameter_payload(id).unwrap();
                        let right = second.parameter_payload(id).unwrap();
                        assert_eq!(left.as_ptr(), right.as_ptr());
                        if matches!(slot, 3 | 5) {
                            let left = first.case_payload(id).unwrap();
                            let right = second.case_payload(id).unwrap();
                            assert_eq!(left.as_ptr(), right.as_ptr());
                        }
                    } else {
                        assert_eq!(actual, None);
                    }
                }
                (first, second, receipt)
            });
        assert!(!refused && first_error.is_none());
        for index in [first, second] {
            assert_eq!(index, old);
            assert_eq!(
                crate::cache_codec::encode(&index).unwrap(),
                crate::cache_codec::encode(&old).unwrap()
            );
        }
        assert_eq!(receipt.is_some(), count > 2);
    }
}

#[test]
fn vector_work_exact_short_zero_and_sticky_failure_keep_whole_producer_drop_paid() {
    // Existing immutable producer input belongs to the source owner, outside
    // the complete paid constructor/refusal/Drop observation window.
    let source = crate::prelude::declarations();
    for limit in [0, VECTOR_WORK - 1, VECTOR_WORK] {
        let ((success, allocation), refused, used, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut reserve = |n| {
                        if reserve_active_required(n) {
                            Ok(())
                        } else {
                            Err(Stop(1))
                        }
                    };
                    let mut payer = ExtensionPayer::acquire(&mut reserve).unwrap();
                    payer.restrict_for_test(limit);
                    let result = VectorExtension::acquire(&source[2], &mut payer);
                    let receipt = payer.receipt();
                    if limit == VECTOR_WORK {
                        assert!(result.is_ok());
                        assert_eq!(
                            (
                                receipt.limit,
                                receipt.used,
                                receipt.remaining,
                                receipt.first
                            ),
                            (VECTOR_WORK, VECTOR_WORK, 0, None)
                        );
                        drop(result);
                        let refusal = payer.work(1).unwrap_err();
                        assert_eq!(
                            refusal,
                            FixedFailure::Work(WorkRefusal {
                                requested: 1,
                                remaining: 0
                            })
                        );
                        let at = current_metrics().allocated;
                        assert_eq!(payer.work(0), Err(refusal));
                        assert_eq!(
                            payer.bytes(usize::MAX),
                            Err(FixedFailure::Work(WorkRefusal {
                                requested: 1,
                                remaining: 0
                            }))
                        );
                        assert_eq!(current_metrics().allocated, at);
                        true
                    } else {
                        let Err(Failure::Payer(FixedFailure::Work(selected))) = result else {
                            panic!("expected exact work refusal")
                        };
                        let at = current_metrics().allocated;
                        assert_eq!(payer.work(0), Err(FixedFailure::Work(selected)));
                        assert_eq!(payer.bytes(usize::MAX), Err(FixedFailure::Work(selected)));
                        assert_eq!(current_metrics().allocated, at);
                        false
                    }
                })
            });
        assert_eq!(success, limit == VECTOR_WORK);
        assert!(!refused && first.is_none());
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= used);
    }
}

#[test]
fn vector_every_byte_acquisition_and_whole_owner_exact_short_zero_are_sticky() {
    // Existing immutable producer input belongs to the source owner, outside
    // the complete paid constructor/refusal/Drop observation window.
    let source = crate::prelude::declarations();
    let mut requests = Vec::new();
    let (_, refused, exact, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut reserve = |n| {
            requests.push(n);
            if reserve_active_required(n) {
                Ok(())
            } else {
                Err(Stop(1))
            }
        };
        let mut payer = ExtensionPayer::acquire(&mut reserve).unwrap();
        drop(VectorExtension::acquire(&source[2], &mut payer).unwrap());
    });
    assert!(!refused && exact > 0);
    for stop in 0..requests.len() {
        let mut seen = 0;
        let mut selected_at = 0;
        let ((result, allocation), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut reserve = |n| {
                    assert_eq!(n, requests[seen]);
                    let at = seen;
                    seen += 1;
                    if at == stop {
                        selected_at = current_metrics().allocated;
                        Err(Stop(at))
                    } else if reserve_active_required(n) {
                        Ok(())
                    } else {
                        panic!("ample byte account")
                    }
                };
                let mut payer = match ExtensionPayer::acquire(&mut reserve) {
                    Ok(p) => p,
                    Err(e) => return Err(e),
                };
                let result = VectorExtension::acquire(&source[2], &mut payer);
                let Err(Failure::Payer(FixedFailure::Bytes(error))) = result else {
                    panic!("expected exact byte error")
                };
                let at = current_metrics().allocated;
                assert_eq!(payer.bytes(0), Err(FixedFailure::Bytes(error)));
                assert_eq!(payer.work(usize::MAX), Err(FixedFailure::Bytes(error)));
                assert_eq!(current_metrics().allocated, at);
                Err(error)
            })
        });
        assert_eq!(result, Err::<(), _>(Stop(stop)));
        assert!(!refused && !allocation.invalid);
        assert_eq!(seen, stop + 1);
        assert_eq!(allocation.allocated, selected_at);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    for cap in [0, exact - 1, exact] {
        let ((success, allocation), refused, used, first) = with_limit_usage_refusal(cap, || {
            measure(|| {
                let mut reserve = |n| {
                    if reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(Stop(1))
                    }
                };
                let mut payer = match ExtensionPayer::acquire(&mut reserve) {
                    Ok(p) => p,
                    Err(_) => return false,
                };
                let result = VectorExtension::acquire(&source[2], &mut payer);
                let success = result.is_ok();
                drop(result);
                success
            })
        });
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        if cap == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(used, exact);
            assert!(allocation.allocated <= used);
        } else {
            assert!(!success && refused && first.is_some());
            if cap == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn vector_work_owner_foreign_then_restored_preserves_first_raw_detached() {
    use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
    let (((), allocation), failed, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut owner = IdentityReservation::acquire().unwrap();
            let mut reserve = |n| owner.reserve(n);
            let mut payer = ExtensionPayer::acquire(&mut reserve).unwrap();
            let (_, nested_failed, nested_used, _) = with_limit_usage_refusal(usize::MAX, || {
                assert_eq!(payer.work(0), Err(FixedFailure::Bytes(Refused::Detached)));
            });
            assert!(!nested_failed && nested_used == 0);
            let at = current_metrics().allocated;
            assert_eq!(payer.bytes(0), Err(FixedFailure::Bytes(Refused::Detached)));
            assert_eq!(
                payer.work(usize::MAX),
                Err(FixedFailure::Bytes(Refused::Detached))
            );
            assert_eq!(current_metrics().allocated, at);
            #[allow(
                clippy::drop_non_drop,
                reason = "End this borrowed proof at the measured lifetime boundary."
            )]
            drop(payer);
            #[allow(
                clippy::drop_non_drop,
                reason = "End this borrowed proof at the measured lifetime boundary."
            )]
            drop(reserve);
            drop(owner);
        })
    });
    assert!(failed);
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn matching_foreign_ast_never_acquires_or_substitutes_extension_authority() {
    let original = &crate::prelude::declarations()[..3];
    let foreign = original.to_vec();
    let (_, refused, used, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut invocation = Invocation::<Stop>::new();
        let mut called = false;
        let error = invocation
            .ensure_selected(&foreign, &mut |_| {
                called = true;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(
            error,
            InvocationFailure::Producer(super::super::ProducerFailure::Schema)
        );
        assert!(!called);
        assert_eq!(invocation.extension_receipt(), None);
        let retried = invocation
            .ensure_selected(original, &mut |_| {
                called = true;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(retried, error);
        assert!(!called);
    });
    assert!(!refused && used == 0 && first.is_none());
    let (_, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut invocation = Invocation::new();
        let mut reserve = |n| {
            if reserve_active_required(n) {
                Ok(())
            } else {
                Err(Stop(1))
            }
        };
        let (_, vector) = invocation.ensure_selected(original, &mut reserve).unwrap();
        assert!(VectorExtension::for_source(vector, original, &foreign[2]).is_none());
        assert!(VectorExtension::for_source(vector, &foreign, &foreign[2]).is_none());
        assert!(VectorExtension::for_source(vector, original, &original[2]).is_some());
    });
    assert!(!refused);
}

#[test]
fn extension_failure_remains_whole_invocation_sticky_even_for_base_only_retry() {
    let source = crate::prelude::declarations();
    assert_eq!(source[2].stable_id, crate::prelude::VEC_ID);
    assert_eq!(source[2].name, "Vec");
    assert_eq!(source[2].type_parameters[0].name, "T");
    let (_, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut invocation = Invocation::new();
        invocation.restrict_extension_work(0);
        let mut reserve = |n| {
            if reserve_active_required(n) {
                Ok(())
            } else {
                Err(Stop(1))
            }
        };
        let error = invocation
            .ensure_selected(&source[..3], &mut reserve)
            .unwrap_err();
        assert_eq!(
            error,
            InvocationFailure::ExtensionWork(WorkRefusal {
                requested: 1,
                remaining: 0
            })
        );
        let mut called = false;
        let retry = invocation
            .ensure_selected(&source[..2], &mut |_| {
                called = true;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(retry, error);
        assert!(!called);
        assert_eq!(invocation.receipt().unwrap().remaining, 0);
    });
    assert!(!refused && first.is_none());
}

#[test]
fn every_extension_slot_work_exact_short_zero_and_partial_roots_drop_without_allocation() {
    // Existing immutable producer input belongs to the source owner, outside
    // the complete paid constructor/refusal/Drop observation window.
    let source = crate::prelude::declarations();
    for target in 0..9 {
        let bound = crate::hir::prelude_bundle::extension_model::WORK[target];
        for limit in [0, bound - 1, bound] {
            let ((success, allocation), refused, used, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut reserve = |n| {
                            if reserve_active_required(n) {
                                Ok(())
                            } else {
                                Err(Stop(1))
                            }
                        };
                        reserve(2 * std::mem::size_of::<VectorExtension>()).unwrap();
                        let mut catalog = VectorExtension::empty();
                        for slot in 0..=target {
                            let mut payer =
                                ExtensionPayer::acquire_for(slot, &mut reserve).unwrap();
                            if slot == target {
                                payer.restrict_for_test(limit);
                            }
                            let result = crate::hir::prelude_bundle::extension_producer::acquire(
                                slot,
                                &source[slot + 2],
                                &catalog.values,
                                &mut payer,
                            );
                            if slot == target && limit < bound {
                                let Err(Failure::Payer(FixedFailure::Work(selected))) = result
                                else {
                                    panic!("exact slot work refusal")
                                };
                                let at = current_metrics().allocated;
                                assert_eq!(payer.work(0), Err(FixedFailure::Work(selected)));
                                assert_eq!(
                                    payer.bytes(usize::MAX),
                                    Err(FixedFailure::Work(selected))
                                );
                                drop(catalog);
                                assert_eq!(current_metrics().allocated, at);
                                return false;
                            }
                            let receipt = payer.receipt();
                            let expected = crate::hir::prelude_bundle::extension_model::WORK[slot];
                            assert_eq!(
                                (
                                    receipt.limit,
                                    receipt.used,
                                    receipt.remaining,
                                    receipt.first
                                ),
                                (expected, expected, 0, None)
                            );
                            catalog.values[slot] = Some(result.unwrap());
                        }
                        drop(catalog);
                        true
                    })
                });
            assert_eq!(success, limit == bound);
            assert!(!refused && first.is_none());
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= used);
        }
    }
}

#[test]
fn complete_selected_suffix_every_byte_acquisition_exact_short_zero_and_sticky_drop() {
    let source = crate::prelude::declarations();
    let mut requests = Vec::new();
    let (_, refused, exact, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut invocation = Invocation::new();
        let mut reserve = |n| {
            requests.push(n);
            if reserve_active_required(n) {
                Ok(())
            } else {
                Err(Stop(1))
            }
        };
        invocation.ensure_selected(source, &mut reserve).unwrap();
        drop(invocation);
    });
    assert!(!refused && exact > 0);
    for stop in 0..requests.len() {
        let mut seen = 0;
        let mut selected_at = 0;
        let (((), allocation), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut invocation = Invocation::new();
                let mut reserve = |n| {
                    assert_eq!(n, requests[seen]);
                    let current = seen;
                    seen += 1;
                    if current == stop {
                        selected_at = current_metrics().allocated;
                        Err(Stop(current))
                    } else if reserve_active_required(n) {
                        Ok(())
                    } else {
                        panic!("ample byte owner")
                    }
                };
                assert!(
                    matches!(invocation.ensure_selected(source, &mut reserve), Err(InvocationFailure::Bytes(Stop(index))) if index == stop)
                );
                let mut forbidden =
                    |_| -> Result<(), Stop> { panic!("sticky invocation cannot requery") };
                assert!(
                    matches!(invocation.ensure_selected(&source[..2], &mut forbidden), Err(InvocationFailure::Bytes(Stop(index))) if index == stop)
                );
                drop(invocation);
            })
        });
        assert!(!refused && !allocation.invalid);
        assert_eq!(seen, stop + 1);
        assert_eq!(allocation.allocated, selected_at);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    for cap in [0, exact - 1, exact] {
        let ((success, allocation), refused, used, first) = with_limit_usage_refusal(cap, || {
            measure(|| {
                let mut invocation = Invocation::new();
                let mut reserve = |n| {
                    if reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(Stop(1))
                    }
                };
                let success = invocation.ensure_selected(source, &mut reserve).is_ok();
                drop(invocation);
                success
            })
        });
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        if cap == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(used, exact);
            assert!(allocation.allocated <= used);
        } else {
            assert!(!success && refused && first.is_some());
            if cap == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
