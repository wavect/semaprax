use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};
use std::rc::Rc;

fn program(prefix: usize) -> ResolvedProgram {
    let locals = (0..prefix)
        .map(|i| format!("let n{i}={i};"))
        .collect::<String>();
    let source = format!("module fused879; @id(\"f\") fn f(a:bool,b:bool)->i64 {{{locals} let n=if a {{1}}else{{2}}; if b {{n+1}}else{{n+2}}}} @id(\"main\") fn main()->i64 {{f(true,false)}}");
    let parsed = crate::check(&source, "fused879.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}
// Checked arithmetic emits real status observations; constant let bindings do
// not lengthen a branch history and cannot exercise the surviving full bank.
fn checked_prefix_program(prefix: usize) -> ResolvedProgram {
    let mut source = String::from("module long879; @id(\"f\") fn f(a:bool,b:bool,x:i64)->i64 {");
    for index in 0..prefix {
        let input = if index == 0 {
            "x".to_owned()
        } else {
            format!("x{}", index - 1)
        };
        source.push_str(&format!("let x{index}={input}+1;"));
    }
    source.push_str("let n=if a {1}else{2}; if b {n+");
    source.push_str(&format!("x{}", prefix - 1));
    source.push_str("}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false,0)}");
    let checked = crate::check(&source, "long879.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "long879.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let mut program = crate::hir::resolve(&checked).unwrap();
    // The ordinary two-edge route moves spare capacity and can avoid both
    // generic banks even on this long history. As in the existing three-edge
    // controls, retain every checked edge and append one final edge to exercise
    // the general receiving path using the real compiler-owned CFG types.
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let block = function.cleanup_plan.blocks.iter().rposition(|block|
        matches!(&block.terminator, CleanupTerminator::Branch(edges) if edges.len() == 2)
    ).unwrap();
    let CleanupTerminator::Branch(edges) = &function.cleanup_plan.blocks[block].terminator else {
        unreachable!()
    };
    let mut edge = function.cleanup_plan.edges[edges[edges.len() - 1].0 as usize].clone();
    let id = crate::cleanup_plan::EdgeId(u32::try_from(function.cleanup_plan.edges.len()).unwrap());
    edge.id = id;
    function.cleanup_plan.edges.push(edge);
    let CleanupTerminator::Branch(edges) = &mut function.cleanup_plan.blocks[block].terminator
    else {
        unreachable!()
    };
    edges.push(id);
    program
}
fn values(
    function: &ResolvedFunction,
    width: usize,
    capacity: usize,
) -> Vec<Rc<SkeletonObservation>> {
    let mut values = Vec::with_capacity(capacity);
    for _ in 0..width {
        values.push(Rc::new(SkeletonObservation::Boolean {
            expression: function.body.id.clone(),
            value: true,
        }));
    }
    values
}
#[test]
#[allow(
    clippy::int_plus_one,
    reason = "The bounded fixture keeps the original allocation-width arithmetic for exact comparison."
)]
fn production_fused_probe_preserves_every_original_take_decision_and_bounds_copy_receivers() {
    let program = program(0);
    let function = &program.functions[0];
    with_limit_usage_refusal(1 << 30, || {
        let mut budget = ReplayBudget::new();
        let mut selected_copy = false;
        for width in [0usize, 1, 2, 16, 42, 96] {
            for capacity in [width, width + 1, width * 2 + 1] {
                let values = values(function, width, capacity);
                for target in 0..function.cleanup_plan.blocks.len() {
                    for remaining_edges in [0, 1] {
                        for edge_count in [1, 2, 3] {
                            budget
                                .reserve_bytes(
                                    function,
                                    2 * TAKE_CALL_CONTROLS + ADDITIONAL_CONTROLS,
                                    "two independent take query receivers",
                                )
                                .unwrap();
                            let expected = can_take(TakeCandidate {
                                values: &values,
                                remaining_edges,
                                edge_count,
                                budget: &budget,
                                plan: &function.cleanup_plan,
                                target: BlockId(target as u32),
                            });
                            let actual = probe(TakeCandidate {
                                values: &values,
                                remaining_edges,
                                edge_count,
                                budget: &budget,
                                plan: &function.cleanup_plan,
                                target: BlockId(target as u32),
                            });
                            assert_eq!(
                                actual.take, expected,
                                "prefix={width} target={target} capacity={capacity}"
                            );
                            if let Some(copy) = actual.copy_capacity {
                                selected_copy = true;
                                let original =
                                    physical_storage::replay_growth_capacity(width, width + 1)
                                        .unwrap();
                                assert!(copy >= width + 1 && copy < original);
                                assert!(
                                    (original - copy) * size_of::<Rc<SkeletonObservation>>()
                                        > ADDITIONAL_CONTROLS
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(
            selected_copy,
            "the fused copy route must actually be selected"
        );
    });
}

#[test]
fn production_fused_enumerator_matches_the_complete_predecessor_paths_work_and_materializations() {
    for prefix in [0, 42, 96] {
        let program = program(prefix);
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(1 << 29, || {
            measure(|| {
                for function in &program.functions {
                    let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let begin = skeleton_materializations();
                    let expected = before::plan_skeleton_paths(function, &mut old).unwrap();
                    let old_events = skeleton_materializations() - begin;
                    let begin = skeleton_materializations();
                    let actual =
                        crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut current)
                            .unwrap();
                    assert_eq!(actual, expected);
                    assert_eq!(skeleton_materializations() - begin, old_events);
                    assert_eq!(current.remaining, old.remaining);
                    assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                    drop((actual, expected));
                    drop(current);
                    drop(old);
                }
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    assert!(value.len() <= 1024);
    let mut bytes = [0; 1024];
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<usize, Failure>,
    remaining: usize,
    skeleton: usize,
    events: usize,
    sticky: Option<Failure>,
}
fn observe(
    old: bool,
    function: &ResolvedFunction,
    work: usize,
    limit: usize,
    floor: usize,
) -> (Outcome, Metrics, bool, usize) {
    let ((outcome, allocation), refused, paid, _) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            assert!(crate::bounded_output::set_active_floor(floor));
            let begin = skeleton_materializations();
            let result = if old {
                before::plan_skeleton_paths(function, &mut budget)
            } else {
                crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut budget)
            };
            let result = result
                .map(|paths| {
                    let n = paths.len();
                    drop(paths);
                    n
                })
                .map_err(failure);
            let events = skeleton_materializations() - begin;
            let remaining = budget.remaining;
            let skeleton = budget.skeleton_remaining;
            crate::bounded_output::clear_active_floor();
            let sticky = if result.is_err() {
                let bytes = crate::bounded_output::active_remaining();
                let error = budget
                    .reserve_bytes(function, 0, "fused879 sticky refusal")
                    .err()
                    .unwrap();
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                Some(failure(error))
            } else {
                None
            };
            drop(budget);
            Outcome {
                result,
                remaining,
                skeleton,
                events,
                sticky,
            }
        })
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    (outcome, allocation, refused, paid)
}
#[test]
fn production_fused_clone_keeps_original_work_failures_and_actual_exact_short_floor_drop() {
    for prefix in [0, 42, 96] {
        let program = program(prefix);
        let function = &program.functions[0];
        let (old, old_allocation, old_refused, old_paid) =
            observe(true, function, MAX_REPLAY_WORK_UNITS, 1 << 29, 0);
        let (current, current_allocation, current_refused, current_paid) =
            observe(false, function, MAX_REPLAY_WORK_UNITS, 1 << 29, 0);
        assert_eq!(old, current);
        assert!(!old_refused && !current_refused);
        assert!(
            old_allocation.allocated <= old_paid && current_allocation.allocated <= current_paid
        );
        let spent = MAX_REPLAY_WORK_UNITS - current.skeleton;
        for work in [0, 1, spent / 2, spent - 1, spent] {
            assert_eq!(
                observe(true, function, work, 1 << 29, 0).0,
                observe(false, function, work, 1 << 29, 0).0,
                "all six original work failure fields"
            );
        }
        let (exact, _, refused, paid) =
            observe(false, function, MAX_REPLAY_WORK_UNITS, current_paid, 0);
        assert_eq!(exact, current);
        assert!(!refused);
        assert_eq!(paid, current_paid);
        for limit in [0, 1, current_paid / 2, current_paid - 1] {
            let (refused, _, _, _) = observe(false, function, MAX_REPLAY_WORK_UNITS, limit, 0);
            assert_eq!(refused.result.as_ref().err().unwrap().code, "SPX-H006");
            assert!(refused.sticky.is_some());
        }
        let (floored, _, refused, _) =
            observe(false, function, MAX_REPLAY_WORK_UNITS, current_paid, 1);
        assert!(floored.result.is_err() && floored.sticky.is_some() && refused);
        eprintln!("OPT879 production-only prefix={prefix}: old/current paid={old_paid}/{current_paid}, allocated={}/{}, highwater={}/{}",old_allocation.allocated,current_allocation.allocated,old_allocation.highwater,current_allocation.highwater);
    }
}

#[derive(Debug, Eq, PartialEq)]
struct FrontierOutcome {
    result: Result<usize, Failure>,
    remaining: usize,
    skeleton: usize,
    spent: usize,
    events: usize,
    sticky: Option<Failure>,
}
#[allow(
    clippy::type_complexity,
    reason = "The independently compiled control retains its exact tuple layout and receiving types."
)]
fn frontier_observe(
    function: &ResolvedFunction,
    limit: usize,
    floor: usize,
) -> (
    (FrontierOutcome, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
    Option<(&'static str, u32)>,
) {
    let (value, refused, paid, first, site) =
        crate::bounded_output::with_limit_usage_refusal_site(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                assert!(crate::bounded_output::set_active_floor(floor));
                let begin = skeleton_materializations();
                let result =
                    crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut budget);
                let (result, sticky) = match result {
                    Ok(paths) => {
                        let count = paths.len();
                        drop(paths);
                        (Ok(count), None)
                    }
                    Err(error) => {
                        let first = failure(error);
                        let bytes = crate::bounded_output::active_remaining();
                        let work = (
                            budget.remaining,
                            budget.skeleton_remaining,
                            budget.skeleton_spent,
                        );
                        let allocated =
                            crate::test_allocation_observer::current_metrics().allocated;
                        let sticky = failure(
                            budget
                                .reserve_bytes(function, 0, "879 receiving sticky zero")
                                .unwrap_err(),
                        );
                        assert_eq!(crate::bounded_output::active_remaining(), bytes);
                        assert_eq!(
                            (
                                budget.remaining,
                                budget.skeleton_remaining,
                                budget.skeleton_spent
                            ),
                            work
                        );
                        assert_eq!(
                            crate::test_allocation_observer::current_metrics().allocated,
                            allocated
                        );
                        (Err(first), Some(sticky))
                    }
                };
                let value = FrontierOutcome {
                    result,
                    remaining: budget.remaining,
                    skeleton: budget.skeleton_remaining,
                    spent: budget.skeleton_spent,
                    events: skeleton_materializations() - begin,
                    sticky,
                };
                drop(budget);
                crate::bounded_output::clear_active_floor();
                value
            })
        });
    assert!(!value.1.invalid && value.1.live == 0);
    assert_eq!(value.1.allocated, value.1.freed);
    assert!(value.1.allocated <= paid && value.1.highwater <= paid);
    (
        value,
        refused,
        paid,
        first,
        site.map(|location| (location.file(), location.line())),
    )
}
// Independent existing primitive, not a reconstructed message or old bill.
// Bootstrap under a real active ledger, then consume successful bytes to the
// exact target remainder before testing its actual reservation and sticky path.
fn frontier_expected(
    function: &ResolvedFunction,
    requested: usize,
    remaining: usize,
    floor: usize,
    phase: &str,
) -> (Failure, Failure) {
    let ((first, sticky), refused, _, receipt) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        budget.prepare_builder(function).unwrap();
        let available = crate::bounded_output::active_remaining().unwrap();
        assert!(crate::bounded_output::reserve_active_required(
            available - remaining
        ));
        assert!(crate::bounded_output::set_active_floor(floor));
        let first = failure(
            budget
                .reserve_bytes(function, requested, phase)
                .unwrap_err(),
        );
        let bytes = crate::bounded_output::active_remaining();
        let sticky = failure(
            budget
                .reserve_bytes(function, 0, "879 receiving sticky zero")
                .unwrap_err(),
        );
        assert_eq!(crate::bounded_output::active_remaining(), bytes);
        drop(budget);
        crate::bounded_output::clear_active_floor();
        (first, sticky)
    });
    assert!(refused);
    let receipt = receipt.unwrap();
    assert_eq!(
        (receipt.requested, receipt.remaining, receipt.floor),
        (requested, remaining, floor)
    );
    (first, sticky)
}
#[test]
#[allow(
    clippy::drop_non_drop,
    reason = "This explicit lifetime endpoint is part of the complete ownership and allocation observation."
)]
fn production_fused_earlier_bank_and_original_take_frontiers_keep_six_fields_first_receipt_and_whole_drop(
) {
    let program = checked_prefix_program(branch_clone::short_plan::LIMIT + 1);
    let function = &program.functions[0];
    assert!(!cleanup_inert_large_decisions_can_be_summarized(function));
    assert!(!status_only_paths_can_be_summarized(function));
    let source = include_str!("../../../replay.rs");
    for (needle, requested, phase) in [
        (
            "branch_clone::production_capacity::production_inline_controls::<",
            production_inline_controls::<Rc<SkeletonObservation>>(),
            "branch clone receiving controls",
        ),
        (
            "branch_clone::TAKE_CALL_CONTROLS,",
            TAKE_CALL_CONTROLS,
            "plan final branch probe receiving",
        ),
    ] {
        assert_eq!(
            source.lines().filter(|line| line.contains(needle)).count(),
            1
        );
        let member = source
            .lines()
            .position(|line| line.contains(needle))
            .unwrap();
        let target_line = u32::try_from(
            source
                .lines()
                .take(member + 1)
                .enumerate()
                .filter(|(_, line)| line.contains("budget.reserve_bytes("))
                .map(|(i, _)| i)
                .last()
                .expect("the actual bank belongs to the preceding reservation call")
                + 1,
        )
        .unwrap();
        let mut limit = 0;
        let mut prefix = None;
        // Actual receipt staircase: no guessed old bill or offset.
        for _ in 0..100_000 {
            let (outcome, refused, paid, receipt, site) = frontier_observe(function, limit, 0);
            assert!(refused && outcome.0.result.is_err(), "required bank {phase}, function {}, blocks {}, paid {paid}, limit {limit}, outcome {:?}", function.id, function.cleanup_plan.blocks.len(), outcome.0);
            let receipt = receipt.unwrap();
            if receipt.requested == requested
                && site.is_some_and(|(path, line)| {
                    path.ends_with("cleanup_plan/replay.rs") && line == target_line
                })
            {
                prefix = Some(paid);
                break;
            }
            let next = paid.checked_add(receipt.requested).unwrap();
            assert!(
                next > limit,
                "each real refused request advances the staircase"
            );
            limit = next;
        }
        let prefix = prefix.expect("the actual production receiving frontier must be reached");
        let exact = prefix.checked_add(requested).unwrap();
        let (outcome, _, paid, _, _) = frontier_observe(function, exact, 0);
        assert!(
            paid >= exact,
            "this exact bank admits before a later refusal"
        );
        drop(outcome);
        for (limit, floor, remaining) in [
            (prefix, 0, 0),
            (exact - 1, 0, requested - 1),
            (exact, 1, requested),
        ] {
            let (first, refused, paid, receipt, site) = frontier_observe(function, limit, floor);
            assert!(refused);
            assert_eq!(paid, prefix);
            let receipt = receipt.unwrap();
            assert_eq!(
                (receipt.requested, receipt.remaining, receipt.floor),
                (requested, remaining, floor)
            );
            assert!(site.is_some_and(
                |(path, line)| path.ends_with("cleanup_plan/replay.rs") && line == target_line
            ));
            let expected = frontier_expected(function, requested, remaining, floor, phase);
            assert_eq!(
                first.0.result.as_ref().err(),
                Some(&expected.0),
                "all six original primitive diagnostic fields"
            );
            assert_eq!(
                first.0.sticky.as_ref(),
                Some(&expected.1),
                "all six sticky diagnostic fields"
            );
            let (again, again_refused, again_paid, again_receipt, again_site) =
                frontier_observe(function, limit, floor);
            assert_eq!(
                again, first,
                "complete Diagnostic, work, allocation, peak and Drop snapshots"
            );
            assert_eq!(
                (again_refused, again_paid, again_receipt, again_site),
                (refused, paid, Some(receipt), site),
                "the first receipt, stage and tracked caller site remain sticky"
            );
        }
    }
}

#[test]
fn current_fused_enumerator_matches_complete_main_c883_arithmetic_control() {
    let mut source = String::from("module main_c883; @id(\"f\") fn f(x:i64)->i64 {");
    for index in 0..96 {
        let input = if index == 0 {
            "x".to_owned()
        } else {
            format!("x{}", index - 1)
        };
        source.push_str(&format!("let x{index}={input}+1;"));
    }
    source.push_str("x95} @id(\"main\") fn main()->i64 {f(0)}");
    let parsed = crate::check(&source, "main-c883.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let parsed = crate::check(&canonical, "main-c883.spx").unwrap();
    assert_eq!(crate::format::canonical(&parsed), canonical);
    let program = crate::hir::resolve(&parsed).unwrap();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut main = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let begin = skeleton_materializations();
                let expected = main_c883::plan_skeleton_paths(function, &mut main).unwrap();
                let events = skeleton_materializations() - begin;
                let begin = skeleton_materializations();
                let actual =
                    crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut current)
                        .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(skeleton_materializations() - begin, events);
                assert_eq!(current.remaining, main.remaining);
                assert_eq!(current.skeleton_remaining, main.skeleton_remaining);
                drop((actual, expected));
                drop(current);
                drop(main);
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

fn observe_main_a73(
    old: bool,
    function: &ResolvedFunction,
    work: usize,
    limit: usize,
    floor: usize,
) -> (Outcome, Metrics, bool, usize) {
    let ((outcome, allocation), refused, paid, _) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(work);
            assert!(crate::bounded_output::set_active_floor(floor));
            let begin = skeleton_materializations();
            let result = if old {
                before_short::plan_skeleton_paths(function, &mut budget)
            } else {
                crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut budget)
            };
            let result = result
                .map(|paths| {
                    let n = paths.len();
                    drop(paths);
                    n
                })
                .map_err(failure);
            let events = skeleton_materializations() - begin;
            let remaining = budget.remaining;
            let skeleton = budget.skeleton_remaining;
            crate::bounded_output::clear_active_floor();
            let sticky = if result.is_err() {
                let bytes = crate::bounded_output::active_remaining();
                let error = budget
                    .reserve_bytes(function, 0, "fused879 sticky refusal")
                    .err()
                    .unwrap();
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                Some(failure(error))
            } else {
                None
            };
            drop(budget);
            Outcome {
                result,
                remaining,
                skeleton,
                events,
                sticky,
            }
        })
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    (outcome, allocation, refused, paid)
}

#[test]
fn short_copy_matches_whole_main_a73_paths_work_and_materializations_with_less_debit() {
    let mut saved = false;
    for prefix in [0, 42, 96] {
        let program = program(prefix);
        for function in &program.functions {
            let (before, old_heap, old_refused, old_paid) =
                observe_main_a73(true, function, MAX_REPLAY_WORK_UNITS, 1 << 29, 0);
            let (current, heap, refused, paid) =
                observe_main_a73(false, function, MAX_REPLAY_WORK_UNITS, 1 << 29, 0);
            assert_eq!(current, before);
            assert!(!old_refused && !refused);
            assert!(heap.allocated <= paid && old_heap.allocated <= old_paid);
            assert!(heap.highwater <= paid && old_heap.highwater <= old_paid);
            saved |= paid < old_paid;
            let spent = MAX_REPLAY_WORK_UNITS - current.skeleton;
            for work in [0, 1, spent / 2, spent.saturating_sub(1), spent] {
                assert_eq!(
                    observe_main_a73(true, function, work, 1 << 29, 0).0,
                    observe_main_a73(false, function, work, 1 << 29, 0).0,
                    "all six original work diagnostic fields and sticky state"
                );
            }
            assert_eq!(
                observe_main_a73(false, function, MAX_REPLAY_WORK_UNITS, paid, 0).0,
                current
            );
            for limit in [0, 1, paid / 2, paid - 1] {
                let (value, _, _, _) =
                    observe_main_a73(false, function, MAX_REPLAY_WORK_UNITS, limit, 0);
                assert!(value.result.is_err() && value.sticky.is_some());
            }
            let (value, _, refused, _) =
                observe_main_a73(false, function, MAX_REPLAY_WORK_UNITS, paid, 1);
            assert!(value.result.is_err() && value.sticky.is_some() && refused);
            eprintln!(
                "short-history main-a73/current paid={old_paid}/{paid}; heap={}/{}",
                old_heap.allocated, heap.allocated
            );
        }
    }
    assert!(
        saved,
        "the actual complete new enumerator must save reservations"
    );
}

#[test]
fn short_copy_cutoff_covers_maximum_possible_old_backing_saving() {
    eprintln!(
        "actual short-copy cutoff {}",
        super::super::short_plan::LIMIT
    );
    let full = production_inline_controls::<Rc<SkeletonObservation>>() + TAKE_CALL_CONTROLS;
    let new = branch_clone::short_plan::COPY + branch_clone::short_plan::SELECT;
    let width = size_of::<Rc<SkeletonObservation>>();
    for length in 0..branch_clone::short_plan::LIMIT {
        let backing = if length == 0 {
            width
        } else {
            2 * length * width
        };
        assert!(
            new + backing < full,
            "length={length}, full={full}, new={new}, backing={backing}"
        );
    }
    assert!(new + 2 * branch_clone::short_plan::LIMIT * width >= full);
}

#[test]
fn short_copy_receiving_banks_keep_six_primitive_fields_first_receipt_floor_and_drop() {
    let program = program(42);
    let function = &program.functions[0];
    for (requested, phase) in [
        (
            branch_clone::short_plan::SELECT,
            "branch short-history selection controls",
        ),
        (
            branch_clone::short_plan::COPY,
            "branch short-history copy controls",
        ),
    ] {
        let mut limit = 0;
        let mut frontier = None;
        for _ in 0..100_000 {
            let (value, refused, paid, receipt, site) = frontier_observe(function, limit, 0);
            assert!(refused && value.0.result.is_err());
            let receipt = receipt.unwrap();
            let failure = value.0.result.as_ref().unwrap_err();
            let message = &failure.message.0[..failure.message.1];
            if receipt.requested == requested && message.ends_with(phase.as_bytes()) {
                frontier = Some((paid, site));
                break;
            }
            let next = paid.checked_add(receipt.requested).unwrap();
            assert!(next > limit);
            limit = next;
        }
        let (prefix, actual_site) = frontier.expect("the actual short-history bank must execute");
        assert!(actual_site.is_some());
        let exact = prefix + requested;
        {
            let (_value, _, paid, _, _) = frontier_observe(function, exact, 0);
            assert!(paid >= exact);
        }
        for (limit, floor, remaining) in [
            (prefix, 0, 0),
            (exact - 1, 0, requested - 1),
            (exact, 1, requested),
        ] {
            let (value, refused, paid, receipt, site) = frontier_observe(function, limit, floor);
            assert!(refused);
            assert_eq!(paid, prefix);
            let receipt = receipt.unwrap();
            assert_eq!(
                (receipt.requested, receipt.remaining, receipt.floor),
                (requested, remaining, floor)
            );
            assert_eq!(site, actual_site);
            let expected = frontier_expected(function, requested, remaining, floor, phase);
            assert_eq!(value.0.result.as_ref().err(), Some(&expected.0));
            assert_eq!(value.0.sticky.as_ref(), Some(&expected.1));
            let again = frontier_observe(function, limit, floor);
            assert_eq!(again.0, value);
            assert_eq!(
                (again.1, again.2, again.3, again.4),
                (refused, paid, Some(receipt), site)
            );
        }
    }
}
