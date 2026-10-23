use super::*;

#[test]
fn ordinary_copy_scalar_probe_matches_the_complete_classifier_without_heap_backing() {
    let checked = crate::check(
        "module scalar_probe; @id(\"main\") fn main()->i64 {0}",
        "scalar-probe.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    for ty in [ResolvedType::I64, ResolvedType::Bool] {
        let expected = crate::cleanup::type_needs_resource_cleanup(&program, &ty).unwrap();
        assert!(!expected);
        let ((actual, allocation), refused, paid, receipt) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                crate::test_allocation_observer::measure(|| {
                    type_needs_drop(&program, function, &ty)
                })
            });
        assert_eq!(actual.unwrap(), expected);
        assert!(!refused && receipt.is_none());
        assert_eq!(paid, 0);
        assert!(!allocation.invalid);
        assert_eq!(
            (allocation.allocated, allocation.freed, allocation.live),
            (0, 0, 0)
        );
    }
}

#[test]
fn owned_leaf_probes_still_use_the_complete_ordinary_classifier() {
    let checked = crate::check(
        "module scalar_probe; @id(\"main\") fn main()->i64 {0}",
        "scalar-probe.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    for ty in [ResolvedType::Bytes, ResolvedType::String] {
        let expected = crate::cleanup::type_needs_resource_cleanup(&program, &ty).unwrap();
        assert!(expected);
        assert_eq!(type_needs_drop(&program, function, &ty).unwrap(), expected);
    }
}
