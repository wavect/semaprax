use std::path::Path;

use super::*;

const FIXTURE: &str = r#"
module test.loan_plan_cfg;
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

fn fixture() -> ResolvedProgram {
    let ast = crate::parse(FIXTURE, Path::new("loan-plan-cfg.spx")).unwrap();
    assert!(crate::verify::verify(&ast).is_empty());
    crate::hir::resolve(&ast).unwrap()
}

fn run_mutation(name: &str, mut mutate: impl FnMut(&mut LoanPlan, &ResolvedFunction)) {
    let mut program = fixture();
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.run")
        .unwrap();
    let snapshot = program.functions[index].clone();
    mutate(&mut program.functions[index].loan_plan, &snapshot);
    assert!(crate::hir::validate_core(&program).is_ok());
    let error = match crate::hir::validate(&program) {
        Err(error) => error,
        Ok(()) => panic!("mutation `{name}` unexpectedly validated"),
    };
    assert_eq!(error.code, "SPX-H006");
}

#[test]
fn nested_and_lazy_paths_have_edge_qualified_terminations() {
    let program = fixture();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.run")
        .unwrap();
    let view = function
        .loan_plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::SliceView && loan.parent.is_none())
        .unwrap();
    assert!(
        view.end_edges.len() >= 3,
        "nested/lazy exits are edge-qualified"
    );
    assert!(view
        .end_edges
        .iter()
        .all(|edge| (*edge as usize) < function.loan_plan.edges.len()));
}

#[test]
fn every_attached_plan_surface_is_replayed_exactly() {
    run_mutation("schema", |plan, _| plan.schema = "forged");
    run_mutation("id", |plan, _| plan.loans[0].id = LoanId(255));
    run_mutation("site", |plan, function| {
        plan.loans[0].site = function.body.id.clone()
    });
    run_mutation("origin", |plan, function| {
        plan.loans[0].origin.root = function.params[0].id.clone()
    });
    run_mutation("parent", |plan, _| {
        plan.loans[0].parent = Some(plan.loans[0].id)
    });
    run_mutation("start", |plan, _| {
        plan.loans[0].start.phase = LoanPointPhase::After
    });
    run_mutation("ends", |plan, _| plan.loans[0].ends.clear());
    run_mutation("end_edges", |plan, _| plan.loans[0].end_edges.clear());
    run_mutation("endpoint starts", |plan, _| {
        plan.endpoints
            .iter_mut()
            .find(|endpoint| !endpoint.starts.is_empty())
            .unwrap()
            .starts
            .clear()
    });
    run_mutation("endpoint live", |plan, _| {
        plan.endpoints[0].live_after.push(LoanId(255))
    });
    run_mutation("edge live", |plan, _| {
        plan.edges
            .iter_mut()
            .find(|edge| !edge.live.is_empty())
            .unwrap()
            .live
            .clear()
    });
    run_mutation("omission", |plan, _| {
        plan.loans.pop();
    });
}

#[test]
fn own_match_payload_loans_are_canonical_and_cannot_be_omitted() {
    let source = r#"
module test.loan_plan_owned_match;

@id("loan.consume")
fn consume(value: own Bytes) -> i64 { 1 }

@id("loan.inspect")
fn inspect(input: own Option<Bytes>) -> i64 {
match own input {
    Option::None {} => 0,
    Option::Some { value: bytes } => {
        let observed = if byte_len(bytes_as_slice(bytes)) == 1usize { 1 } else { 0 };
        consume(bytes) + observed
    },
}
}

@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, Path::new("loan-plan-owned-match.spx")).unwrap();
    assert!(crate::verify::verify(&ast).is_empty());
    let mut program = crate::hir::resolve(&ast).expect("owned match payload loan resolves");
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.inspect")
        .unwrap();
    let function = &program.functions[index];
    let match_expression = match &function.body.kind {
        ResolvedExprKind::Match { .. } => &function.body,
        ResolvedExprKind::Block { tail, .. } => tail,
        _ => &function.body,
    };
    let ResolvedExprKind::Match { arms, .. } = &match_expression.kind else {
        panic!("fixture body remains a match")
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
        panic!("Some arm retains its variant payload")
    };
    let owned_payload = &fields[0].binding;
    assert_eq!(owned_payload.ownership, OwnershipMode::Own);
    assert!(function
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.origin.root == owned_payload.id));

    program.functions[index].loan_plan = LoanPlan::empty_v1();
    assert!(crate::hir::validate_core(&program).is_ok());
    let error = crate::hir::validate(&program)
        .expect_err("an owned match payload loan cannot be omitted from hostile HIR");
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("forged shared-loan evidence"));
}

#[test]
fn option_try_residual_edge_terminates_a_normal_path_loan() {
    // Source admission currently rejects `?` with a live owned byte
    // carrier. Exercise the defensive HIR planner directly so that future
    // admission cannot inherit a CFG that lacks the residual-return path.
    let program = fixture();
    let mut function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.run")
        .unwrap()
        .clone();
    let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let operand_id = {
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("fixture body remains a block")
        };
        let ResolvedStatement::Let {
            value: observed, ..
        } = &mut statements[2]
        else {
            panic!("third statement remains the observed branch")
        };
        let ResolvedExprKind::If { condition, .. } = &mut observed.kind else {
            panic!("observed value remains an if")
        };
        let mut operand = (**condition).clone();
        operand.id = ExpressionId::new(&execution, "body.s2.value.condition.operand");
        let operand_id = operand.id.clone();
        condition.kind = ResolvedExprKind::TryOption {
            operand: Box::new(operand),
            option: crate::hir::DeclarationId::new("prelude.option"),
            some_case: crate::hir::DeclarationId::new("prelude.option.some"),
            some_field: crate::hir::DeclarationId::new("prelude.option.some.value"),
            none_case: crate::hir::DeclarationId::new("prelude.option.none"),
            residual_type: ResolvedType::Bool,
        };
        operand_id
    };

    let plan = build_plan(&program, &function).expect("defensive TryOption CFG builds");
    let from = plan
        .endpoints
        .iter()
        .position(|endpoint| {
            endpoint.point.expression == operand_id && endpoint.point.phase == LoanPointPhase::After
        })
        .unwrap() as u16;
    let to = plan
        .endpoints
        .iter()
        .position(|endpoint| {
            endpoint.point.expression == function.body.id
                && endpoint.point.phase == LoanPointPhase::After
        })
        .unwrap() as u16;
    let residual_edge =
        plan.edges
            .iter()
            .position(|edge| edge.from == from && edge.to == to)
            .expect("TryOption must retain an immediate residual-return edge") as u16;
    let loan = plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::SliceView && loan.parent.is_none())
        .unwrap();
    assert!(loan.end_edges.contains(&residual_edge));
}

#[test]
fn loan_limit_accepts_256_and_rejects_257() {
    fn source(count: usize) -> String {
        let mut source = String::from(
            "module test.loan_limit;\n@id(\"loan.limit\") fn limit(input: borrow Slice<u8>) -> i64 {\nlet owned = bytes_copy(input);\n",
        );
        for index in 0..count {
            source.push_str(&format!("let view{index} = bytes_as_slice(owned);\n"));
        }
        source.push_str("0\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");
        source
    }
    let accepted = crate::parse(&source(256), Path::new("loan-limit-256.spx")).unwrap();
    assert!(crate::verify::verify(&accepted).is_empty());
    if let Err(diagnostics) = crate::hir::resolve(&accepted) {
        panic!("256-loan boundary rejected: {diagnostics:?}");
    }
    let rejected = crate::parse(&source(257), Path::new("loan-limit-257.spx")).unwrap();
    assert!(crate::verify::verify(&rejected).is_empty());
    assert!(crate::hir::resolve(&rejected)
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-H006"));
}

#[test]
fn loan_free_function_above_cfg_point_bound_preserves_legacy_admission() {
    let program = fixture();
    let mut function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap()
        .clone();
    let seed = match &function.body.kind {
        ResolvedExprKind::Block { tail, .. } => (**tail).clone(),
        _ => panic!("resolved fixture body must be a block"),
    };
    let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    function.requires = (0..2_100)
        .map(|index| {
            let mut expression = seed.clone();
            expression.id = ExpressionId::new(&execution, &format!("preflight.{index}"));
            expression
        })
        .collect();

    let plan = build_plan(&program, &function)
        .expect("loan-free preflight must run before the 4,096-point CFG bound");
    assert_eq!(plan.schema, LOAN_PLAN_SCHEMA_V1);
    assert!(plan.loans.is_empty());
    assert!(plan.endpoints.is_empty());
    assert!(plan.edges.is_empty());
}
