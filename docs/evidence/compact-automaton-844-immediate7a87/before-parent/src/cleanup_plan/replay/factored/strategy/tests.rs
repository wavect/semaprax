//! Census observations and physical storage are independently observed here.
//! Synthetic automata establish representation choice, never source admission.
use super::*;

fn classes(paths: &[SkeletonPath]) -> TerminalCounts {
    paths.iter().fold(TerminalCounts::ZERO, |counts, path| {
        counts.add(TerminalCounts::terminal(path.terminal))
    })
}

#[test]
fn exact_strategy_census_matches_independent_small_terminal_multisets() {
    for source in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let (program, function) = lexical_plan_tests::fixture(source);
        let mut budget = ReplayBudget::new();
        let candidate = Candidate::reconstruct(&program, &function, &mut budget).unwrap();
        assert_eq!(candidate.strategy, Strategy::Enumerated);
        assert_eq!(candidate.hir, candidate.plan);
        for (root, reported) in [
            (candidate.built.hir_root, candidate.hir),
            (candidate.built.plan_root, candidate.plan),
        ] {
            let paths = lexical_plan_tests::enumerate(&candidate.built.automaton, root);
            assert_eq!(reported, classes(&paths).census());
        }
        assert!(budget.remaining < MAX_REPLAY_WORK_UNITS);
        assert!(!budget.merge_paths);
        assert_eq!(budget.skeleton_remaining, 0);
    }
}

/// Repeated duplicate epsilon routes give exactly 2^depth acceptances of one
/// terminal. No saturation, identity deduplication, or source quota is involved.
fn product(
    function: &ResolvedFunction,
    depth: usize,
    terminal: SkeletonTerminal,
) -> BuiltComparison {
    let mut budget = ReplayBudget::new();
    let mut automaton = Automaton::default();
    let sink = automaton.node(function, &mut budget).unwrap();
    let mut root = automaton
        .end(function, &mut budget, sink, terminal)
        .unwrap();
    for _ in 0..depth {
        let parent = automaton.node(function, &mut budget).unwrap();
        automaton
            .epsilon(function, &mut budget, parent, root)
            .unwrap();
        automaton
            .epsilon(function, &mut budget, parent, root)
            .unwrap();
        root = parent;
    }
    BuiltComparison {
        automaton,
        sink,
        hir_root: root,
        plan_root: root,
    }
}

#[test]
fn exact_exceeded_census_selects_distinct_factored_candidate_without_discount() {
    let (_, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let mut budget = ReplayBudget::new();
    let built = product(&function, 17, SkeletonTerminal::Success);
    let candidate = Candidate::from_built(&function, built, &mut budget).unwrap();
    assert_eq!(candidate.strategy, Strategy::LexicalFactored);
    assert_eq!(
        candidate.hir,
        Census::Exceeded {
            classes: TerminalCounts {
                success: Number::Known(131_072),
                ..TerminalCounts::ZERO
            },
            total: 131_072,
        }
    );
    assert_eq!(candidate.plan, candidate.hir);
    assert!(budget.remaining < MAX_REPLAY_WORK_UNITS);
    assert!(
        path_summary::path_budget_errors(&function, 131_072, 131_072).is_err(),
        "the ordinary enumerating ceiling remains unchanged"
    );
}

#[test]
fn arithmetic_overflow_retains_each_actual_terminal_class_and_refuses_choice() {
    let (_, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let built = product(&function, 64, SkeletonTerminal::LoopLatch);
    let mut budget = ReplayBudget::new();
    let candidate = Candidate::from_built(&function, built, &mut budget).unwrap();
    assert_eq!(candidate.strategy, Strategy::RefusedArithmeticOverflow);
    assert_eq!(
        candidate.hir,
        Census::ArithmeticOverflow {
            classes: TerminalCounts {
                latch: Number::ArithmeticOverflow,
                ..TerminalCounts::ZERO
            },
        }
    );
    assert_eq!(candidate.plan, candidate.hir);
    // Total overflow also remains explicit when every individual class fits.
    assert_eq!(
        TerminalCounts {
            success: Number::Known(1 << 63),
            failure: Number::Known(1 << 63),
            latch: Number::Known(0),
        }
        .census(),
        Census::ArithmeticOverflow {
            classes: TerminalCounts {
                success: Number::Known(1 << 63),
                failure: Number::Known(1 << 63),
                latch: Number::Known(0),
            }
        }
    );
}

#[test]
fn cycles_unknown_targets_and_nonterminal_sinks_are_not_censuses() {
    let (_, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    for attack in 0..3 {
        let mut built = product(&function, 1, SkeletonTerminal::Success);
        let root = built.hir_root;
        match attack {
            0 => built.automaton.nodes[root].epsilon.push(root),
            1 => built.automaton.nodes[root].epsilon.push(usize::MAX),
            2 => built.automaton.nodes[built.sink].epsilon.push(root),
            _ => unreachable!(),
        }
        let mut budget = ReplayBudget::new();
        assert!(census(&function, &built, root, &mut budget).is_err());
        assert!(budget.remaining <= MAX_REPLAY_WORK_UNITS);
    }
}

fn probe(
    function: &ResolvedFunction,
    built: &BuiltComparison,
    bytes: usize,
) -> (
    Result<usize, Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        // Creation and release of every local census carrier are inside this
        // observer; the independently supplied automaton remains borrowed.
        census(function, built, built.hir_root, &mut budget)?;
        census(function, built, built.plan_root, &mut budget)?;
        Ok(budget.physical.as_ref().unwrap().charged() - before)
    })
}

#[test]
fn exact_census_creation_and_release_are_precharged_at_exact_and_one_short() {
    let (_, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let built = product(&function, 64, SkeletonTerminal::Failure);
    let (result, observed) = probe(&function, &built, 1 << 20);
    let charged = result.unwrap();
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    let baseline = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (exact, observed) = probe(&function, &built, admission + charged);
    assert_eq!(exact.unwrap(), charged);
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    let (short, _) = probe(&function, &built, admission + charged - 1);
    assert!(short.is_err());
}

#[test]
fn failed_census_keeps_spent_global_work() {
    let (_, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let built = product(&function, 4, SkeletonTerminal::Success);
    let mut budget = ReplayBudget::new();
    budget.remaining = built.automaton.nodes.len() * 2 + 3;
    assert!(census(&function, &built, built.hir_root, &mut budget).is_err());
    assert_eq!(budget.remaining, 0);
    assert!(budget
        .charge(&function, 1, "attempted strategy retry")
        .is_err());
}
