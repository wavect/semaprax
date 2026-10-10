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
            let errors = value.unwrap_err();
            assert_eq!(errors[0].code, "SPX-G171");
            assert!(overflow);
            // The refused replacement allocates no keys. Its diagnostic still
            // appends the complete message to the same cumulative ledger.
            assert_eq!(
                debit,
                small_count * std::mem::size_of::<usize>() + errors[0].message.len()
            );
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

#[test]
fn generic_instances_reuse_one_scratch_and_refuse_one_short() {
    let program = crate::parse(
        r#"
module generic.scratch;
@id("generic.consume") fn consume(bytes: own Bytes) -> i64 { 0 }
@id("generic.keep") fn keep<T>(value: T, input: borrow Slice<u8>) -> T {
    let owned = bytes_copy(input);
    let _ = consume(owned);
    value
}
@id("generic.main") fn main(input: borrow Slice<u8>) -> i64 {
    let number = keep<i64>(1, input);
    if keep<bool>(true, input) { number } else { 0 }
}
"#,
        std::path::Path::new("generic-scratch.spx"),
    )
    .unwrap();
    let instances = crate::hir::resolve(&program).unwrap().function_instances;
    assert_eq!(instances.len(), 2);
    assert!(instances
        .iter()
        .all(|instance| !instance.function.loan_plan.loans.is_empty()));
    let programs = vec![program];
    let authored = super::super::index_authored(&programs).unwrap();
    let wires = instances
        .iter()
        .map(|instance| crate::cache_codec::encode(&instance.function.loan_plan).unwrap())
        .collect::<Vec<_>>();

    let (reused, overflow, exact) = bounded_output::with_limit_usage(usize::MAX, || {
        let mut scratch = RetentionScratch::default();
        super::super::owned_generics::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances.clone(),
            Some(&mut scratch),
        )
    });
    assert!(!overflow);
    let (retained, imported) = reused.unwrap();
    assert_eq!(retained.len(), 2);
    assert!(imported.is_empty());
    assert_eq!(
        retained
            .iter()
            .map(|instance| crate::cache_codec::encode(&instance.function.loan_plan).unwrap())
            .collect::<Vec<_>>(),
        wires
    );

    let (separate, overflow, separate_debit) = bounded_output::with_limit_usage(usize::MAX, || {
        for instance in &instances {
            let mut scratch = RetentionScratch::default();
            super::super::owned_generics::retain_module_instances(
                &programs[0],
                &programs,
                &authored,
                vec![instance.clone()],
                Some(&mut scratch),
            )?;
        }
        Ok::<_, Vec<Diagnostic>>(())
    });
    separate.unwrap();
    assert!(!overflow);
    assert!(
        exact < separate_debit,
        "one compact instance pass reuses its retained census scratch"
    );

    let (exact_result, overflow, debit) = bounded_output::with_limit_usage(exact, || {
        let mut scratch = RetentionScratch::default();
        super::super::owned_generics::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances.clone(),
            Some(&mut scratch),
        )
    });
    assert!(exact_result.is_ok());
    assert!(!overflow);
    assert_eq!(debit, exact);

    let (one_short, overflow, _) = bounded_output::with_limit_usage(exact - 1, || {
        let mut scratch = RetentionScratch::default();
        super::super::owned_generics::retain_module_instances(
            &programs[0],
            &programs,
            &authored,
            instances.clone(),
            Some(&mut scratch),
        )
    });
    assert_eq!(one_short.unwrap_err()[0].code, "SPX-G171");
    assert!(overflow);
}
