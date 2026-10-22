//! Independent previous constructor and complete current lifetime controls.
include!("early_release/before.rs");

#[test]
fn complete_previous_constructor_and_current_only_release_delta_remain_exact() {
    use sha2::{Digest, Sha256};
    let hash = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    let original = include_str!("early_release/before.rs")
        .strip_prefix("// Complete immediate previous owner, independently executable.\nuse super::*;\n#[rustfmt::skip]\n").unwrap()
        .replacen("fn build_comparison_before_release<", "fn build_comparison<", 1);
    assert_eq!(
        hash(&original),
        "b7bc2b128d764cfe6e96620ea8e4e614bb3e263dfd6510608a4f747dd5fb2c05"
    );
    let current = include_str!("../factored.rs");
    let current = &current[current.find("fn build_comparison<'hir>(").unwrap()..];
    let current = current
        .split_once("\n#[path = \"factored/strategy.rs\"]")
        .unwrap()
        .0;
    let addition = r###"        // Automaton labels now own every retained observation. Each explicit
        // release pays its argument/formal and reservation-result transport.
        #[cfg(test)]
        early_release::frontier(0, budget);
        budget.reserve_array::<(dag_memo::Memo, dag_memo::Memo, Result<(), Diagnostic>)>(
            function,
            1,
            "factored DAG memo release transport",
        )?;
        drop(memo);
        #[cfg(test)]
        early_release::frontier(1, budget);
        budget.reserve_array::<(Vec<SkeletonPath>, Vec<SkeletonPath>, Result<(), Diagnostic>)>(
            function,
            1,
            "factored HIR paths release transport",
        )?;
        drop(hir_paths);
        #[cfg(test)]
        early_release::frontier(2, budget);
"###;
    assert_eq!(current.matches(addition).count(), 1);
    assert_eq!(current.replacen(addition, "", 1), original);
}

fn compare(program: &ResolvedProgram, function: &ResolvedFunction, work: usize) {
    let mut old_budget = ReplayBudget::with_skeleton_limit(work);
    let mut current_budget = ReplayBudget::with_skeleton_limit(work);
    let old = build_comparison_before_release(program, function, &mut old_budget, None, None);
    let current = build_comparison(program, function, &mut current_budget, None, None);
    match (&old, &current) {
        (Ok(old), Ok(current)) => {
            assert_eq!(
                (current.sink, current.hir_root, current.plan_root),
                (old.sink, old.hir_root, old.plan_root)
            );
            assert_eq!(current.automaton.nodes.len(), old.automaton.nodes.len());
            for (current, old) in current.automaton.nodes.iter().zip(&old.automaton.nodes) {
                assert_eq!(current.moves(), old.moves());
                assert_eq!(current.epsilon(), old.epsilon());
            }
        }
        (Err(old), Err(current)) => assert_eq!(current.json(), old.json()),
        _ => panic!("early release changed the complete constructor result"),
    }
    assert_eq!(
        (
            current_budget.remaining,
            current_budget.skeleton_remaining,
            current_budget.skeleton_spent,
            current_budget.lent_skeleton,
            current_budget.merge_paths
        ),
        (
            old_budget.remaining,
            old_budget.skeleton_remaining,
            old_budget.skeleton_spent,
            old_budget.lent_skeleton,
            old_budget.merge_paths
        )
    );
}

#[test]
fn early_release_preserves_all_automaton_labels_order_diagnostics_and_spent_work() {
    for source in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let (program, function) = lexical_plan_tests::fixture(source);
        for work in [0, 1, 4, 32, 256, MAX_REPLAY_WORK_UNITS] {
            compare(&program, &function, work);
        }
        let mut forged = function.clone();
        forged.cleanup_plan.blocks[0].transitions.clear();
        compare(&program, &forged, MAX_REPLAY_WORK_UNITS);
    }
}

thread_local! {
    static FRONTIERS: std::cell::Cell<[Option<usize>; 3]> = const {std::cell::Cell::new([None;3])};
}
pub(super) fn frontier(index: usize, budget: &ReplayBudget) {
    if let Some(physical) = &budget.physical {
        FRONTIERS.with(|record| {
            let mut values = record.get();
            values[index] = Some(physical.charged());
            record.set(values);
        });
    }
}
fn probe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
    limit: usize,
) -> (
    bool,
    usize,
    [Option<usize>; 3],
    crate::test_allocation_observer::Metrics,
) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    FRONTIERS.with(|record| record.set([None; 3]));
    let ((success, charged), metrics) = crate::test_allocation_observer::measure(|| {
        let result = if old {
            build_comparison_before_release(program, function, &mut budget, None, None)
        } else {
            build_comparison(program, function, &mut budget, None, None)
        };
        let success = result.is_ok();
        drop(result);
        let charged = budget.physical.as_ref().unwrap().charged();
        if !success {
            assert!(budget
                .reserve_bytes(function, 0, "release sticky retry")
                .is_err());
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
        }
        drop(budget);
        (success, charged)
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    (
        success,
        charged,
        FRONTIERS.with(std::cell::Cell::get),
        metrics,
    )
}

#[test]
fn complete_constructor_drop_observes_both_old_and_released_owner_lifetimes() {
    let memo_transport =
        std::mem::size_of::<(dag_memo::Memo, dag_memo::Memo, Result<(), Diagnostic>)>();
    let paths_transport =
        std::mem::size_of::<(Vec<SkeletonPath>, Vec<SkeletonPath>, Result<(), Diagnostic>)>();
    for source in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let (program, _) = lexical_plan_tests::fixture(source);
        // Selected physical replay authenticates the exact held member. The
        // fixture's detached clone belongs to the ordinary comparison oracle.
        let function = program
            .functions()
            .iter()
            .find(|function| function.id.as_str() == "probe")
            .unwrap();
        let old = probe(&program, function, true, 64 << 20);
        let current = probe(&program, function, false, 64 << 20);
        assert!(old.0 && current.0 && old.3.allocated > 0 && current.3.allocated > 0);
        assert_eq!(current.1, old.1 + memo_transport + paths_transport);
        let [Some(before_memo), Some(before_paths), Some(after_paths)] = current.2 else {
            panic!("missing actual release frontiers")
        };
        assert_eq!(before_paths - before_memo, memo_transport);
        assert_eq!(after_paths - before_paths, paths_transport);
        assert!(probe(&program, function, false, current.1).0);
        assert!(!probe(&program, function, false, current.1 - 1).0);
        for (limit, memo_passed, paths_passed) in [
            (before_memo + memo_transport - 1, false, false),
            (before_memo + memo_transport, true, false),
            (before_paths + paths_transport - 1, true, false),
            (before_paths + paths_transport, true, true),
        ] {
            let attempt = probe(&program, function, false, limit);
            assert_eq!(attempt.2[1].is_some(), memo_passed);
            assert_eq!(attempt.2[2].is_some(), paths_passed);
        }
        // Includes the ordinary DAG destructor's existing pending Vec. All new
        // transports remain spent; this is not a complete-funding/peak claim.
        eprintln!(
            "OPT903 function={} old_paid={} current_paid={} old={:?} current={:?}",
            function.id, old.1, current.1, old.3, current.3
        );
    }
}
