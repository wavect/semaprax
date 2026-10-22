//! Full flat producer controls: authored here, run by the integrated owner.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

pub(super) fn program() -> ResolvedProgram {
    let source = "module history; @id(\"f\") fn f(a:bool,b:bool,c:bool)->i64 {let n=if a {1}else{2};let m=if b {n+1}else{n+2};if c {m+1}else{m+2}} @id(\"main\") fn main()->i64 {f(true,false,true)}";
    let checked = crate::check(source, "history.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "history.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn budget(program: &ResolvedProgram, function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    budget
}

#[test]
fn complete_old_plan_and_materialization_work_are_preserved() {
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = budget(&program, function);
                let mut new = budget(&program, function);
                let start = skeleton_materializations();
                let mut flat = before::produce(function, &mut old).unwrap();
                let materializations = skeleton_materializations() - start;
                let start = skeleton_materializations();
                let actual = produce(function, &mut new).unwrap();
                assert_eq!(skeleton_materializations() - start, materializations);
                assert_eq!(new.skeleton_spent, old.skeleton_spent);
                assert_eq!(new.skeleton_remaining, old.skeleton_remaining);
                assert!(new.remaining <= old.remaining);
                flat.sort();
                compare(function, &mut new, actual, &flat).unwrap();
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid);
    assert!(
        paid >= allocation.allocated,
        "paid {paid}, allocated {}",
        allocation.allocated
    );
}

#[test]
fn late_changes_terminal_and_multiplicity_fail_closed() {
    let program = program();
    with_limit_usage_refusal(usize::MAX, || {
        let function = &program.functions[0];
        let mut old = budget(&program, function);
        let mut flat = before::produce(function, &mut old).unwrap();
        flat.sort();
        assert!(flat.len() > 1);
        for mutation in 0..4 {
            let mut expected = flat.clone();
            match mutation {
                0 => {
                    expected.pop();
                }
                1 => expected.push(expected.last().unwrap().clone()),
                2 => expected.last_mut().unwrap().terminal = SkeletonTerminal::Failure,
                _ => {
                    expected.last_mut().unwrap().observations = Vec::new().into();
                }
            }
            expected.sort();
            let mut new = budget(&program, function);
            let actual = produce(function, &mut new).unwrap();
            let error = compare(function, &mut new, actual, &expected).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
        }
    });
}

#[test]
fn distinct_segment_boundaries_order_by_complete_values() {
    let program = program();
    with_limit_usage_refusal(usize::MAX, || {
        let function = &program.functions[0];
        let mut old = budget(&program, function);
        let mut expected = before::produce(function, &mut old).unwrap();
        expected.sort();
        // The same complete old values, reallocated into distinct Rc owners and
        // split one item per segment. Duplicates remain separate result paths.
        let mut arena = Vec::new();
        let mut paths = Vec::new();
        let mut max_depth = 0;
        for path in &expected {
            let mut previous = None;
            let mut depth = 0;
            for observation in path.observations.iter() {
                depth += 1;
                let index = arena.len();
                arena.push(Segment {
                    previous,
                    items: vec![Rc::new((**observation).clone())],
                    depth,
                });
                previous = Some(index);
            }
            max_depth = max_depth.max(depth);
            paths.push(PlanPath {
                history: History {
                    previous,
                    tail: Vec::new(),
                },
                terminal: path.terminal,
            });
        }
        paths.reverse();
        let mut new = budget(&program, function);
        compare(
            function,
            &mut new,
            HistorySet {
                arena,
                paths,
                max_depth,
            },
            &expected,
        )
        .unwrap();
    });
}

#[test]
fn first_skeleton_refusal_and_sticky_zero_are_preserved() {
    let program = program();
    with_limit_usage_refusal(usize::MAX, || {
        let function = &program.functions[0];
        for limit in 0..12 {
            let mut old = ReplayBudget::with_skeleton_limit(limit);
            let mut new = ReplayBudget::with_skeleton_limit(limit);
            let first = before::produce(function, &mut old).err();
            let second = produce(function, &mut new).err();
            assert_eq!(first, second);
            assert_eq!(new.skeleton_spent, old.skeleton_spent);
            assert!(new.reserve_bytes(function, 0, "sticky zero").is_err());
        }
    });
}

#[test]
fn retained_unobserved_and_lent_owners_are_excluded() {
    let program = program();
    let function = &program.functions[0];
    assert!(!eligible(function, &ReplayBudget::unobserved()));
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = budget(&program, function);
        assert!(eligible(function, &budget));
        budget.lent_skeleton = true;
        assert!(!eligible(function, &budget));
        budget.lent_skeleton = false;
        budget.merge_paths = true;
        assert!(!eligible(function, &budget));
        budget.merge_paths = false;
        budget.admission = AdmissionMode::SelectedRetained;
        assert!(!eligible(function, &budget));
    });
}

#[test]
fn full_comparator_exact_one_short_and_zero_leave_no_live_backing() {
    let program = program();
    let function = &program.functions[0];
    let mut old = ReplayBudget::unobserved();
    reserve_program_skeleton_work(&program, std::iter::once(function), &mut old).unwrap();
    let mut expected = before::produce(function, &mut old).unwrap();
    expected.sort();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result =
                    reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                        .and_then(|_reserved_slots| produce(function, &mut budget))
                        .and_then(|actual| compare(function, &mut budget, actual, &expected));
                let success = result.is_ok();
                drop(result);
                drop(budget);
                success
            })
        })
    };
    let ((success, allocation), refused, paid, first) = observe(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    let ((success, allocation), refused, exact_paid, first) = observe(paid);
    assert!(success && !refused && first.is_none());
    assert_eq!(exact_paid, paid);
    assert!(!allocation.invalid && allocation.live == 0);
    for limit in [0, paid - 1] {
        let ((success, allocation), refused, _, first) = observe(limit);
        assert!(!success && refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn malformed_private_links_fail_without_recursive_drop_or_indexing() {
    let program = program();
    with_limit_usage_refusal(usize::MAX, || {
        let function = &program.functions[0];
        for parent in [0, usize::MAX] {
            let mut budget = budget(&program, function);
            let actual = HistorySet {
                arena: vec![Segment {
                    previous: Some(parent),
                    items: Vec::new(),
                    depth: 1,
                }],
                paths: vec![PlanPath {
                    history: History {
                        previous: Some(0),
                        tail: Vec::new(),
                    },
                    terminal: SkeletonTerminal::Success,
                }],
                max_depth: 1,
            };
            let expected = vec![SkeletonPath {
                observations: Vec::new().into(),
                terminal: SkeletonTerminal::Success,
            }];
            assert_eq!(
                compare(function, &mut budget, actual, &expected)
                    .unwrap_err()
                    .code,
                "SPX-H006"
            );
        }
    });
}

#[test]
fn ordinary_default_retains_flat_verifier_and_candidate_selection_is_scoped() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut normal = budget(&program, function);
        assert!(eligible(function, &normal));
        assert!(!select(function, &mut normal).unwrap());
        {
            let _candidate = enable_for_test();
            let mut selected = budget(&program, function);
            assert!(select(function, &mut selected).unwrap());
            {
                let _nested = enable_for_test();
                let mut selected = budget(&program, function);
                assert!(select(function, &mut selected).unwrap());
            }
            let mut selected = budget(&program, function);
            assert!(select(function, &mut selected).unwrap());
            assert!(!select(function, &mut ReplayBudget::unobserved()).unwrap());
        }
        let mut normal = budget(&program, function);
        assert!(!select(function, &mut normal).unwrap());
    });
}
