//! Exact index backing, owner lifetime and final owning output; no forecast claim.
use super::*;

#[test]
fn sparse_checked_ordinals_preserve_cold_warm_body_identity_old_wire_and_paid_drop() {
    let source = (0..9).fold(String::from("module local;"), |mut source, index| {
        let name = if index == 8 {
            "main".to_owned()
        } else {
            format!("f{index}")
        };
        source.push_str(&format!(
            " @id(\"local.f{index}\") fn {name}() -> i64 {{ {index} }}"
        ));
        source
    });
    for count in [3usize, 5] {
        for cache_first in [false, true] {
            let (mut pass, synthetic, owner) =
                crate::project::incremental::checked_hir::tests::cold_source(&source);
            let (warm, _, _) = pass.checked_module("local.spx", &synthetic).unwrap();
            let full_wire = crate::cache_codec::encode(&*owner).unwrap();
            let keep = |function: &hir::ResolvedFunction| {
                let index = function
                    .id
                    .as_str()
                    .strip_prefix("local.f")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                index % 2 == 0 && index / 2 < count
            };
            let expected = select_owned_oracle(owner.clone(), |p| &p.functions, keep)
                .unwrap()
                .into_owned();
            let expected_wire = crate::cache_codec::encode(&expected).unwrap();
            // One overlapping local Vec header and exactly count authored usize
            // ordinals. The module's retained carrier is already separately paid.
            let request = (3 + count) * std::mem::size_of::<usize>();
            let payload = count * std::mem::size_of::<usize>();
            assert_eq!(
                std::mem::size_of::<Vec<usize>>(),
                3 * std::mem::size_of::<usize>()
            );
            let (observed, overflow, charged, _) =
                crate::bounded_output::with_limit_usage_refusal(request, || {
                    crate::test_allocation_observer::measure(|| {
                        let view = Retained::select(warm.clone(), |p| &p.functions, keep).unwrap();
                        assert_eq!(view.len(), count);
                        assert!(matches!(
                            &view,
                            Retained::Checked {
                                selection: Selection::Ordinals(_),
                                ..
                            }
                        ));
                        for (ordinal, actual) in view.iter().enumerate() {
                            assert!(std::ptr::eq(actual, &owner.functions[ordinal * 2]));
                            assert!(std::ptr::eq(
                                &actual.body,
                                &warm.functions[ordinal * 2].body
                            ));
                            assert_eq!(view.get(ordinal).unwrap() as *const _, actual as *const _);
                        }
                        assert!(view.get(count).is_none());
                        assert_eq!(
                            view.iter().next_back().unwrap().id,
                            owner.functions[(count - 1) * 2].id
                        );
                        drop(view);
                    })
                    .1
                });
            assert!(!overflow);
            assert_eq!(charged, request);
            assert_eq!((observed.allocations, observed.reallocations), (1, 0));
            assert_eq!(
                (
                    observed.allocated,
                    observed.highwater,
                    observed.freed,
                    observed.live
                ),
                (payload, payload, payload, 0)
            );
            assert!(!observed.invalid);
            let ((refused, sticky, refused_allocations), overflow, charged, _) =
                crate::bounded_output::with_limit_usage_refusal(request - 1, || {
                    let ((refused, sticky), observation) =
                        crate::test_allocation_observer::measure(|| {
                            let refused =
                                Retained::select(warm.clone(), |p| &p.functions, keep).is_err();
                            let sticky = !crate::bounded_output::reserve_active_required(0);
                            assert!(
                                Retained::select(warm.clone(), |p| &p.functions, |_| true).is_err()
                            );
                            (refused, sticky)
                        });
                    // Diagnostics are inside this scope; a refusal publishes no
                    // ordinal backing or cloned HIR. No total diagnostic-cost claim.
                    assert!(!observation.invalid && observation.live == 0);
                    (refused, sticky, observation.allocations)
                });
            assert!(overflow && refused && sticky);
            assert_eq!(charged, 0);
            assert!(refused_allocations > 0);
            let view = Retained::select(warm.clone(), |p| &p.functions, keep).unwrap();
            assert_eq!(
                crate::graph::graph_schema_from_parts_without_loans(
                    &owner.interfaces,
                    &owner.types,
                    &view,
                    &owner.function_templates
                )
                .unwrap(),
                crate::graph::graph_schema_from_parts_without_loans(
                    &owner.interfaces,
                    &owner.types,
                    &expected,
                    &owner.function_templates
                )
                .unwrap()
            );
            if cache_first {
                drop(pass);
                drop(owner);
                drop(warm);
            } else {
                drop(warm);
                drop(view);
                assert_eq!(crate::cache_codec::encode(&*owner).unwrap(), full_wire);
                continue;
            }
            // Materialization is the real owning boundary, not an internal view.
            let published = view.into_owned();
            assert_eq!(published, expected);
            assert_eq!(
                crate::cache_codec::encode(&published).unwrap(),
                expected_wire
            );
            for (actual, old) in published.iter().zip(&expected) {
                assert!(!std::ptr::eq(&actual.body, &old.body));
            }
        }
    }
}
