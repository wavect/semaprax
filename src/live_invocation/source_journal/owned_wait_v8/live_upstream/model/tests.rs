//! Real offline SDK dispatch and real same-FD ACKs. No trusted ACK fixture.
use super::*;
use crate::agent_lifecycle::iterative::source_live::SourceProposalPolicy;
use crate::provider_adapter_sdk::adapter::{
    AdapterEvent, AdapterPoll, AdapterRefusal, AdapterRequest, AdapterSettlement,
};
use crate::provider_adapter_sdk::capability::AdapterCapabilities;
use crate::provider_adapter_sdk::fixture_adapters::{base_capabilities, usage};
use crate::provider_adapter_sdk::{AdapterInvocationCapability, ProviderAdapter};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Arc};
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
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
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
fn park<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j crate::agent_runtime::AgentCancellation,
) -> ParkedLiveOwnedRunV8<'j> {
    let input = super::super::tests::input(journal.context());
    let initialized = match initialize_live_actor_v8(journal, input, cancel) {
        Ok(Ok(x)) => x,
        _ => panic!("real Initialize"),
    };
    let observed = match super::super::observe::observe_live_actor_v8(initialized) {
        Ok(x) => x,
        Err(_) => panic!("real Observe"),
    };
    match super::super::wait::start_live_actor_v8(observed) {
        Ok(x) => x,
        Err(_) => panic!("real helper"),
    }
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
#[test]
fn owned_wait_live_model_real_sdk_settlement_usage_and_resume_keep_same_owner() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let response = document(&context);
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let parked = park(&journal, &cancel);
        let weak = parked.owner.test_weak();
        assert_eq!(weak.len(), 1, "real nonempty owned State leaf inventory");
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
        let mut source = source(journal.context(), &mut factory);
        let completed = match model_live_actor_v8(parked, &mut source, &Clock) {
            Ok(x) => x,
            Err(f) => panic!(
                "real model/Resume {:?} {:?} {:?}",
                f.error, f.reason, f.diagnostics
            ),
        };
        assert_eq!(completed.completed, 14);
        assert_eq!(completed.session.sequence(), 15);
        assert!(completed.owner.consumed() > 0);
        let actual = completed.owner.test_weak();
        assert!(actual.iter().all(
            |w| w.strong_count() == 1 && weak.iter().any(|old| std::sync::Weak::ptr_eq(old, w))
        ));
        assert_eq!(
            (
                counts.borrow().factories,
                counts.borrow().starts,
                counts.borrow().polls
            ),
            (1, 1, 3)
        );
        let prompt: serde_json::Value =
            serde_json::from_slice(&counts.borrow().requests[0]).unwrap();
        assert_eq!(prompt["schema"], "semaprax.source-adapter-prompt.v1");
        assert_eq!(
            prompt["task_hex"],
            crate::live_invocation::identity::hex(b"owned task")
        );
        let expected_state = crate::resumable_effects::owned_frame::v2::ordinary_state_bytes(
            journal.context().test_runtime_execution().1.wait(),
            &completed
                .owner
                .checked_facts(journal.context().test_runtime_execution().1.wait())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            prompt["state"],
            serde_json::from_str::<serde_json::Value>(&expected_state).unwrap()
        );
        assert!(
            std::str::from_utf8(&counts.borrow().requests[0])
                .unwrap()
                .contains(&format!(",\"state\":{expected_state},\"observation\":")),
            "actual SDK prompt embeds the exact frozen declaration-order State bytes"
        );
        let folded = completed.session.fold_for_live_test();
        assert_eq!(
            folded.reserved_total,
            4 * journal
                .context()
                .test_runtime_execution()
                .1
                .evaluation_fuel() as u64
        );
        assert_eq!(folded.stages, 2);
        journal.begin_session().unwrap();
        drop(completed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
#[test]
fn owned_wait_live_model_ack_faults_never_dispatch_before_intent_or_resume_before_reservation() {
    for append in [11, 12, 13, 14, 15] {
        for persisted in [false, true] {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
                |context, mut lease, key| {
                    let response = document(&context);
                    let context = context.with_initialization(&lease).unwrap();
                    if persisted {
                        lease.test_fail_after_write(append)
                    } else {
                        lease.test_fail_before_write(append)
                    };
                    let journal =
                        SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                    let cancel = crate::agent_runtime::AgentCancellation::new();
                    let parked = park(&journal, &cancel);
                    let weak = parked.owner.test_weak();
                    let counts = Rc::new(RefCell::new(Counts::default()));
                    let mut factory =
                        factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
                    let mut source = source(journal.context(), &mut factory);
                    let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
                        Err(f) => f,
                        Ok(_) => panic!("failed ACK"),
                    };
                    assert_eq!(failed.error, SourceJournalError::Uncertain);
                    assert_eq!(counts.borrow().starts, usize::from(append > 11));
                    match &failed.owner {
                        LiveModelFailureOwnerV8::Parked(_) => assert!(append <= 14),
                        LiveModelFailureOwnerV8::Resume(_) => assert_eq!(append, 15),
                    }
                    assert!(weak.iter().all(|w| w.strong_count() == 1));
                    assert_eq!(
                        failed.held.validate_guard(),
                        Err(SourceJournalError::Poisoned)
                    );
                    drop(failed);
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
            );
        }
    }
}
#[test]
fn owned_wait_live_model_callbacks_cancel_before_further_provider_or_resume_work() {
    for phase in ["factory", "caps", "start", "poll"] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let response = document(&context);
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let parked = park(&journal, &cancel);
            let weak = parked.owner.test_weak();
            let trigger = cancel.clone();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(
                Rc::clone(&counts),
                script(&response),
                Rc::new(move |at| {
                    if at == phase {
                        trigger.cancel()
                    }
                }),
            );
            let mut source = source(journal.context(), &mut factory);
            let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
                Err(f) => f,
                Ok(_) => panic!("callback cancel"),
            };
            assert_eq!(
                failed.reason,
                Some(super::super::super::super::SourceAttemptFailure::Cancelled)
            );
            assert!(matches!(failed.owner, LiveModelFailureOwnerV8::Parked(_)));
            assert_eq!(journal.begin_session().unwrap().sequence(), 13);
            assert_eq!(counts.borrow().polls, usize::from(phase == "poll"));
            assert_eq!(
                counts.borrow().starts,
                usize::from(phase == "poll" || phase == "start")
            );
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_wait_live_model_malformed_sdk_is_exact_failed_settlement_without_resume() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let parked = park(&journal, &cancel);
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(Rc::clone(&counts), script(b"malformed"), Rc::new(|_| {}));
        let mut source = source(journal.context(), &mut factory);
        let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
            Err(f) => f,
            Ok(_) => panic!("malformed SDK"),
        };
        assert_eq!(
            failed.reason,
            Some(super::super::super::super::SourceAttemptFailure::MalformedResponse)
        );
        assert_eq!(counts.borrow().polls, 1);
        assert_eq!(journal.begin_session().unwrap().sequence(), 13);
        assert_eq!(failed.diagnostics.len(), 1);
        assert_eq!(failed.diagnostics[0].code, "source.adapter_decode");
    });
}
#[test]
fn owned_wait_live_model_guard_authenticates_tail_before_next_poll_and_keeps_abort_primary() {
    for phase in ["poll", "cancel"] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime_store(
            true,
            |context, lease, key, directory| {
                let response = if phase == "cancel" {
                    b"malformed".to_vec()
                } else {
                    document(&context)
                };
                let context = context.with_initialization(&lease).unwrap();
                let journal =
                    SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                let cancel = crate::agent_runtime::AgentCancellation::new();
                let parked = park(&journal, &cancel);
                let weak = parked.owner.test_weak();
                let path = std::fs::read_dir(directory)
                    .unwrap()
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .find(|p| {
                        p.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .ends_with(".source-owned-wait.jsonl")
                    })
                    .expect("held source journal");
                let counts = Rc::new(RefCell::new(Counts::default()));
                let action = Rc::new(move |at: &str| {
                    if at == phase {
                        use std::io::Write;
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(&path)
                            .unwrap()
                            .write_all(b"tampered\n")
                            .unwrap();
                    }
                });
                let mut factory = factory(Rc::clone(&counts), script(&response), action);
                let mut source = source(journal.context(), &mut factory);
                let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
                    Err(f) => f,
                    Ok(_) => panic!("callback history drift"),
                };
                assert_eq!(counts.borrow().polls, 1);
                assert!(failed.held.validate_guard().is_err());
                if phase == "cancel" {
                    assert_eq!(
                        failed.reason,
                        Some(super::super::super::super::SourceAttemptFailure::MalformedResponse)
                    );
                }
                assert!(weak.iter().all(|w| w.strong_count() == 1));
                drop(failed);
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
        );
    }
}
#[test]
fn owned_wait_live_model_cancel_callback_panic_cannot_replace_decoder_primary() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let parked = park(&journal, &cancel);
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(
            Rc::clone(&counts),
            script(b"malformed"),
            Rc::new(|at| {
                if at == "cancel" {
                    panic!("cancel observation")
                }
            }),
        );
        let mut source = source(journal.context(), &mut factory);
        let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
            Err(f) => f,
            Ok(_) => panic!("decoder refusal"),
        };
        assert_eq!(
            failed.reason,
            Some(super::super::super::super::SourceAttemptFailure::MalformedResponse)
        );
        assert_eq!(counts.borrow().polls, 1);
        assert_eq!(
            failed.held.validate_guard(),
            Err(SourceJournalError::Poisoned)
        );
    });
}
#[test]
fn owned_wait_live_model_noncheckpointed_profile_refuses_before_intent_and_factory() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let parked = park(&journal, &cancel);
        let weak = parked.owner.test_weak();
        let (_, e) = journal.context().test_runtime_execution();
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(Rc::clone(&counts), Vec::new(), Rc::new(|_| {}));
        let mut source = StreamingSourceProposalAdapter::new_bound(
            &mut factory,
            AdapterInvocationCapability::grant("negative"),
            e.wait().lifecycle().proposal_schema(),
            e.model().clone(),
            e.model().invocation_capability(),
        )
        .unwrap();
        let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
            Err(f) => f,
            Ok(_) => panic!("noncheckpointed"),
        };
        assert_eq!(journal.begin_session().unwrap().sequence(), 10);
        assert_eq!(counts.borrow().factories, 0);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(failed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}
struct CancellingClock {
    cancel: crate::agent_runtime::AgentCancellation,
    calls: std::cell::Cell<usize>,
}
impl crate::live_invocation::InvocationClock for CancellingClock {
    fn now_millis(&self) -> i64 {
        let next = self.calls.get() + 1;
        self.calls.set(next);
        if next == 2 {
            self.cancel.cancel()
        }
        1
    }
}
impl SourceInvocationClock for CancellingClock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_model_clock_callback_cancel_stops_before_factory() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let response = document(&context);
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let parked = park(&journal, &cancel);
        let clock = CancellingClock {
            cancel: cancel.clone(),
            calls: std::cell::Cell::new(0),
        };
        let counts = Rc::new(RefCell::new(Counts::default()));
        let mut factory = factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
        let mut source = source(journal.context(), &mut factory);
        let failed = match model_live_actor_v8(parked, &mut source, &clock) {
            Err(f) => f,
            Ok(_) => panic!("clock cancelled"),
        };
        assert_eq!(
            failed.reason,
            Some(super::super::super::super::SourceAttemptFailure::Cancelled)
        );
        assert_eq!(counts.borrow().factories, 0);
        assert_eq!(clock.calls.get(), 2);
        assert_eq!(journal.begin_session().unwrap().sequence(), 13);
    });
}
#[test]
fn owned_wait_live_model_usage_returned_with_cancellation_is_recorded_without_another_poll() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let parked = park(&journal, &cancel);
        let trigger = cancel.clone();
        let counts = Rc::new(RefCell::new(Counts::default()));
        let events = vec![AdapterPoll::Event(AdapterEvent::Usage {
            tokens_in: 2,
            tokens_out: 3,
            cost_micros: 1,
        })];
        let mut factory = factory(
            Rc::clone(&counts),
            events,
            Rc::new(move |at| {
                if at == "poll" {
                    trigger.cancel()
                }
            }),
        );
        let mut source = source(journal.context(), &mut factory);
        let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
            Err(f) => f,
            Ok(_) => panic!("cancelled usage callback"),
        };
        assert_eq!(
            failed.reason,
            Some(super::super::super::super::SourceAttemptFailure::Cancelled)
        );
        assert_eq!(failed.usage, reported_usage(Some((2, 3, 1))));
        assert_eq!(counts.borrow().polls, 1);
        assert_eq!(journal.begin_session().unwrap().sequence(), 13);
    });
}
struct JournalDeadlineClock<'a> {
    journal: &'a SourceOwnedWaitJournalV8,
    expire_at: usize,
}
impl crate::live_invocation::InvocationClock for JournalDeadlineClock<'_> {
    fn now_millis(&self) -> i64 {
        if self.journal.begin_session().unwrap().sequence() >= self.expire_at {
            1000
        } else {
            1
        }
    }
}
impl SourceInvocationClock for JournalDeadlineClock<'_> {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_model_late_deadline_blocks_resume_reservation_or_source_entry() {
    for expire_at in [13, 14] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let response = document(&context);
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let parked = park(&journal, &cancel);
            let weak = parked.owner.test_weak();
            let clock = JournalDeadlineClock {
                journal: &journal,
                expire_at,
            };
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
            let mut source = source(journal.context(), &mut factory);
            let failed = match model_live_actor_v8(parked, &mut source, &clock) {
                Err(f) => f,
                Ok(_) => panic!("expired after raw settlement ACK"),
            };
            assert_eq!(counts.borrow().starts, 1);
            assert_eq!(journal.begin_session().unwrap().sequence(), expire_at);
            if expire_at == 13 {
                assert_eq!(failed.error, SourceJournalError::Time);
                assert!(matches!(&failed.owner, LiveModelFailureOwnerV8::Parked(_)));
            } else {
                assert!(matches!(
                    &failed.owner,
                    LiveModelFailureOwnerV8::Resume(LiveWaitResumeOutcomeV8::Refused(_))
                ));
            }
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_wait_live_model_each_external_adapter_panic_retains_owner_and_permanently_quarantines() {
    for phase in ["factory", "caps", "start", "poll"] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let response = document(&context);
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let parked = park(&journal, &cancel);
            let weak = parked.owner.test_weak();
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(
                Rc::clone(&counts),
                script(&response),
                Rc::new(move |at| {
                    if at == phase {
                        panic!("external callback {phase}")
                    }
                }),
            );
            let mut source = source(journal.context(), &mut factory);
            let failed = match model_live_actor_v8(parked, &mut source, &Clock) {
                Err(f) => f,
                Ok(_) => panic!("panicked callback"),
            };
            assert_eq!(failed.error, SourceJournalError::Poisoned);
            assert!(matches!(&failed.owner, LiveModelFailureOwnerV8::Parked(_)));
            assert_eq!(counts.borrow().polls, usize::from(phase == "poll"));
            assert_eq!(journal.hold().err(), Some(SourceJournalError::Poisoned));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
struct PanickingClock {
    phase: &'static str,
    calls: std::cell::Cell<usize>,
}
impl crate::live_invocation::InvocationClock for PanickingClock {
    fn now_millis(&self) -> i64 {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if self.phase == "now" || (self.phase == "sdk_now" && n == 2) {
            panic!("host clock callback")
        }
        1
    }
}
impl SourceInvocationClock for PanickingClock {
    fn clock_domain(&self) -> &str {
        if self.phase == "domain" {
            panic!("host clock domain callback")
        }
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_model_clock_panics_before_and_after_intent_preserve_real_owner() {
    for phase in ["domain", "now", "sdk_now"] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let response = document(&context);
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let parked = park(&journal, &cancel);
            let weak = parked.owner.test_weak();
            let clock = PanickingClock {
                phase,
                calls: std::cell::Cell::new(0),
            };
            let counts = Rc::new(RefCell::new(Counts::default()));
            let mut factory = factory(Rc::clone(&counts), script(&response), Rc::new(|_| {}));
            let mut source = source(journal.context(), &mut factory);
            let failed = match model_live_actor_v8(parked, &mut source, &clock) {
                Err(f) => f,
                Ok(_) => panic!("clock panic"),
            };
            assert_eq!(failed.error, SourceJournalError::Poisoned);
            assert_eq!(counts.borrow().factories, 0);
            assert_eq!(journal.hold().err(), Some(SourceJournalError::Poisoned));
            assert!(weak.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(weak.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

/// A real model/ACK/source fixture, not a trusted ACK or owner constructor.
pub(in crate::live_invocation::source_journal::owned_wait_v8::live_upstream) fn completed_test_actor<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn SourceInvocationClock,
    budget: i64,
) -> CompletedLiveOwnedRunV8<'j> {
    let response = String::from_utf8(document(journal.context()))
        .unwrap()
        .replace(
            "proposal.budget\":\"3",
            &format!("proposal.budget\":\"{budget}"),
        );
    let parked = park(journal, cancel);
    let counts = Rc::new(RefCell::new(Counts::default()));
    let mut factory = factory(
        Rc::clone(&counts),
        script(response.as_bytes()),
        Rc::new(|_| {}),
    );
    let mut adapter = source(journal.context(), &mut factory);
    let completed = model_live_actor_v8(parked, &mut adapter, clock).unwrap_or_else(|failed| {
        panic!(
            "real SDK/Resume {:?} {:?}",
            failed.error, failed.diagnostics
        )
    });
    assert_eq!(
        (
            counts.borrow().factories,
            counts.borrow().starts,
            counts.borrow().polls
        ),
        (1, 1, 3)
    );
    completed
}
