//! Complete immediate-old/current replay over the same checked fixtures.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"
module boxed;
@id("f") fn f(flag:bool)->i64 {
    let scratch=bytes_zeroed(3usize);
    let view=bytes_as_slice(scratch);
    if flag && byte_len(view)==3usize {7}else{9}
}
@id("main") fn main()->i64 {f(false)}
"#;

fn program() -> ResolvedProgram {
    let checked = crate::check(SOURCE, "singleton-path-group.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "singleton-path-group.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn replay(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    if old {
        before_final_edge::validate_path_states(
            program, function, &storage, &leaves, budget, None, None,
        )
    } else {
        validate_path_states(program, function, &storage, &leaves, budget, None, None)
    }
}

#[test]
fn final_edge_move_keeps_complete_immediate_work_and_hostile_diagnostic() {
    let program = program();
    for function in &program.functions {
        let original_plan = function.cleanup_plan.clone();
        let mut full_old = ReplayBudget::unobserved();
        replay(&program, function, true, &mut full_old).unwrap();
        let work = MAX_REPLAY_WORK_UNITS - full_old.remaining;
        for limit in [
            0,
            1,
            2,
            7,
            31,
            127,
            work.saturating_sub(1),
            work,
            MAX_REPLAY_WORK_UNITS,
        ] {
            let mut old = ReplayBudget::unobserved();
            let mut current = ReplayBudget::unobserved();
            old.remaining = limit;
            current.remaining = limit;
            let expected = replay(&program, function, true, &mut old);
            let actual = replay(&program, function, false, &mut current);
            assert_eq!(actual, expected, "function={} work={limit}", function.id);
            assert_eq!(current.remaining, old.remaining);
            assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
            assert_eq!(function.cleanup_plan, original_plan);
        }
    }
    // Direct low-level callers retain the old generic behavior even when
    // the enclosing production CFG certificate would reject this cardinality.
    let original = program.functions.iter().find(|f| f.name == "f").unwrap();
    for count in [0, 1, 3] {
        let mut malformed = original.clone();
        let edges = malformed
            .cleanup_plan
            .blocks
            .iter_mut()
            .find_map(|block| {
                if let CleanupTerminator::Branch(edges) = &mut block.terminator {
                    Some(edges)
                } else {
                    None
                }
            })
            .expect("fixture owns a checked two-edge branch");
        let first = edges[0];
        edges.resize(count, first);
        let before = malformed.cleanup_plan.clone();
        let mut old = ReplayBudget::unobserved();
        let mut current = ReplayBudget::unobserved();
        assert_eq!(
            replay(&program, &malformed, false, &mut current),
            replay(&program, &malformed, true, &mut old)
        );
        assert_eq!(current.remaining, old.remaining);
        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
        assert_eq!(malformed.cleanup_plan, before);
    }
    let mut forged = program
        .functions
        .iter()
        .find(|f| f.name == "f")
        .unwrap()
        .clone();
    let block = forged
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .any(|t| matches!(t, CleanupTransition::Initialize { .. }))
        })
        .expect("original owned fixture initializes storage");
    let index = block
        .transitions
        .iter()
        .position(|t| matches!(t, CleanupTransition::Initialize { .. }))
        .unwrap();
    block
        .transitions
        .insert(index + 1, block.transitions[index].clone());
    let original_plan = forged.cleanup_plan.clone();
    let mut old = ReplayBudget::unobserved();
    let mut current = ReplayBudget::unobserved();
    let expected = replay(&program, &forged, true, &mut old).unwrap_err();
    let actual = replay(&program, &forged, false, &mut current).unwrap_err();
    assert_eq!(expected.code, "SPX-H006");
    assert_eq!(actual, expected); // all six Diagnostic fields, not only its code
    assert_eq!(current.remaining, old.remaining);
    assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
    assert_eq!(forged.cleanup_plan, original_plan);
}

#[test]
fn final_edge_move_reports_whole_immediate_costs_and_exact_short_zero_drop() {
    let program = program();
    let function = program.functions.iter().find(|f| f.name == "f").unwrap();
    let observe = |old, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = replay(&program, function, old, &mut budget);
                let success = result.is_ok();
                if let Err(error) = &result {
                    assert_eq!(error.code, "SPX-H006");
                }
                drop(result);
                let before_drop = crate::test_allocation_observer::current_metrics().allocated;
                drop(budget);
                assert_eq!(
                    crate::test_allocation_observer::current_metrics().allocated,
                    before_drop
                );
                success
            })
        })
    };
    let old = observe(true, usize::MAX);
    let current = observe(false, usize::MAX);
    assert!(old.0 .0 && current.0 .0 && !old.1 && !current.1);
    assert!(old.3.is_none() && current.3.is_none());
    eprintln!("final-edge-state-move whole immediate-old/current: old_paid={} old={:?}; current_paid={} current={:?}", old.2, old.0.1, current.2, current.0.1);
    for observed in [&old, &current] {
        let allocation = &observed.0 .1;
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= observed.2);
    }
    assert!(current.0 .1.allocations < old.0 .1.allocations);
    assert!(current.0 .1.allocated < old.0 .1.allocated);
    let exact = current.2;
    for limit in [0, exact - 1, exact] {
        let result = observe(false, limit);
        assert_eq!(result.0 .0, limit == exact);
        assert_eq!(result.1, limit != exact);
        assert_eq!(result.3.is_some(), limit != exact);
        let allocation = result.0 .1;
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= result.2 && result.2 <= limit);
        if limit == exact {
            assert_eq!(result.2, exact);
        }
        if limit == 0 {
            assert_eq!(allocation.allocated, 0);
        }
    }
}
