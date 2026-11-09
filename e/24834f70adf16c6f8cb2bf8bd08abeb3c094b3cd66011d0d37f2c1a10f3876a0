//! Joint loans use the original complete comparator and genuine HIR final paths.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use skeleton_work::{enumerated_prefix, ObservationDagKind, SkeletonWork};

fn budget(program: &ResolvedProgram, function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    budget
}

fn shared_expected(program: &ResolvedProgram, function: &ResolvedFunction) -> Vec<SkeletonPath> {
    let mut budget = budget(program, function);
    // This is the actual HIR finalizer, not a projection of the new PLAN arena.
    let mut paths = hir_skeleton_paths(program, function, &mut budget).unwrap();
    let mut work = SkeletonWork::new(program, function, &mut budget);
    for path in &mut paths {
        assert!(path.observations.prefix.is_none(), "closed flat fixture");
        let count = path.observations.items.len();
        let cut = count / 2;
        work.budget
            .reserve_bytes(
                function,
                enumerated_prefix::SHARE_CONTROLS,
                "joint HIR head kind",
            )
            .unwrap();
        work.budget
            .reserve_array::<Observation>(function, cut, "joint HIR head items")
            .unwrap();
        let head = enumerated_prefix::node(
            &mut work,
            ObservationDagKind::Seq(None, path.observations.items[..cut].to_vec()),
        )
        .unwrap();
        work.budget
            .reserve_bytes(
                function,
                enumerated_prefix::SHARE_CONTROLS,
                "joint HIR tail kind",
            )
            .unwrap();
        work.budget
            .reserve_array::<Observation>(function, count - cut, "joint HIR tail items")
            .unwrap();
        let tail = enumerated_prefix::node(
            &mut work,
            ObservationDagKind::Seq(None, path.observations.items[cut..].to_vec()),
        )
        .unwrap();
        work.budget
            .reserve_array::<Rc<skeleton_work::ObservationDag>>(
                function,
                2,
                "joint HIR Cat children",
            )
            .unwrap();
        work.budget
            .reserve_bytes(
                function,
                enumerated_prefix::SHARE_CONTROLS,
                "joint HIR Cat kind",
            )
            .unwrap();
        path.observations.prefix = Some(
            enumerated_prefix::node(&mut work, ObservationDagKind::Cat(vec![head, tail])).unwrap(),
        );
        path.observations.items.clear();
    }
    drop(work);
    // Deliberately destroy construction order; only complete content + terminal
    // decides the original HIR sort. The two arenas share no indices or nodes.
    paths.reverse();
    enumerated_prefix::sort(&mut paths, function, &mut budget).unwrap();
    paths
}

fn joint(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expected: &[SkeletonPath],
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    reserve_program_skeleton_work(program, std::iter::once(function), budget)?;
    paid!(budget, function, PRODUCE_CALL, "joint PLAN produce caller")?;
    let produced = produce(function, budget)?;
    paid!(budget, function, COMPARE_CALL, "joint PLAN compare caller")?;
    compare(produced, expected, budget)
}

#[test]
fn public_and_funded_joint_loans_consume_complete_seq_cat_hir() {
    let program = super::super::super::tests::program();
    for function in &program.functions {
        let expected = shared_expected(&program, function);
        // Keep both independent complete predecessors: the whole old flat PLAN
        // producer and the whole immediate old comparison owner on flat HIR.
        let mut old_budget = budget(&program, function);
        let mut old_plan = before::produce(function, &mut old_budget).unwrap();
        let mut hir_budget = budget(&program, function);
        let mut old_hir = hir_skeleton_paths(&program, function, &mut hir_budget).unwrap();
        old_plan.sort();
        old_hir.sort();
        assert_eq!(old_plan, old_hir);
        let mut old_budget = budget(&program, function);
        let produced = produce(function, &mut old_budget).unwrap();
        super::super::super::before_comparison::compare(
            function,
            &mut old_budget,
            produced.set,
            &old_hir,
        )
        .unwrap();
        let mut public = ReplayBudget::new();
        assert_eq!(public.payer, AllocationPayer::LegacyUnobserved);
        joint(&program, function, &expected, &mut public).unwrap();
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut actual = ReplayBudget::new();
                assert_eq!(actual.payer, AllocationPayer::ActiveBuilder);
                joint(&program, function, &expected, &mut actual).unwrap();
            })
        });
        assert!(!refused && first.is_none() && !allocation.invalid);
        assert!(paid >= allocation.allocated);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn joint_late_shared_content_terminal_and_multiplicity_refuse() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let expected = shared_expected(&program, function);
    assert!(expected.len() > 1);
    for mutation in 0..4 {
        let mut changed = expected.clone();
        match mutation {
            0 => {
                changed.pop();
            }
            1 => changed.push(changed.last().unwrap().clone()),
            2 => changed.last_mut().unwrap().terminal = SkeletonTerminal::LoopLatch,
            _ => {
                // Preserve the entire shared prefix, then add a late complete
                // observation. A marker/index/first-value comparison misses it.
                changed.last_mut().unwrap().observations.items.push(Rc::new(
                    SkeletonObservation::Boolean {
                        expression: function.body.id.clone(),
                        value: false,
                    },
                ));
            }
        }
        let mut budget = budget(&program, function);
        enumerated_prefix::sort(&mut changed, function, &mut budget).unwrap();
        let error = joint(&program, function, &changed, &mut budget).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("disagrees with typed HIR"));
    }
}

#[test]
fn joint_complete_drop_exact_short_zero_and_unsupported_callers() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let expected = shared_expected(&program, function);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = joint(&program, function, &expected, &mut budget);
                let success = result.is_ok();
                if !success {
                    assert!(budget
                        .reserve_bytes(function, 0, "joint sticky zero")
                        .is_err());
                }
                success
            })
        })
    };
    let ((success, allocation), refused, paid, first) = run(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    let ((success, allocation), refused, exact, first) = run(paid);
    assert!(success && !refused && first.is_none());
    assert_eq!(exact, paid);
    assert!(!allocation.invalid && allocation.live == 0);
    for limit in [0, paid - 1] {
        let ((success, allocation), refused, _, first) = run(limit);
        assert!(!success && refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    for excluded in 0..3 {
        let mut budget = budget(&program, function);
        let produced = produce(function, &mut budget).unwrap();
        match excluded {
            0 => budget.admission = AdmissionMode::SelectedRetained,
            1 => budget.lent_skeleton = true,
            _ => budget.merge_paths = true,
        }
        let error = compare(produced, &expected, &mut budget).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("requires its ordinary caller"));
    }
}
