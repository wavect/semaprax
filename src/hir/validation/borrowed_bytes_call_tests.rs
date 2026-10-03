use super::*;

const SOURCE: &str = r#"
module test.borrowed_bytes_call_hir;
@id("packet.type") record Packet {
  @id("packet.payload") payload: Bytes,
  @id("packet.sibling") sibling: Bytes,
}
@id("packet.inspect") fn inspect(value: borrow Bytes) -> usize {
  byte_len(bytes_as_slice(value))
}
@id("packet.caller") fn caller(packet: own Packet) -> usize { inspect(packet.payload) }
@id("app.main") fn main() -> i64 { 0 }
"#;

fn fixture() -> ResolvedProgram {
    let parsed = crate::parse(
        SOURCE,
        std::path::Path::new("borrowed-bytes-call-hir-v1.spx"),
    )
    .expect("fixture parses");
    let program = crate::hir::resolve(&parsed).expect("fixture resolves");
    crate::hir::validate(&program).expect("fixture validates");
    program
}

fn caller(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.caller")
        .expect("caller function")
}

fn caller_mut(program: &mut ResolvedProgram) -> &mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "packet.caller")
        .expect("caller function")
}

fn call_argument_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("caller body remains a block")
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        panic!("caller tail remains a call")
    };
    &mut args[0]
}

#[test]
fn resolver_preserves_borrowed_bytes_and_attaches_projected_call_loan_identity() {
    let program = fixture();
    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.inspect")
        .expect("inspect function");
    assert_eq!(inspect.params[0].ownership, OwnershipMode::Borrow);

    let loan = caller(&program)
        .loan_plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::BorrowedCall { argument: 0 })
        .expect("borrowed call loan");
    assert_eq!(
        loan.origin.projections,
        [PlaceProjection::Field(DeclarationId::new("packet.payload"))]
    );
    assert_eq!(loan.start.phase, LoanPointPhase::Before);
    assert!(!loan.end_edges.is_empty());
}

#[test]
fn hostile_call_place_and_attached_loan_field_identity_fail_closed() {
    let mut forged_hir = fixture();
    let argument = call_argument_mut(caller_mut(&mut forged_hir));
    let ResolvedExprKind::Place(place) = &mut argument.kind else {
        panic!("borrowed argument remains a place")
    };
    place.projections = vec![PlaceProjection::Field(DeclarationId::new("packet.sibling"))];
    let diagnostic = crate::hir::validate(&forged_hir).expect_err("HIR drift must fail");
    assert_eq!(diagnostic.code, "SPX-H006");

    let mut forged_plan = fixture();
    let loan = caller_mut(&mut forged_plan)
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| loan.cause == LoanCause::BorrowedCall { argument: 0 })
        .expect("borrowed call loan");
    loan.origin.projections = vec![PlaceProjection::Field(DeclarationId::new("packet.sibling"))];
    let diagnostic = crate::hir::validate(&forged_plan).expect_err("loan identity drift must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
}

#[test]
fn hostile_non_place_borrowed_bytes_argument_is_h006() {
    let mut program = fixture();
    let argument = call_argument_mut(caller_mut(&mut program));
    argument.kind = ResolvedExprKind::Int(0);
    let diagnostic = crate::hir::validate(&program).expect_err("non-place must fail");
    assert_eq!(diagnostic.code, "SPX-H006");
}
