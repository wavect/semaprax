//! Physical scratch reuse does not change retained proof ownership or authority.
use super::*;
use crate::bounded_output;

fn count(function: &ResolvedFunction) -> usize {
    candidate_count(function).unwrap().unwrap()
}

fn metadata(function: &ResolvedFunction) -> usize {
    count(function) * (std::mem::size_of::<usize>() + std::mem::size_of::<u8>())
}

fn larger_proof(function: &ResolvedFunction) -> ResolvedFunction {
    let mut larger = function.clone();
    let body = larger.body;
    let id = ExpressionId::from_owned(format!("{}-larger", body.id.as_str()));
    let mut endpoint = larger.loan_plan.endpoints[0].clone();
    endpoint.point.expression = id.clone();
    larger.loan_plan.endpoints.push(endpoint);
    larger.body = ResolvedExpr {
        id,
        ty: body.ty.clone(),
        ownership: body.ownership,
        span: body.span,
        kind: E::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(body),
        },
    };
    larger
}

#[test]
fn reused_census_allocates_once_and_preserves_legacy_receipts_and_proof() {
    let function = tests::function();
    let expected = retained_function_loan_bytes(&function).unwrap();
    let wire = crate::cache_codec::encode(&function.loan_plan).unwrap();
    let legacy_metadata =
        inventory_count(&function).unwrap().unwrap() * std::mem::size_of::<usize>();
    let metadata = metadata(&function);
    let (legacy, overflow, debit) = bounded_output::with_limit_usage(2 * legacy_metadata, || {
        [
            retained_function_loan_bytes(&function),
            retained_function_loan_bytes(&function),
        ]
    });
    assert!(legacy.into_iter().all(|value| value.unwrap() == expected));
    assert!(!overflow);
    assert_eq!(debit, 2 * legacy_metadata);
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
    let larger = larger_proof(&small);
    let expected = retained_function_loan_bytes(&larger).unwrap();
    let small_count = count(&small);
    let large_count = count(&larger);
    assert_eq!(large_count, small_count + 1);
    let bytes = metadata(&small) + metadata(&larger);
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
            assert_eq!(debit, metadata(&small) + errors[0].message.len());
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
fn prepared_module_census_avoids_intermediate_allocations_at_exact_limits() {
    let small = tests::function();
    let large = larger_proof(&small);
    let expected = [
        retained_function_loan_bytes(&small).unwrap(),
        retained_function_loan_bytes(&large).unwrap(),
    ];
    let wires =
        [&small, &large].map(|function| crate::cache_codec::encode(&function.loan_plan).unwrap());
    let maximum = metadata(&large);
    let mut scratch = RetentionScratch::default();
    let (result, overflow, debit) = bounded_output::with_limit_usage(maximum, || {
        scratch.prepare([&small, &large].into_iter())?;
        let pointer = scratch.keys.as_ptr();
        for (function, expected) in [&small, &large].into_iter().zip(expected) {
            assert_eq!(scratch.measure(function)?, expected);
            assert_eq!(scratch.keys.as_ptr(), pointer);
        }
        Ok::<_, Vec<Diagnostic>>(())
    });
    result.unwrap();
    assert!(!overflow);
    assert_eq!(debit, maximum);
    assert_eq!(
        [&small, &large]
            .map(|function| { crate::cache_codec::encode(&function.loan_plan).unwrap() }),
        wires
    );
    let mut scratch = RetentionScratch::default();
    let (refused, overflow, _) = bounded_output::with_limit_usage(maximum - 1, || {
        scratch.prepare([&small, &large].into_iter())
    });
    assert_eq!(refused.unwrap_err()[0].code, "SPX-G171");
    assert!(overflow);
    assert_eq!(scratch.keys.capacity(), 0, "refuse before allocation");
    let mut scratch = RetentionScratch::default();
    let (legacy, overflow, debit) = bounded_output::with_limit_usage(usize::MAX, || {
        scratch.measure(&small)?;
        scratch.measure(&large)
    });
    assert_eq!(legacy.unwrap(), expected[1]);
    assert!(!overflow);
    assert_eq!(debit, maximum + metadata(&small));
}

#[test]
fn generic_instances_reuse_one_scratch_and_refuse_one_short() {
    let program = crate::parse(
        r#"
module generic.scratch;
@id("generic.consume") fn consume(text: own string) -> i64 { 0 }
@id("generic.keep") fn keep<T>(value: T) -> T {
    let owned = "scratch";
    let view = string_as_str(owned);
    let observed_length = string_len(view);
    let consumed_status = consume(owned);
    value
}
@id("generic.main") fn main() -> i64 {
    let number = keep<i64>(1);
    if keep<bool>(true) { number } else { 0 }
}
"#,
        std::path::Path::new("generic-scratch.spx"),
    )
    .unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let instances = resolved.function_instances;
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

#[test]
fn candidate_intersection_does_not_allocate_for_unreferenced_hir_identities() {
    let function = tests::function();
    let mut wide = function.clone();
    let E::Block { statements, .. } = &mut wide.body.kind else {
        panic!("body");
    };
    let statement = statements[0].clone();
    for index in 0..2500 {
        let mut extra = statement.clone();
        let crate::hir::ResolvedStatement::Let { value, .. } = &mut extra else {
            panic!("fixture first let");
        };
        value.id = ExpressionId::from_owned(format!("unreferenced-wide-{index}"));
        statements.push(extra);
    }
    let mut full_hir_count = 0;
    visit_function(&wide, &mut |_| {
        full_hir_count += 1;
        Ok(())
    })
    .unwrap_or_else(|_| panic!("wide shallow fixture"));
    assert!(full_hir_count > crate::loan_plan::MAX_LOAN_ENDPOINTS_V1);
    assert!(
        inventory_count(&wide).unwrap().is_none(),
        "legacy bounded receipt still falls back"
    );
    let candidate_count = count(&wide);
    assert!(full_hir_count > candidate_count * 2);
    assert_eq!(candidate_count, count(&function));
    let expected = inventory_bytes(&wide, &mut Vec::new()).unwrap();
    let proof = crate::cache_codec::encode(&wide.loan_plan).unwrap();
    let mut scratch = RetentionScratch::default();
    let (result, overflow, debit) =
        bounded_output::with_limit_usage(metadata(&wide), || scratch.measure(&wide));
    assert_eq!(result.unwrap(), expected);
    assert!(!overflow);
    assert_eq!(debit, metadata(&wide));
    assert_eq!(scratch.keys.capacity(), candidate_count);
    assert_eq!(scratch.matched.capacity(), candidate_count);
    assert_eq!(crate::cache_codec::encode(&wide.loan_plan).unwrap(), proof);
    let mut scratch = RetentionScratch::default();
    let (result, overflow, _) =
        bounded_output::with_limit_usage(metadata(&wide) - 1, || scratch.measure(&wide));
    assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
    assert!(overflow);
    assert_eq!(scratch.keys.capacity(), 0);
    assert_eq!(scratch.matched.capacity(), 0);
}

#[test]
fn candidate_intersection_checks_unmatched_hir_and_proof_backing_before_allocation() {
    for corrupt_proof in [false, true] {
        let mut function = tests::function();
        let (missing, overflow, _) = bounded_output::with_limit_usage(0, || {
            ExpressionId::from_owned("missing-unmatched-backing".to_owned())
        });
        assert!(overflow);
        if corrupt_proof {
            function.loan_plan.loans[0].site = missing;
        } else {
            function.body.id = missing;
        }
        let mut scratch = RetentionScratch::default();
        let (result, overflow, debit) =
            bounded_output::with_limit_usage(usize::MAX, || scratch.measure(&function));
        let errors = result.unwrap_err();
        assert_eq!(errors[0].code, "SPX-G171");
        assert!(!overflow);
        assert_eq!(
            debit,
            errors[0].message.len() + errors[0].help.as_ref().map_or(0, String::len),
            "the complete refusal message and stage help are allocated before any scratch"
        );
        assert_eq!(scratch.keys.capacity(), 0);
        assert_eq!(scratch.matched.capacity(), 0);
    }
}

#[test]
fn candidate_intersection_keeps_equal_text_distinct_full_capacity_and_exact_unmatched_debit() {
    let mut function = tests::function();
    let mut scratch = RetentionScratch::default();
    let before = scratch.measure(&function).unwrap();
    let index = function
        .loan_plan
        .endpoints
        .iter()
        .position(|endpoint| {
            scratch
                .keys
                .binary_search(&endpoint.point.expression.shared_allocation_key().unwrap())
                .is_ok()
        })
        .unwrap();
    let original = function.loan_plan.endpoints[index].point.expression.clone();
    let mut text = String::with_capacity(original.as_str().len() + 4096);
    text.push_str(original.as_str());
    let independent = ExpressionId::from_untrimmed_backing_for_test(text);
    assert_eq!(independent, original);
    assert_ne!(
        independent.shared_allocation_key(),
        original.shared_allocation_key()
    );
    let independent_key = independent.shared_allocation_key().unwrap();
    let extra = independent.shared_allocation_bytes().unwrap();
    assert!(
        extra > original.as_str().len() + 4096,
        "include retained carrier and complete String capacity"
    );
    function.loan_plan.endpoints[index].point.expression = independent;
    let wire = crate::cache_codec::encode(&function.loan_plan).unwrap();
    assert_eq!(
        retained_function_loan_bytes(&function).unwrap(),
        before + extra
    );
    let exact = metadata(&function) + std::mem::size_of::<&ExpressionId>();
    for limit in [exact, exact - 1] {
        let mut scratch = RetentionScratch::default();
        let (result, overflow, debit) =
            bounded_output::with_limit_usage(limit, || scratch.measure(&function));
        if limit == exact {
            assert_eq!(result.unwrap(), before + extra);
            assert!(!overflow);
            assert_eq!(debit, exact);
            assert!(scratch.keys.binary_search(&independent_key).is_err());
        } else {
            assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
            assert!(overflow);
            assert!(debit >= metadata(&function));
        }
        assert_eq!(
            crate::cache_codec::encode(&function.loan_plan).unwrap(),
            wire
        );
    }
}


#[test]
fn distinct_candidate_prefilter_keeps_collisions_exact_without_allocating() {
    for keys in [&[0usize, 4181, 0, 4181][..], &[1usize, 2, 3, 1, 4, 2][..]] {
        let expected = keys.iter().copied().collect::<std::collections::BTreeSet<_>>().len();
        let (actual, overflow, debit) = bounded_output::with_limit_usage(0, || {
            distinct_keys(|| keys.iter().copied())
        });
        assert_eq!(actual.unwrap().unwrap().0, expected);
        assert!(!overflow);
        assert_eq!(debit, 0);
    }
    // These two distinct keys collide in the fixed bitmap. Exact comparison
    // must retain both; a bitmap hit must never grant physical sharing.
    let bucket = |key: usize| ((key as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 52) as usize;
    assert_eq!(bucket(0), bucket(4181));
    let (distinct, comparisons) = distinct_keys(|| [0usize, 4181].into_iter()).unwrap().unwrap();
    assert_eq!((distinct, comparisons), (2, 1));
    let (distinct, comparisons) = distinct_keys(|| 0usize..512).unwrap().unwrap();
    assert_eq!(distinct, 512);
    assert_eq!(comparisons, 0, "a definitely new bucket needs no prior scan");
}
