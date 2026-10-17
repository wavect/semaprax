//! Small complete-language oracle and cumulative storage probes for OPT878.
use super::*;

fn fixture(lexical: bool) -> ResolvedProgram {
    let source = if lexical {
        "module keys.lexical; @id(\"main\") fn main()->i64 {let s=\"held\"; let mut i=0; while i<1 {i=i+1; if i==1 {return 1;}; 0} return 0;}"
    } else {
        "module keys.merge; @id(\"main\") fn main()->i64 {0}"
    };
    let parsed = crate::check(source, "merge-keys.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}

fn expand(observations: &Observations) -> Vec<Vec<Observation>> {
    fn dag(node: &ObservationDag) -> Vec<Vec<Observation>> {
        match &node.kind {
            ObservationDagKind::Seq(prefix, items) => {
                let mut values = prefix.as_ref().map_or_else(|| vec![vec![]], |p| dag(p));
                for value in &mut values {
                    value.extend_from_slice(items);
                }
                values
            }
            ObservationDagKind::Alt(parts) => parts.iter().flat_map(|p| dag(p)).collect(),
            ObservationDagKind::Cat(parts) => {
                let mut values = vec![vec![]];
                for part in parts {
                    let suffixes = dag(part);
                    values = values
                        .into_iter()
                        .flat_map(|prefix| {
                            suffixes.iter().map(move |suffix| {
                                let mut value = prefix.clone();
                                value.extend_from_slice(suffix);
                                value
                            })
                        })
                        .collect();
                }
                values
            }
        }
    }
    let mut values = observations
        .prefix
        .as_ref()
        .map_or_else(|| vec![vec![]], |p| dag(p));
    for value in &mut values {
        value.extend_from_slice(&observations.items);
    }
    values
}

pub(in crate::cleanup_plan::replay) type Snapshot = (
    Vec<Vec<Observation>>,
    Option<CleanupPlace>,
    bool,
    bool,
    lexical_skeleton::Continuation,
    Vec<(u32, CleanupPlace)>,
);
pub(in crate::cleanup_plan::replay) fn snapshot(states: Vec<CallSkeletonState>) -> Vec<Snapshot> {
    states
        .into_iter()
        .map(|(p, commits)| {
            (
                expand(&p.observations),
                p.owned_source,
                p.failed,
                p.residual,
                p.continuation,
                commits,
            )
        })
        .collect()
}

fn inputs(function: &ResolvedFunction, count: usize) -> Vec<CallSkeletonState> {
    (0..count)
        .map(|i| {
            // Deliberately non-sorted first appearance and duplicates with different
            // observations. Commit order is content, never a set or repaired order.
            let mut path = empty_expr_path();
            path.failed = i % 7 == 2;
            path.residual = i % 7 == 3;
            if i % 7 == 4 {
                path.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
            }
            path.observations
                .items
                .push(std::rc::Rc::new(SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: i % 2 == 0,
                }));
            let place = CleanupPlace::whole(StorageId::ProvisionalResult);
            let commits = match i % 7 {
                5 => vec![(0, place.clone()), (1, place)],
                6 => vec![(1, place.clone()), (0, place)],
                _ => vec![],
            };
            (path, commits)
        })
        .collect()
}

#[test]
fn borrowed_merge_keys_match_complete_before_languages_order_and_work() {
    for lexical in [false, true] {
        let program = fixture(lexical);
        let function = program.functions().get(0).unwrap();
        for selected in [false, true] {
            for count in [0, 1, 2, 7, 31, 64] {
                for calls in [false, true] {
                    let mut outcomes = Vec::new();
                    for before in [true, false] {
                        let mut budget = if selected {
                            ReplayBudget::with_physical(function, 1 << 24).unwrap()
                        } else {
                            ReplayBudget::new()
                        };
                        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                        budget.merge_paths = true;
                        let mut work = SkeletonWork::new(&program, function, &mut budget);
                        if lexical {
                            work.active_copy_region = Some(function.body.id.clone());
                        }
                        let states = inputs(function, count);
                        let states = if calls {
                            if before {
                                work.before_merged_call_states(states)
                            } else {
                                work.merged_call_states(states)
                            }
                            .unwrap()
                        } else {
                            let paths = states.into_iter().map(|(p, _)| p).collect();
                            let paths = if before {
                                work.before_merged_paths(paths)
                            } else {
                                work.merged_paths(paths)
                            }
                            .unwrap();
                            paths.into_iter().map(|p| (p, vec![])).collect()
                        };
                        outcomes.push((snapshot(states), work.budget.skeleton_remaining));
                    }
                    assert_eq!(
                        outcomes[0], outcomes[1],
                        "lexical={lexical} selected={selected} count={count} calls={calls}"
                    );
                }
            }
        }
    }
}

#[test]
fn borrowed_merge_keys_keep_original_work_refusal_before_new_storage() {
    let program = fixture(false);
    let function = program.functions().get(0).unwrap();
    for calls in [false, true] {
        let mut errors = Vec::new();
        for before in [true, false] {
            let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
            budget.skeleton_remaining = 0;
            budget.merge_paths = true;
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let charged = work.budget.physical.as_ref().unwrap().charged();
            let states = inputs(function, 7);
            let error = if calls {
                if before {
                    work.before_merged_call_states(states)
                } else {
                    work.merged_call_states(states)
                }
                .err()
                .unwrap()
            } else {
                let paths = states.into_iter().map(|(p, _)| p).collect();
                if before {
                    work.before_merged_paths(paths)
                } else {
                    work.merged_paths(paths)
                }
                .err()
                .unwrap()
            };
            assert_eq!(charged, work.budget.physical.as_ref().unwrap().charged());
            errors.push(error);
        }
        assert_eq!(errors[0], errors[1], "all six original diagnostic fields");
    }
}

#[test]
fn borrowed_suffix_route_retains_complete_before_multiplicity() {
    let program = fixture(false);
    let function = program.functions().get(0).unwrap();
    let mut outcomes = Vec::new();
    for before in [true, false] {
        let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        budget.merge_paths = true;
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let paths: Vec<_> = inputs(function, 31).into_iter().map(|(p, _)| p).collect();
        let suffixes = if before {
            work.before_merged_suffixes(&paths)
        } else {
            work.merged_suffixes(&paths)
        }
        .unwrap();
        outcomes.push(snapshot(
            suffixes
                .into_owned()
                .into_iter()
                .map(|p| (p, vec![]))
                .collect(),
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
}

fn storage_run(
    program: &ResolvedProgram,
    limit: usize,
    calls: bool,
    before: bool,
) -> (
    Result<usize, Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let function = program.functions().get(0).unwrap();
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let charged = work.budget.physical.as_ref().unwrap().charged();
    let mut states = inputs(function, 31);
    let paths = if calls {
        Vec::new()
    } else {
        std::mem::take(&mut states)
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    };
    crate::test_allocation_observer::measure(|| {
        let result = if calls {
            if before {
                work.before_merged_call_states(states)
            } else {
                work.merged_call_states(states)
            }
            .map(drop)
        } else {
            if before {
                work.before_merged_paths(paths)
            } else {
                work.merged_paths(paths)
            }
            .map(drop)
        };
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        result.map(|()| work.budget.physical.as_ref().unwrap().charged() - charged)
    })
}

#[test]
fn borrowed_merge_directory_pays_storage_and_every_refusal_through_drop() {
    let program = fixture(false);
    let function = program.functions().get(0).unwrap();
    // Setup is held before the measured operation, as in the existing DAG owner.
    let mut setup = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    setup.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    setup.merge_paths = true;
    let work = SkeletonWork::new(&program, function, &mut setup);
    let admission = work.budget.physical.as_ref().unwrap().charged();
    drop(work);
    for calls in [false, true] {
        let (success, metrics) = storage_run(&program, 1 << 24, calls, false);
        let held = success.unwrap();
        assert!(!metrics.invalid);
        assert!(metrics.allocated <= held && metrics.highwater <= held);
        assert_eq!(metrics.live, 0, "destruction stays inside the observer");
        let (exact, _) = storage_run(&program, admission + held, calls, false);
        assert_eq!(exact.unwrap(), held);
        // Every integer below success includes first directory construction,
        // duplicate lookup, member growth and final publication boundaries.
        for allowance in 0..held {
            let (result, observed) = storage_run(&program, admission + allowance, calls, false);
            assert!(result.is_err(), "premature success at {allowance}/{held}");
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0, "refusal leaked at {allowance}/{held}");
        }
        let (old, old_metrics) = storage_run(&program, 1 << 24, calls, true);
        eprintln!("OPT878 borrowed keys calls={calls}: previous held={} allocated={} highwater={}; current held={held} allocated={} highwater={}", old.unwrap(), old_metrics.allocated, old_metrics.highwater, metrics.allocated, metrics.highwater);
    }
}
