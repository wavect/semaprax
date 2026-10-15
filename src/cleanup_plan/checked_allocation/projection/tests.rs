//! Deferred semantic/physical failure contours use the actual joined source.
use super::*;
use crate::cleanup_plan::{
    AllocationOperation, CleanupTransition, EdgeCondition, StatusLane, StatusProducer,
};
use crate::conformance::{OperationOutcome, TraceEventKind, TraceOutcome, TraceResult};
const SOURCE: &str = r#"module checked.deferred;
@id("checked.deferred.main") fn reserve() -> usize {
    let values = vec_with_capacity<string>(1usize);
    let reserved = vec_reserve_owned<string>(values, 1usize);
    vec_capacity<string>(reserved)
}
@id("checked.deferred.entry") fn main() -> i64 { 0 }
"#;
#[test]
fn deferred_allocating_call_keeps_caller_epochs_until_both_successes_and_rejects_early_commit() {
    let source = crate::parse(SOURCE, "checked-deferred.spx").unwrap();
    let entry = DeclarationId::new("checked.deferred.main");
    let prepared = super::super::prepare(&source, &entry).unwrap();
    assert!(
        matches!(
            crate::cleanup_plan::execute::execute_for_conformance(
                prepared.ordinary_program(),
                &entry,
                crate::cleanup_plan::CleanupScenario::new(
                    "ordinary-usize",
                    Some(TraceResult::Usize(2))
                ),
            ),
            Err(crate::cleanup_plan::execute::CleanupExecutionError::UnsupportedResultType(_))
        ),
        "the private receiving repair must not widen the older public oracle"
    );
    let mut owner = prepared.receive().unwrap();
    let function = &owner.program.functions[0];
    let plan = &function.cleanup_plan;
    let physical = plan
        .status_sources
        .iter()
        .filter(|source| {
            matches!(
                &source.producer,
                StatusProducer::CheckedAllocation {
                    operation: AllocationOperation::VecBacking,
                    ..
                }
            )
        })
        .next_back()
        .expect("the actual reserve backing source")
        .id
        .clone();
    let semantic = plan
        .status_sources
        .iter()
        .find(|source| {
            source.id.expression == physical.expression
                && source.id.lane == StatusLane::OperationFailure
        })
        .expect("reserve semantic bounds source precedes allocation")
        .id
        .clone();
    let failure_status = |allocation| {
        if allocation {
            crate::conformance::NormalizedStatus::try_new(
                crate::cleanup_plan::ALLOCATION_DOMAIN_V1,
                1,
                crate::conformance::StatusClass::Adapter,
                crate::conformance::Retryability::Known(false),
            )
            .unwrap()
        } else {
            // The operation model's semantic domain remains separate from allocation.
            crate::conformance::NormalizedStatus::try_new(
                crate::vec_ops::STATUS_DOMAIN,
                1,
                crate::conformance::StatusClass::Adapter,
                crate::conformance::Retryability::Known(false),
            )
            .unwrap()
        }
    };
    for (failed, allocation) in [(&semantic, false), (&physical, true)] {
        let mut scenario = crate::cleanup_plan::CleanupScenario::new("deferred-refusal", None);
        for source in &plan.status_sources {
            scenario.operations.insert(
                source.id.clone(),
                if source.id == *failed {
                    OperationOutcome::Failure(failure_status(allocation))
                } else {
                    OperationOutcome::Success
                },
            );
            if source.id == *failed {
                break;
            }
        }
        let trace = crate::cleanup_plan::execute::checked_allocation::execute_for_conformance(
            &owner, &entry, scenario,
        )
        .unwrap();
        assert!(matches!(trace.outcome, TraceOutcome::Failure { .. }));
        assert_eq!(
            trace
                .events
                .iter()
                .filter(|event| matches!(event.event, TraceEventKind::FinalizeEnd { .. }))
                .count(),
            1
        );
        assert!(!trace.events.iter().any(|event| matches!(&event.event,
            TraceEventKind::CallCommit { arguments, .. } if arguments.len() == 1)));
        assert!(!trace
            .events
            .iter()
            .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
    }
    let mut success =
        crate::cleanup_plan::CleanupScenario::new("deferred-success", Some(TraceResult::Usize(2)));
    for source in &plan.status_sources {
        success
            .operations
            .insert(source.id.clone(), OperationOutcome::Success);
    }
    let trace = crate::cleanup_plan::execute::checked_allocation::execute_for_conformance(
        &owner, &entry, success,
    )
    .unwrap();
    assert!(matches!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::Usize(2)
        }
    ));
    assert_eq!(
        trace
            .events
            .iter()
            .filter(|event| matches!(&event.event,
        TraceEventKind::CallCommit { arguments, .. } if arguments.len() == 1))
            .count(),
        1
    );
    // Move the existing commit ahead of the physical success, rather than
    // inventing a token or replacing an ordinary plan with attached evidence.
    let plan = &mut owner.program.functions[0].cleanup_plan;
    let (block, index) = plan
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, value)| {
            value
                .transitions
                .iter()
                .position(|transition| {
                    matches!(transition,
            CleanupTransition::CallCommit { call, .. } if call == &physical.expression)
                })
                .map(|index| (block, index))
        })
        .unwrap();
    let transition = plan.blocks[block].transitions.remove(index);
    let before = plan
        .edges
        .iter()
        .find(|edge| matches!(&edge.condition, EdgeCondition::StatusZero(id) if id == &physical))
        .unwrap()
        .from;
    plan.blocks[before.0 as usize].transitions.push(transition);
    assert!(
        owner.replay().is_err(),
        "an early deferred commit cannot certify itself"
    );
}
