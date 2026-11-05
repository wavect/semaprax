//! Independent small multiset and physical storage oracles; no strategy claim.
use super::*;

fn fixture() -> ResolvedProgram {
    let source = "module dag.storage; @id(\"dag.main\") fn main()->i64 {0}";
    let parsed = crate::check(source, "dag-storage.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}

/// Expand only small authored DAGs. No automaton, weighted subset construction
/// or canonicalizer is used, and duplicates remain ordinary vector members.
fn multiset(dag: &ObservationDag) -> Vec<Vec<Observation>> {
    match &dag.kind {
        ObservationDagKind::Seq(prefix, items) => {
            let mut paths = prefix
                .as_ref()
                .map_or_else(|| vec![vec![]], |prefix| multiset(prefix));
            for path in &mut paths {
                path.extend_from_slice(items);
            }
            paths
        }
        ObservationDagKind::Alt(parts) => parts.iter().flat_map(|part| multiset(part)).collect(),
        ObservationDagKind::Cat(parts) => {
            let mut paths = vec![vec![]];
            for part in parts {
                let suffixes = multiset(part);
                paths = paths
                    .into_iter()
                    .flat_map(|prefix| {
                        suffixes.iter().map(move |suffix| {
                            let mut path = prefix.clone();
                            path.extend_from_slice(suffix);
                            path
                        })
                    })
                    .collect();
            }
            paths
        }
    }
}

#[test]
fn precharged_union_retains_duplicate_empty_alternatives() {
    let program = fixture();
    let function = program.functions().get(0).expect("current replay ordinal");
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let union = work
        .union_observations(vec![Observations::default(), Observations::default()])
        .unwrap();
    assert_eq!(multiset(union.prefix.as_ref().unwrap()).len(), 2);
    let mut concatenated = union.clone();
    work.extend_factored_observations(&mut concatenated, &union)
        .unwrap();
    assert_eq!(multiset(concatenated.prefix.as_ref().unwrap()).len(), 4);
}

fn storage_probe(
    program: &ResolvedProgram,
    limit: usize,
) -> (
    Result<usize, Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let function = program.functions().get(0).expect("current replay ordinal");
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    // Existing legacy index construction belongs to its own admission owner;
    // this isolated observer measures only the new DAG storage operations.
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let before = work.budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let mut root = work.charged_dag(ObservationDagKind::Seq(None, Vec::new()))?;
        for _ in 0..1024 {
            work.budget.reserve_array::<Rc<ObservationDag>>(
                function,
                2,
                "test duplicate DAG members",
            )?;
            root = work.charged_dag(ObservationDagKind::Alt(vec![root.clone(), root]))?;
        }
        // Include all destruction in the physical oracle; the scratch is shared
        // with `work`, so explicitly drop its owner inside the measured region.
        drop(root);
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        Ok(work.budget.physical.as_ref().unwrap().charged() - before)
    })
}

#[test]
fn deep_shared_dag_creation_and_drop_fit_the_precharged_frontier() {
    let program = fixture();
    let (result, observed) = storage_probe(&program, 1 << 24);
    let charged = result.unwrap();
    assert!(!observed.invalid);
    assert!(observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    // Obtain the existing budget's exact held header independently rather than
    // guessing the Box carrier layout used by the enclosing admission owner.
    let function = program.functions().get(0).expect("current replay ordinal");
    let baseline = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (exact, observed) = storage_probe(&program, admission + charged);
    assert_eq!(exact.unwrap(), charged);
    assert!(observed.allocated <= charged && observed.highwater <= charged && observed.live == 0);
    let (short, _) = storage_probe(&program, admission + charged - 1);
    assert!(
        short.is_err(),
        "one-short storage must refuse before final allocation"
    );
}

#[test]
fn merge_keys_preserve_pending_owned_commits_and_failure_classes() {
    let program = fixture();
    let function = program.functions().get(0).expect("current replay ordinal");
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let normal = empty_expr_path();
    let mut failed = empty_expr_path();
    failed.failed = true;
    let mut residual = empty_expr_path();
    residual.residual = true;
    let mut owned = empty_expr_path();
    owned.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
    let paths = work
        .merged_paths(vec![normal.clone(), failed, residual, owned])
        .unwrap();
    assert_eq!(paths.len(), 4);
    let commits = vec![(0, CleanupPlace::whole(StorageId::ProvisionalResult))];
    let states = work
        .merged_call_states(vec![(normal.clone(), vec![]), (normal, commits)])
        .unwrap();
    assert_eq!(
        states.len(),
        2,
        "pending ownership commits must never coalesce"
    );
}

#[test]
fn lexical_failure_and_abrupt_duplicates_stay_separate() {
    let source = "module separate.lexical; @id(\"probe\") fn probe(n:i64)->i64 {let owner=\"prefix\"; let mut i=0; while i<1 {i=i+1; if n==0 {return 1;}; 0} return 0;} @id(\"main\") fn main()->i64 {0}";
    let parsed = crate::check(source, "separate-lexical.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let function = program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let mut failed = empty_expr_path();
    failed.failed = true;
    let merged = work.merged_paths(vec![failed.clone(), failed]).unwrap();
    assert_eq!(
        merged.len(),
        2,
        "failure multiplicity is carried by separate paths first"
    );
    let normal = empty_expr_path();
    let merged = work
        .merged_paths(vec![normal.clone(), normal.clone()])
        .unwrap();
    assert_eq!(
        merged.len(),
        2,
        "normal paths outside a Copy scope stay separate"
    );
    // This isolated merge oracle supplies the independently owned region scope.
    work.active_copy_region = Some(function.body.id.clone());
    let merged = work.merged_paths(vec![normal.clone(), normal]).unwrap();
    assert_eq!(merged.len(), 1);
    let commits = vec![(0, CleanupPlace::whole(StorageId::ProvisionalResult))];
    let normal = empty_expr_path();
    let calls = work
        .merged_call_states(vec![(normal.clone(), commits.clone()), (normal, commits)])
        .unwrap();
    assert_eq!(
        calls.len(),
        2,
        "pending-own epochs stay separate even if they match"
    );
}

#[test]
fn sibling_suffixes_share_one_authentic_prefix_without_path_state_merging() {
    let program = fixture();
    let function = program.functions().get(0).expect("current replay ordinal");
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.remaining = 0;
    budget.lent_skeleton = true;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let mut prefix = empty_expr_path();
    let expression = work
        .clone_owned(&function.body.id, "test prefix identity")
        .unwrap();
    work.push_observation(
        &mut prefix,
        SkeletonObservation::Boolean {
            expression,
            value: true,
        },
        "test prefix",
    )
    .unwrap();
    let expected = prefix.observations.items.clone();
    let union = work
        .union_observations(vec![Observations::default(), Observations::default()])
        .unwrap();
    let mut normal = empty_expr_path();
    normal.observations = union.clone();
    let mut failed = empty_expr_path();
    failed.observations = union;
    failed.failed = true;
    let result = sequence_skeleton_paths(vec![prefix], &[normal, failed], &mut work).unwrap();
    assert_eq!(result.len(), 2);
    assert!(!result[0].failed && result[1].failed);
    let head = |path: &ExprSkeletonPath| {
        let root = path.observations.prefix.as_ref().unwrap();
        let ObservationDagKind::Cat(parts) = &root.kind else {
            panic!("concatenation")
        };
        parts[0].clone()
    };
    assert!(Rc::ptr_eq(&head(&result[0]), &head(&result[1])));
    for path in &result {
        assert_eq!(
            multiset(path.observations.prefix.as_ref().unwrap()),
            vec![expected.clone(), expected.clone()]
        );
    }
}

fn shared_prefix_probe(
    program: &ResolvedProgram,
    physical: usize,
    work_limit: usize,
) -> (
    Result<(usize, usize), ()>,
    crate::test_allocation_observer::Metrics,
) {
    let function = program.functions().get(0).expect("current replay ordinal");
    let mut budget = ReplayBudget::with_physical(function, physical).unwrap();
    budget.remaining = 0;
    budget.skeleton_remaining = work_limit;
    budget.lent_skeleton = true;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let before = work.budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let result = (|| {
            let mut prefix = empty_expr_path();
            let expression = work.clone_owned(&function.body.id, "test prefix identity")?;
            work.push_observation(
                &mut prefix,
                SkeletonObservation::Boolean {
                    expression,
                    value: true,
                },
                "test prefix",
            )?;
            work.share_prefix_tail(&mut prefix.observations)?;
            // Repeated siblings reuse the same moved tail instead of rebuilding it.
            for _ in 0..32 {
                let sibling = work.clone_observations(&prefix.observations, "test sibling")?;
                assert!(Rc::ptr_eq(
                    prefix.observations.prefix.as_ref().unwrap(),
                    sibling.prefix.as_ref().unwrap()
                ));
                drop(sibling);
            }
            drop(prefix);
            Ok::<(), Diagnostic>(())
        })();
        // Release the session scratch inside observation even on refusal.
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        match result {
            Ok(()) => Ok((
                work.budget.physical.as_ref().unwrap().charged() - before,
                work_limit - work.budget.skeleton_remaining,
            )),
            Err(diagnostic) => {
                assert_eq!(diagnostic.code, "SPX-H006");
                // Diagnostics own text. Release it within the observed scope,
                // just like the DAG scratch, rather than retaining it in the
                // allocation observer's allocation-free refusal result.
                drop(diagnostic);
                Err(())
            }
        }
    })
}
#[test]
fn shared_prefix_creation_through_drop_has_exact_work_and_backing_limits() {
    let program = fixture();
    let (result, observed) = shared_prefix_probe(&program, 1 << 20, MAX_REPLAY_WORK_UNITS);
    let (charged, spent) = result.unwrap();
    assert!(
        !observed.invalid
            && observed.live == 0
            && observed.allocated <= charged
            && observed.highwater <= charged
    );
    let baseline = ReplayBudget::with_physical(
        program.functions().get(0).expect("current replay ordinal"),
        1 << 20,
    )
    .unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (result, observed) = shared_prefix_probe(&program, admission + charged, spent);
    assert_eq!(result.unwrap(), (charged, spent));
    assert!(
        !observed.invalid
            && observed.live == 0
            && observed.allocated <= charged
            && observed.highwater <= charged
    );
    for (physical, work) in [
        (admission + charged - 1, spent),
        (admission + charged, spent - 1),
    ] {
        let (result, observed) = shared_prefix_probe(&program, physical, work);
        assert!(result.is_err());
        assert!(!observed.invalid && observed.live == 0);
        assert!(observed.allocated <= charged && observed.highwater <= charged);
    }
}

#[test]
fn selected_refusal_backing_is_prepaid_sticky_and_observed_through_drop() {
    let program = fixture();
    let function = program.functions().get(0).expect("current replay ordinal");
    let (result, observed) = crate::test_allocation_observer::measure(|| {
        let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
        let paid = budget.physical.as_ref().unwrap().charged();
        let first = budget
            .reserve_bytes(function, 1 << 20, "hostile physical refusal")
            .unwrap_err();
        assert_eq!(first.code, "SPX-H006");
        assert!(first
            .message
            .contains("physical reservation exhausted during hostile physical refusal"));
        drop(first);
        let after = budget.physical.as_ref().unwrap().charged();
        assert_eq!(after, paid);
        assert_eq!(
            budget.reserve_bytes(function, 0, "retry").unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(
            budget.charge(function, 0, "retry work").unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(
            budget.reserve_skeleton(function, 0).unwrap_err().code,
            "SPX-H006"
        );
        drop(budget);
        paid
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(
        observed.allocations, 2,
        "one paid text buffer and one reservation owner"
    );
    assert_eq!(
        observed.allocated,
        std::mem::size_of::<super::super::super::physical_storage::PhysicalReservation>()
            + function.id.as_str().len()
            + 512
    );
    assert!(observed.allocated <= result && observed.highwater <= result);
}

mod tail_moves;
