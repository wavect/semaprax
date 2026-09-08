use super::*;

const SOURCE: &str = r#"
module test.closures;
@id("closure.helper") fn helper(value:i64)->i64{value+1}
@id("closure.main") fn main()->i64{
    let mut left=2;
    let right=3;
    let callback=fn(value:i64)->i64{value+left+right};
    left=100;
    callback(37)
}
"#;

fn resolved() -> ResolvedProgram {
    let checked = crate::check(SOURCE, "closures.spx").unwrap();
    crate::hir::resolve(&checked).unwrap()
}

fn creation(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "closure.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[2] else {
        panic!("closure binding")
    };
    value
}

fn rejects(program: &ResolvedProgram) {
    assert_eq!(crate::hir::validate(program).unwrap_err().code, "SPX-H006");
}

#[test]
fn closures_hir_replays_snapshot_inventory_and_private_body() {
    let program = resolved();
    crate::hir::validate(&program).unwrap();
    let sites = inventory(&program);
    assert_eq!(sites.len(), 1);
    let ResolvedExprKind::Closure {
        captures,
        parameters,
        ..
    } = &sites[0].kind
    else {
        panic!("closure")
    };
    assert_eq!(captures.len(), 2);
    assert_eq!(parameters.len(), 1);
    let product = closure_function(&program, sites[0]).unwrap();
    assert_eq!(product.id, closure_id(&sites[0].id));
    assert_eq!(product.params.len(), 3);
    assert_eq!(product.params[0].id, captures[0].binding.id);
    assert_eq!(product.params[2].id, parameters[0].id);
}

#[test]
fn closures_hir_rejects_effectful_capture_creation() {
    let mut program = resolved();
    let ResolvedExprKind::Closure { captures, .. } = &mut creation(&mut program).kind else {
        panic!("closure")
    };
    captures[0].value.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new("closure.helper"),
        instance: None,
        type_arguments: Vec::new(),
        args: vec![captures[0].value.clone()],
    };
    rejects(&program);
}

#[test]
fn closures_hir_rejects_capture_type_and_order_forgery() {
    let mut program = resolved();
    let ResolvedExprKind::Closure { captures, .. } = &mut creation(&mut program).kind else {
        panic!("closure")
    };
    captures[0].binding.ty = ResolvedType::Bool;
    rejects(&program);

    let mut program = resolved();
    let ResolvedExprKind::Closure { captures, .. } = &mut creation(&mut program).kind else {
        panic!("closure")
    };
    // Preserve the canonical private slots while reversing only the snapshot
    // roots. Independent validation must enforce the ordered outer inventory.
    let first = captures[0].value.kind.clone();
    captures[0].value.kind = captures[1].value.kind.clone();
    captures[1].value.kind = first;
    rejects(&program);
}

#[test]
fn closures_hir_rejects_private_binding_and_body_identity_forgery() {
    let mut program = resolved();
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        ..
    } = &mut creation(&mut program).kind
    else {
        panic!("closure")
    };
    parameters[0].id = captures[0].binding.id.clone();
    rejects(&program);

    let mut program = resolved();
    let expression = creation(&mut program);
    let outer_id = expression.id.clone();
    let ResolvedExprKind::Closure { body, .. } = &mut expression.kind else {
        panic!("closure")
    };
    body.id = outer_id;
    rejects(&program);
}

#[test]
fn closures_hir_rejects_authored_private_identity_collision() {
    let program = resolved();
    let derived = closure_id(&inventory(&program)[0].id);
    let colliding = SOURCE.replace("closure.helper", derived.as_str());
    let checked = crate::check(&colliding, "closures.spx").unwrap();
    let diagnostics = crate::hir::resolve(&checked).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "SPX-H006" && d.message.contains("collid")));
}
