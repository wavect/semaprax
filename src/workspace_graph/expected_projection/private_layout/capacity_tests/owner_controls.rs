//! Existing owner, sticky and nested floor rules under the smaller real backing.
use super::super::registry;
use super::{
    active_remaining, index_authored, measure, observe, padded, programs, same_diagnostics,
    with_limit_usage_refusal, LayoutClosure, MAX_BUILDER_BYTES, PROVIDER, SINGLE_CALLER, WORK,
};
use crate::bounded_output::{clear_active_floor, set_active_floor};
#[test]
fn cold_registry_exact_short_zero_and_foreign_restored_contexts_keep_receipts_sticky() {
    let parsed = programs(SINGLE_CALLER, &padded(PROVIDER));
    let authored = index_authored(&parsed).unwrap();
    let baseline = registry::LayoutRegistry::new(&parsed, &authored);
    let (answer, overflow, used, _) = with_limit_usage_refusal(MAX_BUILDER_BYTES, || {
        baseline.get(&parsed[0], &authored, &parsed)
    });
    drop(answer.unwrap());
    assert!(!overflow);
    assert_eq!(baseline.receipt(), used);
    drop(baseline);
    for allowance in [used, used - 1, 0] {
        let (((), metrics), _, outer_used, _) = with_limit_usage_refusal(allowance, || {
            measure(|| {
                let registry = registry::LayoutRegistry::new(&parsed, &authored);
                let answer = registry.get(&parsed[0], &authored, &parsed);
                assert_eq!(answer.is_ok(), allowance == used);
                drop(answer);
                let first = registry.receipt();
                if allowance != used {
                    let remaining = active_remaining();
                    assert!(registry.get(&parsed[0], &authored, &parsed).is_err());
                    assert_eq!(registry.receipt(), first);
                    assert_eq!(active_remaining(), remaining);
                }
                assert!(first <= allowance);
                drop(registry);
            })
        });
        assert!(outer_used <= allowance);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
    let stale = parsed.clone();
    let stale_authored = index_authored(&stale).unwrap();
    for attack in 0..3 {
        let (((), metrics), _, _, _) = with_limit_usage_refusal(MAX_BUILDER_BYTES, || {
            measure(|| {
                let registry = registry::LayoutRegistry::new(&parsed, &authored);
                drop(registry.get(&parsed[0], &authored, &parsed).unwrap());
                let first = registry.receipt();
                let result = match attack {
                    0 => registry.get(&stale[0], &authored, &parsed),
                    1 => registry.get(&parsed[0], &stale_authored, &parsed),
                    _ => registry.get(&parsed[0], &authored, &stale),
                };
                assert!(result.is_err());
                drop(result);
                assert!(registry.get(&parsed[0], &authored, &parsed).is_err());
                assert_eq!(registry.receipt(), first);
                drop(registry);
            })
        });
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
}
#[test]
fn same_required_derived_bytes_preserve_nested_parent_floor_and_full_drop() {
    let parsed = programs(SINGLE_CALLER, &padded(PROVIDER));
    let authored = index_authored(&parsed).unwrap();
    let baseline = observe(&parsed, &authored, false, MAX_BUILDER_BYTES, WORK, None);
    let floor = std::mem::size_of::<LayoutClosure<'_>>();
    let (((), metrics), overflow, used, _) =
        with_limit_usage_refusal(baseline.reserved + floor, || {
            measure(|| {
                assert!(set_active_floor(floor));
                let ((), child_overflow, child_used, _) =
                    with_limit_usage_refusal(baseline.reserved, || {
                        let mut work = WORK;
                        let result = LayoutClosure::derive_with_work(
                            &parsed[0], &authored, &parsed, &mut work, None,
                        );
                        assert!(result.is_ok());
                        drop(result);
                    });
                assert!(!child_overflow);
                assert_eq!(child_used, baseline.reserved);
                assert_eq!(active_remaining(), Some(floor));
                clear_active_floor();
            })
        });
    assert!(!overflow);
    assert_eq!(used, baseline.reserved);
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    // Floor refusal is not relabelled as a sticky byte refusal: preserve the
    // existing reserve_active policy and the actual failed semantic result.
    let (result, _, _, _) = with_limit_usage_refusal(baseline.reserved + floor - 1, || {
        assert!(set_active_floor(floor));
        let mut work = WORK;
        let result =
            LayoutClosure::derive_with_work(&parsed[0], &authored, &parsed, &mut work, None);
        clear_active_floor();
        result
    });
    assert!(result.is_err());
}
#[test]
fn unchanged_signature_owner_rejects_foreign_type_then_cannot_recover_after_restoration() {
    let parsed = programs(SINGLE_CALLER, &padded(PROVIDER));
    let authored = index_authored(&parsed).unwrap();
    let target = authored.get("make").unwrap();
    let current_ty = &target.function.unwrap().return_type;
    let foreign_ty = current_ty.clone();
    for old in [true, false] {
        let (((), metrics), _, _, _) = with_limit_usage_refusal(MAX_BUILDER_BYTES, || {
            measure(|| {
                let mut work = WORK;
                let layout = if old {
                    LayoutClosure::derive_with_work_before_capacity(
                        &parsed[0], &authored, &parsed, &mut work, None,
                    )
                } else {
                    LayoutClosure::derive_with_work(&parsed[0], &authored, &parsed, &mut work, None)
                }
                .unwrap();
                assert_eq!(
                    layout
                        .signature_owner(&parsed[0], target, current_ty, &authored, &parsed)
                        .unwrap(),
                    Some(true)
                );
                let wrong = layout
                    .signature_owner(&parsed[0], target, &foreign_ty, &authored, &parsed)
                    .err()
                    .unwrap();
                let restored = layout
                    .signature_owner(&parsed[0], target, current_ty, &authored, &parsed)
                    .err()
                    .unwrap();
                same_diagnostics(&wrong, &restored);
                drop(restored);
                drop(wrong);
                drop(layout);
            })
        });
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
}
