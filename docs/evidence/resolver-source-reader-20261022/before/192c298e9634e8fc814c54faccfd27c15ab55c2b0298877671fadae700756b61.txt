use super::*;
use crate::hir::{ResolvedExprKind, ResolvedType};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn children(count: usize) -> Vec<ResolvedExpr> {
    let source = "module extraction; @id(\"extraction.main\") fn main() -> i64 { 7 }";
    let ast = crate::check(source, "extraction.spx").unwrap();
    let resolved = crate::hir::resolve(&ast).unwrap();
    (0..count)
        .map(|index| {
            let mut child = resolved.functions[0].body.clone();
            child.kind = ResolvedExprKind::Int(index as i64);
            child.ty = ResolvedType::I64;
            child
        })
        .collect()
}

#[test]
fn pair_and_triple_extract_in_authored_order_without_temporary_allocation() {
    for count in [2, 3] {
        let mut old = children(count + 1);
        let mut new = old.clone();
        let (expected, old_allocations) =
            crate::test_allocation_observer::measure(|| saved_old(&mut old, count));
        let (actual, observed) = crate::test_allocation_observer::measure(|| {
            if count == 2 {
                let (a, b) = take_pair(&mut new);
                ([Some(a), Some(b), None], 2)
            } else {
                let (a, b, c) = take_triple(&mut new);
                ([Some(a), Some(b), Some(c)], 3)
            }
        });
        assert!(!observed.invalid && !old_allocations.invalid);
        assert_eq!(
            (
                observed.allocations,
                observed.reallocations,
                observed.allocated
            ),
            (0, 0, 0)
        );
        assert_eq!(old_allocations.allocations, 1);
        assert_eq!(
            old_allocations.allocated,
            count * std::mem::size_of::<ResolvedExpr>()
        );
        assert_eq!(new, old);
        assert_eq!(actual.1, count);
        for (actual, expected) in actual.0.into_iter().flatten().zip(expected) {
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn short_fixed_result_inputs_refuse_before_mutation_and_old_oracle_matches() {
    for count in [2, 3] {
        for length in 0..count {
            let mut current = children(length);
            let expected = current.clone();
            let refusal = catch_unwind(AssertUnwindSafe(|| {
                if count == 2 {
                    drop(take_pair(&mut current));
                } else {
                    drop(take_triple(&mut current));
                }
            }));
            assert!(refusal.is_err());
            assert_eq!(current, expected);
            let old = catch_unwind(AssertUnwindSafe(|| saved_old(&mut current, count)));
            assert!(old.is_err());
            assert_eq!(current, expected);
        }
    }
}

#[test]
fn saved_split_off_resolver_keeps_complete_hir_loans_cleanup_and_lazy_shapes() {
    let mut wide = String::from(
        "module extraction; @id(\"extraction.main\") fn main() -> i64 { let mut sum = 0;",
    );
    for index in 0..128 {
        wide.push_str(&format!(
            "sum = sum + if true {{ {index} }} else {{ 1 / 0 }};"
        ));
    }
    wide.push_str("sum }");
    let nested = (0..32).fold("7".to_owned(), |tail, _| {
        format!("if true {{ {tail} + 1 }} else {{ 1 / 0 }}")
    });
    let cases = [wide, format!("module extraction; @id(\"extraction.main\") fn main() -> i64 {{ {nested} }}"),
        "module extraction; @id(\"extraction.main\") fn main() -> i64 { let bytes = bytes_zeroed(3usize); let view = bytes_as_slice(bytes); if byte_len(view) == 3usize && true { 7 + 2 } else { 1 / 0 } }".to_owned(),
        "module extraction; @id(\"extraction.main\") fn main() -> i64 { if true { 1 / 0 } else { 3 } }".to_owned()];
    for (case, source) in cases.into_iter().enumerate() {
        let ast = crate::check(&source, "extraction.spx").unwrap();
        let old = with_saved_old(|| crate::hir::resolve(&ast)).unwrap();
        let current = crate::hir::resolve(&ast).unwrap();
        crate::hir::validate(&old).unwrap();
        crate::hir::validate(&current).unwrap();
        assert_eq!(
            crate::interpreter::evaluate_resolved_zero_arg_i64(&old, "extraction.main", 1_000_000)
                .unwrap(),
            crate::interpreter::evaluate_resolved_zero_arg_i64(
                &current,
                "extraction.main",
                1_000_000
            )
            .unwrap()
        );
        let old_wire = crate::cache_codec::encode(&old);
        let current_wire = crate::cache_codec::encode(&current);
        assert_eq!(
            old_wire, current_wire,
            "complete wire outcome differs for case {case}"
        );
        if case == 1 {
            // The unchanged 32-level HIR fixture exceeds the codec's existing
            // nesting envelope; compare its exact refusal, retain full HIR and
            // runtime parity above/below, and never increase that envelope.
            let errors = current_wire.unwrap_err();
            assert!(!errors.is_empty());
            assert!(errors.iter().all(|error| error.code == "SPX-G305"));
        } else {
            assert!(
                current_wire.is_ok(),
                "admitted wire fixture {case}: {current_wire:?}"
            );
        }
        for (old, current) in old.functions.iter().zip(&current.functions) {
            assert_eq!(old.body, current.body);
            assert_eq!(old.loan_plan, current.loan_plan);
            assert_eq!(old.cleanup, current.cleanup);
            assert_eq!(old.cleanup_plan, current.cleanup_plan);
        }
    }
}
