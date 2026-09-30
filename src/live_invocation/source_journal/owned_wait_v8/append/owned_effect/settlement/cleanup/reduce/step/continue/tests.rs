//! Genuine Continue owner and fixed original ACKs; no reconstructed State.
use super::*;
use crate::agent_lifecycle::iterative::source_live::SourceProposalPolicy;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveMovedStepV8;
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
    ),
) {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
        true,
        |context, lease, key, _| {
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
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock { now: Cell::new(1) };
            super::super::tests::test_moved(&journal, &cancel, &policy, &clock, |moved, weak| {
                callback(&journal, moved, weak, &cancel, &clock)
            });
        },
    );
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
    });
}
#[test]
fn owned_continue_driver_dispatches_next_turn_model_once_and_records_settlement() {
    with_moved(|journal, moved, weak, _, _| {
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

        let model = advance_live_owned_continued_model_v8(journal, prepared, &adapter)
            .unwrap_or_else(|_| panic!("actual Model request and ACK"));

        assert_eq!(journal.begin_session().unwrap().sequence(), model_sequence + 1);
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
        assert_eq!(journal.begin_session().unwrap().sequence(), settled_sequence + 1);
        let resume_sequence = journal.begin_session().unwrap().sequence();
        let model = advance_live_owned_continued_resume_v8(journal, model)
            .unwrap_or_else(|_| panic!("Usage and Resume ACKs before actual resumed source"));
        assert_eq!(journal.begin_session().unwrap().sequence(), resume_sequence + 2);
        assert_eq!(
            crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8(),
            resume_entries + 1,
            "only the fourth Model ACK may resume source"
        );
        assert!(weak.iter().any(|owner| owner.strong_count() == 1));

        drop(model);
        assert!(weak.iter().all(|owner| owner.upgrade().is_none()));
    });
}
#[test]
fn owned_continue_actual_state_and_observe_acks_preserve_owner_ledger_and_cumulative_funding() {
    with_moved(|journal, moved, weak, _, _| {
        let before = journal.begin_session().unwrap();
        let (r, s, turn, _) = before.inventory.continuation_facts().unwrap();
        let (ordinary_observation, ordinary_consumed) = moved.test_observe_oracle();
        let ledger = *moved.accounting();
        let fuel = journal.context().ordinary().max_steps_per_stage().unwrap() as u64;
        let selected = moved
            .prepare_continue()
            .unwrap_or_else(|_| panic!("actual Continue selection"));
        let current = ack(journal, selected)
            .advance_continue()
            .unwrap_or_else(|_| panic!("actual StateCommitted"));
        let LiveContinueAcknowledgedV8::State(state) = current else {
            panic!("State owner")
        };
        let after_state = journal.begin_session().unwrap();
        let (nr, ns, next, _) = after_state.inventory.continuation_facts().unwrap();
        assert_eq!((nr, ns, next), (r, s, turn + 1));
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        let observed = ack(
            journal,
            state
                .prepare_observe()
                .unwrap_or_else(|_| panic!("fullF Observe obligation")),
        )
        .advance_continue()
        .unwrap_or_else(|_| panic!("sole actual Observe"));
        let LiveContinueAcknowledgedV8::Observed(observed) = observed else {
            panic!("actual observed/failed owner")
        };
        assert!(observed.is_observed());
        assert_eq!(observed.test_observation(), &ordinary_observation);
        assert_eq!(observed.consumed(), ordinary_consumed);
        assert_eq!(observed.turn(), turn + 1);
        assert_eq!(observed.accounting(), &ledger);
        assert!(observed.consumed() > 0 && observed.consumed() <= fuel as usize);
        let after = journal.begin_session().unwrap();
        let (nr, ns, _, _) = after.inventory.continuation_facts().unwrap();
        assert_eq!((nr, ns), (r + fuel, s + 1));
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(observed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_continue_cancel_or_expired_clock_before_ack_keeps_real_state_and_zero_new_stage() {
    for expired in [false, true] {
        with_moved(|journal, moved, weak, cancel, clock| {
            let before = journal.begin_session().unwrap();
            let seq = before.sequence();
            let original = moved
                .prepare_continue()
                .unwrap_or_else(|_| panic!("live selector"));
            if expired {
                clock.now.set(
                    journal
                        .context()
                        .ordinary()
                        .deadline_millis()
                        .checked_add(1)
                        .unwrap(),
                );
            } else {
                cancel.cancel();
            }
            let failure = before
                .append_owned_continue(original)
                .err()
                .expect("actual prewrite guard refusal");
            assert!(matches!(
                failure,
                LiveOwnedContinueAppendFailureV8::Before { .. }
            ));
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(seq > 0);
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
#[cfg(unix)]
fn owned_continue_state_and_observe_real_append_faults_never_evaluate_or_remint() {
    for observe in [false, true] {
        for after in [false, true] {
            with_moved(|journal, moved, weak, _, _| {
                let entries_before = crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8();
                let mut obligation = moved
                    .prepare_continue()
                    .unwrap_or_else(|_| panic!("State selection"));
                if observe {
                    let LiveContinueAcknowledgedV8::State(state) = ack(journal, obligation)
                        .advance_continue()
                        .unwrap_or_else(|_| panic!("State ACK"))
                    else {
                        panic!()
                    };
                    obligation = state
                        .prepare_observe()
                        .unwrap_or_else(|_| panic!("Observe original selection"));
                }
                {
                    let number = obligation.sequence() + 1;
                    let mut lease = journal.lease.borrow_mut();
                    if after {
                        lease.test_fail_after_sync(number)
                    } else {
                        lease.test_fail_before_write(number)
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continue(obligation)
                    .err()
                    .expect("actual persistence failure");
                assert_eq!(crate::interpreter::resumable::owned_frame::registered_stage::reduce::test_continue_observe_entries_v8(), entries_before);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}

#[test]
fn owned_continue_failed_observe_retains_actual_state_and_observed_consumption() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_continued_observe_ensures_store(
        |context, lease, key, _| {
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
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = AgentCancellation::new();
            let policy = CapabilityPolicy::new(vec!["read".into()]).unwrap();
            let clock = Clock { now: Cell::new(1) };
            super::super::tests::test_moved(&journal, &cancel, &policy, &clock, |moved, weak| {
                let ledger = *moved.accounting();
                let LiveContinueAcknowledgedV8::State(state) = ack(
                    &journal,
                    moved
                        .prepare_continue()
                        .unwrap_or_else(|_| panic!("Continue")),
                )
                .advance_continue()
                .unwrap_or_else(|_| panic!("State ACK")) else {
                    panic!("State")
                };
                let LiveContinueAcknowledgedV8::Observed(failed) = ack(
                    &journal,
                    state
                        .prepare_observe()
                        .unwrap_or_else(|_| panic!("Observe reservation")),
                )
                .advance_continue()
                .unwrap_or_else(|_| panic!("actual failed Observe retained")) else {
                    panic!("Observe outcome")
                };
                assert!(failed.is_failed());
                assert!(!failed.is_observed());
                assert_eq!(failed.turn(), 1);
                assert!(failed.consumed() > 0);
                assert_eq!(failed.accounting(), &ledger);
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        },
    );
}
