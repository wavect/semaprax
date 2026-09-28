//! Real continued SDK and Resume: one owner, ledger, token and ACK history.
use super::super::super::super::super::tests::{ack, with_continued};
use super::*;
use crate::agent_lifecycle::iterative::source_live::SourceProposalPolicy;
use crate::provider_adapter_sdk::adapter::{
    AdapterEvent, AdapterPoll, AdapterRefusal, AdapterRequest, AdapterSettlement,
};
use crate::provider_adapter_sdk::capability::AdapterCapabilities;
use crate::provider_adapter_sdk::fixture_adapters::{base_capabilities, usage};
use crate::provider_adapter_sdk::{
    AdapterInvocationCapability, ProviderAdapter, StreamingSourceProposalAdapter,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
#[derive(Default)]
struct Counts {
    factories: usize,
    caps: usize,
    starts: usize,
    polls: usize,
    cancels: usize,
    requests: Vec<Vec<u8>>,
}
struct Probe {
    counts: Rc<RefCell<Counts>>,
    script: VecDeque<AdapterPoll>,
    caps: AdapterCapabilities,
    action: Rc<dyn Fn(&str)>,
}
impl ProviderAdapter for Probe {
    fn capabilities(&self) -> &AdapterCapabilities {
        self.counts.borrow_mut().caps += 1;
        (self.action)("caps");
        &self.caps
    }
    fn start(
        &mut self,
        _: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        self.counts.borrow_mut().starts += 1;
        self.counts
            .borrow_mut()
            .requests
            .push(request.request_bytes.clone());
        (self.action)("start");
        Ok(())
    }
    fn poll(&mut self) -> AdapterPoll {
        self.counts.borrow_mut().polls += 1;
        let next = self
            .script
            .pop_front()
            .expect("no work after final SDK response");
        (self.action)("poll");
        next
    }
    fn cancel(&mut self, _: &str) {
        self.counts.borrow_mut().cancels += 1;
        (self.action)("cancel");
    }
}
fn document(context: &CheckedOwnedWaitJournalContextV8) -> Vec<u8> {
    let (_, e) = context.test_runtime_execution();
    format!("{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":\"{}\",\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"3\",\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n",e.wait().lifecycle().proposal_schema().schema().digest()).into_bytes()
}
fn script(response: &[u8]) -> Vec<AdapterPoll> {
    vec![
        AdapterPoll::Event(AdapterEvent::Delta(response.to_vec())),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Settled(AdapterSettlement {
            response_bytes: response.to_vec(),
            usage: usage(2, 3, 1),
        }),
    ]
}
fn source<'a>(
    context: &'a CheckedOwnedWaitJournalContextV8,
    factory: &'a mut dyn crate::provider_adapter_sdk::SourceAdapterFactory,
) -> StreamingSourceProposalAdapter<'a> {
    let (_, e) = context.test_runtime_execution();
    let model = e.model();
    StreamingSourceProposalAdapter::new_bound_checkpointed(
        factory,
        AdapterInvocationCapability::grant("offline actual model seam"),
        e.wait().lifecycle().proposal_schema(),
        model.clone(),
        model.invocation_capability(),
        SourceProposalPolicy {
            deployment_binding: model.digest(),
            response_limit: e.ordinary().response_limit(),
            reservation_units: e.ordinary().reservation_units(),
        },
    )
    .unwrap()
}
fn factory(
    counts: Rc<RefCell<Counts>>,
    script: Vec<AdapterPoll>,
    action: Rc<dyn Fn(&str)>,
) -> impl crate::provider_adapter_sdk::SourceAdapterFactory {
    move || {
        counts.borrow_mut().factories += 1;
        action("factory");
        let mut caps = base_capabilities("owned-wait-inert-test", true);
        caps.max_request_bytes = 65_536;
        Box::new(Probe {
            counts: Rc::clone(&counts),
            script: script.clone().into(),
            caps,
            action: Rc::clone(&action),
        }) as Box<dyn ProviderAdapter>
    }
}
fn prepared<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedObserveSettlementAppendV8<'j>,
) -> LiveContinuedPreparedPhaseV8<'j> {
    let settled = ack(journal, owner);
    let observed = settled
        .prepare_turn_observed()
        .unwrap_or_else(|_| panic!("TurnObserved"));
    let carried = ack(journal, observed)
        .into_continued_wait()
        .unwrap_or_else(|_| panic!("same actual State"));
    let created = carried
        .prepare_start_created()
        .unwrap_or_else(|_| panic!("Created"));
    let created = journal
        .begin_session()
        .unwrap()
        .append_owned_continued_start(created)
        .unwrap_or_else(|_| panic!("Created ACK"))
        .advance_continued_start()
        .unwrap_or_else(|_| panic!("Created advance"));
    let reserved = created
        .prepare_start_reservation()
        .unwrap_or_else(|_| panic!("Start F"));
    let reserved = journal
        .begin_session()
        .unwrap()
        .append_owned_continued_start(reserved)
        .unwrap_or_else(|_| panic!("Start ACK"))
        .advance_continued_start()
        .unwrap_or_else(|_| panic!("Start advance"));
    let actual = reserved
        .enter_actual_source()
        .unwrap_or_else(|_| panic!("sole actual Start"));
    let prepared = actual
        .prepare_checkpoint()
        .unwrap_or_else(|_| panic!("actual checkpoint"));
    journal
        .begin_session()
        .unwrap()
        .append_owned_continued_prepared(prepared)
        .unwrap_or_else(|_| panic!("Prepared ACK"))
        .advance_continued_prepared()
        .unwrap_or_else(|_| panic!("Prepared advance"))
}
fn acknowledge<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    selected: LiveOwnedContinuedModelAppendV8<'j>,
) -> LiveContinuedModelV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_continued_model(selected)
        .unwrap_or_else(|_| panic!("fixed model ACK"))
        .advance_continued_model()
        .unwrap_or_else(|_| panic!("true model successor"))
}
fn resume_entries() -> usize {
    crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_resume_entries_v8()
}
#[test]
fn owned_continued_model_real_sdk_resume_completed_keeps_owner_and_all_ledger_dimensions() {
    with_continued(false, |journal, owner, weak, ledger, _, _, _| {
        let parked = prepared(journal, owner);
        let state = parked.owner.owner.test_ordinary_start().0;
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(
            counts.clone(),
            script(&document(journal.context())),
            Rc::new(|_| {}),
        );
        let mut adapter = source(journal.context(), &mut factory);
        let entry = resume_entries();
        let sequence = parked.session.sequence();
        let selected = parked
            .prepare_model_intent(&adapter)
            .unwrap_or_else(|_| panic!("bound actual request"));
        assert_eq!(counts.borrow().factories, 0);
        assert_eq!(selected.owner.ordinal, 1);
        let intent = acknowledge(journal, selected);
        assert_eq!(resume_entries(), entry);
        let settled = intent
            .dispatch_model(&mut adapter)
            .unwrap_or_else(|_| panic!("sole SDK call"));
        assert_eq!(counts.borrow().requests.len(), 1);
        let request = counts.borrow().requests[0].clone();
        let prompt: serde_json::Value = serde_json::from_slice(&request).unwrap();
        assert_eq!(prompt["turn"], 1);
        assert!(prompt["previous_effect_hex"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert_eq!(settled.owner.accounting(), &ledger);
        let settled = acknowledge(
            journal,
            settled.prepare_next().unwrap_or_else(|_| panic!("Settled")),
        );
        let usage = acknowledge(
            journal,
            settled.prepare_next().unwrap_or_else(|_| panic!("Usage")),
        );
        let selected = usage
            .prepare_next()
            .unwrap_or_else(|_| panic!("full Resume F"));
        assert!(
            matches!(selected.selected(),EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved{turn:1,phase:journal_model::PhaseV8::Resume,replay_of:None,fuel,..})if *fuel==journal.context().ready_runtime().unwrap().1.evaluation_fuel() as u64)
        );
        assert_eq!(
            resume_entries(),
            entry,
            "no evaluation before true full-F ACK"
        );
        let reserved = acknowledge(journal, selected);
        let actual = reserved
            .resume_actual()
            .unwrap_or_else(|_| panic!("actual Resume"));
        assert_eq!(resume_entries(), entry + 1);
        assert_eq!(actual.owner.accounting(), &ledger);
        let completed = actual
            .prepare_next()
            .unwrap_or_else(|_| panic!("Completed"));
        assert!(
            matches!(completed.selected(),EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitCompleted{turn:1,consumed,..})if *consumed>0)
        );
        let completed = acknowledge(journal, completed);
        assert_eq!(completed.session().sequence(), sequence + 5);
        assert_eq!(completed.owner.accounting(), &ledger);
        let ModelOwnerV8::Resumed(resumed) = &completed.owner else {
            panic!("actual terminal State")
        };
        let binding = journal.context().ready_runtime().unwrap().1.wait();
        assert_eq!(resumed.owner.checked_model_facts(binding), Some(state));
        assert_eq!(counts.borrow().factories, 1);
        assert_eq!(counts.borrow().starts, 1);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(completed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[cfg(unix)]
#[test]
fn owned_continued_model_ack_faults_never_repeat_sdk_or_resume() {
    for row in 0..5 {
        for mode in 0..4 {
            with_continued(false, |journal, owner, weak, ledger, _, _, _| {
                let parked = prepared(journal, owner);
                let counts = Rc::new(RefCell::new(Counts::default()));
                let mut factory = factory(
                    counts.clone(),
                    script(&document(journal.context())),
                    Rc::new(|_| {}),
                );
                let mut adapter = source(journal.context(), &mut factory);
                let mut selected = parked
                    .prepare_model_intent(&adapter)
                    .unwrap_or_else(|_| panic!("Intent"));
                if row > 0 {
                    let mut owner = acknowledge(journal, selected)
                        .dispatch_model(&mut adapter)
                        .unwrap_or_else(|_| panic!("SDK"));
                    selected = owner.prepare_next().unwrap_or_else(|_| panic!("Settled"));
                    if row >= 2 {
                        owner = acknowledge(journal, selected);
                        selected = owner.prepare_next().unwrap_or_else(|_| panic!("Usage"));
                    }
                    if row >= 3 {
                        owner = acknowledge(journal, selected);
                        selected = owner.prepare_next().unwrap_or_else(|_| panic!("Resume"));
                    }
                    if row == 4 {
                        selected = acknowledge(journal, selected)
                            .resume_actual()
                            .unwrap_or_else(|_| panic!("actual Resume"))
                            .prepare_next()
                            .unwrap_or_else(|_| panic!("Completed"));
                    }
                }
                assert_eq!(selected.owner.owner.accounting(), &ledger);
                let calls = counts.borrow().polls;
                let entries = resume_entries();
                let number = selected.sequence() + 1;
                {
                    let mut lease = journal.test_observe_lease().borrow_mut();
                    match mode {
                        0 => lease.test_fail_before_write(number),
                        1 => lease.test_fail_after_write(number),
                        2 => lease.test_fail_before_sync(number),
                        _ => lease.test_fail_after_sync(number),
                    }
                }
                let failure = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_model(selected)
                    .err()
                    .expect("real persistence uncertainty");
                assert!(
                    failure.test_is_in_doubt(),
                    "row {row}, physical window {mode} must reach InDoubt"
                );
                assert_eq!(counts.borrow().polls, calls);
                assert_eq!(resume_entries(), entries);
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                drop(failure);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            });
        }
    }
}
#[test]
fn owned_continued_model_external_callback_panics_quarantine_same_owner() {
    for phase in ["factory", "caps", "start", "poll"] {
        with_continued(false, |journal, owner, weak, ledger, _, _, _| {
            let parked = prepared(journal, owner);
            let counts = Rc::new(RefCell::new(Counts::default()));
            let action = Rc::new(move |at: &str| {
                if at == phase {
                    panic!("actual {phase} callback")
                }
            });
            let mut factory = factory(counts.clone(), script(&document(journal.context())), action);
            let mut adapter = source(journal.context(), &mut factory);
            let selected = parked
                .prepare_model_intent(&adapter)
                .unwrap_or_else(|_| panic!("Intent"));
            let intent = acknowledge(journal, selected);
            let entries = resume_entries();
            let failure = intent
                .dispatch_model(&mut adapter)
                .err()
                .expect("caught callback keeps actual owner");
            assert_eq!(failure.owner.owner.accounting(), &ledger);
            assert_eq!(resume_entries(), entries);
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            drop(failure);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_continued_model_cancel_after_poll_keeps_selected_primary_and_no_resume() {
    with_continued(false, |journal, owner, weak, ledger, _, _, cancel| {
        let parked = prepared(journal, owner);
        let counts = Rc::new(RefCell::new(Counts::default()));
        let trigger = cancel.clone();
        let action = Rc::new(move |at: &str| {
            if at == "poll" {
                trigger.cancel();
            }
        });
        let mut factory = factory(counts.clone(), script(&document(journal.context())), action);
        let mut adapter = source(journal.context(), &mut factory);
        let selected = parked
            .prepare_model_intent(&adapter)
            .unwrap_or_else(|_| panic!("Intent"));
        let entries = resume_entries();
        let owner = acknowledge(journal, selected)
            .dispatch_model(&mut adapter)
            .unwrap_or_else(|_| panic!("failed SDK settlement retained"));
        assert!(matches!(
            &owner.dispatched,
            Some(OwnedModelSettlementV8::Failed {
                reason: SourceAttemptFailure::Cancelled,
                ..
            })
        ));
        let owner = acknowledge(
            journal,
            owner
                .prepare_next()
                .unwrap_or_else(|_| panic!("actual Failed")),
        );
        let owner = acknowledge(
            journal,
            owner
                .prepare_next()
                .unwrap_or_else(|_| panic!("actual Usage")),
        );
        let failure = owner
            .prepare_next()
            .err()
            .expect("no Resume after actual SDK failure");
        assert_eq!(failure.owner.owner.accounting(), &ledger);
        assert_eq!(resume_entries(), entries);
        assert_eq!(counts.borrow().polls, 1);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(failure);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_continued_model_accounting_proof_is_exact_current_context_prefix() {
    with_continued(false, |journal, owner, weak, ledger, _, _, _| {
        let owner = prepared(journal, owner);
        let session = journal.begin_session().unwrap();
        let inventory = session.test_observe_inventory();
        let mac = inventory.authentication_tail();
        assert!(inventory.test_model_accounting_matches(
            journal.context(),
            session.acknowledged_bytes(),
            session.sequence(),
            mac
        ));
        assert!(!inventory.test_model_accounting_matches(
            journal.context(),
            session.acknowledged_bytes(),
            session.sequence(),
            &"0".repeat(64)
        ));
        assert!(!inventory.test_model_accounting_matches(
            journal.context(),
            session.acknowledged_bytes() + 1,
            session.sequence(),
            mac
        ));
        assert!(!inventory.test_model_accounting_matches(
            journal.context(),
            session.acknowledged_bytes(),
            session.sequence() + 1,
            mac
        ));
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|foreign, _, _| {
            assert!(!inventory.test_model_accounting_matches(
                &foreign,
                session.acknowledged_bytes(),
                session.sequence(),
                mac
            ));
        });
        let (_, _, checked) = session.continued_model_request_basis().unwrap();
        assert_eq!(checked, ledger);
        assert!(weak.iter().any(|w| w.strong_count() == 1));
        drop(owner);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_continued_model_cancel_does_not_mask_actual_tail_or_policy_loss() {
    for policy_loss in [false, true] {
        static DENIED: std::sync::OnceLock<crate::resumable_effects::CapabilityPolicy> =
            std::sync::OnceLock::new();
        let denied = DENIED
            .get_or_init(|| crate::resumable_effects::CapabilityPolicy::new(Vec::new()).unwrap());
        with_continued(false, |journal, owner, weak, ledger, _, _, cancel| {
            let parked = prepared(journal, owner);
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(
                counts.clone(),
                script(&document(journal.context())),
                Rc::new(|_| {}),
            );
            let adapter = source(journal.context(), &mut factory);
            let selected = parked
                .prepare_model_intent(&adapter)
                .unwrap_or_else(|_| panic!("true Intent"));
            let mut intent = acknowledge(journal, selected);
            if policy_loss {
                let ModelOwnerV8::Parked(parked) = &mut intent.owner else {
                    panic!("actual park");
                };
                parked.owner.owner.test_model_policy(denied);
            } else {
                journal
                    .test_observe_lease()
                    .borrow_mut()
                    .append(b"x")
                    .unwrap();
            }
            cancel.cancel();
            let entry = resume_entries();
            let permit = LiveContinuedModelIntentPermitV8 {
                owner: &intent,
                admission: std::cell::Cell::new(None),
            };
            assert!(permit.validate_guard().is_err());
            assert!(
                permit.validate_store().is_err(),
                "admission refusal cannot revive authority loss"
            );
            assert_eq!(intent.owner.accounting(), &ledger);
            assert_eq!(counts.borrow().factories, 0);
            assert_eq!(resume_entries(), entry);
            assert!(journal.hold().is_err());
            assert!(journal.begin_session().is_err());
            assert!(weak.iter().any(|w| w.strong_count() == 1));
            drop(intent);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
