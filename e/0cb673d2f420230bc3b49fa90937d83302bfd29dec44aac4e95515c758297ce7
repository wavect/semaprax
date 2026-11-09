//! Exact read-only index construction behind complete live HIR validation.
use super::*;
#[path = "validation_before.rs"]
mod before;
fn program() -> ResolvedProgram {
    let source = "module index.receipt; @id(\"helper\") fn helper(x:i64)->i64{x+1} @id(\"main\") fn main()->i64{helper(41)}";
    let parsed = crate::check(source, "index-receipt.spx").unwrap();
    hir::resolve(&parsed).unwrap()
}
fn same(a: &PersistentCallIndex, b: &PersistentCallIndex) {
    assert_eq!(a.sites_by_expression, b.sites_by_expression);
    assert_eq!(a.calls_by_owner, b.calls_by_owner);
    assert_eq!(a.callers_by_callee, b.callers_by_callee);
    assert_eq!(a.kinds_by_owner, b.kinds_by_owner);
    assert_eq!(a.origins_by_owner, b.origins_by_owner);
}
#[test]
fn checked_index_preserves_every_map_and_only_removes_duplicate_hir_validation() {
    let program = program();
    let limit = 64 * 1024 * 1024;
    let (checked, refused, validation_bytes, _) =
        crate::bounded_output::with_limit_usage_refusal(limit, || hir::validate(&program));
    checked.unwrap();
    assert!(!refused && validation_bytes > 0);
    let (old, refused, old_paid, _) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            PersistentCallIndex::build_before(&program)
        });
    let old = old.unwrap();
    assert!(!refused);
    let owner = OrdinaryValidatedProgram::validate(&program).unwrap();
    let (current, refused, paid, _) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            PersistentCallIndex::build_validated(&owner)
        });
    same(&old, &current.unwrap());
    assert!(!refused);
    assert_eq!(old_paid - paid, validation_bytes);
    for bytes in [0, old_paid - 1, old_paid] {
        let (old, old_refused, old_bill, old_first) =
            crate::bounded_output::with_limit_usage_refusal(bytes, || {
                PersistentCallIndex::build_before(&program)
            });
        let (new, new_refused, new_bill, new_first) =
            crate::bounded_output::with_limit_usage_refusal(bytes, || {
                PersistentCallIndex::build(&program)
            });
        assert_eq!(old_refused, new_refused);
        assert_eq!(old_bill, new_bill);
        assert_eq!(old_first, new_first);
        match (old, new) {
            (Ok(a), Ok(b)) => same(&a, &b),
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => panic!("raw constructor verdict changed"),
        }
    }
    let before = include_str!("validation_before.rs");
    let a = before.find("        let mut index = Self {").unwrap();
    let b = before.find("        Ok(index)\n").unwrap() + "        Ok(index)\n".len();
    assert_eq!(
        include_str!("../call_index.rs")
            .matches(&before[a..b])
            .count(),
        1
    );
}
#[test]
fn sealed_index_cannot_clear_sticky_refusal_or_admit_mutated_hir() {
    let mut program = program();
    let owner = OrdinaryValidatedProgram::validate(&program).unwrap();
    let ((result, metrics), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            crate::test_allocation_observer::measure(|| {
                PersistentCallIndex::build_validated(&owner)
            })
        });
    assert!(result.is_err() && refused && first.is_some());
    assert_eq!(paid, 0);
    assert_eq!(metrics.allocated, 0);
    assert_eq!(metrics.freed, 0);
    assert!(!metrics.invalid);
    program.entrypoint = DeclarationId::new("missing.main");
    let expected = hir::validate(&program).unwrap_err();
    match OrdinaryValidatedProgram::validate(&program) {
        Err(error) => assert_eq!(error, expected),
        Ok(_) => panic!("hostile HIR received an ordinary proof"),
    }
    assert_eq!(
        PersistentCallIndex::build(&program).err().unwrap(),
        expected
    );
    assert_eq!(
        PersistentCallIndex::build_before(&program).err().unwrap(),
        expected
    );
}
