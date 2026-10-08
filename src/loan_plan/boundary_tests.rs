//! Exact-capacity and first-overflow evidence for Shared Loan Plan v1.
//!
//! These fixtures operate on resolved, typed HIR and always retain a real
//! own-root loan. Point/edge padding uses disconnected Boolean preconditions.
//! The checked-work boundary instead places Boolean statements before real
//! last uses of every loan, so the production planner performs the measured
//! reachability and live-edge work.

use std::path::Path;

use crate::ast::Span;
use crate::hir::{
    FunctionExecutionId, PatternValue, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedMatchArm, ResolvedMatchMode, ResolvedMatchPattern, ResolvedStatement,
    ResolvedType, ValueId,
};

use super::*;

fn fixture(loan_count: usize) -> (ResolvedProgram, usize) {
    fixture_with_uses(loan_count, false)
}

fn fixture_with_uses(loan_count: usize, retain_last_uses: bool) -> (ResolvedProgram, usize) {
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
    if retain_last_uses {
        for index in 0..loan_count {
            source.push_str(&format!(
                "let boundary_use_{index} = byte_len(boundary_view_{index});\n"
            ));
        }
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

fn padding_statement(
    function: &ResolvedFunction,
    path: &str,
    value: ResolvedExpr,
) -> ResolvedStatement {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    ResolvedStatement::Let {
        binding: ResolvedBinding {
            id: ValueId::local(&execution, &format!("{path}.binding")),
            name: format!("__loan_work_{}", path.replace('.', "_")),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Bool,
            span: Span::default(),
        },
        mutable: false,
        value,
        span: Span::default(),
    }
}

fn add_live_padding(
    function: &mut ResolvedFunction,
    loan_count: usize,
    prefix: &str,
    leaves: usize,
    branches: usize,
    matches: usize,
) {
    let mut padding = Vec::new();
    for index in 0..leaves {
        let path = format!("{prefix}.leaf.{index}");
        padding.push(padding_statement(
            function,
            &path,
            bool_leaf(function, &path),
        ));
    }
    for index in 0..branches {
        let path = format!("{prefix}.branch.{index}");
        padding.push(padding_statement(
            function,
            &path,
            branch_root(function, &path),
        ));
    }
    for index in 0..matches {
        let path = format!("{prefix}.match.{index}");
        padding.push(padding_statement(
            function,
            &path,
            three_arm_match_root(function, &path),
        ));
    }
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("boundary fixture body remains a block")
    };
    let before_last_uses = 1 + loan_count;
    statements.splice(before_last_uses..before_last_uses, padding);
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

fn uncached_live_nodes(cfg: &Cfg<'_>, start: u16, seeds: &BTreeSet<u16>) -> BTreeSet<u16> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(node) = pending.pop() {
        if reachable.insert(node) {
            pending.extend(cfg.successors[node as usize].iter().rev().copied());
        }
    }
    let mut live = BTreeSet::new();
    let mut pending = seeds
        .iter()
        .filter(|seed| reachable.contains(seed))
        .copied()
        .collect::<Vec<_>>();
    pending.push(start);
    while let Some(node) = pending.pop() {
        if !reachable.contains(&node) || !live.insert(node) || node == start {
            continue;
        }
        pending.extend(cfg.predecessors[node as usize].iter().rev().copied());
    }
    live
}

fn all_edge_liveness(cfg: &Cfg<'_>, live: &[BTreeSet<u16>]) -> work::EdgeLiveness {
    let mut edge_live = vec![Vec::new(); cfg.edges.len()];
    let mut termination_edges = vec![Vec::new(); live.len()];
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for (edge_index, (from, to)) in cfg.edges.iter().copied().enumerate() {
            if nodes.contains(&from) && nodes.contains(&to) {
                edge_live[edge_index].push(id);
            } else if nodes.contains(&from) {
                termination_edges[loan_index].push(edge_index as u16);
            }
        }
    }
    (edge_live, termination_edges)
}

#[test]
fn cached_reachability_and_live_source_edges_preserve_canonical_proof() {
    let (program, index) = fixture(2);
    let function = &program.functions[index];
    let mut cfg_work = WorkCounter::new(usize::MAX);
    let cfg = build_cfg(function, &mut cfg_work).expect("fixture CFG builds");
    let start = cfg
        .node(&function.body, LoanPointPhase::Before)
        .expect("body start is indexed");
    let end = cfg
        .node(&function.body, LoanPointPhase::After)
        .expect("body end is indexed");
    let seeds = BTreeSet::from([end]);
    let expected = uncached_live_nodes(&cfg, start, &seeds);

    let mut reachable = work::ReachabilityCache::default();
    let mut work = WorkCounter::new(usize::MAX);
    let first = work::live_nodes(&cfg, start, &seeds, &mut reachable, &mut work).unwrap();
    let first_work = work.used;
    let second = work::live_nodes(&cfg, start, &seeds, &mut reachable, &mut work).unwrap();
    let second_work = work.used - first_work;
    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert!(second_work < first_work);

    let live = vec![first, BTreeSet::from([start])];
    let expected_edges = all_edge_liveness(&cfg, &live);
    let before_edges = work.used;
    let actual_edges = work::edge_liveness(&cfg, &live, &mut work).unwrap();
    assert_eq!(actual_edges, expected_edges);
    assert!(work.used - before_edges < live.len() * cfg.edges.len());
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
    // Each byte_len last use is itself a synchronous borrowed-call loan. Pair
    // 128 own-root views with their 128 real last-use calls to exercise the
    // exact 256-loan boundary without exceeding it before work measurement.
    const WORK_ROOT_LOANS: usize = MAX_LOANS_PER_FUNCTION_V1 / 2;
    let (program, index) = fixture_with_uses(WORK_ROOT_LOANS, true);
    let base = program.functions[index].clone();
    let (base_result, base_work) = build_cfg_plan_with_work_limit(&program, &base, usize::MAX);
    base_result.expect("the unpadded boundary fixture builds");
    let (base_points, base_edges) = cfg_counts(&base);

    let measure_delta = |leaves, branches, matches| {
        let mut probe = base.clone();
        add_live_padding(
            &mut probe,
            WORK_ROOT_LOANS,
            "work.probe",
            leaves,
            branches,
            matches,
        );
        let (result, used) = build_cfg_plan_with_work_limit(&program, &probe, usize::MAX);
        result.expect("a one-shape work probe builds");
        let (points, edges) = cfg_counts(&probe);
        (used - base_work, points - base_points, edges - base_edges)
    };
    let (leaf_work, leaf_points, leaf_edges) = measure_delta(1, 0, 0);
    let (branch_work, branch_points, branch_edges) = measure_delta(0, 1, 0);
    let (match_work, match_points, match_edges) = measure_delta(0, 0, 1);
    let mut tuning_probe = base.clone();
    add_padding(&mut tuning_probe, "work.tuning.probe", 1, 0, 0);
    let (tuning_result, tuning_used) =
        build_cfg_plan_with_work_limit(&program, &tuning_probe, usize::MAX);
    tuning_result.expect("the residue-tuning work probe builds");
    let (tuning_points, tuning_edges) = cfg_counts(&tuning_probe);
    let tuning_work = tuning_used - base_work;
    let tuning_points = tuning_points - base_points;
    let tuning_edges = tuning_edges - base_edges;

    let mut shape = None;
    for matches in 0..=((MAX_LOAN_PLAN_WORK_V1 - base_work) / match_work) {
        let after_matches = base_work + matches * match_work;
        for branches in 0..=((MAX_LOAN_PLAN_WORK_V1 - after_matches) / branch_work) {
            let after_branches = after_matches + branches * branch_work;
            let max_live_leaves = (MAX_LOAN_PLAN_WORK_V1 - after_branches) / leaf_work;
            for live_leaves in (0..=max_live_leaves).rev() {
                let remaining = MAX_LOAN_PLAN_WORK_V1 - after_branches - live_leaves * leaf_work;
                if !remaining.is_multiple_of(tuning_work) {
                    continue;
                }
                let tuning_leaves = remaining / tuning_work;
                let points = base_points
                    + match_points * matches
                    + branch_points * branches
                    + leaf_points * live_leaves
                    + tuning_points * tuning_leaves;
                let edges = base_edges
                    + match_edges * matches
                    + branch_edges * branches
                    + leaf_edges * live_leaves
                    + tuning_edges * tuning_leaves;
                if points <= MAX_LOAN_ENDPOINTS_V1 - leaf_points
                    && edges <= MAX_LOAN_EDGES_V1 - leaf_edges
                {
                    shape = Some((live_leaves, branches, matches, tuning_leaves));
                    break;
                }
            }
            if shape.is_some() {
                break;
            }
        }
        if shape.is_some() {
            break;
        }
    }
    let (live_leaves, branches, matches, tuning_leaves) = shape.unwrap_or_else(|| {
        panic!(
            "a one-million-work fixture exists: base work/points/edges={base_work}/{base_points}/{base_edges}, live leaf={leaf_work}/{leaf_points}/{leaf_edges}, branch={branch_work}/{branch_points}/{branch_edges}, match={match_work}/{match_points}/{match_edges}, tuning leaf={tuning_work}/{tuning_points}/{tuning_edges}"
        )
    });
    let mut exact = base.clone();
    add_live_padding(
        &mut exact,
        WORK_ROOT_LOANS,
        "work.boundary",
        live_leaves,
        branches,
        matches,
    );
    add_padding(&mut exact, "work.tuning.boundary", tuning_leaves, 0, 0);

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

    add_live_padding(&mut exact, WORK_ROOT_LOANS, "work.overflow", 1, 0, 0);
    let (overflow, used) = build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1);
    let error = overflow.unwrap_err();
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1 + 1);
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "loan analysis exceeds 1,000,000 checked work"
    );
}
