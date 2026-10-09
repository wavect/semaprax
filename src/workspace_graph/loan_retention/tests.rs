use super::*;
use crate::bounded_output;
use std::collections::BTreeMap;

fn function() -> ResolvedFunction {
    let source = r#"
module test.retained_loan_union;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("loan.run")
fn run(input: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let length = byte_len(view);
    take(owned)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, "retained-loan-union.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
        .functions
        .into_iter()
        .find(|function| function.id.as_str() == "loan.run")
        .unwrap()
}

fn hir_keys(function: &ResolvedFunction) -> BTreeMap<usize, usize> {
    let mut keys = BTreeMap::new();
    visit_function(function, &mut |identity| {
        keys.insert(
            identity.shared_allocation_key().unwrap(),
            identity.shared_allocation_bytes().unwrap(),
        );
        Ok(())
    })
    .unwrap_or_else(|_| panic!("fixture identity inventory"));
    keys
}

#[test]
fn physical_union_counts_body_backing_once_and_preserves_exact_proof_bytes() {
    let function = function();
    assert!(!function.loan_plan.loans.is_empty());
    let wire = crate::cache_codec::encode(&function.loan_plan).unwrap();
    let hir = hir_keys(&function);
    let mut shared = BTreeMap::new();
    let mut insert = |identity: &ExpressionId| {
        let key = identity.shared_allocation_key().unwrap();
        if hir.contains_key(&key) {
            shared.insert(key, identity.shared_allocation_bytes().unwrap());
        }
    };
    for loan in &function.loan_plan.loans {
        insert(&loan.site);
        insert(&loan.start.expression);
        for end in &loan.ends {
            insert(&end.expression);
        }
    }
    for endpoint in &function.loan_plan.endpoints {
        insert(&endpoint.point.expression);
    }
    assert!(!shared.is_empty());
    let full = retained_loan_plan_bytes(&function.loan_plan).unwrap();
    let union = retained_function_loan_bytes(&function).unwrap();
    assert_eq!(full - union, shared.values().sum::<usize>());
    assert_eq!(
        crate::cache_codec::encode(&function.loan_plan).unwrap(),
        wire
    );
}

#[test]
fn physical_union_inventory_admits_exact_bytes_and_refuses_one_short() {
    let function = function();
    let mut hir_count = 0usize;
    visit_function(&function, &mut |_| {
        hir_count += 1;
        Ok(())
    })
    .unwrap_or_else(|_| panic!("fixture identity inventory"));
    let proof_count = function.loan_plan.endpoints.len()
        + function
            .loan_plan
            .loans
            .iter()
            .map(|loan| 2 + loan.ends.len())
            .sum::<usize>();
    let metadata = (hir_count + proof_count) * std::mem::size_of::<usize>();
    let expected = retained_function_loan_bytes(&function).unwrap();
    let (result, overflow, used) =
        bounded_output::with_limit_usage(metadata, || retained_function_loan_bytes(&function));
    assert_eq!(result.unwrap(), expected);
    assert!(!overflow);
    assert_eq!(used, metadata);
    let wire = crate::cache_codec::encode(&function.loan_plan).unwrap();
    let (result, overflow, _) =
        bounded_output::with_limit_usage(metadata - 1, || retained_function_loan_bytes(&function));
    assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
    assert!(overflow);
    assert_eq!(
        crate::cache_codec::encode(&function.loan_plan).unwrap(),
        wire
    );
}

#[test]
fn physical_union_keeps_equal_text_with_distinct_backing_fully_charged() {
    let mut function = function();
    let hir = hir_keys(&function);
    let index = function
        .loan_plan
        .endpoints
        .iter()
        .position(|endpoint| {
            hir.contains_key(&endpoint.point.expression.shared_allocation_key().unwrap())
        })
        .unwrap();
    let before = retained_function_loan_bytes(&function).unwrap();
    let original = function.loan_plan.endpoints[index].point.expression.clone();
    let independent = ExpressionId::from_owned(original.as_str().to_owned());
    assert_eq!(original, independent);
    assert_ne!(
        original.shared_allocation_key(),
        independent.shared_allocation_key()
    );
    let extra = independent.shared_allocation_bytes().unwrap();
    function.loan_plan.endpoints[index].point.expression = independent;
    assert_eq!(
        retained_function_loan_bytes(&function).unwrap(),
        before + extra
    );
}

#[test]
fn physical_union_refuses_missing_hir_or_loan_backing() {
    let missing = || {
        let (identity, overflow, used) =
            bounded_output::with_limit_usage(0, || ExpressionId::from_owned(String::new()));
        assert!(overflow);
        assert_eq!(used, 0);
        identity
    };
    let mut body = function();
    body.body.id = missing();
    assert_eq!(
        retained_function_loan_bytes(&body).unwrap_err()[0].code,
        "SPX-G171"
    );
    let mut proof = function();
    proof.loan_plan.endpoints[0].point.expression = missing();
    assert_eq!(
        retained_function_loan_bytes(&proof).unwrap_err()[0].code,
        "SPX-G171"
    );
}

#[test]
fn physical_union_uncertain_inventory_keeps_the_complete_proof_charge() {
    let mut function = function();
    let full = retained_loan_plan_bytes(&function.loan_plan).unwrap();
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
    assert_eq!(retained_function_loan_bytes(&function).unwrap(), full);
}
