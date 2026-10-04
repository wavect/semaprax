//! Genuine Continue owner and fixed original ACKs; no reconstructed State.
use super::*;
use crate::agent_lifecycle::iterative::source_live::SourceProposalPolicy;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveMovedStepV8;
use crate::agent_lifecycle::authorization::target_protocol::{
    TargetHostError, TargetHostHandler, TargetHostRequest, TargetResponseSink, TypedCarrier,
};
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::{InvocationClock,SourceInvocationClock};
use crate::provider_adapter_sdk::adapter::{
    AdapterEvent, AdapterPoll, AdapterRefusal, AdapterRequest, AdapterSettlement,
};
use crate::provider_adapter_sdk::capability::AdapterCapabilities;
use crate::provider_adapter_sdk::fixture_adapters::{base_capabilities, usage};
use crate::provider_adapter_sdk::{
    AdapterInvocationCapability, ProviderAdapter, StreamingSourceProposalAdapter,
};
use crate::resumable_effects::CapabilityPolicy;
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
struct ActualEffectProbe {
    calls: usize,
}
impl TargetHostHandler for ActualEffectProbe {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.calls += 1;
        let payload =
            b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n"
                .to_vec();
        let wire = TypedCarrier::new(request.operation().result_type(), payload)
            .unwrap_or_else(|_| panic!("actual effect response"))
            .encode();
        sink.write(&wire).map_err(|_| TargetHostError::Failed)
    }
}

struct DispatchProbe {
    starts: Rc<Cell<usize>>,
    polls: VecDeque<AdapterPoll>,
    capabilities: AdapterCapabilities,
}
impl ProviderAdapter for DispatchProbe {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }
    fn start(
        &mut self,
        _: &AdapterInvocationCapability,
        _: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        self.starts.set(self.starts.get() + 1);
        Ok(())
    }
    fn poll(&mut self) -> AdapterPoll {
        self.polls.pop_front().expect("one scripted Model response")
    }
    fn cancel(&mut self, _: &str) {}
}
fn model_document(context: &CheckedOwnedWaitJournalContextV8) -> Vec<u8> {
    let (_, execution) = context.test_runtime_execution();
    format!(
        "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":\"{}\",\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"3\",\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n",
        execution.wait().lifecycle().proposal_schema().schema().digest()
    )
    .into_bytes()
}
struct Clock {
    now: Cell<i64>,
}
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.now.get()
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
fn with_moved(
    callback: impl for<'j> FnOnce(
        &'j SourceOwnedWaitJournalV8,
        LiveMovedStepV8<'j>,
        Vec<std::sync::Weak<[u8]>>,
        &'j AgentCancellation,
        &'j Clock,
    ) -> bool,
) {
    with_moved_profile(false, false, callback)
}
fn with_moved_profile(
    three_turns: bool,
    later_observe_ensures: bool,
    callback: impl for<'j> FnOnce(
        &'j SourceOwnedWaitJournalV8,
        LiveMovedStepV8<'j>,
        Vec<std::sync::Weak<[u8]>>,
        &'j AgentCancellation,
        &'j Clock,
    ) -> bool,
) {
    let run = |context: CheckedOwnedWaitJournalContextV8,
               lease,
               key,
               directory: &std::path::Path| {
        let context = context.with_cumulative_initialization(&lease).unwrap();
        let crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedRunCreated { execution, .. } = &context.fold().created else {
                panic!("actual Created execution");
            };
        assert_eq!(
            execution,
            context.ready_runtime().unwrap().1.ordinary().invocation()
        );
        assert_ne!(
            execution,
            context.ordinary().invocation(),
            "E and derived I8 are distinct"
        );
        let context = Arc::new(context);
        let retained = Arc::clone(&context);
        let journal = SourceOwnedWaitJournalV8::open(context, key, lease).unwrap();
        let cancel = AgentCancellation::new();
        let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
        let clock = Clock { now: Cell::new(1) };
        let reopen_terminal = Cell::new(false);
        super::super::tests::test_moved(&journal, &cancel, &policy, &clock, |moved, weak| {
            reopen_terminal.set(callback(&journal, moved, weak, &cancel, &clock));
        });
        drop(journal);
        if reopen_terminal.get() {
            let registration = retained.registration().clone();
            let lease = crate::resumable_effects::owned_frame::recover_source_owned_wait_v8(
                    std::fs::File::open(directory).unwrap(),
                    &registration,
                    registration.expected_facts().clone(),
                    crate::resumable_effects::owned_frame::ExplicitStoreRegistrationGrant::for_trusted_host(true).unwrap(),
                ).unwrap();
            let recovered = SourceOwnedWaitJournalV8::open(
                retained,
                crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]),
                lease,
            )
            .unwrap();
            let terminal = recovered.terminal_evidence().unwrap();
            assert_eq!(
                terminal.status(),
                crate::live_invocation::source_journal::SourceTerminalStatus::Complete
            );
            assert!(!terminal.evidence().is_empty());
            assert!(terminal.carrier().is_some());
        }
    };
    if later_observe_ensures {
        CheckedOwnedWaitJournalContextV8::test_with_actual_three_turn_observe_ensures_store(run)
    } else if three_turns {
        CheckedOwnedWaitJournalContextV8::test_with_actual_three_turn_store(run)
    } else {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(true, run)
    }
}
fn ack<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedContinueAppendV8<'j>,
) -> VerifiedOwnedContinueAppendV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_continue(owner)
        .unwrap_or_else(|_| panic!("actual sameFD continuation ACK"))
}
#[test]
fn owned_continue_driver_advances_one_real_step_into_the_next_turn() {
    with_moved(|journal, moved, weak, _, _| {
        let before = journal.begin_session().unwrap();
        let (reserved, stages, turn, _) = before.inventory.continuation_facts().unwrap();
        let observation = moved.test_observe_oracle();
        let accounting = *moved.accounting();

        let observed = advance_live_owned_continue_v8(journal, moved)
            .unwrap_or_else(|_| panic!("two ACKs advance the actual Continue owner"));

        assert!(observed.is_observed());
        assert_eq!(observed.turn(), turn + 1);
        assert_eq!(observed.test_observation(), &observation.0);
        assert_eq!(observed.consumed(), observation.1);
        assert_eq!(observed.accounting(), &accounting);
        let after = journal.begin_session().unwrap();
        let (next_reserved, next_stages, next_turn, _) =
            after.inventory.continuation_facts().unwrap();
        let fuel = journal.context().ordinary().max_steps_per_stage().unwrap() as u64;
        assert_eq!(
            (next_reserved, next_stages, next_turn),
            (reserved + fuel, stages + 1, turn + 1)
        );
        assert!(weak.iter().any(|owner| owner.strong_count() == 1));

        drop(observed);
        assert!(weak.iter().all(|owner| owner.upgrade().is_none()));
        false
    });
}
fn continued_reduce_chain_step_ack(fault: u8, three_turns: bool, later_observe_ensures: bool) {
    let run = move || {
        with_moved_profile(
            three_turns,
            later_observe_ensures,
            |journal, moved, weak, _, _| {
                let observed = advance_live_owned_continue_v8(journal, moved)
                    .unwrap_or_else(|_| panic!("actual Continue driver"));
                let settled = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_observe_settlement(
                        observed
                            .prepare_observe_settlement()
                            .unwrap_or_else(|_| panic!("next-turn Observe settlement")),
                    )
                    .unwrap_or_else(|_| panic!("Observe settlement ACK"))
                    .advance_observe_settlement()
                    .unwrap_or_else(|_| panic!("settled next-turn Observe"));
                let settled = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_observe_settlement(
                        settled
                            .prepare_turn_observed()
                            .unwrap_or_else(|_| panic!("next-turn observed selector")),
                    )
                    .unwrap_or_else(|_| panic!("turn-observed ACK"))
                    .advance_observe_settlement()
                    .unwrap_or_else(|_| panic!("carried next-turn Observe"));
                let carried = settled
                    .into_continued_wait()
                    .unwrap_or_else(|_| panic!("actual carried owner"));
                let created = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_start(
                        carried
                            .prepare_start_created()
                            .unwrap_or_else(|_| panic!("actual Start Created selector")),
                    )
                    .unwrap_or_else(|_| panic!("Start Created ACK"))
                    .advance_continued_start()
                    .unwrap_or_else(|_| panic!("next-turn Start Created"));
                let reserved = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_start(
                        created
                            .prepare_start_reservation()
                            .unwrap_or_else(|_| panic!("actual Start reservation selector")),
                    )
                    .unwrap_or_else(|_| panic!("Start reservation ACK"))
                    .advance_continued_start()
                    .unwrap_or_else(|_| panic!("next-turn Start Reserved"));
                let entries = crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries();
                let sequence = journal.begin_session().unwrap().sequence();

                let prepared = advance_live_owned_continued_start_v8(journal, reserved)
                    .unwrap_or_else(|_| panic!("sole actual Start source entry and Prepared ACK"));

                prepared.validate_live().unwrap();
                assert_eq!(journal.begin_session().unwrap().sequence(), sequence + 1);
                assert_eq!(
            crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries(),
            entries + 1,
            "Prepared ACK cannot replay Start source"
        );
                let (_, execution) = journal.context().test_runtime_execution();
                let model = execution.model();
                let starts = Rc::new(Cell::new(0));
                let response = model_document(journal.context());
                let factory_starts = Rc::clone(&starts);
                let mut factory = move || -> Box<dyn ProviderAdapter> {
                    let mut capabilities = base_capabilities("owned-wait-inert-test", true);
                    capabilities.max_request_bytes = 65_536;
                    Box::new(DispatchProbe {
                        starts: Rc::clone(&factory_starts),
                        polls: vec![
                            AdapterPoll::Event(AdapterEvent::Delta(response.clone())),
                            AdapterPoll::Event(AdapterEvent::Completed),
                            AdapterPoll::Settled(AdapterSettlement {
                                response_bytes: response.clone(),
                                usage: usage(2, 3, 1),
                            }),
                        ]
                        .into(),
                        capabilities,
                    })
                };
                let mut adapter = StreamingSourceProposalAdapter::new_bound_checkpointed(
                    &mut factory,
                    AdapterInvocationCapability::grant("continued Model driver test"),
                    execution.wait().lifecycle().proposal_schema(),
                    model.clone(),
                    model.invocation_capability(),
                    SourceProposalPolicy {
                        deployment_binding: model.digest(),
                        response_limit: execution.ordinary().response_limit(),
                        reservation_units: execution.ordinary().reservation_units(),
                    },
                )
                .unwrap_or_else(|_| panic!("actual checked Model adapter"));
                let model_sequence = journal.begin_session().unwrap().sequence();
                let resume_entries = crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8();
                let start_entries = crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries();

                let model = advance_live_owned_continued_model_v8(journal, prepared, &adapter)
                    .unwrap_or_else(|_| panic!("actual Model request and ACK"));

                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    model_sequence + 1
                );
                let (_, _, model_turn, _) = journal
                    .begin_session()
                    .unwrap()
                    .continued_model_facts()
                    .unwrap_or_else(|_| panic!("actual continued Model inventory"));
                assert_eq!(model_turn, 1);
                let settled_sequence = journal.begin_session().unwrap().sequence();
                let model = advance_live_owned_continued_dispatch_v8(journal, model, &mut adapter)
                    .unwrap_or_else(|_| panic!("sole SDK dispatch and Settled ACK"));
                assert_eq!(starts.get(), 1);
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    settled_sequence + 1
                );
                let resume_sequence = journal.begin_session().unwrap().sequence();
                let model =
                    advance_live_owned_continued_resume_v8(journal, model).unwrap_or_else(|_| {
                        panic!("Usage and Resume ACKs before actual resumed source")
                    });
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    resume_sequence + 2
                );
                let completed_sequence = journal.begin_session().unwrap().sequence();
                let model = advance_live_owned_continued_completed_v8(journal, model)
                    .unwrap_or_else(|_| panic!("actual Completed ACK"));
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    completed_sequence + 1
                );
                let authorize_sequence = journal.begin_session().unwrap().sequence();
                let authorization = advance_live_owned_continued_authorize_v8(journal, model)
                    .unwrap_or_else(|_| panic!("five actual authorization ACKs"));
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    authorize_sequence + 5
                );
                let effect_sequence = journal.begin_session().unwrap().sequence();
                let effect = advance_live_owned_continued_effect_v8(journal, authorization)
                    .unwrap_or_else(|failure| match failure {
                        LiveContinuedEffectDriverFailureV8::Prepare(_) => {
                            panic!("effect admission before Ready ACK")
                        }
                        LiveContinuedEffectDriverFailureV8::Next(_) => {
                            panic!("effect Consumed selection after promotion")
                        }
                        LiveContinuedEffectDriverFailureV8::Session { .. } => {
                            panic!("effect append session")
                        }
                        LiveContinuedEffectDriverFailureV8::Append(_) => {
                            panic!("effect physical append")
                        }
                        LiveContinuedEffectDriverFailureV8::Advance(_) => {
                            panic!("effect Ready promotion or Consumed advance")
                        }
                    });
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    effect_sequence + 2
                );
                let intent_sequence = journal.begin_session().unwrap().sequence();
                let intent = advance_live_owned_continued_intent_v8(journal, effect)
                    .unwrap_or_else(|_| panic!("actual effect preparation and Intent ACK"));
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    intent_sequence + 1
                );
                let settlement_sequence = journal.begin_session().unwrap().sequence();
                let mut host = ActualEffectProbe { calls: 0 };
                let recorded =
                    advance_live_owned_continued_effect_dispatch_v8(journal, intent, &mut host)
                        .unwrap_or_else(|_| {
                            panic!("one actual effect host dispatch and two settlement ACKs")
                        });
                assert_eq!(host.calls, 1, "actual effect host dispatch exactly once");
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    settlement_sequence + 2,
                    "ordinary settlement and owned settlement-record rows"
                );
                recorded
                    .validate_live()
                    .unwrap_or_else(|_| panic!("recorded settlement owner"));
                let settlement_session = journal.begin_session().unwrap();
                let (_, _, settlement_turn, settlement_row) = settlement_session
                    .continued_settlement_facts()
                    .unwrap_or_else(|_| panic!("recorded continued settlement inventory"));
                assert_eq!(settlement_turn, 1);
                assert!(matches!(
                    settlement_row,
                    crate::live_invocation::source_journal::owned_wait_v8::EntryV8::Owned(
                        crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded {
                            turn: 1,
                            attempt: 0,
                            settlement,
                            ..
                        }
                    ) if *settlement == u32::try_from(settlement_sequence).unwrap()
                ));
                let cleanup_sequence = journal.begin_session().unwrap().sequence();
                let cleanup_actions = Rc::new(Cell::new(0));
                let observed_cleanup_actions = Rc::clone(&cleanup_actions);
                let cleanup =
                    advance_live_owned_continued_cleanup_v8(journal, recorded, move |_| {
                        observed_cleanup_actions.set(observed_cleanup_actions.get() + 1);
                    })
                    .unwrap_or_else(|_| panic!("continued cleanup Started and Settled ACKs"));
                assert_eq!(
                    cleanup_actions.get(),
                    1,
                    "the recorded continuation releases its physical cleanup exactly once"
                );
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    cleanup_sequence + 2,
                    "cleanup Started and sticky-receipt Settled rows"
                );
                cleanup
                    .validate_live()
                    .unwrap_or_else(|_| panic!("settled continued cleanup owner"));
                let reduce_sequence = journal.begin_session().unwrap().sequence();
                let reserved = advance_live_owned_continued_reduce_v8(journal, cleanup)
                    .unwrap_or_else(|_| panic!("one continued Reduce reservation ACK"));
                assert_eq!(
                    journal.begin_session().unwrap().sequence(),
                    reduce_sequence + 1,
                    "one turn-1 Reduce reservation row"
                );
                reserved
                    .validate_live()
                    .unwrap_or_else(|_| panic!("turn-1 Reduce reservation owner"));
                let charged_funding = {
                    let current = journal.begin_session().unwrap();
                    let (r, s, _, _, _) = current.inventory.continued_reduce_facts().unwrap();
                    (r, s)
                };
                let accounting = *reserved.accounting();
                let evaluated = reserved
                    .evaluate()
                    .unwrap_or_else(|_| panic!("actual turn-1 physical reducer evaluation"));
                let facts = evaluated
                    .stage_facts()
                    .unwrap_or_else(|_| panic!("actual turn-1 reducer facts"));
                assert!(
                    facts.step().is_some(),
                    "the real reducer produced a full Step"
                );
                assert!(facts.consumed() <= facts.allowance());
                assert_eq!(*evaluated.accounting(), accounting);
                let selected = evaluated
                    .prepare_step()
                    .unwrap_or_else(|_| panic!("real turn-1 Step"));
                let selected_row = selected.selected_row().clone();
                assert!(matches!(&selected_row,
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceStaged {
                    turn: 1, attempt: 0, consumed, ..
                }) if *consumed == u64::try_from(facts.consumed()).unwrap()));
                let before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
                let sequence = journal.begin_session().unwrap().sequence();
                if fault == 1 {
                    #[cfg(unix)]
                    journal
                        .lease
                        .try_borrow_mut()
                        .unwrap()
                        .test_fail_before_write(sequence + 1);
                    #[cfg(not(unix))]
                    unreachable!("fault injection is Unix-only");
                    let failed = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_step(selected)
                        .err()
                        .expect("real append fault must refuse Step ACK");
                    assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::settlement::cleanup::reduce::step::LiveOwnedStepAppendFailureV8::Append { .. }));
                    assert_eq!(
                        [weak[0].strong_count(), weak[1].strong_count()],
                        [1, 0],
                        "failed append retains the carried State and retires the prior Outcome"
                    );
                    #[cfg(unix)]
                    assert_eq!(
                        journal
                            .lease
                            .try_borrow()
                            .unwrap()
                            .test_persisted_snapshot()
                            .unwrap(),
                        before,
                        "the injected prewrite fault left persisted bytes unchanged"
                    );
                    assert!(journal.begin_session().is_err());
                    assert!(journal.hold().is_err());
                    drop(failed);
                } else {
                    let acknowledged = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_step(selected)
                        .unwrap_or_else(|_| panic!("durable turn-1 Step ACK"));
                    acknowledged.validate_live().unwrap();
                    let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(staged) = acknowledged
                    .advance_step().unwrap_or_else(|_| panic!("same evaluated Step owner")) else {panic!("continued owner")};
                    staged.validate_live().unwrap();
                    assert_eq!(journal.begin_session().unwrap().sequence(), sequence + 1);
                    let after = journal.begin_session().unwrap();
                    let (reserved, stages, turn, attempt, last) =
                        after.inventory.step_reduce_facts().unwrap();
                    assert_eq!((turn, attempt), (1, 0));
                    assert_eq!(last, &selected_row);
                    let spent = staged.hold().unwrap();
                    spent
                        .validate_step_guard(journal, after.sequence(), after.acknowledged_bytes())
                        .unwrap();
                    assert_eq!((reserved, stages), charged_funding);
                    assert_ne!(
                        journal.lease.try_borrow_mut().unwrap().read().unwrap(),
                        before
                    );
                    assert_eq!(
                        [weak[0].strong_count(), weak[1].strong_count()],
                        [1, 0],
                        "ACKed holder retains the carried State and retires the prior Outcome"
                    );
                    let cleanup = staged
                        .prepare_cleanup()
                        .unwrap_or_else(|_| panic!("real turn-1 cleanup basis"));
                    assert!(
                        matches!(cleanup.selected_row(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupStarted {
                    turn: 1, attempt: 0, consumed, operations, ..
                }) if *consumed == u64::try_from(facts.consumed()).unwrap() && operations == facts.operations())
                    );
                    let cleanup_before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
                    if fault == 2 {
                        #[cfg(unix)]
                        journal
                            .lease
                            .try_borrow_mut()
                            .unwrap()
                            .test_fail_before_write(sequence + 2);
                        #[cfg(not(unix))]
                        unreachable!("fault injection is Unix-only");
                        let failed = journal
                            .begin_session()
                            .unwrap()
                            .append_owned_step(cleanup)
                            .err()
                            .expect("cleanup Started must not ACK through a prewrite fault");
                        assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::settlement::cleanup::reduce::step::LiveOwnedStepAppendFailureV8::Append { .. }));
                        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                        #[cfg(unix)]
                        assert_eq!(
                            journal
                                .lease
                                .try_borrow()
                                .unwrap()
                                .test_persisted_snapshot()
                                .unwrap(),
                            cleanup_before
                        );
                        assert!(journal.begin_session().is_err());
                        assert!(journal.hold().is_err());
                        drop(failed);
                    } else {
                        let acknowledged = journal
                            .begin_session()
                            .unwrap()
                            .append_owned_step(cleanup)
                            .unwrap_or_else(|_| panic!("durable turn-1 cleanup Started ACK"));
                        acknowledged.validate_live().unwrap();
                        let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(started) = acknowledged.advance_step()
                        .unwrap_or_else(|_| panic!("same staged reducer after cleanup Started")) else {panic!("continued cleanup owner")};
                        started.validate_live().unwrap();
                        assert_eq!(journal.begin_session().unwrap().sequence(), sequence + 2);
                        assert!(matches!(journal.begin_session().unwrap().inventory.step_reduce_facts().unwrap().4,
                        EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupStarted { turn: 1, attempt: 0, .. })));
                        assert_eq!([weak[0].strong_count(), weak[1].strong_count()], [1, 0]);
                        let mut step_cleanup_actions = 0;
                        let released = started
                            .release(|_| step_cleanup_actions += 1)
                            .unwrap_or_else(|_| panic!("one physical turn-1 Step cleanup"));
                        assert_eq!(step_cleanup_actions, 1);
                        released.validate_live().unwrap();
                        let receipt = released
                            .prepare_receipt()
                            .unwrap_or_else(|_| panic!("actual turn-1 cleanup receipt"));
                        let selected_receipt = receipt.selected_row().clone();
                        assert!(matches!(&selected_receipt,
                        EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceCleanupSettled {
                            turn: 1, attempt: 0, receipt, ..
                        }) if receipt["settlement"] == "completed" && receipt["operations"].as_array().is_some_and(|a| a.len() == 1)));
                        let receipt_before =
                            journal.lease.try_borrow_mut().unwrap().read().unwrap();
                        if fault == 3 {
                            #[cfg(unix)]
                            journal
                                .lease
                                .try_borrow_mut()
                                .unwrap()
                                .test_fail_before_write(sequence + 3);
                            #[cfg(not(unix))]
                            unreachable!("fault injection is Unix-only");
                            let failed = journal
                                .begin_session()
                                .unwrap()
                                .append_owned_step(receipt)
                                .err()
                                .expect("receipt cannot ACK through a prewrite fault");
                            assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::settlement::cleanup::reduce::step::LiveOwnedStepAppendFailureV8::Append { .. }));
                            assert_eq!(
                                step_cleanup_actions, 1,
                                "postrelease fault cannot rerun cleanup"
                            );
                            #[cfg(unix)]
                            assert_eq!(
                                journal
                                    .lease
                                    .try_borrow()
                                    .unwrap()
                                    .test_persisted_snapshot()
                                    .unwrap(),
                                receipt_before
                            );
                            assert!(journal.begin_session().is_err());
                            assert!(journal.hold().is_err());
                            drop(failed);
                        } else {
                            let acknowledged = journal
                                .begin_session()
                                .unwrap()
                                .append_owned_step(receipt)
                                .unwrap_or_else(|_| panic!("turn-1 cleanup receipt ACK"));
                            acknowledged.validate_live().unwrap();
                            let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(settled) = acknowledged.advance_step()
                            .unwrap_or_else(|_| panic!("same released turn-1 owner")) else {panic!("continued receipt owner")};
                            settled.validate_live().unwrap();
                            assert_eq!(journal.begin_session().unwrap().sequence(), sequence + 3);
                            assert_eq!(
                                journal
                                    .begin_session()
                                    .unwrap()
                                    .inventory
                                    .step_reduce_facts()
                                    .unwrap()
                                    .4,
                                &selected_receipt
                            );
                            assert_eq!(step_cleanup_actions, 1);
                            let ready = settled
                                .into_ready()
                                .unwrap_or_else(|_| panic!("one-use ReadyStep after receipt"));
                            let transfer = ready
                                .prepare_transfer()
                                .unwrap_or_else(|_| panic!("actual turn-1 transfer reservation"));
                            let transfer_row = transfer.selected_row().clone();
                            assert!(matches!(&transfer_row, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStepTransferReserved { turn: 1, attempt: 0, .. })));
                            let transfer_before =
                                journal.lease.try_borrow_mut().unwrap().read().unwrap();
                            if fault == 4 {
                                #[cfg(unix)]
                                journal
                                    .lease
                                    .try_borrow_mut()
                                    .unwrap()
                                    .test_fail_before_write(sequence + 4);
                                #[cfg(not(unix))]
                                unreachable!("fault injection is Unix-only");
                                let failed = journal
                                    .begin_session()
                                    .unwrap()
                                    .append_owned_step(transfer)
                                    .err()
                                    .expect(
                                        "transfer reservation cannot ACK through prewrite fault",
                                    );
                                assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::settlement::cleanup::reduce::step::LiveOwnedStepAppendFailureV8::Append { .. }));
                                assert_eq!(step_cleanup_actions, 1);
                                #[cfg(unix)]
                                assert_eq!(
                                    journal
                                        .lease
                                        .try_borrow()
                                        .unwrap()
                                        .test_persisted_snapshot()
                                        .unwrap(),
                                    transfer_before
                                );
                                assert!(journal.begin_session().is_err());
                                assert!(journal.hold().is_err());
                                drop(failed);
                            } else {
                                let acknowledged = journal
                                    .begin_session()
                                    .unwrap()
                                    .append_owned_step(transfer)
                                    .unwrap_or_else(|_| panic!("turn-1 transfer reservation ACK"));
                                let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(ready) = acknowledged.advance_step()
                                .unwrap_or_else(|_| panic!("same ReadyStep after transfer ACK")) else {panic!("continued ReadyStep")};
                                ready.validate_live().unwrap();
                                let moved = ready
                                    .move_fields()
                                    .unwrap_or_else(|_| panic!("actual turn-1 Step field move"));
                                assert_eq!(
                                    moved.kind(),
                                    Some(if three_turns { "continue" } else { "complete" })
                                );
                                let completed = moved
                                    .prepare_completed()
                                    .unwrap_or_else(|_| panic!("actual mapped target"));
                                let completed_row = completed.selected_row().clone();
                                assert!(matches!(&completed_row, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStepTransferCompleted { turn: 1, attempt: 0, .. })));
                                let acknowledged = journal
                                    .begin_session()
                                    .unwrap()
                                    .append_owned_step(completed)
                                    .unwrap_or_else(|_| panic!("turn-1 transfer completion ACK"));
                                let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(moved) = acknowledged.advance_step()
                                .unwrap_or_else(|_| panic!("same mapped owner after completion")) else {panic!("continued mapped owner")};
                                let transition = moved
                                    .prepare_transition()
                                    .unwrap_or_else(|_| panic!("actual turn-1 Transition"));
                                assert!(
                                    matches!(transition.selected_row(), EntryV8::Ordinary(SourceJournalEntry::Transition { turn: 1, attempt: 0, case, .. }) if *case == if three_turns { crate::live_invocation::source_journal::SourceTransitionCase::Continue } else { crate::live_invocation::source_journal::SourceTransitionCase::Complete })
                                );
                                let acknowledged = journal
                                    .begin_session()
                                    .unwrap()
                                    .append_owned_step(transition)
                                    .unwrap_or_else(|_| panic!("turn-1 Transition ACK"));
                                let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(mapped) = acknowledged.advance_step()
                                .unwrap_or_else(|_| panic!("same mapped owner after Transition")) else {panic!("continued Transition owner")};
                                mapped.validate_live().unwrap();
                                assert_eq!(
                                    journal.begin_session().unwrap().sequence(),
                                    sequence + 6
                                );
                                let after = journal.begin_session().unwrap();
                                let (r, s, turn, attempt, last) =
                                    after.inventory.step_reduce_facts().unwrap();
                                assert_eq!(
                                    (r, s, turn, attempt),
                                    (charged_funding.0, charged_funding.1, 1, 0)
                                );
                                assert!(
                                    matches!(last, EntryV8::Ordinary(SourceJournalEntry::Transition { case, .. }) if *case == if three_turns { crate::live_invocation::source_journal::SourceTransitionCase::Continue } else { crate::live_invocation::source_journal::SourceTransitionCase::Complete })
                                );
                                assert_eq!(step_cleanup_actions, 1);
                                if three_turns {
                                    let before =
                                        journal.lease.try_borrow_mut().unwrap().read().unwrap();
                                    if fault == 6 {
                                        #[cfg(unix)]
                                        journal
                                            .lease
                                            .try_borrow_mut()
                                            .unwrap()
                                            .test_fail_before_write(sequence + 7);
                                        #[cfg(not(unix))]
                                        unreachable!("fault injection is Unix-only");
                                    }
                                    mapped.validate_live().unwrap_or_else(|error| {
                                        panic!(
                                        "mapped turn-1 Step became stale before handoff: {error:?}"
                                    )
                                    });
                                    let (_, old_turn) =
                                        mapped.continue_transition().unwrap_or_else(|error| {
                                            panic!("turn-1 Continue transition cursor: {error:?}")
                                        });
                                    assert_eq!(old_turn, 1);
                                    let target = mapped.continue_target().unwrap_or_else(|error| {
                                        panic!("turn-1 mapped target: {error:?}")
                                    });
                                    assert_eq!(target["kind"], "continue");
                                    assert!(target.get("state").is_some());
                                    let inputs = mapped.continue_inputs().unwrap_or_else(|error| {
                                        panic!("turn-1 physical inputs: {error:?}")
                                    });
                                    assert_eq!(
                                        (inputs.turn, inputs.attempt),
                                        (old_turn, 0),
                                        "mapped Step must retain the current effect coordinates"
                                    );
                                    let (_, execution) = journal.context().ready_runtime().unwrap();
                                    assert!(old_turn + 1 < execution.ordinary().max_iterations());
                                    mapped.continued_model_origin().unwrap_or_else(|error| {
                                        panic!("turn-1 model origin: {error:?}")
                                    });
                                    let advanced = super::advance_live_owned_later_continue_v8(
                                        journal, mapped,
                                    );
                                    if fault == 6 {
                                        let failed =
                                            advanced.err().expect("turn-2 State prewrite refusal");
                                        assert!(matches!(
                                            &failed,
                                            super::LiveLaterContinueDriverFailureV8::StateAppend(_)
                                        ));
                                        #[cfg(unix)]
                                        assert_eq!(
                                            journal
                                                .lease
                                                .try_borrow()
                                                .unwrap()
                                                .test_persisted_snapshot()
                                                .unwrap(),
                                            before
                                        );
                                        assert!(weak.iter().any(|owner| owner.strong_count() == 1));
                                        assert!(journal.begin_session().is_err());
                                        drop(failed);
                                    } else {
                                        let observed = advanced.unwrap_or_else(|failure| {
                                        use super::LiveLaterContinueDriverFailureV8 as D;
                                        use super::LiveOwnedLaterContinueAppendFailureV8 as A;
                                        use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::later::LiveLaterContinueFailureV8 as L;
                                        let append = |failure: &A<'_>| match failure {
                                            A::Before { error, .. } => format!("before: {error:?}"),
                                            A::Append { failure, .. } => match failure {
                                                crate::live_invocation::source_journal::owned_wait_v8::append::AppendFailureV8::PhysicalBeforeCandidate { error, .. } => format!("physical before candidate: {error:?}"),
                                                crate::live_invocation::source_journal::owned_wait_v8::append::AppendFailureV8::CandidateRefused { error, .. } => format!("candidate refused: {error:?}"),
                                                crate::live_invocation::source_journal::owned_wait_v8::append::AppendFailureV8::PrewriteRefused { error, .. } => format!("prewrite refused: {error:?}"),
                                                crate::live_invocation::source_journal::owned_wait_v8::append::AppendFailureV8::InDoubt { error, .. } => format!("in doubt: {error:?}"),
                                            },
                                            A::Acknowledged { error, .. } => format!("acknowledged: {error:?}"),
                                            A::After { error, .. } => format!("after: {error:?}"),
                                        };
                                        let later = |failure: &L<'_>| match failure {
                                            L::Before { error, .. } => format!("before: {error:?}"),
                                            L::State { error, .. } => format!("state: {error:?}"),
                                            L::Acknowledged { error, .. } => format!("acknowledged: {error:?}"),
                                            L::Observe { owner, .. } => match owner {
                                                crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveContinuedObserveFailureV8::Before { error, .. } => format!("physical Observe before: {error:?}"),
                                                crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveContinuedObserveFailureV8::Committed { error, .. } => format!("physical Observe committed: {error:?}"),
                                                crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveContinuedObserveFailureV8::After { error, .. } => format!("physical Observe after: {error:?}"),
                                            },
                                            L::After { error, .. } => format!("after: {error:?}"),
                                        };
                                        let detail = match &failure {
                                            D::Prepare(f) => format!("prepare {}", later(f)),
                                            D::StateSession { error, .. } => format!("State session: {error:?}"),
                                            D::StateAppend(f) => format!("State append {}", append(f)),
                                            D::StateAdvance(f) => format!("State advance {}", later(f)),
                                            D::StateAcknowledged(_) => "State ACK phase mismatch".to_owned(),
                                            D::ObservePrepare(f) => format!("Observe prepare {}", later(f)),
                                            D::ObserveSession { error, .. } => format!("Observe session: {error:?}"),
                                            D::ObserveAppend(f) => format!("Observe append {}", append(f)),
                                            D::ObserveAdvance(f) => format!("Observe advance {}", later(f)),
                                            D::ObserveAcknowledged(_) => "Observe ACK phase mismatch".to_owned(),
                                        };
                                        panic!("turn-2 physical Observe and two exact ACKs: {detail}")
                                    });
                                        assert_eq!(observed.is_observed(), !later_observe_ensures);
                                        assert_eq!(observed.turn(), 2);
                                        let after = journal.begin_session().unwrap();
                                        let (reserved, stages, turn, last) =
                                            after.inventory.continuation_facts().unwrap();
                                        let fuel = journal
                                            .context()
                                            .ordinary()
                                            .max_steps_per_stage()
                                            .unwrap()
                                            as u64;
                                        assert_eq!(turn, 2);
                                        assert_eq!(reserved, charged_funding.0 + fuel);
                                        assert_eq!(stages, charged_funding.1 + 1);
                                        assert!(matches!(last, EntryV8::Ordinary(SourceJournalEntry::StageReservation { turn: 2, attempt: None, role: crate::live_invocation::source_journal::SourceStageRole::Observe, .. })));
                                        assert!(weak.iter().any(|owner| owner.strong_count() == 1));
                                        let settlement =
                                            observed.prepare_observe_settlement().unwrap_or_else(
                                                |_| panic!("turn-2 physical Observe settlement"),
                                            );
                                        assert_eq!(
                                        matches!(settlement.selected(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedObserveSettled { turn: 2, settlement: crate::live_invocation::source_journal::owned_wait_v8::model::ObserveSettlementV8::Failed { .. }, .. })),
                                        later_observe_ensures,
                                    );
                                        let settlement_before =
                                            journal.lease.try_borrow_mut().unwrap().read().unwrap();
                                        if fault == 7 {
                                            #[cfg(unix)]
                                            journal
                                                .lease
                                                .try_borrow_mut()
                                                .unwrap()
                                                .test_fail_before_write(after.sequence() + 1);
                                            #[cfg(not(unix))]
                                            unreachable!("fault injection is Unix-only");
                                            let failed = journal
                                                .begin_session()
                                                .unwrap()
                                                .append_owned_observe_settlement(settlement)
                                                .err()
                                                .expect("turn-2 settlement prewrite refusal");
                                            assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::observe_settlement::LiveOwnedObserveSettlementAppendFailureV8::Append { .. }));
                                            #[cfg(unix)]
                                            assert_eq!(
                                                journal
                                                    .lease
                                                    .try_borrow()
                                                    .unwrap()
                                                    .test_persisted_snapshot()
                                                    .unwrap(),
                                                settlement_before
                                            );
                                            assert!(journal.begin_session().is_err());
                                            assert!(weak
                                                .iter()
                                                .any(|owner| owner.strong_count() == 1));
                                            drop(failed);
                                        } else {
                                            let settled = journal
                                                .begin_session()
                                                .unwrap()
                                                .append_owned_observe_settlement(settlement)
                                                .unwrap_or_else(|_| {
                                                    panic!("turn-2 Observe settlement ACK")
                                                })
                                                .advance_observe_settlement()
                                                .unwrap_or_else(|_| {
                                                    panic!(
                                                        "retained turn-2 Observe settlement owner"
                                                    )
                                                });
                                            if later_observe_ensures {
                                                let before_cleanup =
                                                    journal.begin_session().unwrap();
                                                let (reserved, stages, turn, _, _, _, _) =
                                                    before_cleanup
                                                        .failed_observe_cleanup_facts()
                                                        .unwrap();
                                                assert_eq!(
                                                    (reserved, stages, turn),
                                                    (
                                                        charged_funding.0 + fuel,
                                                        charged_funding.1 + 1,
                                                        2
                                                    )
                                                );
                                                #[cfg(unix)]
                                                let persisted_before_cleanup = journal
                                                    .lease
                                                    .try_borrow_mut()
                                                    .unwrap()
                                                    .read()
                                                    .unwrap();
                                                if matches!(fault, 21..=23) {
                                                    #[cfg(unix)]
                                                    journal
                                                        .lease
                                                        .try_borrow_mut()
                                                        .unwrap()
                                                        .test_fail_before_write(
                                                            before_cleanup.sequence()
                                                                + usize::from(fault - 20),
                                                        );
                                                }
                                                let mut failed_cleanup_actions = 0;
                                                let result = crate::live_invocation::source_journal::owned_wait_v8::live_upstream::observe::settlement::failed_state::stop_failed_observe_state_v8(settled, |_| failed_cleanup_actions += 1);
                                                if matches!(fault, 21..=23) {
                                                    let quarantined = result.err().expect("later failed Observe persistence refusal retains the reached State owner");
                                                    assert_eq!(
                                                        quarantined.status(),
                                                        SourceJournalError::Poisoned
                                                    );
                                                    let expected_actions = journal
                                                        .context()
                                                        .ready_runtime()
                                                        .unwrap()
                                                        .1
                                                        .wait()
                                                        .observe()
                                                        .helper()
                                                        .liveness()
                                                        .failure_cleanup
                                                        .len();
                                                    assert_eq!(
                                                        failed_cleanup_actions,
                                                        if fault == 21 { 0 } else { expected_actions },
                                                        "only the Started prewrite refusal can avoid the one physical cleanup",
                                                    );
                                                    assert!(weak
                                                        .iter()
                                                        .any(|owner| owner.strong_count() == 1));
                                                    assert!(journal.begin_session().is_err());
                                                    #[cfg(unix)]
                                                    assert_eq!(
                                                        journal
                                                            .lease
                                                            .try_borrow()
                                                            .unwrap()
                                                            .test_persisted_snapshot()
                                                            .unwrap(),
                                                        persisted_before_cleanup
                                                    );
                                                    drop(quarantined);
                                                } else {
                                                    let stopped = result.unwrap_or_else(|_| panic!("later failed Observe cleanup and sticky Stop"));
                                                    let expected_actions = journal
                                                        .context()
                                                        .ready_runtime()
                                                        .unwrap()
                                                        .1
                                                        .wait()
                                                        .observe()
                                                        .helper()
                                                        .liveness()
                                                        .failure_cleanup
                                                        .len();
                                                    assert_eq!(
                                                        failed_cleanup_actions,
                                                        expected_actions
                                                    );
                                                    let current = journal.begin_session().unwrap();
                                                    let (r, s, t, _) = current
                                                        .test_observe_inventory()
                                                        .failed_observe_cleanup_current_facts()
                                                        .unwrap();
                                                    assert_eq!(
                                                        (r, s, t),
                                                        (reserved, stages, turn),
                                                        "State cleanup cannot recharge a stage"
                                                    );
                                                    assert!(matches!(current.test_observe_inventory().test_observe_entries().last().unwrap().entry,
                                                    EntryV8::Ordinary(SourceJournalEntry::Stop { turn: Some(2), attempt: None, status: crate::live_invocation::source_journal::SourceStopStatus::Rejected, reason: crate::live_invocation::source_journal::SourceStopReason::StageRefused })
                                                ));
                                                    drop(stopped);
                                                }
                                                assert_eq!(host.calls, 1);
                                                assert_eq!(starts.get(), 1);
                                                assert_eq!(cleanup_actions.get(), 1);
                                                assert_eq!(step_cleanup_actions, 1);
                                                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8(), resume_entries + 1);
                                                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries(), start_entries);
                                                assert!(weak
                                                    .iter()
                                                    .all(|owner| owner.upgrade().is_none()));
                                                return false;
                                            }
                                            let observed_row =
                                                settled.prepare_turn_observed().unwrap_or_else(
                                                    |_| panic!("turn-2 TurnObserved candidate"),
                                                );
                                            assert!(matches!(
                                                observed_row.selected(),
                                                EntryV8::Ordinary(
                                                    SourceJournalEntry::TurnObserved {
                                                        turn: 2,
                                                        ..
                                                    }
                                                )
                                            ));
                                            let settled = journal
                                                .begin_session()
                                                .unwrap()
                                                .append_owned_observe_settlement(observed_row)
                                                .unwrap_or_else(|_| {
                                                    panic!("turn-2 TurnObserved ACK")
                                                })
                                                .advance_observe_settlement()
                                                .unwrap_or_else(|_| {
                                                    panic!("retained turn-2 observed owner")
                                                });
                                            let final_session = journal.begin_session().unwrap();
                                            let (
                                                final_reserved,
                                                final_stages,
                                                final_turn,
                                                final_row,
                                            ) = final_session
                                                .inventory
                                                .observe_settlement_facts()
                                                .unwrap();
                                            assert_eq!(
                                                (final_reserved, final_stages, final_turn),
                                                (reserved, stages, 2)
                                            );
                                            assert!(matches!(
                                                final_row,
                                                EntryV8::Ordinary(
                                                    SourceJournalEntry::TurnObserved {
                                                        turn: 2,
                                                        ..
                                                    }
                                                )
                                            ));
                                            assert!(weak
                                                .iter()
                                                .any(|owner| owner.strong_count() == 1));
                                            let carried =
                                                settled.into_later_wait().unwrap_or_else(|_| {
                                                    panic!(
                                                    "physical turn-2 State and observation carry"
                                                )
                                                });
                                            carried.validate_live().unwrap();
                                            let created =
                                                carried.prepare_start_created().unwrap_or_else(
                                                    |_| panic!("turn-2 Created from actual carry"),
                                                );
                                            assert!(matches!(created.selected(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitCreated { turn: 2, attempt: 0, .. })));
                                            let created_before = journal
                                                .lease
                                                .try_borrow_mut()
                                                .unwrap()
                                                .read()
                                                .unwrap();
                                            if fault == 8 {
                                                #[cfg(unix)]
                                                journal
                                                    .lease
                                                    .try_borrow_mut()
                                                    .unwrap()
                                                    .test_fail_before_write(
                                                        final_session.sequence() + 1,
                                                    );
                                                #[cfg(not(unix))]
                                                unreachable!("fault injection is Unix-only");
                                                let failed = journal
                                                    .begin_session()
                                                    .unwrap()
                                                    .append_owned_later_start(created)
                                                    .err()
                                                    .expect("turn-2 Created prewrite refusal");
                                                assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_start::LiveOwnedLaterStartAppendFailureV8::Append { .. }));
                                                #[cfg(unix)]
                                                assert_eq!(
                                                    journal
                                                        .lease
                                                        .try_borrow()
                                                        .unwrap()
                                                        .test_persisted_snapshot()
                                                        .unwrap(),
                                                    created_before
                                                );
                                                assert!(journal.begin_session().is_err());
                                                assert!(weak
                                                    .iter()
                                                    .any(|owner| owner.strong_count() == 1));
                                                drop(failed);
                                            } else {
                                                let created = journal
                                                    .begin_session()
                                                    .unwrap()
                                                    .append_owned_later_start(created)
                                                    .unwrap_or_else(|_| {
                                                        panic!("turn-2 Created ACK")
                                                    })
                                                    .advance_later_start()
                                                    .unwrap_or_else(|_| {
                                                        panic!("retained turn-2 Created owner")
                                                    });
                                                created.validate_live().unwrap();
                                                let reserved = created
                                                    .prepare_start_reservation()
                                                    .unwrap_or_else(|_| {
                                                        panic!("turn-2 original Start reservation")
                                                    });
                                                let (_, execution) =
                                                    journal.context().ready_runtime().unwrap();
                                                assert!(
                                                    matches!(reserved.selected(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved { turn: 2, attempt: 0, phase: crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Start, replay_of: None, fuel, .. }) if *fuel == execution.evaluation_fuel() as u64)
                                                );
                                                if fault == 9 {
                                                    let before = journal
                                                        .lease
                                                        .try_borrow_mut()
                                                        .unwrap()
                                                        .read()
                                                        .unwrap();
                                                    #[cfg(unix)]
                                                    journal
                                                        .lease
                                                        .try_borrow_mut()
                                                        .unwrap()
                                                        .test_fail_before_write(
                                                            final_session.sequence() + 2,
                                                        );
                                                    #[cfg(not(unix))]
                                                    unreachable!("fault injection is Unix-only");
                                                    let failed = journal
                                                        .begin_session()
                                                        .unwrap()
                                                        .append_owned_later_start(reserved)
                                                        .err()
                                                        .expect("turn-2 Start prewrite refusal");
                                                    assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::continued_start::LiveOwnedLaterStartAppendFailureV8::Append { .. }));
                                                    #[cfg(unix)]
                                                    assert_eq!(
                                                        journal
                                                            .lease
                                                            .try_borrow()
                                                            .unwrap()
                                                            .test_persisted_snapshot()
                                                            .unwrap(),
                                                        before
                                                    );
                                                    assert!(journal.begin_session().is_err());
                                                    assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries(), start_entries);
                                                    assert!(weak
                                                        .iter()
                                                        .any(|owner| owner.strong_count() == 1));
                                                    drop(failed);
                                                } else {
                                                    let started = journal
                                                        .begin_session()
                                                        .unwrap()
                                                        .append_owned_later_start(reserved)
                                                        .unwrap_or_else(|_| {
                                                            panic!("turn-2 Start reservation ACK")
                                                        })
                                                        .advance_later_start()
                                                        .unwrap_or_else(|_| {
                                                            panic!("retained turn-2 Start owner")
                                                        });
                                                    started.validate_live().unwrap();
                                                    assert_eq!(
                                                        started.sequence(),
                                                        final_session.sequence() + 2
                                                    );
                                                    assert!(weak
                                                        .iter()
                                                        .any(|owner| owner.strong_count() == 1));
                                                    let entered = started
                                                        .enter_actual_source()
                                                        .unwrap_or_else(|_| {
                                                            panic!("one-use turn-2 source entry")
                                                        });
                                                    entered.validate_live().unwrap();
                                                    let (state, request, ordinary_steps) =
                                                        entered.test_ordinary_start();
                                                    assert_eq!(
                                                        entered.consumed(),
                                                        Some(ordinary_steps as u64)
                                                    );
                                                    assert_eq!(
                                                        entered.test_accounting(),
                                                        &accounting
                                                    );
                                                    assert_eq!(crate::interpreter::resumable::checkpoint::channel_json(&request), entered.test_observation().copy_arguments()[0]["value"]);
                                                    assert_eq!(state, target["state"]);
                                                    assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries(), start_entries + 1);
                                                    assert!(weak
                                                        .iter()
                                                        .any(|owner| owner.strong_count() == 1));
                                                    prepared::run(
                                                        journal,
                                                        entered,
                                                        &mut adapter,
                                                        fault,
                                                        &weak,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    let (mapped, refusal) =
                                        mapped.claim_complete_report().err().expect(
                                            "Transition alone cannot claim the physical Report",
                                        );
                                    assert_eq!(refusal, crate::live_invocation::source_journal::SourceJournalError::Order);
                                    mapped.validate_live().unwrap();
                                    assert!(journal.terminal_evidence().is_err(), "a terminal Transition alone cannot be recovered as a terminal receipt");
                                    let input = crate::live_invocation::source_journal::SourceTerminalEvidenceInput {
                                completed_stages: s,
                                omitted_stage_rows: s,
                                stage_rows: Vec::new(),
                                checked_run_evidence: None,
                            };
                                    let terminal =
                                        mapped.prepare_terminal(input).unwrap_or_else(|_| {
                                            panic!("actual terminal publication candidate")
                                        });
                                    assert!(
                                        matches!(terminal.selected_row(), EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
                                turn: Some(1), status: crate::live_invocation::source_journal::SourceTerminalStatus::Complete,
                                committed_stage_fuel, stages, carrier: Some(_), ..
                            }) if *committed_stage_fuel == charged_funding.0 && *stages == s)
                                    );
                                    let terminal_before =
                                        journal.lease.try_borrow_mut().unwrap().read().unwrap();
                                    if fault == 5 {
                                        #[cfg(unix)]
                                        journal
                                            .lease
                                            .try_borrow_mut()
                                            .unwrap()
                                            .test_fail_before_write(sequence + 7);
                                        #[cfg(not(unix))]
                                        unreachable!("fault injection is Unix-only");
                                        let failed = journal
                                    .begin_session()
                                    .unwrap()
                                    .append_owned_step(terminal)
                                    .err()
                                    .expect(
                                        "terminal publication cannot ACK through prewrite fault",
                                    );
                                        assert!(matches!(&failed, crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::settlement::cleanup::reduce::step::LiveOwnedStepAppendFailureV8::Append { .. }));
                                        #[cfg(unix)]
                                        assert_eq!(
                                            journal
                                                .lease
                                                .try_borrow()
                                                .unwrap()
                                                .test_persisted_snapshot()
                                                .unwrap(),
                                            terminal_before
                                        );
                                        assert!(journal.begin_session().is_err());
                                        assert!(journal.hold().is_err());
                                        assert!(journal.terminal_evidence().is_err());
                                        assert_eq!(step_cleanup_actions, 1);
                                        drop(failed);
                                    } else {
                                        let acknowledged = journal
                                            .begin_session()
                                            .unwrap()
                                            .append_owned_step(terminal)
                                            .unwrap_or_else(|_| panic!("terminal publication ACK"));
                                        let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(terminal_owner) = acknowledged.advance_step()
                                    .unwrap_or_else(|_| panic!("same mapped owner after terminal ACK")) else { panic!("continued terminal owner") };
                                        terminal_owner.validate_live().unwrap();
                                        let final_session = journal.begin_session().unwrap();
                                        assert_eq!(final_session.sequence(), sequence + 7);
                                        let (r, final_stages, turn, attempt, last) =
                                            final_session.inventory.step_reduce_facts().unwrap();
                                        assert_eq!(
                                            (r, final_stages, turn, attempt),
                                            (charged_funding.0, s, 1, 0)
                                        );
                                        assert!(matches!(last, EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot { status: crate::live_invocation::source_journal::SourceTerminalStatus::Complete, .. })));
                                        let recovered = journal.terminal_evidence().unwrap();
                                        assert_eq!(recovered.status(), crate::live_invocation::source_journal::SourceTerminalStatus::Complete);
                                        assert!(!recovered.evidence().is_empty());
                                        assert!(recovered.carrier().is_some());
                                        let claimed = terminal_owner
                                        .claim_complete_report()
                                        .unwrap_or_else(|_| {
                                            panic!("one-use actual Report claim after terminal ACK")
                                        });
                                        let delivered = claimed.delivery_projection().unwrap();
                                        assert_eq!(delivered["kind"], "complete");
                                        assert!(delivered["report"]["fields"].as_array().is_some());
                                        assert!(
                                            weak.iter().any(|owner| owner.strong_count() == 1),
                                            "claimed Report retains an original physical leaf"
                                        );
                                        drop(claimed);
                                    }
                                }
                            }
                        }
                    }
                }
                assert_eq!(
                    host.calls, 1,
                    "Reduce reservation never redispatches the effect host"
                );
                assert_eq!(
                    starts.get(),
                    1 + usize::from(
                        three_turns
                            && (matches!(fault, 0 | 12..=20)
                                || (fault == 21 && !later_observe_ensures))
                    ),
                    "turn-two physical Model dispatch occurs only after its Intent ACK"
                );
                assert_eq!(
                    cleanup_actions.get(),
                    1,
                    "Reduce reservation never repeats cleanup"
                );
                assert_eq!(
            crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8(),
            resume_entries + 1 + usize::from(three_turns && (matches!(fault, 0 | 15..=20) || (fault == 21 && !later_observe_ensures))),
            "only each exact Resume ACK may consume its own physical park"
        );
                assert_eq!(
                crate::interpreter::resumable::owned_frame::registered_stage::reduce::PreparedHeldContinuedWaitV2::test_start_entries(),
                start_entries + usize::from(three_turns && (matches!(fault, 0 | 10..=20) || (fault == 21 && !later_observe_ensures))),
                "only the turn-two Start ACK may enter the source helper again"
            );
                assert!(weak.iter().all(|owner| owner.upgrade().is_none()));
                (fault == 0 && !three_turns) || fault == 18
            },
        )
    };
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run)
        .expect("real-chain test thread")
        .join()
        .expect("real-chain test completion");
}
#[test]
fn owned_continue_driver_dispatches_next_turn_model_once_and_records_settlement() {
    continued_reduce_chain_step_ack(0, false, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_real_prewrite_fault_retains_owner_and_poison() {
    continued_reduce_chain_step_ack(1, false, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_cleanup_started_prewrite_fault_retains_staged_owner() {
    continued_reduce_chain_step_ack(2, false, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_cleanup_receipt_prewrite_fault_keeps_release_sticky() {
    continued_reduce_chain_step_ack(3, false, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_transfer_prewrite_fault_never_moves_ready_fields() {
    continued_reduce_chain_step_ack(4, false, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_terminal_prewrite_fault_retains_mapped_owner_and_poison() {
    continued_reduce_chain_step_ack(5, false, false);
}
#[test]
fn owned_continued_step_hands_real_state_to_turn_two_and_acks_observe() {
    continued_reduce_chain_step_ack(0, true, false);
}
#[test]
fn owned_continued_step_turn_two_state_prewrite_refusal_retains_mapped_owner() {
    continued_reduce_chain_step_ack(6, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_settlement_prewrite_refusal_retains_observed_owner() {
    continued_reduce_chain_step_ack(7, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_created_prewrite_refusal_retains_later_owner() {
    continued_reduce_chain_step_ack(8, true, false);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_start_prewrite_refusal_never_enters_source() {
    continued_reduce_chain_step_ack(9, true, false);
}
#[test]
fn owned_continued_step_turn_two_failed_observe_cleans_state_and_stops() {
    continued_reduce_chain_step_ack(0, true, true);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_failed_observe_started_prewrite_retains_state() {
    continued_reduce_chain_step_ack(21, true, true);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_failed_observe_receipt_prewrite_keeps_cleanup_sticky() {
    continued_reduce_chain_step_ack(22, true, true);
}
#[test]
#[cfg(unix)]
fn owned_continued_step_turn_two_failed_observe_stop_prewrite_retains_released_state() {
    continued_reduce_chain_step_ack(23, true, true);
}
mod actual_state;
mod composed;
mod faults;
mod prepared;
