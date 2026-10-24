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
    let mut starved = ReplayBudget::new();
    starved.remaining = 64;
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

/// A fitting scalar sequence has additive checked-failure paths and no owning
/// entry state. It previously used the complete enumerator at comparison time.
fn fitting_status_sequence(count: usize) -> ResolvedProgram {
    let mut source = String::from(
        "module test.fitting_factored;\n@id(\"app.main\") fn main() -> i64 {\nlet mut total = 0;\n",
    );
    for _ in 0..count {
        source.push_str("total = total + 1;\n");
    }
    source.push_str("total\n}\n");
    hir::resolve(&parse(&source, Path::new("fitting-factored.spx")).unwrap()).unwrap()
}

fn fitting_select(
    function: &ResolvedFunction,
    cfg: usize,
    hir: usize,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.reserve_bytes(
        function,
        super::super::factored::FITTING_SELECTOR_CONTROLS,
        "fitting factored comparison selector receivers",
    )?;
    super::super::factored::fitting_comparison_selected_paid(function, cfg, hir, budget)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FittingOutcome {
    success: bool,
    spent: usize,
    held: usize,
}

/// Includes selector, both independently reconstructed languages, weighted
/// proof (and canonical fallback), every owned temporary and full Drop.
fn fitting_probe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    cfg: usize,
    hir: usize,
    limit: usize,
) -> (
    (FittingOutcome, crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    crate::bounded_output::with_limit_usage_refusal(limit, || {
        crate::test_allocation_observer::measure(|| {
            let mut budget = ReplayBudget::new();
            // The unchanged preflight owns this pool. The fitting comparison
            // must restore it even on failure; only ordinary free work pays.
            budget.skeleton_remaining = 17;
            let result = fitting_select(function, cfg, hir, &mut budget).and_then(|selected| {
                assert!(selected);
                super::super::factored::compare(program, function, &mut budget)
            });
            let success = matches!(result, Ok(Some(true)));
            if let Err(error) = &result {
                assert_eq!(error.code, "SPX-H006");
                assert!(budget.refusal_spent || crate::bounded_output::active_refused());
                assert!(budget
                    .reserve_bytes(function, 0, "retry fitting proof")
                    .is_err());
            }
            let outcome = FittingOutcome {
                success,
                spent: MAX_REPLAY_WORK_UNITS - budget.remaining,
                held: budget.skeleton_remaining,
            };
            drop(result);
            drop(budget);
            outcome
        })
    })
}

#[test]
fn fitting_scalar_comparison_keeps_admission_and_independent_enumerator() {
    let program = fitting_status_sequence(32);
    let main = function(&program, "app.main");
    let cfg = branch_sensitive_cfg_bounds(&main).unwrap().terminal_paths;
    let hir = hir_terminal_path_bound(&main).unwrap();
    assert_eq!(main.cleanup_plan.status_sources.len(), 32);
    assert!(cfg <= 256 && hir <= 256);
    assert!(!super::super::factored::selected(&main, cfg, hir));
    validate_replay_size_budget(&main).unwrap();
    assert_program_skeleton_authority(&program);
    let mut control = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let mut expected = hir_skeleton_paths(&program, &main, &mut control).unwrap();
    let mut actual = plan_skeleton_paths(&main, &mut control).unwrap();
    expected.sort();
    actual.sort();
    assert_eq!(expected, actual);
    let (selected, refused, _, _) =
        crate::bounded_output::with_limit_usage_refusal(1 << 29, || {
            let mut budget = ReplayBudget::new();
            budget.skeleton_remaining = 17;
            let selected = super::super::factored::validate(&program, &main, &mut budget).unwrap();
            assert_eq!(budget.skeleton_remaining, 17);
            selected
        });
    assert!(selected && !refused, "the actual comparison entry selects");

    let ((baseline, metrics), refused, charged, _) =
        fitting_probe(&program, &main, cfg, hir, 1 << 29);
    assert!(baseline.success && !refused);
    assert_eq!(baseline.held, 17);
    assert!(baseline.spent > 0);
    assert!(!metrics.invalid && metrics.allocated <= charged);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let ((exact, metrics), refused, paid, _) = fitting_probe(&program, &main, cfg, hir, charged);
    assert_eq!(exact, baseline);
    assert!(!refused && paid == charged);
    assert!(!metrics.invalid && metrics.allocated <= paid);
    assert_eq!(metrics.live, 0);
    for cut in [0, 1, charged - 1] {
        let ((short, metrics), refused, paid, receipt) =
            fitting_probe(&program, &main, cfg, hir, cut);
        assert!(!short.success && refused && receipt.is_some());
        assert_eq!(short.held, 17);
        assert!(paid <= cut && !metrics.invalid && metrics.allocated <= paid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}

#[test]
fn fitting_selector_preserves_tiny_foreign_and_owned_controls() {
    let program = fitting_status_sequence(32);
    let main = function(&program, "app.main");
    let cfg = branch_sensitive_cfg_bounds(&main).unwrap().terminal_paths;
    let hir = hir_terminal_path_bound(&main).unwrap();
    let mut legacy = ReplayBudget::unobserved();
    assert!(!fitting_select(&main, cfg, hir, &mut legacy).unwrap());
    let tiny_program = fitting_status_sequence(31);
    let tiny = function(&tiny_program, "app.main");
    let owned_program = independent_decisions(1);
    let mut owned = function(&owned_program, "app.main");
    // Keep a genuine slot, but give the header enough genuine checked-status
    // rows to isolate the ownership exclusion from the threshold exclusion.
    owned.cleanup_plan.status_sources = main.cleanup_plan.status_sources.clone();
    let ((), refused, _, _) = crate::bounded_output::with_limit_usage_refusal(1 << 24, || {
        let mut budget = ReplayBudget::new();
        assert!(!fitting_select(&tiny, cfg, hir, &mut budget).unwrap());
        assert!(!fitting_select(&owned, cfg, hir, &mut budget).unwrap());
        assert!(!fitting_select(&main, 4_097, hir, &mut budget).unwrap());
        assert!(!fitting_select(&main, cfg, 4_097, &mut budget).unwrap());
        budget.admission = AdmissionMode::SelectedRetained;
        assert!(!fitting_select(&main, cfg, hir, &mut budget).unwrap());
        budget.admission = AdmissionMode::Ordinary;
        budget.payer = AllocationPayer::SelectedReservation;
        assert!(!super::super::factored::fitting_comparison_selected_paid(
            &main,
            cfg,
            hir,
            &mut budget,
        )
        .unwrap());
    });
    assert!(!refused);
}

#[test]
fn fitting_scalar_header_never_proves_a_mutated_plan() {
    let program = fitting_status_sequence(32);
    let main = function(&program, "app.main");
    let mut malformed = main.clone();
    malformed.cleanup_plan.schema = "semaprax.cleanup.invalid";
    let error = validate_structure(&program, &malformed).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("schema"));

    let mut changed = main.clone();
    let edge = changed
        .cleanup_plan
        .edges
        .iter_mut()
        .find(|edge| matches!(edge.condition, EdgeCondition::StatusZero(_)))
        .unwrap();
    let EdgeCondition::StatusZero(source) = &edge.condition else {
        unreachable!()
    };
    edge.condition = EdgeCondition::StatusNonzero(source.clone());
    assert_eq!(fresh_compare(&program, &changed), Some(false));
    assert_independent_replay_rejects(&program, &changed);

    let mut cycle = main.clone();
    cycle.cleanup_plan.edges[0].to = cycle.cleanup_plan.entry;
    let error = validate_structure(&program, &cycle).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("failed independent replay"));
    let cfg = branch_sensitive_cfg_bounds(&main).unwrap().terminal_paths;
    let hir = hir_terminal_path_bound(&main).unwrap();
    let (selected, refused, _, _) =
        crate::bounded_output::with_limit_usage_refusal(1 << 24, || {
            fitting_select(&cycle, cfg, hir, &mut ReplayBudget::new()).unwrap()
        });
    assert!(!selected && !refused);
}
