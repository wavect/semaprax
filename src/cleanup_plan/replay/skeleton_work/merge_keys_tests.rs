//! Small complete-language oracle and cumulative storage probes for OPT878.
use super::*;
use std::rc::Rc;

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

// This is the existing structural foreign-child contract: ordinary consumers
// may pay a conservative immutable bound; selected DAG parents require the
// same destruction reservation. It is not a function restriction on Work.
pub(in crate::cleanup_plan::replay) fn boundary_foreign_root(
    program: &ResolvedProgram, function: &ResolvedFunction,
) -> Rc<ObservationDag> {
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    work.selected_dag_for_foreign_control(ObservationDagKind::Seq(None, Vec::new())).unwrap()
}
pub(in crate::cleanup_plan::replay) fn boundary_diagnostic(
    function: &ResolvedFunction, reason: &str,
) -> Diagnostic {
    Diagnostic::io("SPX-H006", format!(
        "cleanup plan for function `{}` failed independent replay: {reason}", function.id,
    ))
}

#[test]
fn borrowed_merge_floor_foreign_frontiers_keep_six_fields_sticky_and_complete_drop() {
    use crate::bounded_output::{active_remaining, clear_active_floor, set_active_floor, with_limit_usage_refusal};
    use crate::test_allocation_observer::measure;
    let program = fixture(false);
    let function = program.functions().get(0).unwrap();
    for calls in [false, true] {
        let (((), metrics), refused, _, _) = with_limit_usage_refusal(1 << 28, || measure(|| {
            for selected in [false, true] {
                let mut outcomes = Vec::new();
                for old in [true, false] {
                    let foreign = boundary_foreign_root(&program, function);
                    let mut budget = if selected { ReplayBudget::with_physical(function, 1 << 24).unwrap() } else { ReplayBudget::new() };
                    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                    budget.merge_paths = true;
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let mut states = inputs(function, 14);
                    for (path, _) in &mut states { path.observations.prefix = Some(foreign.clone()); }
                    let result = if calls {
                        if old { work.before_merged_call_states(states) } else { work.merged_call_states(states) }
                    } else {
                        let paths = states.into_iter().map(|(path, _)| path).collect();
                        let result = if old { work.before_merged_paths(paths) } else { work.merged_paths(paths) };
                        result.map(|paths| paths.into_iter().map(|path| (path, vec![])).collect())
                    };
                    if selected {
                        assert_eq!(result.as_ref().err().unwrap(), &boundary_diagnostic(function, "factored DAG crosses its destruction reservation"));
                    } else { assert!(result.is_ok(), "ordinary immutable foreign children remain admitted"); }
                    outcomes.push(result.map(snapshot));
                    drop(work); drop(budget); drop(foreign);
                }
                assert_eq!(outcomes[0], outcomes[1], "complete language or six-field foreign refusal; calls={calls} selected={selected}");
                drop(outcomes);
            }
        }));
        assert!(!refused && !metrics.invalid); assert_eq!(metrics.live, 0); assert_eq!(metrics.allocated, metrics.freed);
        for selected in [false, true] {
            let (((), metrics), refused, _, receipt) = with_limit_usage_refusal(1 << 28, || measure(|| {
                let mut budget = if selected { ReplayBudget::with_physical(function, 1 << 24).unwrap() } else { ReplayBudget::new() };
                budget.reserve_bytes(function, 1, "merge boundary fixture preflight").unwrap();
                budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS; budget.merge_paths = true;
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let states = inputs(function, 14);
                let paths = if calls { Vec::new() } else { states.iter().map(|(path, _)| path.clone()).collect() };
                assert!(set_active_floor(active_remaining().unwrap()));
                let error = if calls { work.merged_call_states(states).err().unwrap() } else { drop(states); work.merged_paths(paths).err().unwrap() };
                let expected = boundary_diagnostic(function, "cleanup replay physical reservation exhausted during factored borrowed merge directory");
                assert_eq!(error, expected, "all six first-floor diagnostic fields");
                assert!(work.budget.refusal_spent); clear_active_floor();
                let remaining = active_remaining(); let physical = work.budget.physical.as_ref().map(|p| p.charged());
                let retry = work.budget.reserve_bytes(function, 0, "merge boundary sticky retry").unwrap_err();
                assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                assert_eq!(active_remaining(), remaining); assert_eq!(work.budget.physical.as_ref().map(|p| p.charged()), physical);
                assert_eq!(error, expected, "retry cannot replace first six-field receipt");
                drop((retry, error, expected)); drop(work); drop(budget);
            }));
            assert!(refused && receipt.is_some()); assert!(!metrics.invalid); assert_eq!(metrics.live, 0); assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn retained_search_matches_whole_immediate_borrowed_predecessor_and_original_work() {
    for lexical in [false, true] {
        let program = fixture(lexical); let function = &program.functions[0];
        for count in [0, 1, 2, 7, 31, 64] { for calls in [false, true] { for selected in [false, true] {
            let mut results = Vec::new();
            for old in [true, false] {
                let mut budget = if selected { ReplayBudget::with_physical(function, 1 << 26).unwrap() } else { ReplayBudget::new() };
                budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS; budget.merge_paths = true;
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                if lexical { work.active_copy_region = Some(function.body.id.clone()); }
                let states = inputs(function, count);
                let result = if calls {
                    if old { work.before878_merged_call_states(states) } else { work.merged_call_states(states) }
                } else {
                    let paths = states.into_iter().map(|(path, _)| path).collect();
                    let paths = if old { work.before878_merged_paths(paths) } else { work.merged_paths(paths) }.unwrap();
                    Ok(paths.into_iter().map(|path| (path, vec![])).collect())
                }.unwrap();
                results.push((snapshot(result), work.budget.remaining, work.budget.skeleton_remaining, work.budget.skeleton_spent));
            }
            assert_eq!(results[0], results[1], "whole878/current count={count} calls={calls} selected={selected} lexical={lexical}");
        } } }
    }
}

struct RetainedRun {
    error: Option<crate::cleanup_plan::replay::merge_census::Failure>,
    report: crate::cleanup_plan::replay::merge_census::Report,
    admission: usize, paid: usize,
}
fn retained_run(program: &ResolvedProgram, calls: bool, old: bool, physical: usize, allowance: Option<usize>)
    -> (RetainedRun, crate::test_allocation_observer::Metrics) {
    use crate::cleanup_plan::replay::merge_census::{Scope, failure};
    let function = &program.functions[0];
    let ((run, metrics), _, _, _) = crate::bounded_output::with_limit_usage_refusal(1 << 28, || crate::test_allocation_observer::measure(|| {
        let mut budget = ReplayBudget::with_physical(function, physical).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS; budget.merge_paths = true;
        let mut work = SkeletonWork::new(program, function, &mut budget);
        let admission = work.budget.physical.as_ref().unwrap().charged();
        let states = inputs(function, 31);
        if let Some(allowance) = allowance {
            let remaining = crate::bounded_output::active_remaining().unwrap();
            assert!(crate::bounded_output::set_active_floor(remaining.checked_sub(allowance).unwrap()));
        }
        let scope = Scope::enter(function, work.budget);
        let result = if calls {
            if old { work.before878_merged_call_states(states) } else { work.merged_call_states(states) }.map(drop)
        } else {
            let paths = states.into_iter().map(|(path, _)| path).collect();
            if old { work.before878_merged_paths(paths) } else { work.merged_paths(paths) }.map(drop)
        };
        let error = result.as_ref().err().map(failure);
        if let Some(first) = &error {
            crate::bounded_output::clear_active_floor();
            let current = crate::test_allocation_observer::current_metrics();
            let paid = work.budget.physical.as_ref().unwrap().charged();
            let remaining = (work.budget.remaining, work.budget.skeleton_remaining, work.budget.skeleton_spent);
            let retry = work.budget.reserve_bytes(function, 0, "retained merge sticky retry").unwrap_err();
            assert_eq!(failure(&retry), failure(&Diagnostic::io("SPX-H006", String::new())));
            assert_eq!(work.budget.physical.as_ref().unwrap().charged(), paid);
            assert_eq!((work.budget.remaining, work.budget.skeleton_remaining, work.budget.skeleton_spent), remaining);
            assert_eq!(crate::test_allocation_observer::current_metrics(), current);
            assert_eq!(failure(result.as_ref().err().unwrap()), *first); drop(retry);
        }
        let paid = work.budget.physical.as_ref().unwrap().charged() - admission;
        let report = scope.report(); assert!(report.complete && report.foreign == 0);
        assert_eq!(report.cells.iter().map(|cell| cell.acquired).sum::<usize>(), paid, "actual primitive acquisitions reconcile selected charged(), never unrelated ACTIVE");
        drop(scope); crate::bounded_output::clear_active_floor();
        work.release_observation_scratch_for_test(); drop(work); drop(budget); drop(result);
        RetainedRun { error, report, admission, paid }
    }));
    assert!(!metrics.invalid); assert_eq!(metrics.live, 0); assert_eq!(metrics.allocated, metrics.freed);
    (run, metrics)
}
pub(in crate::cleanup_plan::replay) fn retained_primitive_oracle(program: &ResolvedProgram, event: crate::cleanup_plan::replay::merge_census::Event,
    physical: usize, allowance: Option<usize>) -> crate::cleanup_plan::replay::merge_census::Failure {
    use crate::cleanup_plan::replay::merge_census::failure;
    let function = &program.functions[0];
    let ((saved, metrics), _, _, _) = crate::bounded_output::with_limit_usage_refusal(1 << 28, || crate::test_allocation_observer::measure(|| {
        let mut budget = ReplayBudget::with_physical(function, physical).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS; budget.merge_paths = true;
        let mut work = SkeletonWork::new(program, function, &mut budget);
        if let Some(allowance) = allowance {
            assert!(crate::bounded_output::set_active_floor(crate::bounded_output::active_remaining().unwrap() - allowance));
        }
        work.budget.reserve_bytes(function, event.charged_before, "independent actual merge prefix").unwrap();
        let error = work.budget.reserve_bytes(function, event.requested, event.phase).unwrap_err();
        let saved = failure(&error); crate::bounded_output::clear_active_floor();
        work.release_observation_scratch_for_test(); drop(work); drop(budget); drop(error); saved
    }));
    assert!(!metrics.invalid); assert_eq!(metrics.live, 0); saved
}
#[test]
fn retained_search_actual_owner_query_and_step_frontiers_keep_six_fields_sticky_floor_and_drop() {
    let program = fixture(false);
    for calls in [false, true] {
        let (success, metrics) = retained_run(&program, calls, false, 1 << 26, None);
        assert!(success.error.is_none());
        let (old, old_metrics) = retained_run(&program, calls, true, 1 << 26, None);
        assert!(old.error.is_none());
        eprintln!("OPT881 borrowed calls={calls}: whole878 paid={} heap={} peak={}; retained paid={} heap={} peak={}; old/new census={:?}/{:?}; no application-fit inference", old.paid, old_metrics.allocated, old_metrics.highwater, success.paid, metrics.allocated, metrics.highwater, old.report.cells, success.report.cells);
        for category in [1, 2, 3] {
            for event in [success.report.first[category].unwrap(), success.report.last[category].unwrap()] {
                assert!(event.requested > 0);
                let before = success.admission + event.charged_before;
                let (short, _) = retained_run(&program, calls, false, before + event.requested - 1, None);
                assert_eq!(short.error.unwrap(), retained_primitive_oracle(&program, event, before + event.requested - 1, None));
                let (exact, _) = retained_run(&program, calls, false, before + event.requested, None);
                assert!(exact.paid >= event.charged_before + event.requested);
                let floor = event.charged_before + event.requested - 1;
                let (floored, _) = retained_run(&program, calls, false, 1 << 26, Some(floor));
                assert_eq!(floored.error.unwrap(), retained_primitive_oracle(&program, event, 1 << 26, Some(floor)));
            }
        }
        let (zero, _) = retained_run(&program, calls, false, 1 << 26, Some(0));
        assert!(zero.error.is_some()); assert_eq!(zero.paid, 0);
    }
}

#[test]
fn retained_merge_owner_preserves_complete_immediate_and_earlier_sources_without_hidden_callback() {
    let original = include_str!("../../../../docs/evidence/opt881-retained-merge-owner-20261017/before/src/cleanup_plan/replay/skeleton_work/merge_keys.rs");
    let mut complete = include_str!("merge_keys_878.rs").to_owned();
    for name in ["merged_paths", "merged_suffixes", "merged_call_states"] {
        let before = format!("before878_{name}"); let after = name;
        assert!(complete.contains(&before)); complete = complete.replace(&before, &after);
    }
    assert_eq!(complete, original, "complete immediate bodies and prices, no dispatch into new search");
    assert_eq!(include_str!("merge_keys_before.rs"), include_str!("../../../../docs/evidence/opt881-retained-merge-owner-20261017/before/src/cleanup_plan/replay/skeleton_work/merge_keys_before.rs"));
    let current = include_str!("merge_keys.rs"); assert!(!current.contains(".binary_search_by("));
    assert_eq!(current.matches("super::merge_search::search!(").count(), 2);
    let owned = include_str!("../owned_suffix.rs"); assert!(!owned.contains(".binary_search_by("));
    assert_eq!(owned.matches("skeleton_work::merge_search::search!(").count(), 1);
    let original_owned = include_str!("../../../../docs/evidence/opt881-retained-merge-owner-20261017/before/src/cleanup_plan/replay/owned_suffix.rs");
    let mut owned = include_str!("../owned_suffix_878.rs").to_owned();
    for (before, after) in [
        ("#[path = \"owned_suffix_878_vector.rs\"]\nmod sequence_vector;", "mod sequence_vector;\n#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod before;"),
        ("$crate::cleanup_plan::replay::owned_suffix_878::", "$crate::cleanup_plan::replay::owned_suffix::"),
        ("work.before878_merged_paths(", "work.merged_paths("),
        ("skeleton_work::merge_keys_878::", "skeleton_work::merge_keys::"),
    ] { assert!(owned.contains(before)); owned = owned.replace(before, after); }
    assert_eq!(owned, original_owned, "complete whole immediate owned producer/dispatch/prices");
    assert_eq!(include_str!("../owned_suffix_878_vector.rs").replace("::owned_suffix_878::sequence_vector::", "::owned_suffix::sequence_vector::"), include_str!("../owned_suffix/sequence_vector.rs"));
}
