//! Copy discovery does not infer transfers from an argument's expression mode.
use super::*;

fn fixture() -> (ResolvedProgram, ResolvedFunction) {
    let source = r#"module copy.regions;
@id("copy.probe") fn probe(values: own Vec<i64>)->i64 {
    if vec_len<i64>(values)>0usize {1}else{0}
}
@id("copy.consume") fn consume(values: own Vec<i64>)->i64 {0}
@id("copy.own") fn own_call(values: own Vec<i64>)->i64 {consume(values)}
@id("main") fn main()->i64 {0}
"#;
    let parsed = crate::check(source, "copy-regions.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let function = program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "copy.probe")
        .unwrap()
        .clone();
    (program, function)
}

fn call(expression: &ResolvedExpr) -> &ResolvedExpr {
    let mut cursor = expression_children::ReplayChildCursor::new(expression);
    if matches!(expression.kind, ResolvedExprKind::Call { .. }) {
        return expression;
    }
    while let Some(child) = cursor.next() {
        if let Some(found) = find_call(child) {
            return found;
        }
    }
    panic!("authored call retained")
}
fn find_call(expression: &ResolvedExpr) -> Option<&ResolvedExpr> {
    if matches!(expression.kind, ResolvedExprKind::Call { .. }) {
        return Some(expression);
    }
    let mut cursor = expression_children::ReplayChildCursor::new(expression);
    while let Some(child) = cursor.next() {
        if let Some(found) = find_call(child) {
            return Some(found);
        }
    }
    None
}

#[test]
fn borrowed_vec_read_with_own_place_argument_is_copy_inert() {
    let (program, function) = fixture();
    let expression = call(&function.body);
    let ResolvedExprKind::Call { args, .. } = &expression.kind else {
        unreachable!()
    };
    assert_eq!(args[0].ownership, OwnershipMode::Own);
    assert!(matches!(args[0].kind, ResolvedExprKind::Place(_)));
    assert_eq!(
        call_mode(&program, expression, 0),
        Some(OwnershipMode::Borrow)
    );
    assert!(eligible(
        &program,
        &function,
        &function.body,
        &mut ReplayBudget::new()
    )
    .unwrap());
}

#[test]
fn owned_commit_unknown_callee_and_wrong_result_are_not_copy_regions() {
    let (program, function) = fixture();
    let consuming = program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "copy.own")
        .unwrap();
    assert!(!eligible(
        &program,
        consuming,
        &consuming.body,
        &mut ReplayBudget::new()
    )
    .unwrap());
    let mut forged = call(&function.body).clone();
    if let ResolvedExprKind::Call { callee, .. } = &mut forged.kind {
        *callee = DeclarationId::new("unknown.callee");
    }
    assert!(!eligible(&program, &function, &forged, &mut ReplayBudget::new()).unwrap());
    let mut wrong = call(&function.body).clone();
    wrong.ty = ResolvedType::Bool;
    assert!(!eligible(&program, &function, &wrong, &mut ReplayBudget::new()).unwrap());
}

#[test]
fn fixed_copy_discovery_scratch_is_reserved_before_use() {
    let (program, function) = fixture();
    let mut budget = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    let (result, observed) = crate::test_allocation_observer::measure(|| {
        eligible(&program, &function, &function.body, &mut budget)
    });
    assert!(result.unwrap());
    assert!(!observed.invalid && observed.allocated == 0 && observed.live == 0);
    let charged = budget.physical.as_ref().unwrap().charged() - before;
    let mut exact = ReplayBudget::with_physical(&function, before + charged).unwrap();
    assert!(eligible(&program, &function, &function.body, &mut exact).unwrap());
    let mut short = ReplayBudget::with_physical(&function, before + charged - 1).unwrap();
    assert!(eligible(&program, &function, &function.body, &mut short).is_err());
    assert!(short.physical.as_mut().unwrap().reserve(0).is_err());
}
