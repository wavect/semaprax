//! The factored typed-control skeleton comparison: selected above the
//! enumeration threshold, a genuine check of the plan, and bounded by the
//! same work budget, whose exhaustion keeps the path-budget diagnostic.
use super::*;
use std::fmt::Write as _;

/// `main` keeps one owned string live across `count` independent checked
/// scalar decisions.
fn independent_decisions(count: usize) -> ResolvedProgram {
    let mut source = String::from(
        "module test.factored;\n@id(\"app.main\") fn main() -> i64 {\nlet text = \"hello\";\nlet n = 7;\nlet mut total = 0;\n",
    );
    for index in 0..count {
        writeln!(
            source,
            "total = total + if n > {index} {{ 1 }} else {{ 0 }};"
        )
        .unwrap();
    }
    source.push_str("total + string_len(text)\n}\n");
    let parsed = parse(&source, Path::new("factored.spx")).unwrap();
    let diagnostics = crate::verify::verify(&parsed);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&parsed).unwrap()
}

fn fresh_compare(program: &ResolvedProgram, function: &ResolvedFunction) -> Option<bool> {
    let mut budget = ReplayBudget::new();
    super::super::factored::compare(program, function, &mut budget).unwrap()
}

#[test]
fn independent_decisions_select_the_factored_comparison_and_verify() {
    let program = independent_decisions(30);
    let main = function(&program, "app.main");
    let semantic_paths = hir_terminal_path_bound(&main).unwrap();
    let cfg_paths = branch_sensitive_cfg_bounds(&main).unwrap().terminal_paths;
    assert!(semantic_paths > MAX_REPLAY_PATHS && cfg_paths > MAX_REPLAY_PATHS);
    assert!(super::super::factored::selected(
        &main,
        cfg_paths,
        semantic_paths
    ));
    assert!(!main.cleanup_plan.slots.is_empty(), "the string stays live");
    validate_structure(&program, &main).unwrap();
    assert_eq!(fresh_compare(&program, &main), Some(true));
}

#[test]
fn factored_comparison_rejects_plans_that_change_the_skeleton() {
    let program = independent_decisions(24);
    let main = function(&program, "app.main");

    // A decision whose two edges claim the same outcome.
    let mut duplicated = main.clone();
    let edge = duplicated
        .cleanup_plan
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            CleanupTerminator::Branch(edges) => edges.iter().copied().find(|edge| {
                matches!(
                    duplicated.cleanup_plan.edges[edge.0 as usize].condition,
                    EdgeCondition::BooleanResult(_, true)
                )
            }),
            _ => None,
        })
        .expect("a Boolean decision edge");
    let EdgeCondition::BooleanResult(_, value) =
        &mut duplicated.cleanup_plan.edges[edge.0 as usize].condition
    else {
        unreachable!();
    };
    *value = false;
    assert_eq!(fresh_compare(&program, &duplicated), Some(false));
    assert_independent_replay_rejects(&program, &duplicated);

    // The owned string's initialization disappears from every path.
    let mut uninitialized = main.clone();
    let block = uninitialized
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .any(|transition| matches!(transition, CleanupTransition::Initialize { .. }))
        })
        .expect("the string literal initializes its slot");
    block
        .transitions
        .retain(|transition| !matches!(transition, CleanupTransition::Initialize { .. }));
    assert_eq!(fresh_compare(&program, &uninitialized), Some(false));
    assert_independent_replay_rejects(&program, &uninitialized);
}

/// Formerly `kernel_boundary::fifteen_independent_scalar_comparisons_diagnostic_names_the_combinatorial_driver_and_remedy`:
/// source text no longer reaches the enumeration diagnostic, so its wording
/// is pinned where it still applies, a factored comparison that runs out of
/// the shared work budget above the enumeration ceiling.
#[test]
fn budget_exhausted_factored_comparison_keeps_the_path_budget_diagnostic() {
    let program = independent_decisions(20);
    let main = function(&program, "app.main");
    assert!(hir_terminal_path_bound(&main).unwrap() > MAX_REPLAY_PATHS);
    let mut starved = ReplayBudget {
        remaining: 64,
        skeleton_remaining: 0,
        merge_paths: false,
    };
    let error = super::super::factored::validate(&program, &main, &mut starved).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(
        error.message.contains("exceeding the 65536 path budget"),
        "{}",
        error.message
    );
    assert!(error.message.contains("combined independently"));
    assert!(error.message.contains("combinatorially"));
    assert!(
        error.message.contains("mutually exclusive") || error.message.contains("separate calls")
    );
}
