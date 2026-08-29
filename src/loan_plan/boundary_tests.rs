//! Exact-capacity and first-overflow evidence for Shared Loan Plan v1.
//!
//! These fixtures operate on resolved, typed HIR and always retain a real
//! own-root loan. Padding roots are disconnected Boolean preconditions, so
//! they change only the CFG/work dimension under test and cannot extend a
//! loan lifetime or alter cleanup meaning.

use std::path::Path;

use crate::ast::Span;
use crate::hir::{
    FunctionExecutionId, PatternValue, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchArm, ResolvedMatchMode, ResolvedMatchPattern, ResolvedType,
};

use super::*;

fn fixture(loan_count: usize) -> (ResolvedProgram, usize) {
    let mut source = String::from(
        "module test.loan_plan_boundaries;\n\
         @id(\"loan.boundary\")\n\
         fn boundary(input: borrow Slice<u8>) -> i64 {\n\
         let owned = bytes_copy(input);\n",
    );
    for index in 0..loan_count {
        source.push_str(&format!(
            "let boundary_view_{index} = bytes_as_slice(owned);\n"
        ));
    }
    source.push_str("0\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");
    let ast = crate::parse(&source, Path::new("loan-plan-boundaries.spx")).unwrap();
    assert!(crate::verify::verify(&ast).is_empty());
    let program = crate::hir::resolve(&ast).unwrap();
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.boundary")
        .unwrap();
    (program, index)
}

fn expression_id(function: &ResolvedFunction, path: &str) -> ExpressionId {
    ExpressionId::new(&FunctionExecutionId::Monomorphic(function.id.clone()), path)
}

fn bool_leaf(function: &ResolvedFunction, path: &str) -> ResolvedExpr {
    ResolvedExpr {
        id: expression_id(function, path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Bool(true),
        span: Span::default(),
    }
}

fn branch_root(function: &ResolvedFunction, path: &str) -> ResolvedExpr {
    ResolvedExpr {
        id: expression_id(function, path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::If {
            condition: Box::new(bool_leaf(function, &format!("{path}.condition"))),
            then_branch: Box::new(bool_leaf(function, &format!("{path}.then"))),
            else_branch: Box::new(bool_leaf(function, &format!("{path}.else"))),
        },
        span: Span::default(),
    }
}

fn three_arm_match_root(function: &ResolvedFunction, path: &str) -> ResolvedExpr {
    let arm = |suffix: &str, pattern| ResolvedMatchArm {
        pattern,
        guard: None,
        value: bool_leaf(function, &format!("{path}.arm.{suffix}")),
        span: Span::default(),
    };
    ResolvedExpr {
        id: expression_id(function, path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Value,
            scrutinee: Box::new(bool_leaf(function, &format!("{path}.scrutinee"))),
            arms: vec![
                arm(
                    "true",
                    ResolvedMatchPattern::Literal(PatternValue::Bool(true)),
                ),
                arm(
                    "false",
                    ResolvedMatchPattern::Literal(PatternValue::Bool(false)),
                ),
                arm("fallback", ResolvedMatchPattern::Wildcard),
            ],
        },
        span: Span::default(),
    }
}

fn add_padding(
    function: &mut ResolvedFunction,
    prefix: &str,
    leaves: usize,
    branches: usize,
    matches: usize,
) {
    for index in 0..leaves {
        let expression = bool_leaf(function, &format!("{prefix}.leaf.{index}"));
        function.requires.push(expression);
    }
    for index in 0..branches {
        let expression = branch_root(function, &format!("{prefix}.branch.{index}"));
        function.requires.push(expression);
    }
    for index in 0..matches {
        let expression = three_arm_match_root(function, &format!("{prefix}.match.{index}"));
        function.requires.push(expression);
    }
}

fn cfg_counts(function: &ResolvedFunction) -> (usize, usize) {
    let mut work = WorkCounter::new(usize::MAX);
    let cfg = build_cfg(function, &mut work).expect("boundary fixture CFG builds");
    (cfg.points.len(), cfg.edges.len())
}

fn install_plan(
    mut program: ResolvedProgram,
    index: usize,
    function: ResolvedFunction,
    plan: LoanPlan,
) -> ResolvedProgram {
    let mut function = function;
    function.loan_plan = plan;
    program.functions[index] = function;
    program
}

#[test]
fn exact_4096_program_points_rebuild_and_first_representable_overflow_fail_closed() {
    let (program, index) = fixture(1);
    let mut function = program.functions[index].clone();
    let (base_points, _) = cfg_counts(&function);
    assert_eq!(base_points % 2, 0);
    let leaves = (MAX_LOAN_ENDPOINTS_V1 - base_points) / 2;
    add_padding(&mut function, "point.boundary", leaves, 0, 0);

    let plan = build_plan(&program, &function).expect("4,096 points are admitted");
    assert_eq!(plan.endpoints.len(), MAX_LOAN_ENDPOINTS_V1);
    assert!(plan.edges.iter().any(|edge| {
        edge.from as usize == MAX_LOAN_ENDPOINTS_V1 - 2
            && edge.to as usize == MAX_LOAN_ENDPOINTS_V1 - 1
    }));
    assert_eq!(plan, build_plan(&program, &function).unwrap());
    let authenticated = install_plan(program.clone(), index, function.clone(), plan.clone());
    validate_program(&authenticated).expect("the exact-boundary carrier replays");

    let mut forged = authenticated;
    forged.functions[index]
        .loan_plan
        .endpoints
        .swap(0, MAX_LOAN_ENDPOINTS_V1 - 1);
    assert_eq!(validate_program(&forged).unwrap_err().code, "SPX-H006");

    add_padding(&mut function, "point.overflow", 1, 0, 0);
    let error = build_plan(&program, &function).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.message, "function exceeds 4,096 loan program points");
}

#[test]
fn exact_4096_cfg_edges_rebuild_and_edge_4097_fails_before_point_capacity() {
    let (program, index) = fixture(1);
    let mut function = program.functions[index].clone();
    let (base_points, base_edges) = cfg_counts(&function);
    let mut shape = None;
    for matches in 0..=((MAX_LOAN_EDGES_V1 - base_edges) / 11) {
        for branches in 0..=((MAX_LOAN_EDGES_V1 - base_edges - 11 * matches) / 8) {
            let leaves = MAX_LOAN_EDGES_V1 - base_edges - 11 * matches - 8 * branches;
            let points = base_points + 10 * matches + 8 * branches + 2 * leaves;
            if points <= MAX_LOAN_ENDPOINTS_V1 - 2 {
                shape = Some((leaves, branches, matches));
                break;
            }
        }
        if shape.is_some() {
            break;
        }
    }
    let (leaves, branches, matches) = shape.expect("an isolated exact-edge fixture exists");
    add_padding(&mut function, "edge.boundary", leaves, branches, matches);

    let plan = build_plan(&program, &function).expect("4,096 edges are admitted");
    assert_eq!(plan.edges.len(), MAX_LOAN_EDGES_V1);
    let exact_points = plan.endpoints.len();
    assert!(exact_points <= MAX_LOAN_ENDPOINTS_V1 - 2);
    assert_eq!(plan, build_plan(&program, &function).unwrap());
    let authenticated = install_plan(program.clone(), index, function.clone(), plan);
    validate_program(&authenticated).expect("the exact-edge carrier replays");

    let mut forged = authenticated;
    forged.functions[index]
        .loan_plan
        .edges
        .swap(0, MAX_LOAN_EDGES_V1 - 1);
    assert_eq!(validate_program(&forged).unwrap_err().code, "SPX-H006");

    add_padding(&mut function, "edge.overflow", 1, 0, 0);
    assert!(exact_points + 2 <= MAX_LOAN_ENDPOINTS_V1);
    let error = build_plan(&program, &function).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.message, "function exceeds 4,096 loan CFG edges");
}

#[test]
fn exact_million_work_build_replays_and_the_first_extra_unit_is_fail_closed() {
    let (program, index) = fixture(MAX_LOANS_PER_FUNCTION_V1);
    let base = program.functions[index].clone();
    let (base_result, base_work) = build_cfg_plan_with_work_limit(&program, &base, usize::MAX);
    base_result.expect("the unpadded boundary fixture builds");

    let measure_delta = |leaves, branches, matches| {
        let mut probe = base.clone();
        add_padding(&mut probe, "work.probe", leaves, branches, matches);
        let (result, used) = build_cfg_plan_with_work_limit(&program, &probe, usize::MAX);
        result.expect("a one-shape work probe builds");
        used - base_work
    };
    let leaf_work = measure_delta(1, 0, 0);
    let branch_work = measure_delta(0, 1, 0);
    let match_work = measure_delta(0, 0, 1);
    let (base_points, base_edges) = cfg_counts(&base);

    let mut shape = None;
    for matches in 0..=((MAX_LOAN_PLAN_WORK_V1 - base_work) / match_work) {
        let after_matches = base_work + matches * match_work;
        for branches in 0..=((MAX_LOAN_PLAN_WORK_V1 - after_matches) / branch_work) {
            let remaining = MAX_LOAN_PLAN_WORK_V1 - after_matches - branches * branch_work;
            if !remaining.is_multiple_of(leaf_work) {
                continue;
            }
            let leaves = remaining / leaf_work;
            let points = base_points + 10 * matches + 8 * branches + 2 * leaves;
            let edges = base_edges + 11 * matches + 8 * branches + leaves;
            if points <= MAX_LOAN_ENDPOINTS_V1 - 2 && edges < MAX_LOAN_EDGES_V1 {
                shape = Some((leaves, branches, matches));
                break;
            }
        }
        if shape.is_some() {
            break;
        }
    }
    let (leaves, branches, matches) = shape.expect("a one-million-work fixture exists");
    let mut exact = base.clone();
    add_padding(&mut exact, "work.boundary", leaves, branches, matches);

    let (plan, used) = build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1);
    let plan = plan.expect("exactly 1,000,000 work units are admitted");
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1);
    assert_eq!(plan.loans.len(), MAX_LOANS_PER_FUNCTION_V1);
    let authenticated = install_plan(program.clone(), index, exact.clone(), plan);
    validate_program(&authenticated).expect("the exact-work carrier replays");

    let (one_too_small, used) =
        build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1 - 1);
    let error = one_too_small.unwrap_err();
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1);
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "loan analysis exceeds 1,000,000 checked work"
    );

    add_padding(&mut exact, "work.overflow", 1, 0, 0);
    let (overflow, used) = build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1);
    let error = overflow.unwrap_err();
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1 + 1);
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "loan analysis exceeds 1,000,000 checked work"
    );
}
