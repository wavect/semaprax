//! Physical scratch reuse does not change retained proof ownership or authority.
use super::*;
use crate::bounded_output;

fn count(function: &ResolvedFunction) -> usize {
    inventory_count(function).unwrap().unwrap()
}

#[test]
fn reused_census_allocates_once_and_preserves_legacy_receipts_and_proof() {
    let function = tests::function();
    let expected = retained_function_loan_bytes(&function).unwrap();
    let wire = crate::cache_codec::encode(&function.loan_plan).unwrap();
    let metadata = count(&function) * std::mem::size_of::<usize>();
    let (legacy, overflow, debit) = bounded_output::with_limit_usage(2 * metadata, || {
        [
            retained_function_loan_bytes(&function),
            retained_function_loan_bytes(&function),
        ]
    });
    assert!(legacy.into_iter().all(|value| value.unwrap() == expected));
    assert!(!overflow);
    assert_eq!(debit, 2 * metadata);
    let mut scratch = RetentionScratch::default();
    let (retained, overflow, debit) = bounded_output::with_limit_usage(metadata, || {
        let first = scratch.measure(&function)?;
        let backing = scratch.keys.as_ptr();
        for _ in 0..200 {
            assert_eq!(scratch.measure(&function)?, first);
            assert_eq!(
                scratch.keys.as_ptr(),
                backing,
                "reuse the actual allocation"
            );
        }
        Ok::<_, Vec<Diagnostic>>(first)
    });
    assert_eq!(retained.unwrap(), expected);
    assert!(!overflow);
    assert_eq!(debit, metadata);
    assert_eq!(
        crate::cache_codec::encode(&function.loan_plan).unwrap(),
        wire
    );
}

#[test]
fn scratch_growth_reserves_the_whole_new_carrier_and_refuses_one_short() {
    let small = tests::function();
    let mut larger = small.clone();
    let body = larger.body;
    larger.body = ResolvedExpr {
        id: body.id.clone(),
        ty: body.ty.clone(),
        ownership: body.ownership,
        span: body.span,
        kind: E::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(body),
        },
    };
    let expected = retained_function_loan_bytes(&larger).unwrap();
    let small_count = count(&small);
    let large_count = count(&larger);
    assert_eq!(large_count, small_count + 1);
    let bytes = (small_count + large_count) * std::mem::size_of::<usize>();
    for limit in [bytes, bytes - 1] {
        let mut scratch = RetentionScratch::default();
        let (value, overflow, debit) = bounded_output::with_limit_usage(limit, || {
            scratch.measure(&small)?;
            scratch.measure(&larger)
        });
        if limit == bytes {
            assert_eq!(value.unwrap(), expected);
            assert!(!overflow);
            assert_eq!(debit, bytes);
        } else {
            assert_eq!(value.unwrap_err()[0].code, "SPX-G171");
            assert!(overflow);
            assert_eq!(debit, small_count * std::mem::size_of::<usize>());
        }
    }
}

#[test]
fn prior_function_keys_never_exclude_independent_or_foreign_proof_storage() {
    let first = tests::function();
    let mut next = tests::function();
    let foreign = first.loan_plan.loans[0].site.clone();
    let key = foreign.shared_allocation_key().unwrap();
    next.loan_plan.loans[0].site = foreign;
    let expected = retained_function_loan_bytes(&next).unwrap();
    let wire = crate::cache_codec::encode(&next.loan_plan).unwrap();
    let mut scratch = RetentionScratch::default();
    scratch.measure(&first).unwrap();
    assert!(scratch.keys.binary_search(&key).is_ok());
    assert_eq!(scratch.measure(&next).unwrap(), expected);
    assert!(scratch.keys.binary_search(&key).is_err());
    assert_eq!(crate::cache_codec::encode(&next.loan_plan).unwrap(), wire);
}

#[test]
fn scratch_uncertain_inventory_keeps_full_charge_and_missing_backing_refuses() {
    let mut function = tests::function();
    let full = retained_loan_plan_bytes(&function.loan_plan).unwrap();
    let mut scratch = RetentionScratch::default();
    scratch.measure(&function).unwrap();
    for _ in 0..crate::cache_codec::MAX_DEPTH {
        let body = function.body;
        function.body = ResolvedExpr {
            id: body.id.clone(),
            ty: body.ty.clone(),
            ownership: body.ownership,
            span: body.span,
            kind: E::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: Box::new(body),
            },
        };
    }
    assert_eq!(scratch.measure(&function).unwrap(), full);
    assert!(scratch.keys.is_empty());
    let (identity, overflow, _) = bounded_output::with_limit_usage(0, || {
        ExpressionId::from_owned("missing-backing".to_owned())
    });
    assert!(overflow);
    function.body.id = identity;
    assert_eq!(scratch.measure(&function).unwrap_err()[0].code, "SPX-G171");
    assert!(scratch.keys.is_empty());
}
