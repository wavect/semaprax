//! Full ancestry equality, malformed-input precedence and paid physical bounds.
use super::*;
use crate::bounded_output::{with_limit, with_limit_usage_refusal, ReservationRefusal};
use crate::cleanup_plan::{
    CleanupEdge, CleanupRegion, CleanupResultSource, EdgeCondition, StatusLane, StatusSourceId,
};
use crate::test_allocation_observer::{measure, Metrics};

fn base() -> ResolvedFunction {
    let program = super::super::tests::program();
    program
        .functions
        .iter()
        .find(|function| {
            !function.cleanup_plan.blocks.is_empty() && !function.cleanup_plan.exits.is_empty()
        })
        .unwrap()
        .clone()
}

fn subject(base: &ResolvedFunction, parents: &[Option<u32>]) -> (ResolvedFunction, ExitTarget) {
    let mut function = base.clone();
    function.cleanup_plan.regions = parents
        .iter()
        .enumerate()
        .map(|(index, parent)| CleanupRegion {
            id: CleanupRegionId(index as u32),
            parent: parent.map(CleanupRegionId),
            slots: Vec::new(),
            normal_scope_end: crate::cleanup_plan::ExitTargetId(0),
        })
        .collect();
    let mut block = function.cleanup_plan.blocks[0].clone();
    block.id = BlockId(0);
    block.region = CleanupRegionId(0);
    let mut target = block.clone();
    target.id = BlockId(1);
    function.cleanup_plan.blocks = vec![block, target];
    function.cleanup_plan.edges = vec![CleanupEdge {
        id: EdgeId(0),
        from: BlockId(0),
        to: BlockId(1),
        condition: EdgeCondition::Always,
    }];
    let mut exit = function.cleanup_plan.exits[0].clone();
    exit.from = BlockId(0);
    exit.leaves_regions.clear();
    exit.finalize_in_order.clear();
    exit.continuation = ExitContinuation::Continue(EdgeId(0));
    (function, exit)
}

fn kinds(function: &ResolvedFunction) -> Vec<ExitContinuation> {
    let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let source = CleanupResultSource::Scalar {
        expression: ExpressionId::from_owned("matching.result".into()),
    };
    vec![
        ExitContinuation::Continue(EdgeId(0)),
        ExitContinuation::LoopBreak {
            target: crate::hir::ControlTarget::Loop {
                owner: owner.clone(),
                site: ExpressionId::from_owned("matching.loop".into()),
            },
            edge: EdgeId(0),
        },
        ExitContinuation::FunctionReturn {
            target: crate::hir::ControlTarget::Function(owner),
            edge: EdgeId(0),
            source: source.clone(),
        },
        ExitContinuation::CommitResult { source },
        ExitContinuation::ReturnFailure {
            source: StatusSourceId {
                expression: ExpressionId::from_owned("matching.failure".into()),
                lane: StatusLane::OperationFailure,
            },
        },
        ExitContinuation::ReturnUnit,
    ]
}

fn current(function: &ResolvedFunction, exit: &ExitTarget) -> Result<bool, Diagnostic> {
    let (result, refused) = with_limit(1 << 26, || {
        let mut budget = ReplayBudget::new();
        exit_regions_match(function, exit, &mut budget)
    });
    assert!(!refused);
    result
}

#[test]
fn topological_forests_and_reverse_numberings_keep_the_complete_original_prefix() {
    let base = base();
    let mut cases = 0;
    for length in 1..=5usize {
        let total = (1..=length).product::<usize>();
        for mut encoded in 0..total {
            let mut parents = Vec::new();
            for index in 0..length {
                let digit = encoded % (index + 1);
                encoded /= index + 1;
                parents.push((digit != 0).then(|| (digit - 1) as u32));
            }
            for reverse in [false, true] {
                let mut numbered = vec![None; length];
                let ordinal = |index: usize| if reverse { length - index - 1 } else { index };
                for (index, parent) in parents.iter().enumerate() {
                    numbered[ordinal(index)] = parent.map(|parent| ordinal(parent as usize) as u32);
                }
                let (mut function, mut exit) = subject(&base, &numbered);
                let kinds = kinds(&function);
                for source in 0..length {
                    for target in 0..length {
                        function.cleanup_plan.blocks[0].region = CleanupRegionId(source as u32);
                        function.cleanup_plan.blocks[1].region = CleanupRegionId(target as u32);
                        for kind in &kinds {
                            exit.continuation = kind.clone();
                            let expected = crate::cleanup_plan::replay::expected_exit_regions(
                                &function, &exit,
                            )
                            .unwrap();
                            exit.leaves_regions = expected.clone();
                            assert_eq!(current(&function, &exit), Ok(true));
                            exit.leaves_regions.push(CleanupRegionId(u32::MAX));
                            assert_eq!(current(&function, &exit), Ok(false));
                            exit.leaves_regions = expected.clone();
                            if !expected.is_empty() {
                                exit.leaves_regions[0] = CleanupRegionId(u32::MAX);
                                assert_eq!(current(&function, &exit), Ok(false));
                                exit.leaves_regions = expected.clone();
                                exit.leaves_regions.pop();
                                assert_eq!(current(&function, &exit), Ok(false));
                            }
                            if expected.len() > 1 {
                                exit.leaves_regions = expected.clone();
                                exit.leaves_regions.reverse();
                                assert_eq!(current(&function, &exit), Ok(false));
                            }
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 30_000);
}

#[test]
fn every_original_produced_non_latch_exit_keeps_its_full_expected_result() {
    let program = super::super::tests::program();
    let mut checked = 0;
    for function in &program.functions {
        for exit in &function.cleanup_plan.exits {
            if matches!(exit.continuation, ExitContinuation::LoopLatch { .. }) {
                continue;
            }
            let expected = crate::cleanup_plan::replay::expected_exit_regions(function, exit)
                .map(|regions| regions == exit.leaves_regions);
            assert_eq!(current(function, exit), expected);
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn malformed_full_chains_precede_an_authored_prefix_mismatch() {
    let base = base();
    let (mut function, mut exit) = subject(&base, &[None, Some(0), Some(u32::MAX)]);
    function.cleanup_plan.blocks[0].region = CleanupRegionId(1);
    function.cleanup_plan.blocks[1].region = CleanupRegionId(2);
    exit.leaves_regions = vec![CleanupRegionId(u32::MAX)];
    for fault in 0..5 {
        let mut function = function.clone();
        let mut exit = exit.clone();
        match fault {
            0 => exit.from = BlockId(u32::MAX),
            1 => function.cleanup_plan.blocks[0].region = CleanupRegionId(u32::MAX),
            2 => exit.continuation = ExitContinuation::Continue(EdgeId(u32::MAX)),
            3 => function.cleanup_plan.edges[0].to = BlockId(u32::MAX),
            _ => {}
        }
        let expected = crate::cleanup_plan::replay::expected_exit_regions(&function, &exit)
            .map(|regions| regions == exit.leaves_regions);
        assert!(expected.is_err());
        assert_eq!(current(&function, &exit), expected);
    }
    let (mut function, mut exit) = subject(&base, &[Some(0)]);
    exit.continuation = ExitContinuation::ReturnUnit;
    function.cleanup_plan.blocks[0].region = CleanupRegionId(0);
    with_limit(1 << 26, || {
        let mut budget = ReplayBudget::new();
        budget.remaining = 64;
        let result = exit_regions_match(&function, &exit, &mut budget).unwrap_err();
        assert_eq!(result.code, "SPX-H006");
        assert!(
            result.message.contains(
                "cleanup replay work budget exhausted during cleanup exit region comparison"
            ),
            "{result:?}"
        );
    });
}

struct Observation {
    accepted: bool,
    work: usize,
    debit: usize,
    refused: bool,
    first: Option<ReservationRefusal>,
    allocation: Metrics,
}

fn observe(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    work: usize,
    bytes: usize,
    matching: bool,
    floor: usize,
) -> Observation {
    let ((value, allocation), refused, debit, first) = with_limit_usage_refusal(bytes, || {
        assert!(crate::bounded_output::set_active_floor(floor));
        let measured = measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work;
            let accepted = if matching {
                let result = exit_regions_match(function, exit, &mut budget);
                let accepted = matches!(&result, Ok(true));
                drop(result);
                accepted
            } else {
                let result = super::super::expected_exit_regions(function, exit, &mut budget);
                let accepted = matches!(&result, Ok(regions) if *regions == exit.leaves_regions);
                drop(result);
                accepted
            };
            let used = work - budget.remaining;
            if budget.refusal_spent {
                let remaining = budget.remaining;
                let bytes = crate::bounded_output::active_remaining();
                let retry = exit_regions_match(function, exit, &mut budget);
                assert!(retry.is_err());
                drop(retry);
                assert_eq!(budget.remaining, remaining);
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                crate::bounded_output::clear_active_floor();
                let retry = exit_regions_match(function, exit, &mut budget);
                assert!(retry.is_err());
                drop(retry);
                assert_eq!(budget.remaining, remaining);
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
            }
            drop(budget);
            (accepted, used)
        });
        if !crate::bounded_output::active_refused() {
            assert!(crate::bounded_output::active_remaining().unwrap() >= floor);
        }
        crate::bounded_output::clear_active_floor();
        measured
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= debit);
    Observation {
        accepted: value.0,
        work: value.1,
        debit,
        refused,
        first,
        allocation,
    }
}

#[test]
fn physical_and_work_frontiers_keep_full_payment_sticky_refusal_and_last_drop() {
    let base = base();
    let parents = (0..64)
        .map(|index| (index != 0).then(|| index - 1))
        .collect::<Vec<_>>();
    let (mut function, mut exit) = subject(&base, &parents);
    function.cleanup_plan.blocks[0].region = CleanupRegionId(63);
    function.cleanup_plan.blocks[1].region = CleanupRegionId(0);
    exit.leaves_regions =
        crate::cleanup_plan::replay::expected_exit_regions(&function, &exit).unwrap();
    let old = observe(&function, &exit, MAX_REPLAY_WORK_UNITS, 1 << 26, false, 0);
    let full = observe(&function, &exit, MAX_REPLAY_WORK_UNITS, 1 << 26, true, 0);
    assert!(old.accepted && full.accepted && !old.refused && !full.refused);
    assert!(full.debit > 0 && full.work > 0);
    let (_, refused, prepared, _) = with_limit_usage_refusal(1 << 26, || {
        let mut budget = ReplayBudget::new();
        budget.prepare_builder(&function).unwrap();
    });
    assert!(!refused);
    assert!(
        full.debit
            >= prepared
                + std::mem::size_of::<Matching<'_>>()
                + 2 * std::mem::size_of::<Result<bool, Diagnostic>>()
    );
    assert!(full.debit < old.debit);
    assert!(full.allocation.allocated < old.allocation.allocated);
    for bytes in [0, full.debit - 1, full.debit] {
        let value = observe(&function, &exit, MAX_REPLAY_WORK_UNITS, bytes, true, 0);
        assert_eq!(value.accepted, bytes == full.debit);
        assert_eq!(value.refused, bytes != full.debit);
        assert_eq!(value.first.is_some(), bytes != full.debit);
    }
    for bytes in [37, 37 + full.debit - 1, 37 + full.debit] {
        let value = observe(&function, &exit, MAX_REPLAY_WORK_UNITS, bytes, true, 37);
        assert_eq!(value.accepted, bytes == 37 + full.debit);
        assert_eq!(value.refused, bytes != 37 + full.debit);
        if let Some(first) = value.first {
            assert_eq!(first.floor, 37);
        }
    }
    for work in [0, full.work - 1, full.work] {
        let value = observe(&function, &exit, work, 1 << 26, true, 0);
        assert_eq!(value.accepted, work == full.work);
        assert!(!value.refused);
    }
}
