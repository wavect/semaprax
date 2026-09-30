//! Outer malformed Proposal retry is tested below the SDK stream decoder.
//! The existing checkpointed Source fixture supplies an acknowledged settlement;
//! the real session, checked wrapper, ledger and journal own every transition.
use super::*;
use crate::agent_lifecycle::iterative::compile_source_agent_lifecycle_v2;
use crate::live_invocation::source_journal::{SourceExecutionEntryV7, SourceModelWaitEntryV7};
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;

#[test]
fn acknowledged_malformed_proposal_retries_through_real_model_wait_session() {
    let source_text = format!(
        "{}{}",
        include_str!("../../../../examples/offline-repair-project/src/app.spx"),
        r#"
@id("fixture.agent.fn.await_proposal")
fn await_proposal(observation: Observation) -> Proposal
    yields Observation -> Proposal
{
    yield observation
}
"#,
    );
    let compiled = compile_source_agent_lifecycle_v2(
        &source_text,
        "source-model-wait-retry-unit.spx",
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .expect("retained FixtureAgent wrapper compiles");
    let wait = compiled
        .model_wait_binding("fixture.agent.fn.await_proposal", 1000)
        .unwrap();
    let valid = Source::valid_response(&compiled);
    let mut source = Source {
        responses: vec![b"malformed".to_vec(), valid.clone(), valid],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut driver = crate::agent_lifecycle::iterative::driver::ReadDriver { read: &mut read };
    let mut store = Store::default();
    let clock = Clock { now: 1 };
    let key = SourceCheckpointKey::new([17; 32]);
    let outcome = compiled
        .run_live_durable_with_model_wait(
            request(&task(), &policy(3), &clock, &AgentCancellation::default()),
            &mut source,
            &mut driver,
            &mut store,
            &wait,
            &key,
        )
        .expect("outer malformed Proposal is refused then retried");
    assert_eq!(
        outcome.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Complete
    );
    assert_eq!(
        (
            source.calls,
            read.calls,
            outcome.model_dispatches,
            outcome.effect_dispatches
        ),
        (3, 2, 3, 2)
    );
    assert_eq!(outcome.checkpoint.wait_fuel().unwrap(), 5000);
    assert_eq!(outcome.checkpoint.committed_reserved_units(), 3);
    let entries = outcome.checkpoint.execution_entries_v7();
    let refused = entries
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                SourceExecutionEntryV7::Ordinary(SourceJournalEntry::ProposalRefused {
                    turn: 0,
                    attempt: 0,
                    ..
                })
            )
        })
        .expect("invalid settled Proposal receives ordinary refusal");
    let retry = entries
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                SourceExecutionEntryV7::Wait(SourceModelWaitEntryV7::Prepared {
                    turn: 0,
                    attempt: 1,
                    ..
                })
            )
        })
        .expect("next attempt prepares its own wait");
    assert!(refused < retry);
    assert!(!entries.iter().any(|(_, e)| matches!(
        e,
        SourceExecutionEntryV7::Wait(SourceModelWaitEntryV7::Completed {
            turn: 0,
            attempt: 0,
            ..
        }) | SourceExecutionEntryV7::Wait(SourceModelWaitEntryV7::EvaluationReserved {
            turn: 0,
            attempt: 0,
            phase: crate::live_invocation::source_journal::SourceModelWaitPhaseV7::Resume,
            ..
        })
    )));
    assert_eq!(
        entries
            .iter()
            .filter(|(_, e)| matches!(
                e,
                SourceExecutionEntryV7::Wait(SourceModelWaitEntryV7::Prepared { .. })
            ))
            .count(),
        3
    );
    assert_eq!(
        entries
            .iter()
            .filter(|(_, e)| matches!(
                e,
                SourceExecutionEntryV7::Wait(SourceModelWaitEntryV7::Completed { .. })
            ))
            .count(),
        2
    );
}
