use super::*;
use crate::hir::{DeclarationId, FunctionExecutionId};
use std::path::Path;

fn point(expression: ExpressionId) -> LoanProgramPoint {
    LoanProgramPoint {
        expression,
        phase: LoanPointPhase::Before,
    }
}

fn endpoint_plan(expressions: Vec<ExpressionId>) -> LoanPlan {
    LoanPlan {
        schema: LOAN_PLAN_SCHEMA_V1,
        loans: Vec::new(),
        endpoints: expressions
            .into_iter()
            .map(|expression| LoanEndpoint {
                point: point(expression),
                live_before: Vec::new(),
                starts: Vec::new(),
                kills: Vec::new(),
                live_after: Vec::new(),
            })
            .collect(),
        edges: Vec::new(),
    }
}

fn execution() -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(DeclarationId::new("identity.loan"))
}

#[test]
fn loan_capacity_counts_shared_identity_backing_once_but_independent_equal_bytes_twice() {
    let owner = execution();
    let first = ExpressionId::new(&owner, "endpoint");
    let shared = endpoint_plan(vec![first.clone(), first.clone()]);
    let independent = endpoint_plan(vec![
        ExpressionId::new(&owner, "endpoint"),
        ExpressionId::new(&owner, "endpoint"),
    ]);

    let shared_bytes = owned_capacity_bytes(&shared).expect("shared plan capacity");
    let independent_bytes = owned_capacity_bytes(&independent).expect("independent plan capacity");
    assert_eq!(
        shared.endpoints[0].point.expression,
        shared.endpoints[1].point.expression
    );
    assert_eq!(
        shared.endpoints[0].point.expression.shared_allocation_key(),
        shared.endpoints[1].point.expression.shared_allocation_key()
    );
    assert_ne!(
        independent.endpoints[0]
            .point
            .expression
            .shared_allocation_key(),
        independent.endpoints[1]
            .point
            .expression
            .shared_allocation_key()
    );
    assert_eq!(
        independent_bytes - shared_bytes,
        first.shared_allocation_bytes().expect("identity backing")
    );
}

#[test]
fn loan_capacity_metadata_reservation_is_exact_and_refuses_before_allocation() {
    let owner = execution();
    let expression = ExpressionId::new(&owner, "shared-endpoint");
    let plan = endpoint_plan(vec![expression.clone(), expression]);
    let metadata_bytes = 2 * std::mem::size_of::<&ExpressionId>();

    let (capacity, overflowed, used) =
        crate::bounded_output::with_limit_usage(metadata_bytes, || owned_capacity_bytes(&plan));
    assert!(capacity.is_some());
    assert!(!overflowed);
    assert_eq!(used, metadata_bytes);

    let (capacity, overflowed, used) =
        crate::bounded_output::with_limit_usage(metadata_bytes - 1, || owned_capacity_bytes(&plan));
    assert_eq!(capacity, None);
    assert!(overflowed);
    assert_eq!(used, 0);
}

#[test]
fn shared_expression_endpoints_replay_and_forged_endpoint_still_fails_closed() {
    let source = r#"
module test.shared_loan_identity;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("loan.run")
fn run(input: borrow Slice<u8>, outer: bool, inner: bool) -> i64 {
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = if outer {
        if inner { byte_len(view) > 0usize && byte_len(view) < 9usize } else { false }
    } else { false };
    take(owned)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("shared-loan-identity.spx")).unwrap();
    assert!(crate::verify::verify(&ast).is_empty());
    let mut program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.run")
        .expect("loan-bearing function");
    let function = &program.functions[index];
    assert_eq!(build_plan(&program, function).unwrap(), function.loan_plan);

    let endpoint = program.functions[index]
        .loan_plan
        .endpoints
        .iter()
        .position(|endpoint| endpoint.point.expression.shared_allocation_key().is_some())
        .expect("endpoint with identity backing");
    let forged = ExpressionId::new(&execution(), "forged-endpoint");
    program.functions[index].loan_plan.endpoints[endpoint]
        .point
        .expression = forged;
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
}
